//! MCP tool definitions for Ferris Aegis.
//!
//! This module provides both static tools (file_read) and dynamic tools
//! generated from SKILL.md specifications. Dynamic tools are registered
//! at runtime via the SkillMcpHandler.

use ferris_aegis_observability::CoreMetrics;
use ferris_aegis_skills::{McpToolDefinition, McpToolGenerator, SkillMcpHandler};
use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ServerInfo, ServerCapabilities, Implementation, ProtocolVersion, Tool};
use rmcp::{tool, tool_handler, tool_router, ServerHandler, ErrorData as McpError};
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::Instrument;

/// Parameters for the `file_read` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct FileReadParams {
    /// Absolute path to the file to read.
    #[schemars(description = "The absolute path to the file to read")]
    pub path: String,

    /// Maximum number of bytes to read. Defaults to 65536 (64 KiB).
    #[schemars(description = "Maximum bytes to read (default: 65536)")]
    pub max_bytes: Option<usize>,
}

/// Read a file from the local filesystem.
pub fn read_file_inner(path: &str, max_bytes: usize) -> Result<String, String> {
    let path = Path::new(path);

    // Security: reject non-absolute paths
    if !path.is_absolute() {
        return Err("Path must be absolute".to_string());
    }

    // Security: reject paths with directory traversal
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("Cannot resolve path: {e}"))?;

    if canonical.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err("Directory traversal not allowed".to_string());
    }

    let metadata = std::fs::metadata(&canonical)
        .map_err(|e| format!("Cannot access file: {e}"))?;

    if metadata.is_dir() {
        return Err("Path is a directory, not a file".to_string());
    }

    let file_size = metadata.len() as usize;
    let bytes_to_read = file_size.min(max_bytes);

    let content = std::fs::read(&canonical)
        .map_err(|e| format!("Failed to read file: {e}"))?;

    let truncated = content.len() > bytes_to_read;
    let content = if truncated { &content[..bytes_to_read] } else { &content };

    let text = String::from_utf8_lossy(content).to_string();

    let mut result = text;
    if truncated {
        result.push_str(&format!(
            "\n\n[TRUNCATED: showing {} of {} bytes]",
            bytes_to_read, file_size
        ));
    }

    Ok(result)
}

/// Dynamic tool handler for skills
struct DynamicSkillHandler {
    skill_handler: Arc<SkillMcpHandler>,
    tool_schemas: HashMap<String, Tool>,
}

impl DynamicSkillHandler {
    fn new(skill_handler: Arc<SkillMcpHandler>) -> Self {
        Self {
            skill_handler,
            tool_schemas: HashMap::new(),
        }
    }

    async fn refresh_tools(&mut self) {
        let tools = self.skill_handler.list_tools().await;
        self.tool_schemas.clear();
        for tool in tools {
            self.tool_schemas.insert(tool.name.clone(), tool);
        }
    }

    fn get_tool(&self, name: &str) -> Option<&Tool> {
        self.tool_schemas.get(name)
    }

    async fn call_tool(&self, name: &str, args: Value) -> Result<CallToolResult, McpError> {
        // Find which skill this tool corresponds to
        // Tool names are in format: category_name
        let skill_id = format!("skill:{}", name.replace('_', ":"));
        
        let result = self.skill_handler
            .execute_skill(&skill_id, args, None, None)
            .await
            .map_err(|e| McpError::internal_error(format!("Skill execution failed: {}", e), None))?;
        
        Ok(result)
    }
}

/// The Ferris Aegis MCP server with dynamic skill tool support.
#[derive(Clone)]
pub struct AegisMcpServer {
    /// The tool router for static tools
    tool_router: ToolRouter<Self>,
    /// Prometheus metrics handle
    metrics: CoreMetrics,
    /// Dynamic skill handler
    skill_handler: Option<Arc<RwLock<DynamicSkillHandler>>>,
}

impl AegisMcpServer {
    /// Create a new MCP server instance with the given metrics handle.
    pub fn new(metrics: CoreMetrics) -> Self {
        Self {
            tool_router: Self::tool_router(),
            metrics,
            skill_handler: None,
        }
    }

    /// Create a new MCP server with skill support.
    pub fn with_skills(
        metrics: CoreMetrics,
        skill_handler: Arc<SkillMcpHandler>,
    ) -> Self {
        let mut server = Self::new(metrics);
        server.skill_handler = Some(Arc::new(RwLock::new(DynamicSkillHandler::new(skill_handler))));
        server
    }

    /// Get the list of all tools (static + dynamic)
    pub async fn list_all_tools(&self) -> Vec<Tool> {
        let mut tools = vec![
            Tool {
                name: "file_read".to_string(),
                description: "Read a file from the local filesystem. Returns the file contents as text. Only absolute paths are accepted.".to_string(),
                input_schema: schema_for!(FileReadParams),
                output_schema: None,
                annotations: None,
            }
        ];

        if let Some(ref skill_handler) = self.skill_handler {
            let handler = skill_handler.read().await;
            tools.extend(handler.tool_schemas.values().cloned());
        }

        tools
    }

    /// Call a tool by name (static or dynamic)
    pub async fn call_tool_by_name(&self, name: &str, args: Value) -> Result<CallToolResult, McpError> {
        // Try static tools first
        if name == "file_read" {
            let params: FileReadParams = serde_json::from_value(args)
                .map_err(|e| McpError::invalid_params(format!("Invalid params: {}", e), None))?;
            return self.file_read(Parameters(params)).await;
        }

        // Try dynamic skill tools
        if let Some(ref skill_handler) = self.skill_handler {
            let handler = skill_handler.read().await;
            if handler.get_tool(name).is_some() {
                return handler.call_tool(name, args).await;
            }
        }

        Err(McpError::method_not_found(format!("Tool not found: {}", name), None))
    }

    /// Refresh dynamic tools from registry
    pub async fn refresh_skills(&self) {
        if let Some(ref skill_handler) = self.skill_handler {
            let mut handler = skill_handler.write().await;
            handler.refresh_tools().await;
        }
    }
}

#[tool_router]
impl AegisMcpServer {
    #[tool(description = "Read a file from the local filesystem. Returns the file contents as text. Only absolute paths are accepted.")]
    async fn file_read(
        &self,
        Parameters(params): Parameters<FileReadParams>,
    ) -> Result<CallToolResult, McpError> {
        let span = tracing::info_span!(
            "mcp.tool.file_read",
            path = %params.path,
            max_bytes = params.max_bytes.unwrap_or(65536),
        );

        async move {
            let max_bytes = params.max_bytes.unwrap_or(65536);

            match read_file_inner(&params.path, max_bytes) {
                Ok(content) => {
                    self.metrics.tool_ok("file_read");
                    tracing::debug!(
                        path = %params.path,
                        bytes = content.len(),
                        "file_read succeeded"
                    );
                    Ok(CallToolResult::success(vec![Content::text(content)]))
                }
                Err(err) => {
                    self.metrics.tool_error("file_read");
                    tracing::warn!(
                        path = %params.path,
                        error = %err,
                        "file_read failed"
                    );
                    Ok(CallToolResult::success(vec![Content::text(format!(
                        "Error: {err}"
                    ))]))
                }
            }
        }
        .instrument(span)
        .await
    }
}

// Custom ServerHandler implementation that supports dynamic tools
#[tool_handler]
impl ServerHandler for AegisMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_11_25,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation {
                name: "ferris-aegis".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                ..Default::default()
            },
            instructions: Some(
                "Ferris Aegis MCP Server — The Rust Guardian for Autonomous Intelligence.\n\
                 Available tools: file_read (read a file from the local filesystem) plus dynamically loaded skills.\n\
                 Security: only absolute paths accepted; directory traversal is rejected."
                    .to_string(),
            ),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_read_rejects_relative_path() {
        let result = read_file_inner("relative/path.txt", 65536);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("absolute"));
    }

    #[test]
    fn file_read_rejects_nonexistent_path() {
        let result = read_file_inner("/nonexistent/file.txt", 65536);
        assert!(result.is_err());
    }

    #[test]
    fn file_read_works_on_real_file() {
        let result = read_file_inner(file!(), 1024);
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("file_read_works_on_real_file"));
    }

    #[test]
    fn file_read_respects_max_bytes() {
        let result = read_file_inner(file!(), 10);
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("[TRUNCATED"));
    }
}
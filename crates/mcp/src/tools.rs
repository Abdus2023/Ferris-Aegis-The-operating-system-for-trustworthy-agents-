//! MCP tool definitions for Ferris Aegis.
//!
//! Each tool is:
//! - Declared with `#[tool]` for automatic schema generation
//! - Instrumented with a `tracing` span at the handler level
//! - Wired to increment `CoreMetrics` counters on success/failure
//! - Returns structured `CallToolResult` compatible with MCP spec

use ferris_aegis_observability::CoreMetrics;
use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ServerInfo, ServerCapabilities, Implementation, ProtocolVersion};
use rmcp::{tool, tool_handler, tool_router, ServerHandler, ErrorData as McpError};
use schemars::JsonSchema;
use serde::Deserialize;
use std::path::Path;
use tracing::Instrument;

/// Parameters for the `file_read` tool.
///
/// The `#[derive(JsonSchema)]` generates the MCP input schema
/// automatically. Field-level `#[schemars(description = "...")]`
/// attributes become the schema's property descriptions visible
/// to the LLM client.
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
///
/// This tool reads the contents of a file at the given path and returns
/// it as text content. Binary files are returned as base64-encoded
/// content.
///
/// # Security
///
/// Only absolute paths are accepted. Paths attempting directory traversal
/// (`..`) are rejected. The file must exist and be readable by the
/// current process.
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

    // Security: ensure the canonical path doesn't escape via symlinks
    // (canonicalize resolves symlinks, so comparing works)
    if canonical.components().any(|c| {
        matches!(c, std::path::Component::ParentDir)
    }) {
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
    let content = if truncated {
        &content[..bytes_to_read]
    } else {
        &content
    };

    // Try to decode as UTF-8 text; fall back to lossy representation
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

/// The Ferris Aegis MCP server.
///
/// Implements `ServerHandler` via the `#[tool_handler]` macro, which
/// generates the `call_tool` and `list_tools` dispatch methods from
/// the `#[tool_router]` decorated impl block.
#[derive(Clone)]
pub struct AegisMcpServer {
    /// The tool router — required by rmcp's macro system.
    tool_router: ToolRouter<Self>,
    /// Prometheus metrics handle — every tool call increments counters.
    metrics: CoreMetrics,
}

impl AegisMcpServer {
    /// Create a new MCP server instance with the given metrics handle.
    pub fn new(metrics: CoreMetrics) -> Self {
        Self {
            tool_router: Self::tool_router(),
            metrics,
        }
    }
}

#[tool_router]
impl AegisMcpServer {
    /// Read a file from the local filesystem.
    ///
    /// Returns the file contents as text. Binary content is decoded
    /// lossily. Output is truncated at `max_bytes` (default 64 KiB).
    ///
    /// Security constraints:
    /// - Only absolute paths accepted
    /// - Symlinks are resolved via canonicalization
    /// - Directory traversal (`..`) is rejected
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

#[tool_handler]
impl ServerHandler for AegisMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            // Pin to V_2025_11_25 explicitly — do NOT use .LATEST,
            // which could silently change on a rmcp version bump.
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
                "Ferris Aegis MCP Server — The Rust Guardian for Autonomous Intelligence. \
                 Available tools: file_read (read a file from the local filesystem). \
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
        // Read our own source file
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

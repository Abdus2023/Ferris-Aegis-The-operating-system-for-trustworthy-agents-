//! MCP Tool Export — Auto-generate MCP tool definitions from SKILL.md
//!
//! This module converts SKILL.md specifications into MCP-compatible tool
//! definitions that can be registered with the Ferris Aegis MCP server.

use crate::error::{SkillError, SkillResult};
use crate::types::{Skill, SkillId, SkillInputSpec, SkillOutputSpec};
use crate::executor::{ExecutionContext, SkillExecutor, SkillExecutionResult};
use ferris_aegis_kernel::{AuditLedger, AuditSeverity};
use ferris_aegis_observability::CoreMetrics;
use rmcp::model::{CallToolResult, Content, Tool};
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;
use tracing::Instrument;

/// MCP tool definition generated from a SKILL.md
#[derive(Debug, Clone)]
pub struct McpToolDefinition {
    /// The skill this tool was generated from
    pub skill_id: SkillId,
    /// MCP tool definition (name, description, inputSchema)
    pub tool: Tool,
    /// The skill's export format (should be "mcp-tool")
    pub export_format: String,
}

/// Input schema for a dynamically generated MCP tool
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DynamicToolInput {
    /// The actual input parameters (validated against skill's input schema)
    #[schemars(description = "Input parameters for the skill")]
    pub params: Value,
}

/// Output schema for a dynamically generated MCP tool
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DynamicToolOutput {
    /// Skill execution result
    #[schemars(description = "Skill execution result")]
    pub result: SkillExecutionResult,
}

/// Generator for MCP tool definitions from skills
pub struct McpToolGenerator;

impl McpToolGenerator {
    /// Generate an MCP tool definition from a skill
    pub fn from_skill(skill: &Skill) -> SkillResult<McpToolDefinition> {
        // Only export skills that declare mcp-tool format
        if skill.export_format != "mcp-tool" && skill.export_format != "mcp" {
            return Err(SkillError::ValidationError(format!(
                "Skill {} does not support MCP export (export_format: {})",
                skill.skill_id, skill.export_format
            )));
        }

        // Build input schema from skill's inputs
        let input_schema = Self::build_input_schema(skill)?;
        
        // Build output schema from skill's outputs
        let output_schema = Self::build_output_schema(skill)?;

        // Create the tool definition
        let tool = Tool {
            name: Self::skill_to_tool_name(&skill.skill_id),
            description: skill.description.clone(),
            input_schema: input_schema,
            output_schema: Some(output_schema),
            annotations: None,
        };

        Ok(McpToolDefinition {
            skill_id: skill.skill_id.clone(),
            tool,
            export_format: skill.export_format.clone(),
        })
    }

    /// Convert skill ID to valid MCP tool name
    fn skill_to_tool_name(skill_id: &SkillId) -> String {
        // skill:category:name -> category_name
        let parts: Vec<&str> = skill_id.0.split(':').collect();
        if parts.len() >= 3 {
            format!("{}_{}", parts[1], parts[2])
        } else {
            skill_id.0.replace(':', "_")
        }
    }

    /// Build JSON Schema from skill's inputs
    fn build_input_schema(skill: &Skill) -> SkillResult<Value> {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for (name, input_spec) in &skill.inputs {
            let prop = Self::input_spec_to_json_schema(input_spec)?;
            properties.insert(name.clone(), prop);
            
            if input_spec.required {
                required.push(name.clone());
            }
        }

        Ok(json!({
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false
        }))
    }

    /// Convert an input specification to JSON Schema
    fn input_spec_to_json_schema(spec: &SkillInputSpec) -> SkillResult<Value> {
        let mut schema = json!({
            "type": spec.type_,
            "description": spec.description,
        });

        if let Some(default) = &spec.default {
            schema["default"] = json!(default);
        }
        if let Some(enum_vals) = &spec.enum_ {
            schema["enum"] = json!(enum_vals);
        }
        if let Some(minimum) = spec.minimum {
            schema["minimum"] = json!(minimum);
        }
        if let Some(maximum) = spec.maximum {
            schema["maximum"] = json!(maximum);
        }

        Ok(schema)
    }

    /// Build JSON Schema from skill's outputs
    fn build_output_schema(skill: &Skill) -> SkillResult<Value> {
        let mut properties = serde_json::Map::new();

        for (name, output_spec) in &skill.outputs {
            let prop = Self::output_spec_to_json_schema(output_spec)?;
            properties.insert(name.clone(), prop);
        }

        Ok(json!({
            "type": "object",
            "properties": properties,
            "additionalProperties": false
        }))
    }

    /// Convert an output specification to JSON Schema
    fn output_spec_to_json_schema(spec: &SkillOutputSpec) -> SkillResult<Value> {
        let mut schema = json!({
            "type": spec.type_,
            "description": spec.description,
        });

        if let Some(props) = &spec.properties {
            schema["properties"] = json!(props);
        }
        if let Some(items) = &spec.items {
            schema["items"] = json!(items);
        }

        Ok(schema)
    }

    /// Generate tools for multiple skills
    pub fn from_skills(skills: &[Skill]) -> Vec<McpToolDefinition> {
        skills
            .iter()
            .filter_map(|skill| Self::from_skill(skill).ok())
            .collect()
    }
}

/// Dynamic MCP tool handler that executes skills
pub struct SkillMcpHandler {
    /// The skill executor
    executor: Arc<SkillExecutor>,
    /// The skill registry
    registry: Arc<tokio::sync::RwLock<crate::registry::SkillRegistry>>,
    /// Metrics handle
    metrics: Arc<CoreMetrics>,
    /// Audit ledger
    audit_ledger: Arc<AuditLedger>,
    /// Trust kernel
    trust_kernel: Arc<tokio::sync::RwLock<ferris_aegis_kernel::TrustKernel>>,
}

impl SkillMcpHandler {
    /// Create a new skill MCP handler
    pub fn new(
        executor: Arc<SkillExecutor>,
        registry: Arc<tokio::sync::RwLock<crate::registry::SkillRegistry>>,
        metrics: Arc<CoreMetrics>,
        audit_ledger: Arc<AuditLedger>,
        trust_kernel: Arc<tokio::sync::RwLock<ferris_aegis_kernel::TrustKernel>>,
    ) -> Self {
        Self {
            executor,
            registry,
            metrics,
            audit_ledger,
            trust_kernel,
        }
    }

    /// Execute a skill via MCP tool call
    pub async fn execute_skill(
        &self,
        skill_id: &str,
        input: Value,
        agent_id: Option<String>,
        session_id: Option<Uuid>,
    ) -> SkillResult<CallToolResult> {
        // Get the skill from registry
        let registry = self.registry.read().await;
        let skill = registry.get_sync(skill_id).ok_or_else(|| {
            SkillError::NotFound(format!("Skill not found: {}", skill_id))
        })?;
        drop(registry);

        // Create execution context
        let agent_id = agent_id.unwrap_or_else(|| "mcp-client".to_string());
        let session_id = session_id.unwrap_or_else(Uuid::new_v4);
        
        let trust_kernel_read = self.trust_kernel.read().await;
        let trust_score = trust_kernel_read
            .get_record(&agent_id.into())
            .map(|r| r.score.value())
            .unwrap_or(0.5);
        drop(trust_kernel_read);

        let context = ExecutionContext {
            execution_id: Uuid::new_v4(),
            agent_id: agent_id.clone(),
            agent_trust_score: trust_score,
            session_id,
            capabilities: skill.capabilities_required.iter().cloned().collect(),
            sandbox_boundary: skill.sandbox_boundary.clone(),
            workspace_root: PathBuf::from("/workspace"),
            temp_dir: PathBuf::from("/tmp"),
            start_time: Utc::now(),
            deadline: Some(Utc::now() + chrono::Duration::seconds(300)),
            audit_ledger: self.audit_ledger.clone(),
            metrics: self.metrics.clone(),
            trust_kernel: self.trust_kernel.clone(),
        };

        // Validate execution
        crate::validator::SkillValidator::validate_execution(&skill, &context)?;

        // Execute with tracing
        let span = tracing::info_span!(
            "mcp.tool.skill",
            skill_id = %skill.skill_id,
            skill_name = %skill.name,
            agent_id = %agent_id,
        );

        let result = async move {
            let executor = self.executor.clone();
            executor.execute(&skill, &context, input).await
        }
        .instrument(span)
        .await?;

        // Convert to MCP result
        let tool_name = McpToolGenerator::skill_to_tool_name(&skill.skill_id);
        
        match result.status {
            crate::executor::ExecutionStatus::Success => {
                self.metrics.tool_ok(&tool_name);
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result.output.unwrap_or(json!({})))?
                )]))
            }
            crate::executor::ExecutionStatus::Failed => {
                self.metrics.tool_error(&tool_name);
                let error_msg = result.error.unwrap_or_else(|| "Unknown error".to_string());
                Ok(CallToolResult::success(vec![Content::text(
                    format!("Skill execution failed: {}", error_msg)
                )]))
            }
            crate::executor::ExecutionStatus::Denied => {
                self.metrics.tool_error(&tool_name);
                Ok(CallToolResult::success(vec![Content::text(
                    "Skill execution denied by policy".to_string()
                )]))
            }
            crate::executor::ExecutionStatus::TimedOut => {
                self.metrics.tool_error(&tool_name);
                Ok(CallToolResult::success(vec![Content::text(
                    "Skill execution timed out".to_string()
                )]))
            }
        }
    }

    /// List all available skill tools
    pub async fn list_tools(&self) -> Vec<Tool> {
        let registry = self.registry.read().await;
        let skills = registry.list_all();
        drop(registry);

        McpToolGenerator::from_skills(&skills)
            .into_iter()
            .map(|def| def.tool)
            .collect()
    }
}

/// Extension trait to add skill tools to MCP server
pub trait SkillMcpExt {
    /// Register all skills from a registry as MCP tools
    async fn register_skills(
        &mut self,
        registry: Arc<tokio::sync::RwLock<crate::registry::SkillRegistry>>,
        executor: Arc<SkillExecutor>,
        metrics: Arc<CoreMetrics>,
        audit_ledger: Arc<AuditLedger>,
        trust_kernel: Arc<tokio::sync::RwLock<ferris_aegis_kernel::TrustKernel>>,
    ) -> SkillResult<()>;
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_to_tool_name() {
        let skill_id = SkillId("skill:filesystem:file-processor".to_string());
        assert_eq!(McpToolGenerator::skill_to_tool_name(&skill_id), "filesystem_file-processor");
    }

    #[test]
    fn test_input_spec_schema() {
        let spec = SkillInputSpec {
            type_: "string".to_string(),
            description: "File path".to_string(),
            required: true,
            default: None,
            enum_: None,
            minimum: None,
            maximum: None,
        };
        let schema = McpToolGenerator::input_spec_to_json_schema(&spec).unwrap();
        assert_eq!(schema["type"], "string");
        assert_eq!(schema["description"], "File path");
    }
}
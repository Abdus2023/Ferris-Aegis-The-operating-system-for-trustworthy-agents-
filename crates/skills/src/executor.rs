use crate::error::{SkillError, SkillResult};
use crate::types::*;
use ferris_aegis_kernel::{AuditLedger, AuditSeverity, TrustKernel};
use ferris_aegis_observability::CoreMetrics;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;
use chrono::Utc;

/// Skill execution context passed from the runtime.
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub execution_id: Uuid,
    pub agent_id: String,
    pub agent_trust_score: f64,
    pub session_id: Uuid,
    pub capabilities: HashSet<Capability>,
    pub sandbox_boundary: String,
    pub workspace_root: PathBuf,
    pub temp_dir: PathBuf,
    pub start_time: chrono::DateTime<Utc>,
    pub deadline: Option<chrono::DateTime<Utc>>,
    pub audit_ledger: Arc<AuditLedger>,
    pub metrics: Arc<CoreMetrics>,
    pub trust_kernel: Arc<tokio::sync::RwLock<TrustKernel>>,
}

impl ExecutionContext {
    /// Check if this context has a given capability (supports wildcards).
    pub fn has_capability(&self, cap: &Capability) -> bool {
        self.capabilities.iter().any(|c| c.matches(&cap.0))
    }

    /// Record an action to the audit ledger.
    pub fn record_action(&self, action: &str, outcome: &str) -> SkillResult<()> {
        let target = format!("skill:{}", action);
        let allowed = outcome == "success" || outcome == "allow";
        let severity = if allowed { AuditSeverity::Info } else { AuditSeverity::Warning };
        
        self.audit_ledger.append(
            self.agent_id.clone().into(),
            action.to_string(),
            target,
            allowed,
            severity,
        );
        Ok(())
    }

    /// Record a checkpoint event.
    pub fn record_checkpoint(&self, message: &str) -> SkillResult<()> {
        self.audit_ledger.append(
            self.agent_id.clone().into(),
            "skill:checkpoint".to_string(),
            message.to_string(),
            true,
            AuditSeverity::Info,
        );
        Ok(())
    }

    /// Emit a capability used event.
    pub fn emit_capability_used(&self, capability: &str) -> SkillResult<()> {
        self.audit_ledger.append(
            self.agent_id.clone().into(),
            "capability:used".to_string(),
            capability.to_string(),
            true,
            AuditSeverity::Info,
        );
        Ok(())
    }

    /// Get the workspace root path.
    pub fn workspace_root(&self) -> &PathBuf {
        &self.workspace_root
    }

    /// Get the temp directory path.
    pub fn temp_dir(&self) -> &PathBuf {
        &self.temp_dir
    }
}

/// Result of skill execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillExecutionResult {
    pub execution_id: Uuid,
    pub skill_id: SkillId,
    pub status: ExecutionStatus,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub trace_id: Option<String>,
}

/// Skill execution status.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    Success,
    Failed,
    Denied,
    TimedOut,
}

/// Trait for skill implementations.
#[async_trait::async_trait]
pub trait SkillImpl: Send + Sync {
    /// Execute the skill with the given input and context.
    async fn execute(
        &self,
        input: serde_json::Value,
        context: &ExecutionContext,
    ) -> SkillResult<SkillExecutionResult>;

    /// Get the skill metadata.
    fn metadata(&self) -> SkillMetadata;

    /// Validate input against the skill's schema.
    fn validate_input(&self, input: &serde_json::Value) -> SkillResult<()>;

    /// Get the skill's entrypoint function name.
    fn entrypoint(&self) -> &str;
}

/// Built-in skill implementations.
pub mod builtin {
    use super::*;
    use std::collections::HashMap;

    /// No-op skill for testing.
    pub struct NoopSkill {
        pub metadata: SkillMetadata,
    }

    #[async_trait::async_trait]
    impl SkillImpl for NoopSkill {
        async fn execute(
            &self,
            _input: serde_json::Value,
            context: &ExecutionContext,
        ) -> SkillResult<SkillExecutionResult> {
            let start = Instant::now();
            context.record_action("noop:execute", "success")?;
            context.record_checkpoint("No-op skill executed")?;
            
            Ok(SkillExecutionResult {
                execution_id: context.execution_id,
                skill_id: self.metadata.skill_id.clone(),
                status: ExecutionStatus::Success,
                output: Some(serde_json::json!({ "message": "No-op completed" })),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                trace_id: None,
            })
        }

        fn metadata(&self) -> SkillMetadata {
            self.metadata.clone()
        }

        fn validate_input(&self, _input: &serde_json::Value) -> SkillResult<()> {
            Ok(())
        }

        fn entrypoint(&self) -> &str {
            "run"
        }
    }

    /// Echo skill for testing input/output.
    pub struct EchoSkill {
        pub metadata: SkillMetadata,
    }

    #[async_trait::async_trait]
    impl SkillImpl for EchoSkill {
        async fn execute(
            &self,
            input: serde_json::Value,
            context: &ExecutionContext,
        ) -> SkillResult<SkillExecutionResult> {
            let start = Instant::now();
            context.record_action("echo:execute", "success")?;
            context.emit_capability_used("filesystem:read")?;
            
            Ok(SkillExecutionResult {
                execution_id: context.execution_id,
                skill_id: self.metadata.skill_id.clone(),
                status: ExecutionStatus::Success,
                output: Some(input),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                trace_id: None,
            })
        }

        fn metadata(&self) -> SkillMetadata {
            self.metadata.clone()
        }

        fn validate_input(&self, _input: &serde_json::Value) -> SkillResult<()> {
            Ok(())
        }

        fn entrypoint(&self) -> &str {
            "echo"
        }
    }
}

/// The main skill executor that runs skills within the Aegis sandbox.
pub struct SkillExecutor {
    /// Registry of built-in skill implementations.
    builtin_skills: HashMap<String, Box<dyn SkillImpl>>,
}

impl SkillExecutor {
    /// Create a new skill executor.
    pub fn new() -> Self {
        let mut builtin_skills: HashMap<String, Box<dyn SkillImpl>> = HashMap::new();
        
        // Register built-in skills
        builtin_skills.insert(
            "noop".to_string(),
            Box::new(builtin::NoopSkill {
                metadata: SkillMetadata {
                    skill_id: SkillId("skill:builtin:noop".to_string()),
                    version: "1.0.0".to_string(),
                    category: "builtin".to_string(),
                    trust_level_minimum: TrustLevelRequired::Unverified,
                    capabilities_required: vec![],
                    last_loaded: Utc::now(),
                },
            }),
        );
        
        builtin_skills.insert(
            "echo".to_string(),
            Box::new(builtin::EchoSkill {
                metadata: SkillMetadata {
                    skill_id: SkillId("skill:builtin:echo".to_string()),
                    version: "1.0.0".to_string(),
                    category: "builtin".to_string(),
                    trust_level_minimum: TrustLevelRequired::Unverified,
                    capabilities_required: vec![Capability("filesystem:read".to_string())],
                    last_loaded: Utc::now(),
                },
            }),
        );

        Self { builtin_skills }
    }

    /// Execute a skill by its ID.
    pub async fn execute(
        &self,
        skill: &Skill,
        context: &ExecutionContext,
        input: serde_json::Value,
    ) -> SkillResult<SkillExecutionResult> {
        let start = Instant::now();
        
        // Validate execution context
        crate::validator::SkillValidator::validate_execution(skill, context)?;
        
        // Record execution start
        context.record_action(&format!("skill:execute:{}", skill.skill_id), "start")?;
        
        // Try to find a built-in implementation
        if let Some(impl_) = self.builtin_skills.get(&skill.name.to_lowercase()) {
            let result = impl_.execute(input, context).await?;
            context.record_action(&format!("skill:execute:{}", skill.skill_id), "success")?;
            return Ok(result);
        }
        
        // For external skills, we'd invoke via MCP, HTTP, or WASM
        // This is a placeholder for the actual execution mechanism
        let result = self.execute_external(skill, context, input).await?;
        
        context.record_action(&format!("skill:execute:{}", skill.skill_id), "success")?;
        Ok(result)
    }

    /// Execute an external skill (MCP tool, HTTP endpoint, WASM module).
    async fn execute_external(
        &self,
        skill: &Skill,
        context: &ExecutionContext,
        input: serde_json::Value,
    ) -> SkillResult<SkillExecutionResult> {
        let start = Instant::now();
        
        // Record capability usage
        for cap in &skill.capabilities_required {
            context.emit_capability_used(&cap.0)?;
        }
        
        // TODO: Implement actual execution based on export_format:
        // - mcp-tool: Call via MCP
        // - http-rpc: HTTP POST to endpoint
        // - stdio: Spawn subprocess
        // - wasm: Execute in wasmtime sandbox
        
        // For now, return a mock result
        Ok(SkillExecutionResult {
            execution_id: context.execution_id,
            skill_id: skill.skill_id.clone(),
            status: ExecutionStatus::Success,
            output: Some(serde_json::json!({
                "skill": skill.name,
                "executed": true,
                "input_received": input
            })),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            trace_id: None, // Would be set from OTel context
        })
    }

    /// Register a custom skill implementation.
    pub fn register_builtin(&mut self, name: &str, impl_: Box<dyn SkillImpl>) {
        self.builtin_skills.insert(name.to_lowercase(), impl_);
    }

    /// Check if a built-in skill exists.
    pub fn has_builtin(&self, name: &str) -> bool {
        self.builtin_skills.contains_key(&name.to_lowercase())
    }
}

impl Default for SkillExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;

    fn test_context() -> ExecutionContext {
        ExecutionContext {
            execution_id: Uuid::new_v4(),
            agent_id: "test-agent".to_string(),
            agent_trust_score: 0.5,
            session_id: Uuid::new_v4(),
            capabilities: HashSet::new(),
            sandbox_boundary: "standard".to_string(),
            workspace_root: PathBuf::from("/workspace"),
            temp_dir: PathBuf::from("/tmp"),
            start_time: Utc::now(),
            deadline: None,
            audit_ledger: Arc::new(AuditLedger::new()),
            metrics: Arc::new(CoreMetrics::new()),
            trust_kernel: Arc::new(tokio::sync::RwLock::new(TrustKernel::new())),
        }
    }

    #[tokio::test]
    async fn test_noop_skill_execution() {
        let executor = SkillExecutor::new();
        let skill = Skill {
            skill_version: "1.0.0".to_string(),
            skill_id: SkillId("skill:builtin:noop".to_string()),
            name: "noop".to_string(),
            category: "builtin".to_string(),
            description: "No-op skill".to_string(),
            version: "1.0.0".to_string(),
            author: "Test".to_string(),
            license: "MIT".to_string(),
            tags: vec![],
            keywords: vec![],
            maintainer: None,
            capabilities_required: vec![],
            trust_level_minimum: TrustLevelRequired::Unverified,
            sandbox_boundary: "restricted".to_string(),
            dependencies: vec![],
            triggers: vec![],
            resource_limits: ResourceLimits::default(),
            policies: vec![],
            execution_protocol: "aegis:rpc/1.0".to_string(),
            protocol_version: "V_2025_11_25".to_string(),
            export_format: "mcp-tool".to_string(),
            compatible_agents: vec![],
            signature: None,
            content: String::new(),
        };

        let context = test_context();
        let result = executor.execute(&skill, &context, serde_json::json!({})).await.unwrap();
        
        assert_eq!(result.status, ExecutionStatus::Success);
        assert!(result.output.is_some());
    }

    #[tokio::test]
    async fn test_echo_skill_execution() {
        let executor = SkillExecutor::new();
        let skill = Skill {
            skill_version: "1.0.0".to_string(),
            skill_id: SkillId("skill:builtin:echo".to_string()),
            name: "echo".to_string(),
            category: "builtin".to_string(),
            description: "Echo skill".to_string(),
            version: "1.0.0".to_string(),
            author: "Test".to_string(),
            license: "MIT".to_string(),
            tags: vec![],
            keywords: vec![],
            maintainer: None,
            capabilities_required: vec![Capability("filesystem:read".to_string())],
            trust_level_minimum: TrustLevelRequired::Unverified,
            sandbox_boundary: "restricted".to_string(),
            dependencies: vec![],
            triggers: vec![],
            resource_limits: ResourceLimits::default(),
            policies: vec![],
            execution_protocol: "aegis:rpc/1.0".to_string(),
            protocol_version: "V_2025_11_25".to_string(),
            export_format: "mcp-tool".to_string(),
            compatible_agents: vec![],
            signature: None,
            content: String::new(),
        };

        let mut context = test_context();
        context.capabilities.insert(Capability("filesystem:read".to_string()));
        
        let input = serde_json::json!({"message": "hello"});
        let result = executor.execute(&skill, &context, input.clone()).await.unwrap();
        
        assert_eq!(result.status, ExecutionStatus::Success);
        assert_eq!(result.output, Some(input));
    }
}
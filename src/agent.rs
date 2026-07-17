//! # Agent Runtime — Lifecycle Management for Autonomous Agents
//!
//! The Agent Runtime manages the complete lifecycle of autonomous agents:
//! spawning, execution, suspension, resumption, and termination. Every
//! state transition is tracked and auditable.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

use crate::kernel::TrustKernel;
use crate::policy::PolicyEngine;
use crate::sandbox::Capability;

/// Unique identifier for an agent instance
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AgentId(String);

impl AgentId {
    /// Create a new agent ID from a name
    pub fn new(name: &str) -> Self {
        Self(format!("{}-{}", name, Uuid::new_v4().as_simple()))
    }

    /// Create an agent ID from a known value (for testing/restoration)
    pub fn from_raw(raw: &str) -> Self {
        Self(raw.to_string())
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Current status of an agent
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentStatus {
    /// Agent is being initialized
    Spawning,
    /// Agent is actively running
    Running,
    /// Agent is paused (can be resumed)
    Suspended,
    /// Agent has completed its task
    Completed,
    /// Agent was terminated (manually or by policy)
    Terminated,
    /// Agent failed due to an error
    Failed,
    /// Agent was quarantined by the Guard
    Quarantined,
}

impl AgentStatus {
    /// Whether the agent is in an active state
    pub fn is_active(&self) -> bool {
        matches!(self, AgentStatus::Running | AgentStatus::Spawning)
    }

    /// Whether the agent can be resumed
    pub fn can_resume(&self) -> bool {
        matches!(self, AgentStatus::Suspended)
    }

    /// Whether the agent is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            AgentStatus::Completed | AgentStatus::Terminated | AgentStatus::Failed
        )
    }
}

impl fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentStatus::Spawning => write!(f, "spawning"),
            AgentStatus::Running => write!(f, "running"),
            AgentStatus::Suspended => write!(f, "suspended"),
            AgentStatus::Completed => write!(f, "completed"),
            AgentStatus::Terminated => write!(f, "terminated"),
            AgentStatus::Failed => write!(f, "failed"),
            AgentStatus::Quarantined => write!(f, "quarantined"),
        }
    }
}

/// Agent state: persistent data carried across lifecycle transitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    /// Arbitrary key-value state data
    data: HashMap<String, serde_json::Value>,
}

impl AgentState {
    /// Create a new empty agent state
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Set a value in the agent state
    pub fn set(&mut self, key: &str, value: serde_json::Value) {
        self.data.insert(key.to_string(), value);
    }

    /// Get a value from the agent state
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.data.get(key)
    }

    /// Remove a value from the agent state
    pub fn remove(&mut self, key: &str) -> Option<serde_json::Value> {
        self.data.remove(key)
    }

    /// Check if a key exists in the state
    pub fn contains(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// Number of entries in the state
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Whether the state is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl Default for AgentState {
    fn default() -> Self {
        Self::new()
    }
}

/// An agent instance managed by the runtime
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    /// Unique identifier
    pub id: AgentId,
    /// Human-readable name
    pub name: String,
    /// Agent version
    pub version: String,
    /// Current status
    pub status: AgentStatus,
    /// Persistent state
    pub state: AgentState,
    /// Capabilities granted to this agent
    pub capabilities: Vec<Capability>,
    /// When the agent was spawned
    pub spawned_at: chrono::DateTime<chrono::Utc>,
    /// Last status change
    pub last_transition: chrono::DateTime<chrono::Utc>,
    /// Number of actions performed
    pub action_count: u64,
    /// Parent agent, if spawned by another agent
    pub parent: Option<AgentId>,
}

impl Agent {
    /// Create a new agent
    pub fn new(name: &str, version: &str) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: AgentId::new(name),
            name: name.to_string(),
            version: version.to_string(),
            status: AgentStatus::Spawning,
            state: AgentState::new(),
            capabilities: Vec::new(),
            spawned_at: now,
            last_transition: now,
            action_count: 0,
            parent: None,
        }
    }

    /// Transition to a new status
    pub fn transition(&mut self, new_status: AgentStatus) -> AgentStatus {
        let old = self.status;
        self.status = new_status;
        self.last_transition = chrono::Utc::now();
        old
    }

    /// Grant a capability to the agent
    pub fn grant_capability(&mut self, cap: Capability) {
        if !self.capabilities.contains(&cap) {
            self.capabilities.push(cap);
        }
    }

    /// Revoke a capability from the agent
    pub fn revoke_capability(&mut self, cap: &Capability) {
        self.capabilities.retain(|c| c != cap);
    }

    /// Check if the agent has a specific capability
    pub fn has_capability(&self, cap: &Capability) -> bool {
        self.capabilities.contains(cap)
    }

    /// Record that the agent performed an action
    pub fn record_action(&mut self) {
        self.action_count += 1;
    }

    /// Set the parent agent
    pub fn with_parent(mut self, parent: AgentId) -> Self {
        self.parent = Some(parent);
        self
    }
}

/// The Agent Runtime — manages agent lifecycles
pub struct AgentRuntime {
    /// The trust kernel for trust verification
    trust_kernel: TrustKernel,
    /// The policy engine for enforcement
    policy_engine: PolicyEngine,
    /// Active agents indexed by ID
    agents: HashMap<AgentId, Agent>,
}

impl AgentRuntime {
    /// Create a new agent runtime
    pub fn new(trust_kernel: TrustKernel, policy_engine: PolicyEngine) -> Self {
        Self {
            trust_kernel,
            policy_engine,
            agents: HashMap::new(),
        }
    }

    /// Spawn a new agent into the runtime
    pub async fn spawn(&mut self, name: &str, version: &str) -> anyhow::Result<AgentId> {
        let mut agent = Agent::new(name, version);

        // Register with trust kernel
        self.trust_kernel.register(&agent.id);

        // Apply initial capabilities based on trust level
        let record = self.trust_kernel.get_record(&agent.id);
        if let Some(rec) = record {
            if rec.level.as_u8() >= crate::kernel::TrustLevel::Standard.as_u8() {
                agent.grant_capability(Capability::FileSystemRead);
            }
        }

        // Transition to running
        agent.transition(AgentStatus::Running);

        let id = agent.id.clone();
        self.agents.insert(id.clone(), agent);

        tracing::info!(agent_id = %id, name = name, "Agent spawned");
        Ok(id)
    }

    /// Suspend a running agent
    pub async fn suspend(&mut self, agent_id: &AgentId) -> anyhow::Result<()> {
        let agent = self.agents.get_mut(agent_id).ok_or_else(|| {
            anyhow::anyhow!("Agent not found: {}", agent_id)
        })?;

        if !agent.status.is_active() {
            return Err(anyhow::anyhow!(
                "Cannot suspend agent in {} state",
                agent.status
            ));
        }

        agent.transition(AgentStatus::Suspended);
        tracing::info!(agent_id = %agent_id, "Agent suspended");
        Ok(())
    }

    /// Resume a suspended agent
    pub async fn resume(&mut self, agent_id: &AgentId) -> anyhow::Result<()> {
        let agent = self.agents.get_mut(agent_id).ok_or_else(|| {
            anyhow::anyhow!("Agent not found: {}", agent_id)
        })?;

        if !agent.status.can_resume() {
            return Err(anyhow::anyhow!(
                "Cannot resume agent in {} state",
                agent.status
            ));
        }

        agent.transition(AgentStatus::Running);
        tracing::info!(agent_id = %agent_id, "Agent resumed");
        Ok(())
    }

    /// Terminate an agent
    pub async fn terminate(&mut self, agent_id: &AgentId) -> anyhow::Result<()> {
        let agent = self.agents.get_mut(agent_id).ok_or_else(|| {
            anyhow::anyhow!("Agent not found: {}", agent_id)
        })?;

        if agent.status.is_terminal() {
            return Err(anyhow::anyhow!(
                "Agent already in terminal state: {}",
                agent.status
            ));
        }

        agent.transition(AgentStatus::Terminated);
        tracing::info!(agent_id = %agent_id, "Agent terminated");
        Ok(())
    }

    /// Quarantine an agent (used by the Guard)
    pub async fn quarantine(&mut self, agent_id: &AgentId) -> anyhow::Result<()> {
        let agent = self.agents.get_mut(agent_id).ok_or_else(|| {
            anyhow::anyhow!("Agent not found: {}", agent_id)
        })?;

        agent.transition(AgentStatus::Quarantined);
        // Strip all capabilities
        agent.capabilities.clear();
        tracing::warn!(agent_id = %agent_id, "Agent quarantined — all capabilities revoked");
        Ok(())
    }

    /// Get a reference to an agent
    pub fn get_agent(&self, agent_id: &AgentId) -> Option<&Agent> {
        self.agents.get(agent_id)
    }

    /// Get a mutable reference to an agent
    pub fn get_agent_mut(&mut self, agent_id: &AgentId) -> Option<&mut Agent> {
        self.agents.get_mut(agent_id)
    }

    /// List all agents and their statuses
    pub fn list_agents(&self) -> Vec<(&AgentId, &AgentStatus, &str)> {
        self.agents
            .iter()
            .map(|(id, agent)| (id, &agent.status, agent.name.as_str()))
            .collect()
    }

    /// Get the number of active agents
    pub fn active_count(&self) -> usize {
        self.agents
            .values()
            .filter(|a| a.status.is_active())
            .count()
    }

    /// Get total agent count
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    /// Get a reference to the trust kernel
    pub fn trust_kernel(&self) -> &TrustKernel {
        &self.trust_kernel
    }

    /// Get a mutable reference to the trust kernel
    pub fn trust_kernel_mut(&mut self) -> &mut TrustKernel {
        &mut self.trust_kernel
    }

    /// Get a reference to the policy engine
    pub fn policy_engine(&self) -> &PolicyEngine {
        &self.policy_engine
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_creation() {
        let agent = Agent::new("test-agent", "1.0.0");
        assert_eq!(agent.name, "test-agent");
        assert_eq!(agent.version, "1.0.0");
        assert_eq!(agent.status, AgentStatus::Spawning);
    }

    #[test]
    fn test_agent_transition() {
        let mut agent = Agent::new("test", "1.0.0");
        let old = agent.transition(AgentStatus::Running);
        assert_eq!(old, AgentStatus::Spawning);
        assert_eq!(agent.status, AgentStatus::Running);
    }

    #[test]
    fn test_agent_capabilities() {
        let mut agent = Agent::new("test", "1.0.0");
        agent.grant_capability(Capability::NetworkAccess);
        assert!(agent.has_capability(&Capability::NetworkAccess));
        assert!(!agent.has_capability(&Capability::FileSystemWrite));

        agent.revoke_capability(&Capability::NetworkAccess);
        assert!(!agent.has_capability(&Capability::NetworkAccess));
    }

    #[test]
    fn test_agent_state() {
        let mut state = AgentState::new();
        state.set("key", serde_json::json!("value"));
        assert!(state.contains("key"));
        assert_eq!(state.get("key").unwrap(), "value");
        state.remove("key");
        assert!(!state.contains("key"));
    }

    #[test]
    fn test_status_properties() {
        assert!(AgentStatus::Running.is_active());
        assert!(!AgentStatus::Suspended.is_active());
        assert!(AgentStatus::Suspended.can_resume());
        assert!(AgentStatus::Completed.is_terminal());
        assert!(!AgentStatus::Running.is_terminal());
    }

    #[tokio::test]
    async fn test_runtime_spawn_and_terminate() {
        let kernel = TrustKernel::new();
        let policy = PolicyEngine::new();
        let mut runtime = AgentRuntime::new(kernel, policy);

        let id = runtime.spawn("test", "1.0.0").await.unwrap();
        assert_eq!(runtime.active_count(), 1);

        runtime.terminate(&id).await.unwrap();
        assert_eq!(runtime.active_count(), 0);
    }

    #[tokio::test]
    async fn test_runtime_suspend_resume() {
        let kernel = TrustKernel::new();
        let policy = PolicyEngine::new();
        let mut runtime = AgentRuntime::new(kernel, policy);

        let id = runtime.spawn("test", "1.0.0").await.unwrap();
        runtime.suspend(&id).await.unwrap();

        let agent = runtime.get_agent(&id).unwrap();
        assert_eq!(agent.status, AgentStatus::Suspended);

        runtime.resume(&id).await.unwrap();
        let agent = runtime.get_agent(&id).unwrap();
        assert_eq!(agent.status, AgentStatus::Running);
    }
}

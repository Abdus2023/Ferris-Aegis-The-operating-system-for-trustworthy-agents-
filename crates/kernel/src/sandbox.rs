//! # Sandbox — Capability-Based Isolation for Agent Execution
//!
//! The Sandbox provides capability-based security boundaries that constrain
//! what agents can do. Every capability must be explicitly granted — there
//! is no ambient authority. This follows the principle of least privilege.
//!
//! ## Capabilities
//!
//! Capabilities are tokens that grant specific permissions. An agent without
//! a capability cannot perform the associated actions, regardless of its
//! trust level.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::agent::AgentId;
use crate::kernel::TrustLevel;

/// A capability token granting a specific permission
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Read access to the filesystem
    FileSystemRead,
    /// Write access to the filesystem
    FileSystemWrite,
    /// Network connectivity (outbound)
    NetworkAccess,
    /// Ability to spawn child processes
    ProcessSpawn,
    /// Access to system environment variables
    EnvironmentAccess,
    /// Ability to communicate with other agents
    InterAgentComm,
    /// Access to the audit ledger (read-only)
    AuditRead,
    /// Ability to modify policies (requires Sovereign trust level)
    PolicyModify,
    /// Access to cryptographic operations (signing, encryption)
    CryptoOperations,
    /// Ability to use extended memory (beyond default allocation)
    ExtendedMemory,
    /// Access to real-time clock and timers
    TimerAccess,
    /// Ability to manage other agents (supervisory)
    AgentManagement,
}

impl Capability {
    /// Get the minimum trust level required for this capability
    pub fn min_trust_level(&self) -> TrustLevel {
        match self {
            Capability::FileSystemRead => TrustLevel::Standard,
            Capability::FileSystemWrite => TrustLevel::Elevated,
            Capability::NetworkAccess => TrustLevel::Standard,
            Capability::ProcessSpawn => TrustLevel::Elevated,
            Capability::EnvironmentAccess => TrustLevel::Standard,
            Capability::InterAgentComm => TrustLevel::Probationary,
            Capability::AuditRead => TrustLevel::Standard,
            Capability::PolicyModify => TrustLevel::Sovereign,
            Capability::CryptoOperations => TrustLevel::Elevated,
            Capability::ExtendedMemory => TrustLevel::Standard,
            Capability::TimerAccess => TrustLevel::Probationary,
            Capability::AgentManagement => TrustLevel::Sovereign,
        }
    }

    /// Get a human-readable name for this capability
    pub fn name(&self) -> &'static str {
        match self {
            Capability::FileSystemRead => "fs:read",
            Capability::FileSystemWrite => "fs:write",
            Capability::NetworkAccess => "net:access",
            Capability::ProcessSpawn => "proc:spawn",
            Capability::EnvironmentAccess => "env:access",
            Capability::InterAgentComm => "comm:inter-agent",
            Capability::AuditRead => "audit:read",
            Capability::PolicyModify => "policy:modify",
            Capability::CryptoOperations => "crypto:ops",
            Capability::ExtendedMemory => "mem:extended",
            Capability::TimerAccess => "timer:access",
            Capability::AgentManagement => "agent:manage",
        }
    }

    /// List all available capabilities
    pub fn all() -> &'static [Capability] {
        &[
            Capability::FileSystemRead,
            Capability::FileSystemWrite,
            Capability::NetworkAccess,
            Capability::ProcessSpawn,
            Capability::EnvironmentAccess,
            Capability::InterAgentComm,
            Capability::AuditRead,
            Capability::PolicyModify,
            Capability::CryptoOperations,
            Capability::ExtendedMemory,
            Capability::TimerAccess,
            Capability::AgentManagement,
        ]
    }
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Resource limits for a sandbox
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum memory in bytes
    pub max_memory_bytes: u64,
    /// Maximum CPU time in seconds
    pub max_cpu_seconds: u64,
    /// Maximum number of file descriptors
    pub max_fds: u32,
    /// Maximum network bandwidth in bytes per second
    pub max_network_bps: u64,
    /// Maximum number of child processes
    pub max_processes: u32,
    /// Maximum number of actions per minute
    pub max_actions_per_minute: u32,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: 512 * 1024 * 1024, // 512 MB
            max_cpu_seconds: 3600,                 // 1 hour
            max_fds: 256,
            max_network_bps: 10 * 1024 * 1024,    // 10 MB/s
            max_processes: 8,
            max_actions_per_minute: 1000,
        }
    }
}

impl ResourceLimits {
    /// Create resource limits for a restricted sandbox
    pub fn restricted() -> Self {
        Self {
            max_memory_bytes: 64 * 1024 * 1024,   // 64 MB
            max_cpu_seconds: 300,                   // 5 minutes
            max_fds: 32,
            max_network_bps: 1024 * 1024,          // 1 MB/s
            max_processes: 0,                       // No child processes
            max_actions_per_minute: 100,
        }
    }

    /// Create resource limits for an unrestricted sandbox (Sovereign agents)
    pub fn unrestricted() -> Self {
        Self {
            max_memory_bytes: 4 * 1024 * 1024 * 1024, // 4 GB
            max_cpu_seconds: 86400,                     // 24 hours
            max_fds: 1024,
            max_network_bps: 100 * 1024 * 1024,        // 100 MB/s
            max_processes: 64,
            max_actions_per_minute: 10000,
        }
    }
}

/// The boundary of a sandbox — what an agent can and cannot do
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxBoundary {
    /// The agent this boundary belongs to
    pub agent_id: AgentId,
    /// Granted capabilities
    pub capabilities: Vec<Capability>,
    /// Resource limits
    pub resource_limits: ResourceLimits,
    /// Allowed filesystem paths (read)
    pub allowed_read_paths: Vec<String>,
    /// Allowed filesystem paths (write)
    pub allowed_write_paths: Vec<String>,
    /// Allowed network endpoints
    pub allowed_network_endpoints: Vec<String>,
    /// Whether the sandbox is currently locked (cannot be modified)
    pub locked: bool,
}

impl SandboxBoundary {
    /// Create a new sandbox boundary for an agent
    pub fn new(agent_id: AgentId) -> Self {
        Self {
            agent_id,
            capabilities: Vec::new(),
            resource_limits: ResourceLimits::default(),
            allowed_read_paths: vec!["/workspace".to_string()],
            allowed_write_paths: vec!["/workspace".to_string()],
            allowed_network_endpoints: Vec::new(),
            locked: false,
        }
    }

    /// Grant a capability to the sandbox
    pub fn grant(&mut self, cap: Capability) -> Result<(), SandboxError> {
        if self.locked {
            return Err(SandboxError::BoundaryLocked);
        }
        if !self.capabilities.contains(&cap) {
            self.capabilities.push(cap);
        }
        Ok(())
    }

    /// Revoke a capability from the sandbox
    pub fn revoke(&mut self, cap: &Capability) -> Result<(), SandboxError> {
        if self.locked {
            return Err(SandboxError::BoundaryLocked);
        }
        self.capabilities.retain(|c| c != cap);
        Ok(())
    }

    /// Check if the sandbox has a specific capability
    pub fn has_capability(&self, cap: &Capability) -> bool {
        self.capabilities.contains(cap)
    }

    /// Lock the sandbox boundary (prevents further modifications)
    pub fn lock(&mut self) {
        self.locked = true;
    }

    /// Create a minimal boundary with only basic capabilities
    pub fn minimal(agent_id: AgentId) -> Self {
        Self {
            agent_id,
            capabilities: vec![Capability::TimerAccess, Capability::InterAgentComm],
            resource_limits: ResourceLimits::restricted(),
            allowed_read_paths: vec!["/workspace".to_string()],
            allowed_write_paths: vec![],
            allowed_network_endpoints: vec![],
            locked: false,
        }
    }

    /// Create a full boundary with all capabilities
    pub fn full(agent_id: AgentId) -> Self {
        Self {
            agent_id,
            capabilities: Capability::all().to_vec(),
            resource_limits: ResourceLimits::unrestricted(),
            allowed_read_paths: vec!["/*".to_string()],
            allowed_write_paths: vec!["/workspace/*".to_string()],
            allowed_network_endpoints: vec!["*".to_string()],
            locked: false,
        }
    }
}

/// Errors that can occur in sandbox operations
#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    /// The sandbox boundary is locked and cannot be modified
    #[error("sandbox boundary is locked")]
    BoundaryLocked,

    /// The agent lacks the required capability
    #[error("agent lacks capability: {0}")]
    CapabilityDenied(String),

    /// Resource limit exceeded
    #[error("resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),

    /// Path not allowed
    #[error("path not allowed: {0}")]
    PathNotAllowed(String),

    /// Network endpoint not allowed
    #[error("network endpoint not allowed: {0}")]
    NetworkEndpointNotAllowed(String),
}

/// The Sandbox Manager — creates and manages sandbox boundaries
#[derive(Debug)]
pub struct Sandbox {
    /// Active sandbox boundaries indexed by agent ID
    boundaries: HashMap<AgentId, SandboxBoundary>,
}

impl Sandbox {
    /// Create a new sandbox manager
    pub fn new() -> Self {
        Self {
            boundaries: HashMap::new(),
        }
    }

    /// Create a sandbox boundary for an agent based on its trust level
    pub fn create_boundary(
        &mut self,
        agent_id: AgentId,
        trust_level: TrustLevel,
    ) -> &SandboxBoundary {
        let boundary = match trust_level {
            TrustLevel::Unverified => SandboxBoundary::minimal(agent_id.clone()),
            TrustLevel::Probationary => {
                let mut b = SandboxBoundary::new(agent_id.clone());
                b.capabilities = vec![
                    Capability::TimerAccess,
                    Capability::InterAgentComm,
                    Capability::FileSystemRead,
                ];
                b.resource_limits = ResourceLimits::restricted();
                b
            }
            TrustLevel::Standard => {
                let mut b = SandboxBoundary::new(agent_id.clone());
                b.capabilities = vec![
                    Capability::FileSystemRead,
                    Capability::NetworkAccess,
                    Capability::EnvironmentAccess,
                    Capability::InterAgentComm,
                    Capability::AuditRead,
                    Capability::ExtendedMemory,
                    Capability::TimerAccess,
                ];
                b
            }
            TrustLevel::Elevated => {
                let mut b = SandboxBoundary::new(agent_id.clone());
                b.capabilities = vec![
                    Capability::FileSystemRead,
                    Capability::FileSystemWrite,
                    Capability::NetworkAccess,
                    Capability::ProcessSpawn,
                    Capability::EnvironmentAccess,
                    Capability::InterAgentComm,
                    Capability::AuditRead,
                    Capability::CryptoOperations,
                    Capability::ExtendedMemory,
                    Capability::TimerAccess,
                ];
                b
            }
            TrustLevel::Sovereign => SandboxBoundary::full(agent_id.clone()),
        };

        self.boundaries.insert(agent_id.clone(), boundary);
        self.boundaries.get(&agent_id).unwrap()
    }

    /// Get the boundary for an agent
    pub fn get_boundary(&self, agent_id: &AgentId) -> Option<&SandboxBoundary> {
        self.boundaries.get(agent_id)
    }

    /// Check if an agent is allowed to perform an action
    pub fn check_capability(
        &self,
        agent_id: &AgentId,
        capability: &Capability,
    ) -> Result<(), SandboxError> {
        let boundary = self
            .boundaries
            .get(agent_id)
            .ok_or_else(|| SandboxError::CapabilityDenied(format!(
                "no sandbox boundary for agent {}", agent_id
            )))?;

        if boundary.has_capability(capability) {
            Ok(())
        } else {
            Err(SandboxError::CapabilityDenied(format!(
                "agent {} lacks capability {}",
                agent_id, capability
            )))
        }
    }

    /// Grant a capability to an agent's sandbox
    pub fn grant_capability(
        &mut self,
        agent_id: &AgentId,
        capability: Capability,
    ) -> Result<(), SandboxError> {
        let boundary = self
            .boundaries
            .get_mut(agent_id)
            .ok_or_else(|| SandboxError::CapabilityDenied(format!(
                "no sandbox boundary for agent {}", agent_id
            )))?;

        boundary.grant(capability)
    }

    /// Revoke a capability from an agent's sandbox
    pub fn revoke_capability(
        &mut self,
        agent_id: &AgentId,
        capability: &Capability,
    ) -> Result<(), SandboxError> {
        let boundary = self
            .boundaries
            .get_mut(agent_id)
            .ok_or_else(|| SandboxError::CapabilityDenied(format!(
                "no sandbox boundary for agent {}", agent_id
            )))?;

        boundary.revoke(capability)
    }

    /// Remove an agent's sandbox (when the agent terminates)
    pub fn remove_boundary(&mut self, agent_id: &AgentId) -> Option<SandboxBoundary> {
        self.boundaries.remove(agent_id)
    }

    /// List all sandboxed agents
    pub fn list_sandboxes(&self) -> Vec<(&AgentId, usize)> {
        self.boundaries
            .iter()
            .map(|(id, b)| (id, b.capabilities.len()))
            .collect()
    }
}

impl Default for Sandbox {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_agent_id() -> AgentId {
        AgentId::new("test-agent")
    }

    #[test]
    fn test_capability_min_trust() {
        assert_eq!(Capability::PolicyModify.min_trust_level(), TrustLevel::Sovereign);
        assert_eq!(Capability::FileSystemRead.min_trust_level(), TrustLevel::Standard);
        assert_eq!(Capability::TimerAccess.min_trust_level(), TrustLevel::Probationary);
    }

    #[test]
    fn test_sandbox_boundary_grant_revoke() {
        let mut boundary = SandboxBoundary::new(test_agent_id());

        boundary.grant(Capability::NetworkAccess).unwrap();
        assert!(boundary.has_capability(&Capability::NetworkAccess));

        boundary.revoke(&Capability::NetworkAccess).unwrap();
        assert!(!boundary.has_capability(&Capability::NetworkAccess));
    }

    #[test]
    fn test_sandbox_boundary_lock() {
        let mut boundary = SandboxBoundary::new(test_agent_id());
        boundary.lock();

        let result = boundary.grant(Capability::NetworkAccess);
        assert!(matches!(result, Err(SandboxError::BoundaryLocked)));
    }

    #[test]
    fn test_sandbox_create_boundary_by_trust() {
        let mut sandbox = Sandbox::new();
        let agent_id = test_agent_id();

        let boundary = sandbox.create_boundary(agent_id.clone(), TrustLevel::Unverified);
        assert!(boundary.has_capability(&Capability::TimerAccess));
        assert!(!boundary.has_capability(&Capability::NetworkAccess));

        let agent_id2 = AgentId::new("sovereign-agent");
        let boundary2 = sandbox.create_boundary(agent_id2, TrustLevel::Sovereign);
        assert!(boundary2.has_capability(&Capability::PolicyModify));
    }

    #[test]
    fn test_sandbox_check_capability() {
        let mut sandbox = Sandbox::new();
        let agent_id = test_agent_id();
        sandbox.create_boundary(agent_id.clone(), TrustLevel::Standard);

        assert!(sandbox.check_capability(&agent_id, &Capability::NetworkAccess).is_ok());
        assert!(sandbox.check_capability(&agent_id, &Capability::ProcessSpawn).is_err());
    }

    #[test]
    fn test_resource_limits_defaults() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_memory_bytes, 512 * 1024 * 1024);
        assert!(limits.max_processes > 0);
    }
}

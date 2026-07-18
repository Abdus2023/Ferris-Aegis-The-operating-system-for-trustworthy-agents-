//! # Policy Engine — Declarative Governance for Autonomous Agents
//!
//! The Policy Engine defines and enforces rules governing agent behavior.
//! Policies are declarative — they describe *what* is allowed, not *how*
//! to enforce it. The engine evaluates every agent action against active
//! policies before allowing it to proceed.
//!
//! ## Policy Structure
//!
//! ```toml
//! [policy]
//! name = "default-safety"
//! version = "1.0"
//! priority = 100
//!
//! [[rules]]
//! action = "file:write"
//! effect = "deny"
//! targets = ["/etc/*", "/var/*"]
//!
//! [[rules]]
//! action = "network:connect"
//! effect = "allow"
//! targets = ["*.example.com:443"]
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Whether a policy rule allows or denies an action
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Effect {
    /// The action is permitted
    Allow,
    /// The action is forbidden
    Deny,
}

impl std::fmt::Display for Effect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Effect::Allow => write!(f, "allow"),
            Effect::Deny => write!(f, "deny"),
        }
    }
}

/// A single rule within a policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    /// The action pattern this rule matches (e.g., "file:write", "network:connect")
    pub action: String,
    /// Whether to allow or deny the action
    pub effect: Effect,
    /// Target patterns for the action (e.g., file paths, hostnames)
    pub targets: Vec<String>,
    /// Optional condition expression
    pub condition: Option<String>,
    /// Optional human-readable description
    pub description: Option<String>,
}

impl PolicyRule {
    /// Check if this rule matches a given action
    pub fn matches_action(&self, action: &str) -> bool {
        // Support glob-style matching in action patterns
        if self.action.contains('*') {
            glob_match(&self.action, action)
        } else {
            self.action == action
        }
    }

    /// Check if this rule matches a given target
    pub fn matches_target(&self, target: &str) -> bool {
        if self.targets.is_empty() {
            return true;
        }
        self.targets.iter().any(|pattern| glob_match(pattern, target))
    }
}

/// The verdict returned by the policy engine for an action
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PolicyVerdict {
    /// The action is explicitly allowed
    Allowed,
    /// The action is explicitly denied
    Denied { reason: String },
    /// No policy matched — default deny
    NoMatch,
}

impl PolicyVerdict {
    /// Whether the action is allowed to proceed
    pub fn is_allowed(&self) -> bool {
        matches!(self, PolicyVerdict::Allowed)
    }
}

/// A complete policy document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    /// Human-readable policy name
    pub name: String,
    /// Policy version
    pub version: String,
    /// Priority — higher priority policies override lower ones
    pub priority: i32,
    /// Whether this policy is active
    pub enabled: bool,
    /// The rules that make up this policy
    pub rules: Vec<PolicyRule>,
    /// Default effect when no rule matches
    pub default_effect: Effect,
}

impl Policy {
    /// Create a new policy with a name and version
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            priority: 0,
            enabled: true,
            rules: Vec::new(),
            default_effect: Effect::Deny,
        }
    }

    /// Set the priority of this policy
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set the default effect
    pub fn with_default_effect(mut self, effect: Effect) -> Self {
        self.default_effect = effect;
        self
    }

    /// Add a rule to this policy
    pub fn add_rule(&mut self, rule: PolicyRule) {
        self.rules.push(rule);
    }

    /// Create a default safety policy
    pub fn default_safety() -> Self {
        let mut policy = Self::new("default-safety", "1.0.0")
            .with_priority(100)
            .with_default_effect(Effect::Deny);

        // Deny writing to system paths
        policy.add_rule(PolicyRule {
            action: "file:write".to_string(),
            effect: Effect::Deny,
            targets: vec![
                "/etc/*".to_string(),
                "/var/*".to_string(),
                "/sys/*".to_string(),
                "/proc/*".to_string(),
            ],
            condition: None,
            description: Some("Deny writes to system directories".to_string()),
        });

        // Deny network connections to internal networks
        policy.add_rule(PolicyRule {
            action: "network:connect".to_string(),
            effect: Effect::Deny,
            targets: vec![
                "10.*".to_string(),
                "172.16.*".to_string(),
                "192.168.*".to_string(),
                "localhost:*".to_string(),
                "127.0.0.1:*".to_string(),
            ],
            condition: None,
            description: Some("Deny connections to internal networks".to_string()),
        });

        // Allow file reads from workspace
        policy.add_rule(PolicyRule {
            action: "file:read".to_string(),
            effect: Effect::Allow,
            targets: vec!["/workspace/*".to_string()],
            condition: None,
            description: Some("Allow reads from workspace directory".to_string()),
        });

        // Deny code execution
        policy.add_rule(PolicyRule {
            action: "exec:*".to_string(),
            effect: Effect::Deny,
            targets: vec![],
            condition: None,
            description: Some("Deny arbitrary code execution".to_string()),
        });

        policy
    }
}

/// The Policy Engine — evaluates actions against policies
#[derive(Debug)]
pub struct PolicyEngine {
    /// Active policies indexed by name
    policies: HashMap<String, Policy>,
}

impl PolicyEngine {
    /// Create a new empty policy engine
    pub fn new() -> Self {
        Self {
            policies: HashMap::new(),
        }
    }

    /// Create a policy engine with the default safety policy
    pub fn with_defaults() -> Self {
        let mut engine = Self::new();
        let policy = Policy::default_safety();
        engine.add_policy(policy);
        engine
    }

    /// Load policies from a TOML file
    pub fn from_file(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_toml(&content)
    }

    /// Parse policies from TOML string
    pub fn from_toml(content: &str) -> anyhow::Result<Self> {
        #[derive(Deserialize)]
        struct PolicyFile {
            policy: PolicyHeader,
            #[serde(rename = "rules", default)]
            rules: Vec<PolicyRule>,
        }

        #[derive(Deserialize)]
        struct PolicyHeader {
            name: String,
            version: String,
            #[serde(default)]
            priority: i32,
            #[serde(default = "default_true")]
            enabled: bool,
            #[serde(default = "default_deny")]
            default_effect: Effect,
        }

        fn default_true() -> bool {
            true
        }

        fn default_deny() -> Effect {
            Effect::Deny
        }

        let file: PolicyFile = toml::from_str(content)?;
        let mut policy = Policy::new(&file.policy.name, &file.policy.version)
            .with_priority(file.policy.priority)
            .with_default_effect(file.policy.default_effect);
        policy.enabled = file.policy.enabled;
        policy.rules = file.rules;

        let mut engine = Self::new();
        engine.add_policy(policy);
        Ok(engine)
    }

    /// Add a policy to the engine
    pub fn add_policy(&mut self, policy: Policy) {
        self.policies.insert(policy.name.clone(), policy);
    }

    /// Remove a policy by name
    pub fn remove_policy(&mut self, name: &str) -> Option<Policy> {
        self.policies.remove(name)
    }

    /// Evaluate an action against all policies
    pub fn evaluate(&self, action: &str, target: &str) -> PolicyVerdict {
        // Sort policies by priority (highest first)
        let mut sorted: Vec<&Policy> = self.policies
            .values()
            .filter(|p| p.enabled)
            .collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));

        for policy in sorted {
            for rule in &policy.rules {
                if rule.matches_action(action) && rule.matches_target(target) {
                    return match rule.effect {
                        Effect::Allow => PolicyVerdict::Allowed,
                        Effect::Deny => PolicyVerdict::Denied {
                            reason: format!(
                                "Denied by policy '{}' rule: {}",
                                policy.name,
                                rule.description.as_deref().unwrap_or("unnamed rule")
                            ),
                        },
                    };
                }
            }
        }

        // No rule matched — use the highest-priority policy's default
        if let Some(highest) = sorted.first() {
            match highest.default_effect {
                Effect::Allow => PolicyVerdict::Allowed,
                Effect::Deny => PolicyVerdict::Denied {
                    reason: format!("No matching rule in policy '{}'; default deny", highest.name),
                },
            }
        } else {
            PolicyVerdict::NoMatch
        }
    }

    /// List all policy names
    pub fn list_policies(&self) -> Vec<&str> {
        self.policies.keys().map(|s| s.as_str()).collect()
    }

    /// Get the number of active policies
    pub fn policy_count(&self) -> usize {
        self.policies.len()
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple glob-style pattern matching
fn glob_match(pattern: &str, text: &str) -> bool {
    glob::Pattern::new(pattern)
        .map(|p| p.matches(text))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_safety_policy() {
        let engine = PolicyEngine::with_defaults();

        // Should deny writing to /etc
        let verdict = engine.evaluate("file:write", "/etc/passwd");
        assert!(matches!(verdict, PolicyVerdict::Denied { .. }));

        // Should allow reading from workspace
        let verdict = engine.evaluate("file:read", "/workspace/data.txt");
        assert_eq!(verdict, PolicyVerdict::Allowed);

        // Should deny code execution
        let verdict = engine.evaluate("exec:shell", "/bin/bash");
        assert!(matches!(verdict, PolicyVerdict::Denied { .. }));
    }

    #[test]
    fn test_custom_policy() {
        let mut engine = PolicyEngine::new();
        let mut policy = Policy::new("custom", "1.0")
            .with_priority(50)
            .with_default_effect(Effect::Deny);

        policy.add_rule(PolicyRule {
            action: "network:connect".to_string(),
            effect: Effect::Allow,
            targets: vec!["api.example.com:*".to_string()],
            condition: None,
            description: Some("Allow API connections".to_string()),
        });

        engine.add_policy(policy);

        let verdict = engine.evaluate("network:connect", "api.example.com:443");
        assert_eq!(verdict, PolicyVerdict::Allowed);

        let verdict = engine.evaluate("network:connect", "evil.com:443");
        assert!(matches!(verdict, PolicyVerdict::Denied { .. }));
    }

    #[test]
    fn test_priority_ordering() {
        let mut engine = PolicyEngine::new();

        // Low-priority allow
        let mut low = Policy::new("low-priority", "1.0").with_priority(10);
        low.add_rule(PolicyRule {
            action: "file:read".to_string(),
            effect: Effect::Allow,
            targets: vec!["*".to_string()],
            condition: None,
            description: None,
        });
        engine.add_policy(low);

        // High-priority deny
        let mut high = Policy::new("high-priority", "1.0").with_priority(100);
        high.add_rule(PolicyRule {
            action: "file:read".to_string(),
            effect: Effect::Deny,
            targets: vec!["/secret/*".to_string()],
            condition: None,
            description: None,
        });
        engine.add_policy(high);

        // High priority deny should win
        let verdict = engine.evaluate("file:read", "/secret/key.pem");
        assert!(matches!(verdict, PolicyVerdict::Denied { .. }));

        // Low priority allow should apply elsewhere
        let verdict = engine.evaluate("file:read", "/workspace/file.txt");
        assert_eq!(verdict, PolicyVerdict::Allowed);
    }

    #[test]
    fn test_policy_rule_matching() {
        let rule = PolicyRule {
            action: "file:*".to_string(),
            effect: Effect::Deny,
            targets: vec!["/etc/*".to_string()],
            condition: None,
            description: None,
        };

        assert!(rule.matches_action("file:read"));
        assert!(rule.matches_action("file:write"));
        assert!(!rule.matches_action("network:connect"));

        assert!(rule.matches_target("/etc/passwd"));
        assert!(!rule.matches_target("/workspace/file.txt"));
    }
}

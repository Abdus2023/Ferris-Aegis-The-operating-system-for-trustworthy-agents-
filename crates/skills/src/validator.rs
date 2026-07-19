use crate::error::{SkillError, SkillResult};
use crate::types::*;
use std::collections::{HashMap, HashSet};
use petgraph::graph::{DiGraph, NodeIndex};
use ed25519_dalek::{VerifyingKey, Signature};

/// Validates skills: static schema checks, dependency resolution, policy enforcement.
pub struct SkillValidator;

impl SkillValidator {
    /// Perform static validation on a skill before execution.
    pub fn validate_static(skill: &Skill) -> SkillResult<()> {
        // 1. Validate frontmatter schema
        Self::validate_frontmatter(skill)?;
        // 2. Validate skill ID format
        Self::validate_skill_id(&skill.skill_id)?;
        // 3. Validate capabilities
        Self::validate_capabilities(&skill.capabilities_required)?;
        // 4. Validate triggers
        Self::validate_triggers(&skill.triggers)?;
        // 5. Validate policies
        Self::validate_policies(&skill.policies)?;
        Ok(())
    }

    /// Validate skill can execute in a given context.
    pub fn validate_execution(
        skill: &Skill,
        context: &SkillExecutionContext,
    ) -> SkillResult<()> {
        // 1. Trust level check
        let required_score = skill.trust_level_minimum.as_f64();
        if context.agent_trust_score < required_score {
            return Err(SkillError::TrustLevelInsufficient {
                required: format!("{}", skill.trust_level_minimum),
                actual: format!("{:.2}", context.agent_trust_score),
            });
        }

        // 2. Capability check
        for cap in &skill.capabilities_required {
            if !context.has_capability(cap) {
                return Err(SkillError::CapabilityDenied(cap.0.clone()));
            }
        }

        // 3. Resource limits check (early)
        Self::validate_resource_limits(skill)?;

        // 4. Deadline check
        if let Some(deadline) = context.deadline {
            if chrono::Utc::now() > deadline {
                return Err(SkillError::ExecutionTimeout);
            }
        }
        Ok(())
    }

    /// Validate frontmatter has all required fields.
    fn validate_frontmatter(skill: &Skill) -> SkillResult<()> {
        if skill.skill_version.is_empty() {
            return Err(SkillError::MissingRequiredField("skill_version".to_string()));
        }
        if skill.name.is_empty() {
            return Err(SkillError::MissingRequiredField("name".to_string()));
        }
        if skill.category.is_empty() {
            return Err(SkillError::MissingRequiredField("category".to_string()));
        }
        if skill.capabilities_required.is_empty() {
            return Err(SkillError::MissingRequiredField(
                "capabilities_required".to_string(),
            ));
        }
        Ok(())
    }

    /// Validate skill ID follows format: skill:<category>:<name>
    fn validate_skill_id(id: &SkillId) -> SkillResult<()> {
        let parts: Vec<&str> = id.0.split(':').collect();
        if parts.len() != 3 {
            return Err(SkillError::InvalidSkillId(id.0.clone()));
        }
        if parts[0] != "skill" {
            return Err(SkillError::InvalidSkillId(id.0.clone()));
        }
        if parts[1].is_empty() || parts[2].is_empty() {
            return Err(SkillError::InvalidSkillId(id.0.clone()));
        }
        Ok(())
    }

    /// Validate capability format: <domain>:<operation>[:<scope>]
    fn validate_capabilities(caps: &[Capability]) -> SkillResult<()> {
        for cap in caps {
            let parts: Vec<&str> = cap.0.split(':').collect();
            if parts.len() < 2 {
                return Err(SkillError::ValidationError(format!(
                    "Invalid capability format: {} (expected: domain:operation[:scope])",
                    cap
                )));
            }
        }
        Ok(())
    }

    /// Validate triggers have valid events and patterns.
    fn validate_triggers(triggers: &[Trigger]) -> SkillResult<()> {
        for trigger in triggers {
            if trigger.event.is_empty() {
                return Err(SkillError::ValidationError(
                    "Trigger must have an event".to_string(),
                ));
            }
            // Validate regex patterns if present
            if let Some(pattern) = &trigger.pattern {
                regex::Regex::new(pattern).map_err(|e| {
                    SkillError::ValidationError(format!("Invalid trigger pattern: {}", e))
                })?;
            }
        }
        Ok(())
    }

    /// Validate policies are well-formed.
    fn validate_policies(policies: &[PolicyRule]) -> SkillResult<()> {
        for policy in policies {
            if policy.id.is_empty() {
                return Err(SkillError::ValidationError(
                    "Policy must have an ID".to_string(),
                ));
            }
            if policy.rule.is_empty() {
                return Err(SkillError::ValidationError(
                    "Policy must have a rule".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Validate resource limits are parseable.
    fn validate_resource_limits(skill: &Skill) -> SkillResult<()> {
        // TODO: Parse and validate size/time strings (100MB, 30s, etc.)
        if skill.resource_limits.max_file_size.is_empty() {
            return Err(SkillError::ValidationError(
                "max_file_size cannot be empty".to_string(),
            ));
        }
        Ok(())
    }

    /// Verify a skill's signature using the provided public key.
    pub fn verify_signature(skill: &Skill, public_key: &[u8]) -> SkillResult<()> {
        let sig_data = skill.signature.as_ref()
            .ok_or(SkillError::SignatureVerificationFailed)?;
        let verifying_key = VerifyingKey::from_bytes(
            public_key
                .try_into()
                .map_err(|_| SkillError::SignatureVerificationFailed)?,
        )
        .map_err(|_| SkillError::SignatureVerificationFailed)?;
        let signature = Signature::from_bytes(
            sig_data
                .signature
                .as_bytes()
                .try_into()
                .map_err(|_| SkillError::SignatureVerificationFailed)?,
        );
        let signable_bytes = Self::get_signable_bytes(skill);
        verifying_key
            .verify(&signable_bytes, &signature)
            .map_err(|_| SkillError::SignatureVerificationFailed)
    }

    /// Get the bytes that should be signed (all metadata except signature).
    pub fn get_signable_bytes(skill: &Skill) -> Vec<u8> {
        let mut data = format!(
            "{}:{}:{}:{}",
            skill.skill_id, skill.version, skill.category, skill.author
        );
        for cap in &skill.capabilities_required {
            data.push(':');
            data.push_str(&cap.0);
        }
        data.into_bytes()
    }
}

/// Dependency resolver with cycle detection.
pub struct DependencyResolver;

impl DependencyResolver {
    /// Resolve all dependencies recursively, detecting cycles.
    pub fn resolve(
        skill: &Skill,
        registry: &super::registry::SkillRegistry,
    ) -> SkillResult<Vec<Skill>> {
        let mut graph = DiGraph::new();
        let mut resolved = HashMap::new();
        let mut visiting = HashSet::new();
        let root_idx = graph.add_node(skill.skill_id.clone());
        Self::resolve_recursive(
            skill,
            root_idx,
            &mut graph,
            &mut resolved,
            &mut visiting,
            registry,
        )?;
        Ok(resolved.into_values().collect())
    }

    fn resolve_recursive(
        skill: &Skill,
        node_idx: NodeIndex,
        graph: &mut DiGraph<SkillId, ()>,
        resolved: &mut HashMap<SkillId, Skill>,
        visiting: &mut HashSet<SkillId>,
        registry: &super::registry::SkillRegistry,
    ) -> SkillResult<()> {
        visiting.insert(skill.skill_id.clone());
        for dep in &skill.dependencies {
            match dep {
                Dependency::Skill {
                    skill: skill_ref,
                    version: _version,
                    optional,
                    fallback: _fallback,
                } => {
                    let dep_skill = registry.get(skill_ref).ok_or_else(|| {
                        if *optional {
                            tracing::warn!("Optional dependency not found: {}", skill_ref);
                            return SkillError::DependencyNotFound {
                                skill: skill_ref.clone(),
                                version: "unknown".to_string(),
                            };
                        }
                        SkillError::DependencyNotFound {
                            skill: skill_ref.clone(),
                            version: "unknown".to_string(),
                        }
                    })?;
                    // Cycle detection
                    if visiting.contains(&dep_skill.skill_id) {
                        return Err(SkillError::CircularDependency(format!(
                            "{} -> {}",
                            skill.skill_id, dep_skill.skill_id
                        )));
                    }
                    let dep_idx = graph.add_node(dep_skill.skill_id.clone());
                    graph.add_edge(node_idx, dep_idx, ());
                    if !resolved.contains_key(&dep_skill.skill_id) {
                        resolved.insert(dep_skill.skill_id.clone(), dep_skill.clone());
                        Self::resolve_recursive(
                            dep_skill,
                            dep_idx,
                            graph,
                            resolved,
                            visiting,
                            registry,
                        )?;
                    }
                }
                _ => {} // System tools and crates don't have transitive deps in this model
            }
        }
        visiting.remove(&skill.skill_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_skill_id() {
        let valid_id = SkillId("skill:filesystem:test".to_string());
        assert!(SkillValidator::validate_skill_id(&valid_id).is_ok());
        let invalid_id = SkillId("invalid:format".to_string());
        assert!(SkillValidator::validate_skill_id(&invalid_id).is_err());
    }

    #[test]
    fn test_validate_capabilities() {
        let valid = vec![
            Capability("filesystem:read".to_string()),
            Capability("network:connect".to_string()),
        ];
        assert!(SkillValidator::validate_capabilities(&valid).is_ok());
        let invalid = vec![Capability("invalid".to_string())];
        assert!(SkillValidator::validate_capabilities(&invalid).is_err());
    }
}
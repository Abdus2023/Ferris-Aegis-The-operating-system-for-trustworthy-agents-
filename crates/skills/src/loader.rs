use crate::error::{SkillError, SkillResult};
use crate::types::*;
use std::fs;
use std::path::Path;
use yaml_rust2::{Yaml, YamlLoader};
use chrono::Utc;

/// Parses SKILL.md frontmatter and content.
pub struct FrontmatterParser;

impl FrontmatterParser {
    /// Parse frontmatter from a string.
    /// Expects format:
    /// ```
    /// ---
    /// key: value
    /// ---
    /// # Content here
    /// ```
    pub fn parse(content: &str) -> SkillResult<(Skill, String)> {
        // Split on frontmatter delimiters
        let parts: Vec<&str> = content.splitn(3, "---").collect();

        if parts.len() < 3 {
            return Err(SkillError::FrontmatterParseError(
                "Missing frontmatter delimiters (---)".to_string(),
            ));
        }

        let frontmatter_str = parts[1].trim();
        let body = parts[2].trim().to_string();

        // Parse YAML frontmatter
        let yaml_docs = YamlLoader::load_from_str(frontmatter_str)?;
        if yaml_docs.is_empty() {
            return Err(SkillError::FrontmatterParseError(
                "Empty frontmatter".to_string(),
            ));
        }

        let yaml = &yaml_docs[0];
        let mut skill = Self::yaml_to_skill(&yaml)?;
        skill.content = body;

        Ok((skill, skill.content.clone()))
    }

    /// Convert YAML frontmatter to Skill struct.
    fn yaml_to_skill(yaml: &Yaml) -> SkillResult<Skill> {
        let get_str = |key: &str| {
            yaml[key]
                .as_str()
                .ok_or_else(|| SkillError::MissingRequiredField(key.to_string()))
        };

        let skill_id_str = get_str("skill_id")?;
        let skill_id = SkillId(skill_id_str.to_string());

        // Validate skill ID format
        if !skill_id_str.starts_with("skill:") {
            return Err(SkillError::InvalidSkillId(skill_id_str.to_string()));
        }

        let capabilities_required = yaml["capabilities_required"]
            .as_vec()
            .ok_or_else(|| SkillError::MissingRequiredField("capabilities_required".to_string()))?
            .iter()
            .filter_map(|y| y.as_str().map(|s| Capability(s.to_string())))
            .collect();

        let trust_level_str = get_str("trust_level_minimum")?;
        let trust_level_minimum = Self::parse_trust_level(trust_level_str)?;

        let resource_limits = ResourceLimits {
            max_file_size: yaml["resource_limits"]["max_file_size"]
                .as_str()
                .unwrap_or("100MB")
                .to_string(),
            max_execution_time: yaml["resource_limits"]["max_execution_time"]
                .as_str()
                .unwrap_or("30s")
                .to_string(),
            max_memory: yaml["resource_limits"]["max_memory"]
                .as_str()
                .unwrap_or("256MB")
                .to_string(),
            max_concurrent_calls: yaml["resource_limits"]["max_concurrent_calls"]
                .as_i64()
                .unwrap_or(5) as usize,
        };

        let policies = yaml["policies"]
            .as_vec()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|y| {
                let id = y["id"].as_str()?.to_string();
                let rule = y["rule"].as_str()?.to_string();
                let effect_str = y["effect"].as_str().unwrap_or("deny");
                let effect = match effect_str {
                    "allow" => PolicyEffect::Allow,
                    "alert" => PolicyEffect::Alert,
                    _ => PolicyEffect::Deny,
                };
                Some(PolicyRule { id, rule, effect })
            })
            .collect();

        let tags = yaml["tags"]
            .as_vec()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|y| y.as_str().map(|s| s.to_string()))
            .collect();

        let keywords = yaml["keywords"]
            .as_vec()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|y| y.as_str().map(|s| s.to_string()))
            .collect();

        let dependencies = Self::parse_dependencies(&yaml)?;
        let triggers = Self::parse_triggers(&yaml)?;
        let compatible_agents = Self::parse_compatible_agents(&yaml)?;

        Ok(Skill {
            skill_version: get_str("skill_version")?.to_string(),
            skill_id,
            name: get_str("name")?.to_string(),
            category: get_str("category")?.to_string(),
            description: get_str("description")?.to_string(),
            version: get_str("version")?.to_string(),
            author: get_str("author")?.to_string(),
            license: get_str("license")?.to_string(),
            tags,
            keywords,
            maintainer: yaml["maintainer"].as_str().map(|s| s.to_string()),
            capabilities_required,
            trust_level_minimum,
            sandbox_boundary: get_str("sandbox_boundary")?.to_string(),
            dependencies,
            triggers,
            resource_limits,
            policies,
            execution_protocol: get_str("execution_protocol")?.to_string(),
            protocol_version: get_str("protocol_version")?.to_string(),
            export_format: get_str("export_format")?.to_string(),
            compatible_agents,
            signature: None,
            content: String::new(),
        })
    }

    fn parse_trust_level(s: &str) -> SkillResult<TrustLevelRequired> {
        match s.to_lowercase().as_str() {
            "unverified" => Ok(TrustLevelRequired::Unverified),
            "probationary" => Ok(TrustLevelRequired::Probationary),
            "standard" => Ok(TrustLevelRequired::Standard),
            "elevated" => Ok(TrustLevelRequired::Elevated),
            "sovereign" => Ok(TrustLevelRequired::Sovereign),
            _ => Err(SkillError::ValidationError(
                format!("Invalid trust level: {}", s),
            )),
        }
    }

    fn parse_dependencies(yaml: &Yaml) -> SkillResult<Vec<Dependency>> {
        let mut deps = vec![];

        if let Some(dep_list) = yaml["dependencies"].as_vec() {
            for dep_yaml in dep_list {
                if let Some(skill) = dep_yaml["skill"].as_str() {
                    deps.push(Dependency::Skill {
                        skill: skill.to_string(),
                        version: dep_yaml["version"]
                            .as_str()
                            .unwrap_or("*")
                            .to_string(),
                        optional: dep_yaml["optional"].as_bool().unwrap_or(false),
                        fallback: dep_yaml["fallback"].as_bool().unwrap_or(true),
                    });
                } else if let Some(_) = dep_yaml["system"].as_hash() {
                    let mut tools = std::collections::HashMap::new();
                    if let Some(sys_tools) = dep_yaml["system"].as_hash() {
                        for (k, v) in sys_tools {
                            if let (Some(tool_name), Some(version)) = (k.as_str(), v.as_str()) {
                                tools.insert(tool_name.to_string(), version.to_string());
                            }
                        }
                    }
                    deps.push(Dependency::SystemTool { tools });
                } else if let Some(crate_name) = dep_yaml["crate"].as_str() {
                    deps.push(Dependency::Crate {
                        name: crate_name.to_string(),
                        version: dep_yaml["version"]
                            .as_str()
                            .unwrap_or("*")
                            .to_string(),
                    });
                }
            }
        }

        Ok(deps)
    }

    fn parse_triggers(yaml: &Yaml) -> SkillResult<Vec<Trigger>> {
        let mut triggers = vec![];

        if let Some(trigger_list) = yaml["triggers"].as_vec() {
            for trigger_yaml in trigger_list {
                if let Some(event) = trigger_yaml["event"].as_str() {
                    triggers.push(Trigger {
                        event: event.to_string(),
                        action: trigger_yaml["action"].as_str().map(|s| s.to_string()),
                        pattern: trigger_yaml["pattern"].as_str().map(|s| s.to_string()),
                        weight: trigger_yaml["weight"].as_i64().unwrap_or(50) as u32,
                    });
                }
            }
        }

        Ok(triggers)
    }

    fn parse_compatible_agents(yaml: &Yaml) -> SkillResult<Vec<AgentCompatibility>> {
        let mut agents = vec![];

        if let Some(agent_list) = yaml["compatible_agents"].as_vec() {
            for agent_yaml in agent_list {
                if let Some(name) = agent_yaml["name"].as_str() {
                    let features = agent_yaml["features"]
                        .as_vec()
                        .unwrap_or(&vec![])
                        .iter()
                        .filter_map(|y| y.as_str().map(|s| s.to_string()))
                        .collect();

                    agents.push(AgentCompatibility {
                        name: name.to_string(),
                        min_version: agent_yaml["min_version"]
                            .as_str()
                            .unwrap_or("1.0")
                            .to_string(),
                        features,
                    });
                }
            }
        }

        Ok(agents)
    }
}

/// Loads SKILL.md files from disk.
pub struct SkillLoader;

impl SkillLoader {
    /// Load a single skill from a file.
    pub fn from_file(path: &Path) -> SkillResult<Skill> {
        let content = fs::read_to_string(path).map_err(|e| SkillError::IoError(e))?;
        let (skill, _) = FrontmatterParser::parse(&content)?;
        Ok(skill)
    }

    /// Load all skills from a directory.
    pub fn from_directory(dir: &Path) -> SkillResult<Vec<Skill>> {
        let mut skills = vec![];

        for entry in fs::read_dir(dir).map_err(|e| SkillError::IoError(e))? {
            let entry = entry.map_err(|e| SkillError::IoError(e))?;
            let path = entry.path();

            if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
                match Self::from_file(&path) {
                    Ok(skill) => {
                        tracing::debug!("Loaded skill: {} from {}", skill.skill_id, path.display());
                        skills.push(skill);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load skill from {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(skills)
    }

    /// Load all skills recursively from a directory tree.
    pub fn from_directory_recursive(dir: &Path) -> SkillResult<Vec<Skill>> {
        let mut skills = vec![];

        fn walk_dir(dir: &Path, skills: &mut Vec<Skill>) -> SkillResult<()> {
            for entry in fs::read_dir(dir).map_err(|e| SkillError::IoError(e))? {
                let entry = entry.map_err(|e| SkillError::IoError(e))?;
                let path = entry.path();

                if path.is_dir() {
                    walk_dir(&path, skills)?;
                } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
                    match SkillLoader::from_file(&path) {
                        Ok(skill) => {
                            tracing::debug!(
                                "Loaded skill: {} from {}",
                                skill.skill_id,
                                path.display()
                            );
                            skills.push(skill);
                        }
                        Err(e) => {
                            tracing::warn!("Failed to load skill from {}: {}", path.display(), e);
                        }
                    }
                }
            }
            Ok(())
        }

        walk_dir(dir, &mut skills)?;
        Ok(skills)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter() {
        let content = r#"---
skill_version: "1.0.0"
skill_id: "skill:filesystem:test"
name: "Test Skill"
category: "filesystem"
description: "A test skill"
version: "1.0.0"
author: "Test"
license: "MIT"
capabilities_required:
  - filesystem:read
trust_level_minimum: "standard"
sandbox_boundary: "restricted"
execution_protocol: "aegis:rpc/1.0"
protocol_version: "V_2025_11_25"
export_format: "mcp-tool"
---
# Content here
"#;

        let (skill, body) = FrontmatterParser::parse(content).unwrap();
        assert_eq!(skill.skill_id.0, "skill:filesystem:test");
        assert_eq!(skill.name, "Test Skill");
        assert!(body.contains("Content here"));
    }
}

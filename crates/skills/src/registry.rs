use crate::error::{SkillError, SkillResult};
use crate::types::*;
use crate::loader::{SkillLoader, FrontmatterParser};
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

/// In-memory skill cache with LRU eviction.
#[derive(Debug)]
pub struct SkillCache {
    cache: LruCache<SkillId, Arc<Skill>>,
}

impl SkillCache {
    /// Create a new cache with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: LruCache::new(NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(100).unwrap())),
        }
    }

    /// Get a skill from the cache.
    pub fn get(&mut self, id: &SkillId) -> Option<Arc<Skill>> {
        self.cache.get(id).cloned()
    }

    /// Put a skill in the cache.
    pub fn put(&mut self, id: SkillId, skill: Arc<Skill>) {
        self.cache.put(id, skill);
    }

    /// Remove a skill from the cache.
    pub fn remove(&mut self, id: &SkillId) -> Option<Arc<Skill>> {
        self.cache.pop(id)
    }

    /// Get the current cache size.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Clear the cache.
    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

/// Skill registry for discovery, loading, and caching.
#[derive(Debug)]
pub struct SkillRegistry {
    /// All loaded skills indexed by skill ID.
    skills: HashMap<SkillId, Arc<Skill>>,
    /// Skills indexed by category.
    by_category: HashMap<String, Vec<SkillId>>,
    /// Skills indexed by capability.
    by_capability: HashMap<String, Vec<SkillId>>,
    /// LRU cache for recently accessed skills.
    cache: Arc<RwLock<SkillCache>>,
    /// Base directory for skill files.
    base_dir: Option<std::path::PathBuf>,
}

impl SkillRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
            by_category: HashMap::new(),
            by_capability: HashMap::new(),
            cache: Arc::new(RwLock::new(SkillCache::new(100))),
            base_dir: None,
        }
    }

    /// Create a new registry with a base directory.
    pub fn with_base_dir(dir: impl AsRef<Path>) -> Self {
        let mut registry = Self::new();
        registry.base_dir = Some(dir.as_ref().to_path_buf());
        registry
    }

    /// Load a single skill from a file.
    pub fn load_from_file(&mut self, path: &Path) -> SkillResult<Arc<Skill>> {
        let skill = SkillLoader::from_file(path)?;
        self.register_skill(skill)
    }

    /// Load all skills from a directory (non-recursive).
    pub fn load_from_directory(&mut self, dir: &Path) -> SkillResult<usize> {
        let skills = SkillLoader::from_directory(dir)?;
        let count = skills.len();
        for skill in skills {
            self.register_skill(skill)?;
        }
        Ok(count)
    }

    /// Load all skills from a directory recursively.
    pub fn load_from_directory_recursive(&mut self, dir: &Path) -> SkillResult<usize> {
        let skills = SkillLoader::from_directory_recursive(dir)?;
        let count = skills.len();
        for skill in skills {
            self.register_skill(skill)?;
        }
        Ok(count)
    }

    /// Register a skill in the registry.
    fn register_skill(&mut self, skill: Skill) -> SkillResult<Arc<Skill>> {
        // Validate the skill first
        crate::validator::SkillValidator::validate_static(&skill)?;

        let skill_id = skill.skill_id.clone();
        let category = skill.category.clone();
        let capabilities: Vec<String> = skill.capabilities_required.iter().map(|c| c.0.clone()).collect();
        
        let arc_skill = Arc::new(skill);

        // Add to main index
        self.skills.insert(skill_id.clone(), arc_skill.clone());

        // Add to category index
        self.by_category
            .entry(category)
            .or_default()
            .push(skill_id.clone());

        // Add to capability index
        for cap in capabilities {
            self.by_capability
                .entry(cap)
                .or_default()
                .push(skill_id.clone());
        }

        // Update cache
        let mut cache = self.cache.blocking_write();
        cache.put(skill_id.clone(), arc_skill.clone());

        tracing::debug!("Registered skill: {}", skill_id);
        Ok(arc_skill)
    }

    /// Get a skill by ID (checks cache first).
    pub async fn get(&self, id: &str) -> Option<Arc<Skill>> {
        let skill_id = SkillId(id.to_string());
        
        // Check cache first
        {
            let mut cache = self.cache.write().await;
            if let Some(skill) = cache.get(&skill_id) {
                return Some(skill);
            }
        }

        // Fall back to main index
        self.skills.get(&skill_id).cloned()
    }

    /// Get a skill by ID synchronously (checks cache first).
    pub fn get_sync(&self, id: &str) -> Option<Arc<Skill>> {
        let skill_id = SkillId(id.to_string());
        
        // Check cache first
        if let Ok(mut cache) = self.cache.try_write() {
            if let Some(skill) = cache.get(&skill_id) {
                return Some(skill);
            }
        }

        // Fall back to main index
        self.skills.get(&skill_id).cloned()
    }

    /// List all skills in a category.
    pub fn by_category(&self, category: &str) -> Vec<Arc<Skill>> {
        self.by_category
            .get(category)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.skills.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// List all skills requiring a specific capability.
    pub fn by_capability(&self, capability: &str) -> Vec<Arc<Skill>> {
        self.by_capability
            .get(capability)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.skills.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// List all skill IDs.
    pub fn list_ids(&self) -> Vec<&SkillId> {
        self.skills.keys().collect()
    }

    /// List all skills.
    pub fn list_all(&self) -> Vec<Arc<Skill>> {
        self.skills.values().cloned().collect()
    }

    /// Get the number of registered skills.
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    /// Search skills by name or keyword.
    pub fn search(&self, query: &str) -> Vec<Arc<Skill>> {
        let query_lower = query.to_lowercase();
        self.skills
            .values()
            .filter(|skill| {
                skill.name.to_lowercase().contains(&query_lower)
                    || skill.description.to_lowercase().contains(&query_lower)
                    || skill.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
                    || skill.keywords.iter().any(|k| k.to_lowercase().contains(&query_lower))
            })
            .cloned()
            .collect()
    }

    /// Get skills compatible with a specific agent/runtime.
    pub fn compatible_with(&self, agent_name: &str, min_version: &str) -> Vec<Arc<Skill>> {
        self.skills
            .values()
            .filter(|skill| {
                skill.compatible_agents.iter().any(|a| {
                    a.name == agent_name 
                        && version_check(&a.min_version, min_version)
                })
            })
            .cloned()
            .collect()
    }

    /// Remove a skill from the registry.
    pub fn remove(&mut self, id: &SkillId) -> Option<Arc<Skill>> {
        if let Some(skill) = self.skills.remove(id) {
            // Remove from category index
            if let Some(vec) = self.by_category.get_mut(&skill.category) {
                vec.retain(|sid| sid != id);
            }

            // Remove from capability index
            for cap in &skill.capabilities_required {
                if let Some(vec) = self.by_capability.get_mut(&cap.0) {
                    vec.retain(|sid| sid != id);
                }
            }

            // Remove from cache
            if let Ok(mut cache) = self.cache.try_write() {
                cache.remove(id);
            }

            Some(skill)
        } else {
            None
        }
    }

    /// Clear all skills from the registry.
    pub fn clear(&mut self) {
        self.skills.clear();
        self.by_category.clear();
        self.by_capability.clear();
        if let Ok(mut cache) = self.cache.try_write() {
            cache.clear();
        }
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple version comparison (semver-like).
fn version_check(required: &str, available: &str) -> bool {
    // Simplified: just check if available >= required for major.minor
    // In production, use semver crate
    if required == "*" || required == "any" {
        return true;
    }
    
    // Strip comparison operators
    let required = required.trim_start_matches(">=").trim_start_matches(">").trim_start_matches("~").trim_start_matches("^");
    let available = available.trim_start_matches(">=").trim_start_matches(">").trim_start_matches("~").trim_start_matches("^");
    
    // Parse major.minor.patch
    let req_parts: Vec<u32> = required.split('.').filter_map(|s| s.parse().ok()).collect();
    let avl_parts: Vec<u32> = available.split('.').filter_map(|s| s.parse().ok()).collect();
    
    if req_parts.is_empty() || avl_parts.is_empty() {
        return true; // If we can't parse, allow it
    }
    
    // Compare major, then minor, then patch
    for (r, a) in req_parts.iter().zip(avl_parts.iter()) {
        if a > r {
            return true;
        } else if a < r {
            return false;
        }
    }
    
    true // Equal or available has more components
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_skill_file(dir: &Path, name: &str, category: &str, caps: &[&str]) {
        let caps_yaml = caps.iter().map(|c| format!("  - \"{}\"", c)).collect::<Vec<_>>().join("\n");
        let content = format!(r#"---
spec_version: "1.0.0"
skill_id: "skill:{}:{}"
name: "{} Skill"
category: "{}"
description: "A test skill"
version: "1.0.0"
author: "Test"
license: "MIT"
capabilities_required:
{}
trust_level_minimum: "probationary"
sandbox_boundary: "restricted"
execution_protocol: "aegis:rpc/1.0"
protocol_version: "V_2025_11_25"
export_format: "mcp-tool"
---
# Test skill content
"#, category, name, name, category, caps_yaml);

        let path = dir.join(format!("{}.md", name));
        fs::write(path, content).unwrap();
    }

    #[test]
    fn test_registry_load_directory() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path();

        create_test_skill_file(dir, "file-reader", "filesystem", &["filesystem:read"]);
        create_test_skill_file(dir, "file-writer", "filesystem", &["filesystem:write"]);
        create_test_skill_file(dir, "web-fetcher", "network", &["network:http"]);

        let mut registry = SkillRegistry::new();
        let count = registry.load_from_directory(dir).unwrap();
        
        assert_eq!(count, 3);
        assert_eq!(registry.len(), 3);
    }

    #[test]
    fn test_registry_by_category() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path();

        create_test_skill_file(dir, "file-reader", "filesystem", &["filesystem:read"]);
        create_test_skill_file(dir, "web-fetcher", "network", &["network:http"]);

        let mut registry = SkillRegistry::new();
        registry.load_from_directory(dir).unwrap();

        let fs_skills = registry.by_category("filesystem");
        assert_eq!(fs_skills.len(), 1);
        assert_eq!(fs_skills[0].skill_id.0, "skill:filesystem:file-reader");

        let net_skills = registry.by_category("network");
        assert_eq!(net_skills.len(), 1);
        assert_eq!(net_skills[0].skill_id.0, "skill:network:web-fetcher");
    }

    #[test]
    fn test_registry_by_capability() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path();

        create_test_skill_file(dir, "file-reader", "filesystem", &["filesystem:read"]);
        create_test_skill_file(dir, "file-writer", "filesystem", &["filesystem:write"]);
        create_test_skill_file(dir, "web-fetcher", "network", &["network:http"]);

        let mut registry = SkillRegistry::new();
        registry.load_from_directory(dir).unwrap();

        let read_skills = registry.by_capability("filesystem:read");
        assert_eq!(read_skills.len(), 1);

        let write_skills = registry.by_capability("filesystem:write");
        assert_eq!(write_skills.len(), 1);

        let http_skills = registry.by_capability("network:http");
        assert_eq!(http_skills.len(), 1);
    }

    #[test]
    fn test_registry_search() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path();

        create_test_skill_file(dir, "file-reader", "filesystem", &["filesystem:read"]);
        create_test_skill_file(dir, "web-fetcher", "network", &["network:http"]);

        let mut registry = SkillRegistry::new();
        registry.load_from_directory(dir).unwrap();

        let results = registry.search("file");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].skill_id.0, "skill:filesystem:file-reader");

        let results = registry.search("web");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].skill_id.0, "skill:network:web-fetcher");

        let results = registry.search("nonexistent");
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_version_check() {
        assert!(version_check("1.0.0", "1.0.0"));
        assert!(version_check("1.0.0", "1.0.1"));
        assert!(version_check("1.0.0", "1.1.0"));
        assert!(version_check("1.0.0", "2.0.0"));
        assert!(!version_check("1.1.0", "1.0.0"));
        assert!(!version_check("2.0.0", "1.0.0"));
        assert!(version_check("*", "any"));
    }
}
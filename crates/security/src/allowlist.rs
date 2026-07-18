//! Tool allowlist — deny-by-default tool dispatch.
//!
//! Every tool call must pass through the allowlist check before execution.
//! Unknown tools return an error. This is the first gate in the security
//! pipeline.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// The verdict returned by the allowlist check.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AllowlistVerdict {
    /// The tool is on the allowlist and may proceed.
    Allowed,
    /// The tool is not on the allowlist and must be rejected.
    Denied,
}

impl AllowlistVerdict {
    /// Whether the tool call is allowed to proceed.
    pub fn is_allowed(&self) -> bool {
        matches!(self, AllowlistVerdict::Allowed)
    }
}

/// A deny-by-default tool allowlist.
///
/// Only tools explicitly registered in the allowlist may be called.
/// Everything else is denied. This is the correct default for an
/// agent operating system — agents start with nothing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAllowlist {
    /// The set of allowed tool names.
    allowed: HashSet<String>,
}

impl ToolAllowlist {
    /// Create an empty allowlist (everything denied).
    pub fn new() -> Self {
        Self {
            allowed: HashSet::new(),
        }
    }

    /// Create an allowlist with the default safe tools.
    pub fn default_safe() -> Self {
        let mut list = Self::new();
        list.allow("file_read");
        list
    }

    /// Add a tool to the allowlist.
    pub fn allow(&mut self, tool_name: &str) {
        self.allowed.insert(tool_name.to_string());
    }

    /// Remove a tool from the allowlist.
    pub fn deny(&mut self, tool_name: &str) {
        self.allowed.remove(tool_name);
    }

    /// Check whether a tool call is allowed.
    pub fn check(&self, tool_name: &str) -> AllowlistVerdict {
        if self.allowed.contains(tool_name) {
            AllowlistVerdict::Allowed
        } else {
            AllowlistVerdict::Denied
        }
    }

    /// List all allowed tools.
    pub fn allowed_tools(&self) -> Vec<&str> {
        self.allowed.iter().map(|s| s.as_str()).collect()
    }

    /// Number of allowed tools.
    pub fn len(&self) -> usize {
        self.allowed.len()
    }

    /// Whether the allowlist is empty.
    pub fn is_empty(&self) -> bool {
        self.allowed.is_empty()
    }
}

impl Default for ToolAllowlist {
    fn default() -> Self {
        Self::default_safe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_allowlist_denies_everything() {
        let list = ToolAllowlist::new();
        assert_eq!(list.check("file_read"), AllowlistVerdict::Denied);
        assert_eq!(list.check("exec_shell"), AllowlistVerdict::Denied);
    }

    #[test]
    fn default_safe_allows_file_read() {
        let list = ToolAllowlist::default_safe();
        assert_eq!(list.check("file_read"), AllowlistVerdict::Allowed);
        assert_eq!(list.check("exec_shell"), AllowlistVerdict::Denied);
    }

    #[test]
    fn add_and_remove_tools() {
        let mut list = ToolAllowlist::new();
        list.allow("network_get");
        assert_eq!(list.check("network_get"), AllowlistVerdict::Allowed);

        list.deny("network_get");
        assert_eq!(list.check("network_get"), AllowlistVerdict::Denied);
    }

    #[test]
    fn unknown_tools_denied() {
        let mut list = ToolAllowlist::default_safe();
        list.allow("file_read");
        // Even with file_read allowed, anything else is still denied
        assert_eq!(list.check("file_write"), AllowlistVerdict::Denied);
        assert_eq!(list.check("exec_bash"), AllowlistVerdict::Denied);
        assert_eq!(list.check("make_me_coffee"), AllowlistVerdict::Denied);
    }
}

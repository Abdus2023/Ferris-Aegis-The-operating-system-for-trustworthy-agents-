//! Ferris Aegis Session — Conversation session management.
//!
//! Sessions group related conversation turns for an agent. Each session
//! carries a unique identifier, tracks context, and supports serialization
//! for persistence and the Agent-to-Agent protocol.
//!
//! # Design
//!
//! A session is the unit of multi-turn conversation for a single agent.
//! Sessions are:
//! - **Identifiable** — each session has a UUID-based ID
//! - **Clonable** — sessions can be shared across threads and async tasks
//! - **Serializable** — sessions can be persisted and transmitted via A2A
//! - **Contextual** — sessions track metadata about the conversation context

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A conversation session for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier.
    pub id: String,
    /// The agent this session belongs to.
    pub agent_id: String,
    /// The current conversation turn (0-based).
    pub turn: u64,
    /// The session context (e.g. "research", "code-review").
    pub context: String,
    /// Whether the session is active.
    pub active: bool,
    /// When the session was created.
    pub created_at: DateTime<Utc>,
    /// When the session was last active.
    pub last_active: DateTime<Utc>,
    /// Arbitrary metadata (model, temperature, etc.).
    pub metadata: serde_json::Value,
}

impl Session {
    /// Create a new session for an agent.
    pub fn new(agent_id: &str, context: &str) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            turn: 0,
            context: context.to_string(),
            active: true,
            created_at: now,
            last_active: now,
            metadata: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// Advance the session to the next turn.
    pub fn advance_turn(&mut self) {
        self.turn += 1;
        self.last_active = Utc::now();
    }

    /// Deactivate the session.
    pub fn deactivate(&mut self) {
        self.active = false;
        self.last_active = Utc::now();
    }

    /// Reactivate the session.
    pub fn activate(&mut self) {
        self.active = true;
        self.last_active = Utc::now();
    }

    /// Set a metadata value.
    pub fn set_metadata(&mut self, key: &str, value: serde_json::Value) {
        if let Some(obj) = self.metadata.as_object_mut() {
            obj.insert(key.to_string(), value);
        }
    }

    /// Get a metadata value.
    pub fn get_metadata(&self, key: &str) -> Option<&serde_json::Value> {
        self.metadata.as_object()?.get(key)
    }

    /// Duration since the session was last active.
    pub fn idle_duration(&self) -> chrono::Duration {
        Utc::now() - self.last_active
    }

    /// Whether the session has been idle for longer than the given duration.
    pub fn is_idle_longer_than(&self, duration: chrono::Duration) -> bool {
        self.idle_duration() > duration
    }
}

/// A session manager that tracks all active sessions for an agent.
#[derive(Debug, Clone)]
pub struct SessionManager {
    /// Active sessions indexed by session ID.
    sessions: std::collections::HashMap<String, Session>,
}

impl SessionManager {
    /// Create a new empty session manager.
    pub fn new() -> Self {
        Self {
            sessions: std::collections::HashMap::new(),
        }
    }

    /// Create a new session for an agent.
    pub fn create_session(&mut self, agent_id: &str, context: &str) -> Session {
        let session = Session::new(agent_id, context);
        self.sessions.insert(session.id.clone(), session.clone());
        tracing::info!(
            session_id = %session.id,
            agent_id = agent_id,
            context = context,
            "Session created"
        );
        session
    }

    /// Get a session by ID.
    pub fn get_session(&self, session_id: &str) -> Option<&Session> {
        self.sessions.get(session_id)
    }

    /// Get a mutable session by ID.
    pub fn get_session_mut(&mut self, session_id: &str) -> Option<&mut Session> {
        self.sessions.get_mut(session_id)
    }

    /// List all active sessions for an agent.
    pub fn active_sessions_for(&self, agent_id: &str) -> Vec<&Session> {
        self.sessions
            .values()
            .filter(|s| s.agent_id == agent_id && s.active)
            .collect()
    }

    /// Deactivate all sessions for an agent.
    pub fn deactivate_agent_sessions(&mut self, agent_id: &str) {
        for session in self.sessions.values_mut() {
            if session.agent_id == agent_id && session.active {
                session.deactivate();
            }
        }
    }

    /// Remove a session.
    pub fn remove_session(&mut self, session_id: &str) -> Option<Session> {
        let session = self.sessions.remove(session_id);
        if session.is_some() {
            tracing::info!(session_id = session_id, "Session removed");
        }
        session
    }

    /// Advance the turn of a session.
    pub fn advance_session(&mut self, session_id: &str) -> anyhow::Result<()> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {session_id}"))?;
        session.advance_turn();
        Ok(())
    }

    /// Number of active sessions.
    pub fn active_count(&self) -> usize {
        self.sessions.values().filter(|s| s.active).count()
    }

    /// Total number of sessions (active + inactive).
    pub fn total_count(&self) -> usize {
        self.sessions.len()
    }

    /// Clean up inactive sessions older than the given duration.
    pub fn cleanup(&mut self, max_idle: chrono::Duration) -> usize {
        let before = self.sessions.len();
        self.sessions
            .retain(|_, s| s.active || !s.is_idle_longer_than(max_idle));
        before - self.sessions.len()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_creation() {
        let session = Session::new("agent-1", "research");
        assert_eq!(session.agent_id, "agent-1");
        assert_eq!(session.context, "research");
        assert_eq!(session.turn, 0);
        assert!(session.active);
    }

    #[test]
    fn session_clone_is_derived() {
        let session = Session::new("agent-1", "test");
        let cloned = session.clone();
        assert_eq!(session.id, cloned.id);
        assert_eq!(session.agent_id, cloned.agent_id);
        assert_eq!(session.turn, cloned.turn);
    }

    #[test]
    fn session_turn_advance() {
        let mut session = Session::new("agent-1", "test");
        assert_eq!(session.turn, 0);
        session.advance_turn();
        assert_eq!(session.turn, 1);
        session.advance_turn();
        assert_eq!(session.turn, 2);
    }

    #[test]
    fn session_activate_deactivate() {
        let mut session = Session::new("agent-1", "test");
        assert!(session.active);
        session.deactivate();
        assert!(!session.active);
        session.activate();
        assert!(session.active);
    }

    #[test]
    fn session_metadata() {
        let mut session = Session::new("agent-1", "test");
        session.set_metadata("model", serde_json::json!("gpt-4"));
        assert_eq!(
            session.get_metadata("model").unwrap(),
            &serde_json::json!("gpt-4")
        );
    }

    #[test]
    fn session_manager_create_and_retrieve() {
        let mut manager = SessionManager::new();
        let session = manager.create_session("agent-1", "research");
        let retrieved = manager.get_session(&session.id).unwrap();
        assert_eq!(retrieved.agent_id, "agent-1");
    }

    #[test]
    fn session_manager_active_sessions() {
        let mut manager = SessionManager::new();
        manager.create_session("agent-1", "coding");
        manager.create_session("agent-1", "research");
        manager.create_session("agent-2", "debugging");

        assert_eq!(manager.active_sessions_for("agent-1").len(), 2);
        assert_eq!(manager.active_sessions_for("agent-2").len(), 1);
        assert_eq!(manager.active_count(), 3);
    }

    #[test]
    fn session_manager_deactivate_agent() {
        let mut manager = SessionManager::new();
        manager.create_session("agent-1", "coding");
        manager.create_session("agent-1", "research");

        assert_eq!(manager.active_sessions_for("agent-1").len(), 2);
        manager.deactivate_agent_sessions("agent-1");
        assert_eq!(manager.active_sessions_for("agent-1").len(), 0);
    }

    #[test]
    fn session_serialization_roundtrip() {
        let session = Session::new("agent-1", "test");
        let json = serde_json::to_string(&session).unwrap();
        let deserialized: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(session.id, deserialized.id);
        assert_eq!(session.agent_id, deserialized.agent_id);
        assert_eq!(session.turn, deserialized.turn);
    }
}

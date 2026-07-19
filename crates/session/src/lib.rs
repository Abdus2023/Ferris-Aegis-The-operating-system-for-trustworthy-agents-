//! Ferris Aegis Session Manager — Four-field budget tracking for agent sessions.
//!
//! Every agent session is bounded by a four-field budget checked at the
//! start of every round:
//!
//! - `max_tokens` — Total token spend across all provider calls
//! - `max_cost_usd` — Total cost in USD
//! - `max_rounds` — Maximum number of ReAct rounds
//! - `max_wall_clock_secs` — Maximum wall-clock time
//!
//! When any limit is exceeded, the session is terminated gracefully.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// A four-field budget for an agent session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    /// Maximum total tokens across all provider calls.
    pub max_tokens: u64,
    /// Maximum total cost in USD.
    pub max_cost_usd: f64,
    /// Maximum number of ReAct rounds.
    pub max_rounds: u32,
    /// Maximum wall-clock time in seconds.
    pub max_wall_clock_secs: u64,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            max_tokens: 1_000_000,
            max_cost_usd: 10.0,
            max_rounds: 50,
            max_wall_clock_secs: 600, // 10 minutes
        }
    }
}

impl Budget {
    /// Create a new budget with specified limits.
    pub fn new(max_tokens: u64, max_cost_usd: f64, max_rounds: u32, max_wall_clock_secs: u64) -> Self {
        Self {
            max_tokens,
            max_cost_usd,
            max_rounds,
            max_wall_clock_secs,
        }
    }

    /// Create an unlimited budget (use only in testing).
    pub fn unlimited() -> Self {
        Self {
            max_tokens: u64::MAX,
            max_cost_usd: f64::MAX,
            max_rounds: u32::MAX,
            max_wall_clock_secs: u64::MAX,
        }
    }
}

/// Current consumption against a budget.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConsumption {
    /// Tokens consumed so far.
    pub tokens_used: u64,
    /// Cost in USD so far.
    pub cost_usd: f64,
    /// Rounds completed so far.
    pub rounds_completed: u32,
    /// Seconds elapsed since session start.
    pub elapsed_secs: u64,
}

impl Default for BudgetConsumption {
    fn default() -> Self {
        Self {
            tokens_used: 0,
            cost_usd: 0.0,
            rounds_completed: 0,
            elapsed_secs: 0,
        }
    }
}

/// Which budget limit was exceeded.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BudgetLimit {
    /// Token limit exceeded.
    TokensExceeded,
    /// Cost limit exceeded.
    CostExceeded,
    /// Round limit exceeded.
    RoundsExceeded,
    /// Wall-clock time exceeded.
    WallClockExceeded,
}

impl std::fmt::Display for BudgetLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BudgetLimit::TokensExceeded => write!(f, "token budget exceeded"),
            BudgetLimit::CostExceeded => write!(f, "cost budget exceeded"),
            BudgetLimit::RoundsExceeded => write!(f, "round budget exceeded"),
            BudgetLimit::WallClockExceeded => write!(f, "wall-clock budget exceeded"),
        }
    }
}

/// The state of a session.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionState {
    /// Session is active and accepting rounds.
    Active,
    /// Session has completed normally.
    Completed,
    /// Session was terminated due to budget exhaustion.
    BudgetExhausted(BudgetLimit),
    /// Session was cancelled by the caller.
    Cancelled,
    /// Session failed due to an error.
    Failed,
}

impl SessionState {
    /// Whether the session is still active.
    pub fn is_active(&self) -> bool {
        matches!(self, SessionState::Active)
    }

    /// Whether the session is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        !self.is_active()
    }
}

/// A session tracking an agent's budget and state.
#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier.
    pub id: String,
    /// The agent this session belongs to.
    pub agent_id: String,
    /// The budget for this session.
    pub budget: Budget,
    /// Current consumption.
    pub consumption: BudgetConsumption,
    /// Current session state.
    pub state: SessionState,
    /// When the session was created.
    pub created_at: DateTime<Utc>,
    /// When the session ended, if it has.
    pub ended_at: Option<DateTime<Utc>>,
}

impl Session {
    /// Create a new session for an agent with a budget.
    pub fn new(agent_id: &str, budget: Budget) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            budget,
            consumption: BudgetConsumption::default(),
            state: SessionState::Active,
            created_at: Utc::now(),
            ended_at: None,
        }
    }

    /// Check the budget before starting a new round.
    ///
    /// Returns `Ok(())` if the round may proceed, or the
    /// `BudgetLimit` that was exceeded.
    pub fn check_budget(&self) -> Result<(), BudgetLimit> {
        if self.consumption.tokens_used >= self.budget.max_tokens {
            return Err(BudgetLimit::TokensExceeded);
        }
        if self.consumption.cost_usd >= self.budget.max_cost_usd {
            return Err(BudgetLimit::CostExceeded);
        }
        if self.consumption.rounds_completed >= self.budget.max_rounds {
            return Err(BudgetLimit::RoundsExceeded);
        }
        let elapsed = (Utc::now() - self.created_at)
            .num_seconds()
            .unwrap_or(0) as u64;
        if elapsed >= self.budget.max_wall_clock_secs {
            return Err(BudgetLimit::WallClockExceeded);
        }
        Ok(())
    }

    /// Record token consumption from a provider call.
    pub fn record_tokens(&mut self, tokens: u64, cost_usd: f64) {
        self.consumption.tokens_used += tokens;
        self.consumption.cost_usd += cost_usd;
    }

    /// Record a completed round.
    pub fn record_round(&mut self) {
        self.consumption.rounds_completed += 1;
    }

    /// Transition the session to a terminal state.
    pub fn end(&mut self, state: SessionState) {
        self.state = state;
        self.ended_at = Some(Utc::now());
    }

    /// Remaining tokens in the budget.
    pub fn remaining_tokens(&self) -> u64 {
        self.budget.max_tokens.saturating_sub(self.consumption.tokens_used)
    }

    /// Remaining cost in USD.
    pub fn remaining_cost(&self) -> f64 {
        (self.budget.max_cost_usd - self.consumption.cost_usd).max(0.0)
    }

    /// Remaining rounds.
    pub fn remaining_rounds(&self) -> u32 {
        self.budget.max_rounds.saturating_sub(self.consumption.rounds_completed)
    }
}

/// A thread-safe session manager.
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
}

use std::collections::HashMap;

impl SessionManager {
    /// Create a new session manager.
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new session for an agent.
    pub async fn create_session(&self, agent_id: &str, budget: Budget) -> String {
        let session = Session::new(agent_id, budget);
        let id = session.id.clone();
        self.sessions.write().await.insert(id.clone(), session);
        tracing::info!(session_id = %id, agent_id = agent_id, "Session created");
        id
    }

    /// Get a session by ID.
    pub async fn get_session(&self, session_id: &str) -> Option<Session> {
        self.sessions.read().await.get(session_id).cloned()
    }

    /// Record token consumption in a session.
    pub async fn record_tokens(&self, session_id: &str, tokens: u64, cost_usd: f64) -> anyhow::Result<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;
        session.record_tokens(tokens, cost_usd);
        Ok(())
    }

    /// Complete a round in a session, checking budget.
    pub async fn complete_round(&self, session_id: &str) -> Result<(), BudgetLimit> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(session_id)
            .ok_or_else(|| BudgetLimit::RoundsExceeded)?; // Session not found = can't proceed

        session.check_budget()?;
        session.record_round();
        Ok(())
    }

    /// Check if a session has remaining budget.
    pub async fn check_budget(&self, session_id: &str) -> Result<(), BudgetLimit> {
        let sessions = self.sessions.read().await;
        let session = sessions.get(session_id)
            .ok_or_else(|| BudgetLimit::RoundsExceeded)?;
        session.check_budget()
    }

    /// End a session.
    pub async fn end_session(&self, session_id: &str, state: SessionState) -> anyhow::Result<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;
        session.end(state);
        tracing::info!(session_id = session_id, state = ?session.state, "Session ended");
        Ok(())
    }

    /// List all active sessions.
    pub async fn active_sessions(&self) -> Vec<String> {
        let sessions = self.sessions.read().await;
        sessions.values()
            .filter(|s| s.state.is_active())
            .map(|s| s.id.clone())
            .collect()
    }

    /// Get a cancellation token for a session.
    /// When the token is cancelled, the session should wind down.
    pub fn cancellation_token(&self) -> CancellationToken {
        CancellationToken::new()
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
    fn budget_default_values() {
        let budget = Budget::default();
        assert_eq!(budget.max_tokens, 1_000_000);
        assert_eq!(budget.max_rounds, 50);
    }

    #[test]
    fn session_check_budget_allows_when_within_limits() {
        let session = Session::new("test-agent", Budget::default());
        assert!(session.check_budget().is_ok());
    }

    #[test]
    fn session_detects_token_exhaustion() {
        let mut session = Session::new("test-agent", Budget::new(100, 10.0, 50, 600));
        session.consumption.tokens_used = 100;
        let result = session.check_budget();
        assert_eq!(result, Err(BudgetLimit::TokensExceeded));
    }

    #[test]
    fn session_detects_cost_exhaustion() {
        let mut session = Session::new("test-agent", Budget::new(1000, 1.0, 50, 600));
        session.consumption.cost_usd = 1.0;
        let result = session.check_budget();
        assert_eq!(result, Err(BudgetLimit::CostExceeded));
    }

    #[test]
    fn session_detects_round_exhaustion() {
        let mut session = Session::new("test-agent", Budget::new(1000, 10.0, 2, 600));
        session.consumption.rounds_completed = 2;
        let result = session.check_budget();
        assert_eq!(result, Err(BudgetLimit::RoundsExceeded));
    }

    #[test]
    fn session_detects_wall_clock_exhaustion() {
        let mut session = Session::new("test-agent", Budget::new(1000, 10.0, 50, 0)); // 0 seconds
        let result = session.check_budget();
        assert_eq!(result, Err(BudgetLimit::WallClockExceeded));
    }

    #[test]
    fn session_remaining_calculations() {
        let mut session = Session::new("test-agent", Budget::new(1000, 10.0, 10, 600));
        session.consumption.tokens_used = 300;
        session.consumption.cost_usd = 3.0;
        session.consumption.rounds_completed = 4;

        assert_eq!(session.remaining_tokens(), 700);
        assert!((session.remaining_cost() - 7.0).abs() < f64::EPSILON);
        assert_eq!(session.remaining_rounds(), 6);
    }

    #[tokio::test]
    async fn session_manager_create_and_get() {
        let manager = SessionManager::new();
        let id = manager.create_session("agent-1", Budget::default()).await;
        let session = manager.get_session(&id).await.unwrap();
        assert_eq!(session.agent_id, "agent-1");
        assert!(session.state.is_active());
    }

    #[tokio::test]
    async fn session_manager_record_tokens() {
        let manager = SessionManager::new();
        let id = manager.create_session("agent-1", Budget::new(1000, 10.0, 10, 600)).await;
        manager.record_tokens(&id, 500, 2.0).await.unwrap();

        let session = manager.get_session(&id).await.unwrap();
        assert_eq!(session.consumption.tokens_used, 500);
        assert!((session.consumption.cost_usd - 2.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn session_manager_budget_enforcement() {
        let manager = SessionManager::new();
        let id = manager.create_session("agent-1", Budget::new(100, 10.0, 2, 600)).await;

        // First round should work
        manager.complete_round(&id).await.unwrap();

        // Second round should work
        manager.complete_round(&id).await.unwrap();

        // Third round should fail
        let result = manager.complete_round(&id).await;
        assert_eq!(result, Err(BudgetLimit::RoundsExceeded));
    }

    #[tokio::test]
    async fn session_end_transition() {
        let manager = SessionManager::new();
        let id = manager.create_session("agent-1", Budget::default()).await;
        manager.end_session(&id, SessionState::Completed).await.unwrap();

        let session = manager.get_session(&id).await.unwrap();
        assert_eq!(session.state, SessionState::Completed);
        assert!(session.ended_at.is_some());
    }
}

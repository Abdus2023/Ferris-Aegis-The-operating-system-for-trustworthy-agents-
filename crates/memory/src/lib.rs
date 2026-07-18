//! Ferris Aegis Episodic Memory — SQLite-backed conversation history.
//!
//! Episodic memory stores agent conversation turns as discrete episodes.
//! Each episode records the role, content, and metadata for a single
//! turn in an agent's conversation.
//!
//! # Design Principle: Episodic Before Semantic
//!
//! This crate implements **episodic memory** only — the raw record of
//! what happened and when. Semantic memory (embeddings, knowledge graphs,
//! vector search) is deferred to a later phase. Episodic memory is the
//! foundation that semantic memory will index and summarize.
//!
//! # Storage
//!
//! SQLite via `sqlx 0.9` with `runtime-tokio`. A single table holds all
//! episodes, indexed by agent ID and timestamp. The database file is
//! created automatically on first connection.

use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single episode in an agent's memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    /// Unique episode identifier.
    pub id: String,
    /// The agent this episode belongs to.
    pub agent_id: String,
    /// The role of the speaker (user, assistant, system, tool).
    pub role: String,
    /// The content of the episode.
    pub content: String,
    /// When this episode was recorded.
    pub timestamp: DateTime<Utc>,
    /// Optional metadata (token count, model, etc.).
    pub metadata: serde_json::Value,
}

/// The episodic memory store backed by SQLite.
pub struct EpisodicMemory {
    /// The SQLite connection pool.
    pool: sqlx::SqlitePool,
}

impl EpisodicMemory {
    /// Open an episodic memory store at the given database path.
    ///
    /// Creates the database and schema if it doesn't exist.
    pub async fn open(database_url: &str) -> anyhow::Result<Self> {
        let pool = sqlx::SqlitePool::connect(database_url)
            .await
            .context("failed to connect to SQLite database")?;

        Self::initialize_schema(&pool).await?;

        Ok(Self { pool })
    }

    /// Open an in-memory database for testing.
    pub async fn open_in_memory() -> anyhow::Result<Self> {
        Self::open("sqlite::memory:").await
    }

    /// Initialize the database schema.
    async fn initialize_schema(pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS episodes (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                metadata TEXT NOT NULL DEFAULT '{}'
            );

            CREATE INDEX IF NOT EXISTS idx_episodes_agent_id ON episodes(agent_id);
            CREATE INDEX IF NOT EXISTS idx_episodes_timestamp ON episodes(timestamp);
            CREATE INDEX IF NOT EXISTS idx_episodes_agent_timestamp ON episodes(agent_id, timestamp);
            "#,
        )
        .execute(pool)
        .await
        .context("failed to initialize database schema")?;

        Ok(())
    }

    /// Record a new episode.
    pub async fn record(
        &self,
        agent_id: &str,
        role: &str,
        content: &str,
        metadata: Option<serde_json::Value>,
    ) -> anyhow::Result<Episode> {
        let id = Uuid::new_v4().to_string();
        let timestamp = Utc::now();
        let metadata = metadata.unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

        sqlx::query(
            r#"
            INSERT INTO episodes (id, agent_id, role, content, timestamp, metadata)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(agent_id)
        .bind(role)
        .bind(content)
        .bind(timestamp.to_rfc3339())
        .bind(metadata.to_string())
        .execute(&self.pool)
        .await
        .context("failed to insert episode")?;

        let episode = Episode {
            id,
            agent_id: agent_id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            timestamp,
            metadata,
        };

        tracing::debug!(
            episode_id = %episode.id,
            agent_id = agent_id,
            role = role,
            "Episode recorded"
        );

        Ok(episode)
    }

    /// Retrieve recent episodes for an agent.
    pub async fn recent(
        &self,
        agent_id: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<Episode>> {
        let rows = sqlx::query_as::<_, EpisodeRow>(
            r#"
            SELECT id, agent_id, role, content, timestamp, metadata
            FROM episodes
            WHERE agent_id = ?
            ORDER BY timestamp DESC
            LIMIT ?
            "#,
        )
        .bind(agent_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("failed to query episodes")?;

        rows.into_iter().map(|r| r.into_episode()).collect()
    }

    /// Retrieve episodes for an agent within a time range.
    pub async fn range(
        &self,
        agent_id: &str,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
    ) -> anyhow::Result<Vec<Episode>> {
        let rows = sqlx::query_as::<_, EpisodeRow>(
            r#"
            SELECT id, agent_id, role, content, timestamp, metadata
            FROM episodes
            WHERE agent_id = ? AND timestamp >= ? AND timestamp <= ?
            ORDER BY timestamp ASC
            "#,
        )
        .bind(agent_id)
        .bind(start.to_rfc3339())
        .bind(end.to_rfc3339())
        .fetch_all(&self.pool)
        .await
        .context("failed to query episodes by time range")?;

        rows.into_iter().map(|r| r.into_episode()).collect()
    }

    /// Search episodes by content (simple LIKE search).
    ///
    /// For production, this would use FTS5. For Phase 3, LIKE is
    /// sufficient to demonstrate the query interface.
    pub async fn search(
        &self,
        agent_id: &str,
        query: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<Episode>> {
        let pattern = format!("%{query}%");
        let rows = sqlx::query_as::<_, EpisodeRow>(
            r#"
            SELECT id, agent_id, role, content, timestamp, metadata
            FROM episodes
            WHERE agent_id = ? AND content LIKE ?
            ORDER BY timestamp DESC
            LIMIT ?
            "#,
        )
        .bind(agent_id)
        .bind(&pattern)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("failed to search episodes")?;

        rows.into_iter().map(|r| r.into_episode()).collect()
    }

    /// Count episodes for an agent.
    pub async fn count(&self, agent_id: &str) -> anyhow::Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM episodes WHERE agent_id = ?",
        )
        .bind(agent_id)
        .fetch_one(&self.pool)
        .await
        .context("failed to count episodes")?;

        Ok(row.0)
    }

    /// Delete all episodes for an agent.
    pub async fn clear(&self, agent_id: &str) -> anyhow::Result<u64> {
        let result = sqlx::query("DELETE FROM episodes WHERE agent_id = ?")
            .bind(agent_id)
            .execute(&self.pool)
            .await
            .context("failed to clear episodes")?;

        Ok(result.rows_affected())
    }

    /// Close the database connection pool.
    pub async fn close(self) {
        self.pool.close().await;
    }
}

/// Internal row representation for SQL query results.
#[derive(Debug, sqlx::FromRow)]
struct EpisodeRow {
    id: String,
    agent_id: String,
    role: String,
    content: String,
    timestamp: String,
    metadata: String,
}

impl EpisodeRow {
    fn into_episode(self) -> anyhow::Result<Episode> {
        let timestamp = DateTime::parse_from_rfc3339(&self.timestamp)
            .context("failed to parse timestamp")?
            .to_utc();

        let metadata: serde_json::Value =
            serde_json::from_str(&self.metadata).unwrap_or(serde_json::Value::Null);

        Ok(Episode {
            id: self.id,
            agent_id: self.agent_id,
            role: self.role,
            content: self.content,
            timestamp,
            metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_memory() -> EpisodicMemory {
        EpisodicMemory::open_in_memory().await.unwrap()
    }

    #[tokio::test]
    async fn record_and_retrieve_episode() {
        let memory = test_memory().await;

        let episode = memory
            .record("agent-1", "user", "Hello, agent!", None)
            .await
            .unwrap();

        assert_eq!(episode.agent_id, "agent-1");
        assert_eq!(episode.role, "user");
        assert_eq!(episode.content, "Hello, agent!");

        let recent = memory.recent("agent-1", 10).await.unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].content, "Hello, agent!");
    }

    #[tokio::test]
    async fn multiple_episodes_ordered_by_time() {
        let memory = test_memory().await;

        memory
            .record("agent-1", "user", "First", None)
            .await
            .unwrap();
        memory
            .record("agent-1", "assistant", "Second", None)
            .await
            .unwrap();
        memory
            .record("agent-1", "user", "Third", None)
            .await
            .unwrap();

        let recent = memory.recent("agent-1", 10).await.unwrap();
        assert_eq!(recent.len(), 3);
        // Most recent first
        assert_eq!(recent[0].content, "Third");
        assert_eq!(recent[1].content, "Second");
        assert_eq!(recent[2].content, "First");
    }

    #[tokio::test]
    async fn episodes_isolated_by_agent() {
        let memory = test_memory().await;

        memory
            .record("agent-1", "user", "Agent 1 message", None)
            .await
            .unwrap();
        memory
            .record("agent-2", "user", "Agent 2 message", None)
            .await
            .unwrap();

        let recent_1 = memory.recent("agent-1", 10).await.unwrap();
        let recent_2 = memory.recent("agent-2", 10).await.unwrap();

        assert_eq!(recent_1.len(), 1);
        assert_eq!(recent_2.len(), 1);
        assert_eq!(recent_1[0].content, "Agent 1 message");
        assert_eq!(recent_2[0].content, "Agent 2 message");
    }

    #[tokio::test]
    async fn search_episodes() {
        let memory = test_memory().await;

        memory
            .record("agent-1", "user", "What is the weather?", None)
            .await
            .unwrap();
        memory
            .record("agent-1", "assistant", "The weather is sunny", None)
            .await
            .unwrap();
        memory
            .record("agent-1", "user", "Tell me a joke", None)
            .await
            .unwrap();

        let results = memory.search("agent-1", "weather", 10).await.unwrap();
        assert_eq!(results.len(), 2); // question and answer both contain "weather"
    }

    #[tokio::test]
    async fn count_episodes() {
        let memory = test_memory().await;

        for i in 0..5 {
            memory
                .record("agent-1", "user", &format!("Message {}", i), None)
                .await
                .unwrap();
        }

        let count = memory.count("agent-1").await.unwrap();
        assert_eq!(count, 5);
    }

    #[tokio::test]
    async fn clear_episodes() {
        let memory = test_memory().await;

        memory
            .record("agent-1", "user", "Temporary", None)
            .await
            .unwrap();

        let deleted = memory.clear("agent-1").await.unwrap();
        assert_eq!(deleted, 1);

        let count = memory.count("agent-1").await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn episode_with_metadata() {
        let memory = test_memory().await;

        let metadata = serde_json::json!({
            "model": "gpt-4",
            "tokens": 42
        });

        let episode = memory
            .record("agent-1", "assistant", "Hello!", Some(metadata))
            .await
            .unwrap();

        assert_eq!(episode.metadata["model"], "gpt-4");
        assert_eq!(episode.metadata["tokens"], 42);
    }
}

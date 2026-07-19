//! Ferris Aegis Semantic Memory — Embedding storage and concept extraction.
//!
//! Semantic memory builds on episodic memory to provide higher-level
//! memory capabilities: concept extraction, embedding storage, and
//! semantic search across conversation history.
//!
//! # Design
//!
//! Semantic memory is the "understanding" layer. While episodic memory
//! records "what happened," semantic memory extracts "what it means."
//!
//! - **Concepts** — Named entities, ideas, and topics extracted from conversations
//! - **Embeddings** — Float vector representations stored alongside episodes
//! - **Labels** — User-defined tags for organizing memories
//! - **Summaries** — Compressed representations of multi-turn conversations
//!
//! # Storage
//!
//! SQLite via `sqlx 0.9` with additional tables for concepts, embeddings,
//! and labels. The concepts table uses FTS5 for full-text search.

use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A concept extracted from a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    /// Unique concept identifier.
    pub id: String,
    /// The agent this concept relates to.
    pub agent_id: String,
    /// The episode this concept was extracted from.
    pub episode_id: Option<String>,
    /// The concept name (e.g. "machine_learning", "rust_programming").
    pub name: String,
    /// A short description or definition.
    pub description: String,
    /// Associated labels.
    pub labels: Vec<String>,
    /// Confidence score of the extraction (0.0–1.0).
    pub confidence: f64,
    /// When the concept was extracted.
    pub created_at: DateTime<Utc>,
}

/// An embedding vector stored alongside an episode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEmbedding {
    /// Unique embedding identifier.
    pub id: String,
    /// The episode this embedding relates to.
    pub episode_id: String,
    /// The agent this embedding belongs to.
    pub agent_id: String,
    /// The embedding vector (as JSON array of floats).
    pub vector: Vec<f32>,
    /// The model used to generate the embedding.
    pub model: String,
    /// The dimension of the embedding vector.
    pub dimensions: u32,
    /// When the embedding was created.
    pub created_at: DateTime<Utc>,
}

/// A semantic memory summary of a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    /// Unique summary identifier.
    pub id: String,
    /// The agent this summary relates to.
    pub agent_id: String,
    /// The session this summary covers.
    pub session_id: Option<String>,
    /// The summary text.
    pub content: String,
    /// The episode IDs this summary covers.
    pub covered_episodes: Vec<String>,
    /// When the summary was created.
    pub created_at: DateTime<Utc>,
}

/// The semantic memory store backed by SQLite.
pub struct SemanticMemory {
    /// The SQLite connection pool.
    pool: sqlx::SqlitePool,
}

impl SemanticMemory {
    /// Open a semantic memory store at the given database path.
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
        // Concepts table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS concepts (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                episode_id TEXT,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                labels TEXT NOT NULL DEFAULT '[]',
                confidence REAL NOT NULL DEFAULT 1.0,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_concepts_agent ON concepts(agent_id);
            "#,
        )
        .execute(pool)
        .await
        .context("failed to create concepts table")?;

        // Embeddings table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS embeddings (
                id TEXT PRIMARY KEY,
                episode_id TEXT NOT NULL,
                agent_id TEXT NOT NULL,
                vector TEXT NOT NULL,
                model TEXT NOT NULL DEFAULT 'unknown',
                dimensions INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_embeddings_agent ON embeddings(agent_id);
            CREATE INDEX IF NOT EXISTS idx_embeddings_episode ON embeddings(episode_id);
            "#,
        )
        .execute(pool)
        .await
        .context("failed to create embeddings table")?;

        // Summaries table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS summaries (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                session_id TEXT,
                content TEXT NOT NULL,
                covered_episodes TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_summaries_agent ON summaries(agent_id);
            "#,
        )
        .execute(pool)
        .await
        .context("failed to create summaries table")?;

        Ok(())
    }

    /// Store a concept.
    pub async fn store_concept(&self, concept: &Concept) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO concepts (id, agent_id, episode_id, name, description, labels, confidence, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&concept.id)
        .bind(&concept.agent_id)
        .bind(&concept.episode_id)
        .bind(&concept.name)
        .bind(&concept.description)
        .bind(serde_json::to_string(&concept.labels).unwrap_or_default())
        .bind(concept.confidence)
        .bind(concept.created_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .context("failed to store concept")?;

        Ok(())
    }

    /// Search concepts by name or description.
    pub async fn search_concepts(
        &self,
        agent_id: &str,
        query: &str,
    ) -> anyhow::Result<Vec<Concept>> {
        let pattern = format!("%{query}%");
        let rows = sqlx::query_as::<_, ConceptRow>(
            r#"
            SELECT id, agent_id, episode_id, name, description, labels, confidence, created_at
            FROM concepts
            WHERE agent_id = ? AND (name LIKE ? OR description LIKE ?)
            ORDER BY confidence DESC
            LIMIT 50
            "#,
        )
        .bind(agent_id)
        .bind(&pattern)
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await
        .context("failed to search concepts")?;

        rows.into_iter().map(|r| r.into_concept()).collect()
    }

    /// List all concepts for an agent.
    pub async fn list_concepts(&self, agent_id: &str) -> anyhow::Result<Vec<Concept>> {
        let rows = sqlx::query_as::<_, ConceptRow>(
            r#"
            SELECT id, agent_id, episode_id, name, description, labels, confidence, created_at
            FROM concepts
            WHERE agent_id = ?
            ORDER BY confidence DESC
            "#,
        )
        .bind(agent_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to list concepts")?;

        rows.into_iter().map(|r| r.into_concept()).collect()
    }

    /// Store an embedding.
    pub async fn store_embedding(&self, embedding: &StoredEmbedding) -> anyhow::Result<()> {
        let vector_json = serde_json::to_string(&embedding.vector)?;

        sqlx::query(
            r#"
            INSERT INTO embeddings (id, episode_id, agent_id, vector, model, dimensions, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&embedding.id)
        .bind(&embedding.episode_id)
        .bind(&embedding.agent_id)
        .bind(&vector_json)
        .bind(&embedding.model)
        .bind(embedding.dimensions as i64)
        .bind(embedding.created_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .context("failed to store embedding")?;

        Ok(())
    }

    /// Retrieve an embedding by episode ID.
    pub async fn get_embedding(
        &self,
        episode_id: &str,
    ) -> anyhow::Result<Option<StoredEmbedding>> {
        let row = sqlx::query_as::<_, EmbeddingRow>(
            r#"
            SELECT id, episode_id, agent_id, vector, model, dimensions, created_at
            FROM embeddings
            WHERE episode_id = ?
            LIMIT 1
            "#,
        )
        .bind(episode_id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to get embedding")?;

        row.map(|r| r.into_embedding()).transpose()
    }

    /// Compute cosine similarity between two vectors.
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| f64::from(*x) * f64::from(*y)).sum();
        let norm_a: f64 = a.iter().map(|x| f64::from(*x) * f64::from(*x)).sum::<f64>().sqrt();
        let norm_b: f64 = b.iter().map(|x| f64::from(*x) * f64::from(*x)).sum::<f64>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a * norm_b)
    }

    /// Store a summary.
    pub async fn store_summary(&self, summary: &Summary) -> anyhow::Result<()> {
        let covered_json = serde_json::to_string(&summary.covered_episodes)?;

        sqlx::query(
            r#"
            INSERT INTO summaries (id, agent_id, session_id, content, covered_episodes, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&summary.id)
        .bind(&summary.agent_id)
        .bind(&summary.session_id)
        .bind(&summary.content)
        .bind(&covered_json)
        .bind(summary.created_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .context("failed to store summary")?;

        Ok(())
    }

    /// List summaries for an agent.
    pub async fn list_summaries(&self, agent_id: &str) -> anyhow::Result<Vec<Summary>> {
        let rows = sqlx::query_as::<_, SummaryRow>(
            r#"
            SELECT id, agent_id, session_id, content, covered_episodes, created_at
            FROM summaries
            WHERE agent_id = ?
            ORDER BY created_at DESC
            LIMIT 50
            "#,
        )
        .bind(agent_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to list summaries")?;

        rows.into_iter().map(|r| r.into_summary()).collect()
    }

    /// Extract concepts from a conversation text (simple keyword-based).
    ///
    /// In production, this would use an LLM or NLP pipeline. For Phase 4,
    /// it performs simple keyword matching against a configurable dictionary.
    pub fn extract_concepts(
        &self,
        agent_id: &str,
        episode_id: Option<&str>,
        text: &str,
    ) -> Vec<Concept> {
        let lower = text.to_lowercase();
        let mut concepts = Vec::new();

        let patterns: &[(&str, &str)] = &[
            ("rust", "The Rust programming language"),
            ("machine learning", "Machine learning and AI concepts"),
            ("api", "Application Programming Interface usage"),
            ("database", "Database operations and design"),
            ("security", "Security-related topics"),
            ("testing", "Software testing and quality assurance"),
            ("deployment", "Software deployment and CI/CD"),
            ("error", "Error handling and debugging"),
        ];

        let now = Utc::now();
        for (keyword, description) in patterns {
            if lower.contains(keyword) {
                concepts.push(Concept {
                    id: Uuid::new_v4().to_string(),
                    agent_id: agent_id.to_string(),
                    episode_id: episode_id.map(|s| s.to_string()),
                    name: keyword.replace(' ', "_").to_string(),
                    description: description.to_string(),
                    labels: vec![keyword.to_string()],
                    confidence: 0.8,
                    created_at: now,
                });
            }
        }

        concepts
    }

    /// Close the database connection pool.
    pub async fn close(self) {
        self.pool.close().await;
    }
}

// ── Internal row types for SQLx ────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct ConceptRow {
    id: String,
    agent_id: String,
    episode_id: Option<String>,
    name: String,
    description: String,
    labels: String,
    confidence: f64,
    created_at: String,
}

impl ConceptRow {
    fn into_concept(self) -> anyhow::Result<Concept> {
        let created_at = DateTime::parse_from_rfc3339(&self.created_at)
            .context("failed to parse created_at")?
            .to_utc();

        let labels: Vec<String> =
            serde_json::from_str(&self.labels).unwrap_or_default();

        Ok(Concept {
            id: self.id,
            agent_id: self.agent_id,
            episode_id: self.episode_id,
            name: self.name,
            description: self.description,
            labels,
            confidence: self.confidence,
            created_at,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct EmbeddingRow {
    id: String,
    episode_id: String,
    agent_id: String,
    vector: String,
    model: String,
    dimensions: i64,
    created_at: String,
}

impl EmbeddingRow {
    fn into_embedding(self) -> anyhow::Result<StoredEmbedding> {
        let created_at = DateTime::parse_from_rfc3339(&self.created_at)
            .context("failed to parse created_at")?
            .to_utc();

        let vector: Vec<f32> = serde_json::from_str(&self.vector)?;

        Ok(StoredEmbedding {
            id: self.id,
            episode_id: self.episode_id,
            agent_id: self.agent_id,
            vector,
            model: self.model,
            dimensions: self.dimensions as u32,
            created_at,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct SummaryRow {
    id: String,
    agent_id: String,
    session_id: Option<String>,
    content: String,
    covered_episodes: String,
    created_at: String,
}

impl SummaryRow {
    fn into_summary(self) -> anyhow::Result<Summary> {
        let created_at = DateTime::parse_from_rfc3339(&self.created_at)
            .context("failed to parse created_at")?
            .to_utc();

        let covered_episodes: Vec<String> =
            serde_json::from_str(&self.covered_episodes).unwrap_or_default();

        Ok(Summary {
            id: self.id,
            agent_id: self.agent_id,
            session_id: self.session_id,
            content: self.content,
            covered_episodes,
            created_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_memory() -> SemanticMemory {
        SemanticMemory::open_in_memory().await.unwrap()
    }

    #[tokio::test]
    async fn store_and_list_concepts() {
        let memory = test_memory().await;

        let concept = Concept {
            id: Uuid::new_v4().to_string(),
            agent_id: "agent-1".to_string(),
            episode_id: None,
            name: "rust_programming".to_string(),
            description: "Rust programming language concepts".to_string(),
            labels: vec!["rust".to_string(), "programming".to_string()],
            confidence: 0.95,
            created_at: Utc::now(),
        };

        memory.store_concept(&concept).await.unwrap();

        let concepts = memory.list_concepts("agent-1").await.unwrap();
        assert_eq!(concepts.len(), 1);
        assert_eq!(concepts[0].name, "rust_programming");
        assert_eq!(concepts[0].confidence, 0.95);
    }

    #[tokio::test]
    async fn search_concepts_finds_match() {
        let memory = test_memory().await;

        for (i, (name, desc)) in [
            ("rust", "Rust programming"),
            ("python", "Python scripting"),
            ("database", "SQL databases"),
        ]
        .iter()
        .enumerate()
        {
            memory
                .store_concept(&Concept {
                    id: Uuid::new_v4().to_string(),
                    agent_id: "agent-1".to_string(),
                    episode_id: None,
                    name: name.to_string(),
                    description: desc.to_string(),
                    labels: vec![],
                    confidence: 0.9 - (i as f64 * 0.1),
                    created_at: Utc::now(),
                })
                .await
                .unwrap();
        }

        let results = memory.search_concepts("agent-1", "programming").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "rust");
    }

    #[tokio::test]
    async fn store_and_retrieve_embedding() {
        let memory = test_memory().await;

        let embedding = StoredEmbedding {
            id: Uuid::new_v4().to_string(),
            episode_id: "ep-1".to_string(),
            agent_id: "agent-1".to_string(),
            vector: vec![0.1, 0.2, 0.3, 0.4],
            model: "text-embedding-3".to_string(),
            dimensions: 4,
            created_at: Utc::now(),
        };

        memory.store_embedding(&embedding).await.unwrap();

        let retrieved = memory.get_embedding("ep-1").await.unwrap().unwrap();
        assert_eq!(retrieved.vector.len(), 4);
        assert_eq!(retrieved.model, "text-embedding-3");
    }

    #[test]
    fn cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let sim = SemanticMemory::cosine_similarity(&a, &a);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = SemanticMemory::cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < 0.001);
    }

    #[test]
    fn cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = SemanticMemory::cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn store_and_list_summaries() {
        let memory = test_memory().await;

        let summary = Summary {
            id: Uuid::new_v4().to_string(),
            agent_id: "agent-1".to_string(),
            session_id: Some("session-1".to_string()),
            content: "The agent discussed Rust programming patterns.".to_string(),
            covered_episodes: vec!["ep-1".to_string(), "ep-2".to_string()],
            created_at: Utc::now(),
        };

        memory.store_summary(&summary).await.unwrap();

        let summaries = memory.list_summaries("agent-1").await.unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].content, summary.content);
        assert_eq!(summaries[0].covered_episodes.len(), 2);
    }

    #[test]
    fn extract_concepts_from_text() {
        let memory = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(test_memory());

        let concepts = memory.extract_concepts("agent-1", Some("ep-1"), "I love Rust and testing!");
        assert!(concepts.iter().any(|c| c.name == "rust"));
        assert!(concepts.iter().any(|c| c.name == "testing"));
        assert!(!concepts.iter().any(|c| c.name == "deployment"));
    }
}

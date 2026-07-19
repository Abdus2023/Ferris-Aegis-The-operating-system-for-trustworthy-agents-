//! Ferris Aegis Semantic Memory — Vector similarity search for agent knowledge.
//!
//! Semantic memory stores agent knowledge as vector embeddings, enabling
//! similarity-based retrieval. This is the layer that sits on top of
//! episodic memory (Phase 3) — episodic records *what happened*, semantic
//! memory records *what was learned*.
//!
//! # Timing Gate
//!
//! This crate has an independent timing gate: **two weeks of Phase 3
//! episodic memory in production** before enabling semantic memory. This
//! ensures the vector index gets sized against real query patterns instead
//! of guesses. BM25/RRF fusion is a Phase 5 item.
//!
//! # Architecture
//!
//! In production, this uses PostgreSQL with `pgvector`. For Phase 4, we
//! provide an in-memory implementation that demonstrates the query
//! interface and can be swapped for the real pgvector backend later.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The dimensionality of embedding vectors.
/// OpenAI text-embedding-3-small uses 1536 dimensions.
pub const DEFAULT_EMBEDDING_DIM: usize = 1536;

/// A knowledge entry in semantic memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    /// Unique entry identifier.
    pub id: String,
    /// The agent that produced this knowledge.
    pub agent_id: String,
    /// The source episode IDs that contributed to this knowledge.
    pub source_episodes: Vec<String>,
    /// The text content of the knowledge.
    pub content: String,
    /// The embedding vector (produced by an embedding model).
    pub embedding: Vec<f32>,
    /// When this entry was created.
    pub created_at: DateTime<Utc>,
    /// Metadata (model used, confidence score, etc.).
    pub metadata: serde_json::Value,
}

/// A similarity search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// The matching knowledge entry.
    pub entry: KnowledgeEntry,
    /// Cosine similarity score (0.0 to 1.0).
    pub similarity: f64,
}

/// The semantic memory store.
///
/// Phase 4 provides an in-memory implementation. The pgvector backend
/// will be added behind the same trait when the production gate is met.
pub struct SemanticMemory {
    /// In-memory store of knowledge entries.
    entries: Vec<KnowledgeEntry>,
    /// Whether the store is enabled (respects the timing gate).
    enabled: bool,
}

impl SemanticMemory {
    /// Create a new semantic memory store.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
        }
    }

    /// Create a disabled store (all operations are no-ops).
    /// Use this when the timing gate hasn't been met yet.
    pub fn disabled() -> Self {
        Self {
            entries: Vec::new(),
            enabled: false,
        }
    }

    /// Whether semantic memory is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Store a new knowledge entry with its embedding.
    pub fn store(
        &mut self,
        agent_id: &str,
        content: &str,
        embedding: Vec<f32>,
        source_episodes: Vec<String>,
        metadata: Option<serde_json::Value>,
    ) -> anyhow::Result<String> {
        if !self.enabled {
            return Ok(String::new());
        }

        let id = Uuid::new_v4().to_string();
        let entry = KnowledgeEntry {
            id: id.clone(),
            agent_id: agent_id.to_string(),
            source_episodes,
            content: content.to_string(),
            embedding,
            created_at: Utc::now(),
            metadata: metadata.unwrap_or(serde_json::Value::Null),
        };

        self.entries.push(entry);
        tracing::debug!(id = %id, agent_id = agent_id, "Knowledge stored");
        Ok(id)
    }

    /// Search for similar knowledge using cosine similarity.
    pub fn search(
        &self,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f64,
    ) -> Vec<SearchResult> {
        if !self.enabled {
            return Vec::new();
        }

        let mut results: Vec<SearchResult> = self.entries
            .iter()
            .map(|entry| {
                let similarity = cosine_similarity(query_embedding, &entry.embedding);
                SearchResult {
                    entry: entry.clone(),
                    similarity,
                }
            })
            .filter(|r| r.similarity >= min_similarity)
            .collect();

        // Sort by similarity descending
        results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        results
    }

    /// Search for similar knowledge for a specific agent.
    pub fn search_for_agent(
        &self,
        agent_id: &str,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f64,
    ) -> Vec<SearchResult> {
        self.search(query_embedding, limit * 2, min_similarity)
            .into_iter()
            .filter(|r| r.entry.agent_id == agent_id)
            .take(limit)
            .collect()
    }

    /// Get a knowledge entry by ID.
    pub fn get(&self, id: &str) -> Option<&KnowledgeEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Delete all entries for an agent.
    pub fn clear_for_agent(&mut self, agent_id: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|e| e.agent_id != agent_id);
        before - self.entries.len()
    }
}

impl Default for SemanticMemory {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute cosine similarity between two vectors.
///
/// Returns a value between -1.0 and 1.0, where 1.0 means identical
/// direction and 0.0 means orthogonal.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| (*x as f64) * (*y as f64)).sum();
    let norm_a: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// Generate a simple embedding for testing purposes.
///
/// This creates a deterministic but non-meaningful embedding vector
/// from text content. In production, use a real embedding model
/// (e.g., OpenAI text-embedding-3-small).
pub fn mock_embedding(text: &str, dim: usize) -> Vec<f32> {
    let mut embedding = Vec::with_capacity(dim);
    let bytes = text.as_bytes();
    for i in 0..dim {
        let byte_val = bytes[i % bytes.len()] as f32;
        // Normalize to small range around 0
        embedding.push((byte_val - 128.0) / 128.0);
    }
    // Normalize the vector
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for val in embedding.iter_mut() {
            *val /= norm;
        }
    }
    embedding
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_similarity_identical_vectors() {
        let v = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_opposite_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_empty_vectors() {
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
    }

    #[test]
    fn store_and_retrieve() {
        let mut memory = SemanticMemory::new();
        let embedding = mock_embedding("test content", 8);

        let id = memory.store("agent-1", "test content", embedding, vec![], None).unwrap();
        let entry = memory.get(&id).unwrap();
        assert_eq!(entry.content, "test content");
    }

    #[test]
    fn search_returns_similar_entries() {
        let mut memory = SemanticMemory::new();

        let embedding_a = mock_embedding("Rust programming language", 8);
        let embedding_b = mock_embedding("Rust coding practices", 8);
        let embedding_c = mock_embedding("Cooking Italian food", 8);

        memory.store("agent-1", "Rust programming", embedding_a.clone(), vec![], None).unwrap();
        memory.store("agent-1", "Rust coding", embedding_b, vec![], None).unwrap();
        memory.store("agent-1", "Italian cooking", embedding_c, vec![], None).unwrap();

        let results = memory.search(&embedding_a, 3, 0.0);
        assert_eq!(results.len(), 3);
        // The exact same embedding should be the top result
        assert!((results[0].similarity - 1.0).abs() < 1e-6);
    }

    #[test]
    fn search_for_agent_filters_correctly() {
        let mut memory = SemanticMemory::new();

        let emb = mock_embedding("shared knowledge", 8);
        memory.store("agent-1", "Knowledge A", emb.clone(), vec![], None).unwrap();
        memory.store("agent-2", "Knowledge B", emb, vec![], None).unwrap();

        let query = mock_embedding("shared knowledge", 8);
        let results = memory.search_for_agent("agent-1", &query, 10, 0.0);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.agent_id, "agent-1");
    }

    #[test]
    fn min_similarity_filter() {
        let mut memory = SemanticMemory::new();

        let emb_a = mock_embedding("hello world", 8);
        let emb_b = mock_embedding("completely different topic", 8);
        memory.store("agent-1", "Hello", emb_a.clone(), vec![], None).unwrap();
        memory.store("agent-1", "Different", emb_b, vec![], None).unwrap();

        // Only exact match should have similarity 1.0
        let results = memory.search(&emb_a, 10, 0.99);
        assert!(results.len() <= 1);
    }

    #[test]
    fn disabled_store_is_noop() {
        let mut memory = SemanticMemory::disabled();
        let emb = mock_embedding("test", 8);
        let id = memory.store("agent-1", "test", emb, vec![], None).unwrap();
        assert!(id.is_empty());
        assert_eq!(memory.len(), 0);
    }

    #[test]
    fn clear_for_agent() {
        let mut memory = SemanticMemory::new();
        let emb = mock_embedding("test", 8);
        memory.store("agent-1", "A", emb.clone(), vec![], None).unwrap();
        memory.store("agent-2", "B", emb.clone(), vec![], None).unwrap();
        memory.store("agent-1", "C", emb, vec![], None).unwrap();

        let deleted = memory.clear_for_agent("agent-1");
        assert_eq!(deleted, 2);
        assert_eq!(memory.len(), 1);
    }
}

pub mod memory;
pub mod queries;
pub mod storage;

// Re-export query engine components
pub use queries::{QueryEngine, CentralityMetrics};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Core node in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KGNode {
    pub uuid: Uuid,
    pub name: String,
    pub node_type: String,
    pub summary: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub group_id: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl KGNode {
    pub fn new(name: String, node_type: String, summary: String, group_id: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            uuid: Uuid::new_v4(),
            name,
            node_type,
            summary,
            created_at: now,
            updated_at: now,
            group_id,
            metadata: HashMap::new(),
        }
    }

    pub fn update_summary(&mut self, summary: String) {
        self.summary = summary;
        self.updated_at = Utc::now();
    }

    pub fn add_metadata(&mut self, key: String, value: serde_json::Value) {
        self.metadata.insert(key, value);
        self.updated_at = Utc::now();
    }
}

/// Edge connecting two nodes in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KGEdge {
    pub uuid: Uuid,
    pub source_node_uuid: Uuid,
    pub target_node_uuid: Uuid,
    pub relation_type: String,
    pub summary: String,
    pub weight: f32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub group_id: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl KGEdge {
    pub fn new(
        source_node_uuid: Uuid,
        target_node_uuid: Uuid,
        relation_type: String,
        summary: String,
        weight: f32,
        group_id: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            uuid: Uuid::new_v4(),
            source_node_uuid,
            target_node_uuid,
            relation_type,
            summary,
            weight,
            created_at: now,
            updated_at: now,
            group_id,
            metadata: HashMap::new(),
        }
    }

    pub fn update_weight(&mut self, weight: f32) {
        self.weight = weight;
        self.updated_at = Utc::now();
    }
}

/// Episode representing a memory or event in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub uuid: Uuid,
    pub name: String,
    pub content: String,
    pub source: EpisodeSource,
    pub source_description: String,
    pub created_at: DateTime<Utc>,
    pub group_id: Option<String>,
    pub entity_uuids: Vec<Uuid>,
    pub edge_uuids: Vec<Uuid>,
    pub embedding: Option<Vec<f32>>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EpisodeSource {
    Text,
    Json,
    Message,
}

impl Episode {
    pub fn new(
        name: String,
        content: String,
        source: EpisodeSource,
        source_description: String,
        group_id: Option<String>,
    ) -> Self {
        Self {
            uuid: Uuid::new_v4(),
            name,
            content,
            source,
            source_description,
            created_at: Utc::now(),
            group_id,
            entity_uuids: Vec::new(),
            edge_uuids: Vec::new(),
            embedding: None,
            metadata: HashMap::new(),
        }
    }

    pub fn set_embedding(&mut self, embedding: Vec<f32>) {
        self.embedding = Some(embedding);
    }

    pub fn add_entity(&mut self, entity_uuid: Uuid) {
        if !self.entity_uuids.contains(&entity_uuid) {
            self.entity_uuids.push(entity_uuid);
        }
    }

    pub fn add_edge(&mut self, edge_uuid: Uuid) {
        if !self.edge_uuids.contains(&edge_uuid) {
            self.edge_uuids.push(edge_uuid);
        }
    }
}

/// Search result for knowledge graph queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub nodes: Vec<KGNode>,
    pub edges: Vec<KGEdge>,
    pub episodes: Vec<Episode>,
    pub scores: HashMap<Uuid, f32>,
}

impl SearchResult {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            episodes: Vec::new(),
            scores: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, node: KGNode, score: f32) {
        self.scores.insert(node.uuid, score);
        self.nodes.push(node);
    }

    pub fn add_edge(&mut self, edge: KGEdge, score: f32) {
        self.scores.insert(edge.uuid, score);
        self.edges.push(edge);
    }

    pub fn add_episode(&mut self, episode: Episode, score: f32) {
        self.scores.insert(episode.uuid, score);
        self.episodes.push(episode);
    }

    /// Get nodes with their scores as tuples
    pub fn nodes_with_scores(&self) -> Vec<(KGNode, f32)> {
        self.nodes.iter().map(|node| {
            let score = self.scores.get(&node.uuid).unwrap_or(&0.0);
            (node.clone(), *score)
        }).collect()
    }

    /// Get episodes with their scores as tuples  
    pub fn episodes_with_scores(&self) -> Vec<(Episode, f32)> {
        self.episodes.iter().map(|episode| {
            let score = self.scores.get(&episode.uuid).unwrap_or(&0.0);
            (episode.clone(), *score)
        }).collect()
    }

    pub fn sort_by_score(&mut self) {
        self.nodes.sort_by(|a, b| {
            let score_a = self.scores.get(&a.uuid).unwrap_or(&0.0);
            let score_b = self.scores.get(&b.uuid).unwrap_or(&0.0);
            score_b.partial_cmp(score_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        self.edges.sort_by(|a, b| {
            let score_a = self.scores.get(&a.uuid).unwrap_or(&0.0);
            let score_b = self.scores.get(&b.uuid).unwrap_or(&0.0);
            score_b.partial_cmp(score_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        self.episodes.sort_by(|a, b| {
            let score_a = self.scores.get(&a.uuid).unwrap_or(&0.0);
            let score_b = self.scores.get(&b.uuid).unwrap_or(&0.0);
            score_b.partial_cmp(score_a).unwrap_or(std::cmp::Ordering::Equal)
        });
    }
}

impl Default for SearchResult {
    fn default() -> Self {
        Self::new()
    }
} 
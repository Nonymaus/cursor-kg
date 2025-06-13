use std::collections::HashMap;
use uuid::Uuid;

use super::{KGNode, KGEdge, Episode};

/// In-memory cache for frequently accessed graph data
pub struct GraphMemoryCache {
    nodes: HashMap<Uuid, KGNode>,
    edges: HashMap<Uuid, KGEdge>,
    episodes: HashMap<Uuid, Episode>,
    max_size: usize,
}

impl GraphMemoryCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            episodes: HashMap::new(),
            max_size,
        }
    }

    pub fn get_node(&self, uuid: &Uuid) -> Option<&KGNode> {
        self.nodes.get(uuid)
    }

    pub fn insert_node(&mut self, node: KGNode) {
        if self.nodes.len() >= self.max_size {
            // Simple eviction: remove oldest
            // TODO: Implement LRU eviction
        }
        self.nodes.insert(node.uuid, node);
    }

    pub fn get_edge(&self, uuid: &Uuid) -> Option<&KGEdge> {
        self.edges.get(uuid)
    }

    pub fn insert_edge(&mut self, edge: KGEdge) {
        if self.edges.len() >= self.max_size {
            // Simple eviction: remove oldest
            // TODO: Implement LRU eviction
        }
        self.edges.insert(edge.uuid, edge);
    }

    pub fn get_episode(&self, uuid: &Uuid) -> Option<&Episode> {
        self.episodes.get(uuid)
    }

    pub fn insert_episode(&mut self, episode: Episode) {
        if self.episodes.len() >= self.max_size {
            // Simple eviction: remove oldest
            // TODO: Implement LRU eviction
        }
        self.episodes.insert(episode.uuid, episode);
    }
} 
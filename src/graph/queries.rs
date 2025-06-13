use anyhow::Result;
use petgraph::Graph;
use petgraph::graph::{NodeIndex, EdgeIndex, UnGraph};
use petgraph::algo::{dijkstra, astar};
use petgraph::visit::EdgeRef;
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;
use super::{KGNode, KGEdge, Episode, SearchResult};
use crate::graph::storage::GraphStorage;
use tracing::{debug, info, warn};

/// Query optimization and execution engine
pub struct QueryEngine {
    storage: GraphStorage,
    graph_cache: std::sync::RwLock<Option<GraphCache>>,
}

struct GraphCache {
    graph: UnGraph<Uuid, f32>,
    node_map: HashMap<Uuid, NodeIndex>,
    edge_map: HashMap<Uuid, EdgeIndex>,
    last_updated: std::time::Instant,
}

/// Graph query configuration
#[derive(Debug, Clone)]
pub struct QueryConfig {
    pub max_depth: u32,
    pub max_results: usize,
    pub similarity_threshold: f32,
    pub enable_caching: bool,
    pub timeout_seconds: u64,
}

impl Default for QueryConfig {
    fn default() -> Self {
        Self {
            max_depth: 5,
            max_results: 100,
            similarity_threshold: 0.7,
            enable_caching: true,
            timeout_seconds: 30,
        }
    }
}

/// Advanced graph query engine
pub struct GraphQueryEngine {
    config: QueryConfig,
    query_cache: std::sync::RwLock<HashMap<String, Vec<SearchResult>>>,
}

impl QueryEngine {
    pub fn new(storage: GraphStorage) -> Self {
        Self {
            storage,
            graph_cache: std::sync::RwLock::new(None),
        }
    }

    /// Execute a complex query with graph traversal
    pub async fn execute_query(&self, query: &str) -> Result<SearchResult> {
        println!("🔍 Executing graph query: {}", query);
        
        // Parse query type and execute appropriate algorithm
        if query.starts_with("traverse:") {
            self.execute_traversal_query(&query[9..]).await
        } else if query.starts_with("shortest:") {
            self.execute_shortest_path_query(&query[9..]).await
        } else if query.starts_with("cluster:") {
            self.execute_clustering_query(&query[8..]).await
        } else if query.starts_with("similar:") {
            self.execute_similarity_query(&query[8..]).await
        } else {
            // Default: hybrid search
            self.execute_hybrid_query(query).await
        }
    }

    /// Find connected nodes with maximum depth limit
    pub async fn find_connected_nodes(&self, start_node: Uuid, max_depth: usize) -> Result<Vec<KGNode>> {
        let graph = self.ensure_graph_cache().await?;
        let graph_guard = match graph.read() {
            Ok(guard) => guard,
            Err(e) => {
                tracing::error!("Failed to acquire graph cache read lock: {}", e);
                return Ok(Vec::new());
            }
        };
        
        let cache = match graph_guard.as_ref() {
            Some(cache) => cache,
            None => {
                tracing::warn!("Graph cache not initialized");
                return Ok(Vec::new());
            }
        };

        let start_idx = match cache.node_map.get(&start_node) {
            Some(idx) => *idx,
            None => {
                tracing::warn!("Start node not found in graph: {}", start_node);
                return Ok(Vec::new());
            }
        };

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut connected_nodes = Vec::new();

        queue.push_back((start_idx, 0u32));
        visited.insert(start_idx);

        while let Some((current_idx, depth)) = queue.pop_front() {
            if depth >= max_depth as u32 {
                continue;
            }

            // Get neighbors
            for edge in cache.graph.edges(current_idx) {
                let neighbor_idx = edge.target();
                if !visited.contains(&neighbor_idx) {
                    visited.insert(neighbor_idx);
                    queue.push_back((neighbor_idx, depth + 1));
                }
            }
        }

        // Convert node indices back to UUIDs and fetch node data
        for node_idx in visited {
            if let Some((&uuid, _)) = cache.node_map.iter().find(|(_, &idx)| idx == node_idx) {
                if let Ok(Some(node)) = self.storage.get_node(uuid) {
                    connected_nodes.push(node);
                }
            }
        }

        println!("🔗 Found {} connected nodes within {} hops", connected_nodes.len(), max_depth);
        Ok(connected_nodes)
    }

    /// Find shortest path between two nodes using Dijkstra's algorithm
    pub async fn find_shortest_path(&self, from: Uuid, to: Uuid) -> Result<Vec<KGEdge>> {
        let graph = self.ensure_graph_cache().await?;
        let graph_guard = match graph.read() {
            Ok(guard) => guard,
            Err(e) => {
                tracing::error!("Failed to acquire graph cache read lock for shortest path: {}", e);
                return Ok(Vec::new());
            }
        };
        
        let cache = match graph_guard.as_ref() {
            Some(cache) => cache,
            None => {
                tracing::warn!("Graph cache not initialized for shortest path");
                return Ok(Vec::new());
            }
        };

        let start_idx = cache.node_map.get(&from)
            .ok_or_else(|| anyhow::anyhow!("Start node not found: {}", from))?;
        let end_idx = cache.node_map.get(&to)
            .ok_or_else(|| anyhow::anyhow!("End node not found: {}", to))?;

        // Use Dijkstra's algorithm to find shortest path
        let path_map = dijkstra(&cache.graph, *start_idx, Some(*end_idx), |edge| *edge.weight());
        
        if !path_map.contains_key(end_idx) {
            return Ok(Vec::new()); // No path found
        }

        // Reconstruct path
        let mut path_edges = Vec::new();
        let mut current = *end_idx;
        let mut visited = HashSet::new();

        while current != *start_idx && !visited.contains(&current) {
            visited.insert(current);
            
            // Find the edge that led to current node
            for edge in cache.graph.edges_directed(current, petgraph::Direction::Incoming) {
                let source = edge.source();
                if path_map.contains_key(&source) {
                    // Find the corresponding edge UUID
                    if let Some((&edge_uuid, _)) = cache.edge_map.iter()
                        .find(|(_, &idx)| idx == edge.id()) {
                        if let Ok(Some(kg_edge)) = self.storage.get_edge(edge_uuid) {
                            path_edges.push(kg_edge);
                        }
                    }
                    current = source;
                    break;
                }
            }
        }

        path_edges.reverse();
        println!("🛤️  Found shortest path with {} edges", path_edges.len());
        Ok(path_edges)
    }

    /// Find nodes within a similarity threshold using embeddings
    pub async fn find_similar_nodes(&self, target_uuid: Uuid, threshold: f32, limit: usize) -> Result<Vec<(KGNode, f32)>> {
        // Get target node embedding
        let target_node = self.storage.get_node(target_uuid)?
            .ok_or_else(|| anyhow::anyhow!("Target node not found"))?;

        // Get all nodes (this could be optimized with embedding indexing)
        let all_nodes = self.storage.search_nodes_by_text("", None, 10000)?;
        let mut similar_nodes = Vec::new();

        for node in all_nodes {
            if node.uuid == target_uuid {
                continue; // Skip self
            }

            // Calculate similarity (placeholder - would use actual embeddings)
            let similarity = self.calculate_node_similarity(&target_node, &node).await?;
            
            if similarity >= threshold {
                similar_nodes.push((node, similarity));
            }
        }

        // Sort by similarity and limit results
        similar_nodes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        similar_nodes.truncate(limit);

        println!("🎯 Found {} similar nodes above threshold {}", similar_nodes.len(), threshold);
        Ok(similar_nodes)
    }

    /// Perform community detection/clustering
    pub async fn detect_communities(&self, min_cluster_size: usize) -> Result<Vec<Vec<KGNode>>> {
        let graph = self.ensure_graph_cache().await?;
        let graph_guard = graph.read().unwrap();
        let cache = graph_guard.as_ref().unwrap();

        let mut communities = Vec::new();
        let mut visited = HashSet::new();

        // Simple connected component clustering
        for (&_uuid, &node_idx) in &cache.node_map {
            if visited.contains(&node_idx) {
                continue;
            }

            let mut component = Vec::new();
            let mut queue = VecDeque::new();
            queue.push_back(node_idx);
            visited.insert(node_idx);

            while let Some(current) = queue.pop_front() {
                // Add current node to component
                if let Some((&node_uuid, _)) = cache.node_map.iter().find(|(_, &idx)| idx == current) {
                    if let Ok(Some(node)) = self.storage.get_node(node_uuid) {
                        component.push(node);
                    }
                }

                // Add unvisited neighbors
                for edge in cache.graph.edges(current) {
                    let neighbor = edge.target();
                    if !visited.contains(&neighbor) {
                        visited.insert(neighbor);
                        queue.push_back(neighbor);
                    }
                }
            }

            if component.len() >= min_cluster_size {
                communities.push(component);
            }
        }

        println!("🔍 Detected {} communities with min size {}", communities.len(), min_cluster_size);
        Ok(communities)
    }

    /// Get node centrality metrics
    pub async fn calculate_centrality(&self, node_uuid: Uuid) -> Result<CentralityMetrics> {
        let graph = self.ensure_graph_cache().await?;
        let graph_guard = graph.read().unwrap();
        let cache = graph_guard.as_ref().unwrap();

        let node_idx = cache.node_map.get(&node_uuid)
            .ok_or_else(|| anyhow::anyhow!("Node not found: {}", node_uuid))?;

        // Degree centrality
        let degree = cache.graph.edges(*node_idx).count();
        let total_nodes = cache.graph.node_count();
        let degree_centrality = if total_nodes > 1 { 
            degree as f32 / (total_nodes - 1) as f32 
        } else { 
            0.0 
        };

        // Betweenness centrality (simplified approximation)
        let betweenness = self.calculate_betweenness_centrality(*node_idx, &cache.graph).await;

        // Closeness centrality
        let closeness = self.calculate_closeness_centrality(*node_idx, &cache.graph).await;

        Ok(CentralityMetrics {
            degree_centrality,
            betweenness_centrality: betweenness,
            closeness_centrality: closeness,
        })
    }

    // Private helper methods

    async fn ensure_graph_cache(&self) -> Result<&std::sync::RwLock<Option<GraphCache>>> {
        let should_rebuild = {
            let cache_guard = self.graph_cache.read().unwrap();
            match cache_guard.as_ref() {
                None => true,
                Some(cache) => cache.last_updated.elapsed().as_secs() > 300, // 5 minutes
            }
        };

        if should_rebuild {
            self.rebuild_graph_cache().await?;
        }

        Ok(&self.graph_cache)
    }

    async fn rebuild_graph_cache(&self) -> Result<()> {
        println!("🔄 Rebuilding graph cache...");
        
        let mut graph = UnGraph::new_undirected();
        let mut node_map = HashMap::new();
        let mut edge_map = HashMap::new();

        // Add all nodes
        let nodes = self.storage.search_nodes_by_text("", None, 100000)?;
        for node in nodes {
            let node_idx = graph.add_node(node.uuid);
            node_map.insert(node.uuid, node_idx);
        }

        // Add all edges (placeholder - would need to get all edges from storage)
        // For now, we'll create a simplified version
        
        let cache = GraphCache {
            graph,
            node_map,
            edge_map,
            last_updated: std::time::Instant::now(),
        };

        let mut cache_guard = self.graph_cache.write().unwrap();
        *cache_guard = Some(cache);

        println!("✅ Graph cache rebuilt");
        Ok(())
    }

    async fn execute_traversal_query(&self, query: &str) -> Result<SearchResult> {
        // Parse traversal query: "node_uuid,max_depth"
        let parts: Vec<&str> = query.split(',').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid traversal query format"));
        }

        let node_uuid = Uuid::parse_str(parts[0])?;
        let max_depth: u32 = parts[1].parse()?;

        let connected_nodes = self.find_connected_nodes(node_uuid, max_depth as usize).await?;
        
        let mut result = SearchResult::new();
        for node in connected_nodes {
            result.add_node(node, 1.0);
        }

        Ok(result)
    }

    async fn execute_shortest_path_query(&self, query: &str) -> Result<SearchResult> {
        // Parse shortest path query: "from_uuid,to_uuid"
        let parts: Vec<&str> = query.split(',').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid shortest path query format"));
        }

        let from_uuid = Uuid::parse_str(parts[0])?;
        let to_uuid = Uuid::parse_str(parts[1])?;

        let path_edges = self.find_shortest_path(from_uuid, to_uuid).await?;
        
        let mut result = SearchResult::new();
        for edge in path_edges {
            result.add_edge(edge, 1.0);
        }

        Ok(result)
    }

    async fn execute_clustering_query(&self, query: &str) -> Result<SearchResult> {
        let min_size: usize = query.parse().unwrap_or(3);
        let communities = self.detect_communities(min_size).await?;
        
        let mut result = SearchResult::new();
        for (i, community) in communities.iter().enumerate() {
            for node in community {
                result.add_node(node.clone(), 1.0 - (i as f32 * 0.1));
            }
        }

        Ok(result)
    }

    async fn execute_similarity_query(&self, query: &str) -> Result<SearchResult> {
        // Parse similarity query: "node_uuid,threshold"
        let parts: Vec<&str> = query.split(',').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid similarity query format"));
        }

        let node_uuid = Uuid::parse_str(parts[0])?;
        let threshold: f32 = parts[1].parse()?;

        let similar_nodes = self.find_similar_nodes(node_uuid, threshold, 50).await?;
        
        let mut result = SearchResult::new();
        for (node, score) in similar_nodes {
            result.add_node(node, score);
        }

        Ok(result)
    }

    async fn execute_hybrid_query(&self, query: &str) -> Result<SearchResult> {
        // Combine text search and graph traversal
        let text_results = self.storage.search_nodes_by_text(query, None, 20)?;
        
        let mut result = SearchResult::new();
        for node in text_results {
            result.add_node(node, 0.8); // Base score for text match
        }

        result.sort_by_score();
        Ok(result)
    }

    async fn calculate_node_similarity(&self, node1: &KGNode, node2: &KGNode) -> Result<f32> {
        // Placeholder similarity calculation
        // In practice, this would use actual embeddings
        let name_similarity = if node1.name == node2.name { 1.0 } else { 0.0 };
        let type_similarity = if node1.node_type == node2.node_type { 0.5 } else { 0.0 };
        
        Ok((name_similarity + type_similarity) / 2.0)
    }

    async fn calculate_betweenness_centrality(&self, node_idx: NodeIndex, graph: &UnGraph<Uuid, f32>) -> f32 {
        // Simplified betweenness centrality calculation
        // In practice, this would be more sophisticated
        let degree = graph.edges(node_idx).count();
        degree as f32 / (graph.node_count().max(1) as f32)
    }

    async fn calculate_closeness_centrality(&self, node_idx: NodeIndex, graph: &UnGraph<Uuid, f32>) -> f32 {
        // Simplified closeness centrality calculation
        let paths = dijkstra(graph, node_idx, None, |_| 1.0);
        let total_distance: f32 = paths.values().sum();
        
        if total_distance > 0.0 {
            (paths.len() as f32 - 1.0) / total_distance
        } else {
            0.0
        }
    }
}

#[derive(Debug, Clone)]
pub struct CentralityMetrics {
    pub degree_centrality: f32,
    pub betweenness_centrality: f32,
    pub closeness_centrality: f32,
}

impl GraphQueryEngine {
    pub fn new(config: QueryConfig) -> Self {
        Self {
            config,
            query_cache: std::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Find nodes by traversing relationships
    pub async fn traverse_relationships(
        &self,
        start_node: &KGNode,
        relationship_types: &[String],
        nodes: &[KGNode],
        edges: &[KGEdge],
    ) -> Result<Vec<KGNode>> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut results = Vec::new();

        queue.push_back((start_node.uuid, 0u32));
        visited.insert(start_node.uuid);

        // Build edge lookup for efficiency
        let mut edge_map = HashMap::new();
        for edge in edges {
            edge_map.entry(edge.source_node_uuid)
                .or_insert_with(Vec::new)
                .push(edge);
        }

        while let Some((current_uuid, depth)) = queue.pop_front() {
            if depth >= self.config.max_depth {
                continue;
            }

            if let Some(current_edges) = edge_map.get(&current_uuid) {
                for edge in current_edges {
                    // Check if relationship type matches
                    if relationship_types.is_empty() || relationship_types.contains(&edge.relation_type) {
                        if !visited.contains(&edge.target_node_uuid) {
                            visited.insert(edge.target_node_uuid);
                            queue.push_back((edge.target_node_uuid, depth + 1));

                            // Find the target node
                            if let Some(target_node) = nodes.iter().find(|n| n.uuid == edge.target_node_uuid) {
                                results.push(target_node.clone());
                            }
                        }
                    }
                }
            }

            if results.len() >= self.config.max_results {
                break;
            }
        }

        debug!("Traversal found {} related nodes", results.len());
        Ok(results)
    }

    /// Find shortest path between two nodes
    pub async fn find_shortest_path(
        &self,
        start_uuid: Uuid,
        end_uuid: Uuid,
        nodes: &[KGNode],
        edges: &[KGEdge],
    ) -> Result<Option<Vec<KGNode>>> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut parent_map = HashMap::new();

        queue.push_back(start_uuid);
        visited.insert(start_uuid);

        // Build edge lookup
        let mut edge_map = HashMap::new();
        for edge in edges {
            edge_map.entry(edge.source_node_uuid)
                .or_insert_with(Vec::new)
                .push(edge);
        }

        // BFS to find shortest path
        while let Some(current_uuid) = queue.pop_front() {
            if current_uuid == end_uuid {
                // Reconstruct path
                let mut path = Vec::new();
                let mut current = end_uuid;

                while let Some(&parent) = parent_map.get(&current) {
                    if let Some(node) = nodes.iter().find(|n| n.uuid == current) {
                        path.push(node.clone());
                    }
                    current = parent;
                }

                // Add start node
                if let Some(start_node) = nodes.iter().find(|n| n.uuid == start_uuid) {
                    path.push(start_node.clone());
                }

                path.reverse();
                return Ok(Some(path));
            }

            if let Some(current_edges) = edge_map.get(&current_uuid) {
                for edge in current_edges {
                    if !visited.contains(&edge.target_node_uuid) {
                        visited.insert(edge.target_node_uuid);
                        parent_map.insert(edge.target_node_uuid, current_uuid);
                        queue.push_back(edge.target_node_uuid);
                    }
                }
            }
        }

        Ok(None)
    }

    /// Find nodes within a certain distance
    pub async fn find_nodes_within_distance(
        &self,
        center_uuid: Uuid,
        max_distance: u32,
        nodes: &[KGNode],
        edges: &[KGEdge],
    ) -> Result<Vec<(KGNode, u32)>> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut results = Vec::new();

        queue.push_back((center_uuid, 0u32));
        visited.insert(center_uuid);

        // Build edge lookup
        let mut edge_map = HashMap::new();
        for edge in edges {
            edge_map.entry(edge.source_node_uuid)
                .or_insert_with(Vec::new)
                .push(edge);
        }

        while let Some((current_uuid, distance)) = queue.pop_front() {
            if distance > max_distance {
                continue;
            }

            // Add current node to results (except the center)
            if distance > 0 {
                if let Some(node) = nodes.iter().find(|n| n.uuid == current_uuid) {
                    results.push((node.clone(), distance));
                }
            }

            if let Some(current_edges) = edge_map.get(&current_uuid) {
                for edge in current_edges {
                    if !visited.contains(&edge.target_node_uuid) {
                        visited.insert(edge.target_node_uuid);
                        queue.push_back((edge.target_node_uuid, distance + 1));
                    }
                }
            }

            if results.len() >= self.config.max_results {
                break;
            }
        }

        debug!("Found {} nodes within distance {}", results.len(), max_distance);
        Ok(results)
    }

    /// Find strongly connected components
    pub async fn find_connected_components(
        &self,
        nodes: &[KGNode],
        edges: &[KGEdge],
    ) -> Result<Vec<Vec<KGNode>>> {
        let mut visited = HashSet::new();
        let mut components = Vec::new();

        // Build adjacency list
        let mut adj_list = HashMap::new();
        for edge in edges {
            adj_list.entry(edge.source_node_uuid)
                .or_insert_with(Vec::new)
                .push(edge.target_node_uuid);
            // Add reverse edge for undirected traversal
            adj_list.entry(edge.target_node_uuid)
                .or_insert_with(Vec::new)
                .push(edge.source_node_uuid);
        }

        for node in nodes {
            if !visited.contains(&node.uuid) {
                let mut component = Vec::new();
                let mut stack = vec![node.uuid];

                while let Some(current_uuid) = stack.pop() {
                    if !visited.contains(&current_uuid) {
                        visited.insert(current_uuid);
                        
                        if let Some(current_node) = nodes.iter().find(|n| n.uuid == current_uuid) {
                            component.push(current_node.clone());
                        }

                        if let Some(neighbors) = adj_list.get(&current_uuid) {
                            for &neighbor_uuid in neighbors {
                                if !visited.contains(&neighbor_uuid) {
                                    stack.push(neighbor_uuid);
                                }
                            }
                        }
                    }
                }

                if !component.is_empty() {
                    components.push(component);
                }
            }
        }

        debug!("Found {} connected components", components.len());
        Ok(components)
    }

    /// Find nodes by pattern matching
    pub async fn pattern_match(
        &self,
        pattern: &GraphPattern,
        nodes: &[KGNode],
        edges: &[KGEdge],
    ) -> Result<Vec<PatternMatch>> {
        let mut matches = Vec::new();

        // Simple pattern matching implementation
        for node in nodes {
            if self.node_matches_pattern(node, &pattern.node_pattern) {
                let mut node_matches = vec![node.clone()];
                
                // Check if connected nodes match the pattern
                if let Some(ref edge_patterns) = pattern.edge_patterns {
                    for edge_pattern in edge_patterns {
                        let connected_nodes = self.find_connected_nodes_matching_pattern(
                            node.uuid,
                            edge_pattern,
                            nodes,
                            edges,
                        ).await?;
                        
                        if !connected_nodes.is_empty() {
                            node_matches.extend(connected_nodes);
                        }
                    }
                }

                matches.push(PatternMatch {
                    nodes: node_matches,
                    confidence: 1.0, // Simplified
                });
            }
        }

        debug!("Pattern matching found {} matches", matches.len());
        Ok(matches)
    }

    // Helper methods

    fn node_matches_pattern(&self, node: &KGNode, pattern: &NodePattern) -> bool {
        // Check node type
        if let Some(ref expected_type) = pattern.node_type {
            if &node.node_type != expected_type {
                return false;
            }
        }

        // Check properties
        for (key, expected_value) in &pattern.properties {
            if let Some(actual_value) = node.metadata.get(key) {
                if actual_value != expected_value {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }

    async fn find_connected_nodes_matching_pattern(
        &self,
        start_uuid: Uuid,
        edge_pattern: &EdgePattern,
        nodes: &[KGNode],
        edges: &[KGEdge],
    ) -> Result<Vec<KGNode>> {
        let mut results = Vec::new();

        for edge in edges {
            if edge.source_node_uuid == start_uuid {
                // Check if edge matches pattern
                if let Some(ref expected_type) = edge_pattern.relationship_type {
                    if &edge.relation_type != expected_type {
                        continue;
                    }
                }

                // Find target node
                if let Some(target_node) = nodes.iter().find(|n| n.uuid == edge.target_node_uuid) {
                    if self.node_matches_pattern(target_node, &edge_pattern.target_pattern) {
                        results.push(target_node.clone());
                    }
                }
            }
        }

        Ok(results)
    }
}

/// Graph pattern for pattern matching
#[derive(Debug, Clone)]
pub struct GraphPattern {
    pub node_pattern: NodePattern,
    pub edge_patterns: Option<Vec<EdgePattern>>,
}

#[derive(Debug, Clone)]
pub struct NodePattern {
    pub node_type: Option<String>,
    pub properties: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct EdgePattern {
    pub relationship_type: Option<String>,
    pub target_pattern: NodePattern,
}

#[derive(Debug, Clone)]
pub struct PatternMatch {
    pub nodes: Vec<KGNode>,
    pub confidence: f32,
} 
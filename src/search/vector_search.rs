use anyhow::Result;
use std::collections::BinaryHeap;
use std::cmp::Ordering;
use crate::graph::{KGNode, KGEdge, Episode, SearchResult};
use crate::embeddings::{LocalEmbeddingEngine, cosine_similarity, euclidean_distance};

pub struct VectorSearchEngine {
    embedding_engine: Option<LocalEmbeddingEngine>,
    similarity_threshold: f32,
    distance_metric: DistanceMetric,
}

#[derive(Clone, Debug)]
pub enum DistanceMetric {
    Cosine,
    Euclidean,
    DotProduct,
    Manhattan,
}

#[derive(Debug, Clone)]
struct ScoredItem<T> {
    item: T,
    score: f32,
}

impl<T> PartialEq for ScoredItem<T> {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
    }
}

impl<T> Eq for ScoredItem<T> {}

impl<T> PartialOrd for ScoredItem<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        other.score.partial_cmp(&self.score) // Reverse for max-heap
    }
}

impl<T> Ord for ScoredItem<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

impl VectorSearchEngine {
    pub fn new() -> Self {
        Self {
            embedding_engine: None,
            similarity_threshold: 0.7,
            distance_metric: DistanceMetric::Cosine,
        }
    }

    pub fn with_embedding_engine(mut self, engine: LocalEmbeddingEngine) -> Self {
        self.embedding_engine = Some(engine);
        self
    }

    pub fn with_similarity_threshold(mut self, threshold: f32) -> Self {
        self.similarity_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    pub fn with_distance_metric(mut self, metric: DistanceMetric) -> Self {
        self.distance_metric = metric;
        self
    }

    /// Search for similar nodes using embedding vectors
    pub async fn search_nodes(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<KGNode>> {
        println!("🔍 Vector search for {} similar nodes", limit);
        
        // Get all nodes with embeddings (in practice, this would be optimized with vector indexing)
        let all_nodes = self.get_all_nodes_with_embeddings().await?;
        
        let mut scored_nodes = BinaryHeap::new();
        
        for (node, embedding) in all_nodes {
            let similarity = self.calculate_similarity(query_embedding, &embedding)?;
            
            if similarity >= self.similarity_threshold {
                scored_nodes.push(ScoredItem {
                    item: node,
                    score: similarity,
                });
            }
        }

        // Extract top results
        let mut results = Vec::new();
        for _ in 0..limit.min(scored_nodes.len()) {
            if let Some(scored_item) = scored_nodes.pop() {
                results.push(scored_item.item);
            }
        }

        println!("✅ Found {} similar nodes", results.len());
        Ok(results)
    }

    /// Search for similar episodes using embedding vectors
    pub async fn search_episodes(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<Episode>> {
        println!("🔍 Vector search for {} similar episodes", limit);
        
        // Get all episodes with embeddings
        let all_episodes = self.get_all_episodes_with_embeddings().await?;
        
        let mut scored_episodes = BinaryHeap::new();
        
        for (episode, embedding) in all_episodes {
            let similarity = self.calculate_similarity(query_embedding, &embedding)?;
            
            if similarity >= self.similarity_threshold {
                scored_episodes.push(ScoredItem {
                    item: episode,
                    score: similarity,
                });
            }
        }

        // Extract top results
        let mut results = Vec::new();
        for _ in 0..limit.min(scored_episodes.len()) {
            if let Some(scored_item) = scored_episodes.pop() {
                results.push(scored_item.item);
            }
        }

        println!("✅ Found {} similar episodes", results.len());
        Ok(results)
    }

    /// Advanced semantic search with query expansion
    pub async fn semantic_search(&self, query: &str, candidates: &[KGNode], top_k: usize) -> Result<Vec<(KGNode, f32)>> {
        let engine = self.embedding_engine.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Embedding engine not initialized"))?;

        // Generate query embedding
        let query_embedding = engine.encode_text(query).await?;
        
        // Calculate similarities for all candidates
        let mut scored_candidates = BinaryHeap::new();
        
        for node in candidates {
            // Generate embedding for the node (combining name, summary, etc.)
            let node_text = format!("{} {} {}", node.name, node.node_type, node.summary);
            let node_embedding = engine.encode_text(&node_text).await?;
            
            let similarity = self.calculate_similarity(&query_embedding, &node_embedding)?;
            
            if similarity >= self.similarity_threshold {
                scored_candidates.push(ScoredItem {
                    item: node.clone(),
                    score: similarity,
                });
            }
        }

        // Extract top-k results
        let mut results = Vec::new();
        for _ in 0..top_k.min(scored_candidates.len()) {
            if let Some(scored_item) = scored_candidates.pop() {
                results.push((scored_item.item, scored_item.score));
            }
        }

        println!("🎯 Semantic search found {} relevant nodes", results.len());
        Ok(results)
    }

    /// Approximate nearest neighbor search using HNSW-inspired approach
    pub async fn approximate_knn_search(&self, query_embedding: &[f32], k: usize) -> Result<Vec<(KGNode, f32)>> {
        // This is a simplified version of approximate nearest neighbor search
        // In practice, you'd use a proper HNSW or LSH implementation
        
        let all_nodes = self.get_all_nodes_with_embeddings().await?;
        let mut candidates = Vec::new();
        
        // Sample a subset for approximation (in practice, this would be more sophisticated)
        let sample_size = (all_nodes.len() / 4).max(k * 10).min(all_nodes.len());
        let step = all_nodes.len() / sample_size.max(1);
        
        for (i, (node, embedding)) in all_nodes.iter().enumerate() {
            if i % step == 0 {
                let similarity = self.calculate_similarity(query_embedding, embedding)?;
                candidates.push((node.clone(), similarity));
            }
        }

        // Sort by similarity and return top-k
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
        candidates.truncate(k);

        println!("🚀 Approximate KNN search found {} candidates", candidates.len());
        Ok(candidates)
    }

    /// Multi-vector search combining different aspects
    pub async fn multi_vector_search(&self, query_vectors: &[Vec<f32>], weights: &[f32], limit: usize) -> Result<Vec<KGNode>> {
        if query_vectors.len() != weights.len() {
            return Err(anyhow::anyhow!("Query vectors and weights must have same length"));
        }

        let all_nodes = self.get_all_nodes_with_embeddings().await?;
        let mut scored_nodes = BinaryHeap::new();

        for (node, node_embedding) in all_nodes {
            let mut combined_score = 0.0;
            let mut total_weight = 0.0;

            for (query_vec, weight) in query_vectors.iter().zip(weights.iter()) {
                let similarity = self.calculate_similarity(query_vec, &node_embedding)?;
                combined_score += similarity * weight;
                total_weight += weight;
            }

            if total_weight > 0.0 {
                combined_score /= total_weight;
            }

            if combined_score >= self.similarity_threshold {
                scored_nodes.push(ScoredItem {
                    item: node,
                    score: combined_score,
                });
            }
        }

        let mut results = Vec::new();
        for _ in 0..limit.min(scored_nodes.len()) {
            if let Some(scored_item) = scored_nodes.pop() {
                results.push(scored_item.item);
            }
        }

        println!("🔀 Multi-vector search found {} nodes", results.len());
        Ok(results)
    }

    /// Embedding-based clustering
    pub async fn cluster_nodes(&self, nodes: &[KGNode], num_clusters: usize, max_iterations: usize) -> Result<Vec<Vec<KGNode>>> {
        if nodes.is_empty() || num_clusters == 0 {
            return Ok(Vec::new());
        }

        let engine = self.embedding_engine.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Embedding engine not initialized"))?;

        // Generate embeddings for all nodes
        let mut node_embeddings = Vec::new();
        for node in nodes {
            let node_text = format!("{} {} {}", node.name, node.node_type, node.summary);
            let embedding = engine.encode_text(&node_text).await?;
            node_embeddings.push((node.clone(), embedding));
        }

        // Simple k-means clustering
        let clusters = self.k_means_clustering(&node_embeddings, num_clusters, max_iterations)?;
        
        println!("🔍 Clustered {} nodes into {} clusters", nodes.len(), clusters.len());
        Ok(clusters)
    }

    /// Find outliers based on embedding distances
    pub async fn find_outliers(&self, nodes: &[KGNode], outlier_threshold: f32) -> Result<Vec<KGNode>> {
        if nodes.len() < 3 {
            return Ok(Vec::new());
        }

        let engine = self.embedding_engine.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Embedding engine not initialized"))?;

        let mut node_embeddings = Vec::new();
        for node in nodes {
            let node_text = format!("{} {} {}", node.name, node.node_type, node.summary);
            let embedding = engine.encode_text(&node_text).await?;
            node_embeddings.push((node.clone(), embedding));
        }

        let mut outliers = Vec::new();

        for (i, (node, embedding)) in node_embeddings.iter().enumerate() {
            let mut distances = Vec::new();
            
            // Calculate distances to all other nodes
            for (j, (_, other_embedding)) in node_embeddings.iter().enumerate() {
                if i != j {
                    let distance = match self.distance_metric {
                        DistanceMetric::Cosine => 1.0 - cosine_similarity(embedding, other_embedding),
                        DistanceMetric::Euclidean => euclidean_distance(embedding, other_embedding),
                        _ => 1.0 - cosine_similarity(embedding, other_embedding),
                    };
                    distances.push(distance);
                }
            }

            // Calculate average distance
            let avg_distance = distances.iter().sum::<f32>() / distances.len() as f32;
            
            if avg_distance > outlier_threshold {
                outliers.push(node.clone());
            }
        }

        println!("🎯 Found {} outliers with threshold {}", outliers.len(), outlier_threshold);
        Ok(outliers)
    }

    // Private helper methods

    async fn get_all_nodes_with_embeddings(&self) -> Result<Vec<(KGNode, Vec<f32>)>> {
        // Placeholder implementation - in practice, this would query the database
        // for nodes that have associated embeddings
        Ok(Vec::new())
    }

    async fn get_all_episodes_with_embeddings(&self) -> Result<Vec<(Episode, Vec<f32>)>> {
        // Placeholder implementation - in practice, this would query the database
        // for episodes that have associated embeddings
        Ok(Vec::new())
    }

    fn calculate_similarity(&self, vec1: &[f32], vec2: &[f32]) -> Result<f32> {
        if vec1.len() != vec2.len() {
            return Err(anyhow::anyhow!("Vector dimensions must match"));
        }

        let similarity = match self.distance_metric {
            DistanceMetric::Cosine => cosine_similarity(vec1, vec2),
            DistanceMetric::Euclidean => {
                let distance = euclidean_distance(vec1, vec2);
                1.0 / (1.0 + distance) // Convert distance to similarity
            },
            DistanceMetric::DotProduct => {
                vec1.iter().zip(vec2.iter()).map(|(a, b)| a * b).sum::<f32>()
            },
            DistanceMetric::Manhattan => {
                let distance: f32 = vec1.iter().zip(vec2.iter()).map(|(a, b)| (a - b).abs()).sum();
                1.0 / (1.0 + distance)
            },
        };

        Ok(similarity.clamp(0.0, 1.0))
    }

    fn k_means_clustering(&self, node_embeddings: &[(KGNode, Vec<f32>)], k: usize, max_iterations: usize) -> Result<Vec<Vec<KGNode>>> {
        if node_embeddings.is_empty() || k == 0 {
            return Ok(Vec::new());
        }

        let embedding_dim = node_embeddings[0].1.len();
        let mut centroids = Vec::new();
        let mut assignments = vec![0; node_embeddings.len()];

        // Initialize centroids randomly
        for i in 0..k {
            let idx = i % node_embeddings.len();
            centroids.push(node_embeddings[idx].1.clone());
        }

        for _iteration in 0..max_iterations {
            let mut changed = false;

            // Assign points to nearest centroids
            for (i, (_, embedding)) in node_embeddings.iter().enumerate() {
                let mut best_cluster = 0;
                let mut best_distance = f32::INFINITY;

                for (j, centroid) in centroids.iter().enumerate() {
                    let distance = euclidean_distance(embedding, centroid);
                    if distance < best_distance {
                        best_distance = distance;
                        best_cluster = j;
                    }
                }

                if assignments[i] != best_cluster {
                    assignments[i] = best_cluster;
                    changed = true;
                }
            }

            if !changed {
                break;
            }

            // Update centroids
            for cluster_id in 0..k {
                let cluster_points: Vec<_> = node_embeddings.iter()
                    .enumerate()
                    .filter(|(i, _)| assignments[*i] == cluster_id)
                    .map(|(_, (_, embedding))| embedding)
                    .collect();

                if !cluster_points.is_empty() {
                    let mut new_centroid = vec![0.0; embedding_dim];
                    for point in &cluster_points {
                        for (i, &value) in point.iter().enumerate() {
                            new_centroid[i] += value;
                        }
                    }
                    for value in &mut new_centroid {
                        *value /= cluster_points.len() as f32;
                    }
                    centroids[cluster_id] = new_centroid;
                }
            }
        }

        // Group nodes by cluster
        let mut clusters = vec![Vec::new(); k];
        for (i, (node, _)) in node_embeddings.iter().enumerate() {
            clusters[assignments[i]].push(node.clone());
        }

        // Filter out empty clusters
        Ok(clusters.into_iter().filter(|cluster| !cluster.is_empty()).collect())
    }
} 
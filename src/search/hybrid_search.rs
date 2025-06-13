use anyhow::Result;
use crate::graph::{KGNode, KGEdge, Episode, SearchResult};
use crate::search::{TextSearchEngine, VectorSearchEngine};
use crate::embeddings::LocalEmbeddingEngine;
use std::collections::HashMap;

pub struct HybridSearchEngine {
    text_engine: TextSearchEngine,
    vector_engine: VectorSearchEngine,
    embedding_engine: Option<LocalEmbeddingEngine>,
    fusion_algorithm: FusionAlgorithm,
    text_weight: f32,
    vector_weight: f32,
}

#[derive(Debug, Clone)]
pub enum FusionAlgorithm {
    LinearCombination,
    ReciprocalRankFusion,
    BordaCount,
    WeightedSum,
    MaxScore,
    MinScore,
}

#[derive(Debug, Clone)]
pub struct HybridSearchOptions {
    pub text_weight: f32,
    pub vector_weight: f32,
    pub fusion_algorithm: FusionAlgorithm,
    pub diversification: bool,
    pub re_ranking: bool,
    pub query_expansion: bool,
}

impl Default for HybridSearchOptions {
    fn default() -> Self {
        Self {
            text_weight: 0.6,
            vector_weight: 0.4,
            fusion_algorithm: FusionAlgorithm::LinearCombination,
            diversification: true,
            re_ranking: true,
            query_expansion: false,
        }
    }
}

impl HybridSearchEngine {
    pub fn new(text_engine: TextSearchEngine, vector_engine: VectorSearchEngine) -> Self {
        Self {
            text_engine,
            vector_engine,
            embedding_engine: None,
            fusion_algorithm: FusionAlgorithm::LinearCombination,
            text_weight: 0.6,
            vector_weight: 0.4,
        }
    }

    pub fn with_embedding_engine(mut self, engine: LocalEmbeddingEngine) -> Self {
        self.embedding_engine = Some(engine);
        self
    }

    pub fn with_fusion_algorithm(mut self, algorithm: FusionAlgorithm) -> Self {
        self.fusion_algorithm = algorithm;
        self
    }

    pub fn with_weights(mut self, text_weight: f32, vector_weight: f32) -> Self {
        // Normalize weights
        let total = text_weight + vector_weight;
        self.text_weight = text_weight / total;
        self.vector_weight = vector_weight / total;
        self
    }

    /// Hybrid search combining text and vector similarity
    pub async fn search(&self, query: &str, limit: usize) -> Result<SearchResult> {
        let options = HybridSearchOptions::default();
        self.search_with_options(query, limit, &options).await
    }

    /// Hybrid search with custom options
    pub async fn search_with_options(&self, query: &str, limit: usize, options: &HybridSearchOptions) -> Result<SearchResult> {
        println!("🔍 Hybrid search: '{}' (limit: {})", query, limit);

        // Perform text search
        let text_results = self.text_engine.search_nodes(query, limit * 2).await?;
        
        // Perform vector search if embedding engine is available
        let vector_results = if let Some(embedding_engine) = &self.embedding_engine {
            let query_embedding = embedding_engine.encode_text(query).await?;
            self.vector_engine.search_nodes(&query_embedding, limit * 2).await?
        } else {
            Vec::new()
        };

        // Combine and rank results
        let combined_results = self.combine_results(&text_results, &vector_results, options).await?;
        
        let mut result = SearchResult::new();
        for (node, score) in combined_results.into_iter().take(limit) {
            result.add_node(node, score);
        }

        result.sort_by_score();
        println!("✅ Hybrid search completed");
        Ok(result)
    }

    /// Multi-modal search combining different query types
    pub async fn multi_modal_search(&self, queries: &HashMap<String, String>, limit: usize) -> Result<SearchResult> {
        println!("🔍 Multi-modal search with {} query types", queries.len());

        let mut all_results = Vec::new();
        let mut modal_weights = HashMap::new();

        for (modal_type, query) in queries {
            let weight = self.get_modal_weight(modal_type);
            modal_weights.insert(modal_type.clone(), weight);

            match modal_type.as_str() {
                "text" => {
                    let nodes = self.text_engine.search_nodes(query, limit * 2).await?;
                    for node in nodes {
                        all_results.push((node, 1.0, modal_type.clone()));
                    }
                },
                "semantic" => {
                    if let Some(embedding_engine) = &self.embedding_engine {
                        let query_embedding = embedding_engine.encode_text(query).await?;
                        let nodes = self.vector_engine.search_nodes(&query_embedding, limit * 2).await?;
                        for node in nodes {
                            all_results.push((node, 1.0, modal_type.clone()));
                        }
                    }
                },
                "hybrid" => {
                    let search_result = self.search(query, limit * 2).await?;
                                    for (node, score) in search_result.nodes_with_scores() {
                    all_results.push((node, score, modal_type.clone()));
                }
                },
                _ => {
                    // Default to text search for unknown modal types
                    let nodes = self.text_engine.search_nodes(query, limit * 2).await?;
                    for node in nodes {
                        all_results.push((node, 1.0, modal_type.clone()));
                    }
                }
            }
        }

        // Aggregate results by node UUID and apply modal weights
        let mut node_scores: HashMap<uuid::Uuid, (KGNode, f32, usize)> = HashMap::new();
        
        for (node, score, modal_type) in all_results {
            let weight = modal_weights.get(&modal_type).unwrap_or(&1.0);
            let weighted_score = score * weight;
            
            let entry = node_scores.entry(node.uuid).or_insert((node.clone(), 0.0, 0));
            entry.1 += weighted_score;
            entry.2 += 1;
        }

        // Create final result
        let mut result = SearchResult::new();
        let mut final_results: Vec<(KGNode, f32)> = node_scores.into_values()
            .map(|(node, total_score, count)| (node, total_score / count as f32))
            .collect();

        final_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        final_results.truncate(limit);

        for (node, score) in final_results {
            result.add_node(node, score);
        }

        println!("✅ Multi-modal search completed");
        Ok(result)
    }

    /// Adaptive search that learns from user interactions
    pub async fn adaptive_search(&self, query: &str, user_feedback: &[UserFeedback], limit: usize) -> Result<SearchResult> {
        println!("🔍 Adaptive search with {} feedback items", user_feedback.len());

        // Analyze user feedback to adjust weights
        let (adjusted_text_weight, adjusted_vector_weight) = self.analyze_feedback(user_feedback);
        
        // Perform search with adjusted weights
        let options = HybridSearchOptions {
            text_weight: adjusted_text_weight,
            vector_weight: adjusted_vector_weight,
            ..Default::default()
        };

        let mut result = self.search_with_options(query, limit * 2, &options).await?;
        
        // Apply learned preferences
        self.apply_learned_preferences(&mut result, user_feedback).await?;
        
        // Limit results
        result.nodes.truncate(limit);
        result.episodes.truncate(limit);

        println!("✅ Adaptive search completed");
        Ok(result)
    }

    /// Contextual search that considers previous queries and results
    pub async fn contextual_search(&self, query: &str, context: &SearchContext, limit: usize) -> Result<SearchResult> {
        println!("🔍 Contextual search with {} previous queries", context.previous_queries.len());

        // Expand query based on context
        let expanded_query = self.expand_query_with_context(query, context).await?;
        
        // Perform hybrid search with expanded query
        let mut result = self.search(&expanded_query, limit * 2).await?;
        
        // Re-rank based on context relevance
        self.rerank_with_context(&mut result, context).await?;
        
        // Apply diversity filtering
        if context.diversify_results {
            self.diversify_results(&mut result, 0.3).await?;
        }

        result.nodes.truncate(limit);
        result.episodes.truncate(limit);

        println!("✅ Contextual search completed");
        Ok(result)
    }

    /// Faceted search with multiple dimensions
    pub async fn faceted_search(&self, query: &str, facets: &HashMap<String, Vec<String>>, limit: usize) -> Result<SearchResult> {
        println!("🔍 Faceted search with {} facet types", facets.len());

        let mut result = SearchResult::new();
        let mut facet_results = Vec::new();

        // Search within each facet
        for (facet_type, facet_values) in facets {
            for facet_value in facet_values {
                let faceted_query = format!("{} {}:{}", query, facet_type, facet_value);
                let facet_result = self.search(&faceted_query, limit).await?;
                
                for (node, score) in facet_result.nodes_with_scores() {
                    facet_results.push((node, score, facet_type.clone(), facet_value.clone()));
                }
            }
        }

        // Aggregate and score faceted results
        let mut node_scores: HashMap<uuid::Uuid, (KGNode, f32, Vec<String>)> = HashMap::new();
        
        for (node, score, facet_type, facet_value) in facet_results {
            let entry = node_scores.entry(node.uuid).or_insert((node.clone(), 0.0, Vec::new()));
            entry.1 += score;
            entry.2.push(format!("{}:{}", facet_type, facet_value));
        }

        // Create final result
        let mut final_results: Vec<(KGNode, f32)> = node_scores.into_values()
            .map(|(node, total_score, facets)| {
                // Boost score based on number of matching facets
                let facet_boost = 1.0 + (facets.len() as f32 * 0.1);
                (node, total_score * facet_boost)
            })
            .collect();

        final_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        final_results.truncate(limit);

        for (node, score) in final_results {
            result.add_node(node, score);
        }

        println!("✅ Faceted search completed");
        Ok(result)
    }

    // Private helper methods

    async fn combine_results(&self, text_results: &[KGNode], vector_results: &[KGNode], options: &HybridSearchOptions) -> Result<Vec<(KGNode, f32)>> {
        match options.fusion_algorithm {
            FusionAlgorithm::LinearCombination => {
                self.linear_combination_fusion(text_results, vector_results, options).await
            },
            FusionAlgorithm::ReciprocalRankFusion => {
                self.reciprocal_rank_fusion(text_results, vector_results, options).await
            },
            FusionAlgorithm::BordaCount => {
                self.borda_count_fusion(text_results, vector_results, options).await
            },
            FusionAlgorithm::WeightedSum => {
                self.weighted_sum_fusion(text_results, vector_results, options).await
            },
            FusionAlgorithm::MaxScore => {
                self.max_score_fusion(text_results, vector_results, options).await
            },
            FusionAlgorithm::MinScore => {
                self.min_score_fusion(text_results, vector_results, options).await
            },
        }
    }

    async fn linear_combination_fusion(&self, text_results: &[KGNode], vector_results: &[KGNode], options: &HybridSearchOptions) -> Result<Vec<(KGNode, f32)>> {
        let mut combined_scores: HashMap<uuid::Uuid, (KGNode, f32)> = HashMap::new();

        // Add text results with text weight
        for (rank, node) in text_results.iter().enumerate() {
            let text_score = 1.0 - (rank as f32 / text_results.len().max(1) as f32);
            combined_scores.insert(node.uuid, (node.clone(), text_score * options.text_weight));
        }

        // Add vector results with vector weight
        for (rank, node) in vector_results.iter().enumerate() {
            let vector_score = 1.0 - (rank as f32 / vector_results.len().max(1) as f32);
            let entry = combined_scores.entry(node.uuid).or_insert((node.clone(), 0.0));
            entry.1 += vector_score * options.vector_weight;
        }

        let mut results: Vec<(KGNode, f32)> = combined_scores.into_values().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(results)
    }

    async fn reciprocal_rank_fusion(&self, text_results: &[KGNode], vector_results: &[KGNode], _options: &HybridSearchOptions) -> Result<Vec<(KGNode, f32)>> {
        let mut rrf_scores: HashMap<uuid::Uuid, (KGNode, f32)> = HashMap::new();
        let k = 60.0; // RRF constant

        // Calculate RRF scores for text results
        for (rank, node) in text_results.iter().enumerate() {
            let rrf_score = 1.0 / (k + rank as f32 + 1.0);
            rrf_scores.insert(node.uuid, (node.clone(), rrf_score));
        }

        // Add RRF scores for vector results
        for (rank, node) in vector_results.iter().enumerate() {
            let rrf_score = 1.0 / (k + rank as f32 + 1.0);
            let entry = rrf_scores.entry(node.uuid).or_insert((node.clone(), 0.0));
            entry.1 += rrf_score;
        }

        let mut results: Vec<(KGNode, f32)> = rrf_scores.into_values().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(results)
    }

    async fn borda_count_fusion(&self, text_results: &[KGNode], vector_results: &[KGNode], _options: &HybridSearchOptions) -> Result<Vec<(KGNode, f32)>> {
        let mut borda_scores: HashMap<uuid::Uuid, (KGNode, f32)> = HashMap::new();

        // Borda count for text results
        for (rank, node) in text_results.iter().enumerate() {
            let borda_score = (text_results.len() - rank) as f32;
            borda_scores.insert(node.uuid, (node.clone(), borda_score));
        }

        // Add Borda count for vector results
        for (rank, node) in vector_results.iter().enumerate() {
            let borda_score = (vector_results.len() - rank) as f32;
            let entry = borda_scores.entry(node.uuid).or_insert((node.clone(), 0.0));
            entry.1 += borda_score;
        }

        let mut results: Vec<(KGNode, f32)> = borda_scores.into_values().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(results)
    }

    async fn weighted_sum_fusion(&self, text_results: &[KGNode], vector_results: &[KGNode], options: &HybridSearchOptions) -> Result<Vec<(KGNode, f32)>> {
        // Similar to linear combination but with different normalization
        self.linear_combination_fusion(text_results, vector_results, options).await
    }

    async fn max_score_fusion(&self, text_results: &[KGNode], vector_results: &[KGNode], _options: &HybridSearchOptions) -> Result<Vec<(KGNode, f32)>> {
        let mut max_scores: HashMap<uuid::Uuid, (KGNode, f32)> = HashMap::new();

        // Take max score from text results
        for (rank, node) in text_results.iter().enumerate() {
            let score = 1.0 - (rank as f32 / text_results.len().max(1) as f32);
            max_scores.insert(node.uuid, (node.clone(), score));
        }

        // Take max score from vector results
        for (rank, node) in vector_results.iter().enumerate() {
            let score = 1.0 - (rank as f32 / vector_results.len().max(1) as f32);
            let entry = max_scores.entry(node.uuid).or_insert((node.clone(), 0.0));
            entry.1 = entry.1.max(score);
        }

        let mut results: Vec<(KGNode, f32)> = max_scores.into_values().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(results)
    }

    async fn min_score_fusion(&self, text_results: &[KGNode], vector_results: &[KGNode], _options: &HybridSearchOptions) -> Result<Vec<(KGNode, f32)>> {
        let mut min_scores: HashMap<uuid::Uuid, (KGNode, f32)> = HashMap::new();

        // Initialize with text results
        for (rank, node) in text_results.iter().enumerate() {
            let score = 1.0 - (rank as f32 / text_results.len().max(1) as f32);
            min_scores.insert(node.uuid, (node.clone(), score));
        }

        // Take min score from vector results
        for (rank, node) in vector_results.iter().enumerate() {
            let score = 1.0 - (rank as f32 / vector_results.len().max(1) as f32);
            if let Some(entry) = min_scores.get_mut(&node.uuid) {
                entry.1 = entry.1.min(score);
            }
        }

        let mut results: Vec<(KGNode, f32)> = min_scores.into_values().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(results)
    }

    fn get_modal_weight(&self, modal_type: &str) -> f32 {
        match modal_type {
            "text" => 1.0,
            "semantic" => 1.0,
            "hybrid" => 1.5,
            "contextual" => 1.2,
            "faceted" => 1.1,
            _ => 0.8,
        }
    }

    fn analyze_feedback(&self, feedback: &[UserFeedback]) -> (f32, f32) {
        let mut text_positive = 0;
        let mut text_total = 0;
        let mut vector_positive = 0;
        let mut vector_total = 0;

        for item in feedback {
            if item.search_type == "text" {
                text_total += 1;
                if item.positive {
                    text_positive += 1;
                }
            } else if item.search_type == "vector" {
                vector_total += 1;
                if item.positive {
                    vector_positive += 1;
                }
            }
        }

        let text_success_rate = if text_total > 0 { text_positive as f32 / text_total as f32 } else { 0.5 };
        let vector_success_rate = if vector_total > 0 { vector_positive as f32 / vector_total as f32 } else { 0.5 };

        // Adjust weights based on success rates
        let total = text_success_rate + vector_success_rate;
        if total > 0.0 {
            (text_success_rate / total, vector_success_rate / total)
        } else {
            (0.6, 0.4) // Default weights
        }
    }

    async fn apply_learned_preferences(&self, result: &mut SearchResult, _feedback: &[UserFeedback]) -> Result<()> {
        // Placeholder for applying learned user preferences
        // In practice, this would adjust scores based on user interaction patterns
        Ok(())
    }

    async fn expand_query_with_context(&self, query: &str, context: &SearchContext) -> Result<String> {
        let mut expanded = query.to_string();
        
        // Add terms from previous successful queries
        for prev_query in &context.previous_queries {
            if prev_query.success_score > 0.7 {
                // Extract meaningful terms (simplified)
                let terms: Vec<&str> = prev_query.query.split_whitespace().take(2).collect();
                for term in terms {
                    if !expanded.contains(term) {
                        expanded.push_str(&format!(" {}", term));
                    }
                }
            }
        }

        Ok(expanded)
    }

    async fn rerank_with_context(&self, result: &mut SearchResult, _context: &SearchContext) -> Result<()> {
        // Placeholder for context-based re-ranking
        // In practice, this would adjust scores based on context relevance
        Ok(())
    }

    async fn diversify_results(&self, result: &mut SearchResult, diversity_threshold: f32) -> Result<()> {
        // Simple diversity algorithm - remove results that are too similar
        let mut diversified_nodes: Vec<(KGNode, f32)> = Vec::new();
        
        for (node, score) in result.nodes_with_scores() {
            let mut is_diverse = true;
            
            for (existing_node, _) in &diversified_nodes {
                // Simple similarity check based on type and name
                if node.node_type == existing_node.node_type && 
                   node.name.to_lowercase().contains(&existing_node.name.to_lowercase()) {
                    is_diverse = false;
                    break;
                }
            }
            
            if is_diverse || diversified_nodes.len() < 3 {
                diversified_nodes.push((node.clone(), score));
            }
        }
        
        // Rebuild result with diversified nodes
        let mut new_result = SearchResult::new();
        for (node, score) in diversified_nodes {
            new_result.add_node(node, score);
        }
        *result = new_result;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct UserFeedback {
    pub search_type: String,
    pub positive: bool,
    pub relevance_score: f32,
    pub timestamp: std::time::SystemTime,
}

#[derive(Debug, Clone)]
pub struct SearchContext {
    pub previous_queries: Vec<PreviousQuery>,
    pub user_preferences: HashMap<String, f32>,
    pub diversify_results: bool,
    pub current_task: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PreviousQuery {
    pub query: String,
    pub success_score: f32,
    pub result_count: usize,
    pub timestamp: std::time::SystemTime,
} 
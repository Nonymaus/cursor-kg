use anyhow::Result;
use crate::graph::{KGNode, KGEdge, Episode, SearchResult};
use crate::graph::storage::GraphStorage;
use std::collections::HashMap;
use regex::Regex;
use std::sync::Arc;
use crate::config::SearchConfig;

pub struct TextSearchEngine {
    storage: Arc<GraphStorage>,
    stemming_enabled: bool,
    fuzzy_matching: bool,
    min_score_threshold: f32,
    boost_factors: BoostFactors,
    config: SearchConfig,
}

#[derive(Debug, Clone)]
pub struct BoostFactors {
    pub name_boost: f32,
    pub type_boost: f32,
    pub summary_boost: f32,
    pub content_boost: f32,
    pub metadata_boost: f32,
}

impl Default for BoostFactors {
    fn default() -> Self {
        Self {
            name_boost: 2.0,
            type_boost: 1.5,
            summary_boost: 1.2,
            content_boost: 1.0,
            metadata_boost: 0.8,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub fuzzy_matching: bool,
    pub stemming: bool,
    pub case_sensitive: bool,
    pub phrase_matching: bool,
    pub proximity_search: bool,
    pub wildcard_search: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            fuzzy_matching: true,
            stemming: true,
            case_sensitive: false,
            phrase_matching: true,
            proximity_search: true,
            wildcard_search: true,
        }
    }
}

impl TextSearchEngine {
    pub fn new(storage: Arc<GraphStorage>) -> Self {
        Self {
            storage,
            stemming_enabled: true,
            fuzzy_matching: true,
            min_score_threshold: 0.1,
            boost_factors: BoostFactors::default(),
            config: Default::default(),
        }
    }

    pub fn with_boost_factors(mut self, boost_factors: BoostFactors) -> Self {
        self.boost_factors = boost_factors;
        self
    }

    pub fn with_min_score_threshold(mut self, threshold: f32) -> Self {
        self.min_score_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Enhanced node search with advanced ranking
    pub async fn search_nodes(&self, query: &str, limit: usize) -> Result<Vec<KGNode>> {
        let search_options = SearchOptions::default();
        self.search_nodes_with_options(query, limit, &search_options).await
    }

    /// Node search with custom options
    pub async fn search_nodes_with_options(&self, query: &str, limit: usize, options: &SearchOptions) -> Result<Vec<KGNode>> {
        println!("🔍 Text search for nodes: '{}' (limit: {})", query, limit);

        // Parse and enhance the query
        let enhanced_query = self.enhance_query(query, options)?;
        
        // Perform FTS5 search
        let raw_results = self.storage.search_nodes_by_text(&enhanced_query, None, limit * 2)?;
        
        // Apply advanced scoring and ranking
        let mut scored_results = Vec::new();
        for node in raw_results {
            let score = self.calculate_node_relevance_score(&node, query, options).await?;
            if score >= self.min_score_threshold {
                scored_results.push((node, score));
            }
        }

        // Sort by score and limit results
        scored_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_results.truncate(limit);

        let results: Vec<KGNode> = scored_results.into_iter().map(|(node, _)| node).collect();
        println!("✅ Found {} matching nodes", results.len());
        Ok(results)
    }

    /// Enhanced episode search with content analysis
    pub async fn search_episodes(&self, query: &str, limit: usize) -> Result<Vec<Episode>> {
        let search_options = SearchOptions::default();
        self.search_episodes_with_options(query, limit, &search_options).await
    }

    /// Episode search with custom options
    pub async fn search_episodes_with_options(&self, query: &str, limit: usize, options: &SearchOptions) -> Result<Vec<Episode>> {
        println!("🔍 Text search for episodes: '{}' (limit: {})", query, limit);

        // For episodes, we'll implement a custom search since the storage layer might need enhancement
        let enhanced_query = self.enhance_query(query, options)?;
        
        // Get recent episodes and score them
        let raw_episodes = self.storage.get_recent_episodes(None, limit * 3)?;
        let mut scored_results = Vec::new();

        for episode in raw_episodes {
            let score = self.calculate_episode_relevance_score(&episode, query, options).await?;
            if score >= self.min_score_threshold {
                scored_results.push((episode, score));
            }
        }

        // Sort by score and limit results
        scored_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_results.truncate(limit);

        let results: Vec<Episode> = scored_results.into_iter().map(|(episode, _)| episode).collect();
        println!("✅ Found {} matching episodes", results.len());
        Ok(results)
    }

    /// Multi-field search with field-specific boosting
    pub async fn multi_field_search(&self, field_queries: &HashMap<String, String>, limit: usize) -> Result<SearchResult> {
        println!("🔍 Multi-field search with {} field queries", field_queries.len());

        let mut result = SearchResult::new();
        let mut combined_scores: HashMap<uuid::Uuid, f32> = HashMap::new();

        for (field, query) in field_queries {
            let field_boost = self.get_field_boost_factor(field);
            
            if field == "name" || field == "type" || field == "summary" {
                let nodes = self.search_nodes(query, limit * 2).await?;
                for node in nodes {
                    let base_score = self.calculate_node_relevance_score(&node, query, &SearchOptions::default()).await?;
                    let boosted_score = base_score * field_boost;
                    
                    *combined_scores.entry(node.uuid).or_insert(0.0) += boosted_score;
                    
                    // Add to result if not already present
                    if !result.nodes.iter().any(|n| n.uuid == node.uuid) {
                        result.add_node(node, boosted_score);
                    }
                }
            } else if field == "content" {
                let episodes = self.search_episodes(query, limit * 2).await?;
                for episode in episodes {
                    let base_score = self.calculate_episode_relevance_score(&episode, query, &SearchOptions::default()).await?;
                    let boosted_score = base_score * field_boost;
                    
                    result.add_episode(episode, boosted_score);
                }
            }
        }

        // Update node scores with combined scores - rebuild result
        let mut new_result = SearchResult::new();
        for node in result.nodes {
            let final_score = combined_scores.get(&node.uuid).unwrap_or(&0.0);
            new_result.add_node(node, *final_score);
        }
        for episode in result.episodes {
            let score = result.scores.get(&episode.uuid).unwrap_or(&0.0);
            new_result.add_episode(episode, *score);
        }
        result = new_result;

        result.sort_by_score();
        result.nodes.truncate(limit);
        result.episodes.truncate(limit);

        println!("✅ Multi-field search completed");
        Ok(result)
    }

    /// Phrase search with proximity matching
    pub async fn phrase_search(&self, phrase: &str, proximity: u32, limit: usize) -> Result<Vec<KGNode>> {
        println!("🔍 Phrase search: '{}' (proximity: {})", phrase, proximity);

        // Parse phrase into terms
        let terms: Vec<&str> = phrase.split_whitespace().collect();
        if terms.len() < 2 {
            return self.search_nodes(phrase, limit).await;
        }

        // Build proximity query for FTS5
        let proximity_query = if proximity == 0 {
            format!("\"{}\"", phrase) // Exact phrase
        } else {
            format!("NEAR({}, {})", terms.join(" "), proximity)
        };

        let raw_results = self.storage.search_nodes_by_text(&proximity_query, None, limit * 2)?;
        
        let mut scored_results = Vec::new();
        for node in raw_results {
            let score = self.calculate_phrase_score(&node, &terms, proximity).await?;
            if score >= self.min_score_threshold {
                scored_results.push((node, score));
            }
        }

        scored_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_results.truncate(limit);

        let results: Vec<KGNode> = scored_results.into_iter().map(|(node, _)| node).collect();
        println!("✅ Found {} phrase matches", results.len());
        Ok(results)
    }

    /// Fuzzy search with edit distance tolerance
    pub async fn fuzzy_search(&self, query: &str, max_distance: u32, limit: usize) -> Result<Vec<KGNode>> {
        println!("🔍 Fuzzy search: '{}' (max distance: {})", query, max_distance);

        // Generate fuzzy query patterns
        let fuzzy_patterns = self.generate_fuzzy_patterns(query, max_distance);
        let mut all_results = Vec::new();

        for pattern in fuzzy_patterns {
            let pattern_results = self.storage.search_nodes_by_text(&pattern, None, limit)?;
            all_results.extend(pattern_results);
        }

        // Remove duplicates and score
        let mut unique_results: HashMap<uuid::Uuid, KGNode> = HashMap::new();
        for node in all_results {
            unique_results.insert(node.uuid, node);
        }

        let mut scored_results = Vec::new();
        for node in unique_results.into_values() {
            let score = self.calculate_fuzzy_score(&node, query, max_distance).await?;
            if score >= self.min_score_threshold {
                scored_results.push((node, score));
            }
        }

        scored_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_results.truncate(limit);

        let results: Vec<KGNode> = scored_results.into_iter().map(|(node, _)| node).collect();
        println!("✅ Found {} fuzzy matches", results.len());
        Ok(results)
    }

    /// Boolean search with AND, OR, NOT operators
    pub async fn boolean_search(&self, query: &str, limit: usize) -> Result<Vec<KGNode>> {
        println!("🔍 Boolean search: '{}'", query);

        // Parse boolean query
        let boolean_query = self.parse_boolean_query(query)?;
        
        // Execute boolean search using FTS5 boolean syntax
        let raw_results = self.storage.search_nodes_by_text(&boolean_query, None, limit * 2)?;
        
        let mut scored_results = Vec::new();
        for node in raw_results {
            let score = self.calculate_boolean_score(&node, query).await?;
            if score >= self.min_score_threshold {
                scored_results.push((node, score));
            }
        }

        scored_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_results.truncate(limit);

        let results: Vec<KGNode> = scored_results.into_iter().map(|(node, _)| node).collect();
        println!("✅ Found {} boolean matches", results.len());
        Ok(results)
    }

    // Private helper methods

    fn enhance_query(&self, query: &str, options: &SearchOptions) -> Result<String> {
        let mut enhanced = query.to_string();

        // Apply case normalization if not case sensitive
        if !options.case_sensitive {
            enhanced = enhanced.to_lowercase();
        }

        // Add wildcard support
        if options.wildcard_search && !enhanced.contains('*') && !enhanced.contains('?') {
            enhanced = format!("{}*", enhanced);
        }

        // Apply stemming (simplified)
        if options.stemming {
            enhanced = self.apply_stemming(&enhanced);
        }

        Ok(enhanced)
    }

    async fn calculate_node_relevance_score(&self, node: &KGNode, query: &str, _options: &SearchOptions) -> Result<f32> {
        let query_terms: Vec<&str> = query.split_whitespace().collect();
        let mut total_score = 0.0;

        // Score based on name matches
        let name_score = self.calculate_text_match_score(&node.name, &query_terms);
        total_score += name_score * self.boost_factors.name_boost;

        // Score based on type matches
        let type_score = self.calculate_text_match_score(&node.node_type, &query_terms);
        total_score += type_score * self.boost_factors.type_boost;

        // Score based on summary matches
        let summary_score = self.calculate_text_match_score(&node.summary, &query_terms);
        total_score += summary_score * self.boost_factors.summary_boost;

        // Normalize score
        Ok((total_score / (self.boost_factors.name_boost + self.boost_factors.type_boost + self.boost_factors.summary_boost)).clamp(0.0, 1.0))
    }

    async fn calculate_episode_relevance_score(&self, episode: &Episode, query: &str, _options: &SearchOptions) -> Result<f32> {
        let query_terms: Vec<&str> = query.split_whitespace().collect();
        let mut total_score = 0.0;

        // Score based on name matches
        let name_score = self.calculate_text_match_score(&episode.name, &query_terms);
        total_score += name_score * self.boost_factors.name_boost;

        // Score based on content matches
        let content_score = self.calculate_text_match_score(&episode.content, &query_terms);
        total_score += content_score * self.boost_factors.content_boost;

        // Normalize score
        Ok((total_score / (self.boost_factors.name_boost + self.boost_factors.content_boost)).clamp(0.0, 1.0))
    }

    fn calculate_text_match_score(&self, text: &str, query_terms: &[&str]) -> f32 {
        let text_lower = text.to_lowercase();
        let mut score = 0.0;
        let total_terms = query_terms.len() as f32;

        for term in query_terms {
            let term_lower = term.to_lowercase();
            
            // Exact match bonus
            if text_lower.contains(&term_lower) {
                score += 1.0;
                
                // Position bonus (earlier matches get higher scores)
                if let Some(pos) = text_lower.find(&term_lower) {
                    let position_factor = 1.0 - (pos as f32 / text.len().max(1) as f32);
                    score += position_factor * 0.5;
                }
            }
        }

        score / total_terms
    }

    async fn calculate_phrase_score(&self, node: &KGNode, terms: &[&str], proximity: u32) -> Result<f32> {
        let combined_text = format!("{} {} {}", node.name, node.node_type, node.summary).to_lowercase();
        let mut score = 0.0;

        // Check for exact phrase match
        let phrase = terms.join(" ").to_lowercase();
        if combined_text.contains(&phrase) {
            score += 1.0;
        }

        // Check for proximity matches
        if proximity > 0 {
            score += self.calculate_proximity_score(&combined_text, terms, proximity);
        }

        Ok(score.clamp(0.0, 1.0))
    }

    fn calculate_proximity_score(&self, text: &str, terms: &[&str], max_distance: u32) -> f32 {
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut max_score: f32 = 0.0;

        for (i, word) in words.iter().enumerate() {
            if terms.contains(word) {
                let mut local_score = 1.0;
                let mut found_terms = 1;

                // Look for other terms within proximity
                for j in 1..=max_distance as usize {
                    if i + j < words.len() && terms.contains(&words[i + j]) {
                        local_score += 1.0 / (j as f32);
                        found_terms += 1;
                    }
                    if i >= j && terms.contains(&words[i - j]) {
                        local_score += 1.0 / (j as f32);
                        found_terms += 1;
                    }
                }

                let term_coverage = found_terms as f32 / terms.len() as f32;
                max_score = max_score.max(local_score * term_coverage);
            }
        }

        max_score / terms.len() as f32
    }

    async fn calculate_fuzzy_score(&self, node: &KGNode, query: &str, max_distance: u32) -> Result<f32> {
        let combined_text = format!("{} {} {}", node.name, node.node_type, node.summary);
        let words: Vec<&str> = combined_text.split_whitespace().collect();
        let mut max_score: f32 = 0.0;

        for word in words {
            let distance = self.levenshtein_distance(query, word);
            if distance <= max_distance {
                let similarity = 1.0 - (distance as f32 / query.len().max(word.len()) as f32);
                max_score = max_score.max(similarity);
            }
        }

        Ok(max_score)
    }

    async fn calculate_boolean_score(&self, node: &KGNode, _query: &str) -> Result<f32> {
        // Simplified boolean scoring - in practice, this would parse the boolean expression
        // and calculate scores based on term presence/absence
        Ok(0.8)
    }

    fn get_field_boost_factor(&self, field: &str) -> f32 {
        match field {
            "name" => self.boost_factors.name_boost,
            "type" => self.boost_factors.type_boost,
            "summary" => self.boost_factors.summary_boost,
            "content" => self.boost_factors.content_boost,
            "metadata" => self.boost_factors.metadata_boost,
            _ => 1.0,
        }
    }

    fn apply_stemming(&self, text: &str) -> String {
        // Simplified stemming - in practice, you'd use a proper stemming library
        text.replace("ing", "").replace("ed", "").replace("s", "")
    }

    fn generate_fuzzy_patterns(&self, query: &str, _max_distance: u32) -> Vec<String> {
        // Simplified fuzzy pattern generation
        vec![
            query.to_string(),
            format!("{}*", query),
            format!("*{}", query),
            format!("*{}*", query),
        ]
    }

    fn parse_boolean_query(&self, query: &str) -> Result<String> {
        // Convert simple boolean query to FTS5 syntax
        let mut fts_query = query.to_string();
        
        fts_query = fts_query.replace(" AND ", " ");
        fts_query = fts_query.replace(" OR ", " OR ");
        fts_query = fts_query.replace(" NOT ", " NOT ");
        
        Ok(fts_query)
    }

    fn levenshtein_distance(&self, s1: &str, s2: &str) -> u32 {
        let len1 = s1.len();
        let len2 = s2.len();
        let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

        for i in 0..=len1 {
            matrix[i][0] = i;
        }
        for j in 0..=len2 {
            matrix[0][j] = j;
        }

        let s1_chars: Vec<char> = s1.chars().collect();
        let s2_chars: Vec<char> = s2.chars().collect();

        for i in 1..=len1 {
            for j in 1..=len2 {
                let cost = if s1_chars[i - 1] == s2_chars[j - 1] { 0 } else { 1 };
                matrix[i][j] = (matrix[i - 1][j] + 1)
                    .min(matrix[i][j - 1] + 1)
                    .min(matrix[i - 1][j - 1] + cost);
            }
        }

        matrix[len1][len2] as u32
    }
} 
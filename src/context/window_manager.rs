use anyhow::Result;
use std::collections::{HashMap, VecDeque, BTreeMap};
use std::sync::{Arc, RwLock};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tracing::{debug, info, warn};

use crate::graph::{KGNode, KGEdge, Episode};
use crate::embeddings::LocalEmbeddingEngine;

/// Context window configuration
#[derive(Debug, Clone)]
pub struct ContextWindowConfig {
    pub max_tokens: usize,
    pub overlap_tokens: usize,
    pub priority_boost: f32,
    pub recency_weight: f32,
    pub relevance_threshold: f32,
    pub max_chunks_per_file: usize,
    pub adaptive_chunking: bool,
    pub preserve_code_blocks: bool,
}

impl Default for ContextWindowConfig {
    fn default() -> Self {
        Self {
            max_tokens: 128_000,  // Large context window for modern models
            overlap_tokens: 200,
            priority_boost: 1.5,
            recency_weight: 0.3,
            relevance_threshold: 0.7,
            max_chunks_per_file: 50,
            adaptive_chunking: true,
            preserve_code_blocks: true,
        }
    }
}

/// Context chunk with metadata
#[derive(Debug, Clone)]
pub struct ContextChunk {
    pub id: Uuid,
    pub content: String,
    pub source_file: Option<String>,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
    pub chunk_type: ChunkType,
    pub priority: f32,
    pub relevance_score: f32,
    pub last_accessed: DateTime<Utc>,
    pub access_count: u32,
    pub embedding: Option<Vec<f32>>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChunkType {
    Code,
    Documentation,
    Configuration,
    Test,
    Comment,
    Import,
    Function,
    Class,
    Variable,
    Error,
    Log,
}

/// Context window manager for intelligent content selection
pub struct ContextWindowManager {
    config: ContextWindowConfig,
    chunks: Arc<RwLock<HashMap<Uuid, ContextChunk>>>,
    priority_queue: Arc<RwLock<BTreeMap<OrderedFloat, Uuid>>>,
    file_chunks: Arc<RwLock<HashMap<String, Vec<Uuid>>>>,
    embedding_engine: Option<Arc<LocalEmbeddingEngine>>,
    access_history: Arc<RwLock<VecDeque<(Uuid, DateTime<Utc>)>>>,
    token_estimator: TokenEstimator,
}

/// Wrapper for f32 to make it orderable
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
struct OrderedFloat(f32);

impl Eq for OrderedFloat {}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.partial_cmp(&other.0).unwrap_or(std::cmp::Ordering::Equal)
    }
}

/// Token estimation for different content types
struct TokenEstimator;

impl TokenEstimator {
    fn estimate_tokens(&self, content: &str, chunk_type: &ChunkType) -> usize {
        let base_tokens = content.len() / 4; // Rough approximation: 4 chars per token
        
        // Adjust based on content type
        match chunk_type {
            ChunkType::Code => (base_tokens as f32 * 1.2) as usize, // Code is denser
            ChunkType::Documentation => base_tokens,
            ChunkType::Comment => (base_tokens as f32 * 0.8) as usize,
            ChunkType::Configuration => base_tokens,
            _ => base_tokens,
        }
    }
}

impl ContextWindowManager {
    pub fn new(config: ContextWindowConfig, embedding_engine: Option<Arc<LocalEmbeddingEngine>>) -> Self {
        Self {
            config,
            chunks: Arc::new(RwLock::new(HashMap::new())),
            priority_queue: Arc::new(RwLock::new(BTreeMap::new())),
            file_chunks: Arc::new(RwLock::new(HashMap::new())),
            embedding_engine,
            access_history: Arc::new(RwLock::new(VecDeque::new())),
            token_estimator: TokenEstimator,
        }
    }

    /// Add content to the context window with intelligent chunking
    pub async fn add_content(&self, content: &str, source_file: Option<String>, chunk_type: ChunkType) -> Result<Vec<Uuid>> {
        let chunks = if self.config.adaptive_chunking {
            self.adaptive_chunk(content, &source_file, &chunk_type).await?
        } else {
            self.fixed_chunk(content, &source_file, &chunk_type).await?
        };

        let mut chunk_ids = Vec::new();
        
        // Scope the locks to avoid holding them across await
        {
            let mut chunks_map = self.chunks.write().unwrap();
            let mut file_chunks = self.file_chunks.write().unwrap();
            let mut priority_queue = self.priority_queue.write().unwrap();

            for chunk in chunks {
                let chunk_id = chunk.id;
                let priority = OrderedFloat(chunk.priority);
                
                // Add to file mapping
                if let Some(ref file) = chunk.source_file {
                    file_chunks.entry(file.clone()).or_default().push(chunk_id);
                }

                // Add to priority queue
                priority_queue.insert(priority, chunk_id);
                
                // Store chunk
                chunks_map.insert(chunk_id, chunk);
                chunk_ids.push(chunk_id);
            }
        }

        // Enforce size limits
        self.enforce_limits().await?;

        debug!("Added {} chunks to context window", chunk_ids.len());
        Ok(chunk_ids)
    }

    /// Get optimal context for a query
    pub async fn get_context_for_query(&self, query: &str, max_tokens: Option<usize>) -> Result<Vec<ContextChunk>> {
        let target_tokens = max_tokens.unwrap_or(self.config.max_tokens);
        
        // Generate query embedding for relevance scoring
        let query_embedding = if let Some(ref engine) = self.embedding_engine {
            Some(engine.encode_text(query).await?)
        } else {
            None
        };

        let chunks = self.chunks.read().unwrap();
        let mut scored_chunks: Vec<(f32, ContextChunk)> = Vec::new();

        // Score all chunks for relevance
        for chunk in chunks.values() {
            let relevance_score = if let (Some(ref q_emb), Some(ref c_emb)) = (&query_embedding, &chunk.embedding) {
                self.cosine_similarity(q_emb, c_emb)
            } else {
                self.text_similarity(query, &chunk.content)
            };

            if relevance_score >= self.config.relevance_threshold {
                let final_score = self.calculate_final_score(chunk, relevance_score);
                scored_chunks.push((final_score, chunk.clone()));
            }
        }
        drop(chunks); // Release the lock

        // Sort by score (highest first)
        scored_chunks.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Select chunks within token limit
        let mut selected_chunks = Vec::new();
        let mut total_tokens = 0;

        for (_score, chunk) in scored_chunks {
            let chunk_tokens = self.token_estimator.estimate_tokens(&chunk.content, &chunk.chunk_type);
            
            if total_tokens + chunk_tokens <= target_tokens {
                total_tokens += chunk_tokens;
                selected_chunks.push(chunk);
            } else {
                break;
            }
        }

        // Update access history
        self.update_access_history(&selected_chunks).await;

        info!("Selected {} chunks ({} tokens) for query", selected_chunks.len(), total_tokens);
        Ok(selected_chunks)
    }

    /// Get context for large codebase indexing
    pub async fn get_codebase_context(&self, file_patterns: &[String], priority_files: &[String]) -> Result<Vec<ContextChunk>> {
        let chunks = self.chunks.read().unwrap();
        let file_chunks = self.file_chunks.read().unwrap();
        let mut selected_chunks = Vec::new();
        let mut total_tokens = 0;

        // First, add priority files
        for priority_file in priority_files {
            if let Some(chunk_ids) = file_chunks.get(priority_file) {
                for &chunk_id in chunk_ids {
                    if let Some(chunk) = chunks.get(&chunk_id) {
                        let chunk_tokens = self.token_estimator.estimate_tokens(&chunk.content, &chunk.chunk_type);
                        if total_tokens + chunk_tokens <= self.config.max_tokens {
                            total_tokens += chunk_tokens;
                            selected_chunks.push(chunk.clone());
                        }
                    }
                }
            }
        }

        // Then add chunks matching patterns
        for pattern in file_patterns {
            for (file_path, chunk_ids) in file_chunks.iter() {
                if self.matches_pattern(file_path, pattern) {
                    for &chunk_id in chunk_ids {
                        if let Some(chunk) = chunks.get(&chunk_id) {
                            // Skip if already selected
                            if selected_chunks.iter().any(|c| c.id == chunk_id) {
                                continue;
                            }

                            let chunk_tokens = self.token_estimator.estimate_tokens(&chunk.content, &chunk.chunk_type);
                            if total_tokens + chunk_tokens <= self.config.max_tokens {
                                total_tokens += chunk_tokens;
                                selected_chunks.push(chunk.clone());
                            }
                        }
                    }
                }
            }
        }

        info!("Selected {} chunks ({} tokens) for codebase context", selected_chunks.len(), total_tokens);
        Ok(selected_chunks)
    }

    /// Adaptive chunking based on content structure
    async fn adaptive_chunk(&self, content: &str, source_file: &Option<String>, chunk_type: &ChunkType) -> Result<Vec<ContextChunk>> {
        let mut chunks = Vec::new();
        
        match chunk_type {
            ChunkType::Code => {
                chunks.extend(self.chunk_code(content, source_file).await?);
            },
            ChunkType::Documentation => {
                chunks.extend(self.chunk_documentation(content, source_file).await?);
            },
            _ => {
                chunks.extend(self.fixed_chunk(content, source_file, chunk_type).await?);
            }
        }

        Ok(chunks)
    }

    /// Chunk code content preserving logical boundaries
    async fn chunk_code(&self, content: &str, source_file: &Option<String>) -> Result<Vec<ContextChunk>> {
        let mut chunks = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut current_chunk = String::new();
        let mut chunk_start_line = 1;
        let mut current_line = 1;
        let mut brace_depth = 0;
        let mut in_function = false;

        for line in &lines {
            current_chunk.push_str(line);
            current_chunk.push('\n');

            // Track code structure
            brace_depth += line.matches('{').count() as i32;
            brace_depth -= line.matches('}').count() as i32;

            // Detect function boundaries
            if line.trim_start().starts_with("fn ") || line.trim_start().starts_with("function ") {
                in_function = true;
            }

            // Check if we should create a chunk
            let should_chunk = self.should_create_code_chunk(&current_chunk, brace_depth, in_function, current_line - chunk_start_line + 1);

            if should_chunk {
                let chunk = self.create_chunk(
                    current_chunk.clone(),
                    source_file.clone(),
                    Some(chunk_start_line),
                    Some(current_line),
                    ChunkType::Code,
                ).await?;
                
                chunks.push(chunk);
                
                // Start new chunk with overlap
                let overlap_lines = self.config.overlap_tokens / 10; // Rough estimate
                let overlap_start = current_line.saturating_sub(overlap_lines);
                if overlap_start < lines.len() {
                    current_chunk = lines[overlap_start..=current_line.min(lines.len() - 1)].join("\n");
                } else {
                    current_chunk = String::new();
                }
                chunk_start_line = overlap_start + 1;
            }

            if brace_depth == 0 {
                in_function = false;
            }

            current_line += 1;
        }

        // Add remaining content
        if !current_chunk.trim().is_empty() {
            let chunk = self.create_chunk(
                current_chunk,
                source_file.clone(),
                Some(chunk_start_line),
                Some(current_line - 1),
                ChunkType::Code,
            ).await?;
            chunks.push(chunk);
        }

        Ok(chunks)
    }

    /// Chunk documentation preserving section boundaries
    async fn chunk_documentation(&self, content: &str, source_file: &Option<String>) -> Result<Vec<ContextChunk>> {
        let mut chunks = Vec::new();
        let sections = self.split_by_headers(content);

        for (section_content, start_line, end_line) in sections {
            if section_content.trim().is_empty() {
                continue;
            }

            let chunk = self.create_chunk(
                section_content,
                source_file.clone(),
                Some(start_line),
                Some(end_line),
                ChunkType::Documentation,
            ).await?;
            
            chunks.push(chunk);
        }

        Ok(chunks)
    }

    /// Fixed-size chunking fallback
    async fn fixed_chunk(&self, content: &str, source_file: &Option<String>, chunk_type: &ChunkType) -> Result<Vec<ContextChunk>> {
        let mut chunks = Vec::new();
        let target_size = self.config.max_tokens / 4; // Rough character estimate
        let overlap_size = self.config.overlap_tokens / 4;

        let mut start = 0;
        let mut chunk_num = 0;

        while start < content.len() {
            let end = (start + target_size).min(content.len());
            let chunk_content = content[start..end].to_string();

            let chunk = self.create_chunk(
                chunk_content,
                source_file.clone(),
                None,
                None,
                chunk_type.clone(),
            ).await?;
            
            chunks.push(chunk);
            
            start = end.saturating_sub(overlap_size);
            chunk_num += 1;

            if chunk_num >= self.config.max_chunks_per_file {
                break;
            }
        }

        Ok(chunks)
    }

    /// Create a context chunk with metadata
    async fn create_chunk(
        &self,
        content: String,
        source_file: Option<String>,
        start_line: Option<usize>,
        end_line: Option<usize>,
        chunk_type: ChunkType,
    ) -> Result<ContextChunk> {
        let id = Uuid::new_v4();
        let priority = self.calculate_priority(&content, &chunk_type);
        
        // Generate embedding if engine is available
        let embedding = if let Some(ref engine) = self.embedding_engine {
            match engine.encode_text(&content).await {
                Ok(emb) => Some(emb),
                Err(e) => {
                    warn!("Failed to generate embedding for chunk {}: {}", id, e);
                    None
                }
            }
        } else {
            None
        };

        Ok(ContextChunk {
            id,
            content,
            source_file,
            start_line,
            end_line,
            chunk_type,
            priority,
            relevance_score: 0.0,
            last_accessed: Utc::now(),
            access_count: 0,
            embedding,
            metadata: HashMap::new(),
        })
    }

    // Helper methods

    fn should_create_code_chunk(&self, content: &str, brace_depth: i32, in_function: bool, line_count: usize) -> bool {
        let estimated_tokens = self.token_estimator.estimate_tokens(content, &ChunkType::Code);
        
        // Create chunk if:
        // 1. We've reached token limit
        // 2. We're at a logical boundary (end of function, balanced braces)
        // 3. We've exceeded max lines per chunk
        
        estimated_tokens >= self.config.max_tokens / 8 || // 1/8 of max tokens per chunk
        (brace_depth == 0 && !in_function && line_count > 10) ||
        line_count > 100
    }

    fn split_by_headers(&self, content: &str) -> Vec<(String, usize, usize)> {
        let mut sections = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut current_section = String::new();
        let mut section_start = 1;
        let mut current_line = 1;

        for line in lines {
            // Detect markdown headers
            if line.starts_with('#') || line.starts_with("===") || line.starts_with("---") {
                if !current_section.trim().is_empty() {
                    sections.push((current_section.clone(), section_start, current_line - 1));
                }
                current_section = String::new();
                section_start = current_line;
            }

            current_section.push_str(line);
            current_section.push('\n');
            current_line += 1;
        }

        // Add final section
        if !current_section.trim().is_empty() {
            sections.push((current_section, section_start, current_line - 1));
        }

        sections
    }

    fn calculate_priority(&self, content: &str, chunk_type: &ChunkType) -> f32 {
        let mut priority: f32 = match chunk_type {
            ChunkType::Function => 1.0,
            ChunkType::Class => 0.9,
            ChunkType::Code => 0.8,
            ChunkType::Documentation => 0.7,
            ChunkType::Test => 0.6,
            ChunkType::Configuration => 0.5,
            ChunkType::Comment => 0.3,
            _ => 0.4,
        };

        // Boost priority for important keywords
        if content.contains("TODO") || content.contains("FIXME") || content.contains("BUG") {
            priority += 0.2;
        }

        if content.contains("async") || content.contains("await") {
            priority += 0.1;
        }

        priority.min(1.0)
    }

    fn calculate_final_score(&self, chunk: &ContextChunk, relevance_score: f32) -> f32 {
        let recency_score = self.calculate_recency_score(chunk.last_accessed);
        let access_score = (chunk.access_count as f32).ln_1p() / 10.0; // Logarithmic access boost
        
        relevance_score * 0.6 + 
        chunk.priority * 0.2 + 
        recency_score * self.config.recency_weight + 
        access_score * 0.1
    }

    fn calculate_recency_score(&self, last_accessed: DateTime<Utc>) -> f32 {
        let hours_since_access = Utc::now().signed_duration_since(last_accessed).num_hours() as f32;
        (-hours_since_access / 24.0).exp() // Exponential decay over days
    }

    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot_product / (norm_a * norm_b)
        }
    }

    fn text_similarity(&self, query: &str, content: &str) -> f32 {
        // Simple text similarity based on common words
        let query_words: std::collections::HashSet<&str> = query.split_whitespace().collect();
        let content_words: std::collections::HashSet<&str> = content.split_whitespace().collect();
        
        let intersection = query_words.intersection(&content_words).count();
        let union = query_words.union(&content_words).count();
        
        if union == 0 {
            0.0
        } else {
            intersection as f32 / union as f32
        }
    }

    fn matches_pattern(&self, file_path: &str, pattern: &str) -> bool {
        // Simple glob-like pattern matching
        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                file_path.starts_with(parts[0]) && file_path.ends_with(parts[1])
            } else {
                false
            }
        } else {
            file_path.contains(pattern)
        }
    }

    async fn update_access_history(&self, chunks: &[ContextChunk]) {
        let mut history = self.access_history.write().unwrap();
        let mut chunks_map = self.chunks.write().unwrap();
        
        let now = Utc::now();
        
        for chunk in chunks {
            // Update access count and time
            if let Some(stored_chunk) = chunks_map.get_mut(&chunk.id) {
                stored_chunk.access_count += 1;
                stored_chunk.last_accessed = now;
            }
            
            // Add to history
            history.push_back((chunk.id, now));
            
            // Keep history size manageable
            if history.len() > 1000 {
                history.pop_front();
            }
        }
    }

    async fn enforce_limits(&self) -> Result<()> {
        let total_chunks = {
            let chunks = self.chunks.read().unwrap();
            chunks.len()
        };
        
        if total_chunks > 10000 { // Arbitrary limit
            self.evict_least_important_chunks(total_chunks - 8000).await?;
        }
        
        Ok(())
    }

    async fn evict_least_important_chunks(&self, count: usize) -> Result<()> {
        let chunks_to_evict: Vec<Uuid> = {
            let priority_queue = self.priority_queue.read().unwrap();
            priority_queue
                .iter()
                .take(count)
                .map(|(_, uuid)| *uuid)
                .collect()
        };

        let mut chunks = self.chunks.write().unwrap();
        let mut priority_queue = self.priority_queue.write().unwrap();
        let mut file_chunks = self.file_chunks.write().unwrap();

        for chunk_id in chunks_to_evict {
            if let Some(chunk) = chunks.remove(&chunk_id) {
                // Remove from file mapping
                if let Some(ref file) = chunk.source_file {
                    if let Some(file_chunk_list) = file_chunks.get_mut(file) {
                        file_chunk_list.retain(|&id| id != chunk_id);
                    }
                }
                
                // Remove from priority queue
                priority_queue.retain(|_, uuid| *uuid != chunk_id);
            }
        }

        debug!("Evicted {} chunks to enforce limits", count);
        Ok(())
    }

    /// Get statistics about the context window
    pub fn get_stats(&self) -> ContextWindowStats {
        let chunks = self.chunks.read().unwrap();
        let file_chunks = self.file_chunks.read().unwrap();
        
        let total_chunks = chunks.len();
        let total_files = file_chunks.len();
        let total_tokens: usize = chunks.values()
            .map(|c| self.token_estimator.estimate_tokens(&c.content, &c.chunk_type))
            .sum();

        let chunks_by_type: HashMap<ChunkType, usize> = chunks.values()
            .fold(HashMap::new(), |mut acc, chunk| {
                *acc.entry(chunk.chunk_type.clone()).or_insert(0) += 1;
                acc
            });

        ContextWindowStats {
            total_chunks,
            total_files,
            total_tokens,
            chunks_by_type,
            memory_usage: total_chunks * 1024, // Rough estimate
        }
    }
}

/// Context window statistics
#[derive(Debug)]
pub struct ContextWindowStats {
    pub total_chunks: usize,
    pub total_files: usize,
    pub total_tokens: usize,
    pub chunks_by_type: HashMap<ChunkType, usize>,
    pub memory_usage: usize,
} 
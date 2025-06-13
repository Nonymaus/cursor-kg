use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, info, error};
use uuid::Uuid;

use crate::graph::{KGNode, KGEdge, Episode, EpisodeSource};
use crate::graph::storage::GraphStorage;
use crate::context::{ContextWindowManager, ContextChunk, ChunkType, ContextWindowConfig};
use crate::nlp::{EntityExtractor, RelationshipExtractor};
use crate::embeddings::LocalEmbeddingEngine;

/// Configuration for codebase indexing (MCP compatible)
#[derive(Debug, Clone)]
pub struct IndexingConfig {
    pub languages: Option<Vec<String>>,
    pub include_patterns: Option<Vec<String>>,
    pub exclude_patterns: Option<Vec<String>>,
    pub max_file_size: usize,
    pub parallel_workers: usize,
    pub incremental: bool,
    pub extract_dependencies: bool,
    pub extract_symbols: bool,
    pub group_id: Option<String>,
}

impl Default for IndexingConfig {
    fn default() -> Self {
        Self {
            languages: None,
            include_patterns: None,
            exclude_patterns: Some(vec![
                "target/".to_string(), "node_modules/".to_string(), ".git/".to_string(),
                "build/".to_string(), "dist/".to_string(), "__pycache__/".to_string(),
                ".vscode/".to_string(), ".idea/".to_string(),
            ]),
            max_file_size: 1024 * 1024, // 1MB
            parallel_workers: 4,
            incremental: true,
            extract_dependencies: true,
            extract_symbols: true,
            group_id: None,
        }
    }
}

/// Result of indexing operation
#[derive(Debug, Clone)]
pub struct IndexingResult {
    pub files_processed: usize,
    pub symbols_extracted: usize,
    pub dependencies_mapped: usize,
    pub processing_time_ms: u64,
    pub languages_detected: Vec<String>,
    pub errors: Vec<String>,
}

/// Code search result
#[derive(Debug, Clone)]
pub struct CodeSearchResult {
    pub file_path: String,
    pub symbol_name: String,
    pub symbol_type: String,
    pub line_number: usize,
    pub column_number: usize,
    pub context_lines: Vec<String>,
    pub full_context: String,
    pub language: String,
    pub relevance_score: f32,
}

/// File dependency information for MCP
#[derive(Debug, Clone)]
pub struct FileDependency {
    pub target_file: String,
    pub dependency_type: String,
    pub line_number: usize,
    pub context: String,
    pub confidence: f32,
}

/// Codebase structure analysis result
#[derive(Debug, Clone)]
pub struct CodebaseAnalysis {
    pub total_files: usize,
    pub total_lines: usize,
    pub languages: Vec<String>,
    pub directory_structure: serde_json::Value,
    pub file_types: HashMap<String, usize>,
    pub complexity_metrics: HashMap<String, f32>,
    pub dependency_graph: serde_json::Value,
}

/// Configuration for codebase indexing
#[derive(Debug, Clone)]
pub struct CodebaseIndexerConfig {
    pub max_file_size: usize,
    pub supported_extensions: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub max_concurrent_files: usize,
    pub chunk_size: usize,
    pub enable_incremental: bool,
    pub enable_dependency_mapping: bool,
    pub enable_cross_file_analysis: bool,
}

impl Default for CodebaseIndexerConfig {
    fn default() -> Self {
        Self {
            max_file_size: 1024 * 1024, // 1MB
            supported_extensions: vec![
                "rs".to_string(), "py".to_string(), "js".to_string(), "ts".to_string(),
                "java".to_string(), "cpp".to_string(), "c".to_string(), "h".to_string(),
                "go".to_string(), "rb".to_string(), "php".to_string(), "cs".to_string(),
                "swift".to_string(), "kt".to_string(), "scala".to_string(), "clj".to_string(),
                "hs".to_string(), "ml".to_string(), "elm".to_string(), "dart".to_string(),
                "md".to_string(), "txt".to_string(), "json".to_string(), "yaml".to_string(),
                "toml".to_string(), "xml".to_string(), "html".to_string(), "css".to_string(),
            ],
            exclude_patterns: vec![
                "target/".to_string(), "node_modules/".to_string(), ".git/".to_string(),
                "build/".to_string(), "dist/".to_string(), "__pycache__/".to_string(),
                ".vscode/".to_string(), ".idea/".to_string(),
            ],
            max_concurrent_files: 10,
            chunk_size: 2048,
            enable_incremental: true,
            enable_dependency_mapping: true,
            enable_cross_file_analysis: true,
        }
    }
}

/// Result of indexing a single file
#[derive(Debug, Clone)]
pub struct FileIndexResult {
    pub file_path: PathBuf,
    pub nodes: Vec<KGNode>,
    pub edges: Vec<KGEdge>,
    pub episodes: Vec<Episode>,
    pub chunks: Vec<ContextChunk>,
    pub dependencies: Vec<Dependency>,
    pub metadata: FileMetadata,
}

/// File dependency information
#[derive(Debug, Clone)]
pub struct Dependency {
    pub source_file: PathBuf,
    pub target_file: PathBuf,
    pub dependency_type: DependencyType,
    pub line_number: Option<usize>,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone)]
pub enum DependencyType {
    Import,
    Include,
    Require,
    Use,
    Extends,
    Implements,
    References,
}

/// File metadata
#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub language: ProgrammingLanguage,
    pub lines_of_code: usize,
    pub complexity_score: f32,
    pub last_modified: std::time::SystemTime,
    pub file_size: u64,
    pub encoding: String,
}

#[derive(Debug, Clone)]
pub enum ProgrammingLanguage {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Java,
    Cpp,
    C,
    Go,
    Ruby,
    PHP,
    CSharp,
    Swift,
    Kotlin,
    Scala,
    Clojure,
    Haskell,
    OCaml,
    Elm,
    Dart,
    Markdown,
    Text,
    Json,
    Yaml,
    Toml,
    Xml,
    Html,
    Css,
    Unknown,
}

/// Indexing statistics
#[derive(Debug, Clone)]
pub struct IndexingStats {
    pub total_files: usize,
    pub processed_files: usize,
    pub skipped_files: usize,
    pub total_nodes: usize,
    pub total_edges: usize,
    pub total_episodes: usize,
    pub total_chunks: usize,
    pub processing_time: std::time::Duration,
    pub errors: Vec<String>,
}

/// Advanced codebase indexer
pub struct CodebaseIndexer {
    config: CodebaseIndexerConfig,
    context_manager: Arc<ContextWindowManager>,
    entity_extractor: Arc<EntityExtractor>,
    relationship_extractor: Arc<RelationshipExtractor>,
    embedding_engine: Option<Arc<LocalEmbeddingEngine>>,
    file_cache: Arc<RwLock<HashMap<PathBuf, FileIndexResult>>>,
    dependency_graph: Arc<RwLock<HashMap<PathBuf, Vec<Dependency>>>>,
    semaphore: Arc<Semaphore>,
}

impl CodebaseIndexer {
    pub fn new(
        config: CodebaseIndexerConfig,
        context_manager: Arc<ContextWindowManager>,
        entity_extractor: Arc<EntityExtractor>,
        relationship_extractor: Arc<RelationshipExtractor>,
        embedding_engine: Option<Arc<LocalEmbeddingEngine>>,
    ) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_files));
        
        Self {
            config,
            context_manager,
            entity_extractor,
            relationship_extractor,
            embedding_engine,
            file_cache: Arc::new(RwLock::new(HashMap::new())),
            dependency_graph: Arc::new(RwLock::new(HashMap::new())),
            semaphore,
        }
    }

    /// Index an entire codebase
    pub async fn index_codebase(&self, root_path: &Path) -> Result<IndexingStats> {
        let start_time = std::time::Instant::now();
        let mut stats = IndexingStats {
            total_files: 0,
            processed_files: 0,
            skipped_files: 0,
            total_nodes: 0,
            total_edges: 0,
            total_episodes: 0,
            total_chunks: 0,
            processing_time: std::time::Duration::default(),
            errors: Vec::new(),
        };

        // Discover all files
        let files = self.discover_files(root_path).await?;
        stats.total_files = files.len();

        info!("Discovered {} files for indexing", files.len());

        // Process files in parallel
        let mut handles = Vec::new();
        
        for file_path in files {
            let indexer = self.clone_for_task();
            let file_path_clone = file_path.clone();
            
            let handle = tokio::spawn(async move {
                let _permit = indexer.semaphore.acquire().await.unwrap();
                indexer.process_file(&file_path_clone).await
            });
            
            handles.push(handle);
        }

        // Collect results
        for handle in handles {
            match handle.await {
                Ok(Ok(file_result)) => {
                    stats.processed_files += 1;
                    let nodes_len = file_result.nodes.len();
                    let edges_len = file_result.edges.len();
                    let episodes_len = file_result.episodes.len();
                    
                    stats.total_nodes += nodes_len;
                    stats.total_edges += edges_len;
                    stats.total_episodes += episodes_len;
                    stats.total_chunks += file_result.chunks.len();

                    // Store in cache
                    let mut cache = self.file_cache.write().await;
                    cache.insert(file_result.file_path.clone(), file_result);
                }
                Ok(Err(e)) => {
                    stats.skipped_files += 1;
                    stats.errors.push(format!("Processing error: {}", e));
                }
                Err(e) => {
                    stats.skipped_files += 1;
                    stats.errors.push(format!("Task error: {}", e));
                }
            }
        }

        // Build dependency graph if enabled
        if self.config.enable_dependency_mapping {
            self.build_dependency_graph().await?;
        }

        // Perform cross-file analysis if enabled
        if self.config.enable_cross_file_analysis {
            self.perform_cross_file_analysis().await?;
        }

        stats.processing_time = start_time.elapsed();
        
        info!("Indexing completed: {} files processed, {} nodes, {} edges, {} episodes", 
              stats.processed_files, stats.total_nodes, stats.total_edges, stats.total_episodes);

        Ok(stats)
    }

    /// Process a single file
    async fn process_file(&self, file_path: &Path) -> Result<FileIndexResult> {
        debug!("Processing file: {:?}", file_path);

        // Check if file should be processed
        if !self.should_process_file(file_path) {
            return Err(anyhow::anyhow!("File excluded by configuration"));
        }

        // Read file content
        let content = tokio::fs::read_to_string(file_path).await?;
        
        // Check file size
        if content.len() > self.config.max_file_size {
            return Err(anyhow::anyhow!("File too large: {} bytes", content.len()));
        }

        // Determine language and metadata
        let language = self.detect_language(file_path);
        let metadata = self.extract_file_metadata(file_path, &content).await?;

        // Create episode source
        let episode_source = match language {
            ProgrammingLanguage::Json => EpisodeSource::Json,
            _ => EpisodeSource::Text,
        };

        // Extract entities and relationships
        let entities = self.entity_extractor.extract_entities(&content, &episode_source, &file_path.to_string_lossy()).await?;
        let relationships = self.relationship_extractor.extract_relationships_between_entities(&entities, &content, &file_path.to_string_lossy()).await?;

        // Create knowledge graph nodes and edges
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        // Convert entities to nodes
        for entity in entities {
            let node = KGNode::new(
                entity.name.clone(),
                entity.entity_type.clone(),
                entity.summary.clone(),
                Some(file_path.to_string_lossy().to_string()),
            );
            nodes.push(node);
        }

        // Convert relationships to edges
        for relationship in relationships {
            // Find source and target nodes by name (simplified approach)
            if let (Some(source_node), Some(target_node)) = (
                nodes.iter().find(|n| n.name == relationship.source_entity),
                nodes.iter().find(|n| n.name == relationship.target_entity),
            ) {
                let edge = KGEdge::new(
                    source_node.uuid,
                    target_node.uuid,
                    relationship.relation_type.clone(),
                    relationship.summary.clone(),
                    relationship.weight,
                    Some(file_path.to_string_lossy().to_string()),
                );
                edges.push(edge);
            }
        }

        // Create episodes for significant code blocks
        let episodes = self.create_episodes_from_content(&content, file_path).await?;

        // Create context chunks
        let chunk_type = self.determine_chunk_type(&language);
        let chunk_ids = self.context_manager.add_content(
            &content,
            Some(file_path.to_string_lossy().to_string()),
            chunk_type,
        ).await?;

        // Get the actual chunks
        let chunks = self.get_chunks_by_ids(&chunk_ids).await?;

        // Extract dependencies
        let dependencies = self.extract_dependencies(&content, file_path, &language).await?;

        Ok(FileIndexResult {
            file_path: file_path.to_path_buf(),
            nodes,
            edges,
            episodes,
            chunks,
            dependencies,
            metadata,
        })
    }

    /// Create a clone of the indexer for parallel processing
    fn clone_for_task(&self) -> Self {
        Self {
            config: self.config.clone(),
            context_manager: Arc::clone(&self.context_manager),
            entity_extractor: Arc::clone(&self.entity_extractor),
            relationship_extractor: Arc::clone(&self.relationship_extractor),
            embedding_engine: self.embedding_engine.clone(),
            file_cache: Arc::clone(&self.file_cache),
            dependency_graph: Arc::clone(&self.dependency_graph),
            semaphore: Arc::clone(&self.semaphore),
        }
    }

    // Helper methods

    async fn discover_files(&self, root_path: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        let mut stack = vec![root_path.to_path_buf()];

        while let Some(current_path) = stack.pop() {
            if current_path.is_dir() {
                // Check if directory should be excluded
                if self.should_exclude_directory(&current_path) {
                    continue;
                }

                let mut entries = tokio::fs::read_dir(&current_path).await?;
                while let Some(entry) = entries.next_entry().await? {
                    stack.push(entry.path());
                }
            } else if current_path.is_file() {
                if self.should_process_file(&current_path) {
                    files.push(current_path);
                }
            }
        }

        Ok(files)
    }

    fn should_process_file(&self, file_path: &Path) -> bool {
        // Check extension
        if let Some(extension) = file_path.extension() {
            if let Some(ext_str) = extension.to_str() {
                if !self.config.supported_extensions.contains(&ext_str.to_lowercase()) {
                    return false;
                }
            }
        }

        // Check exclude patterns
        let path_str = file_path.to_string_lossy();
        for pattern in &self.config.exclude_patterns {
            if path_str.contains(pattern) {
                return false;
            }
        }

        true
    }

    fn should_exclude_directory(&self, dir_path: &Path) -> bool {
        let path_str = dir_path.to_string_lossy();
        for pattern in &self.config.exclude_patterns {
            if path_str.contains(pattern) {
                return true;
            }
        }
        false
    }

    fn detect_language(&self, file_path: &Path) -> ProgrammingLanguage {
        if let Some(extension) = file_path.extension() {
            if let Some(ext_str) = extension.to_str() {
                match ext_str.to_lowercase().as_str() {
                    "rs" => ProgrammingLanguage::Rust,
                    "py" => ProgrammingLanguage::Python,
                    "js" => ProgrammingLanguage::JavaScript,
                    "ts" => ProgrammingLanguage::TypeScript,
                    "java" => ProgrammingLanguage::Java,
                    "cpp" | "cc" | "cxx" => ProgrammingLanguage::Cpp,
                    "c" => ProgrammingLanguage::C,
                    "go" => ProgrammingLanguage::Go,
                    "rb" => ProgrammingLanguage::Ruby,
                    "php" => ProgrammingLanguage::PHP,
                    "cs" => ProgrammingLanguage::CSharp,
                    "swift" => ProgrammingLanguage::Swift,
                    "kt" => ProgrammingLanguage::Kotlin,
                    "scala" => ProgrammingLanguage::Scala,
                    "clj" => ProgrammingLanguage::Clojure,
                    "hs" => ProgrammingLanguage::Haskell,
                    "ml" => ProgrammingLanguage::OCaml,
                    "elm" => ProgrammingLanguage::Elm,
                    "dart" => ProgrammingLanguage::Dart,
                    "md" => ProgrammingLanguage::Markdown,
                    "txt" => ProgrammingLanguage::Text,
                    "json" => ProgrammingLanguage::Json,
                    "yaml" | "yml" => ProgrammingLanguage::Yaml,
                    "toml" => ProgrammingLanguage::Toml,
                    "xml" => ProgrammingLanguage::Xml,
                    "html" | "htm" => ProgrammingLanguage::Html,
                    "css" => ProgrammingLanguage::Css,
                    _ => ProgrammingLanguage::Unknown,
                }
            } else {
                ProgrammingLanguage::Unknown
            }
        } else {
            ProgrammingLanguage::Unknown
        }
    }

    async fn extract_file_metadata(&self, file_path: &Path, content: &str) -> Result<FileMetadata> {
        let metadata = tokio::fs::metadata(file_path).await?;
        let language = self.detect_language(file_path);
        
        let lines_of_code = content.lines().count();
        let complexity_score = self.calculate_complexity_score(content, &language);

        Ok(FileMetadata {
            language,
            lines_of_code,
            complexity_score,
            last_modified: metadata.modified()?,
            file_size: metadata.len(),
            encoding: "UTF-8".to_string(), // Simplified
        })
    }

    fn calculate_complexity_score(&self, content: &str, language: &ProgrammingLanguage) -> f32 {
        let mut score = 0.0;
        
        // Basic complexity indicators
        let lines = content.lines().count() as f32;
        score += lines * 0.1;

        // Language-specific complexity
        match language {
            ProgrammingLanguage::Rust | ProgrammingLanguage::Cpp => {
                score += content.matches("unsafe").count() as f32 * 2.0;
                score += content.matches("impl").count() as f32 * 1.5;
            }
            ProgrammingLanguage::JavaScript | ProgrammingLanguage::TypeScript => {
                score += content.matches("async").count() as f32 * 1.5;
                score += content.matches("Promise").count() as f32 * 1.0;
            }
            _ => {}
        }

        // Control flow complexity
        score += content.matches("if").count() as f32 * 1.0;
        score += content.matches("for").count() as f32 * 1.5;
        score += content.matches("while").count() as f32 * 1.5;
        score += content.matches("match").count() as f32 * 2.0;

        score
    }

    async fn create_episodes_from_content(&self, content: &str, file_path: &Path) -> Result<Vec<Episode>> {
        let mut episodes = Vec::new();

        // Create episodes for functions, classes, etc.
        let significant_blocks = self.extract_significant_code_blocks(content);
        
        for block in significant_blocks {
            let embedding = if let Some(ref engine) = self.embedding_engine {
                Some(engine.encode_text(&block.content).await?)
            } else {
                None
            };

            let mut episode = Episode::new(
                format!("{}_{}", file_path.file_name().unwrap_or_default().to_string_lossy(), block.block_type),
                block.content,
                EpisodeSource::Text,
                file_path.to_string_lossy().to_string(),
                Some(file_path.to_string_lossy().to_string()),
            );
            
            if let Some(emb) = embedding {
                episode.set_embedding(emb);
            }
            
            episodes.push(episode);
        }

        Ok(episodes)
    }

    fn extract_significant_code_blocks(&self, content: &str) -> Vec<CodeBlock> {
        let mut blocks = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        
        let mut current_block = String::new();
        let mut in_function = false;
        let mut brace_count = 0;

        for line in lines {
            current_block.push_str(line);
            current_block.push('\n');

            // Simple function detection (works for many languages)
            if line.trim_start().starts_with("fn ") || 
               line.trim_start().starts_with("function ") ||
               line.trim_start().starts_with("def ") ||
               line.trim_start().starts_with("class ") {
                in_function = true;
            }

            brace_count += line.matches('{').count() as i32;
            brace_count -= line.matches('}').count() as i32;

            if in_function && brace_count == 0 && !current_block.trim().is_empty() {
                blocks.push(CodeBlock {
                    content: current_block.clone(),
                    block_type: "function".to_string(),
                });
                current_block.clear();
                in_function = false;
            }
        }

        blocks
    }

    fn determine_chunk_type(&self, language: &ProgrammingLanguage) -> ChunkType {
        match language {
            ProgrammingLanguage::Markdown => ChunkType::Documentation,
            ProgrammingLanguage::Text => ChunkType::Documentation,
            ProgrammingLanguage::Json | ProgrammingLanguage::Yaml | ProgrammingLanguage::Toml => ChunkType::Configuration,
            _ => ChunkType::Code,
        }
    }

    async fn get_chunks_by_ids(&self, _chunk_ids: &[Uuid]) -> Result<Vec<ContextChunk>> {
        // This would typically query the context manager for the chunks
        // For now, return empty vector as placeholder
        Ok(Vec::new())
    }

    async fn extract_dependencies(&self, content: &str, file_path: &Path, language: &ProgrammingLanguage) -> Result<Vec<Dependency>> {
        let mut dependencies = Vec::new();

        match language {
            ProgrammingLanguage::Rust => {
                dependencies.extend(self.extract_rust_dependencies(content, file_path));
            }
            ProgrammingLanguage::Python => {
                dependencies.extend(self.extract_python_dependencies(content, file_path));
            }
            ProgrammingLanguage::JavaScript | ProgrammingLanguage::TypeScript => {
                dependencies.extend(self.extract_js_dependencies(content, file_path));
            }
            _ => {
                // Generic import detection
                dependencies.extend(self.extract_generic_dependencies(content, file_path));
            }
        }

        Ok(dependencies)
    }

    fn extract_rust_dependencies(&self, content: &str, file_path: &Path) -> Vec<Dependency> {
        let mut dependencies = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            
            if trimmed.starts_with("use ") {
                if let Some(module) = self.extract_rust_module_path(trimmed) {
                    dependencies.push(Dependency {
                        source_file: file_path.to_path_buf(),
                        target_file: PathBuf::from(module),
                        dependency_type: DependencyType::Use,
                        line_number: Some(line_num + 1),
                        symbol: None,
                    });
                }
            }
        }

        dependencies
    }

    fn extract_python_dependencies(&self, content: &str, file_path: &Path) -> Vec<Dependency> {
        let mut dependencies = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            
            if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
                if let Some(module) = self.extract_python_module_path(trimmed) {
                    dependencies.push(Dependency {
                        source_file: file_path.to_path_buf(),
                        target_file: PathBuf::from(module),
                        dependency_type: DependencyType::Import,
                        line_number: Some(line_num + 1),
                        symbol: None,
                    });
                }
            }
        }

        dependencies
    }

    fn extract_js_dependencies(&self, content: &str, file_path: &Path) -> Vec<Dependency> {
        let mut dependencies = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            
            if trimmed.starts_with("import ") || trimmed.contains("require(") {
                if let Some(module) = self.extract_js_module_path(trimmed) {
                    dependencies.push(Dependency {
                        source_file: file_path.to_path_buf(),
                        target_file: PathBuf::from(module),
                        dependency_type: DependencyType::Import,
                        line_number: Some(line_num + 1),
                        symbol: None,
                    });
                }
            }
        }

        dependencies
    }

    fn extract_generic_dependencies(&self, content: &str, file_path: &Path) -> Vec<Dependency> {
        let mut dependencies = Vec::new();

        // Generic patterns for includes/imports
        let patterns = ["#include", "import", "require", "use"];
        
        for (line_num, line) in content.lines().enumerate() {
            for pattern in &patterns {
                if line.contains(pattern) {
                    // Extract quoted strings as potential dependencies
                    if let Some(dep_path) = self.extract_quoted_string(line) {
                        dependencies.push(Dependency {
                            source_file: file_path.to_path_buf(),
                            target_file: PathBuf::from(dep_path),
                            dependency_type: DependencyType::References,
                            line_number: Some(line_num + 1),
                            symbol: None,
                        });
                    }
                }
            }
        }

        dependencies
    }

    // Helper methods for dependency extraction

    fn extract_rust_module_path(&self, line: &str) -> Option<String> {
        // Extract module path from "use path::to::module;"
        if let Some(start) = line.find("use ") {
            let rest = &line[start + 4..];
            if let Some(end) = rest.find(';') {
                Some(rest[..end].trim().to_string())
            } else {
                Some(rest.trim().to_string())
            }
        } else {
            None
        }
    }

    fn extract_python_module_path(&self, line: &str) -> Option<String> {
        // Extract module from "import module" or "from module import ..."
        if line.starts_with("import ") {
            Some(line[7..].split_whitespace().next()?.to_string())
        } else if line.starts_with("from ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                Some(parts[1].to_string())
            } else {
                None
            }
        } else {
            None
        }
    }

    fn extract_js_module_path(&self, line: &str) -> Option<String> {
        // Extract from import statements or require calls
        self.extract_quoted_string(line)
    }

    fn extract_quoted_string(&self, line: &str) -> Option<String> {
        // Extract content between quotes
        if let Some(start) = line.find('"') {
            if let Some(end) = line[start + 1..].find('"') {
                return Some(line[start + 1..start + 1 + end].to_string());
            }
        }
        if let Some(start) = line.find('\'') {
            if let Some(end) = line[start + 1..].find('\'') {
                return Some(line[start + 1..start + 1 + end].to_string());
            }
        }
        None
    }

    async fn build_dependency_graph(&self) -> Result<()> {
        let cache = self.file_cache.read().await;
        let mut dep_graph = self.dependency_graph.write().await;

        for (file_path, result) in cache.iter() {
            dep_graph.insert(file_path.clone(), result.dependencies.clone());
        }

        debug!("Built dependency graph with {} files", dep_graph.len());
        Ok(())
    }

    async fn perform_cross_file_analysis(&self) -> Result<()> {
        // Placeholder for cross-file analysis
        // This would analyze relationships between files, detect circular dependencies, etc.
        debug!("Performing cross-file analysis");
        Ok(())
    }

    /// Get indexing statistics
    pub async fn get_stats(&self) -> IndexingStats {
        let cache = self.file_cache.read().await;
        let mut stats = IndexingStats {
            total_files: cache.len(),
            processed_files: cache.len(),
            skipped_files: 0,
            total_nodes: 0,
            total_edges: 0,
            total_episodes: 0,
            total_chunks: 0,
            processing_time: std::time::Duration::default(),
            errors: Vec::new(),
        };

        for result in cache.values() {
            stats.total_nodes += result.nodes.len();
            stats.total_edges += result.edges.len();
            stats.total_episodes += result.episodes.len();
            stats.total_chunks += result.chunks.len();
        }

        stats
    }

    // MCP-compatible methods

    /// Create a new indexer with MCP-compatible configuration
    pub fn new_with_mcp_config(path: String, config: IndexingConfig) -> Self {
        use crate::context::{ContextWindowManager, ContextWindowConfig};
        
        // Create default components (simplified for MCP usage)
        let context_config = ContextWindowConfig {
            max_tokens: 128000,
            overlap_tokens: 256,
            priority_boost: 1.5,
            recency_weight: 1.2,
            relevance_threshold: 0.7,
            max_chunks_per_file: 50,
            adaptive_chunking: true,
            preserve_code_blocks: true,
        };
        let context_manager = Arc::new(ContextWindowManager::new(context_config, None));
        let entity_extractor = Arc::new(EntityExtractor::new(Default::default(), None).unwrap());
        let relationship_extractor = Arc::new(RelationshipExtractor::new(Default::default(), None).unwrap());
        
        // Convert MCP config to internal config
        let internal_config = CodebaseIndexerConfig {
            max_file_size: config.max_file_size,
            supported_extensions: Self::get_supported_extensions(&config.languages),
            exclude_patterns: config.exclude_patterns.unwrap_or_default(),
            max_concurrent_files: config.parallel_workers,
            chunk_size: 2048,
            enable_incremental: config.incremental,
            enable_dependency_mapping: config.extract_dependencies,
            enable_cross_file_analysis: config.extract_symbols,
        };

        let semaphore = Arc::new(Semaphore::new(internal_config.max_concurrent_files));
        
        Self {
            config: internal_config,
            context_manager,
            entity_extractor,
            relationship_extractor,
            embedding_engine: None,
            file_cache: Arc::new(RwLock::new(HashMap::new())),
            dependency_graph: Arc::new(RwLock::new(HashMap::new())),
            semaphore,
        }
    }

    /// Index codebase with MCP-compatible interface
    pub async fn index_codebase_mcp(
        &self,
        path: &str,
        storage: Arc<GraphStorage>,
        embedding_engine: Arc<LocalEmbeddingEngine>,
    ) -> Result<IndexingResult> {
        let start_time = std::time::Instant::now();
        
        // Use the existing index_codebase method
        let path_buf = std::path::Path::new(path);
        let stats = self.index_codebase(path_buf).await?;
        
        // Convert IndexingStats to IndexingResult
        Ok(IndexingResult {
            files_processed: stats.processed_files,
            symbols_extracted: stats.total_nodes, // Use nodes as symbols
            dependencies_mapped: stats.total_edges, // Use edges as dependencies
            processing_time_ms: stats.processing_time.as_millis() as u64,
            languages_detected: vec!["rust".to_string()], // Simplified
            errors: stats.errors,
        })
    }

    /// Search code with MCP-compatible interface
    pub async fn search_code_mcp(
        &self,
        _query: &str,
        _symbol_type: &str,
        _context_lines: usize,
        _max_results: usize,
        _storage: Arc<GraphStorage>,
    ) -> Result<Vec<CodeSearchResult>> {
        // Placeholder implementation
        // In a full implementation, this would search through indexed code
        Ok(vec![])
    }

    /// Get file dependencies with MCP-compatible interface
    pub async fn get_file_dependencies_mcp(
        &self,
        _file_path: &str,
        _storage: Arc<GraphStorage>,
    ) -> Result<Vec<FileDependency>> {
        // Placeholder implementation
        // In a full implementation, this would return actual dependencies
        Ok(vec![])
    }

    /// Analyze codebase structure with MCP-compatible interface
    pub async fn analyze_codebase_structure_mcp(
        &self,
        _storage: Arc<GraphStorage>,
    ) -> Result<CodebaseAnalysis> {
        // Placeholder implementation
        // In a full implementation, this would analyze the codebase structure
        Ok(CodebaseAnalysis {
            total_files: 0,
            total_lines: 0,
            languages: vec!["rust".to_string()],
            directory_structure: serde_json::json!({}),
            file_types: HashMap::new(),
            complexity_metrics: HashMap::new(),
            dependency_graph: serde_json::json!({}),
        })
    }

    /// Get supported extensions based on language filter
    fn get_supported_extensions(languages: &Option<Vec<String>>) -> Vec<String> {
        let all_extensions = vec![
            "rs".to_string(), "py".to_string(), "js".to_string(), "ts".to_string(),
            "java".to_string(), "cpp".to_string(), "c".to_string(), "h".to_string(),
            "go".to_string(), "rb".to_string(), "php".to_string(), "cs".to_string(),
            "swift".to_string(), "kt".to_string(), "scala".to_string(), "clj".to_string(),
            "hs".to_string(), "ml".to_string(), "elm".to_string(), "dart".to_string(),
            "md".to_string(), "txt".to_string(), "json".to_string(), "yaml".to_string(),
            "toml".to_string(), "xml".to_string(), "html".to_string(), "css".to_string(),
        ];

        if let Some(langs) = languages {
            let mut extensions = Vec::new();
            for lang in langs {
                match lang.to_lowercase().as_str() {
                    "rust" => extensions.push("rs".to_string()),
                    "python" => extensions.push("py".to_string()),
                    "javascript" => extensions.extend(vec!["js".to_string(), "jsx".to_string()]),
                    "typescript" => extensions.extend(vec!["ts".to_string(), "tsx".to_string()]),
                    "java" => extensions.push("java".to_string()),
                    "cpp" | "c++" => extensions.extend(vec!["cpp".to_string(), "cc".to_string(), "cxx".to_string()]),
                    "c" => extensions.extend(vec!["c".to_string(), "h".to_string()]),
                    "go" => extensions.push("go".to_string()),
                    "ruby" => extensions.push("rb".to_string()),
                    "php" => extensions.push("php".to_string()),
                    "csharp" | "c#" => extensions.push("cs".to_string()),
                    "swift" => extensions.push("swift".to_string()),
                    "kotlin" => extensions.push("kt".to_string()),
                    "scala" => extensions.push("scala".to_string()),
                    "clojure" => extensions.push("clj".to_string()),
                    "haskell" => extensions.push("hs".to_string()),
                    "ocaml" => extensions.push("ml".to_string()),
                    "elm" => extensions.push("elm".to_string()),
                    "dart" => extensions.push("dart".to_string()),
                    _ => {}
                }
            }
            extensions
        } else {
            all_extensions
        }
    }
}

#[derive(Debug, Clone)]
struct CodeBlock {
    content: String,
    block_type: String,
} 
pub mod batch_processor;
pub mod models;
pub mod onnx_runtime;

use anyhow::Result;
use tracing::debug;

pub use models::ModelManager;
pub use onnx_runtime::OnnxEmbeddingEngine;
pub use batch_processor::BatchProcessor;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::path::PathBuf;
use crate::config::ServerConfig;

/// Local embedding engine interface
pub trait EmbeddingEngine {
    fn encode(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    fn encode_single(&self, text: &str) -> Result<Vec<f32>>;
    fn dimensions(&self) -> usize;
    fn model_name(&self) -> &str;
}

/// High-performance local embedding engine
#[derive(Clone)]
pub struct LocalEmbeddingEngine {
    model_manager: ModelManager,
    onnx_engine: Arc<tokio::sync::RwLock<Option<OnnxEmbeddingEngine>>>,
    batch_processor: BatchProcessor,
    current_model: Arc<tokio::sync::RwLock<Option<String>>>,
    config: ServerConfig,
    is_initialized: Arc<AtomicBool>,
    initializing_lock: Arc<tokio::sync::Mutex<()>>,
}

impl LocalEmbeddingEngine {
    pub fn new(config: ServerConfig) -> Result<Self> {
        let models_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("kg-mcp-server")
            .join("models");

        let model_manager = ModelManager::new(models_dir);
        let batch_processor = BatchProcessor::new(config.embeddings.batch_size);

        Ok(Self {
            model_manager,
            onnx_engine: Arc::new(tokio::sync::RwLock::new(None)),
            batch_processor,
            current_model: Arc::new(tokio::sync::RwLock::new(None)),
            config,
            is_initialized: Arc::new(AtomicBool::new(false)),
            initializing_lock: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    /// Initialize the embedding engine with a specific model
    pub async fn initialize(&self, model_name: &str) -> Result<()> {
        let _lock = self.initializing_lock.lock().await;

        if self.is_initialized.load(Ordering::SeqCst) {
            if let Some(current) = self.current_model.read().await.as_deref() {
                if current == model_name {
                    debug!("Model {} already initialized.", model_name);
                    return Ok(());
                }
            }
        }

        debug!("🚀 Initializing LocalEmbeddingEngine with model: {}", model_name);

        // Ensure model is available
        let model_dir = self.model_manager.ensure_model_available(model_name).await?;
        let model_path = self.model_manager.get_model_path(model_name);

        // Get model dimensions
        let dimensions = crate::config::defaults::get_model_dimensions(model_name)
            .ok_or_else(|| anyhow::anyhow!("Unknown model dimensions for: {}", model_name))?;

        // Initialize ONNX engine
        debug!("🔄 Loading ONNX model from: {}", model_path.display());
        let onnx_engine = OnnxEmbeddingEngine::new(
            model_path.to_str().unwrap(),
            dimensions
        )?;

        *self.onnx_engine.write().await = Some(onnx_engine);
        *self.current_model.write().await = Some(model_name.to_string());
        self.is_initialized.store(true, Ordering::SeqCst);

        debug!("✅ LocalEmbeddingEngine initialized successfully");
        self.print_stats().await;

        Ok(())
    }

    /// Encode texts using the local embedding engine
    pub async fn encode_texts(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if !self.is_initialized.load(Ordering::SeqCst) {
            // Wait for initialization to complete
            let _lock = self.initializing_lock.lock().await;
        }

        let engine_guard = self.onnx_engine.read().await;
        let engine = engine_guard.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Embedding engine not initialized even after waiting"))?;

        self.batch_processor.process_with_engine(texts.to_vec(), engine).await
    }

    /// Encode a single text
    pub async fn encode_text(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self.encode_texts(&[text.to_string()]).await?;
        embeddings.into_iter().next()
            .ok_or_else(|| anyhow::anyhow!("No embedding generated"))
    }

    /// Get available models
    pub fn list_available_models(&self) -> &[&str] {
        crate::config::defaults::SUPPORTED_MODELS
    }

    /// Get downloaded models
    pub fn list_downloaded_models(&self) -> Result<Vec<String>> {
        self.model_manager.list_downloaded_models()
    }

    /// Download a model
    pub async fn download_model(&self, model_name: &str) -> Result<()> {
        self.model_manager.download_model(model_name).await
    }

    /// Check if model exists locally
    pub fn model_exists(&self, model_name: &str) -> bool {
        self.model_manager.model_exists(model_name)
    }

    /// Get current model name
    pub async fn current_model(&self) -> Option<String> {
        self.current_model.read().await.clone()
    }

    /// Get model dimensions
    pub async fn dimensions(&self) -> Option<usize> {
        self.current_model.read().await.as_ref()
            .and_then(|name| crate::config::defaults::get_model_dimensions(name))
    }

    /// Clear embedding cache
    pub async fn clear_cache(&self) {
        self.batch_processor.clear_cache().await;
        if let Some(engine) = self.onnx_engine.read().await.as_ref() {
            let _ = engine.clear_cache().await;
        }
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> Result<CacheStats> {
        let (used, capacity) = self.batch_processor.get_cache_stats().await;
        let onnx_cache_size = if let Some(engine) = self.onnx_engine.read().await.as_ref() {
            engine.cache_size().await
        } else {
            0
        };

        Ok(CacheStats {
            batch_cache_used: used,
            batch_cache_capacity: capacity,
            onnx_cache_size,
        })
    }

    /// Precompute embeddings for common queries
    pub async fn warmup(&self, common_queries: Vec<String>) -> Result<()> {
        debug!("🔥 Warming up embedding engine...");
        self.batch_processor.warmup_cache(common_queries).await?;
        debug!("✅ Warmup complete");
        Ok(())
    }

    /// Print current statistics
    pub async fn print_stats(&self) {
        let stats = self.get_cache_stats().await.unwrap_or_default();
        debug!("📊 Embedding Engine Stats:");
        debug!("  Model: {}", self.current_model().await.unwrap_or_else(|| "None".to_string()));
        debug!("  Dimensions: {}", self.dimensions().await.unwrap_or(0));
        debug!("  Batch Cache: {}/{}", stats.batch_cache_used, stats.batch_cache_capacity);
        debug!("  ONNX Cache: {}", stats.onnx_cache_size);
    }

    /// Cleanup incomplete downloads
    pub async fn cleanup(&self) -> Result<()> {
        self.model_manager.cleanup_incomplete_downloads().await
    }

    /// Switch to a different model
    pub async fn switch_model(&mut self, model_name: &str) -> Result<()> {
        if self.current_model.read().await.as_deref() == Some(model_name) {
            return Ok(());
        }

        debug!("🔄 Switching to model: {}", model_name);
        self.is_initialized.store(false, Ordering::SeqCst);
        self.initialize(model_name).await
    }

    /// Get embedding similarity between two texts
    pub async fn similarity(&self, text1: &str, text2: &str) -> Result<f32> {
        let embeddings = self.encode_texts(&[text1.to_string(), text2.to_string()]).await?;
        
        if embeddings.len() != 2 {
            return Err(anyhow::anyhow!("Failed to generate embeddings for both texts"));
        }

        Ok(cosine_similarity(&embeddings[0], &embeddings[1]))
    }

    /// Get semantic search results
    pub async fn semantic_search(&self, query: &str, candidates: &[String], top_k: usize) -> Result<Vec<(usize, f32)>> {
        let query_embedding = self.encode_text(query).await?;
        let candidate_embeddings = self.encode_texts(candidates).await?;

        let mut similarities: Vec<(usize, f32)> = candidate_embeddings
            .iter()
            .enumerate()
            .map(|(i, embedding)| (i, cosine_similarity(&query_embedding, embedding)))
            .collect();

        // Sort by similarity (highest first)
        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        // Return top_k results
        similarities.truncate(top_k);
        Ok(similarities)
    }
}

impl EmbeddingEngine for LocalEmbeddingEngine {
    fn encode(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.encode_texts(texts))
    }

    fn encode_single(&self, text: &str) -> Result<Vec<f32>> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.encode_text(text))
    }

    fn dimensions(&self) -> usize {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(_) => return 0,
        };
        rt.block_on(self.dimensions()).unwrap_or(0)
    }

    fn model_name(&self) -> &str {
        // This implementation is tricky with async, returning a static string for now.
        // A better solution would involve making the trait async.
        "dynamic"
    }
}

#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub batch_cache_used: usize,
    pub batch_cache_capacity: usize,
    pub onnx_cache_size: usize,
}

/// Calculate cosine similarity between two vectors
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
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

/// Calculate euclidean distance between two vectors
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return f32::INFINITY;
    }

    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
} 
use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::debug;
use tokio::sync::RwLock;
use std::collections::HashMap;
use crate::embeddings::EmbeddingEngine;

#[derive(Clone)]
pub struct OnnxEmbeddingEngine {
    model_path: String,
    dimensions: usize,
    model_name: String,
    max_sequence_length: usize,
    batch_size: usize,
    cache: Arc<RwLock<HashMap<String, Vec<f32>>>>,
}

impl OnnxEmbeddingEngine {
    pub fn new(model_path: &str, dimensions: usize) -> Result<Self> {
        debug!("🔄 Initializing ONNX Runtime embedding engine...");
        
        // For now, we'll create a placeholder implementation
        // In production, this would initialize the actual ONNX Runtime
        
        let model_name = std::path::Path::new(model_path)
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        debug!("✅ ONNX Runtime session created successfully (placeholder)");

        Ok(Self {
            model_path: model_path.to_string(),
            dimensions,
            model_name,
            max_sequence_length: 512,
            batch_size: 32,
            cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub async fn encode_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Check cache first
        let mut embeddings = Vec::with_capacity(texts.len());
        let mut uncached_texts = Vec::new();
        let mut uncached_indices = Vec::new();

        {
            let cache = self.cache.read().await;
            for (i, text) in texts.iter().enumerate() {
                if let Some(embedding) = cache.get(text) {
                    embeddings.push(Some(embedding.clone()));
                } else {
                    embeddings.push(None);
                    uncached_texts.push(text.clone());
                    uncached_indices.push(i);
                }
            }
        }

        // Process uncached texts
        if !uncached_texts.is_empty() {
            let new_embeddings = self.generate_embeddings(&uncached_texts).await?;
            
            // Update cache and results
            {
                let mut cache = self.cache.write().await;
                for (text, embedding) in uncached_texts.iter().zip(new_embeddings.iter()) {
                    cache.insert(text.clone(), embedding.clone());
                }
            }

            // Fill in the uncached embeddings
            for (idx, embedding) in uncached_indices.into_iter().zip(new_embeddings.into_iter()) {
                embeddings[idx] = Some(embedding);
            }
        }

        // Convert to final result
        Ok(embeddings.into_iter()
            .map(|opt| opt.expect("All embeddings should be computed"))
            .collect())
    }

    async fn generate_embeddings(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        // This is a placeholder implementation that generates deterministic embeddings
        // In production, this would use the actual ONNX Runtime for inference
        
        debug!("🔄 Generating embeddings for {} texts", texts.len());
        
        // Simulate processing time
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        
        let embeddings: Vec<Vec<f32>> = texts.iter()
            .map(|text| self.create_deterministic_embedding(text))
            .collect();

        debug!("✅ Generated {} embeddings", embeddings.len());
        Ok(embeddings)
    }

    fn create_deterministic_embedding(&self, text: &str) -> Vec<f32> {
        // Generate a deterministic but varied embedding based on text content
        let mut embedding = vec![0.0; self.dimensions];
        
        // Use a simple hash-based approach to create varied embeddings
        let bytes = text.as_bytes();
        let mut seed = 0u64;
        
        for (i, &byte) in bytes.iter().enumerate() {
            seed = seed.wrapping_mul(31).wrapping_add(byte as u64);
            
            // Use the hash to influence multiple dimensions
            let base_idx = (seed as usize) % self.dimensions;
            for j in 0..10.min(self.dimensions) {
                let idx = (base_idx + j) % self.dimensions;
                let influence = ((seed.wrapping_add(j as u64 * 7)) as f32 / u64::MAX as f32) * 2.0 - 1.0;
                embedding[idx] += influence * 0.1;
            }
        }
        
        // Add some text-length-based features
        let length_factor = (text.len() as f32).ln() / 10.0;
        for i in 0..self.dimensions.min(50) {
            embedding[i] += length_factor * (i as f32 / 100.0);
        }
        
        // Normalize the embedding
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut embedding {
                *x /= norm;
            }
        }
        
        embedding
    }

    pub fn clear_cache(&self) -> tokio::task::JoinHandle<()> {
        let cache = Arc::clone(&self.cache);
        tokio::spawn(async move {
            let mut cache = cache.write().await;
            cache.clear();
        })
    }

    pub async fn cache_size(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }

    pub fn set_batch_size(&mut self, batch_size: usize) {
        self.batch_size = batch_size.max(1);
    }

    pub fn set_max_sequence_length(&mut self, max_length: usize) {
        self.max_sequence_length = max_length.max(1);
    }

    pub fn model_path(&self) -> &str {
        &self.model_path
    }
}

impl EmbeddingEngine for OnnxEmbeddingEngine {
    fn encode(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        // Create a runtime for async operations in sync context
        let rt = tokio::runtime::Runtime::new()
            .context("Failed to create tokio runtime")?;
        
        rt.block_on(self.encode_batch(texts))
    }

    fn encode_single(&self, text: &str) -> Result<Vec<f32>> {
        let texts = vec![text.to_string()];
        let embeddings = self.encode(&texts)?;
        embeddings.into_iter().next()
            .ok_or_else(|| anyhow::anyhow!("No embedding generated"))
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }
} 
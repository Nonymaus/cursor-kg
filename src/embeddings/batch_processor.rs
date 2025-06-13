use anyhow::Result;
use std::collections::HashMap;
use tracing::debug;
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tokio::time::{timeout, Duration};
use crate::embeddings::onnx_runtime::OnnxEmbeddingEngine;

#[derive(Clone)]
pub struct BatchProcessor {
    batch_size: usize,
    max_concurrent_batches: usize,
    timeout_duration: Duration,
    embedding_cache: Arc<RwLock<LruCache<String, Vec<f32>>>>,
    semaphore: Arc<Semaphore>,
}

struct LruCache<K, V> {
    capacity: usize,
    map: HashMap<K, V>,
    order: Vec<K>,
}

impl<K: Clone + std::hash::Hash + Eq, V: Clone> LruCache<K, V> {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            map: HashMap::new(),
            order: Vec::new(),
        }
    }

    fn get(&mut self, key: &K) -> Option<&V> {
        if self.map.contains_key(key) {
            // Move to front (most recently used)
            self.order.retain(|k| k != key);
            self.order.push(key.clone());
            self.map.get(key)
        } else {
            None
        }
    }

    fn put(&mut self, key: K, value: V) {
        if self.map.len() >= self.capacity && !self.map.contains_key(&key) {
            // Remove least recently used
            if let Some(old_key) = self.order.first().cloned() {
                self.map.remove(&old_key);
                self.order.remove(0);
            }
        }

        if self.map.contains_key(&key) {
            self.order.retain(|k| k != &key);
        }

        self.map.insert(key.clone(), value);
        self.order.push(key);
    }

    fn len(&self) -> usize {
        self.map.len()
    }

    fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }
}

impl BatchProcessor {
    pub fn new(batch_size: usize) -> Self {
        let max_concurrent_batches = num_cpus::get().max(4);
        let cache_capacity = 10000; // Store up to 10k embeddings
        
        Self {
            batch_size: batch_size.max(1),
            max_concurrent_batches,
            timeout_duration: Duration::from_secs(30),
            embedding_cache: Arc::new(RwLock::new(LruCache::new(cache_capacity))),
            semaphore: Arc::new(Semaphore::new(max_concurrent_batches)),
        }
    }

    pub async fn process_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        debug!("🔄 Processing batch of {} texts", texts.len());

        // Check cache for existing embeddings
        let (cached_embeddings, uncached_texts, uncached_indices) = 
            self.check_cache(&texts).await;

        let mut final_embeddings = cached_embeddings;

        // Process uncached texts in parallel batches
        if !uncached_texts.is_empty() {
            let new_embeddings = self.process_uncached_batch(uncached_texts.clone()).await?;
            
            // Update cache
            self.update_cache(&uncached_texts, &new_embeddings).await;

            // Merge results
            for (idx, embedding) in uncached_indices.into_iter().zip(new_embeddings.into_iter()) {
                final_embeddings[idx] = Some(embedding);
            }
        }

        // Convert to final result
        let result: Vec<Vec<f32>> = final_embeddings
            .into_iter()
            .map(|opt| opt.expect("All embeddings should be computed"))
            .collect();

        debug!("✅ Batch processing completed: {} embeddings", result.len());
        Ok(result)
    }

    async fn check_cache(&self, texts: &[String]) -> (Vec<Option<Vec<f32>>>, Vec<String>, Vec<usize>) {
        let mut cache = self.embedding_cache.write().await;
        let mut cached_embeddings = Vec::with_capacity(texts.len());
        let mut uncached_texts = Vec::new();
        let mut uncached_indices = Vec::new();

        for (i, text) in texts.iter().enumerate() {
            if let Some(embedding) = cache.get(text) {
                cached_embeddings.push(Some(embedding.clone()));
            } else {
                cached_embeddings.push(None);
                uncached_texts.push(text.clone());
                uncached_indices.push(i);
            }
        }

        (cached_embeddings, uncached_texts, uncached_indices)
    }

    async fn update_cache(&self, texts: &[String], embeddings: &[Vec<f32>]) {
        let mut cache = self.embedding_cache.write().await;
        for (text, embedding) in texts.iter().zip(embeddings.iter()) {
            cache.put(text.clone(), embedding.clone());
        }
    }

    async fn process_uncached_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let mut all_embeddings = Vec::new();
        let mut handles = Vec::new();

        // Split into chunks for parallel processing
        let chunks: Vec<_> = texts.chunks(self.batch_size).collect();
        
        for chunk in chunks {
            let chunk_texts = chunk.to_vec();
            let semaphore = Arc::clone(&self.semaphore);
            let timeout_duration = self.timeout_duration;

            let handle = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.expect("Semaphore closed");
                
                timeout(timeout_duration, async {
                    Self::process_chunk(chunk_texts).await
                }).await
                .map_err(|_| anyhow::anyhow!("Batch processing timed out"))?
            });

            handles.push(handle);
        }

        // Collect results
        for handle in handles {
            let chunk_embeddings = handle.await
                .map_err(|e| anyhow::anyhow!("Task failed: {}", e))??;
            all_embeddings.extend(chunk_embeddings);
        }

        Ok(all_embeddings)
    }

    async fn process_chunk(texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        // This is a placeholder - in real implementation, you'd use the embedding engine
        // For now, return dummy embeddings to demonstrate the structure
                    debug!("  🔄 Processing chunk of {} texts", texts.len());
        
        // Simulate processing time
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // Return dummy embeddings (replace with actual ONNX inference)
        let embeddings: Vec<Vec<f32>> = texts.iter()
            .map(|text| {
                // Generate a deterministic but varied embedding based on text hash
                let hash = Self::simple_hash(text);
                let mut embedding = vec![0.0; 768]; // Assuming 768 dimensions
                
                for i in 0..768 {
                    embedding[i] = ((hash.wrapping_add(i as u64)) as f32 / u64::MAX as f32) * 2.0 - 1.0;
                }
                
                // Normalize
                let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    for x in &mut embedding {
                        *x /= norm;
                    }
                }
                
                embedding
            })
            .collect();

                    debug!("  ✅ Chunk processed: {} embeddings", embeddings.len());
        Ok(embeddings)
    }

    fn simple_hash(text: &str) -> u64 {
        let mut hash = 0u64;
        for byte in text.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
        }
        hash
    }

    pub async fn process_with_engine(
        &self, 
        texts: Vec<String>,
        engine: &OnnxEmbeddingEngine
    ) -> Result<Vec<Vec<f32>>> {
        // Check cache first
        let (cached_embeddings, uncached_texts, uncached_indices) = 
            self.check_cache(&texts).await;

        let mut final_embeddings = cached_embeddings;

        // Process uncached texts with the embedding engine
        if !uncached_texts.is_empty() {
            let new_embeddings = engine.encode_batch(&uncached_texts).await?;
            
            // Update cache
            self.update_cache(&uncached_texts, &new_embeddings).await;

            // Merge results
            for (idx, embedding) in uncached_indices.into_iter().zip(new_embeddings.into_iter()) {
                final_embeddings[idx] = Some(embedding);
            }
        }

        // Convert to final result
        Ok(final_embeddings
            .into_iter()
            .map(|opt| opt.expect("All embeddings should be computed"))
            .collect())
    }

    pub async fn get_cache_stats(&self) -> (usize, usize) {
        let cache = self.embedding_cache.read().await;
        (cache.len(), cache.capacity)
    }

    pub async fn clear_cache(&self) {
        let mut cache = self.embedding_cache.write().await;
        cache.clear();
        debug!("🧹 Embedding cache cleared");
    }

    pub fn set_batch_size(&mut self, batch_size: usize) {
        self.batch_size = batch_size.max(1);
    }

    pub fn set_timeout(&mut self, timeout_secs: u64) {
        self.timeout_duration = Duration::from_secs(timeout_secs);
    }

    pub async fn precompute_embeddings(&self, texts: Vec<String>) -> Result<()> {
        debug!("🚀 Precomputing embeddings for {} texts", texts.len());
        self.process_batch(texts).await?;
        debug!("✅ Precomputation complete");
        Ok(())
    }

    pub async fn warmup_cache(&self, common_queries: Vec<String>) -> Result<()> {
        debug!("🔥 Warming up cache with {} common queries", common_queries.len());
        self.precompute_embeddings(common_queries).await
    }
} 
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{debug, info, warn};
use sha2::{Sha256, Digest};

/// Performance metrics for MCP operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub average_response_time: Duration,
    pub tool_metrics: HashMap<String, ToolMetrics>,
    pub cache_hit_rate: f32,
    pub memory_usage: u64,
    pub uptime: Duration,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Metrics for individual tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMetrics {
    pub name: String,
    pub call_count: u64,
    pub success_count: u64,
    pub error_count: u64,
    pub average_duration: Duration,
    pub min_duration: Duration,
    pub max_duration: Duration,
    pub last_called: Option<chrono::DateTime<chrono::Utc>>,
}

/// Response cache for expensive operations
pub struct ResponseCache {
    cache: Arc<Mutex<HashMap<String, CachedResponse>>>,
    max_entries: usize,
    default_ttl: Duration,
}

#[derive(Clone)]
struct CachedResponse {
    response: Value,
    created_at: Instant,
    ttl: Duration,
    access_count: u64,
    last_accessed: Instant,
}

/// Performance monitor for tracking MCP operations
pub struct PerformanceMonitor {
    start_time: Instant,
    metrics: Arc<Mutex<PerformanceMetrics>>,
    cache: ResponseCache,
    response_times: Arc<Mutex<Vec<Duration>>>,
    max_response_time_samples: usize,
}

impl PerformanceMonitor {
    /// Create a new performance monitor
    pub fn new(cache_size: usize, cache_ttl: Duration) -> Self {
        Self {
            start_time: Instant::now(),
            metrics: Arc::new(Mutex::new(PerformanceMetrics {
                total_requests: 0,
                successful_requests: 0,
                failed_requests: 0,
                average_response_time: Duration::from_millis(0),
                tool_metrics: HashMap::new(),
                cache_hit_rate: 0.0,
                memory_usage: 0,
                uptime: Duration::from_secs(0),
                last_updated: chrono::Utc::now(),
            })),
            cache: ResponseCache::new(cache_size, cache_ttl),
            response_times: Arc::new(Mutex::new(Vec::new())),
            max_response_time_samples: 1000,
        }
    }

    /// Record tool execution metrics
    pub async fn record_tool_execution<F, R>(
        &self,
        tool_name: &str,
        operation: F,
    ) -> Result<R>
    where
        F: std::future::Future<Output = Result<R>>,
    {
        let start = Instant::now();
        
        // Execute operation with timeout
        let result = timeout(Duration::from_secs(30), operation).await;
        
        let duration = start.elapsed();
        let success = result.is_ok() && result.as_ref().unwrap().is_ok();
        
        // Update metrics
        self.update_tool_metrics(tool_name, duration, success).await;
        self.update_response_times(duration).await;
        
        match result {
            Ok(Ok(value)) => {
                self.increment_successful_requests().await;
                Ok(value)
            }
            Ok(Err(e)) => {
                self.increment_failed_requests().await;
                Err(e)
            }
            Err(_) => {
                self.increment_failed_requests().await;
                Err(anyhow::anyhow!("Operation timed out after 30 seconds"))
            }
        }
    }

    /// Try to get cached response
    pub async fn get_cached_response(&self, cache_key: &str) -> Option<Value> {
        self.cache.get(cache_key).await
    }

    /// Cache a response
    pub async fn cache_response(&self, cache_key: String, response: Value, ttl: Option<Duration>) {
        self.cache.set(cache_key, response, ttl).await;
    }

    /// Generate cache key for request
    pub fn generate_cache_key(&self, tool_name: &str, params: &Value) -> String {
        let params_str = serde_json::to_string(params).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(params_str.as_bytes());
        let hash = hasher.finalize();
        format!("{}:{}", tool_name, format!("{:x}", hash))
    }

    /// Get current performance metrics
    pub async fn get_metrics(&self) -> PerformanceMetrics {
        let mut metrics = self.metrics.lock().unwrap().clone();
        metrics.uptime = self.start_time.elapsed();
        metrics.cache_hit_rate = self.cache.get_hit_rate().await;
        metrics.memory_usage = self.estimate_memory_usage().await;
        metrics.last_updated = chrono::Utc::now();
        
        // Update average response time
        if let Ok(response_times) = self.response_times.lock() {
            if !response_times.is_empty() {
                let total: Duration = response_times.iter().sum();
                metrics.average_response_time = total / response_times.len() as u32;
            }
        }
        
        metrics
    }

    /// Start background metrics collection
    pub fn start_background_collection(&self) -> tokio::task::JoinHandle<()> {
        let monitor = self.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                // Cleanup old cache entries
                monitor.cache.cleanup().await;
                
                // Log performance summary
                let metrics = monitor.get_metrics().await;
                info!(
                    "Performance Summary - Requests: {}/{} ({}% success), Avg Response: {:?}, Cache Hit Rate: {:.1}%, Memory: {}MB",
                    metrics.successful_requests,
                    metrics.total_requests,
                    if metrics.total_requests > 0 { (metrics.successful_requests * 100) / metrics.total_requests } else { 0 },
                    metrics.average_response_time,
                    metrics.cache_hit_rate * 100.0,
                    metrics.memory_usage / 1_000_000
                );
                
                // Log slow tools
                for (name, tool_metrics) in &metrics.tool_metrics {
                    if tool_metrics.average_duration > Duration::from_millis(1000) {
                        warn!("Slow tool detected: {} - Avg: {:?}", name, tool_metrics.average_duration);
                    }
                }
            }
        })
    }

    /// Update tool-specific metrics
    async fn update_tool_metrics(&self, tool_name: &str, duration: Duration, success: bool) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.total_requests += 1;
        
        let tool_metrics = metrics.tool_metrics.entry(tool_name.to_string())
            .or_insert_with(|| ToolMetrics {
                name: tool_name.to_string(),
                call_count: 0,
                success_count: 0,
                error_count: 0,
                average_duration: Duration::from_millis(0),
                min_duration: duration,
                max_duration: duration,
                last_called: None,
            });
        
        tool_metrics.call_count += 1;
        tool_metrics.last_called = Some(chrono::Utc::now());
        
        if success {
            tool_metrics.success_count += 1;
        } else {
            tool_metrics.error_count += 1;
        }
        
        // Update duration statistics
        if duration < tool_metrics.min_duration {
            tool_metrics.min_duration = duration;
        }
        if duration > tool_metrics.max_duration {
            tool_metrics.max_duration = duration;
        }
        
        // Calculate rolling average
        let total_duration = tool_metrics.average_duration * (tool_metrics.call_count - 1) as u32 + duration;
        tool_metrics.average_duration = total_duration / tool_metrics.call_count as u32;
    }

    /// Update response time tracking
    async fn update_response_times(&self, duration: Duration) {
        if let Ok(mut response_times) = self.response_times.lock() {
            response_times.push(duration);
            
            // Keep only recent samples
            if response_times.len() > self.max_response_time_samples {
                let len = response_times.len();
                response_times.drain(0..len - self.max_response_time_samples);
            }
        }
    }

    /// Increment successful request counter
    async fn increment_successful_requests(&self) {
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.successful_requests += 1;
        }
    }

    /// Increment failed request counter
    async fn increment_failed_requests(&self) {
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.failed_requests += 1;
        }
    }

    /// Estimate current memory usage
    async fn estimate_memory_usage(&self) -> u64 {
        // Simple memory estimation based on cache size
        let cache_memory = self.cache.estimate_memory_usage().await;
        let base_memory = 10_000_000; // 10MB base estimation
        base_memory + cache_memory
    }
}

impl Clone for PerformanceMonitor {
    fn clone(&self) -> Self {
        Self {
            start_time: self.start_time,
            metrics: Arc::clone(&self.metrics),
            cache: self.cache.clone(),
            response_times: Arc::clone(&self.response_times),
            max_response_time_samples: self.max_response_time_samples,
        }
    }
}

impl ResponseCache {
    fn new(max_entries: usize, default_ttl: Duration) -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            max_entries,
            default_ttl,
        }
    }

    async fn get(&self, key: &str) -> Option<Value> {
        if let Ok(mut cache) = self.cache.lock() {
            if let Some(cached) = cache.get_mut(key) {
                // Check if expired
                if cached.created_at.elapsed() > cached.ttl {
                    cache.remove(key);
                    return None;
                }
                
                // Update access statistics
                cached.access_count += 1;
                cached.last_accessed = Instant::now();
                
                debug!("Cache hit for key: {}", key);
                return Some(cached.response.clone());
            }
        }
        
        debug!("Cache miss for key: {}", key);
        None
    }

    async fn set(&self, key: String, value: Value, ttl: Option<Duration>) {
        if let Ok(mut cache) = self.cache.lock() {
            // Enforce max entries limit
            if cache.len() >= self.max_entries {
                self.evict_lru(&mut cache);
            }
            
            let cached_response = CachedResponse {
                response: value,
                created_at: Instant::now(),
                ttl: ttl.unwrap_or(self.default_ttl),
                access_count: 0,
                last_accessed: Instant::now(),
            };
            
            cache.insert(key, cached_response);
        }
    }

    async fn cleanup(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            let now = Instant::now();
            cache.retain(|_key, cached| {
                now.duration_since(cached.created_at) <= cached.ttl
            });
        }
    }

    async fn get_hit_rate(&self) -> f32 {
        // Simplified hit rate calculation
        0.75 // Placeholder - would track actual hits/misses in production
    }

    async fn estimate_memory_usage(&self) -> u64 {
        if let Ok(cache) = self.cache.lock() {
            // Rough estimation: 1KB per cached entry
            cache.len() as u64 * 1024
        } else {
            0
        }
    }

    fn evict_lru(&self, cache: &mut HashMap<String, CachedResponse>) {
        if let Some(lru_key) = cache.iter()
            .min_by_key(|(_, cached)| cached.last_accessed)
            .map(|(key, _)| key.clone()) {
            cache.remove(&lru_key);
        }
    }
}

impl Clone for ResponseCache {
    fn clone(&self) -> Self {
        Self {
            cache: Arc::clone(&self.cache),
            max_entries: self.max_entries,
            default_ttl: self.default_ttl,
        }
    }
}

/// Connection pool for managing client connections
pub struct ConnectionPool {
    max_connections: usize,
    active_connections: Arc<Mutex<usize>>,
    connection_queue: Arc<Mutex<Vec<tokio::sync::oneshot::Sender<()>>>>,
}

impl ConnectionPool {
    pub fn new(max_connections: usize) -> Self {
        Self {
            max_connections,
            active_connections: Arc::new(Mutex::new(0)),
            connection_queue: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Try to acquire a connection slot
    pub async fn acquire(&self) -> Result<ConnectionGuard> {
        loop {
            // Try to get a connection immediately
            if let Ok(mut active) = self.active_connections.lock() {
                if *active < self.max_connections {
                    *active += 1;
                    return Ok(ConnectionGuard::new(Arc::clone(&self.active_connections)));
                }
            }
            
            // Wait for a connection to become available
            let (tx, rx) = tokio::sync::oneshot::channel();
            if let Ok(mut queue) = self.connection_queue.lock() {
                queue.push(tx);
            }
            
            rx.await.map_err(|_| anyhow::anyhow!("Connection pool closed"))?;
        }
    }

    /// Get current connection statistics
    pub fn get_stats(&self) -> (usize, usize) {
        let active_count = self.active_connections.lock()
            .map(|guard| *guard)
            .unwrap_or(0);
        (active_count, self.max_connections)
    }
}

/// RAII guard for connection slots
pub struct ConnectionGuard {
    active_connections: Arc<Mutex<usize>>,
}

impl ConnectionGuard {
    fn new(active_connections: Arc<Mutex<usize>>) -> Self {
        Self { active_connections }
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active_connections.lock() {
            if *active > 0 {
                *active -= 1;
            }
        }
    }
} 
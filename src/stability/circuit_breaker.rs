use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, warn, error};

/// Circuit breaker states
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    Closed,    // Normal operation
    Open,      // Failing, reject requests
    HalfOpen,  // Testing if service recovered
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub recovery_timeout: Duration,
    pub success_threshold: u32,
    pub timeout: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(60),
            success_threshold: 3,
            timeout: Duration::from_secs(30),
        }
    }
}

/// Circuit breaker for fault tolerance
pub struct CircuitBreaker {
    state: Arc<Mutex<CircuitState>>,
    failure_count: Arc<Mutex<u32>>,
    success_count: Arc<Mutex<u32>>,
    last_failure_time: Arc<Mutex<Option<Instant>>>,
    config: CircuitBreakerConfig,
    name: String,
}

impl CircuitBreaker {
    pub fn new(name: String, config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(Mutex::new(CircuitState::Closed)),
            failure_count: Arc::new(Mutex::new(0)),
            success_count: Arc::new(Mutex::new(0)),
            last_failure_time: Arc::new(Mutex::new(None)),
            config,
            name,
        }
    }

    /// Execute a function with circuit breaker protection
    pub async fn call<F, T, E>(&self, operation: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Display + Send + Sync + 'static,
    {
        // Check if circuit is open
        if self.is_open() {
            return Err(anyhow::anyhow!("Circuit breaker '{}' is OPEN", self.name));
        }

        // Try to transition from half-open to closed if needed
        if self.is_half_open() {
            debug!("Circuit breaker '{}' is HALF-OPEN, testing service", self.name);
        }

        // Execute the operation with timeout
        let start = Instant::now();
        let result = tokio::time::timeout(self.config.timeout, operation).await;

        match result {
            Ok(Ok(value)) => {
                self.on_success();
                debug!("Circuit breaker '{}' - Operation succeeded in {:?}", self.name, start.elapsed());
                Ok(value)
            },
            Ok(Err(e)) => {
                self.on_failure();
                error!("Circuit breaker '{}' - Operation failed: {}", self.name, e);
                Err(anyhow::anyhow!("Operation failed: {}", e))
            },
            Err(_) => {
                self.on_failure();
                error!("Circuit breaker '{}' - Operation timed out after {:?}", self.name, self.config.timeout);
                Err(anyhow::anyhow!("Operation timed out"))
            }
        }
    }

    /// Check if circuit breaker allows requests
    pub fn can_execute(&self) -> bool {
        !self.is_open()
    }

    /// Get current circuit state
    pub fn get_state(&self) -> CircuitState {
        let state = self.state.lock().unwrap();
        state.clone()
    }

    /// Get failure statistics
    pub fn get_stats(&self) -> CircuitBreakerStats {
        let state = self.state.lock().unwrap();
        let failure_count = *self.failure_count.lock().unwrap();
        let success_count = *self.success_count.lock().unwrap();
        let last_failure = *self.last_failure_time.lock().unwrap();

        CircuitBreakerStats {
            name: self.name.clone(),
            state: state.clone(),
            failure_count,
            success_count,
            last_failure_time: last_failure,
            config: self.config.clone(),
        }
    }

    /// Reset circuit breaker to closed state
    pub fn reset(&self) {
        let mut state = self.state.lock().unwrap();
        let mut failure_count = self.failure_count.lock().unwrap();
        let mut success_count = self.success_count.lock().unwrap();
        let mut last_failure = self.last_failure_time.lock().unwrap();

        *state = CircuitState::Closed;
        *failure_count = 0;
        *success_count = 0;
        *last_failure = None;

        debug!("Circuit breaker '{}' has been reset", self.name);
    }

    // Private methods

    fn is_open(&self) -> bool {
        let state = self.state.lock().unwrap();
        match *state {
            CircuitState::Open => {
                // Check if we should transition to half-open
                if let Some(last_failure) = *self.last_failure_time.lock().unwrap() {
                    if last_failure.elapsed() >= self.config.recovery_timeout {
                        drop(state);
                        self.transition_to_half_open();
                        return false;
                    }
                }
                true
            },
            _ => false,
        }
    }

    fn is_half_open(&self) -> bool {
        let state = self.state.lock().unwrap();
        matches!(*state, CircuitState::HalfOpen)
    }

    fn on_success(&self) {
        let mut state = self.state.lock().unwrap();
        let mut success_count = self.success_count.lock().unwrap();
        let mut failure_count = self.failure_count.lock().unwrap();

        match *state {
            CircuitState::HalfOpen => {
                *success_count += 1;
                if *success_count >= self.config.success_threshold {
                    *state = CircuitState::Closed;
                    *success_count = 0;
                    *failure_count = 0;
                    debug!("Circuit breaker '{}' transitioned to CLOSED", self.name);
                }
            },
            CircuitState::Closed => {
                // Reset failure count on success
                *failure_count = 0;
            },
            _ => {}
        }
    }

    fn on_failure(&self) {
        let mut state = self.state.lock().unwrap();
        let mut failure_count = self.failure_count.lock().unwrap();
        let mut last_failure = self.last_failure_time.lock().unwrap();

        *failure_count += 1;
        *last_failure = Some(Instant::now());

        match *state {
            CircuitState::Closed => {
                if *failure_count >= self.config.failure_threshold {
                    *state = CircuitState::Open;
                    warn!("Circuit breaker '{}' transitioned to OPEN after {} failures", 
                          self.name, failure_count);
                }
            },
            CircuitState::HalfOpen => {
                *state = CircuitState::Open;
                warn!("Circuit breaker '{}' transitioned back to OPEN from HALF-OPEN", self.name);
            },
            _ => {}
        }
    }

    fn transition_to_half_open(&self) {
        let mut state = self.state.lock().unwrap();
        let mut success_count = self.success_count.lock().unwrap();

        *state = CircuitState::HalfOpen;
        *success_count = 0;
        debug!("Circuit breaker '{}' transitioned to HALF-OPEN", self.name);
    }
}

/// Circuit breaker statistics
#[derive(Debug, Clone)]
pub struct CircuitBreakerStats {
    pub name: String,
    pub state: CircuitState,
    pub failure_count: u32,
    pub success_count: u32,
    pub last_failure_time: Option<Instant>,
    pub config: CircuitBreakerConfig,
}

/// Circuit breaker registry for managing multiple breakers
pub struct CircuitBreakerRegistry {
    breakers: Arc<Mutex<std::collections::HashMap<String, Arc<CircuitBreaker>>>>,
}

impl CircuitBreakerRegistry {
    pub fn new() -> Self {
        Self {
            breakers: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub fn get_or_create(&self, name: &str, config: CircuitBreakerConfig) -> Arc<CircuitBreaker> {
        let mut breakers = self.breakers.lock().unwrap();
        
        if let Some(breaker) = breakers.get(name) {
            Arc::clone(breaker)
        } else {
            let breaker = Arc::new(CircuitBreaker::new(name.to_string(), config));
            breakers.insert(name.to_string(), Arc::clone(&breaker));
            breaker
        }
    }

    pub fn get_all_stats(&self) -> Vec<CircuitBreakerStats> {
        let breakers = self.breakers.lock().unwrap();
        breakers.values().map(|b| b.get_stats()).collect()
    }

    pub fn reset_all(&self) {
        let breakers = self.breakers.lock().unwrap();
        for breaker in breakers.values() {
            breaker.reset();
        }
    }
}

impl Default for CircuitBreakerRegistry {
    fn default() -> Self {
        Self::new()
    }
} 
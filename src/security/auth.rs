use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub enabled: bool,
    pub api_key: Option<String>,
    pub rate_limit_requests_per_minute: u32,
    pub rate_limit_burst: u32,
    pub admin_operations_require_auth: bool,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Disabled by default for backward compatibility
            api_key: None,
            rate_limit_requests_per_minute: 60,
            rate_limit_burst: 10,
            admin_operations_require_auth: true,
        }
    }
}

/// Rate limiting information for a client
#[derive(Debug, Clone)]
struct RateLimitInfo {
    requests: Vec<Instant>,
    last_request: Instant,
}

impl RateLimitInfo {
    fn new() -> Self {
        Self {
            requests: Vec::new(),
            last_request: Instant::now(),
        }
    }
}

/// Authentication and authorization manager
pub struct AuthManager {
    config: AuthConfig,
    rate_limits: Arc<RwLock<HashMap<String, RateLimitInfo>>>,
}

impl AuthManager {
    pub fn new(config: AuthConfig) -> Self {
        Self {
            config,
            rate_limits: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Validate API key if authentication is enabled
    pub fn validate_api_key(&self, provided_key: Option<&str>) -> Result<bool> {
        if !self.config.enabled {
            return Ok(true); // Authentication disabled
        }
        
        match (&self.config.api_key, provided_key) {
            (Some(expected), Some(provided)) => {
                Ok(expected == provided)
            },
            (Some(_), None) => Ok(false), // API key required but not provided
            (None, _) => Ok(true), // No API key configured
        }
    }
    
    /// Check if operation requires authentication
    pub fn requires_auth(&self, operation: &str) -> bool {
        if !self.config.enabled {
            return false;
        }
        
        // Administrative operations that require authentication
        let admin_operations = [
            "delete_episode",
            "delete_entity_edge", 
            "clear_graph",
            "manage_graph",
        ];
        
        if self.config.admin_operations_require_auth {
            admin_operations.contains(&operation)
        } else {
            false
        }
    }
    
    /// Check rate limits for a client
    pub fn check_rate_limit(&self, client_id: &str) -> Result<bool> {
        let mut rate_limits = self.rate_limits.write()
            .map_err(|_| anyhow!("Failed to acquire rate limit lock"))?;
        
        let now = Instant::now();
        let window = Duration::from_secs(60); // 1 minute window
        
        let rate_info = rate_limits.entry(client_id.to_string())
            .or_insert_with(RateLimitInfo::new);
        
        // Remove old requests outside the window
        rate_info.requests.retain(|&request_time| {
            now.duration_since(request_time) < window
        });
        
        // Check if we're within limits
        if rate_info.requests.len() >= self.config.rate_limit_requests_per_minute as usize {
            return Ok(false); // Rate limit exceeded
        }
        
        // Check burst limit (requests in last 10 seconds)
        let burst_window = Duration::from_secs(10);
        let recent_requests = rate_info.requests.iter()
            .filter(|&&request_time| now.duration_since(request_time) < burst_window)
            .count();
        
        if recent_requests >= self.config.rate_limit_burst as usize {
            return Ok(false); // Burst limit exceeded
        }
        
        // Add current request
        rate_info.requests.push(now);
        rate_info.last_request = now;
        
        Ok(true)
    }
    
    /// Extract client identifier from request
    pub fn get_client_id(&self, headers: &HashMap<String, String>) -> String {
        // Try to get client ID from headers, fallback to IP or generate one
        headers.get("x-client-id")
            .or_else(|| headers.get("x-forwarded-for"))
            .or_else(|| headers.get("x-real-ip"))
            .cloned()
            .unwrap_or_else(|| Uuid::new_v4().to_string())
    }
    
    /// Clean up old rate limit entries
    pub fn cleanup_rate_limits(&self) -> Result<()> {
        let mut rate_limits = self.rate_limits.write()
            .map_err(|_| anyhow!("Failed to acquire rate limit lock"))?;
        
        let now = Instant::now();
        let cleanup_threshold = Duration::from_secs(300); // 5 minutes
        
        rate_limits.retain(|_, rate_info| {
            now.duration_since(rate_info.last_request) < cleanup_threshold
        });
        
        Ok(())
    }
    
    /// Get rate limit status for a client
    pub fn get_rate_limit_status(&self, client_id: &str) -> Result<RateLimitStatus> {
        let rate_limits = self.rate_limits.read()
            .map_err(|_| anyhow!("Failed to acquire rate limit lock"))?;
        
        if let Some(rate_info) = rate_limits.get(client_id) {
            let now = Instant::now();
            let window = Duration::from_secs(60);
            
            let current_requests = rate_info.requests.iter()
                .filter(|&&request_time| now.duration_since(request_time) < window)
                .count();
            
            Ok(RateLimitStatus {
                requests_used: current_requests as u32,
                requests_limit: self.config.rate_limit_requests_per_minute,
                reset_time: rate_info.requests.first()
                    .map(|&first| first + window)
                    .unwrap_or(now),
            })
        } else {
            Ok(RateLimitStatus {
                requests_used: 0,
                requests_limit: self.config.rate_limit_requests_per_minute,
                reset_time: now + Duration::from_secs(60),
            })
        }
    }
}

/// Rate limit status information
#[derive(Debug, Clone, Serialize)]
pub struct RateLimitStatus {
    pub requests_used: u32,
    pub requests_limit: u32,
    pub reset_time: Instant,
}

/// Authentication result
#[derive(Debug, Clone)]
pub enum AuthResult {
    Allowed,
    Denied(String),
    RateLimited,
}

impl AuthResult {
    pub fn is_allowed(&self) -> bool {
        matches!(self, AuthResult::Allowed)
    }
    
    pub fn error_message(&self) -> Option<&str> {
        match self {
            AuthResult::Allowed => None,
            AuthResult::Denied(msg) => Some(msg),
            AuthResult::RateLimited => Some("Rate limit exceeded"),
        }
    }
}

/// Comprehensive authentication check
pub fn authenticate_request(
    auth_manager: &AuthManager,
    operation: &str,
    api_key: Option<&str>,
    client_id: &str,
) -> Result<AuthResult> {
    // Check rate limits first
    if !auth_manager.check_rate_limit(client_id)? {
        return Ok(AuthResult::RateLimited);
    }
    
    // Check if operation requires authentication
    if auth_manager.requires_auth(operation) {
        if !auth_manager.validate_api_key(api_key)? {
            return Ok(AuthResult::Denied("Invalid or missing API key".to_string()));
        }
    }
    
    Ok(AuthResult::Allowed)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_auth_disabled() {
        let config = AuthConfig {
            enabled: false,
            ..Default::default()
        };
        let auth_manager = AuthManager::new(config);
        
        assert!(auth_manager.validate_api_key(None).unwrap());
        assert!(!auth_manager.requires_auth("delete_episode"));
    }
    
    #[test]
    fn test_auth_enabled() {
        let config = AuthConfig {
            enabled: true,
            api_key: Some("test-key".to_string()),
            admin_operations_require_auth: true,
            ..Default::default()
        };
        let auth_manager = AuthManager::new(config);
        
        assert!(auth_manager.validate_api_key(Some("test-key")).unwrap());
        assert!(!auth_manager.validate_api_key(Some("wrong-key")).unwrap());
        assert!(!auth_manager.validate_api_key(None).unwrap());
        assert!(auth_manager.requires_auth("delete_episode"));
    }
    
    #[test]
    fn test_rate_limiting() {
        let config = AuthConfig {
            rate_limit_requests_per_minute: 2,
            rate_limit_burst: 1,
            ..Default::default()
        };
        let auth_manager = AuthManager::new(config);
        
        let client_id = "test-client";
        
        // First request should be allowed
        assert!(auth_manager.check_rate_limit(client_id).unwrap());
        
        // Second request should hit burst limit
        assert!(!auth_manager.check_rate_limit(client_id).unwrap());
    }
}

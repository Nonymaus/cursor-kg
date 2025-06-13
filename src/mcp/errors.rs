use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fmt;
use thiserror::Error;
use tracing::error;

/// MCP-specific error types
#[derive(Debug, Error)]
pub enum McpError {
    #[error("Protocol error: {message}")]
    Protocol { message: String },
    
    #[error("Invalid request: {message}")]
    InvalidRequest { message: String },
    
    #[error("Tool not found: {tool_name}")]
    ToolNotFound { tool_name: String },
    
    #[error("Invalid parameters: {message}")]
    InvalidParameters { message: String },
    
    #[error("Search error: {message}")]
    SearchError { message: String },
    
    #[error("Storage error: {message}")]
    StorageError { message: String },
    
    #[error("Embedding error: {message}")]
    EmbeddingError { message: String },
    
    #[error("Memory optimization error: {message}")]
    MemoryError { message: String },
    
    #[error("Graph operation error: {message}")]
    GraphError { message: String },
    
    #[error("Authentication error: {message}")]
    AuthError { message: String },
    
    #[error("Rate limit exceeded: {message}")]
    RateLimit { message: String },
    
    #[error("Internal server error: {message}")]
    Internal { message: String },
}

/// Error response format for MCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpErrorResponse {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

/// Error context for debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    pub error_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub tool_name: Option<String>,
    pub parameters: Option<Value>,
    pub user_agent: Option<String>,
    pub client_version: Option<String>,
}

impl McpError {
    /// Convert to MCP error response with appropriate error codes
    pub fn to_mcp_error(&self) -> McpErrorResponse {
        let (code, message) = match self {
            McpError::Protocol { message } => (-32700, message.clone()),
            McpError::InvalidRequest { message } => (-32600, message.clone()),
            McpError::ToolNotFound { tool_name } => (-32601, format!("Tool not found: {}", tool_name)),
            McpError::InvalidParameters { message } => (-32602, message.clone()),
            McpError::SearchError { message } => (-32001, format!("Search failed: {}", message)),
            McpError::StorageError { message } => (-32002, format!("Storage failed: {}", message)),
            McpError::EmbeddingError { message } => (-32003, format!("Embedding failed: {}", message)),
            McpError::MemoryError { message } => (-32004, format!("Memory operation failed: {}", message)),
            McpError::GraphError { message } => (-32005, format!("Graph operation failed: {}", message)),
            McpError::AuthError { message } => (-32006, format!("Authentication failed: {}", message)),
            McpError::RateLimit { message } => (-32007, format!("Rate limit exceeded: {}", message)),
            McpError::Internal { message } => (-32603, format!("Internal error: {}", message)),
        };

        McpErrorResponse {
            code,
            message,
            data: None,
        }
    }

    /// Create error with context
    pub fn with_context(self, context: ErrorContext) -> McpErrorWithContext {
        McpErrorWithContext {
            error: self,
            context,
        }
    }
}

/// Error with debugging context
#[derive(Debug)]
pub struct McpErrorWithContext {
    pub error: McpError,
    pub context: ErrorContext,
}

impl fmt::Display for McpErrorWithContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (Error ID: {})", self.error, self.context.error_id)
    }
}

impl std::error::Error for McpErrorWithContext {}

/// Error handler for MCP operations
pub struct ErrorHandler {
    enable_debug_info: bool,
    enable_error_logging: bool,
}

impl ErrorHandler {
    pub fn new(enable_debug_info: bool, enable_error_logging: bool) -> Self {
        Self {
            enable_debug_info,
            enable_error_logging,
        }
    }

    /// Handle an error and convert to appropriate MCP response
    pub fn handle_error(&self, error: anyhow::Error, context: Option<ErrorContext>) -> McpErrorResponse {
        // Try to downcast to McpError first
        if let Some(mcp_error) = error.downcast_ref::<McpError>() {
            let mut response = mcp_error.to_mcp_error();
            
            if self.enable_debug_info {
                if let Some(ref ctx) = context {
                    response.data = Some(json!({
                        "error_id": ctx.error_id,
                        "timestamp": ctx.timestamp,
                        "context": ctx
                    }));
                }
            }
            
            if self.enable_error_logging {
                error!("MCP Error: {} - Context: {:?}", mcp_error, context);
            }
            
            return response;
        }

        // Handle other error types
        let error_message = error.to_string();
        let mcp_error = if error_message.contains("timeout") {
            McpError::Internal { message: "Operation timed out".to_string() }
        } else if error_message.contains("not found") {
            McpError::InvalidRequest { message: "Resource not found".to_string() }
        } else if error_message.contains("permission") || error_message.contains("unauthorized") {
            McpError::AuthError { message: "Insufficient permissions".to_string() }
        } else {
            McpError::Internal { message: "Internal server error".to_string() }
        };

        let mut response = mcp_error.to_mcp_error();
        
        if self.enable_debug_info {
            response.data = Some(json!({
                "original_error": error_message,
                "error_chain": format!("{:?}", error)
            }));
            
            if let Some(ref ctx) = context {
                if let Some(data) = response.data.as_mut() {
                    data.as_object_mut().unwrap().insert("context".to_string(), json!(ctx));
                }
            }
        }
        
        if self.enable_error_logging {
            error!("Unhandled Error: {} - Context: {:?}", error, context);
        }
        
        response
    }

    /// Create error context for tracking
    pub fn create_context(
        &self,
        tool_name: Option<&str>,
        parameters: Option<Value>,
        user_agent: Option<&str>,
        client_version: Option<&str>,
    ) -> ErrorContext {
        ErrorContext {
            error_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            tool_name: tool_name.map(|s| s.to_string()),
            parameters,
            user_agent: user_agent.map(|s| s.to_string()),
            client_version: client_version.map(|s| s.to_string()),
        }
    }
}

/// Validation utilities for MCP parameters
pub struct ParameterValidator;

impl ParameterValidator {
    /// Validate required string parameter
    pub fn require_string(params: &Value, field: &str) -> Result<String, McpError> {
        params.get(field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| McpError::InvalidParameters {
                message: format!("Missing or invalid required parameter: {}", field)
            })
    }

    /// Validate optional string parameter
    pub fn optional_string(params: &Value, field: &str) -> Option<String> {
        params.get(field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Validate UUID parameter
    pub fn require_uuid(params: &Value, field: &str) -> Result<uuid::Uuid, McpError> {
        let uuid_str = Self::require_string(params, field)?;
        uuid::Uuid::parse_str(&uuid_str)
            .map_err(|_| McpError::InvalidParameters {
                message: format!("Invalid UUID format for parameter: {}", field)
            })
    }

    /// Validate integer parameter with bounds
    pub fn require_int_bounded(params: &Value, field: &str, min: i64, max: i64) -> Result<i64, McpError> {
        let value = params.get(field)
            .and_then(|v| v.as_i64())
            .ok_or_else(|| McpError::InvalidParameters {
                message: format!("Missing or invalid integer parameter: {}", field)
            })?;
        
        if value < min || value > max {
            return Err(McpError::InvalidParameters {
                message: format!("Parameter {} must be between {} and {}, got {}", field, min, max, value)
            });
        }
        
        Ok(value)
    }

    /// Validate array parameter
    pub fn require_array<'a>(params: &'a Value, field: &str) -> Result<&'a Vec<Value>, McpError> {
        params.get(field)
            .and_then(|v| v.as_array())
            .ok_or_else(|| McpError::InvalidParameters {
                message: format!("Missing or invalid array parameter: {}", field)
            })
    }

    /// Validate enum parameter
    pub fn require_enum(params: &Value, field: &str, allowed: &[&str]) -> Result<String, McpError> {
        let value = Self::require_string(params, field)?;
        
        if !allowed.contains(&value.as_str()) {
            return Err(McpError::InvalidParameters {
                message: format!("Parameter {} must be one of {:?}, got '{}'", field, allowed, value)
            });
        }
        
        Ok(value)
    }
}

/// Rate limiting for MCP operations
pub struct RateLimiter {
    requests_per_minute: u32,
    requests: std::collections::HashMap<String, Vec<chrono::DateTime<chrono::Utc>>>,
}

impl RateLimiter {
    pub fn new(requests_per_minute: u32) -> Self {
        Self {
            requests_per_minute,
            requests: std::collections::HashMap::new(),
        }
    }

    /// Check if request is allowed for client
    pub fn check_rate_limit(&mut self, client_id: &str) -> Result<(), McpError> {
        let now = chrono::Utc::now();
        let minute_ago = now - chrono::Duration::minutes(1);
        
        let client_requests = self.requests.entry(client_id.to_string()).or_insert_with(Vec::new);
        
        // Remove old requests
        client_requests.retain(|&timestamp| timestamp > minute_ago);
        
        if client_requests.len() >= self.requests_per_minute as usize {
            return Err(McpError::RateLimit {
                message: format!("Rate limit exceeded: {} requests per minute", self.requests_per_minute)
            });
        }
        
        client_requests.push(now);
        Ok(())
    }

    /// Clean up old entries periodically
    pub fn cleanup(&mut self) {
        let minute_ago = chrono::Utc::now() - chrono::Duration::minutes(1);
        self.requests.retain(|_, timestamps| {
            timestamps.retain(|&timestamp| timestamp > minute_ago);
            !timestamps.is_empty()
        });
    }
} 
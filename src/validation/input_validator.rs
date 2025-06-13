use anyhow::{Result, anyhow};
use serde_json::Value;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Maximum allowed string length for various input types
pub const MAX_NAME_LENGTH: usize = 256;
pub const MAX_CONTENT_LENGTH: usize = 1_048_576; // 1MB
pub const MAX_QUERY_LENGTH: usize = 1024;
pub const MAX_PATH_LENGTH: usize = 4096;
pub const MAX_ARRAY_SIZE: usize = 1000;

/// Input validation errors
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Missing required parameter: {field}")]
    MissingParameter { field: String },
    
    #[error("Invalid parameter type for {field}: expected {expected}")]
    InvalidType { field: String, expected: String },
    
    #[error("Parameter {field} exceeds maximum length of {max_length}")]
    ExceedsMaxLength { field: String, max_length: usize },
    
    #[error("Invalid UUID format for {field}: {error}")]
    InvalidUuid { field: String, error: String },
    
    #[error("Invalid path for {field}: {error}")]
    InvalidPath { field: String, error: String },
    
    #[error("Array {field} exceeds maximum size of {max_size}")]
    ArrayTooLarge { field: String, max_size: usize },
    
    #[error("Invalid value for {field}: {error}")]
    InvalidValue { field: String, error: String },
}

/// Input validator for MCP handlers
pub struct InputValidator;

impl InputValidator {
    /// Validate and extract required string parameter
    pub fn require_string(params: &Value, field: &str) -> Result<String, ValidationError> {
        let value = params.get(field)
            .ok_or_else(|| ValidationError::MissingParameter { field: field.to_string() })?;
        
        let string_value = value.as_str()
            .ok_or_else(|| ValidationError::InvalidType { 
                field: field.to_string(), 
                expected: "string".to_string() 
            })?;
        
        Ok(string_value.to_string())
    }
    
    /// Validate and extract required string with length limit
    pub fn require_string_with_limit(params: &Value, field: &str, max_length: usize) -> Result<String, ValidationError> {
        let value = Self::require_string(params, field)?;
        
        if value.len() > max_length {
            return Err(ValidationError::ExceedsMaxLength { 
                field: field.to_string(), 
                max_length 
            });
        }
        
        Ok(value)
    }
    
    /// Validate and extract optional string parameter
    pub fn optional_string(params: &Value, field: &str) -> Option<String> {
        params.get(field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }
    
    /// Validate and extract optional string with length limit
    pub fn optional_string_with_limit(params: &Value, field: &str, max_length: usize) -> Result<Option<String>, ValidationError> {
        if let Some(value) = Self::optional_string(params, field) {
            if value.len() > max_length {
                return Err(ValidationError::ExceedsMaxLength { 
                    field: field.to_string(), 
                    max_length 
                });
            }
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }
    
    /// Validate and extract UUID parameter
    pub fn require_uuid(params: &Value, field: &str) -> Result<Uuid, ValidationError> {
        let uuid_str = Self::require_string(params, field)?;
        
        Uuid::parse_str(&uuid_str)
            .map_err(|e| ValidationError::InvalidUuid { 
                field: field.to_string(), 
                error: e.to_string() 
            })
    }
    
    /// Validate and extract optional UUID parameter
    pub fn optional_uuid(params: &Value, field: &str) -> Result<Option<Uuid>, ValidationError> {
        if let Some(uuid_str) = Self::optional_string(params, field) {
            let uuid = Uuid::parse_str(&uuid_str)
                .map_err(|e| ValidationError::InvalidUuid { 
                    field: field.to_string(), 
                    error: e.to_string() 
                })?;
            Ok(Some(uuid))
        } else {
            Ok(None)
        }
    }
    
    /// Validate and extract numeric parameter
    pub fn require_number<T>(params: &Value, field: &str) -> Result<T, ValidationError> 
    where 
        T: TryFrom<i64> + TryFrom<u64> + TryFrom<f64>,
        T::Error: std::fmt::Display,
    {
        let value = params.get(field)
            .ok_or_else(|| ValidationError::MissingParameter { field: field.to_string() })?;
        
        if let Some(int_val) = value.as_i64() {
            T::try_from(int_val)
                .map_err(|e| ValidationError::InvalidValue { 
                    field: field.to_string(), 
                    error: e.to_string() 
                })
        } else if let Some(uint_val) = value.as_u64() {
            T::try_from(uint_val)
                .map_err(|e| ValidationError::InvalidValue { 
                    field: field.to_string(), 
                    error: e.to_string() 
                })
        } else if let Some(float_val) = value.as_f64() {
            T::try_from(float_val)
                .map_err(|e| ValidationError::InvalidValue { 
                    field: field.to_string(), 
                    error: e.to_string() 
                })
        } else {
            Err(ValidationError::InvalidType { 
                field: field.to_string(), 
                expected: "number".to_string() 
            })
        }
    }
    
    /// Validate and extract optional numeric parameter
    pub fn optional_number<T>(params: &Value, field: &str) -> Result<Option<T>, ValidationError> 
    where 
        T: TryFrom<i64> + TryFrom<u64> + TryFrom<f64>,
        T::Error: std::fmt::Display,
    {
        if params.get(field).is_some() {
            Ok(Some(Self::require_number(params, field)?))
        } else {
            Ok(None)
        }
    }
    
    /// Validate and extract array parameter with size limit
    pub fn require_array_with_limit(params: &Value, field: &str, max_size: usize) -> Result<Vec<Value>, ValidationError> {
        let value = params.get(field)
            .ok_or_else(|| ValidationError::MissingParameter { field: field.to_string() })?;
        
        let array = value.as_array()
            .ok_or_else(|| ValidationError::InvalidType { 
                field: field.to_string(), 
                expected: "array".to_string() 
            })?;
        
        if array.len() > max_size {
            return Err(ValidationError::ArrayTooLarge { 
                field: field.to_string(), 
                max_size 
            });
        }
        
        Ok(array.clone())
    }
    
    /// Validate and extract optional array parameter with size limit
    pub fn optional_array_with_limit(params: &Value, field: &str, max_size: usize) -> Result<Option<Vec<Value>>, ValidationError> {
        if let Some(value) = params.get(field) {
            let array = value.as_array()
                .ok_or_else(|| ValidationError::InvalidType { 
                    field: field.to_string(), 
                    expected: "array".to_string() 
                })?;
            
            if array.len() > max_size {
                return Err(ValidationError::ArrayTooLarge { 
                    field: field.to_string(), 
                    max_size 
                });
            }
            
            Ok(Some(array.clone()))
        } else {
            Ok(None)
        }
    }
    
    /// Validate file path for security (prevent path traversal)
    pub fn validate_path(params: &Value, field: &str) -> Result<PathBuf, ValidationError> {
        let path_str = Self::require_string_with_limit(params, field, MAX_PATH_LENGTH)?;
        
        // Check for path traversal attempts
        if path_str.contains("..") || path_str.contains("~") {
            return Err(ValidationError::InvalidPath { 
                field: field.to_string(), 
                error: "Path traversal not allowed".to_string() 
            });
        }
        
        // Ensure path is valid
        let path = PathBuf::from(&path_str);
        
        // Additional security checks
        if path.is_absolute() && !path_str.starts_with("/tmp") && !path_str.starts_with("/var/tmp") {
            return Err(ValidationError::InvalidPath { 
                field: field.to_string(), 
                error: "Absolute paths not allowed except in temp directories".to_string() 
            });
        }
        
        Ok(path)
    }
    
    /// Sanitize string input by removing potentially dangerous characters
    pub fn sanitize_string(input: &str) -> String {
        input
            .chars()
            .filter(|c| c.is_alphanumeric() || " .-_/".contains(*c))
            .collect()
    }
    
    /// Validate enum value
    pub fn validate_enum(params: &Value, field: &str, allowed_values: &[&str]) -> Result<String, ValidationError> {
        let value = Self::require_string(params, field)?;
        
        if allowed_values.contains(&value.as_str()) {
            Ok(value)
        } else {
            Err(ValidationError::InvalidValue { 
                field: field.to_string(), 
                error: format!("Must be one of: {}", allowed_values.join(", ")) 
            })
        }
    }
}

/// Convert ValidationError to anyhow::Error for compatibility
impl From<ValidationError> for anyhow::Error {
    fn from(err: ValidationError) -> Self {
        anyhow!(err.to_string())
    }
}

use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;

use kg_mcp_server::{
    ServerConfig,
    GraphStorage,
    LocalEmbeddingEngine,
    HybridSearchEngine,
    MemoryOptimizer,
};

/// Test the health check endpoint functionality
#[cfg(test)]
mod health_check_tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check_basic() -> Result<()> {
        // This test verifies that the health check components work
        // without requiring a full HTTP server setup
        
        let config = ServerConfig::default();
        
        // Test database health check
        let storage = GraphStorage::new("test_health.db", &config.database)?;
        let node_count = storage.count_nodes().await?;
        
        // Should start with 0 nodes
        assert_eq!(node_count, 0);
        
        // Clean up
        std::fs::remove_file("test_health.db").ok();
        
        Ok(())
    }

    #[tokio::test]
    async fn test_database_metrics() -> Result<()> {
        let config = ServerConfig::default();
        let storage = GraphStorage::new("test_metrics.db", &config.database)?;
        
        // Test initial counts
        let node_count = storage.count_nodes().await?;
        let edge_count = storage.count_edges().await?;
        let episode_count = storage.count_episodes().await?;
        
        assert_eq!(node_count, 0);
        assert_eq!(edge_count, 0);
        assert_eq!(episode_count, 0);
        
        // Clean up
        std::fs::remove_file("test_metrics.db").ok();
        
        Ok(())
    }

    #[tokio::test]
    async fn test_server_config_validation() -> Result<()> {
        let config = ServerConfig::default();
        
        // Test that configuration validation works
        config.validate()?;
        
        // Test security config conversion
        let auth_config = config.to_auth_config();
        assert!(!auth_config.enabled); // Should be disabled by default
        
        Ok(())
    }

    #[tokio::test]
    async fn test_health_check_with_data() -> Result<()> {
        use kg_mcp_server::graph::{KGNode, Episode, EpisodeSource};
        
        let config = ServerConfig::default();
        let storage = GraphStorage::new("test_health_data.db", &config.database)?;
        
        // Add some test data
        let node = KGNode::new(
            "Test Node".to_string(),
            "TestType".to_string(),
            "Test summary".to_string(),
            Some("test_group".to_string()),
        );
        
        let episode = Episode::new(
            "Test Episode".to_string(),
            "Test content".to_string(),
            EpisodeSource::Text,
            "Test source".to_string(),
            Some("test_group".to_string()),
        );
        
        storage.store_node(&node).await?;
        storage.store_episode(&episode).await?;
        
        // Verify counts
        let node_count = storage.count_nodes().await?;
        let episode_count = storage.count_episodes().await?;
        
        assert_eq!(node_count, 1);
        assert_eq!(episode_count, 1);
        
        // Clean up
        std::fs::remove_file("test_health_data.db").ok();
        
        Ok(())
    }
}

/// Test input validation functionality
#[cfg(test)]
mod validation_tests {
    use super::*;
    use kg_mcp_server::validation::{InputValidator, ValidationError};
    use serde_json::json;

    #[test]
    fn test_string_validation() {
        let params = json!({
            "name": "test_name",
            "missing_field": null
        });
        
        // Test required string
        let result = InputValidator::require_string(&params, "name");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test_name");
        
        // Test missing required string
        let result = InputValidator::require_string(&params, "missing");
        assert!(result.is_err());
        
        // Test optional string
        let result = InputValidator::optional_string(&params, "name");
        assert_eq!(result, Some("test_name".to_string()));
        
        let result = InputValidator::optional_string(&params, "missing");
        assert_eq!(result, None);
    }

    #[test]
    fn test_string_length_validation() {
        let params = json!({
            "short": "ok",
            "long": "this is a very long string that exceeds the limit"
        });
        
        // Test within limit
        let result = InputValidator::require_string_with_limit(&params, "short", 10);
        assert!(result.is_ok());
        
        // Test exceeds limit
        let result = InputValidator::require_string_with_limit(&params, "long", 10);
        assert!(result.is_err());
        
        if let Err(ValidationError::ExceedsMaxLength { field, max_length }) = result {
            assert_eq!(field, "long");
            assert_eq!(max_length, 10);
        } else {
            panic!("Expected ExceedsMaxLength error");
        }
    }

    #[test]
    fn test_uuid_validation() {
        let params = json!({
            "valid_uuid": "550e8400-e29b-41d4-a716-446655440000",
            "invalid_uuid": "not-a-uuid"
        });
        
        // Test valid UUID
        let result = InputValidator::require_uuid(&params, "valid_uuid");
        assert!(result.is_ok());
        
        // Test invalid UUID
        let result = InputValidator::require_uuid(&params, "invalid_uuid");
        assert!(result.is_err());
        
        if let Err(ValidationError::InvalidUuid { field, .. }) = result {
            assert_eq!(field, "invalid_uuid");
        } else {
            panic!("Expected InvalidUuid error");
        }
    }

    #[test]
    fn test_array_validation() {
        let params = json!({
            "small_array": [1, 2, 3],
            "large_array": (0..1001).collect::<Vec<i32>>()
        });
        
        // Test within limit
        let result = InputValidator::require_array_with_limit(&params, "small_array", 10);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 3);
        
        // Test exceeds limit
        let result = InputValidator::require_array_with_limit(&params, "large_array", 1000);
        assert!(result.is_err());
        
        if let Err(ValidationError::ArrayTooLarge { field, max_size }) = result {
            assert_eq!(field, "large_array");
            assert_eq!(max_size, 1000);
        } else {
            panic!("Expected ArrayTooLarge error");
        }
    }

    #[test]
    fn test_path_validation() {
        let params = json!({
            "safe_path": "documents/file.txt",
            "traversal_path": "../../../etc/passwd",
            "absolute_path": "/etc/passwd"
        });
        
        // Test safe relative path
        let result = InputValidator::validate_path(&params, "safe_path");
        assert!(result.is_ok());
        
        // Test path traversal attempt
        let result = InputValidator::validate_path(&params, "traversal_path");
        assert!(result.is_err());
        
        // Test absolute path (should be rejected)
        let result = InputValidator::validate_path(&params, "absolute_path");
        assert!(result.is_err());
    }

    #[test]
    fn test_enum_validation() {
        let params = json!({
            "valid_enum": "option1",
            "invalid_enum": "invalid_option"
        });
        
        let allowed_values = &["option1", "option2", "option3"];
        
        // Test valid enum value
        let result = InputValidator::validate_enum(&params, "valid_enum", allowed_values);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "option1");
        
        // Test invalid enum value
        let result = InputValidator::validate_enum(&params, "invalid_enum", allowed_values);
        assert!(result.is_err());
    }

    #[test]
    fn test_string_sanitization() {
        let input = "Hello, World! <script>alert('xss')</script>";
        let sanitized = InputValidator::sanitize_string(input);
        
        // Should only contain alphanumeric and allowed characters
        assert!(!sanitized.contains("<"));
        assert!(!sanitized.contains(">"));
        assert!(sanitized.contains("Hello"));
        assert!(sanitized.contains("World"));
    }
}

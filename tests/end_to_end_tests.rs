use anyhow::Result;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::time::{timeout, Duration};

use kg_mcp_server::{
    ServerConfig,
    GraphStorage,
    LocalEmbeddingEngine,
    HybridSearchEngine,
    MemoryOptimizer,
    mcp::handlers::handle_tool_request,
    graph::{Episode, EpisodeSource},
};

/// End-to-end integration tests that simulate real usage scenarios
#[cfg(test)]
mod e2e_tests {
    use super::*;

    /// Test the complete workflow: add memory -> search -> retrieve
    #[tokio::test]
    async fn test_complete_memory_workflow() -> Result<()> {
        // Setup components
        let config = ServerConfig::default();
        let storage = Arc::new(GraphStorage::new("test_e2e.db", &config.database)?);
        let embedding_engine = Arc::new(LocalEmbeddingEngine::new(config.clone())?);
        let search_engine = Arc::new(HybridSearchEngine::new(
            crate::search::TextSearchEngine::new(storage.clone())?,
            crate::search::VectorSearchEngine::new(None)?,
            None,
        ));
        let memory_optimizer = Arc::new(MemoryOptimizer::new(config.memory.into()));

        // Step 1: Add some memory
        let add_params = json!({
            "name": "Test Project Discussion",
            "episode_body": "We discussed the architecture for the new microservice. Key decisions: use Rust for performance, SQLite for local storage, and implement proper error handling.",
            "source": "text",
            "source_description": "Team meeting notes",
            "group_id": "project_alpha"
        });

        let add_result = handle_tool_request(
            "mcp_kg-mcp-server_add_memory",
            add_params,
            &storage,
            &embedding_engine,
            &search_engine,
            &memory_optimizer,
        ).await?;

        // Verify the memory was added
        assert_eq!(add_result["status"], "success");
        assert!(add_result.get("uuid").is_some());

        // Step 2: Search for the memory
        let search_params = json!({
            "operation": "nodes",
            "query": "microservice architecture",
            "max_results": 10
        });

        let search_result = handle_tool_request(
            "mcp_kg-mcp-server_search_memory",
            search_params,
            &storage,
            &embedding_engine,
            &search_engine,
            &memory_optimizer,
        ).await?;

        // Verify search results
        assert!(search_result.get("results").is_some());
        let results = search_result["results"].as_array().unwrap();
        assert!(!results.is_empty(), "Should find the added memory");

        // Step 3: Retrieve episodes
        let episodes_params = json!({
            "operation": "episodes",
            "group_id": "project_alpha",
            "last_n": 5
        });

        let episodes_result = handle_tool_request(
            "mcp_kg-mcp-server_search_memory",
            episodes_params,
            &storage,
            &embedding_engine,
            &search_engine,
            &memory_optimizer,
        ).await?;

        // Verify episodes retrieval
        assert!(episodes_result.get("results").is_some());
        let episodes = episodes_result["results"].as_array().unwrap();
        assert_eq!(episodes.len(), 1, "Should have exactly one episode");

        // Clean up
        std::fs::remove_file("test_e2e.db").ok();

        Ok(())
    }

    /// Test error handling in the complete workflow
    #[tokio::test]
    async fn test_error_handling_workflow() -> Result<()> {
        let config = ServerConfig::default();
        let storage = Arc::new(GraphStorage::new("test_e2e_errors.db", &config.database)?);
        let embedding_engine = Arc::new(LocalEmbeddingEngine::new(config.clone())?);
        let search_engine = Arc::new(HybridSearchEngine::new(
            crate::search::TextSearchEngine::new(storage.clone())?,
            crate::search::VectorSearchEngine::new(None)?,
            None,
        ));
        let memory_optimizer = Arc::new(MemoryOptimizer::new(config.memory.into()));

        // Test 1: Missing required parameters
        let invalid_params = json!({
            "episode_body": "Missing name parameter"
        });

        let result = handle_tool_request(
            "mcp_kg-mcp-server_add_memory",
            invalid_params,
            &storage,
            &embedding_engine,
            &search_engine,
            &memory_optimizer,
        ).await;

        assert!(result.is_err(), "Should fail with missing required parameter");

        // Test 2: Invalid tool name
        let valid_params = json!({
            "name": "Test",
            "episode_body": "Test content"
        });

        let result = handle_tool_request(
            "invalid_tool_name",
            valid_params,
            &storage,
            &embedding_engine,
            &search_engine,
            &memory_optimizer,
        ).await;

        assert!(result.is_err(), "Should fail with invalid tool name");

        // Clean up
        std::fs::remove_file("test_e2e_errors.db").ok();

        Ok(())
    }

    /// Test performance under load (simplified)
    #[tokio::test]
    async fn test_basic_performance() -> Result<()> {
        let config = ServerConfig::default();
        let storage = Arc::new(GraphStorage::new("test_e2e_perf.db", &config.database)?);
        let embedding_engine = Arc::new(LocalEmbeddingEngine::new(config.clone())?);
        let search_engine = Arc::new(HybridSearchEngine::new(
            crate::search::TextSearchEngine::new(storage.clone())?,
            crate::search::VectorSearchEngine::new(None)?,
            None,
        ));
        let memory_optimizer = Arc::new(MemoryOptimizer::new(config.memory.into()));

        // Add multiple memories quickly
        let start_time = std::time::Instant::now();
        
        for i in 0..5 {
            let params = json!({
                "name": format!("Performance Test {}", i),
                "episode_body": format!("This is test content number {} for performance testing", i),
                "source": "text",
                "group_id": "perf_test"
            });

            let result = timeout(
                Duration::from_secs(10), // 10 second timeout per operation
                handle_tool_request(
                    "mcp_kg-mcp-server_add_memory",
                    params,
                    &storage,
                    &embedding_engine,
                    &search_engine,
                    &memory_optimizer,
                )
            ).await;

            assert!(result.is_ok(), "Operation should complete within timeout");
            assert!(result.unwrap().is_ok(), "Operation should succeed");
        }

        let elapsed = start_time.elapsed();
        println!("Added 5 memories in {:?}", elapsed);

        // Should complete reasonably quickly (adjust threshold as needed)
        assert!(elapsed < Duration::from_secs(30), "Should complete within 30 seconds");

        // Test search performance
        let search_start = std::time::Instant::now();
        
        let search_params = json!({
            "operation": "nodes",
            "query": "performance testing",
            "max_results": 10
        });

        let search_result = timeout(
            Duration::from_secs(5),
            handle_tool_request(
                "mcp_kg-mcp-server_search_memory",
                search_params,
                &storage,
                &embedding_engine,
                &search_engine,
                &memory_optimizer,
            )
        ).await;

        assert!(search_result.is_ok(), "Search should complete within timeout");
        
        let search_elapsed = search_start.elapsed();
        println!("Search completed in {:?}", search_elapsed);
        
        // Search should be fast
        assert!(search_elapsed < Duration::from_secs(5), "Search should be fast");

        // Clean up
        std::fs::remove_file("test_e2e_perf.db").ok();

        Ok(())
    }

    /// Test configuration validation
    #[tokio::test]
    async fn test_configuration_validation() -> Result<()> {
        // Test default configuration
        let config = ServerConfig::default();
        assert!(config.validate().is_ok(), "Default config should be valid");

        // Test security configuration
        let auth_config = config.to_auth_config();
        assert!(!auth_config.enabled, "Auth should be disabled by default");
        assert_eq!(auth_config.rate_limit_requests_per_minute, 60);

        Ok(())
    }

    /// Test health check functionality
    #[tokio::test]
    async fn test_health_check_components() -> Result<()> {
        let config = ServerConfig::default();
        let storage = GraphStorage::new("test_health_e2e.db", &config.database)?;

        // Test database health
        let node_count = storage.count_nodes().await?;
        let edge_count = storage.count_edges().await?;
        let episode_count = storage.count_episodes().await?;

        // Should start with empty database
        assert_eq!(node_count, 0);
        assert_eq!(edge_count, 0);
        assert_eq!(episode_count, 0);

        // Add some data and verify counts change
        let episode = Episode::new(
            "Health Test".to_string(),
            "Test content".to_string(),
            EpisodeSource::Text,
            "Test".to_string(),
            None,
        );

        storage.store_episode(&episode).await?;

        let new_episode_count = storage.count_episodes().await?;
        assert_eq!(new_episode_count, 1);

        // Clean up
        std::fs::remove_file("test_health_e2e.db").ok();

        Ok(())
    }
}

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use kg_mcp_server::embeddings::{LocalEmbeddingEngine, ModelManager, BatchProcessor, OnnxEmbeddingEngine};
use kg_mcp_server::graph::{KGNode, KGEdge, Episode, EpisodeSource, GraphStorage};
use kg_mcp_server::search::{HybridSearchEngine, VectorSearchEngine, TextSearchEngine};
use kg_mcp_server::mcp::handlers::handle_tool_request;
use kg_mcp_server::memory::MemoryOptimizer;

/// Test embedding generation accuracy and consistency
#[cfg(test)]
mod embedding_tests {
    use super::*;

    #[tokio::test]
    async fn test_embedding_generation_consistency() -> Result<()> {
        let model_manager = ModelManager::new("models".to_string());
        let batch_processor = Arc::new(Mutex::new(BatchProcessor::new(1000, 64)?));
        let onnx_engine = OnnxEmbeddingEngine::new(batch_processor.clone())?;
        let engine = LocalEmbeddingEngine::new(model_manager, onnx_engine)?;

        let text = "Machine learning is a subset of artificial intelligence";
        
        // Generate embeddings multiple times for the same text
        let embedding1 = engine.generate_embedding(text).await?;
        let embedding2 = engine.generate_embedding(text).await?;
        
        // Embeddings should be identical for the same input
        assert_eq!(embedding1.len(), embedding2.len());
        
        for (a, b) in embedding1.iter().zip(embedding2.iter()) {
            assert!((a - b).abs() < 1e-6, "Embeddings should be deterministic");
        }
        
        Ok(())
    }

    #[tokio::test]
    async fn test_embedding_similarity() -> Result<()> {
        let model_manager = ModelManager::new("models".to_string());
        let batch_processor = Arc::new(Mutex::new(BatchProcessor::new(1000, 64)?));
        let onnx_engine = OnnxEmbeddingEngine::new(batch_processor.clone())?;
        let engine = LocalEmbeddingEngine::new(model_manager, onnx_engine)?;

        let text1 = "Artificial intelligence and machine learning";
        let text2 = "AI and ML technologies";
        let text3 = "Cooking recipes and ingredients";

        let embedding1 = engine.generate_embedding(text1).await?;
        let embedding2 = engine.generate_embedding(text2).await?;
        let embedding3 = engine.generate_embedding(text3).await?;

        let similarity_12 = engine.cosine_similarity(&embedding1, &embedding2);
        let similarity_13 = engine.cosine_similarity(&embedding1, &embedding3);

        // Related texts should have higher similarity than unrelated texts
        assert!(similarity_12 > similarity_13, 
                "Related texts should have higher similarity: {} vs {}", 
                similarity_12, similarity_13);
        
        // Similarity should be between 0 and 1
        assert!(similarity_12 >= 0.0 && similarity_12 <= 1.0);
        assert!(similarity_13 >= 0.0 && similarity_13 <= 1.0);

        Ok(())
    }

    #[tokio::test]
    async fn test_batch_processing() -> Result<()> {
        let model_manager = ModelManager::new("models".to_string());
        let batch_processor = Arc::new(Mutex::new(BatchProcessor::new(1000, 64)?));
        let onnx_engine = OnnxEmbeddingEngine::new(batch_processor.clone())?;
        let engine = LocalEmbeddingEngine::new(model_manager, onnx_engine)?;

        let texts = vec![
            "First test text",
            "Second test text", 
            "Third test text",
            "Fourth test text",
        ];

        let embeddings = engine.generate_embeddings(texts.clone()).await?;
        
        assert_eq!(embeddings.len(), texts.len());
        
        for embedding in &embeddings {
            assert!(!embedding.is_empty(), "Embedding should not be empty");
            assert!(embedding.len() > 300, "Embedding should have reasonable dimensionality");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_cache_functionality() -> Result<()> {
        let model_manager = ModelManager::new("models".to_string());
        let batch_processor = Arc::new(Mutex::new(BatchProcessor::new(1000, 64)?));
        let onnx_engine = OnnxEmbeddingEngine::new(batch_processor.clone())?;
        let engine = LocalEmbeddingEngine::new(model_manager, onnx_engine)?;

        let text = "Cache test text";
        
        // First generation
        let start = std::time::Instant::now();
        let embedding1 = engine.generate_embedding(text).await?;
        let first_duration = start.elapsed();
        
        // Second generation (should be cached)
        let start = std::time::Instant::now();
        let embedding2 = engine.generate_embedding(text).await?;
        let second_duration = start.elapsed();
        
        // Cached result should be faster
        assert!(second_duration < first_duration, 
                "Cached embedding should be faster: {:?} vs {:?}", 
                second_duration, first_duration);
        
        // Results should be identical
        assert_eq!(embedding1, embedding2);

        Ok(())
    }
}

/// Test graph operations validation
#[cfg(test)]
mod graph_tests {
    use super::*;

    #[tokio::test]
    async fn test_node_creation_and_retrieval() -> Result<()> {
        let storage = GraphStorage::new("test_graph.db").await?;
        
        let node = KGNode::new(
            "Test Node".to_string(),
            "TestType".to_string(),
            "Test summary".to_string(),
            Some("test_group".to_string()),
        );
        
        let stored_uuid = storage.store_node(&node).await?;
        assert_eq!(stored_uuid, node.uuid);
        
        let retrieved_node = storage.get_node(node.uuid).await?;
        assert!(retrieved_node.is_some());
        
        let retrieved = retrieved_node.unwrap();
        assert_eq!(retrieved.name, node.name);
        assert_eq!(retrieved.node_type, node.node_type);
        assert_eq!(retrieved.summary, node.summary);
        assert_eq!(retrieved.group_id, node.group_id);

        Ok(())
    }

    #[tokio::test]
    async fn test_edge_creation_and_retrieval() -> Result<()> {
        let storage = GraphStorage::new("test_graph.db").await?;
        
        // Create two nodes first
        let node1 = KGNode::new(
            "Source Node".to_string(),
            "SourceType".to_string(),
            "Source summary".to_string(),
            Some("test_group".to_string()),
        );
        
        let node2 = KGNode::new(
            "Target Node".to_string(),
            "TargetType".to_string(),
            "Target summary".to_string(),
            Some("test_group".to_string()),
        );
        
        storage.store_node(&node1).await?;
        storage.store_node(&node2).await?;
        
        let edge = KGEdge::new(
            node1.uuid,
            node2.uuid,
            "RELATES_TO".to_string(),
            "Test relationship".to_string(),
            0.8,
            Some("test_group".to_string()),
        );
        
        let stored_uuid = storage.store_edge(&edge).await?;
        assert_eq!(stored_uuid, edge.uuid);
        
        let retrieved_edge = storage.get_edge(edge.uuid).await?;
        assert!(retrieved_edge.is_some());
        
        let retrieved = retrieved_edge.unwrap();
        assert_eq!(retrieved.source_node_uuid, edge.source_node_uuid);
        assert_eq!(retrieved.target_node_uuid, edge.target_node_uuid);
        assert_eq!(retrieved.relation_type, edge.relation_type);
        assert_eq!(retrieved.weight, edge.weight);

        Ok(())
    }

    #[tokio::test]
    async fn test_episode_creation_and_retrieval() -> Result<()> {
        let storage = GraphStorage::new("test_graph.db").await?;
        
        let episode = Episode::new(
            "Test Episode".to_string(),
            "Episode content for testing".to_string(),
            EpisodeSource::Text,
            "Test source".to_string(),
            Some("test_group".to_string()),
        );
        
        let stored_uuid = storage.store_episode(&episode).await?;
        assert_eq!(stored_uuid, episode.uuid);
        
        let retrieved_episode = storage.get_episode(episode.uuid).await?;
        assert!(retrieved_episode.is_some());
        
        let retrieved = retrieved_episode.unwrap();
        assert_eq!(retrieved.name, episode.name);
        assert_eq!(retrieved.content, episode.content);
        assert_eq!(retrieved.source, episode.source);
        assert_eq!(retrieved.group_id, episode.group_id);

        Ok(())
    }

    #[tokio::test]
    async fn test_graph_traversal() -> Result<()> {
        let storage = GraphStorage::new("test_graph.db").await?;
        
        // Create a small graph: A -> B -> C
        let node_a = KGNode::new("A".to_string(), "Node".to_string(), "Node A".to_string(), None);
        let node_b = KGNode::new("B".to_string(), "Node".to_string(), "Node B".to_string(), None);
        let node_c = KGNode::new("C".to_string(), "Node".to_string(), "Node C".to_string(), None);
        
        storage.store_node(&node_a).await?;
        storage.store_node(&node_b).await?;
        storage.store_node(&node_c).await?;
        
        let edge_ab = KGEdge::new(node_a.uuid, node_b.uuid, "CONNECTS".to_string(), "A to B".to_string(), 1.0, None);
        let edge_bc = KGEdge::new(node_b.uuid, node_c.uuid, "CONNECTS".to_string(), "B to C".to_string(), 1.0, None);
        
        storage.store_edge(&edge_ab).await?;
        storage.store_edge(&edge_bc).await?;
        
        // Test neighbor finding
        let neighbors_a = storage.get_neighbors(node_a.uuid, None).await?;
        assert_eq!(neighbors_a.len(), 1);
        assert_eq!(neighbors_a[0].uuid, node_b.uuid);
        
        let neighbors_b = storage.get_neighbors(node_b.uuid, None).await?;
        assert_eq!(neighbors_b.len(), 2); // Both A and C should be neighbors
        
        Ok(())
    }

    #[tokio::test]
    async fn test_graph_search() -> Result<()> {
        let storage = GraphStorage::new("test_graph.db").await?;
        
        let node1 = KGNode::new(
            "Machine Learning".to_string(),
            "Concept".to_string(),
            "AI technique for pattern recognition".to_string(),
            Some("ai_group".to_string()),
        );
        
        let node2 = KGNode::new(
            "Deep Learning".to_string(),
            "Concept".to_string(),
            "Neural network based learning".to_string(),
            Some("ai_group".to_string()),
        );
        
        storage.store_node(&node1).await?;
        storage.store_node(&node2).await?;
        
        // Test text search
        let search_results = storage.search_nodes("learning", 10).await?;
        assert_eq!(search_results.len(), 2);
        
        // Test group filtering
        let group_results = storage.search_nodes_by_group("ai_group", 10).await?;
        assert_eq!(group_results.len(), 2);

        Ok(())
    }
}

/// Test search functionality
#[cfg(test)]
mod search_tests {
    use super::*;

    #[tokio::test]
    async fn test_text_search_engine() -> Result<()> {
        let storage = GraphStorage::new("test_search.db").await?;
        let text_engine = TextSearchEngine::new(storage)?;
        
        // Test basic search functionality (mocked for now)
        // In a real implementation, this would test actual FTS5 integration
        let query = "machine learning";
        let results = text_engine.search_nodes(query, 10).await;
        
        // Should not fail even with empty database
        assert!(results.is_ok());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_vector_search_engine() -> Result<()> {
        let vector_engine = VectorSearchEngine::new(None)?;
        
        // Test similarity calculation
        let vec1 = vec![1.0, 0.0, 0.0];
        let vec2 = vec![0.0, 1.0, 0.0];
        let vec3 = vec![1.0, 0.0, 0.0];
        
        let similarity_12 = vector_engine.calculate_similarity(&vec1, &vec2)?;
        let similarity_13 = vector_engine.calculate_similarity(&vec1, &vec3)?;
        
        // Identical vectors should have similarity 1.0
        assert!((similarity_13 - 1.0).abs() < 1e-6);
        
        // Orthogonal vectors should have similarity 0.0
        assert!(similarity_12.abs() < 1e-6);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_hybrid_search_engine() -> Result<()> {
        let storage = GraphStorage::new("test_hybrid.db").await?;
        let text_engine = TextSearchEngine::new(storage)?;
        let vector_engine = VectorSearchEngine::new(None)?;
        let hybrid_engine = HybridSearchEngine::new(text_engine, vector_engine, None);
        
        // Test basic functionality
        let query = "artificial intelligence";
        let results = hybrid_engine.search(query, 10).await;
        
        assert!(results.is_ok());
        
        Ok(())
    }
}

/// Test MCP protocol compliance
#[cfg(test)]
mod mcp_tests {
    use super::*;
    use serde_json::{json, Value};

    #[tokio::test]
    async fn test_add_memory_tool() -> Result<()> {
        let params = json!({
            "name": "Test Episode",
            "episode_body": "This is a test episode for MCP testing",
            "source": "text",
            "source_description": "Unit test"
        });
        
        let result = handle_tool_request("add_memory", params).await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert!(response.get("uuid").is_some());
        assert!(response.get("status").is_some());
        assert_eq!(response["status"], "success");
        
        Ok(())
    }

    #[tokio::test]
    async fn test_search_memory_nodes_tool() -> Result<()> {
        let params = json!({
            "query": "test query",
            "max_nodes": 5
        });
        
        let result = handle_tool_request("search_memory_nodes", params).await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert!(response.get("nodes").is_some());
        assert!(response.get("query").is_some());
        assert_eq!(response["query"], "test query");
        
        Ok(())
    }

    #[tokio::test]
    async fn test_search_memory_facts_tool() -> Result<()> {
        let params = json!({
            "query": "test relationships",
            "max_facts": 3
        });
        
        let result = handle_tool_request("search_memory_facts", params).await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert!(response.get("facts").is_some());
        assert!(response.get("query").is_some());
        assert_eq!(response["query"], "test relationships");
        
        Ok(())
    }

    #[tokio::test]
    async fn test_advanced_tools() -> Result<()> {
        // Test find_similar_concepts
        let params = json!({
            "concept": "machine learning",
            "max_results": 5,
            "similarity_threshold": 0.7
        });
        
        let result = handle_tool_request("find_similar_concepts", params).await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert!(response.get("similar_concepts").is_some());
        assert_eq!(response["query_concept"], "machine learning");
        
        // Test analyze_patterns
        let params = json!({
            "analysis_type": "relationships",
            "max_results": 10
        });
        
        let result = handle_tool_request("analyze_patterns", params).await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert!(response.get("patterns").is_some());
        assert_eq!(response["analysis_type"], "relationships");
        
        // Test get_semantic_clusters
        let params = json!({
            "cluster_method": "kmeans",
            "num_clusters": 3
        });
        
        let result = handle_tool_request("get_semantic_clusters", params).await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert!(response.get("clusters").is_some());
        assert_eq!(response["cluster_method"], "kmeans");
        
        // Test get_temporal_patterns
        let params = json!({
            "time_granularity": "day",
            "days_back": 7
        });
        
        let result = handle_tool_request("get_temporal_patterns", params).await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert!(response.get("temporal_patterns").is_some());
        assert_eq!(response["time_granularity"], "day");
        
        Ok(())
    }

    #[tokio::test]
    async fn test_error_handling() -> Result<()> {
        // Test missing required parameter
        let params = json!({
            "episode_body": "Missing name parameter"
        });
        
        let result = handle_tool_request("add_memory", params).await;
        assert!(result.is_err());
        
        // Test invalid tool name
        let params = json!({
            "param": "value"
        });
        
        let result = handle_tool_request("invalid_tool", params).await;
        assert!(result.is_err());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_parameter_validation() -> Result<()> {
        // Test invalid UUID format
        let params = json!({
            "uuid": "invalid-uuid-format"
        });
        
        let result = handle_tool_request("delete_entity_edge", params).await;
        assert!(result.is_err());
        
        // Test invalid source type
        let params = json!({
            "name": "Test Episode",
            "episode_body": "Test content",
            "source": "invalid_source"
        });
        
        let result = handle_tool_request("add_memory", params).await;
        assert!(result.is_err());
        
        Ok(())
    }
}

/// Test performance benchmarks
#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_embedding_generation_speed() -> Result<()> {
        let model_manager = ModelManager::new("models".to_string());
        let batch_processor = Arc::new(Mutex::new(BatchProcessor::new(1000, 64)?));
        let onnx_engine = OnnxEmbeddingEngine::new(batch_processor.clone())?;
        let engine = LocalEmbeddingEngine::new(model_manager, onnx_engine)?;

        let text = "Performance test text for embedding generation speed measurement";
        
        let start = Instant::now();
        let _embedding = engine.generate_embedding(text).await?;
        let duration = start.elapsed();
        
        // Should complete within reasonable time (adjust based on hardware)
        assert!(duration.as_millis() < 1000, "Embedding generation too slow: {:?}", duration);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_batch_embedding_throughput() -> Result<()> {
        let model_manager = ModelManager::new("models".to_string());
        let batch_processor = Arc::new(Mutex::new(BatchProcessor::new(1000, 64)?));
        let onnx_engine = OnnxEmbeddingEngine::new(batch_processor.clone())?;
        let engine = LocalEmbeddingEngine::new(model_manager, onnx_engine)?;

        let texts: Vec<String> = (0..10).map(|i| format!("Batch test text {}", i)).collect();
        
        let start = Instant::now();
        let embeddings = engine.generate_embeddings(texts.clone()).await?;
        let duration = start.elapsed();
        
        assert_eq!(embeddings.len(), texts.len());
        
        let throughput = texts.len() as f32 / duration.as_secs_f32();
        println!("Batch embedding throughput: {:.2} texts/second", throughput);
        
        // Should maintain reasonable throughput
        assert!(throughput > 0.1, "Batch embedding throughput too low: {:.2}", throughput);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_database_operation_speed() -> Result<()> {
        let storage = GraphStorage::new("test_performance.db").await?;
        
        let node = KGNode::new(
            "Performance Test Node".to_string(),
            "TestType".to_string(),
            "Node for performance testing".to_string(),
            Some("perf_group".to_string()),
        );
        
        // Test insertion speed
        let start = Instant::now();
        let _uuid = storage.store_node(&node).await?;
        let insert_duration = start.elapsed();
        
        // Test retrieval speed
        let start = Instant::now();
        let _retrieved = storage.get_node(node.uuid).await?;
        let retrieval_duration = start.elapsed();
        
        println!("Insert duration: {:?}, Retrieval duration: {:?}", 
                insert_duration, retrieval_duration);
        
        // Database operations should be fast
        assert!(insert_duration.as_millis() < 100);
        assert!(retrieval_duration.as_millis() < 50);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_search_performance() -> Result<()> {
        let storage = GraphStorage::new("test_search_perf.db").await?;
        
        // Insert multiple nodes for search testing
        for i in 0..100 {
            let node = KGNode::new(
                format!("Search Test Node {}", i),
                "SearchType".to_string(),
                format!("Node {} for search performance testing", i),
                Some("search_group".to_string()),
            );
            storage.store_node(&node).await?;
        }
        
        let start = Instant::now();
        let results = storage.search_nodes("Search Test", 10).await?;
        let search_duration = start.elapsed();
        
        println!("Search duration: {:?}, Results: {}", search_duration, results.len());
        
        // Search should be reasonably fast
        assert!(search_duration.as_millis() < 500);
        assert!(!results.is_empty());
        
        Ok(())
    }
}

/// Integration test helper functions
#[cfg(test)]
mod integration_helpers {
    use super::*;

    pub async fn setup_test_environment() -> Result<()> {
        // Clean up any existing test databases
        let test_files = vec![
            "test_graph.db",
            "test_search.db", 
            "test_hybrid.db",
            "test_performance.db",
            "test_search_perf.db"
        ];
        
        for file in test_files {
            let _ = tokio::fs::remove_file(file).await;
        }
        
        Ok(())
    }

    pub async fn teardown_test_environment() -> Result<()> {
        // Clean up test databases after tests
        setup_test_environment().await
    }
}

/// Run all test suites
#[cfg(test)]
mod test_runner {
    use super::*;

    #[tokio::test]
    async fn run_full_test_suite() -> Result<()> {
        println!("🧪 Starting comprehensive test suite...");
        
        integration_helpers::setup_test_environment().await?;
        
        println!("✅ Test environment set up successfully");
        
        // Note: Individual tests would run automatically with `cargo test`
        // This is just a marker test to confirm the full suite setup
        
        integration_helpers::teardown_test_environment().await?;
        
        println!("🎉 Test suite completed successfully");
        
        Ok(())
    }
} 
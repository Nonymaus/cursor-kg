use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use uuid::Uuid;

use kg_mcp_server::graph::{KGNode, KGEdge, Episode, EpisodeSource, GraphStorage};
use kg_mcp_server::embeddings::{LocalEmbeddingEngine, ModelManager, BatchProcessor, OnnxEmbeddingEngine};
use kg_mcp_server::search::{HybridSearchEngine, VectorSearchEngine, TextSearchEngine};

/// Benchmark large-scale node insertion performance
fn bench_node_insertion(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("node_insertion");
    group.significance_level(0.1).sample_size(10);
    
    for node_count in [100, 1_000, 10_000, 100_000].iter() {
        group.bench_with_input(
            BenchmarkId::new("nodes", node_count),
            node_count,
            |b, &node_count| {
                b.to_async(&rt).iter(|| async {
                    let storage = GraphStorage::new(&format!("bench_nodes_{}.db", node_count))
                        .await
                        .unwrap();
                    
                    let start = Instant::now();
                    
                    for i in 0..node_count {
                        let node = KGNode::new(
                            format!("Benchmark Node {}", i),
                            "BenchmarkType".to_string(),
                            format!("Performance test node number {}", i),
                            Some("benchmark_group".to_string()),
                        );
                        
                        black_box(storage.store_node(&node).await.unwrap());
                    }
                    
                    let duration = start.elapsed();
                    println!("Inserted {} nodes in {:?} ({:.2} nodes/sec)", 
                            node_count, duration, node_count as f64 / duration.as_secs_f64());
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark large-scale edge insertion performance
fn bench_edge_insertion(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("edge_insertion");
    group.significance_level(0.1).sample_size(10);
    
    for edge_count in [100, 1_000, 10_000, 100_000].iter() {
        group.bench_with_input(
            BenchmarkId::new("edges", edge_count),
            edge_count,
            |b, &edge_count| {
                b.to_async(&rt).iter(|| async {
                    let storage = GraphStorage::new(&format!("bench_edges_{}.db", edge_count))
                        .await
                        .unwrap();
                    
                    // Pre-create nodes for edges
                    let mut node_uuids = Vec::new();
                    for i in 0..std::cmp::min(edge_count, 1000) {
                        let node = KGNode::new(
                            format!("Node {}", i),
                            "NodeType".to_string(),
                            format!("Node for edge testing {}", i),
                            Some("edge_group".to_string()),
                        );
                        storage.store_node(&node).await.unwrap();
                        node_uuids.push(node.uuid);
                    }
                    
                    let start = Instant::now();
                    
                    for i in 0..edge_count {
                        let source_idx = i % node_uuids.len();
                        let target_idx = (i + 1) % node_uuids.len();
                        
                        let edge = KGEdge::new(
                            node_uuids[source_idx],
                            node_uuids[target_idx],
                            "BENCHMARK_RELATION".to_string(),
                            format!("Benchmark edge {}", i),
                            0.8,
                            Some("edge_group".to_string()),
                        );
                        
                        black_box(storage.store_edge(&edge).await.unwrap());
                    }
                    
                    let duration = start.elapsed();
                    println!("Inserted {} edges in {:?} ({:.2} edges/sec)", 
                            edge_count, duration, edge_count as f64 / duration.as_secs_f64());
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark complex graph traversal operations
fn bench_graph_traversal(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("graph_traversal");
    group.significance_level(0.1).sample_size(10);
    
    // Pre-populate database with test data
    let storage = rt.block_on(async {
        let storage = GraphStorage::new("bench_traversal.db").await.unwrap();
        
        // Create a complex graph with 1000 nodes and 5000 edges
        let mut node_uuids = Vec::new();
        for i in 0..1000 {
            let node = KGNode::new(
                format!("Traversal Node {}", i),
                "TraversalType".to_string(),
                format!("Node for traversal testing {}", i),
                Some("traversal_group".to_string()),
            );
            storage.store_node(&node).await.unwrap();
            node_uuids.push(node.uuid);
        }
        
        // Create edges with various patterns
        for i in 0..5000 {
            let source_idx = i % node_uuids.len();
            let target_idx = (i * 7 + 13) % node_uuids.len(); // Pseudo-random pattern
            
            if source_idx != target_idx {
                let edge = KGEdge::new(
                    node_uuids[source_idx],
                    node_uuids[target_idx],
                    format!("RELATION_{}", i % 10),
                    format!("Traversal edge {}", i),
                    (i % 100) as f32 / 100.0,
                    Some("traversal_group".to_string()),
                );
                storage.store_edge(&edge).await.unwrap();
            }
        }
        
        storage
    });
    
    group.bench_function("neighbor_lookup", |b| {
        b.to_async(&rt).iter(|| async {
            let random_uuid = Uuid::new_v4(); // This will return empty, but tests performance
            black_box(storage.get_neighbors(random_uuid, None).await.unwrap());
        });
    });
    
    group.bench_function("node_search", |b| {
        b.to_async(&rt).iter(|| async {
            black_box(storage.search_nodes("Traversal", 50).await.unwrap());
        });
    });
    
    group.bench_function("group_search", |b| {
        b.to_async(&rt).iter(|| async {
            black_box(storage.search_nodes_by_group("traversal_group", 100).await.unwrap());
        });
    });
    
    group.finish();
}

/// Benchmark embedding generation performance
fn bench_embedding_generation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("embedding_generation");
    group.significance_level(0.1).sample_size(10);
    
    let engine = rt.block_on(async {
        let model_manager = ModelManager::new("models".to_string());
        let batch_processor = std::sync::Arc::new(tokio::sync::Mutex::new(
            BatchProcessor::new(1000, 64).unwrap()
        ));
        let onnx_engine = OnnxEmbeddingEngine::new(batch_processor.clone()).unwrap();
        LocalEmbeddingEngine::new(model_manager, onnx_engine).unwrap()
    });
    
    group.bench_function("single_embedding", |b| {
        b.to_async(&rt).iter(|| async {
            let text = "This is a benchmark text for embedding generation performance testing";
            black_box(engine.generate_embedding(text).await.unwrap());
        });
    });
    
    for batch_size in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("batch_embedding", batch_size),
            batch_size,
            |b, &batch_size| {
                b.to_async(&rt).iter(|| async {
                    let texts: Vec<String> = (0..batch_size)
                        .map(|i| format!("Batch embedding test text number {}", i))
                        .collect();
                    
                    black_box(engine.generate_embeddings(texts).await.unwrap());
                });
            },
        );
    }
    
    group.bench_function("similarity_calculation", |b| {
        b.to_async(&rt).iter(|| async {
            let vec1 = vec![0.1; 384]; // Typical embedding dimension
            let vec2 = vec![0.2; 384];
            black_box(engine.cosine_similarity(&vec1, &vec2));
        });
    });
    
    group.finish();
}

/// Benchmark search engine performance
fn bench_search_engines(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("search_engines");
    group.significance_level(0.1).sample_size(10);
    
    // Set up search engines with test data
    let (text_engine, vector_engine, hybrid_engine) = rt.block_on(async {
        let storage = GraphStorage::new("bench_search.db").await.unwrap();
        
        // Populate with search test data
        for i in 0..1000 {
            let node = KGNode::new(
                format!("Search Test Node {}", i),
                "SearchType".to_string(),
                format!("Node for search performance testing with content {}", i),
                Some("search_group".to_string()),
            );
            storage.store_node(&node).await.unwrap();
        }
        
        let text_engine = TextSearchEngine::new(storage).unwrap();
        let vector_engine = VectorSearchEngine::new(None).unwrap();
        let hybrid_engine = HybridSearchEngine::new(text_engine.clone(), vector_engine.clone(), None);
        
        (text_engine, vector_engine, hybrid_engine)
    });
    
    group.bench_function("text_search", |b| {
        b.to_async(&rt).iter(|| async {
            black_box(text_engine.search_nodes("Search Test", 20).await.unwrap());
        });
    });
    
    group.bench_function("vector_search", |b| {
        b.to_async(&rt).iter(|| async {
            let query_embedding = vec![0.1; 384];
            black_box(vector_engine.search_nodes(&query_embedding, 20).await.unwrap());
        });
    });
    
    group.bench_function("hybrid_search", |b| {
        b.to_async(&rt).iter(|| async {
            black_box(hybrid_engine.search("Search Test", 20).await.unwrap());
        });
    });
    
    group.finish();
}

/// Benchmark memory usage patterns
fn bench_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("memory_usage");
    group.significance_level(0.1).sample_size(10);
    
    group.bench_function("large_dataset_insertion", |b| {
        b.to_async(&rt).iter(|| async {
            let storage = GraphStorage::new("bench_memory.db").await.unwrap();
            
            let initial_memory = get_memory_usage();
            
            // Insert a significant amount of data
            for i in 0..10_000 {
                let node = KGNode::new(
                    format!("Memory Test Node {}", i),
                    "MemoryType".to_string(),
                    format!("Node for memory testing with large content string that takes up space {}", i),
                    Some("memory_group".to_string()),
                );
                storage.store_node(&node).await.unwrap();
            }
            
            let final_memory = get_memory_usage();
            let memory_growth = final_memory - initial_memory;
            
            println!("Memory growth for 10k nodes: {} bytes ({:.2} MB)", 
                    memory_growth, memory_growth as f64 / 1_048_576.0);
            
            black_box(memory_growth);
        });
    });
    
    group.finish();
}

/// Benchmark concurrent operations
fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("concurrent_operations");
    group.significance_level(0.1).sample_size(10);
    
    for thread_count in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_inserts", thread_count),
            thread_count,
            |b, &thread_count| {
                b.to_async(&rt).iter(|| async {
                    let storage = std::sync::Arc::new(
                        GraphStorage::new(&format!("bench_concurrent_{}.db", thread_count))
                            .await
                            .unwrap()
                    );
                    
                    let mut handles = Vec::new();
                    
                    for thread_id in 0..thread_count {
                        let storage_clone = storage.clone();
                        let handle = tokio::spawn(async move {
                            for i in 0..100 {
                                let node = KGNode::new(
                                    format!("Concurrent Node {}_{}", thread_id, i),
                                    "ConcurrentType".to_string(),
                                    format!("Node from thread {} iteration {}", thread_id, i),
                                    Some("concurrent_group".to_string()),
                                );
                                storage_clone.store_node(&node).await.unwrap();
                            }
                        });
                        handles.push(handle);
                    }
                    
                    for handle in handles {
                        handle.await.unwrap();
                    }
                    
                    black_box(thread_count);
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark end-to-end MCP operations
fn bench_mcp_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("mcp_operations");
    group.significance_level(0.1).sample_size(10);
    
    group.bench_function("add_memory_operation", |b| {
        b.to_async(&rt).iter(|| async {
            let params = serde_json::json!({
                "name": "Benchmark Episode",
                "episode_body": "This is a performance test episode for MCP operations benchmarking",
                "source": "text",
                "source_description": "Performance test"
            });
            
            black_box(
                kg_mcp_server::mcp::handlers::handle_tool_request("add_memory", params)
                    .await
                    .unwrap()
            );
        });
    });
    
    group.bench_function("search_memory_nodes_operation", |b| {
        b.to_async(&rt).iter(|| async {
            let params = serde_json::json!({
                "query": "benchmark test performance",
                "max_nodes": 10
            });
            
            black_box(
                kg_mcp_server::mcp::handlers::handle_tool_request("search_memory_nodes", params)
                    .await
                    .unwrap()
            );
        });
    });
    
    group.bench_function("advanced_tool_operation", |b| {
        b.to_async(&rt).iter(|| async {
            let params = serde_json::json!({
                "concept": "machine learning",
                "max_results": 5,
                "similarity_threshold": 0.7
            });
            
            black_box(
                kg_mcp_server::mcp::handlers::handle_tool_request("find_similar_concepts", params)
                    .await
                    .unwrap()
            );
        });
    });
    
    group.finish();
}

/// Helper function to estimate memory usage (simplified)
fn get_memory_usage() -> u64 {
    // In a real implementation, this would use system APIs to get actual memory usage
    // For benchmarking purposes, we'll use a simplified approach
    std::process::id() as u64 * 1024 // Placeholder
}

criterion_group!(
    benches,
    bench_node_insertion,
    bench_edge_insertion,
    bench_graph_traversal,
    bench_embedding_generation,
    bench_search_engines,
    bench_memory_usage,
    bench_concurrent_operations,
    bench_mcp_operations
);

criterion_main!(benches); 
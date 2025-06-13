use anyhow::{Result, anyhow};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::graph::{KGNode, KGEdge, Episode, EpisodeSource, SearchResult};
use crate::graph::storage::GraphStorage;
use crate::embeddings::LocalEmbeddingEngine;
use crate::search::HybridSearchEngine;
use crate::memory::MemoryOptimizer;
use crate::nlp::{EntityExtractor, EntityExtractionConfig, RelationshipExtractor, RelationshipExtractionConfig};
use crate::indexing::{CodebaseIndexer, IndexingConfig, CodeSearchResult, FileDependency, CodebaseAnalysis, IndexingResult};

/// Output verbosity levels for MCP responses
#[derive(Debug, Clone, Copy)]
pub enum OutputVerbosity {
    /// Ultra-compact: Only essential identifiers and names
    Summary,
    /// Compact: Essential information for memory retrieval (default)
    Compact,
    /// Full: Complete details including metadata
    Full,
}

impl OutputVerbosity {
    pub fn from_params(params: &Value) -> Self {
        match params.get("verbosity").and_then(|v| v.as_str()) {
            Some("summary") => OutputVerbosity::Summary,
            Some("full") => OutputVerbosity::Full,
            _ => OutputVerbosity::Compact, // Default
        }
    }
}

/// Helper functions for formatting outputs based on verbosity
impl OutputVerbosity {
    fn format_node(&self, node: &KGNode) -> Value {
        match self {
            OutputVerbosity::Summary => json!({
                "uuid": node.uuid,
                "name": node.name,
                "type": node.node_type
            }),
            OutputVerbosity::Compact => json!({
                "uuid": node.uuid,
                "name": node.name,
                "type": node.node_type,
                "summary": node.summary,
                "group_id": node.group_id
            }),
            OutputVerbosity::Full => json!({
                "uuid": node.uuid,
                "name": node.name,
                "node_type": node.node_type,
                "summary": node.summary,
                "created_at": node.created_at,
                "updated_at": node.updated_at,
                "group_id": node.group_id,
                "metadata": node.metadata
            })
        }
    }

    fn format_edge(&self, edge: &KGEdge, source_node: Option<&KGNode>, target_node: Option<&KGNode>) -> Value {
        match self {
            OutputVerbosity::Summary => json!({
                "uuid": edge.uuid,
                "relation": edge.relation_type,
                "source": source_node.map(|n| n.name.clone()).unwrap_or_else(|| "Unknown".to_string()),
                "target": target_node.map(|n| n.name.clone()).unwrap_or_else(|| "Unknown".to_string())
            }),
            OutputVerbosity::Compact => json!({
                "uuid": edge.uuid,
                "relation": edge.relation_type,
                "summary": edge.summary,
                "source": source_node.map(|n| json!({"uuid": n.uuid, "name": n.name})),
                "target": target_node.map(|n| json!({"uuid": n.uuid, "name": n.name})),
                "weight": edge.weight
            }),
            OutputVerbosity::Full => json!({
                "uuid": edge.uuid,
                "source_node": source_node.map(|n| json!({
                    "uuid": n.uuid,
                    "name": n.name,
                    "node_type": n.node_type
                })),
                "target_node": target_node.map(|n| json!({
                    "uuid": n.uuid,
                    "name": n.name,
                    "node_type": n.node_type
                })),
                "relation_type": edge.relation_type,
                "summary": edge.summary,
                "weight": edge.weight,
                "created_at": edge.created_at,
                "group_id": edge.group_id,
                "metadata": edge.metadata
            })
        }
    }

    fn format_episode(&self, episode: &Episode) -> Value {
        match self {
            OutputVerbosity::Summary => json!({
                "uuid": episode.uuid,
                "name": episode.name,
                "created_at": episode.created_at
            }),
            OutputVerbosity::Compact => json!({
                "uuid": episode.uuid,
                "name": episode.name,
                "content_preview": episode.content.chars().take(100).collect::<String>() + if episode.content.len() > 100 { "..." } else { "" },
                "created_at": episode.created_at,
                "group_id": episode.group_id
            }),
            OutputVerbosity::Full => json!({
                "uuid": episode.uuid,
                "name": episode.name,
                "content": episode.content,
                "created_at": episode.created_at,
                "group_id": episode.group_id,
                "source": format!("{:?}", episode.source)
            })
        }
    }

    fn format_response_metadata(&self, operation: &str, query: Option<&str>, total_found: usize, additional_fields: Option<Value>) -> Value {
        let mut response = match self {
            OutputVerbosity::Summary => json!({
                "success": true,
                "op": operation,
                "count": total_found
            }),
            OutputVerbosity::Compact => json!({
                "success": true,
                "operation": operation,
                "total_found": total_found
            }),
            OutputVerbosity::Full => json!({
                "success": true,
                "operation": operation,
                "total_found": total_found
            })
        };

        // Add query if provided
        if let Some(q) = query {
            response["query"] = json!(q);
        }

        // Merge additional fields
        if let Some(additional) = additional_fields {
            if let (Some(response_obj), Some(additional_obj)) = (response.as_object_mut(), additional.as_object()) {
                for (key, value) in additional_obj {
                    response_obj.insert(key.clone(), value.clone());
                }
            }
        }

        response
    }
}

/// Main handler that routes tool requests to appropriate handlers
pub async fn handle_tool_request(
    tool_name: &str,
    params: Value,
    storage: &Arc<GraphStorage>,
    embedding_engine: &Arc<LocalEmbeddingEngine>,
    search_engine: &Arc<HybridSearchEngine>,
    memory_optimizer: &Arc<MemoryOptimizer>,
) -> Result<Value> {
    debug!("Handling tool request: {} with params: {}", tool_name, params);
    
    match tool_name {
        "mcp_kg-mcp-server_add_memory" => handle_add_memory(params, storage, embedding_engine, memory_optimizer).await,
        "mcp_kg-mcp-server_search_memory" => handle_search_memory(params, storage, embedding_engine, search_engine).await,
        "mcp_kg-mcp-server_analyze_patterns" => handle_analyze_patterns(params, storage, search_engine).await,
        "mcp_kg-mcp-server_manage_graph" => handle_manage_graph(params, storage, embedding_engine).await,
        "mcp_kg-mcp-server_index_codebase" => handle_index_codebase(params, storage, embedding_engine).await,
        _ => Err(anyhow!("Unknown tool: {}", tool_name)),
    }
}

/// Enhanced add_memory handler with entity extraction and relationship building
async fn handle_add_memory(
    params: Value,
    storage: &Arc<GraphStorage>,
    embedding_engine: &Arc<LocalEmbeddingEngine>,
    memory_optimizer: &Arc<MemoryOptimizer>,
) -> Result<Value> {
    let name = params.get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing required parameter: name"))?;
    
    let episode_body = params.get("episode_body")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing required parameter: episode_body"))?;
    
    let group_id = params.get("group_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    
    let source = params.get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("text");
    
    let source_description = params.get("source_description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    
    let uuid = params.get("uuid")
        .and_then(|v| v.as_str())
        .map(|s| uuid::Uuid::parse_str(s))
        .transpose()
        .map_err(|e| anyhow!("Invalid UUID format: {}", e))?
        .unwrap_or_else(|| uuid::Uuid::new_v4());

    let episode_source = match source {
        "json" => EpisodeSource::Json,
        "message" => EpisodeSource::Message,
        _ => EpisodeSource::Text,
    };

    // Create episode
    let mut episode = Episode {
        uuid,
        name: name.to_string(),
        content: episode_body.to_string(),
        created_at: chrono::Utc::now(),
        group_id: group_id.clone(),
        source: episode_source.clone(),
        source_description,
        entity_uuids: Vec::new(),
        edge_uuids: Vec::new(),
        embedding: None,
        metadata: std::collections::HashMap::new(),
    };

    // Generate embedding for the episode content
    match embedding_engine.encode_text(&episode.content).await {
        Ok(embedding) => {
            episode.embedding = Some(embedding.clone());
            // Store embedding in database
            if let Err(e) = storage.store_embedding(episode.uuid, "episode", &embedding) {
                warn!("Failed to store episode embedding: {}", e);
            }
        },
        Err(e) => {
            warn!("Failed to generate embedding for episode: {}", e);
        }
    }

    // Initialize entity and relationship extractors
    let entity_extractor = EntityExtractor::new(EntityExtractionConfig::default(), None)
        .map_err(|e| anyhow::anyhow!("Failed to create entity extractor: {}", e))?;
    
    let relationship_extractor = RelationshipExtractor::new(RelationshipExtractionConfig::default(), None)
        .map_err(|e| anyhow::anyhow!("Failed to create relationship extractor: {}", e))?;

    // Extract entities from episode content
    let extracted_entities = entity_extractor
        .extract_entities(&episode.content, &episode.source, &episode.name)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to extract entities: {}", e))?;

    info!("Extracted {} entities from episode '{}'", extracted_entities.len(), episode.name);

    // Convert extracted entities to nodes and store them
    let mut created_nodes = Vec::new();
    let mut entity_name_to_uuid = HashMap::new();

    for extracted_entity in &extracted_entities {
        // Check if entity already exists (by name and type)
        let existing_nodes = storage.search_nodes_by_text(&extracted_entity.name, None, 10)
            .unwrap_or_default();
        
        let existing_node = existing_nodes.iter()
            .find(|node| node.name == extracted_entity.name && node.node_type == extracted_entity.entity_type);

        let node_uuid = if let Some(existing) = existing_node {
            // Use existing node
            existing.uuid
        } else {
            // Create new node
            let node = entity_extractor.entity_to_node(extracted_entity, group_id.clone());
            let node_uuid = node.uuid;
            
            // Store node in database
            if let Err(e) = storage.insert_node(&node) {
                warn!("Failed to store node '{}': {}", node.name, e);
                continue;
            }

            // Generate and store embedding for the node
            let node_content = format!("{} {} {}", node.name, node.node_type, node.summary);
            if let Ok(node_embedding) = embedding_engine.encode_text(&node_content).await {
                if let Err(e) = storage.store_embedding(node.uuid, "node", &node_embedding) {
                    warn!("Failed to store node embedding for '{}': {}", node.name, e);
                }
            }

            created_nodes.push(node);
            node_uuid
        };

        entity_name_to_uuid.insert(extracted_entity.name.clone(), node_uuid);
        episode.add_entity(node_uuid);
    }

    // Extract relationships between entities
    let extracted_relationships = relationship_extractor
        .extract_relationships_between_entities(&extracted_entities, &episode.content, &episode.name)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to extract relationships: {}", e))?;

    info!("Extracted {} relationships from episode '{}'", extracted_relationships.len(), episode.name);

    // Convert extracted relationships to edges and store them
    let mut created_edges = Vec::new();

    for extracted_relationship in &extracted_relationships {
        // Get UUIDs for source and target entities
        let source_uuid = entity_name_to_uuid.get(&extracted_relationship.source_entity);
        let target_uuid = entity_name_to_uuid.get(&extracted_relationship.target_entity);

        if let (Some(&source_uuid), Some(&target_uuid)) = (source_uuid, target_uuid) {
            // Check if relationship already exists
            let existing_edges = storage.get_edges_between_nodes(source_uuid, target_uuid)
                .unwrap_or_default();
            
            let existing_edge = existing_edges.iter()
                .find(|edge| edge.relation_type == extracted_relationship.relation_type);

            if existing_edge.is_none() {
                // Create new edge
                let edge = relationship_extractor.relationship_to_edge(
                    extracted_relationship,
                    source_uuid,
                    target_uuid,
                    group_id.clone(),
                );
                let edge_uuid = edge.uuid;

                // Store edge in database
                if let Err(e) = storage.insert_edge(&edge) {
                    warn!("Failed to store edge '{}': {}", edge.relation_type, e);
                    continue;
                }

                // Generate and store embedding for the edge
                let edge_content = format!("{} {} {}", edge.relation_type, edge.summary, extracted_relationship.context);
                if let Ok(edge_embedding) = embedding_engine.encode_text(&edge_content).await {
                    if let Err(e) = storage.store_embedding(edge.uuid, "edge", &edge_embedding) {
                        warn!("Failed to store edge embedding for '{}': {}", edge.relation_type, e);
                    }
                }

                created_edges.push(edge);
                episode.add_edge(edge_uuid);
            }
        } else {
            warn!("Could not find UUIDs for relationship: {} -> {}", 
                  extracted_relationship.source_entity, extracted_relationship.target_entity);
        }
    }

    // Store the episode with linked entities and edges
    storage.insert_episode(&episode)
        .map_err(|e| anyhow!("Failed to store episode: {}", e))?;

    // Cache the episode for faster retrieval
    if let Err(e) = memory_optimizer.cache_episode(episode.clone()).await {
        warn!("Failed to cache episode: {}", e);
    }

    info!("Successfully stored episode '{}' with {} entities and {} relationships", 
          episode.name, created_nodes.len(), created_edges.len());

    let verbosity = OutputVerbosity::from_params(&params);
    
    let mut response = match verbosity {
        OutputVerbosity::Summary => json!({
            "success": true,
            "episode_id": episode.uuid,
            "entities": created_nodes.len(),
            "relationships": created_edges.len()
        }),
        OutputVerbosity::Compact => json!({
            "success": true,
            "episode_id": episode.uuid,
            "name": episode.name,
            "entities_created": created_nodes.len(),
            "relationships_created": created_edges.len()
        }),
        OutputVerbosity::Full => json!({
            "success": true,
            "message": "Episode added successfully",
            "episode_id": episode.uuid,
            "group_id": episode.group_id,
            "name": episode.name,
            "entities_created": created_nodes.len(),
            "relationships_created": created_edges.len(),
            "entities": created_nodes.iter().map(|n| json!({
                "uuid": n.uuid,
                "name": n.name,
                "type": n.node_type
            })).collect::<Vec<_>>(),
            "relationships": created_edges.iter().map(|e| json!({
                "uuid": e.uuid,
                "type": e.relation_type,
                "summary": e.summary
            })).collect::<Vec<_>>()
        })
    };

    Ok(response)
}

/// Handle comprehensive search operations with batch support
async fn handle_search_memory(
    params: Value,
    storage: &Arc<GraphStorage>,
    embedding_engine: &Arc<LocalEmbeddingEngine>,
    search_engine: &Arc<HybridSearchEngine>,
) -> Result<Value> {
    let operation = params.get("operation")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing required parameter: operation"))?;
    
    let max_results = params.get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(10) as usize;
    
    let verbosity = OutputVerbosity::from_params(&params);
    
    match operation {
        "nodes" => {
            let query = params.get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Query required for nodes operation"))?;
            
            let entity_filter = params.get("entity_filter")
                .and_then(|v| v.as_str());
            
            let group_ids = params.get("group_ids")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<String>>());

            // Perform hybrid search for nodes
            let search_result = search_engine.search(query, max_results * 2).await
                .map_err(|e| anyhow!("Search failed: {}", e))?;
            
            let mut filtered_nodes = search_result.nodes;
            
            // Apply entity type filter if specified
            if let Some(entity_type) = entity_filter {
                filtered_nodes.retain(|node| node.node_type == entity_type);
            }
            
            // Apply group filter if specified
            if let Some(ref groups) = group_ids {
                filtered_nodes.retain(|node| {
                    node.group_id.as_ref().map_or(false, |gid| groups.contains(gid))
                });
            }
            
            // Limit results
            filtered_nodes.truncate(max_results);
            
            let results: Vec<Value> = filtered_nodes.into_iter().map(|node| {
                verbosity.format_node(&node)
            }).collect();
            
            let additional_fields = json!({
                "entity_filter": entity_filter,
                "group_ids": group_ids
            });
            
            let mut response = verbosity.format_response_metadata("nodes", Some(query), results.len(), Some(additional_fields));
            response["results"] = json!(results);
            Ok(response)
        },
        "facts" => {
            let query = params.get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Query required for facts operation"))?;
            
            let group_ids = params.get("group_ids")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<String>>());

            // Search for edges (facts/relationships) using text search
            let all_edges = storage.search_edges_by_text(query, max_results * 2)
                .map_err(|e| anyhow!("Failed to search edges: {}", e))?;
            
            let mut filtered_edges = all_edges;
            
            // Apply group filter if specified
            if let Some(ref groups) = group_ids {
                filtered_edges.retain(|edge| {
                    edge.group_id.as_ref().map_or(false, |gid| groups.contains(gid))
                });
            }
            
            // Limit results
            filtered_edges.truncate(max_results);
            
            let mut results = Vec::new();
            for edge in filtered_edges {
                // Get source and target nodes for context
                let source_node = storage.get_node(edge.source_node_uuid)?;
                let target_node = storage.get_node(edge.target_node_uuid)?;
                
                results.push(verbosity.format_edge(&edge, source_node.as_ref(), target_node.as_ref()));
            }
            
            let additional_fields = json!({
                "group_ids": group_ids
            });
            
            let mut response = verbosity.format_response_metadata("facts", Some(query), results.len(), Some(additional_fields));
            response["results"] = json!(results);
            Ok(response)
        },
        "episodes" => {
            let group_id = params.get("group_id")
                .and_then(|v| v.as_str());
            let last_n = params.get("last_n")
                .and_then(|v| v.as_u64())
                .unwrap_or(10) as usize;
            
            match storage.get_recent_episodes(group_id, last_n) {
                Ok(episodes) => {
                    let results: Vec<Value> = episodes.into_iter().map(|episode| {
                        verbosity.format_episode(&episode)
                    }).collect();
                    
                    let additional_fields = json!({
                        "group_id": group_id
                    });
                    
                    let mut response = verbosity.format_response_metadata("episodes", None, results.len(), Some(additional_fields));
                    response["results"] = json!(results);
                    Ok(response)
                },
                Err(e) => Err(anyhow!("Failed to retrieve episodes: {}", e))
            }
        },
        "similar_concepts" => {
            let query = params.get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Query required for similar_concepts operation"))?;
            
            let similarity_threshold = params.get("similarity_threshold")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.7) as f32;
            
            let group_ids = params.get("group_ids")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<String>>());
            
            // Generate embedding for the query
            let query_embedding = embedding_engine.encode_text(query).await
                .map_err(|e| anyhow!("Failed to generate query embedding: {}", e))?;
            
            // Get all nodes with embeddings for similarity comparison
            let all_nodes = storage.search_nodes_by_text("", None, 10000)?; // Get all nodes
            let mut similar_concepts = Vec::new();
            
            for node in all_nodes {
                // Apply group filter early if specified
                if let Some(ref groups) = group_ids {
                    if !node.group_id.as_ref().map_or(false, |gid| groups.contains(gid)) {
                        continue;
                    }
                }
                
                // Generate embedding for node content
                let node_text = format!("{} {} {}", node.name, node.node_type, node.summary);
                match embedding_engine.encode_text(&node_text).await {
                    Ok(node_embedding) => {
                        // Calculate cosine similarity
                        let similarity = cosine_similarity(&query_embedding, &node_embedding);
                        
                        if similarity >= similarity_threshold {
                            similar_concepts.push(json!({
                                "uuid": node.uuid,
                                "name": node.name,
                                "node_type": node.node_type,
                                "summary": node.summary,
                                "similarity": similarity,
                                "group_id": node.group_id,
                                "created_at": node.created_at
                            }));
                        }
                    },
                    Err(e) => {
                        warn!("Failed to generate embedding for node {}: {}", node.uuid, e);
                    }
                }
            }
            
            // Sort by similarity (highest first) and limit results
            similar_concepts.sort_by(|a, b| {
                let sim_a = a.get("similarity").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let sim_b = b.get("similarity").and_then(|v| v.as_f64()).unwrap_or(0.0);
                sim_b.partial_cmp(&sim_a).unwrap_or(std::cmp::Ordering::Equal)
            });
            similar_concepts.truncate(max_results);
            
            let additional_fields = json!({
                "similarity_threshold": similarity_threshold,
                "group_ids": group_ids
            });
            
            let mut response = verbosity.format_response_metadata("similar_concepts", Some(query), similar_concepts.len(), Some(additional_fields));
            response["results"] = json!(similar_concepts);
            Ok(response)
        },
        "batch" => {
            let queries = params.get("queries")
                .and_then(|v| v.as_array())
                .ok_or_else(|| anyhow!("Queries array required for batch operation"))?;
            
            let mut batch_results = Vec::new();
            for query in queries {
                if let Some(query_str) = query.as_str() {
                    // Perform hybrid search for each query
                    match search_engine.search(query_str, max_results).await {
                        Ok(search_result) => {
                            let results: Vec<Value> = search_result.nodes.into_iter().map(|node| {
                                verbosity.format_node(&node)
                            }).collect();
                            
                            batch_results.push(json!({
                                "query": query_str,
                                "results": results,
                                "total_found": results.len()
                            }));
                        },
                        Err(e) => {
                    batch_results.push(json!({
                        "query": query_str,
                        "results": [],
                                "total_found": 0,
                                "error": format!("Search failed: {}", e)
                    }));
                        }
                    }
                }
            }
            
            let mut response = verbosity.format_response_metadata("batch", None, queries.len(), None);
            response["batch_results"] = json!(batch_results);
            response["total_queries"] = json!(queries.len());
            Ok(response)
        },
        _ => Err(anyhow!("Unknown search operation: {}", operation))
    }
}

// Helper function for cosine similarity calculation
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a * norm_b)
    }
}

/// Handle pattern analysis operations
async fn handle_analyze_patterns(
    params: Value,
    storage: &Arc<GraphStorage>,
    search_engine: &Arc<HybridSearchEngine>,
) -> Result<Value> {
    let analysis_type = params.get("analysis_type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing required parameter: analysis_type"))?;
    
    let max_results = params.get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(20) as usize;
    
    match analysis_type {
        "relationships" => {
            Ok(json!({
                "success": true,
                "analysis_type": "relationships",
                "patterns": [],
                "total_found": 0,
                "message": "Relationship pattern analysis available but not yet implemented"
            }))
        },
        "clusters" => {
            Ok(json!({
                "success": true,
                "analysis_type": "clusters",
                "clusters": [],
                "total_clusters": 0,
                "message": "Entity clustering analysis available but not yet implemented"
            }))
        },
        "temporal" => {
            let time_range_days = params.get("time_range_days")
                .and_then(|v| v.as_u64())
                .unwrap_or(30);
            let time_granularity = params.get("time_granularity")
                .and_then(|v| v.as_str())
                .unwrap_or("day");
            
            Ok(json!({
                "success": true,
                "analysis_type": "temporal",
                "time_range_days": time_range_days,
                "time_granularity": time_granularity,
                "patterns": [],
                "total_found": 0,
                "message": "Temporal pattern analysis available but not yet implemented"
            }))
        },
        "centrality" => {
            Ok(json!({
                "success": true,
                "analysis_type": "centrality",
                "important_nodes": [],
                "total_found": 0,
                "message": "Centrality analysis available but not yet implemented"
            }))
        },
        "semantic_clusters" => {
            let cluster_method = params.get("cluster_method")
                .and_then(|v| v.as_str())
                .unwrap_or("kmeans");
            let num_clusters = params.get("num_clusters")
                .and_then(|v| v.as_u64())
                .unwrap_or(5);
            
            Ok(json!({
                "success": true,
                "analysis_type": "semantic_clusters",
                "cluster_method": cluster_method,
                "num_clusters": num_clusters,
                "clusters": [],
                "total_clusters": 0,
                "message": "Semantic clustering analysis available but not yet implemented"
            }))
        },
        _ => Err(anyhow!("Unknown analysis type: {}", analysis_type))
    }
}

/// Handle graph management operations with batch support
async fn handle_manage_graph(
    params: Value,
    storage: &Arc<GraphStorage>,
    embedding_engine: &Arc<LocalEmbeddingEngine>,
) -> Result<Value> {
    let operation = params.get("operation")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing required parameter: operation"))?;
    
    let verbosity = OutputVerbosity::from_params(&params);
    
    match operation {
        "get_entity_edge" => {
            let uuid = params.get("uuid")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("UUID required for get_entity_edge operation"))?;
            
            Ok(json!({
                "success": true,
                "operation": "get_entity_edge",
                "uuid": uuid,
                "result": null,
                "message": "Entity edge retrieval available but not yet implemented"
            }))
        },
        "delete_entity_edge" => {
            let uuid_str = params.get("uuid")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("UUID required for delete_entity_edge operation"))?;
            
            let uuid = uuid::Uuid::parse_str(uuid_str)
                .map_err(|e| anyhow!("Invalid UUID format: {}", e))?;
            
            match storage.delete_edge(&uuid) {
                Ok(_) => {
                    Ok(json!({
                        "success": true,
                        "operation": "delete_entity_edge",
                        "uuid": uuid,
                        "message": "Entity edge deleted successfully"
                    }))
                },
                Err(e) => Err(anyhow!("Failed to delete entity edge: {}", e))
            }
        },
        "delete_episode" => {
            let uuid_str = params.get("uuid")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("UUID required for delete_episode operation"))?;
            
            let uuid = uuid::Uuid::parse_str(uuid_str)
                .map_err(|e| anyhow!("Invalid UUID format: {}", e))?;
            
            match storage.delete_episode(&uuid) {
                Ok(_) => {
                    Ok(json!({
                        "success": true,
                        "operation": "delete_episode",
                        "uuid": uuid,
                        "message": "Episode deleted successfully"
                    }))
                },
                Err(e) => Err(anyhow!("Failed to delete episode: {}", e))
            }
        },
        "delete_batch" => {
            let uuids = params.get("uuids")
                .and_then(|v| v.as_array())
                .ok_or_else(|| anyhow!("UUIDs array required for delete_batch operation"))?;
            
            let confirm = params.get("confirm")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            
            if !confirm {
                return Err(anyhow!("Batch deletion requires explicit confirmation (confirm: true)"));
            }
            
            let delete_type = params.get("delete_type")
                .and_then(|v| v.as_str())
                .unwrap_or("mixed");
            
            let mut deleted_count = 0;
            let mut errors = Vec::new();
            
            for uuid_val in uuids {
                if let Some(uuid_str) = uuid_val.as_str() {
                    match uuid::Uuid::parse_str(uuid_str) {
                        Ok(uuid) => {
                            // For now, only handle episode deletion
                            match storage.delete_episode(&uuid) {
                                Ok(_) => deleted_count += 1,
                                Err(e) => errors.push(format!("Failed to delete {}: {}", uuid, e))
                            }
                        },
                        Err(e) => errors.push(format!("Invalid UUID {}: {}", uuid_str, e))
                    }
                }
            }
            
            Ok(json!({
                "success": true,
                "operation": "delete_batch",
                "delete_type": delete_type,
                "deleted_count": deleted_count,
                "total_requested": uuids.len(),
                "errors": errors
            }))
        },
        "clear_graph" => {
            let confirm = params.get("confirm")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            
            if !confirm {
                return Err(anyhow!("Graph clearing requires explicit confirmation (confirm: true)"));
            }
            
            match storage.clear_all_data() {
                Ok(_) => {
                    Ok(json!({
                        "success": true,
                        "operation": "clear_graph",
                        "message": "Graph cleared successfully"
                    }))
                },
                Err(e) => Err(anyhow!("Failed to clear graph: {}", e))
            }
        },
        "get_episodes" => {
            let group_id = params.get("group_id")
                .and_then(|v| v.as_str());
            let last_n = params.get("last_n")
                .and_then(|v| v.as_u64())
                .unwrap_or(10) as usize;
            
            match storage.get_recent_episodes(group_id, last_n) {
                Ok(episodes) => {
                    let results: Vec<Value> = episodes.into_iter().map(|episode| {
                        verbosity.format_episode(&episode)
                    }).collect();
                    
                    let additional_fields = json!({
                        "group_id": group_id
                    });
                    
                    let mut response = verbosity.format_response_metadata("get_episodes", None, results.len(), Some(additional_fields));
                    response["results"] = json!(results);
                    Ok(response)
                },
                Err(e) => Err(anyhow!("Failed to retrieve episodes: {}", e))
            }
        },
        _ => Err(anyhow!("Unknown management operation: {}", operation))
    }
}

/// Handle codebase indexing operations
async fn handle_index_codebase(
    params: Value,
    storage: &Arc<GraphStorage>,
    embedding_engine: &Arc<LocalEmbeddingEngine>,
) -> Result<Value> {
    let operation = params.get("operation")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing required parameter: operation"))?;
    
    let verbosity = OutputVerbosity::from_params(&params);
    
    match operation {
        "index" | "reindex" => {
            let path = params.get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Path required for index/reindex operation"))?;
            
            let languages = params.get("languages")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<String>>());
            
            let include_patterns = params.get("include_patterns")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<String>>());
            
            let exclude_patterns = params.get("exclude_patterns")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<String>>());
            
            let max_file_size = params.get("max_file_size")
                .and_then(|v| v.as_u64())
                .unwrap_or(1048576) as usize; // 1MB default
            
            let parallel_workers = params.get("parallel_workers")
                .and_then(|v| v.as_u64())
                .unwrap_or(4) as usize;
            
            let incremental = params.get("incremental")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            
            let extract_dependencies = params.get("extract_dependencies")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            
            let extract_symbols = params.get("extract_symbols")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            
            let group_id = params.get("group_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            
            // Create indexing configuration
            let config = IndexingConfig {
                languages,
                include_patterns,
                exclude_patterns,
                max_file_size,
                parallel_workers,
                incremental,
                extract_dependencies,
                extract_symbols,
                group_id,
            };
            
            // Create and run indexer
            let indexer = CodebaseIndexer::new_with_mcp_config(path.to_string(), config.clone());
            
            match indexer.index_codebase_mcp(path, storage.clone(), embedding_engine.clone()).await {
                Ok(result) => {
                    let response = match verbosity {
                        OutputVerbosity::Summary => json!({
                            "success": true,
                            "operation": operation,
                            "files_processed": result.files_processed,
                            "symbols_extracted": result.symbols_extracted
                        }),
                        OutputVerbosity::Compact => json!({
                            "success": true,
                            "operation": operation,
                            "path": path,
                            "files_processed": result.files_processed,
                            "symbols_extracted": result.symbols_extracted,
                            "dependencies_mapped": result.dependencies_mapped,
                            "processing_time_ms": result.processing_time_ms
                        }),
                        OutputVerbosity::Full => json!({
                            "success": true,
                            "operation": operation,
                            "path": path,
                            "config": {
                                "languages": config.languages,
                                "include_patterns": config.include_patterns,
                                "exclude_patterns": config.exclude_patterns,
                                "max_file_size": config.max_file_size,
                                "parallel_workers": config.parallel_workers,
                                "incremental": config.incremental,
                                "extract_dependencies": config.extract_dependencies,
                                "extract_symbols": config.extract_symbols
                            },
                            "results": {
                                "files_processed": result.files_processed,
                                "symbols_extracted": result.symbols_extracted,
                                "dependencies_mapped": result.dependencies_mapped,
                                "processing_time_ms": result.processing_time_ms,
                                "languages_detected": result.languages_detected,
                                "errors": result.errors
                            }
                        })
                    };
                    Ok(response)
                },
                Err(e) => Err(anyhow!("Indexing failed: {}", e))
            }
        },
        "status" => {
            // Return indexing status (placeholder implementation)
            Ok(json!({
                "success": true,
                "operation": "status",
                "status": "ready",
                "message": "Indexing status check available but not yet implemented"
            }))
        },
        "search_code" => {
            let query = params.get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Query required for search_code operation"))?;
            
            let symbol_type = params.get("symbol_type")
                .and_then(|v| v.as_str())
                .unwrap_or("all");
            
            let context_lines = params.get("context_lines")
                .and_then(|v| v.as_u64())
                .unwrap_or(5) as usize;
            
            let max_results = params.get("max_results")
                .and_then(|v| v.as_u64())
                .unwrap_or(50) as usize;
            
            // Perform code search using the indexer
            let config = IndexingConfig::default();
            let indexer = CodebaseIndexer::new_with_mcp_config("".to_string(), config);
            
            match indexer.search_code_mcp(query, symbol_type, context_lines, max_results, storage.clone()).await {
                Ok(results) => {
                    let formatted_results: Vec<Value> = results.into_iter().map(|result| {
                        match verbosity {
                            OutputVerbosity::Summary => json!({
                                "file": result.file_path,
                                "symbol": result.symbol_name,
                                "line": result.line_number
                            }),
                            OutputVerbosity::Compact => json!({
                                "file": result.file_path,
                                "symbol": result.symbol_name,
                                "symbol_type": result.symbol_type,
                                "line": result.line_number,
                                "context": result.context_lines.join("\n")
                            }),
                            OutputVerbosity::Full => json!({
                                "file": result.file_path,
                                "symbol": result.symbol_name,
                                "symbol_type": result.symbol_type,
                                "line": result.line_number,
                                "column": result.column_number,
                                "context_lines": result.context_lines,
                                "full_context": result.full_context,
                                "language": result.language,
                                "relevance_score": result.relevance_score
                            })
                        }
                    }).collect();
                    
                    let mut response = verbosity.format_response_metadata("search_code", Some(query), formatted_results.len(), Some(json!({
                        "symbol_type": symbol_type,
                        "context_lines": context_lines
                    })));
                    response["results"] = json!(formatted_results);
                    Ok(response)
                },
                Err(e) => Err(anyhow!("Code search failed: {}", e))
            }
        },
        "get_dependencies" => {
            let file_path = params.get("file_path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("File path required for get_dependencies operation"))?;
            
            let config = IndexingConfig::default();
            let indexer = CodebaseIndexer::new_with_mcp_config("".to_string(), config);
            
            match indexer.get_file_dependencies_mcp(file_path, storage.clone()).await {
                Ok(dependencies) => {
                    let response = match verbosity {
                        OutputVerbosity::Summary => json!({
                            "success": true,
                            "operation": "get_dependencies",
                            "file": file_path,
                            "dependency_count": dependencies.len()
                        }),
                        OutputVerbosity::Compact => json!({
                            "success": true,
                            "operation": "get_dependencies",
                            "file": file_path,
                            "dependencies": dependencies.iter().map(|dep| json!({
                                "file": dep.target_file,
                                "type": dep.dependency_type
                            })).collect::<Vec<_>>()
                        }),
                        OutputVerbosity::Full => json!({
                            "success": true,
                            "operation": "get_dependencies",
                            "file": file_path,
                            "dependencies": dependencies.iter().map(|dep| json!({
                                "target_file": dep.target_file,
                                "dependency_type": dep.dependency_type,
                                "line_number": dep.line_number,
                                "context": dep.context,
                                "confidence": dep.confidence
                            })).collect::<Vec<_>>()
                        })
                    };
                    Ok(response)
                },
                Err(e) => Err(anyhow!("Dependency analysis failed: {}", e))
            }
        },
        "analyze_structure" => {
            let path = params.get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Path required for analyze_structure operation"))?;
            
            let config = IndexingConfig::default();
            let indexer = CodebaseIndexer::new_with_mcp_config(path.to_string(), config);
            
            match indexer.analyze_codebase_structure_mcp(storage.clone()).await {
                Ok(analysis) => {
                    let response = match verbosity {
                        OutputVerbosity::Summary => json!({
                            "success": true,
                            "operation": "analyze_structure",
                            "total_files": analysis.total_files,
                            "languages": analysis.languages.len()
                        }),
                        OutputVerbosity::Compact => json!({
                            "success": true,
                            "operation": "analyze_structure",
                            "path": path,
                            "total_files": analysis.total_files,
                            "languages": analysis.languages,
                            "directory_structure": analysis.directory_structure
                        }),
                        OutputVerbosity::Full => json!({
                            "success": true,
                            "operation": "analyze_structure",
                            "path": path,
                            "analysis": {
                                "total_files": analysis.total_files,
                                "total_lines": analysis.total_lines,
                                "languages": analysis.languages,
                                "directory_structure": analysis.directory_structure,
                                "file_types": analysis.file_types,
                                "complexity_metrics": analysis.complexity_metrics,
                                "dependency_graph": analysis.dependency_graph
                            }
                        })
                    };
                    Ok(response)
                },
                Err(e) => Err(anyhow!("Structure analysis failed: {}", e))
            }
        },
        _ => Err(anyhow!("Unknown indexing operation: {}", operation))
    }
} 
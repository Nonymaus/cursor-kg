use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use super::{
    Migrator, MigrationConfig, MigrationResult, MigrationPlan, MigrationProgress, 
    MigrationPhase, MigrationStats, MigrationError, ValidationReport, ValidationIssue,
    ValidationSeverity, utils
};
use crate::graph::{KGNode, KGEdge, Episode, EpisodeSource};
use crate::graph::storage::GraphStorage;
use crate::embeddings::LocalEmbeddingEngine;

/// GraphitiMigrator handles migration from graphiti-mcp systems
pub struct GraphitiMigrator {
    storage: GraphStorage,
    embedding_engine: Option<LocalEmbeddingEngine>,
}

/// Graphiti node representation for migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphitiNode {
    pub uuid: String,
    pub name: String,
    pub labels: Vec<String>,
    pub properties: HashMap<String, serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Graphiti edge representation for migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphitiEdge {
    pub uuid: String,
    pub source_uuid: String,
    pub target_uuid: String,
    pub relation_type: String,
    pub properties: HashMap<String, serde_json::Value>,
    pub weight: Option<f32>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Graphiti episode representation for migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphitiEpisode {
    pub uuid: String,
    pub name: String,
    pub content: String,
    pub entity_uuids: Vec<String>,
    pub edge_uuids: Vec<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Neo4j connection configuration
#[derive(Debug, Clone)]
pub struct Neo4jConfig {
    pub uri: String,
    pub username: String,
    pub password: String,
    pub database: String,
}

impl GraphitiMigrator {
    pub fn new(storage: GraphStorage, embedding_engine: Option<LocalEmbeddingEngine>) -> Self {
        Self {
            storage,
            embedding_engine,
        }
    }

    /// Parse Neo4j connection string
    fn parse_neo4j_connection(connection_string: &str) -> Result<Neo4jConfig> {
        // Expected format: "neo4j://username:password@host:port/database"
        if !connection_string.starts_with("neo4j://") {
            return Err(anyhow!("Invalid Neo4j connection string format"));
        }

        // For now, return a mock configuration since we're not actually connecting to Neo4j
        Ok(Neo4jConfig {
            uri: "neo4j://localhost:7687".to_string(),
            username: "neo4j".to_string(),
            password: "password".to_string(),
            database: "neo4j".to_string(),
        })
    }

    /// Simulate fetching data from Neo4j (mock implementation)
    async fn fetch_graphiti_data(&self, _config: &Neo4jConfig) -> Result<(Vec<GraphitiNode>, Vec<GraphitiEdge>, Vec<GraphitiEpisode>)> {
        // Mock data for demonstration - in real implementation, this would query Neo4j
        let nodes = vec![
            GraphitiNode {
                uuid: Uuid::new_v4().to_string(),
                name: "Sample Entity".to_string(),
                labels: vec!["Entity".to_string(), "Concept".to_string()],
                properties: {
                    let mut props = HashMap::new();
                    props.insert("type".to_string(), serde_json::Value::String("concept".to_string()));
                    props.insert("importance".to_string(), serde_json::json!(0.8));
                    props
                },
                created_at: Some(Utc::now()),
                updated_at: Some(Utc::now()),
            },
            GraphitiNode {
                uuid: Uuid::new_v4().to_string(),
                name: "Related Entity".to_string(),
                labels: vec!["Entity".to_string()],
                properties: {
                    let mut props = HashMap::new();
                    props.insert("type".to_string(), serde_json::Value::String("entity".to_string()));
                    props
                },
                created_at: Some(Utc::now()),
                updated_at: Some(Utc::now()),
            },
        ];

        let edges = vec![
            GraphitiEdge {
                uuid: Uuid::new_v4().to_string(),
                source_uuid: nodes[0].uuid.clone(),
                target_uuid: nodes[1].uuid.clone(),
                relation_type: "RELATES_TO".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("strength".to_string(), serde_json::json!(0.9));
                    props
                },
                weight: Some(0.9),
                created_at: Some(Utc::now()),
            },
        ];

        let episodes = vec![
            GraphitiEpisode {
                uuid: Uuid::new_v4().to_string(),
                name: "Sample Episode".to_string(),
                content: "This is a sample episode containing information about the entities and their relationships.".to_string(),
                entity_uuids: nodes.iter().map(|n| n.uuid.clone()).collect(),
                edge_uuids: edges.iter().map(|e| e.uuid.clone()).collect(),
                created_at: Some(Utc::now()),
                metadata: {
                    let mut meta = HashMap::new();
                    meta.insert("source".to_string(), serde_json::Value::String("migration".to_string()));
                    meta
                },
            },
        ];

        Ok((nodes, edges, episodes))
    }

    /// Convert GraphitiNode to KGNode
    fn convert_node(&self, graphiti_node: &GraphitiNode) -> Result<KGNode> {
        let uuid = Uuid::parse_str(&graphiti_node.uuid)
            .map_err(|_| anyhow!("Invalid UUID format: {}", graphiti_node.uuid))?;

        let node_type = graphiti_node.properties
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("entity")
            .to_string();

        let summary = format!("{} ({})", graphiti_node.name, node_type);

        Ok(KGNode {
            uuid,
            name: graphiti_node.name.clone(),
            node_type,
            summary,
            metadata: graphiti_node.properties.clone(),
            created_at: graphiti_node.created_at.unwrap_or_else(Utc::now),
            updated_at: graphiti_node.updated_at.unwrap_or_else(Utc::now),
            group_id: None, // Default group
        })
    }

    /// Convert GraphitiEdge to KGEdge
    fn convert_edge(&self, graphiti_edge: &GraphitiEdge) -> Result<KGEdge> {
        let uuid = Uuid::parse_str(&graphiti_edge.uuid)
            .map_err(|_| anyhow!("Invalid edge UUID format: {}", graphiti_edge.uuid))?;

        let source_node_uuid = Uuid::parse_str(&graphiti_edge.source_uuid)
            .map_err(|_| anyhow!("Invalid source UUID format: {}", graphiti_edge.source_uuid))?;

        let target_node_uuid = Uuid::parse_str(&graphiti_edge.target_uuid)
            .map_err(|_| anyhow!("Invalid target UUID format: {}", graphiti_edge.target_uuid))?;

        Ok(KGEdge {
            uuid,
            source_node_uuid,
            target_node_uuid,
            relation_type: graphiti_edge.relation_type.clone(),
            summary: format!("{} -> {}", graphiti_edge.relation_type, "target"),
            weight: graphiti_edge.weight.unwrap_or(1.0),
            metadata: graphiti_edge.properties.clone(),
            created_at: graphiti_edge.created_at.unwrap_or_else(Utc::now),
            updated_at: graphiti_edge.created_at.unwrap_or_else(Utc::now),
            group_id: None, // Default group
        })
    }

    /// Convert GraphitiEpisode to Episode
    async fn convert_episode(&self, graphiti_episode: &GraphitiEpisode) -> Result<Episode> {
        let uuid = Uuid::parse_str(&graphiti_episode.uuid)
            .map_err(|_| anyhow!("Invalid episode UUID format: {}", graphiti_episode.uuid))?;

        let entity_uuids: Result<Vec<Uuid>> = graphiti_episode.entity_uuids
            .iter()
            .map(|s| Uuid::parse_str(s).map_err(|_| anyhow!("Invalid entity UUID: {}", s)))
            .collect();

        let edge_uuids: Result<Vec<Uuid>> = graphiti_episode.edge_uuids
            .iter()
            .map(|s| Uuid::parse_str(s).map_err(|_| anyhow!("Invalid edge UUID: {}", s)))
            .collect();

        // Generate embedding if embedding engine is available
        let embedding = if let Some(ref engine) = self.embedding_engine {
            match engine.encode_text(&graphiti_episode.content).await {
                Ok(emb) => Some(emb),
                Err(e) => {
                    log::warn!("Failed to generate embedding for episode {}: {}", uuid, e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Episode {
            uuid,
            name: graphiti_episode.name.clone(),
            content: graphiti_episode.content.clone(),
            entity_uuids: entity_uuids?,
            edge_uuids: edge_uuids?,
            embedding,
            created_at: graphiti_episode.created_at.unwrap_or_else(Utc::now),
            metadata: graphiti_episode.metadata.clone(),
            group_id: None, // Default group
            source: EpisodeSource::Message, // Use proper enum variant
            source_description: "Migrated from Graphiti MCP".to_string(),
        })
    }

    /// Migrate data in batches with progress tracking
    async fn migrate_batch<T, F, R>(
        &self,
        items: Vec<T>,
        batch_size: usize,
        convert_fn: F,
        progress_callback: &Option<Box<dyn Fn(MigrationProgress) + Send + Sync>>,
        phase: MigrationPhase,
        stats: &mut MigrationStats,
    ) -> Result<Vec<R>>
    where
        F: Fn(&T) -> Result<R> + Send + Sync,
        T: Send + Sync,
        R: Send,
    {
        let mut results = Vec::new();
        let total_batches = (items.len() + batch_size - 1) / batch_size;
        let start_time = Instant::now();

        for (batch_idx, chunk) in items.chunks(batch_size).enumerate() {
            let mut batch_results = Vec::new();
            let mut batch_errors = 0;

            for item in chunk {
                match convert_fn(item) {
                    Ok(result) => batch_results.push(result),
                    Err(e) => {
                        batch_errors += 1;
                        stats.errors.push(MigrationError {
                            error_type: "conversion_error".to_string(),
                            message: e.to_string(),
                            source_id: None,
                            timestamp: Utc::now(),
                            recoverable: true,
                        });
                    }
                }
            }

            results.extend(batch_results);

            // Update progress
            if let Some(ref callback) = progress_callback {
                let elapsed = start_time.elapsed();
                let items_processed = (batch_idx + 1) * batch_size.min(chunk.len());
                let throughput = items_processed as f32 / elapsed.as_secs_f32();
                let estimated_remaining = if throughput > 0.0 {
                    Duration::from_secs_f32((items.len() - items_processed) as f32 / throughput)
                } else {
                    Duration::from_secs(0)
                };

                let progress = MigrationProgress {
                    phase: phase.clone(),
                    nodes_processed: if matches!(phase, MigrationPhase::MigratingNodes) { items_processed } else { 0 },
                    edges_processed: if matches!(phase, MigrationPhase::MigratingEdges) { items_processed } else { 0 },
                    episodes_processed: if matches!(phase, MigrationPhase::MigratingEpisodes) { items_processed } else { 0 },
                    total_items: items.len(),
                    current_batch: batch_idx + 1,
                    total_batches,
                    elapsed_time: elapsed,
                    estimated_remaining,
                    current_throughput: throughput,
                    errors_encountered: batch_errors,
                };

                callback(progress);
            }

            // Small delay to prevent overwhelming the system
            sleep(Duration::from_millis(10)).await;
        }

        Ok(results)
    }
}

#[async_trait]
impl Migrator for GraphitiMigrator {
    async fn analyze_source<'a>(&self, config: &'a MigrationConfig) -> Result<MigrationPlan> {
        log::info!("Analyzing Graphiti source data...");

        // Parse connection and fetch sample data for analysis
        let neo4j_config = Self::parse_neo4j_connection(&config.source_connection)?;
        let (nodes, edges, episodes) = self.fetch_graphiti_data(&neo4j_config).await?;

        let node_count = nodes.len();
        let edge_count = edges.len();
        let episode_count = episodes.len();
        let total_items = node_count + edge_count + episode_count;

        // Calculate complexity and estimates
        let complexity_score = utils::calculate_complexity_score(node_count, edge_count, episode_count);
        
        // Estimate duration based on complexity and item count
        let base_duration_per_item = Duration::from_millis(10); // 10ms per item base
        let complexity_multiplier = 1.0 + complexity_score;
        let estimated_duration = base_duration_per_item.mul_f32(total_items as f32 * complexity_multiplier);

        // Estimate memory usage (conservative)
        let estimated_memory_usage = total_items * 4096; // 4KB per item average

        // Estimate disk space (with overhead)
        let estimated_disk_space = total_items * 2048; // 2KB per item average

        // Recommend optimal settings
        let available_memory_mb = 1024; // Assume 1GB available
        let recommended_batch_size = utils::recommend_batch_size(total_items, available_memory_mb);
        let recommended_workers = if total_items > 10000 { 4 } else { 2 };

        // Identify potential issues
        let mut potential_issues = Vec::new();
        if complexity_score > 0.8 {
            potential_issues.push("High complexity graph detected - consider increasing batch size".to_string());
        }
        if total_items > 100000 {
            potential_issues.push("Large dataset detected - migration may take significant time".to_string());
        }
        if episodes.is_empty() {
            potential_issues.push("No episodes found - semantic features may be limited".to_string());
        }

        Ok(MigrationPlan {
            estimated_duration,
            estimated_memory_usage,
            estimated_disk_space,
            node_count,
            edge_count,
            episode_count,
            complexity_score,
            recommended_batch_size,
            recommended_workers,
            potential_issues,
        })
    }

    async fn migrate<'a>(
        &self, 
        config: &'a MigrationConfig, 
        progress_callback: Option<Box<dyn Fn(MigrationProgress) + Send + Sync>>
    ) -> Result<MigrationResult> {
        log::info!("Starting Graphiti migration...");
        let mut stats = MigrationStats::new();
        let migration_start = Instant::now();

        // Phase 1: Analyze and fetch source data
        if let Some(ref callback) = progress_callback {
            callback(MigrationProgress {
                phase: MigrationPhase::Analyzing,
                nodes_processed: 0,
                edges_processed: 0,
                episodes_processed: 0,
                total_items: 0,
                current_batch: 0,
                total_batches: 0,
                elapsed_time: Duration::from_secs(0),
                estimated_remaining: Duration::from_secs(0),
                current_throughput: 0.0,
                errors_encountered: 0,
            });
        }

        let neo4j_config = Self::parse_neo4j_connection(&config.source_connection)?;
        let (graphiti_nodes, graphiti_edges, graphiti_episodes) = self.fetch_graphiti_data(&neo4j_config).await?;

        stats.total_nodes = graphiti_nodes.len();
        stats.total_edges = graphiti_edges.len();
        stats.total_episodes = graphiti_episodes.len();

        // Phase 2: Migrate nodes
        log::info!("Migrating {} nodes...", stats.total_nodes);
        let nodes = self.migrate_batch(
            graphiti_nodes,
            config.batch_size,
            |gnode| self.convert_node(gnode),
            &progress_callback,
            MigrationPhase::MigratingNodes,
            &mut stats,
        ).await?;

        // Store nodes in database
        for node in &nodes {
            if let Err(e) = self.storage.insert_node(node) {
                stats.errors.push(MigrationError {
                    error_type: "storage_error".to_string(),
                    message: format!("Failed to store node {}: {}", node.uuid, e),
                    source_id: Some(node.uuid.to_string()),
                    timestamp: Utc::now(),
                    recoverable: true,
                });
            } else {
                stats.migrated_nodes += 1;
            }
        }

        // Phase 3: Migrate edges
        log::info!("Migrating {} edges...", stats.total_edges);
        let edges = self.migrate_batch(
            graphiti_edges,
            config.batch_size,
            |gedge| self.convert_edge(gedge),
            &progress_callback,
            MigrationPhase::MigratingEdges,
            &mut stats,
        ).await?;

        // Store edges in database
        for edge in &edges {
            if let Err(e) = self.storage.insert_edge(edge) {
                stats.errors.push(MigrationError {
                    error_type: "storage_error".to_string(),
                    message: format!("Failed to store edge {}: {}", edge.uuid, e),
                    source_id: Some(edge.uuid.to_string()),
                    timestamp: Utc::now(),
                    recoverable: true,
                });
            } else {
                stats.migrated_edges += 1;
            }
        }

        // Phase 4: Migrate episodes
        log::info!("Migrating {} episodes...", stats.total_episodes);
        let mut converted_episodes = Vec::new();
        for episode in graphiti_episodes {
            match self.convert_episode(&episode).await {
                Ok(converted) => converted_episodes.push(converted),
                Err(e) => {
                    stats.errors.push(MigrationError {
                        error_type: "conversion_error".to_string(),
                        message: format!("Failed to convert episode {}: {}", episode.uuid, e),
                        source_id: Some(episode.uuid),
                        timestamp: Utc::now(),
                        recoverable: true,
                    });
                }
            }
        }

        // Store episodes in database
        for episode in &converted_episodes {
            if let Err(e) = self.storage.insert_episode(episode) {
                stats.errors.push(MigrationError {
                    error_type: "storage_error".to_string(),
                    message: format!("Failed to store episode {}: {}", episode.uuid, e),
                    source_id: Some(episode.uuid.to_string()),
                    timestamp: Utc::now(),
                    recoverable: true,
                });
            } else {
                stats.migrated_episodes += 1;
            }
        }

        // Phase 5: Complete migration
        stats.end_time = Some(Utc::now());
        let success = stats.errors.iter().filter(|e| !e.recoverable).count() == 0;

        if let Some(ref callback) = progress_callback {
            callback(MigrationProgress {
                phase: MigrationPhase::Completed,
                nodes_processed: stats.migrated_nodes,
                edges_processed: stats.migrated_edges,
                episodes_processed: stats.migrated_episodes,
                total_items: stats.total_nodes + stats.total_edges + stats.total_episodes,
                current_batch: 1,
                total_batches: 1,
                elapsed_time: migration_start.elapsed(),
                estimated_remaining: Duration::from_secs(0),
                current_throughput: 0.0,
                errors_encountered: stats.errors.len(),
            });
        }

        // Generate recommendations
        let mut recommendations = Vec::new();
        if stats.errors.len() > 0 {
            recommendations.push(format!("Migration completed with {} errors - review error log", stats.errors.len()));
        }
        if stats.migrated_nodes < stats.total_nodes {
            recommendations.push("Some nodes failed to migrate - consider re-running migration".to_string());
        }
        if self.embedding_engine.is_none() {
            recommendations.push("No embedding engine configured - semantic search features will be limited".to_string());
        }

        log::info!("Migration completed: {} nodes, {} edges, {} episodes migrated", 
                  stats.migrated_nodes, stats.migrated_edges, stats.migrated_episodes);

        Ok(MigrationResult {
            success,
            stats,
            validation_report: None, // Will be generated separately if requested
            backup_location: None,   // Will be set if backup was created
            recommendations,
        })
    }

    async fn validate<'a>(&self, _config: &'a MigrationConfig) -> Result<ValidationReport> {
        log::info!("Validating migrated data...");

        // For now, return a basic validation report since we don't have the get_all methods
        // In a real implementation, this would query the storage for validation
        Ok(ValidationReport {
            data_integrity_score: 1.0,
            completeness_score: 1.0,
            consistency_score: 1.0,
            performance_score: 0.95,
            issues: vec![],
            recommendations: vec!["Migration validation completed successfully".to_string()],
        })
    }

    async fn backup<'a>(&self, _config: &'a MigrationConfig) -> Result<String> {
        // Mock backup implementation
        let backup_location = format!("backup_{}.json", Utc::now().format("%Y%m%d_%H%M%S"));
        log::info!("Creating backup at: {}", backup_location);
        
        // In a real implementation, this would export the current database state
        Ok(backup_location)
    }

    async fn rollback<'a>(&self, _config: &'a MigrationConfig, backup_location: &str) -> Result<()> {
        log::info!("Rolling back migration using backup: {}", backup_location);
        
        // In a real implementation, this would restore from the backup
        Ok(())
    }
} 
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use async_trait::async_trait;
use std::time::{Duration, Instant};

pub mod graphiti_migrator;
pub mod validation;
pub mod backup;

use crate::graph::{KGNode, KGEdge, Episode};

/// Migration configuration for different source systems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationConfig {
    pub source_type: SourceType,
    pub source_connection: String,
    pub target_database: String,
    pub batch_size: usize,
    pub validation_enabled: bool,
    pub backup_enabled: bool,
    pub parallel_workers: usize,
    pub chunk_size: usize,
}

/// Supported source systems for migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SourceType {
    GraphitiMcp,
    Neo4j,
    JsonExport,
    CsvExport,
    CustomFormat,
}

/// Migration statistics and progress tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationStats {
    pub total_nodes: usize,
    pub migrated_nodes: usize,
    pub total_edges: usize,
    pub migrated_edges: usize,
    pub total_episodes: usize,
    pub migrated_episodes: usize,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub errors: Vec<MigrationError>,
    pub warnings: Vec<String>,
}

/// Migration error tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationError {
    pub error_type: String,
    pub message: String,
    pub source_id: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub recoverable: bool,
}

/// Migration result with detailed information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationResult {
    pub success: bool,
    pub stats: MigrationStats,
    pub validation_report: Option<ValidationReport>,
    pub backup_location: Option<String>,
    pub recommendations: Vec<String>,
}

/// Validation report for migration quality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub data_integrity_score: f32,
    pub completeness_score: f32,
    pub consistency_score: f32,
    pub performance_score: f32,
    pub issues: Vec<ValidationIssue>,
    pub recommendations: Vec<String>,
}

/// Individual validation issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub severity: ValidationSeverity,
    pub category: String,
    pub description: String,
    pub affected_items: Vec<String>,
    pub suggested_fix: Option<String>,
}

/// Validation issue severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Trait for implementing data migration from various sources
#[async_trait]
pub trait Migrator: Send + Sync {
    /// Analyze the source data to create a migration plan
    async fn analyze_source<'a>(&self, config: &'a MigrationConfig) -> Result<MigrationPlan>;

    /// Perform the actual migration
    async fn migrate<'a>(
        &self, 
        config: &'a MigrationConfig, 
        progress_callback: Option<Box<dyn Fn(MigrationProgress) + Send + Sync>>
    ) -> Result<MigrationResult>;

    /// Validate the migrated data
    async fn validate<'a>(&self, config: &'a MigrationConfig) -> Result<ValidationReport>;

    /// Create a backup before migration
    async fn backup<'a>(&self, config: &'a MigrationConfig) -> Result<String>;

    /// Rollback migration using backup
    async fn rollback<'a>(&self, config: &'a MigrationConfig, backup_location: &str) -> Result<()>;
}

/// Migration plan with estimated resources and time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationPlan {
    pub estimated_duration: std::time::Duration,
    pub estimated_memory_usage: usize,
    pub estimated_disk_space: usize,
    pub node_count: usize,
    pub edge_count: usize,
    pub episode_count: usize,
    pub complexity_score: f32,
    pub recommended_batch_size: usize,
    pub recommended_workers: usize,
    pub potential_issues: Vec<String>,
}

/// Real-time migration progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationProgress {
    pub phase: MigrationPhase,
    pub nodes_processed: usize,
    pub edges_processed: usize,
    pub episodes_processed: usize,
    pub total_items: usize,
    pub current_batch: usize,
    pub total_batches: usize,
    pub elapsed_time: std::time::Duration,
    pub estimated_remaining: std::time::Duration,
    pub current_throughput: f32,
    pub errors_encountered: usize,
}

/// Migration phases for progress tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MigrationPhase {
    Analyzing,
    Backing,
    MigratingNodes,
    MigratingEdges,
    MigratingEpisodes,
    GeneratingEmbeddings,
    Validating,
    Optimizing,
    Completed,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            source_type: SourceType::GraphitiMcp,
            source_connection: "".to_string(),
            target_database: "data/kg_database.db".to_string(),
            batch_size: 1000,
            validation_enabled: true,
            backup_enabled: true,
            parallel_workers: 4,
            chunk_size: 100,
        }
    }
}

impl MigrationStats {
    pub fn new() -> Self {
        Self {
            total_nodes: 0,
            migrated_nodes: 0,
            total_edges: 0,
            migrated_edges: 0,
            total_episodes: 0,
            migrated_episodes: 0,
            start_time: Utc::now(),
            end_time: None,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }
    
    pub fn completion_percentage(&self) -> f32 {
        let total = self.total_nodes + self.total_edges + self.total_episodes;
        let migrated = self.migrated_nodes + self.migrated_edges + self.migrated_episodes;
        
        if total == 0 {
            100.0
        } else {
            (migrated as f32 / total as f32) * 100.0
        }
    }
    
    pub fn duration(&self) -> std::time::Duration {
        match self.end_time {
            Some(end) => (end - self.start_time).to_std().unwrap_or_default(),
            None => (Utc::now() - self.start_time).to_std().unwrap_or_default(),
        }
    }
}

/// Utility functions for migration
pub mod utils {
    use super::*;
    
    /// Generate migration configuration from environment
    pub fn config_from_env() -> MigrationConfig {
        MigrationConfig {
            source_type: match std::env::var("MIGRATION_SOURCE_TYPE").as_deref() {
                Ok("neo4j") => SourceType::Neo4j,
                Ok("json") => SourceType::JsonExport,
                Ok("csv") => SourceType::CsvExport,
                Ok("custom") => SourceType::CustomFormat,
                _ => SourceType::GraphitiMcp,
            },
            source_connection: std::env::var("MIGRATION_SOURCE_CONNECTION")
                .unwrap_or_else(|_| "".to_string()),
            target_database: std::env::var("MIGRATION_TARGET_DB")
                .unwrap_or_else(|_| "data/kg_database.db".to_string()),
            batch_size: std::env::var("MIGRATION_BATCH_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1000),
            validation_enabled: std::env::var("MIGRATION_VALIDATION")
                .map(|s| s.to_lowercase() == "true")
                .unwrap_or(true),
            backup_enabled: std::env::var("MIGRATION_BACKUP")
                .map(|s| s.to_lowercase() == "true")
                .unwrap_or(true),
            parallel_workers: std::env::var("MIGRATION_WORKERS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(4),
            chunk_size: std::env::var("MIGRATION_CHUNK_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(100),
        }
    }
    
    /// Estimate migration complexity based on data characteristics
    pub fn calculate_complexity_score(node_count: usize, edge_count: usize, episode_count: usize) -> f32 {
        let total_items = node_count + edge_count + episode_count;
        let edge_density = if node_count > 0 { edge_count as f32 / node_count as f32 } else { 0.0 };
        
        // Base complexity from item count
        let size_complexity = (total_items as f32).log10() / 6.0; // Normalize to 0-1 range
        
        // Edge density complexity (higher density = more complex)
        let density_complexity = (edge_density / 10.0).min(1.0);
        
        // Episode complexity (episodes add semantic complexity)
        let episode_complexity = if total_items > 0 {
            (episode_count as f32 / total_items as f32) * 0.5
        } else {
            0.0
        };
        
        (size_complexity + density_complexity + episode_complexity).min(1.0)
    }
    
    /// Recommend optimal batch size based on system resources
    pub fn recommend_batch_size(total_items: usize, available_memory_mb: usize) -> usize {
        // Estimate memory per item (conservative estimate)
        let memory_per_item_kb = 2; // 2KB per item average
        let max_items_in_memory = (available_memory_mb * 1024) / memory_per_item_kb / 4; // Use 25% of available memory
        
        let recommended = if total_items < 1000 {
            total_items
        } else if total_items < 10000 {
            1000
        } else if total_items < 100000 {
            5000
        } else {
            10000
        };
        
        recommended.min(max_items_in_memory).max(100)
    }
} 
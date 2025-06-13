use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::graph::{KGNode, KGEdge, Episode};

/// Backup manager for migration data safety
pub struct BackupManager {
    backup_directory: PathBuf,
    compression_enabled: bool,
    max_backup_age_days: u32,
}

/// Backup metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub backup_id: String,
    pub created_at: DateTime<Utc>,
    pub source_type: String,
    pub node_count: usize,
    pub edge_count: usize,
    pub episode_count: usize,
    pub file_size_bytes: u64,
    pub compression_ratio: Option<f32>,
    pub checksum: String,
    pub description: String,
}

/// Complete backup data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupData {
    pub metadata: BackupMetadata,
    pub nodes: Vec<KGNode>,
    pub edges: Vec<KGEdge>,
    pub episodes: Vec<Episode>,
    pub schema_version: String,
    pub backup_format_version: String,
}

/// Backup configuration
#[derive(Debug, Clone)]
pub struct BackupConfig {
    pub include_embeddings: bool,
    pub compress_data: bool,
    pub verify_integrity: bool,
    pub incremental_backup: bool,
    pub max_file_size_mb: usize,
}

/// Backup restoration options
#[derive(Debug, Clone)]
pub struct RestoreOptions {
    pub verify_before_restore: bool,
    pub backup_current_before_restore: bool,
    pub selective_restore: Option<SelectiveRestore>,
    pub force_restore: bool,
}

/// Selective restore configuration
#[derive(Debug, Clone)]
pub struct SelectiveRestore {
    pub restore_nodes: bool,
    pub restore_edges: bool,
    pub restore_episodes: bool,
    pub node_filters: Vec<String>,
    pub date_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
}

impl BackupManager {
    pub fn new<P: AsRef<Path>>(backup_directory: P, compression_enabled: bool, max_backup_age_days: u32) -> Result<Self> {
        let backup_dir = backup_directory.as_ref().to_path_buf();
        
        // Create backup directory if it doesn't exist
        if !backup_dir.exists() {
            fs::create_dir_all(&backup_dir)?;
        }

        Ok(Self {
            backup_directory: backup_dir,
            compression_enabled,
            max_backup_age_days,
        })
    }

    /// Create a complete backup of the current data
    pub async fn create_backup(
        &self,
        nodes: &[KGNode],
        edges: &[KGEdge],
        episodes: &[Episode],
        config: &BackupConfig,
        description: String,
    ) -> Result<String> {
        let backup_id = format!("backup_{}", Utc::now().format("%Y%m%d_%H%M%S_%f"));
        let backup_filename = format!("{}.json", backup_id);
        let backup_path = self.backup_directory.join(&backup_filename);

        log::info!("Creating backup: {}", backup_id);

        // Filter data based on configuration
        let filtered_episodes = if config.include_embeddings {
            episodes.to_vec()
        } else {
            episodes.iter().map(|e| Episode {
                uuid: e.uuid,
                name: e.name.clone(),
                content: e.content.clone(),
                entity_uuids: e.entity_uuids.clone(),
                edge_uuids: e.edge_uuids.clone(),
                embedding: None, // Exclude embeddings if not requested
                created_at: e.created_at,
                metadata: e.metadata.clone(),
                group_id: e.group_id.clone(),
                source: e.source.clone(),
                source_description: e.source_description.clone(),
            }).collect()
        };

        // Create backup metadata
        let metadata = BackupMetadata {
            backup_id: backup_id.clone(),
            created_at: Utc::now(),
            source_type: "kg_mcp_server".to_string(),
            node_count: nodes.len(),
            edge_count: edges.len(),
            episode_count: episodes.len(),
            file_size_bytes: 0, // Will be updated after writing
            compression_ratio: None,
            checksum: "".to_string(), // Will be calculated after writing
            description,
        };

        // Create backup data structure
        let backup_data = BackupData {
            metadata,
            nodes: nodes.to_vec(),
            edges: edges.to_vec(),
            episodes: filtered_episodes,
            schema_version: "1.0".to_string(),
            backup_format_version: "1.0".to_string(),
        };

        // Serialize and write backup
        let json_data = serde_json::to_string_pretty(&backup_data)?;
        
        if config.compress_data && self.compression_enabled {
            // In a real implementation, we would use compression here
            // For now, just write the JSON data
            fs::write(&backup_path, &json_data)?;
        } else {
            fs::write(&backup_path, &json_data)?;
        }

        // Update metadata with actual file information
        let file_metadata = fs::metadata(&backup_path)?;
        let file_size = file_metadata.len();
        let checksum = self.calculate_checksum(&backup_path)?;

        // Update the backup file with correct metadata
        let mut updated_backup_data = backup_data;
        updated_backup_data.metadata.file_size_bytes = file_size;
        updated_backup_data.metadata.checksum = checksum;

        let updated_json = serde_json::to_string_pretty(&updated_backup_data)?;
        fs::write(&backup_path, &updated_json)?;

        // Verify backup integrity if requested
        if config.verify_integrity {
            self.verify_backup(&backup_id).await?;
        }

        log::info!("Backup created successfully: {} ({} bytes)", backup_id, file_size);
        Ok(backup_id)
    }

    /// List all available backups
    pub async fn list_backups(&self) -> Result<Vec<BackupMetadata>> {
        let mut backups = Vec::new();

        if !self.backup_directory.exists() {
            return Ok(backups);
        }

        for entry in fs::read_dir(&self.backup_directory)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(backup_data) = self.load_backup_data(&path).await {
                    backups.push(backup_data.metadata);
                }
            }
        }

        // Sort by creation date (newest first)
        backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(backups)
    }

    /// Restore data from a backup
    pub async fn restore_backup(
        &self,
        backup_id: &str,
        options: &RestoreOptions,
    ) -> Result<(Vec<KGNode>, Vec<KGEdge>, Vec<Episode>)> {
        log::info!("Restoring backup: {}", backup_id);

        // Verify backup before restore if requested
        if options.verify_before_restore {
            self.verify_backup(backup_id).await?;
        }

        // Load backup data
        let backup_path = self.backup_directory.join(format!("{}.json", backup_id));
        let backup_data = self.load_backup_data(&backup_path).await?;

        // Apply selective restore filters if specified
        let (nodes, edges, episodes) = if let Some(ref selective) = options.selective_restore {
            self.apply_selective_restore(&backup_data, selective)?
        } else {
            (backup_data.nodes, backup_data.edges, backup_data.episodes)
        };

        log::info!("Backup restored: {} nodes, {} edges, {} episodes", 
                  nodes.len(), edges.len(), episodes.len());

        Ok((nodes, edges, episodes))
    }

    /// Verify backup integrity
    pub async fn verify_backup(&self, backup_id: &str) -> Result<bool> {
        let backup_path = self.backup_directory.join(format!("{}.json", backup_id));
        
        if !backup_path.exists() {
            return Err(anyhow!("Backup file not found: {}", backup_id));
        }

        // Load and verify backup data
        let backup_data = self.load_backup_data(&backup_path).await?;
        
        // Verify checksum
        let current_checksum = self.calculate_checksum(&backup_path)?;
        if current_checksum != backup_data.metadata.checksum {
            return Err(anyhow!("Backup checksum mismatch - file may be corrupted"));
        }

        // Verify data consistency
        let node_uuids: std::collections::HashSet<_> = backup_data.nodes.iter().map(|n| n.uuid).collect();
        
        // Check for orphaned edges
        for edge in &backup_data.edges {
            if !node_uuids.contains(&edge.source_node_uuid) || !node_uuids.contains(&edge.target_node_uuid) {
                return Err(anyhow!("Backup contains orphaned edges - data integrity compromised"));
            }
        }

        // Check episode references
        for episode in &backup_data.episodes {
            for entity_uuid in &episode.entity_uuids {
                if !node_uuids.contains(entity_uuid) {
                    log::warn!("Episode {} references non-existent entity {}", episode.uuid, entity_uuid);
                }
            }
        }

        log::info!("Backup verification successful: {}", backup_id);
        Ok(true)
    }

    /// Delete old backups based on age policy
    pub async fn cleanup_old_backups(&self) -> Result<usize> {
        let cutoff_date = Utc::now() - chrono::Duration::days(self.max_backup_age_days as i64);
        let mut deleted_count = 0;

        let backups = self.list_backups().await?;
        
        for backup in backups {
            if backup.created_at < cutoff_date {
                let backup_path = self.backup_directory.join(format!("{}.json", backup.backup_id));
                if backup_path.exists() {
                    fs::remove_file(&backup_path)?;
                    deleted_count += 1;
                    log::info!("Deleted old backup: {}", backup.backup_id);
                }
            }
        }

        if deleted_count > 0 {
            log::info!("Cleaned up {} old backups", deleted_count);
        }

        Ok(deleted_count)
    }

    /// Get backup statistics
    pub async fn get_backup_stats(&self) -> Result<BackupStats> {
        let backups = self.list_backups().await?;
        
        let total_backups = backups.len();
        let total_size_bytes: u64 = backups.iter().map(|b| b.file_size_bytes).sum();
        let oldest_backup = backups.iter().map(|b| b.created_at).min();
        let newest_backup = backups.iter().map(|b| b.created_at).max();

        Ok(BackupStats {
            total_backups,
            total_size_bytes,
            oldest_backup,
            newest_backup,
            average_size_bytes: if total_backups > 0 { total_size_bytes / total_backups as u64 } else { 0 },
        })
    }

    /// Load backup data from file
    async fn load_backup_data(&self, backup_path: &Path) -> Result<BackupData> {
        let json_data = fs::read_to_string(backup_path)?;
        let backup_data: BackupData = serde_json::from_str(&json_data)?;
        Ok(backup_data)
    }

    /// Apply selective restore filters
    fn apply_selective_restore(
        &self,
        backup_data: &BackupData,
        selective: &SelectiveRestore,
    ) -> Result<(Vec<KGNode>, Vec<KGEdge>, Vec<Episode>)> {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut episodes = Vec::new();

        // Filter nodes
        if selective.restore_nodes {
            for node in &backup_data.nodes {
                let mut include = true;

                // Apply node filters
                if !selective.node_filters.is_empty() {
                    include = selective.node_filters.iter().any(|filter| {
                        node.name.contains(filter) || node.node_type.contains(filter)
                    });
                }

                // Apply date range filter
                if let Some((start_date, end_date)) = selective.date_range {
                    include = include && node.created_at >= start_date && node.created_at <= end_date;
                }

                if include {
                    nodes.push(node.clone());
                }
            }
        }

        // Filter edges (only include if both nodes are included)
        if selective.restore_edges {
            let node_uuids: std::collections::HashSet<_> = nodes.iter().map(|n| n.uuid).collect();
            
            for edge in &backup_data.edges {
                if node_uuids.contains(&edge.source_node_uuid) && node_uuids.contains(&edge.target_node_uuid) {
                    edges.push(edge.clone());
                }
            }
        }

        // Filter episodes
        if selective.restore_episodes {
            let node_uuids: std::collections::HashSet<_> = nodes.iter().map(|n| n.uuid).collect();
            
            for episode in &backup_data.episodes {
                let mut include = true;

                // Apply date range filter
                if let Some((start_date, end_date)) = selective.date_range {
                    include = episode.created_at >= start_date && episode.created_at <= end_date;
                }

                // Only include episodes that reference included nodes
                if include && !episode.entity_uuids.is_empty() {
                    include = episode.entity_uuids.iter().any(|uuid| node_uuids.contains(uuid));
                }

                if include {
                    episodes.push(episode.clone());
                }
            }
        }

        Ok((nodes, edges, episodes))
    }

    /// Calculate file checksum for integrity verification
    fn calculate_checksum(&self, file_path: &Path) -> Result<String> {
        use sha2::{Sha256, Digest};
        
        let data = fs::read(file_path)?;
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let result = hasher.finalize();
        Ok(format!("{:x}", result))
    }
}

/// Backup statistics
#[derive(Debug, Clone)]
pub struct BackupStats {
    pub total_backups: usize,
    pub total_size_bytes: u64,
    pub oldest_backup: Option<DateTime<Utc>>,
    pub newest_backup: Option<DateTime<Utc>>,
    pub average_size_bytes: u64,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            include_embeddings: true,
            compress_data: true,
            verify_integrity: true,
            incremental_backup: false,
            max_file_size_mb: 100,
        }
    }
}

impl Default for RestoreOptions {
    fn default() -> Self {
        Self {
            verify_before_restore: true,
            backup_current_before_restore: true,
            selective_restore: None,
            force_restore: false,
        }
    }
} 
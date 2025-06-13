use anyhow::Result;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use chrono::{DateTime, Utc};

use super::{ValidationReport, ValidationIssue, ValidationSeverity};
use crate::graph::{KGNode, KGEdge, Episode};

/// Data validator for migration quality assurance
pub struct DataValidator {
    strict_mode: bool,
    performance_threshold: f32,
}

/// Validation configuration
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    pub check_referential_integrity: bool,
    pub check_data_completeness: bool,
    pub check_performance_metrics: bool,
    pub check_embedding_quality: bool,
    pub strict_mode: bool,
    pub performance_threshold: f32,
}

/// Detailed validation statistics
#[derive(Debug, Clone)]
pub struct ValidationStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub total_episodes: usize,
    pub orphaned_edges: usize,
    pub invalid_references: usize,
    pub missing_embeddings: usize,
    pub duplicate_entities: usize,
    pub performance_score: f32,
    pub validation_duration: std::time::Duration,
}

impl DataValidator {
    pub fn new(strict_mode: bool, performance_threshold: f32) -> Self {
        Self {
            strict_mode,
            performance_threshold,
        }
    }

    /// Comprehensive data validation
    pub async fn validate_data(
        &self,
        nodes: &[KGNode],
        edges: &[KGEdge],
        episodes: &[Episode],
        config: &ValidationConfig,
    ) -> Result<ValidationReport> {
        let start_time = std::time::Instant::now();
        let mut issues = Vec::new();
        let mut data_integrity_score = 1.0f32;
        let mut completeness_score = 1.0f32;
        let mut consistency_score = 1.0f32;
        let mut performance_score = config.performance_threshold;

        // Build lookup maps for efficient validation
        let node_map: HashMap<Uuid, &KGNode> = nodes.iter().map(|n| (n.uuid, n)).collect();
        let edge_map: HashMap<Uuid, &KGEdge> = edges.iter().map(|e| (e.uuid, e)).collect();
        let episode_map: HashMap<Uuid, &Episode> = episodes.iter().map(|e| (e.uuid, e)).collect();

        // 1. Referential Integrity Checks
        if config.check_referential_integrity {
            let integrity_issues = self.check_referential_integrity(&node_map, &edge_map, &episode_map).await?;
            if !integrity_issues.is_empty() {
                data_integrity_score -= 0.3;
                issues.extend(integrity_issues);
            }
        }

        // 2. Data Completeness Checks
        if config.check_data_completeness {
            let completeness_issues = self.check_data_completeness(nodes, edges, episodes).await?;
            if !completeness_issues.is_empty() {
                completeness_score -= 0.2;
                issues.extend(completeness_issues);
            }
        }

        // 3. Data Consistency Checks
        let consistency_issues = self.check_data_consistency(nodes, edges, episodes).await?;
        if !consistency_issues.is_empty() {
            consistency_score -= 0.2;
            issues.extend(consistency_issues);
        }

        // 4. Performance Metrics
        if config.check_performance_metrics {
            performance_score = self.calculate_performance_score(nodes, edges, episodes).await?;
        }

        // 5. Embedding Quality Checks
        if config.check_embedding_quality {
            let embedding_issues = self.check_embedding_quality(episodes).await?;
            if !embedding_issues.is_empty() {
                completeness_score -= 0.1;
                issues.extend(embedding_issues);
            }
        }

        // Generate recommendations based on issues found
        let recommendations = self.generate_recommendations(&issues, &ValidationStats {
            total_nodes: nodes.len(),
            total_edges: edges.len(),
            total_episodes: episodes.len(),
            orphaned_edges: issues.iter().filter(|i| i.category == "Orphaned Edges").count(),
            invalid_references: issues.iter().filter(|i| i.category == "Invalid References").count(),
            missing_embeddings: issues.iter().filter(|i| i.category == "Missing Embeddings").count(),
            duplicate_entities: issues.iter().filter(|i| i.category == "Duplicate Entities").count(),
            performance_score,
            validation_duration: start_time.elapsed(),
        });

        Ok(ValidationReport {
            data_integrity_score: data_integrity_score.max(0.0),
            completeness_score: completeness_score.max(0.0),
            consistency_score: consistency_score.max(0.0),
            performance_score,
            issues,
            recommendations,
        })
    }

    /// Check referential integrity between nodes, edges, and episodes
    async fn check_referential_integrity(
        &self,
        node_map: &HashMap<Uuid, &KGNode>,
        edge_map: &HashMap<Uuid, &KGEdge>,
        episode_map: &HashMap<Uuid, &Episode>,
    ) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();

        // Check for orphaned edges (edges pointing to non-existent nodes)
        for (edge_uuid, edge) in edge_map {
            if !node_map.contains_key(&edge.source_node_uuid) {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    category: "Orphaned Edges".to_string(),
                    description: format!("Edge {} has invalid source node {}", edge_uuid, edge.source_node_uuid),
                    affected_items: vec![edge_uuid.to_string()],
                    suggested_fix: Some("Remove edge or add missing source node".to_string()),
                });
            }

            if !node_map.contains_key(&edge.target_node_uuid) {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    category: "Orphaned Edges".to_string(),
                    description: format!("Edge {} has invalid target node {}", edge_uuid, edge.target_node_uuid),
                    affected_items: vec![edge_uuid.to_string()],
                    suggested_fix: Some("Remove edge or add missing target node".to_string()),
                });
            }
        }

        // Check for invalid entity references in episodes
        for (episode_uuid, episode) in episode_map {
            for entity_uuid in &episode.entity_uuids {
                if !node_map.contains_key(entity_uuid) {
                    issues.push(ValidationIssue {
                        severity: ValidationSeverity::Warning,
                        category: "Invalid References".to_string(),
                        description: format!("Episode {} references non-existent entity {}", episode_uuid, entity_uuid),
                        affected_items: vec![episode_uuid.to_string()],
                        suggested_fix: Some("Remove invalid entity reference or add missing entity".to_string()),
                    });
                }
            }

            for edge_uuid in &episode.edge_uuids {
                if !edge_map.contains_key(edge_uuid) {
                    issues.push(ValidationIssue {
                        severity: ValidationSeverity::Warning,
                        category: "Invalid References".to_string(),
                        description: format!("Episode {} references non-existent edge {}", episode_uuid, edge_uuid),
                        affected_items: vec![episode_uuid.to_string()],
                        suggested_fix: Some("Remove invalid edge reference or add missing edge".to_string()),
                    });
                }
            }
        }

        Ok(issues)
    }

    /// Check data completeness (missing required fields, empty values)
    async fn check_data_completeness(
        &self,
        nodes: &[KGNode],
        edges: &[KGEdge],
        episodes: &[Episode],
    ) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();

        // Check for nodes with missing essential data
        for node in nodes {
            if node.name.trim().is_empty() {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Warning,
                    category: "Missing Data".to_string(),
                    description: format!("Node {} has empty name", node.uuid),
                    affected_items: vec![node.uuid.to_string()],
                    suggested_fix: Some("Provide a meaningful name for the node".to_string()),
                });
            }

            if node.node_type.trim().is_empty() {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Warning,
                    category: "Missing Data".to_string(),
                    description: format!("Node {} has empty type", node.uuid),
                    affected_items: vec![node.uuid.to_string()],
                    suggested_fix: Some("Assign an appropriate type to the node".to_string()),
                });
            }
        }

        // Check for edges with missing relation types
        for edge in edges {
            if edge.relation_type.trim().is_empty() {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Warning,
                    category: "Missing Data".to_string(),
                    description: format!("Edge {} has empty relation type", edge.uuid),
                    affected_items: vec![edge.uuid.to_string()],
                    suggested_fix: Some("Assign a meaningful relation type to the edge".to_string()),
                });
            }

            if edge.weight <= 0.0 || edge.weight > 1.0 {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Info,
                    category: "Data Quality".to_string(),
                    description: format!("Edge {} has unusual weight: {}", edge.uuid, edge.weight),
                    affected_items: vec![edge.uuid.to_string()],
                    suggested_fix: Some("Verify edge weight is in expected range (0.0-1.0)".to_string()),
                });
            }
        }

        // Check for episodes with missing content
        for episode in episodes {
            if episode.content.trim().is_empty() {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    category: "Missing Data".to_string(),
                    description: format!("Episode {} has empty content", episode.uuid),
                    affected_items: vec![episode.uuid.to_string()],
                    suggested_fix: Some("Provide content for the episode".to_string()),
                });
            }

            if episode.entity_uuids.is_empty() && episode.edge_uuids.is_empty() {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Warning,
                    category: "Missing Data".to_string(),
                    description: format!("Episode {} has no entity or edge references", episode.uuid),
                    affected_items: vec![episode.uuid.to_string()],
                    suggested_fix: Some("Add entity or edge references to the episode".to_string()),
                });
            }
        }

        Ok(issues)
    }

    /// Check data consistency (duplicates, conflicts)
    async fn check_data_consistency(
        &self,
        nodes: &[KGNode],
        edges: &[KGEdge],
        episodes: &[Episode],
    ) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();

        // Check for duplicate node names within the same type
        let mut name_type_map: HashMap<(String, String), Vec<Uuid>> = HashMap::new();
        for node in nodes {
            let key = (node.name.clone(), node.node_type.clone());
            name_type_map.entry(key).or_default().push(node.uuid);
        }

        for ((name, node_type), uuids) in name_type_map {
            if uuids.len() > 1 {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Warning,
                    category: "Duplicate Entities".to_string(),
                    description: format!("Found {} nodes with same name '{}' and type '{}'", uuids.len(), name, node_type),
                    affected_items: uuids.iter().map(|u| u.to_string()).collect(),
                    suggested_fix: Some("Consider merging duplicate entities or adding distinguishing information".to_string()),
                });
            }
        }

        // Check for duplicate edges between same nodes
        let mut edge_map: HashMap<(Uuid, Uuid, String), Vec<Uuid>> = HashMap::new();
        for edge in edges {
            let key = (edge.source_node_uuid, edge.target_node_uuid, edge.relation_type.clone());
            edge_map.entry(key).or_default().push(edge.uuid);
        }

        for ((source, target, relation), uuids) in edge_map {
            if uuids.len() > 1 {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Info,
                    category: "Duplicate Relationships".to_string(),
                    description: format!("Found {} duplicate '{}' relationships between {} and {}", 
                                       uuids.len(), relation, source, target),
                    affected_items: uuids.iter().map(|u| u.to_string()).collect(),
                    suggested_fix: Some("Consider consolidating duplicate relationships".to_string()),
                });
            }
        }

        // Check for temporal consistency
        for node in nodes {
            if node.updated_at < node.created_at {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    category: "Temporal Inconsistency".to_string(),
                    description: format!("Node {} has update time before creation time", node.uuid),
                    affected_items: vec![node.uuid.to_string()],
                    suggested_fix: Some("Correct the timestamps".to_string()),
                });
            }
        }

        Ok(issues)
    }

    /// Check embedding quality and coverage
    async fn check_embedding_quality(&self, episodes: &[Episode]) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();

        let episodes_with_embeddings = episodes.iter().filter(|e| e.embedding.is_some()).count();
        let total_episodes = episodes.len();

        if total_episodes > 0 {
            let embedding_coverage = episodes_with_embeddings as f32 / total_episodes as f32;
            
            if embedding_coverage < 0.5 {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Warning,
                    category: "Missing Embeddings".to_string(),
                    description: format!("Only {:.1}% of episodes have embeddings", embedding_coverage * 100.0),
                    affected_items: vec![],
                    suggested_fix: Some("Generate embeddings for episodes to enable semantic search".to_string()),
                });
            }

            // Check embedding dimensions consistency
            let mut embedding_dimensions = HashSet::new();
            for episode in episodes {
                if let Some(ref embedding) = episode.embedding {
                    embedding_dimensions.insert(embedding.len());
                }
            }

            if embedding_dimensions.len() > 1 {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    category: "Embedding Inconsistency".to_string(),
                    description: format!("Found embeddings with different dimensions: {:?}", embedding_dimensions),
                    affected_items: vec![],
                    suggested_fix: Some("Ensure all embeddings use the same model and dimensions".to_string()),
                });
            }
        }

        Ok(issues)
    }

    /// Calculate overall performance score
    async fn calculate_performance_score(
        &self,
        nodes: &[KGNode],
        edges: &[KGEdge],
        episodes: &[Episode],
    ) -> Result<f32> {
        let total_items = nodes.len() + edges.len() + episodes.len();
        
        // Base score from data size (larger datasets are more complex)
        let size_score = if total_items < 1000 {
            1.0
        } else if total_items < 10000 {
            0.9
        } else if total_items < 100000 {
            0.8
        } else {
            0.7
        };

        // Connectivity score (well-connected graphs are better)
        let connectivity_score = if nodes.is_empty() {
            0.0
        } else {
            let avg_connections = edges.len() as f32 / nodes.len() as f32;
            (avg_connections / 10.0).min(1.0) // Normalize to 0-1, assuming 10 connections per node is optimal
        };

        // Episode coverage score
        let episode_score = if nodes.is_empty() {
            0.0
        } else {
            let episode_coverage = episodes.len() as f32 / nodes.len() as f32;
            (episode_coverage * 2.0).min(1.0) // Normalize assuming 0.5 episodes per node is good
        };

        // Weighted average
        let performance_score = (size_score * 0.4) + (connectivity_score * 0.3) + (episode_score * 0.3);
        
        Ok(performance_score)
    }

    /// Generate actionable recommendations based on validation results
    fn generate_recommendations(&self, issues: &[ValidationIssue], stats: &ValidationStats) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Critical issues
        let critical_issues = issues.iter().filter(|i| matches!(i.severity, ValidationSeverity::Critical)).count();
        if critical_issues > 0 {
            recommendations.push(format!("URGENT: Address {} critical issues before proceeding", critical_issues));
        }

        // Error issues
        let error_issues = issues.iter().filter(|i| matches!(i.severity, ValidationSeverity::Error)).count();
        if error_issues > 0 {
            recommendations.push(format!("Fix {} error-level issues to improve data quality", error_issues));
        }

        // Performance recommendations
        if stats.performance_score < self.performance_threshold {
            recommendations.push(format!("Performance score ({:.2}) below threshold ({:.2}) - consider data optimization", 
                                       stats.performance_score, self.performance_threshold));
        }

        // Specific recommendations based on issue patterns
        if stats.orphaned_edges > 0 {
            recommendations.push("Clean up orphaned edges to improve graph consistency".to_string());
        }

        if stats.missing_embeddings > stats.total_episodes / 2 {
            recommendations.push("Generate embeddings for episodes to enable semantic search capabilities".to_string());
        }

        if stats.duplicate_entities > 0 {
            recommendations.push("Review and consolidate duplicate entities to reduce redundancy".to_string());
        }

        // Data size recommendations
        if stats.total_nodes > 100000 {
            recommendations.push("Large dataset detected - consider implementing data archiving strategy".to_string());
        }

        if recommendations.is_empty() {
            recommendations.push("Data validation passed - no major issues detected".to_string());
        }

        recommendations
    }
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            check_referential_integrity: true,
            check_data_completeness: true,
            check_performance_metrics: true,
            check_embedding_quality: true,
            strict_mode: false,
            performance_threshold: 0.7,
        }
    }
} 
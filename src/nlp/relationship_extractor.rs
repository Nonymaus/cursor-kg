use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;
use tracing::{debug, info};

use crate::graph::KGEdge;
use super::ExtractedEntity;
use crate::embeddings::LocalEmbeddingEngine;

/// Configuration for relationship extraction
#[derive(Debug, Clone)]
pub struct RelationshipExtractionConfig {
    pub min_confidence: f32,
    pub max_relationships_per_text: usize,
    pub enable_semantic_analysis: bool,
    pub custom_patterns: Vec<RelationshipPattern>,
}

impl Default for RelationshipExtractionConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.6,
            max_relationships_per_text: 50,
            enable_semantic_analysis: true,
            custom_patterns: Vec::new(),
        }
    }
}

/// Pattern for custom relationship recognition
#[derive(Debug, Clone)]
pub struct RelationshipPattern {
    pub name: String,
    pub pattern: String,
    pub relationship_type: String,
    pub confidence: f32,
}

/// Extracted relationship information
#[derive(Debug, Clone)]
pub struct ExtractedRelationship {
    pub source_entity: String,
    pub target_entity: String,
    pub relation_type: String,
    pub summary: String,
    pub confidence: f32,
    pub context: String,
    pub weight: f32,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Advanced relationship extractor
#[derive(Clone)]
pub struct RelationshipExtractor {
    config: RelationshipExtractionConfig,
    embedding_engine: Option<std::sync::Arc<LocalEmbeddingEngine>>,
    patterns: Vec<CompiledRelationshipPattern>,
    // Compiled regex patterns for relationship detection
    usage_pattern: Regex,
    comparison_pattern: Regex,
    causation_pattern: Regex,
    temporal_pattern: Regex,
    location_pattern: Regex,
}

#[derive(Clone)]
struct CompiledRelationshipPattern {
    name: String,
    regex: Regex,
    relationship_type: String,
    confidence: f32,
}

impl RelationshipExtractor {
    pub fn new(config: RelationshipExtractionConfig, embedding_engine: Option<std::sync::Arc<LocalEmbeddingEngine>>) -> Result<Self> {
        let mut patterns = Vec::new();
        
        // Compile built-in patterns
        patterns.extend(Self::compile_builtin_patterns()?);
        
        // Compile custom patterns
        for custom_pattern in &config.custom_patterns {
            if let Ok(regex) = Regex::new(&custom_pattern.pattern) {
                patterns.push(CompiledRelationshipPattern {
                    name: custom_pattern.name.clone(),
                    regex,
                    relationship_type: custom_pattern.relationship_type.clone(),
                    confidence: custom_pattern.confidence,
                });
            }
        }

        Ok(Self {
            config,
            embedding_engine,
            patterns,
            usage_pattern: Regex::new(r"\b(\w+)\s+(?:uses?|with|via|through|using)\s+(\w+)\b")?,
            comparison_pattern: Regex::new(r"\b(\w+)\s+(?:vs|versus|compared to|better than|worse than)\s+(\w+)\b")?,
            causation_pattern: Regex::new(r"\b(\w+)\s+(?:causes?|leads to|results in|enables?)\s+(\w+)\b")?,
            temporal_pattern: Regex::new(r"\b(\w+)\s+(?:before|after|during|while)\s+(\w+)\b")?,
            location_pattern: Regex::new(r"\b(\w+)\s+(?:at|in|on|from)\s+(\w+)\b")?,
        })
    }

    /// Extract relationships from text
    pub async fn extract_relationships(&self, text: &str) -> Result<Vec<ExtractedRelationship>> {
        let mut relationships = Vec::new();

        // Pattern-based extraction
        for pattern in &self.patterns {
            for _mat in pattern.regex.find_iter(text) {
                // For now, create placeholder relationships
                // In a real implementation, you'd parse the matched text to extract source/target
                let relationship = ExtractedRelationship {
                    source_entity: "placeholder_source".to_string(),
                    target_entity: "placeholder_target".to_string(),
                    relation_type: pattern.relationship_type.clone(),
                    summary: format!("Relationship extracted using pattern: {}", pattern.name),
                    confidence: pattern.confidence,
                    context: text.to_string(),
                    weight: pattern.confidence,
                    metadata: HashMap::new(),
                };

                if relationship.confidence >= self.config.min_confidence {
                    relationships.push(relationship);
                }
            }
        }

        // Limit number of relationships
        relationships.truncate(self.config.max_relationships_per_text);

        debug!("Extracted {} relationships from text", relationships.len());
        Ok(relationships)
    }

    fn compile_builtin_patterns() -> Result<Vec<CompiledRelationshipPattern>> {
        let mut patterns = Vec::new();

        // Function calls
        patterns.push(CompiledRelationshipPattern {
            name: "function_call".to_string(),
            regex: Regex::new(r"([a-zA-Z_][a-zA-Z0-9_]*)\s*\(")?,
            relationship_type: "calls".to_string(),
            confidence: 0.8,
        });

        // Inheritance
        patterns.push(CompiledRelationshipPattern {
            name: "inheritance".to_string(),
            regex: Regex::new(r"class\s+([A-Z][a-zA-Z0-9_]*)\s*:\s*([A-Z][a-zA-Z0-9_]*)")?,
            relationship_type: "inherits".to_string(),
            confidence: 0.9,
        });

        // Imports
        patterns.push(CompiledRelationshipPattern {
            name: "import".to_string(),
            regex: Regex::new(r"import\s+([a-zA-Z_][a-zA-Z0-9_.]*)")?,
            relationship_type: "imports".to_string(),
            confidence: 0.9,
        });

        Ok(patterns)
    }

    /// Extract relationships between entities
    pub async fn extract_relationships_between_entities(
        &self,
        entities: &[ExtractedEntity],
        content: &str,
        episode_name: &str,
    ) -> Result<Vec<ExtractedRelationship>> {
        let mut relationships = Vec::new();

        // Extract co-occurrence relationships
        relationships.extend(self.extract_co_occurrence_relationships(entities, content, episode_name)?);

        // Extract semantic relationships
        if self.config.enable_semantic_analysis {
            relationships.extend(self.extract_semantic_relationships(entities, content, episode_name)?);
        }

        // Extract pattern-based relationships
        relationships.extend(self.extract_pattern_relationships(content, episode_name)?);

        // Filter by confidence
        relationships.retain(|r| r.confidence >= self.config.min_confidence);

        // Limit results
        relationships.truncate(self.config.max_relationships_per_text);

        Ok(relationships)
    }

    /// Extract co-occurrence relationships between entities
    fn extract_co_occurrence_relationships(
        &self,
        entities: &[ExtractedEntity],
        content: &str,
        episode_name: &str,
    ) -> Result<Vec<ExtractedRelationship>> {
        let mut relationships = Vec::new();
        let co_occurrence_window = 100; // characters

        for (i, entity1) in entities.iter().enumerate() {
            for entity2 in entities.iter().skip(i + 1) {
                let distance = self.calculate_entity_distance(content, &entity1.name, &entity2.name);
                if distance <= co_occurrence_window {
                    let confidence = 1.0 - (distance as f32 / co_occurrence_window as f32);
                    let context = self.get_context_between_entities(content, &entity1.name, &entity2.name);
                    
                    relationships.push(ExtractedRelationship {
                        source_entity: entity1.name.clone(),
                        target_entity: entity2.name.clone(),
                        relation_type: "co_occurs_with".to_string(),
                        summary: format!("{} co-occurs with {} in {}", entity1.name, entity2.name, episode_name),
                        confidence,
                        context,
                        weight: confidence,
                        metadata: {
                            let mut meta = HashMap::new();
                            meta.insert("distance".to_string(), serde_json::Value::Number(serde_json::Number::from(distance)));
                            meta.insert("episode".to_string(), serde_json::Value::String(episode_name.to_string()));
                            meta
                        },
                    });
                }
            }
        }

        Ok(relationships)
    }

    /// Extract semantic relationships using pattern matching
    fn extract_semantic_relationships(
        &self,
        _entities: &[ExtractedEntity],
        content: &str,
        episode_name: &str,
    ) -> Result<Vec<ExtractedRelationship>> {
        let mut relationships = Vec::new();

        // Extract usage relationships
        for usage_match in self.usage_pattern.find_iter(content) {
            if let Some(caps) = self.usage_pattern.captures(usage_match.as_str()) {
                if let (Some(source_match), Some(target_match)) = (caps.get(1), caps.get(2)) {
                    let source_name = source_match.as_str();
                    let target_name = target_match.as_str();
                    
                    relationships.push(ExtractedRelationship {
                        source_entity: source_name.to_string(),
                        target_entity: target_name.to_string(),
                        relation_type: "uses".to_string(),
                        summary: format!("{} uses {} in {}", source_name, target_name, episode_name),
                        confidence: 0.8,
                        context: self.get_context_around_match(content, usage_match.start(), usage_match.end()),
                        weight: 0.8,
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        // Extract comparison relationships
        for comp_match in self.comparison_pattern.find_iter(content) {
            if let Some(caps) = self.comparison_pattern.captures(comp_match.as_str()) {
                if let (Some(source_match), Some(target_match)) = (caps.get(1), caps.get(2)) {
                    let source_name = source_match.as_str();
                    let target_name = target_match.as_str();
                    
                    relationships.push(ExtractedRelationship {
                        source_entity: source_name.to_string(),
                        target_entity: target_name.to_string(),
                        relation_type: "compared_to".to_string(),
                        summary: format!("{} compared to {} in {}", source_name, target_name, episode_name),
                        confidence: 0.7,
                        context: self.get_context_around_match(content, comp_match.start(), comp_match.end()),
                        weight: 0.7,
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        // Extract causation relationships
        for cause_match in self.causation_pattern.find_iter(content) {
            if let Some(caps) = self.causation_pattern.captures(cause_match.as_str()) {
                if let (Some(source_match), Some(target_match)) = (caps.get(1), caps.get(2)) {
                    let source_name = source_match.as_str();
                    let target_name = target_match.as_str();
                    
                    relationships.push(ExtractedRelationship {
                        source_entity: source_name.to_string(),
                        target_entity: target_name.to_string(),
                        relation_type: "causes".to_string(),
                        summary: format!("{} causes {} in {}", source_name, target_name, episode_name),
                        confidence: 0.9,
                        context: self.get_context_around_match(content, cause_match.start(), cause_match.end()),
                        weight: 0.9,
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        // Extract temporal relationships
        for temp_match in self.temporal_pattern.find_iter(content) {
            if let Some(caps) = self.temporal_pattern.captures(temp_match.as_str()) {
                if let (Some(source_match), Some(target_match)) = (caps.get(1), caps.get(2)) {
                    let source_name = source_match.as_str();
                    let target_name = target_match.as_str();
                    
                    relationships.push(ExtractedRelationship {
                        source_entity: source_name.to_string(),
                        target_entity: target_name.to_string(),
                        relation_type: "temporal_relation".to_string(),
                        summary: format!("{} has temporal relation with {} in {}", source_name, target_name, episode_name),
                        confidence: 0.6,
                        context: self.get_context_around_match(content, temp_match.start(), temp_match.end()),
                        weight: 0.6,
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        // Extract location relationships
        for loc_match in self.location_pattern.find_iter(content) {
            if let Some(caps) = self.location_pattern.captures(loc_match.as_str()) {
                if let (Some(source_match), Some(target_match)) = (caps.get(1), caps.get(2)) {
                    let source_name = source_match.as_str();
                    let target_name = target_match.as_str();
                    
                    relationships.push(ExtractedRelationship {
                        source_entity: source_name.to_string(),
                        target_entity: target_name.to_string(),
                        relation_type: "located_at".to_string(),
                        summary: format!("{} located at {} in {}", source_name, target_name, episode_name),
                        confidence: 0.7,
                        context: self.get_context_around_match(content, loc_match.start(), loc_match.end()),
                        weight: 0.7,
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        Ok(relationships)
    }

    /// Extract domain-specific relationships
    fn extract_pattern_relationships(&self, content: &str, episode_name: &str) -> Result<Vec<ExtractedRelationship>> {
        let mut relationships = Vec::new();

        // Browser-technology relationships
        let browser_entities: Vec<_> = ["Chrome", "Firefox", "Safari", "Edge", "Patchright", "Playwright"]
            .iter().filter(|&&browser| content.contains(browser))
            .map(|&browser| ExtractedEntity {
                name: browser.to_string(),
                entity_type: "browser".to_string(),
                summary: format!("Browser: {}", browser),
                confidence: 0.9,
                context: content.to_string(),
                metadata: HashMap::new(),
            }).collect();

        let tech_entities: Vec<_> = ["WebAuthn", "OAuth", "SAML", "JWT", "API", "REST"]
            .iter().filter(|&&tech| content.contains(tech))
            .map(|&tech| ExtractedEntity {
                name: tech.to_string(),
                entity_type: "technology".to_string(),
                summary: format!("Technology: {}", tech),
                confidence: 0.8,
                context: content.to_string(),
                metadata: HashMap::new(),
            }).collect();

        // Create browser-technology support relationships
        for browser in &browser_entities {
            for tech in &tech_entities {
                relationships.push(ExtractedRelationship {
                    source_entity: browser.name.clone(),
                    target_entity: tech.name.clone(),
                    relation_type: "supports".to_string(),
                    summary: format!("{} supports {} technology in {}", browser.name, tech.name, episode_name),
                    confidence: 0.8,
                    context: format!("Browser {} and technology {} mentioned together", browser.name, tech.name),
                    weight: 0.8,
                    metadata: HashMap::new(),
                });
            }
        }

        // Browser-URL relationships
        let url_entities: Vec<_> = content.split_whitespace()
            .filter(|word| word.starts_with("http"))
            .map(|url| ExtractedEntity {
                name: url.to_string(),
                entity_type: "url".to_string(),
                summary: format!("URL: {}", url),
                confidence: 0.9,
                context: content.to_string(),
                metadata: HashMap::new(),
            }).collect();

        for browser in &browser_entities {
            for url in &url_entities {
                relationships.push(ExtractedRelationship {
                    source_entity: browser.name.clone(),
                    target_entity: url.name.clone(),
                    relation_type: "accesses".to_string(),
                    summary: format!("{} accesses {} in {}", browser.name, url.name, episode_name),
                    confidence: 0.9,
                    context: format!("Browser {} accessing URL {}", browser.name, url.name),
                    weight: 0.9,
                    metadata: HashMap::new(),
                });
            }
        }

        Ok(relationships)
    }

    /// Convert extracted relationship to KGEdge
    pub fn relationship_to_edge(
        &self,
        relationship: &ExtractedRelationship,
        source_uuid: Uuid,
        target_uuid: Uuid,
        group_id: Option<String>,
    ) -> KGEdge {
        KGEdge::new(
            source_uuid,
            target_uuid,
            relationship.relation_type.clone(),
            relationship.summary.clone(),
            relationship.weight,
            group_id,
        )
    }

    // Helper methods

    fn calculate_entity_distance(&self, content: &str, entity1: &str, entity2: &str) -> usize {
        if let (Some(pos1), Some(pos2)) = (content.find(entity1), content.find(entity2)) {
            if pos1 < pos2 {
                pos2 - pos1
            } else {
                pos1 - pos2
            }
        } else {
            usize::MAX
        }
    }

    fn get_context_between_entities(&self, content: &str, entity1: &str, entity2: &str) -> String {
        if let (Some(pos1), Some(pos2)) = (content.find(entity1), content.find(entity2)) {
            let start = pos1.min(pos2);
            let end = (pos1.max(pos2) + entity1.len().max(entity2.len())).min(content.len());
            content[start..end].to_string()
        } else {
            String::new()
        }
    }

    fn get_context_around_match(&self, content: &str, start: usize, end: usize) -> String {
        let context_size = 50;
        let context_start = if start > context_size { start - context_size } else { 0 };
        let context_end = if end + context_size < content.len() { end + context_size } else { content.len() };
        
        content[context_start..context_end].to_string()
    }

    fn calculate_semantic_distance(&self, _entity1: &ExtractedEntity, _entity2: &ExtractedEntity) -> f32 {
        let max_distance = 100.0; // characters
        let _uncertainty_penalty = 0.3;
        
        // Simplified distance calculation
        // In a real implementation, you'd use embeddings or other semantic measures
        0.5 // Placeholder
    }

    fn entities_are_semantically_related(&self, entity1: &ExtractedEntity, entity2: &ExtractedEntity) -> bool {
        // Check if entities are in the same category or have overlapping contexts
        entity1.entity_type == entity2.entity_type ||
        entity1.context.contains(&entity2.name) ||
        entity2.context.contains(&entity1.name)
    }

    fn calculate_relationship_confidence(&self, entity1: &ExtractedEntity, entity2: &ExtractedEntity) -> f32 {
        let base_confidence = (entity1.confidence + entity2.confidence) / 2.0;
        
        // Boost confidence if entities are semantically related
        if self.entities_are_semantically_related(entity1, entity2) {
            (base_confidence * 1.2).min(1.0)
        } else {
            base_confidence * 0.8
        }
    }

    fn entities_are_close(&self, content: &str, entity1: &str, entity2: &str) -> bool {
        let max_distance = 100; // characters
        if let (Some(pos1), Some(pos2)) = (content.find(entity1), content.find(entity2)) {
            let distance = if pos1 < pos2 { pos2 - pos1 } else { pos1 - pos2 };
            return distance <= max_distance;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_extract_usage_relationships() {
        let extractor = RelationshipExtractor::new(RelationshipExtractionConfig::default(), None).unwrap();
        
        let entities = vec![
            ExtractedEntity {
                name: "Patchright".to_string(),
                entity_type: "browser_tool".to_string(),
                summary: "Browser automation tool".to_string(),
                confidence: 0.9,
                context: "automation context".to_string(),
                metadata: HashMap::new(),
            },
            ExtractedEntity {
                name: "Chrome".to_string(),
                entity_type: "browser".to_string(),
                summary: "Web browser".to_string(),
                confidence: 0.9,
                context: "browser context".to_string(),
                metadata: HashMap::new(),
            },
        ];

        let content = "Patchright uses Chrome for automation";
        let relationships = extractor.extract_relationships_between_entities(&entities, content, "test").await.unwrap();

        assert!(!relationships.is_empty());
        assert!(relationships.iter().any(|r| r.relation_type == "uses"));
    }

    #[tokio::test]
    async fn test_extract_co_occurrence_relationships() {
        let extractor = RelationshipExtractor::new(RelationshipExtractionConfig::default(), None).unwrap();
        
        let entities = vec![
            ExtractedEntity {
                name: "WebAuthn".to_string(),
                entity_type: "technology".to_string(),
                summary: "Authentication technology".to_string(),
                confidence: 0.8,
                context: "auth context".to_string(),
                metadata: HashMap::new(),
            },
            ExtractedEntity {
                name: "Chrome".to_string(),
                entity_type: "browser".to_string(),
                summary: "Web browser".to_string(),
                confidence: 0.9,
                context: "browser context".to_string(),
                metadata: HashMap::new(),
            },
        ];

        let content = "WebAuthn testing with Chrome browser";
        let relationships = extractor.extract_relationships_between_entities(&entities, content, "test").await.unwrap();

        assert!(!relationships.is_empty());
        assert!(relationships.iter().any(|r| r.relation_type == "co_occurs_with"));
    }
} 
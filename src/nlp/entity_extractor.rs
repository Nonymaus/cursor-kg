use anyhow::{Result, anyhow};
use regex::Regex;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use chrono::Utc;
use tracing::{debug, info};

use crate::graph::{KGNode, EpisodeSource};
use crate::embeddings::LocalEmbeddingEngine;

/// Entity extraction configuration
#[derive(Debug, Clone)]
pub struct EntityExtractionConfig {
    pub min_entity_length: usize,
    pub max_entity_length: usize,
    pub extract_from_json: bool,
    pub extract_technical_terms: bool,
    pub extract_proper_nouns: bool,
    pub extract_quoted_text: bool,
    pub min_confidence: f32,
    pub max_entities_per_text: usize,
    pub enable_fuzzy_matching: bool,
    pub custom_patterns: Vec<EntityPattern>,
}

impl Default for EntityExtractionConfig {
    fn default() -> Self {
        Self {
            min_entity_length: 2,
            max_entity_length: 50,
            extract_from_json: true,
            extract_technical_terms: true,
            extract_proper_nouns: true,
            extract_quoted_text: true,
            min_confidence: 0.6,
            max_entities_per_text: 100,
            enable_fuzzy_matching: true,
            custom_patterns: Vec::new(),
        }
    }
}

/// Pattern for custom entity recognition
#[derive(Debug, Clone)]
pub struct EntityPattern {
    pub name: String,
    pub pattern: String,
    pub entity_type: String,
    pub confidence: f32,
}

/// Extracted entity information
#[derive(Debug, Clone)]
pub struct ExtractedEntity {
    pub name: String,
    pub entity_type: String,
    pub summary: String,
    pub confidence: f32,
    pub context: String,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Entity extractor for knowledge graph population
pub struct EntityExtractor {
    config: EntityExtractionConfig,
    embedding_engine: Option<std::sync::Arc<LocalEmbeddingEngine>>,
    patterns: Vec<CompiledPattern>,
    // Compiled regex patterns for efficiency
    technical_term_regex: Regex,
    proper_noun_regex: Regex,
    quoted_text_regex: Regex,
    url_regex: Regex,
    email_regex: Regex,
    code_block_regex: Regex,
    browser_regex: Regex,
    technology_regex: Regex,
}

#[derive(Clone)]
struct CompiledPattern {
    name: String,
    regex: Regex,
    entity_type: String,
    confidence: f32,
}

impl EntityExtractor {
    pub fn new(config: EntityExtractionConfig, embedding_engine: Option<std::sync::Arc<LocalEmbeddingEngine>>) -> Result<Self> {
        let mut patterns = Vec::new();
        
        // Compile built-in patterns
        patterns.extend(Self::compile_builtin_patterns()?);
        
        // Compile custom patterns
        for custom_pattern in &config.custom_patterns {
            if let Ok(regex) = Regex::new(&custom_pattern.pattern) {
                patterns.push(CompiledPattern {
                    name: custom_pattern.name.clone(),
                    regex,
                    entity_type: custom_pattern.entity_type.clone(),
                    confidence: custom_pattern.confidence,
                });
            }
        }

        Ok(Self {
            config,
            embedding_engine,
            patterns,
            technical_term_regex: Regex::new(r"\b[A-Z][a-zA-Z]*(?:[A-Z][a-zA-Z]*)*\b")?,
            proper_noun_regex: Regex::new(r"\b[A-Z][a-z]+(?:\s+[A-Z][a-z]+)*\b")?,
            quoted_text_regex: Regex::new(r#""([^"]+)"|'([^']+)'|`([^`]+)`"#)?,
            url_regex: Regex::new(r"https?://[^\s]+")?,
            email_regex: Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b")?,
            code_block_regex: Regex::new(r"```[\s\S]*?```|`[^`]+`")?,
            browser_regex: Regex::new(r"\b(?i)(chrome|firefox|safari|edge|chromium|webkit|playwright|puppeteer|selenium|camoufox|patchright|rebrowser)\b")?,
            technology_regex: Regex::new(r"\b(?i)(webauthn|oauth|saml|jwt|api|rest|graphql|json|xml|html|css|javascript|python|rust|docker|kubernetes|aws|gcp|azure)\b")?,
        })
    }

    /// Extract entities from episode content
    pub async fn extract_entities(&self, content: &str, source: &EpisodeSource, episode_name: &str) -> Result<Vec<ExtractedEntity>> {
        let mut entities = Vec::new();

        match source {
            EpisodeSource::Json => {
                entities.extend(self.extract_from_json(content, episode_name)?);
            },
            EpisodeSource::Text | EpisodeSource::Message => {
                entities.extend(self.extract_from_text(content, episode_name)?);
            }
        }

        // Deduplicate entities by name and type
        let mut seen = HashSet::new();
        entities.retain(|entity| {
            let key = (entity.name.clone(), entity.entity_type.clone());
            seen.insert(key)
        });

        // Filter by confidence and length
        entities.retain(|entity| {
            entity.confidence > 0.3 && 
            entity.name.len() >= self.config.min_entity_length &&
            entity.name.len() <= self.config.max_entity_length
        });

        Ok(entities)
    }

    /// Extract entities from JSON content
    fn extract_from_json(&self, content: &str, episode_name: &str) -> Result<Vec<ExtractedEntity>> {
        let mut entities = Vec::new();

        if !self.config.extract_from_json {
            return Ok(entities);
        }

        // Try to parse as JSON
        if let Ok(json_value) = serde_json::from_str::<Value>(content) {
            entities.extend(self.extract_from_json_value(&json_value, episode_name, "")?);
        }

        // Also extract from the raw text content
        entities.extend(self.extract_from_text(content, episode_name)?);

        Ok(entities)
    }

    /// Recursively extract entities from JSON values
    fn extract_from_json_value(&self, value: &Value, episode_name: &str, path: &str) -> Result<Vec<ExtractedEntity>> {
        let mut entities = Vec::new();

        match value {
            Value::Object(obj) => {
                for (key, val) in obj {
                    let new_path = if path.is_empty() { key.clone() } else { format!("{}.{}", path, key) };
                    
                    // Extract key as potential entity
                    if self.is_meaningful_key(key) {
                        entities.push(ExtractedEntity {
                            name: key.clone(),
                            entity_type: "json_key".to_string(),
                            summary: format!("JSON key from {}", episode_name),
                            confidence: 0.6,
                            context: new_path.clone(),
                            metadata: {
                                let mut meta = HashMap::new();
                                meta.insert("json_path".to_string(), serde_json::Value::String(new_path.clone()));
                                meta.insert("value_type".to_string(), serde_json::Value::String(self.get_value_type_name(val)));
                                meta
                            },
                        });
                    }

                    // Recursively extract from value
                    entities.extend(self.extract_from_json_value(val, episode_name, &new_path)?);
                }
            },
            Value::Array(arr) => {
                for (i, val) in arr.iter().enumerate() {
                    let new_path = format!("{}[{}]", path, i);
                    entities.extend(self.extract_from_json_value(val, episode_name, &new_path)?);
                }
            },
            Value::String(s) => {
                // Extract entities from string values
                if s.len() >= self.config.min_entity_length && s.len() <= self.config.max_entity_length {
                    // Check if it's a meaningful string value
                    if self.is_meaningful_string_value(s) {
                        entities.push(ExtractedEntity {
                            name: s.clone(),
                            entity_type: self.classify_string_entity(s),
                            summary: format!("Value from JSON path: {}", path),
                            confidence: 0.7,
                            context: path.to_string(),
                            metadata: {
                                let mut meta = HashMap::new();
                                meta.insert("json_path".to_string(), serde_json::Value::String(path.to_string()));
                                meta.insert("source".to_string(), serde_json::Value::String("json_value".to_string()));
                                meta
                            },
                        });
                    }
                }

                // Also extract entities from the string content itself
                entities.extend(self.extract_from_text(s, episode_name)?);
            },
            _ => {}
        }

        Ok(entities)
    }

    /// Extract entities from text content
    fn extract_from_text(&self, content: &str, episode_name: &str) -> Result<Vec<ExtractedEntity>> {
        let mut entities = Vec::new();

        // Extract URLs
        for url_match in self.url_regex.find_iter(content) {
            entities.push(ExtractedEntity {
                name: url_match.as_str().to_string(),
                entity_type: "url".to_string(),
                summary: format!("URL mentioned in {}", episode_name),
                confidence: 0.9,
                context: self.get_context(content, url_match.start(), url_match.end()),
                metadata: {
                    let mut meta = HashMap::new();
                    meta.insert("domain".to_string(), serde_json::Value::String(self.extract_domain(url_match.as_str())));
                    meta
                },
            });
        }

        // Extract email addresses
        for email_match in self.email_regex.find_iter(content) {
            entities.push(ExtractedEntity {
                name: email_match.as_str().to_string(),
                entity_type: "email".to_string(),
                summary: format!("Email address mentioned in {}", episode_name),
                confidence: 0.9,
                context: self.get_context(content, email_match.start(), email_match.end()),
                metadata: HashMap::new(),
            });
        }

        // Extract browser/automation tools
        for browser_match in self.browser_regex.find_iter(content) {
            entities.push(ExtractedEntity {
                name: browser_match.as_str().to_string(),
                entity_type: "browser_tool".to_string(),
                summary: format!("Browser/automation tool mentioned in {}", episode_name),
                confidence: 0.8,
                context: self.get_context(content, browser_match.start(), browser_match.end()),
                metadata: HashMap::new(),
            });
        }

        // Extract technology terms
        for tech_match in self.technology_regex.find_iter(content) {
            entities.push(ExtractedEntity {
                name: tech_match.as_str().to_string(),
                entity_type: "technology".to_string(),
                summary: format!("Technology mentioned in {}", episode_name),
                confidence: 0.7,
                context: self.get_context(content, tech_match.start(), tech_match.end()),
                metadata: HashMap::new(),
            });
        }

        // Extract quoted text
        if self.config.extract_quoted_text {
            for quote_match in self.quoted_text_regex.find_iter(content) {
                let quoted_text = quote_match.as_str();
                let inner_text = &quoted_text[1..quoted_text.len()-1]; // Remove quotes
                
                if inner_text.len() >= self.config.min_entity_length && inner_text.len() <= self.config.max_entity_length {
                    entities.push(ExtractedEntity {
                        name: inner_text.to_string(),
                        entity_type: "quoted_text".to_string(),
                        summary: format!("Quoted text from {}", episode_name),
                        confidence: 0.6,
                        context: self.get_context(content, quote_match.start(), quote_match.end()),
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        // Extract technical terms (CamelCase, etc.)
        if self.config.extract_technical_terms {
            for tech_match in self.technical_term_regex.find_iter(content) {
                let term = tech_match.as_str();
                if term.len() >= self.config.min_entity_length && self.is_technical_term(term) {
                    entities.push(ExtractedEntity {
                        name: term.to_string(),
                        entity_type: "technical_term".to_string(),
                        summary: format!("Technical term from {}", episode_name),
                        confidence: 0.5,
                        context: self.get_context(content, tech_match.start(), tech_match.end()),
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        // Extract proper nouns
        if self.config.extract_proper_nouns {
            for noun_match in self.proper_noun_regex.find_iter(content) {
                let noun = noun_match.as_str();
                if noun.len() >= self.config.min_entity_length && self.is_meaningful_proper_noun(noun) {
                    entities.push(ExtractedEntity {
                        name: noun.to_string(),
                        entity_type: "proper_noun".to_string(),
                        summary: format!("Proper noun from {}", episode_name),
                        confidence: 0.4,
                        context: self.get_context(content, noun_match.start(), noun_match.end()),
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        Ok(entities)
    }

    /// Convert extracted entity to KGNode
    pub fn entity_to_node(&self, entity: &ExtractedEntity, group_id: Option<String>) -> KGNode {
        let now = Utc::now();
        KGNode {
            uuid: Uuid::new_v4(),
            name: entity.name.clone(),
            node_type: entity.entity_type.clone(),
            summary: entity.summary.clone(),
            metadata: entity.metadata.clone(),
            created_at: now,
            updated_at: now,
            group_id,
        }
    }

    // Helper methods

    fn is_meaningful_key(&self, key: &str) -> bool {
        key.len() >= 2 && 
        !key.chars().all(|c| c.is_numeric()) &&
        !["id", "uuid", "type", "name", "value", "data", "info"].contains(&key.to_lowercase().as_str())
    }

    fn is_meaningful_string_value(&self, value: &str) -> bool {
        value.len() >= self.config.min_entity_length &&
        !value.chars().all(|c| c.is_numeric()) &&
        !value.chars().all(|c| c.is_whitespace()) &&
        value.split_whitespace().count() <= 5 // Not too long
    }

    fn classify_string_entity(&self, value: &str) -> String {
        if self.url_regex.is_match(value) {
            "url".to_string()
        } else if self.email_regex.is_match(value) {
            "email".to_string()
        } else if self.browser_regex.is_match(value) {
            "browser_tool".to_string()
        } else if self.technology_regex.is_match(value) {
            "technology".to_string()
        } else if value.chars().any(|c| c.is_uppercase()) {
            "identifier".to_string()
        } else {
            "string_value".to_string()
        }
    }

    fn get_value_type_name(&self, value: &Value) -> String {
        match value {
            Value::Null => "null".to_string(),
            Value::Bool(_) => "boolean".to_string(),
            Value::Number(_) => "number".to_string(),
            Value::String(_) => "string".to_string(),
            Value::Array(_) => "array".to_string(),
            Value::Object(_) => "object".to_string(),
        }
    }

    fn get_context(&self, content: &str, start: usize, end: usize) -> String {
        let context_size = 50;
        let start_context = if start > context_size { start - context_size } else { 0 };
        let end_context = if end + context_size < content.len() { end + context_size } else { content.len() };
        
        content[start_context..end_context].to_string()
    }

    fn extract_domain(&self, url: &str) -> String {
        if let Some(domain_start) = url.find("://") {
            let after_protocol = &url[domain_start + 3..];
            if let Some(domain_end) = after_protocol.find('/') {
                after_protocol[..domain_end].to_string()
            } else {
                after_protocol.to_string()
            }
        } else {
            url.to_string()
        }
    }

    fn is_technical_term(&self, term: &str) -> bool {
        // Check if it's a meaningful technical term (not just random capitalized words)
        term.len() >= 3 &&
        term.chars().any(|c| c.is_uppercase()) &&
        !["The", "This", "That", "And", "Or", "But", "For", "With", "From", "To", "In", "On", "At", "By"].contains(&term)
    }

    fn is_meaningful_proper_noun(&self, noun: &str) -> bool {
        // Filter out common words that might be capitalized
        let common_words = ["The", "This", "That", "And", "Or", "But", "For", "With", "From", "To", "In", "On", "At", "By", "A", "An"];
        !common_words.contains(&noun) && noun.len() >= 3
    }

    fn compile_builtin_patterns() -> Result<Vec<CompiledPattern>> {
        let mut patterns = Vec::new();

        // Email addresses
        patterns.push(CompiledPattern {
            name: "email".to_string(),
            regex: Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b")?,
            entity_type: "email".to_string(),
            confidence: 0.9,
        });

        // URLs
        patterns.push(CompiledPattern {
            name: "url".to_string(),
            regex: Regex::new(r"https?://[^\s]+")?,
            entity_type: "url".to_string(),
            confidence: 0.9,
        });

        // Function names (common patterns)
        patterns.push(CompiledPattern {
            name: "function".to_string(),
            regex: Regex::new(r"\b(fn|function|def)\s+([a-zA-Z_][a-zA-Z0-9_]*)")?,
            entity_type: "function".to_string(),
            confidence: 0.8,
        });

        // Class names
        patterns.push(CompiledPattern {
            name: "class".to_string(),
            regex: Regex::new(r"\b(class|struct|interface)\s+([A-Z][a-zA-Z0-9_]*)")?,
            entity_type: "class".to_string(),
            confidence: 0.8,
        });

        // Variable names (simplified)
        patterns.push(CompiledPattern {
            name: "variable".to_string(),
            regex: Regex::new(r"\b(let|var|const)\s+([a-zA-Z_][a-zA-Z0-9_]*)")?,
            entity_type: "variable".to_string(),
            confidence: 0.7,
        });

        Ok(patterns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_extract_from_json() {
        let extractor = EntityExtractor::new(EntityExtractionConfig::default(), None).unwrap();
        let json_content = r#"{"browser": "Patchright", "url": "https://example.com", "user_agent": "Chrome/120.0.0.0"}"#;
        
        let entities = extractor.extract_entities(json_content, &EpisodeSource::Json, "test").await.unwrap();
        
        assert!(!entities.is_empty());
        assert!(entities.iter().any(|e| e.name == "Patchright"));
        assert!(entities.iter().any(|e| e.name == "https://example.com"));
    }

    #[tokio::test]
    async fn test_extract_from_text() {
        let extractor = EntityExtractor::new(EntityExtractionConfig::default(), None).unwrap();
        let text_content = "Using Patchright with Chrome for WebAuthn testing at https://example.com";
        
        let entities = extractor.extract_entities(text_content, &EpisodeSource::Text, "test").await.unwrap();
        
        assert!(!entities.is_empty());
        assert!(entities.iter().any(|e| e.name == "Patchright"));
        assert!(entities.iter().any(|e| e.name == "Chrome"));
        assert!(entities.iter().any(|e| e.name == "WebAuthn"));
        assert!(entities.iter().any(|e| e.name == "https://example.com"));
    }
} 
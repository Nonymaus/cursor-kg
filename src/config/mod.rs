pub mod defaults;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use crate::security::AuthConfig;

// use crate::Cli; // Commented out as Cli is not defined in main.rs yet

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub data_dir: PathBuf,
    pub models_dir: PathBuf,
    pub port: u16,
    pub database: DatabaseConfig,
    pub embeddings: EmbeddingConfig,
    pub search: SearchConfig,
    pub memory: MemoryConfig,
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub enable_authentication: bool,
    pub api_key: Option<String>,
    pub admin_operations_require_auth: bool,
    pub rate_limit_requests_per_minute: u32,
    pub rate_limit_burst: u32,
    pub max_content_length: usize,
    pub max_query_length: usize,
    pub max_path_length: usize,
    pub max_array_size: usize,
    pub enable_encryption: bool,
    pub encryption_key_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub filename: String,
    pub connection_pool_size: u32,
    pub enable_wal: bool,
    pub cache_size_kb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    pub model_name: String,
    pub model_size: String, // "small", "medium", "large"
    pub dimensions: usize,
    pub batch_size: usize,
    pub cache_size: usize,
    pub onnx_threads: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    pub max_results: usize,
    pub similarity_threshold: f32,
    pub enable_hybrid_search: bool,
    pub text_search_weight: f32,
    pub vector_search_weight: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub max_episodes_per_group: usize,
    pub episode_retention_days: u32,
    pub auto_cleanup_enabled: bool,
    pub compression_enabled: bool,
}

impl ServerConfig {
    pub fn get_default_log_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".kg-mcp-server.log")
    }

    pub fn load(config_path: Option<&Path>) -> Result<Self> {
        let mut config = if let Some(path) = config_path {
            // Load from file
            let config_str = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read config file: {}", path.display()))?;
            
            toml::from_str(&config_str)
                .with_context(|| format!("Failed to parse config file: {}", path.display()))?
        } else {
            // Use defaults
            Self::default()
        };

        // Configuration loaded from file or defaults
        // CLI overrides would be applied here if Cli was available

        // Ensure directories exist
        std::fs::create_dir_all(&config.data_dir)
            .with_context(|| format!("Failed to create data directory: {}", config.data_dir.display()))?;
        
        std::fs::create_dir_all(&config.models_dir)
            .with_context(|| format!("Failed to create models directory: {}", config.models_dir.display()))?;

        Ok(config)
    }

    pub fn database_path(&self) -> PathBuf {
        self.data_dir.join(&self.database.filename)
    }

    /// Validate configuration settings
    pub fn validate(&self) -> Result<()> {
        // Validate security settings
        if self.security.enable_authentication && self.security.api_key.is_none() {
            return Err(anyhow::anyhow!("API key must be set when authentication is enabled"));
        }

        if self.security.rate_limit_requests_per_minute == 0 {
            return Err(anyhow::anyhow!("Rate limit requests per minute must be greater than 0"));
        }

        if self.security.max_content_length == 0 {
            return Err(anyhow::anyhow!("Max content length must be greater than 0"));
        }

        // Validate database settings
        if self.database.connection_pool_size == 0 {
            return Err(anyhow::anyhow!("Database connection pool size must be greater than 0"));
        }

        // Validate embedding settings
        if self.embeddings.dimensions == 0 {
            return Err(anyhow::anyhow!("Embedding dimensions must be greater than 0"));
        }

        if self.embeddings.batch_size == 0 {
            return Err(anyhow::anyhow!("Embedding batch size must be greater than 0"));
        }

        // Validate search settings
        if self.search.similarity_threshold < 0.0 || self.search.similarity_threshold > 1.0 {
            return Err(anyhow::anyhow!("Similarity threshold must be between 0.0 and 1.0"));
        }

        Ok(())
    }

    /// Convert security config to AuthConfig
    pub fn to_auth_config(&self) -> AuthConfig {
        AuthConfig {
            enabled: self.security.enable_authentication,
            api_key: self.security.api_key.clone(),
            rate_limit_requests_per_minute: self.security.rate_limit_requests_per_minute,
            rate_limit_burst: self.security.rate_limit_burst,
            admin_operations_require_auth: self.security.admin_operations_require_auth,
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("kg-mcp-server");
        
        let models_dir = data_dir.join("models");

        Self {
            data_dir,
            models_dir,
            port: 3000,
            database: DatabaseConfig::default(),
            embeddings: EmbeddingConfig::default(),
            search: SearchConfig::default(),
            memory: MemoryConfig::default(),
            security: SecurityConfig::default(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            filename: "knowledge_graph.db".to_string(),
            connection_pool_size: 16,
            enable_wal: true,
            cache_size_kb: 64 * 1024, // 64MB
        }
    }
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model_name: "nomic-embed-text-v1.5".to_string(),
            model_size: "medium".to_string(),
            dimensions: 768,
            batch_size: 32,
            cache_size: 1000,
            onnx_threads: None, // Use system default
        }
    }
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            max_results: 10,
            similarity_threshold: 0.7,
            enable_hybrid_search: true,
            text_search_weight: 0.3,
            vector_search_weight: 0.7,
        }
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_episodes_per_group: 10000,
            episode_retention_days: 365,
            auto_cleanup_enabled: true,
            compression_enabled: false,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_authentication: false,
            api_key: None,
            admin_operations_require_auth: true,
            rate_limit_requests_per_minute: 60,
            rate_limit_burst: 10,
            max_content_length: 1_048_576, // 1MB
            max_query_length: 1024,
            max_path_length: 4096,
            max_array_size: 1000,
            enable_encryption: false,
            encryption_key_file: None,
        }
    }
}
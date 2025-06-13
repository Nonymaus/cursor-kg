use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tracing::debug;
use std::fs;
use reqwest;
use sha2::{Sha256, Digest};
use tokio::fs as async_fs;
use crate::config::defaults::*;
use std::collections::HashMap;

/// Download and manage embedding models
#[derive(Clone)]
pub struct ModelManager {
    models_dir: PathBuf,
    client: reqwest::Client,
}

impl ModelManager {
    pub fn new(models_dir: PathBuf) -> Self {
        // Ensure models directory exists
        if !models_dir.exists() {
            fs::create_dir_all(&models_dir).expect("Failed to create models directory");
        }
        
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300)) // 5 minute timeout
            .build()
            .expect("Failed to create HTTP client");
            
        Self {
            models_dir,
            client,
        }
    }

    pub async fn download_model(&self, model_name: &str) -> Result<()> {
        debug!("🔄 Downloading model: {}", model_name);
        
        let (model_url, tokenizer_url, config_url) = get_model_urls(model_name)
            .ok_or_else(|| anyhow::anyhow!("Unsupported model: {}", model_name))?;

        let model_dir = self.models_dir.join(model_name);
        async_fs::create_dir_all(&model_dir).await
            .context("Failed to create model directory")?;

        // Download model files
        self.download_file(&model_url, &model_dir.join(MODEL_FILENAME)).await
            .context("Failed to download model file")?;
            
        self.download_file(&tokenizer_url, &model_dir.join(TOKENIZER_FILENAME)).await
            .context("Failed to download tokenizer file")?;
            
        self.download_file(&config_url, &model_dir.join(CONFIG_FILENAME)).await
            .context("Failed to download config file")?;

        // Validate model files
        self.validate_model(model_name).await
            .context("Model validation failed after download")?;

        debug!("✅ Model downloaded successfully: {}", model_name);
        Ok(())
    }

    pub fn model_exists(&self, model_name: &str) -> bool {
        let model_dir = self.models_dir.join(model_name);
        model_dir.join(MODEL_FILENAME).exists() &&
        model_dir.join(TOKENIZER_FILENAME).exists() &&
        model_dir.join(CONFIG_FILENAME).exists()
    }

    pub async fn ensure_model_available(&self, model_name: &str) -> Result<PathBuf> {
        if !self.model_exists(model_name) {
            debug!("📥 Model not found locally, downloading: {}", model_name);
            self.download_model(model_name).await?;
        }
        
        Ok(self.models_dir.join(model_name))
    }

    pub fn get_model_path(&self, model_name: &str) -> PathBuf {
        self.models_dir.join(model_name).join(MODEL_FILENAME)
    }

    pub fn get_tokenizer_path(&self, model_name: &str) -> PathBuf {
        self.models_dir.join(model_name).join(TOKENIZER_FILENAME)
    }

    pub fn get_config_path(&self, model_name: &str) -> PathBuf {
        self.models_dir.join(model_name).join(CONFIG_FILENAME)
    }

    async fn download_file(&self, url: &str, path: &Path) -> Result<()> {
                    debug!("  ⬇️  Downloading: {}", url);
        
        let response = self.client.get(url).send().await
            .context("Failed to start download")?;
            
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Download failed with status: {}", response.status()));
        }

        let content = response.bytes().await
            .context("Failed to download file content")?;

        async_fs::write(path, content).await
            .context("Failed to write downloaded file")?;

        debug!("  ✅ Downloaded: {}", path.display());
        Ok(())
    }

    async fn validate_model(&self, model_name: &str) -> Result<()> {
        let model_path = self.get_model_path(model_name);
        let tokenizer_path = self.get_tokenizer_path(model_name);
        let config_path = self.get_config_path(model_name);

        // Check file existence
        if !model_path.exists() {
            return Err(anyhow::anyhow!("Model file missing: {}", model_path.display()));
        }
        if !tokenizer_path.exists() {
            return Err(anyhow::anyhow!("Tokenizer file missing: {}", tokenizer_path.display()));
        }
        if !config_path.exists() {
            return Err(anyhow::anyhow!("Config file missing: {}", config_path.display()));
        }

        // Check file sizes (basic validation)
        let model_size = async_fs::metadata(&model_path).await?.len();
        let tokenizer_size = async_fs::metadata(&tokenizer_path).await?.len();

        if model_size < 1_000_000 { // Less than 1MB is suspicious for a model
            return Err(anyhow::anyhow!("Model file too small: {} bytes", model_size));
        }
        if tokenizer_size < 1000 { // Less than 1KB is suspicious for a tokenizer
            return Err(anyhow::anyhow!("Tokenizer file too small: {} bytes", tokenizer_size));
        }

                    debug!("  ✅ Model validation passed");
        Ok(())
    }

    pub async fn calculate_checksum(&self, path: &Path) -> Result<String> {
        let content = async_fs::read(path).await?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        Ok(format!("{:x}", hasher.finalize()))
    }

    pub fn list_downloaded_models(&self) -> Result<Vec<String>> {
        let mut models = Vec::new();
        
        if self.models_dir.exists() {
            for entry in fs::read_dir(&self.models_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        if self.model_exists(name) {
                            models.push(name.to_string());
                        }
                    }
                }
            }
        }
        
        Ok(models)
    }

    pub async fn cleanup_incomplete_downloads(&self) -> Result<()> {
        if !self.models_dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(&self.models_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    if !self.model_exists(name) {
                        debug!("🧹 Cleaning up incomplete download: {}", name);
                        async_fs::remove_dir_all(entry.path()).await?;
                    }
                }
            }
        }

        Ok(())
    }
} 
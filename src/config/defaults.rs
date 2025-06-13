/// Default configuration constants for the KG MCP Server

// Embedding model configurations
pub const DEFAULT_NOMIC_MODEL_URL: &str = "https://huggingface.co/nomic-ai/nomic-embed-text-v1.5/resolve/main/onnx/model.onnx";
pub const DEFAULT_NOMIC_TOKENIZER_URL: &str = "https://huggingface.co/nomic-ai/nomic-embed-text-v1.5/resolve/main/tokenizer.json";
pub const DEFAULT_NOMIC_CONFIG_URL: &str = "https://huggingface.co/nomic-ai/nomic-embed-text-v1.5/resolve/main/config.json";

pub const DEFAULT_MINILM_MODEL_URL: &str = "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/onnx/model.onnx";
pub const DEFAULT_MINILM_TOKENIZER_URL: &str = "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer.json";
pub const DEFAULT_MINILM_CONFIG_URL: &str = "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/config.json";

pub const DEFAULT_BGE_MODEL_URL: &str = "https://huggingface.co/BAAI/bge-m3/resolve/main/onnx/model.onnx";
pub const DEFAULT_BGE_TOKENIZER_URL: &str = "https://huggingface.co/BAAI/bge-m3/resolve/main/tokenizer.json";
pub const DEFAULT_BGE_CONFIG_URL: &str = "https://huggingface.co/BAAI/bge-m3/resolve/main/config.json";

// Model specifications
pub const NOMIC_DIMENSIONS: usize = 768;
pub const MINILM_DIMENSIONS: usize = 384;
pub const BGE_DIMENSIONS: usize = 1024;

// Performance defaults
pub const DEFAULT_BATCH_SIZE: usize = 32;
pub const DEFAULT_MAX_SEQUENCE_LENGTH: usize = 512;
pub const DEFAULT_EMBEDDING_CACHE_SIZE: usize = 1000;

// Search defaults
pub const DEFAULT_MAX_SEARCH_RESULTS: usize = 10;
pub const DEFAULT_SIMILARITY_THRESHOLD: f32 = 0.7;
pub const DEFAULT_TEXT_SEARCH_WEIGHT: f32 = 0.3;
pub const DEFAULT_VECTOR_SEARCH_WEIGHT: f32 = 0.7;

// Database defaults
pub const DEFAULT_DB_FILENAME: &str = "knowledge_graph.db";
pub const DEFAULT_CONNECTION_POOL_SIZE: u32 = 16;
pub const DEFAULT_CACHE_SIZE_KB: u32 = 64 * 1024; // 64MB

// Memory management defaults
pub const DEFAULT_MAX_EPISODES_PER_GROUP: usize = 10000;
pub const DEFAULT_EPISODE_RETENTION_DAYS: u32 = 365;

// Model file names
pub const MODEL_FILENAME: &str = "model.onnx";
pub const TOKENIZER_FILENAME: &str = "tokenizer.json";
pub const CONFIG_FILENAME: &str = "config.json";

// Supported model types
pub const SUPPORTED_MODELS: &[&str] = &[
    "nomic-embed-text-v1.5",
    "all-MiniLM-L6-v2", 
    "bge-m3"
];

/// Get model URLs for a given model name
pub fn get_model_urls(model_name: &str) -> Option<(String, String, String)> {
    match model_name {
        "nomic-embed-text-v1.5" => Some((
            DEFAULT_NOMIC_MODEL_URL.to_string(),
            DEFAULT_NOMIC_TOKENIZER_URL.to_string(),
            DEFAULT_NOMIC_CONFIG_URL.to_string(),
        )),
        "all-MiniLM-L6-v2" => Some((
            DEFAULT_MINILM_MODEL_URL.to_string(),
            DEFAULT_MINILM_TOKENIZER_URL.to_string(),
            DEFAULT_MINILM_CONFIG_URL.to_string(),
        )),
        "bge-m3" => Some((
            DEFAULT_BGE_MODEL_URL.to_string(),
            DEFAULT_BGE_TOKENIZER_URL.to_string(),
            DEFAULT_BGE_CONFIG_URL.to_string(),
        )),
        _ => None,
    }
}

/// Get embedding dimensions for a model
pub fn get_model_dimensions(model_name: &str) -> Option<usize> {
    match model_name {
        "nomic-embed-text-v1.5" => Some(NOMIC_DIMENSIONS),
        "all-MiniLM-L6-v2" => Some(MINILM_DIMENSIONS),
        "bge-m3" => Some(BGE_DIMENSIONS),
        _ => None,
    }
} 
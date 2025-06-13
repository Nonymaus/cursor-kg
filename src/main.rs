use anyhow::Result;
use std::env;
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber;

// Import all modules
use kg_mcp_server::{
    ServerConfig,
    GraphStorage,
    LocalEmbeddingEngine,
    HybridSearchEngine,
    MemoryOptimizer,
    McpServer,
    migration::{graphiti_migrator::GraphitiMigrator, MigrationConfig, SourceType, Migrator},
    search::{TextSearchEngine, VectorSearchEngine},
    memory::MemoryConfig as ServerMemoryConfig,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging - redirect to stderr for stdio mode
    let transport_mode = env::var("MCP_TRANSPORT").unwrap_or_else(|_| "stdio".to_string());
    
    if transport_mode == "stdio" {
        // For stdio mode, send ALL logs to stderr to avoid interfering with JSON-RPC
        tracing_subscriber::fmt()
            .with_max_level(Level::ERROR) // Only errors in stdio mode
            .with_target(false)
            .with_writer(std::io::stderr)
            .init();
    } else {
        // For HTTP/SSE mode, normal logging to stdout
        tracing_subscriber::fmt()
            .with_max_level(Level::INFO)
            .with_target(false)
            .init();
    }

    // Only show startup logs in non-stdio modes
    if transport_mode != "stdio" {
        info!("🚀 Starting Knowledge Graph MCP Server...");
    }

    // Load configuration
    let config = ServerConfig::load(None)?;
    if transport_mode != "stdio" {
        info!("✅ Configuration loaded");
    }

    // Initialize database
    let storage = GraphStorage::new(&config.database_path(), &config.database)?;
    if transport_mode != "stdio" {
        info!("✅ Database initialized");
    }

    // Initialize embedding engine
    let mut embedding_engine = LocalEmbeddingEngine::new(config.clone())?;
    
    // Initialize with the configured model
    let model_name = &config.embeddings.model_name;
    if transport_mode != "stdio" {
        info!("🔄 Initializing embedding engine with model: {}", model_name);
    }
    embedding_engine.initialize(model_name).await?;
    
    let embedding_engine_arc = Arc::new(embedding_engine);
    if transport_mode != "stdio" {
        info!("✅ Embedding engine initialized");
    }

    // Initialize search engine
    let text_engine = TextSearchEngine::new(storage.clone());
    let vector_engine = VectorSearchEngine::new();
    let search_engine = HybridSearchEngine::new(text_engine, vector_engine);
    let search_engine_arc = Arc::new(search_engine);
    if transport_mode != "stdio" {
        info!("✅ Search engine initialized");
    }

    // Initialize memory optimizer
    let memory_config = ServerMemoryConfig {
        max_cache_size: 128 * 1024 * 1024, // Reduced from 256MB to 128MB
        cache_ttl: std::time::Duration::from_secs(3600),
        memory_pool_size: 500, // Reduced from 1000
        gc_interval: std::time::Duration::from_secs(300),
        gc_threshold: 0.7, // Reduced from 0.8 to trigger GC earlier
        preload_enabled: false, // Disabled to reduce startup memory pressure
        compression_enabled: false,
        memory_mapping_enabled: false,
    };
    let memory_optimizer = MemoryOptimizer::new(memory_config.clone());
    memory_optimizer.initialize().await?;
    let memory_optimizer_arc = Arc::new(memory_optimizer);
    if transport_mode != "stdio" {
        info!("✅ Memory optimizer initialized");
    }

    // Check if migration is requested
    if let Ok(migration_source) = env::var("MIGRATION_SOURCE") {
        info!("🔄 Migration source detected: {}", migration_source);
        
        // Create migration config
        let migration_config = MigrationConfig {
            source_type: SourceType::GraphitiMcp,
            source_connection: migration_source,
            target_database: config.database_path().to_string_lossy().to_string(),
            batch_size: env::var("MIGRATION_BATCH_SIZE")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .unwrap_or(100),
            parallel_workers: env::var("MIGRATION_WORKERS")
                .unwrap_or_else(|_| "4".to_string())
                .parse()
                .unwrap_or(4),
            validation_enabled: true,
            backup_enabled: true,
            chunk_size: 100,
        };

        // Create migrator
        let migrator = GraphitiMigrator::new(
            storage.clone(), 
            Some(embedding_engine_arc.as_ref().clone())
        );

        // Analyze migration
        info!("📊 Analyzing migration plan...");
        let plan = migrator.analyze_source(&migration_config).await?;
        info!("Migration plan: {} nodes, {} edges, {} episodes",
              plan.node_count, plan.edge_count, plan.episode_count);
        info!("Estimated duration: {:?}", plan.estimated_duration);
        
        // Progress callback
        let progress_callback = Box::new(|progress: kg_mcp_server::migration::MigrationProgress| {
            info!("Migration progress: {:?} - {}/{} items processed",
                  progress.phase, 
                  progress.nodes_processed + progress.edges_processed + progress.episodes_processed,
                  progress.total_items);
        });

        // Execute migration
        info!("🚀 Starting migration...");
        let result = migrator.migrate(&migration_config, Some(progress_callback)).await?;
        
        if result.success {
            info!("✅ Migration completed successfully!");
            info!("Migrated: {} nodes, {} edges, {} episodes", 
                  result.stats.migrated_nodes, 
                  result.stats.migrated_edges, 
                  result.stats.migrated_episodes);
        } else {
            info!("⚠️ Migration completed with {} errors", result.stats.errors.len());
            for error in &result.stats.errors {
                info!("Error: {} - {}", error.error_type, error.message);
            }
        }

        // Validate if requested
        if migration_config.validation_enabled {
            info!("🔍 Validating migrated data...");
            let validation_report = migrator.validate(&migration_config).await?;
            info!("Validation scores - Integrity: {:.2}, Completeness: {:.2}, Consistency: {:.2}",
                  validation_report.data_integrity_score,
                  validation_report.completeness_score,
                  validation_report.consistency_score);
        }
    }

    // Set default transport mode if not specified
    if env::var("MCP_TRANSPORT").is_err() {
        // Default to stdio for MCP protocol compliance
        std::env::set_var("MCP_TRANSPORT", "stdio");
    }
    
    // Set default port for SSE mode
    if env::var("MCP_PORT").is_err() {
        std::env::set_var("MCP_PORT", "8360");
    }

    // Create MCP server with all components
    let storage_arc = Arc::new(storage);
    let server = McpServer::new(
        config.clone(),
        storage_arc.clone(),
        embedding_engine_arc.clone(),
        search_engine_arc.clone(),
        memory_optimizer_arc.clone(),
    );

    // Only show detailed info in non-stdio modes
    if transport_mode != "stdio" {
        info!("🎯 Knowledge Graph MCP Server ready!");
        info!("Database: {}", config.database_path().display());
        info!("Model: {}", config.embeddings.model_name);
        info!("Cache size: {} MB", memory_config.max_cache_size / 1024 / 1024);
        
        if transport_mode == "sse" || transport_mode == "http" {
            info!("🌐 SSE endpoint: http://0.0.0.0:8360/sse");
        }

        // Display some stats
        let node_count = storage_arc.count_nodes().await.unwrap_or(0);
        let edge_count = storage_arc.count_edges().await.unwrap_or(0);
        let episode_count = storage_arc.count_episodes().await.unwrap_or(0);
        
        info!("📈 Current graph stats:");
        info!("  Nodes: {}", node_count);
        info!("  Edges: {}", edge_count);
        info!("  Episodes: {}", episode_count);
    }

    // Start the MCP server
    server.run().await?;

    Ok(())
} 
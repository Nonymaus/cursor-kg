use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::{error, info, Level};
use tracing_subscriber;
use tray_item::{IconSource, TrayItem};
use std::process::Command;

use kg_mcp_server::{
    ServerConfig, GraphStorage, LocalEmbeddingEngine, HybridSearchEngine, McpServer,
    search::{TextSearchEngine, VectorSearchEngine},
    memory::{MemoryConfig, MemoryOptimizer},
};

enum ServerState {
    Starting,
    Running,
    Error(String),
}

fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).with_target(false).init();
    info!("🚀 Initializing Knowledge Graph MCP Menu Bar App...");

    let server_status = Arc::new(Mutex::new(ServerState::Starting));
    let server_status_clone = server_status.clone();

    // --- Run the server in a background thread ---
    thread::spawn(move || {
        info!("Server thread spawned.");
        if let Err(e) = run_server(server_status_clone) {
            error!("Server thread failed: {}", e);
        }
    });

    // Before creating the tray item, clone the server status Arc
    let status_arc = server_status.clone();

    // --- Create the tray item ---
    let mut tray = TrayItem::new("KG MCP Server", IconSource::Resource("AppIcon"))?;
    info!("✅ Tray icon object created.");
    
    // Build the dashboard menu
    let mut inner = tray.inner_mut();
    // Server name label
    inner.add_label("KG MCP Server")?;
    // Status label
    let status_label = match &*status_arc.lock().unwrap() {
        ServerState::Starting => "Status: Starting...",
        ServerState::Running => "Status: Running",
        ServerState::Error(msg) => &format!("Status: Error: {}", msg),
    };
    inner.add_label(status_label)?;
    // Restart action
    let server_status_for_restart = status_arc.clone();
    inner.add_menu_item("Restart Server", move || {
        info!("Restart requested by user.");
        // For now, simply exit; restarting requires more complex thread management
        std::process::exit(0);
    })?;
    // View logs action
    inner.add_menu_item("View Logs", || {
        let log_path = ServerConfig::get_default_log_path();
        if let Err(e) = Command::new("open").arg(log_path).status() {
            error!("Failed to open log file: {}", e);
        }
    })?;
    // Separator
    inner.add_menu_item("---", || {})?;
    // Quit action
    inner.add_quit_item("Quit");
    // Display the menu bar icon and block
    inner.display();

    Ok(())
}

fn run_server(server_status: Arc<Mutex<ServerState>>) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        {
            let mut status = server_status.lock().unwrap();
            *status = ServerState::Starting;
        }

        let config = ServerConfig::load(None)?;
        let storage = Arc::new(GraphStorage::new(&config.database_path(), &config.database)?);
        let embedding_engine = Arc::new(LocalEmbeddingEngine::new(config.clone())?);
        let text_engine = TextSearchEngine::new(storage.clone());
        let vector_engine = VectorSearchEngine::new();
        let search_engine = Arc::new(HybridSearchEngine::new(text_engine, vector_engine));
        let memory_config = MemoryConfig::default();
        let memory_optimizer = Arc::new(MemoryOptimizer::new(memory_config));

        std::env::set_var("MCP_TRANSPORT", "sse");
        std::env::set_var("MCP_PORT", "8360");

        let server = McpServer::new(
            config.clone(),
            storage.clone(),
            embedding_engine.clone(),
            search_engine.clone(),
            memory_optimizer.clone(),
        );

        let server_task = tokio::spawn(async move {
            info!("🎯 Knowledge Graph MCP Server is live!");
            {
                let mut status = server_status.lock().unwrap();
                *status = ServerState::Running;
            }
            server.run().await
        });

        tokio::spawn(async move {
            info!("🔄 Initializing embedding engine in the background...");
            if let Err(e) = embedding_engine.initialize(&config.embeddings.model_name).await {
                error!("Failed to initialize embedding engine: {}", e);
            } else {
                info!("✅ Embedding engine initialized successfully.");
            }

            info!("🔄 Initializing memory optimizer in the background...");
            if let Err(e) = memory_optimizer.initialize().await {
                error!("Failed to initialize memory optimizer: {}", e);
            } else {
                info!("✅ Memory optimizer initialized successfully.");
            }
        });

        server_task.await?
    })
} 
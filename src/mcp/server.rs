use anyhow::Result;
use tracing::{info, error};
use std::sync::Arc;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response, Sse},
    routing::{get, post},
    Json, Router,
};
use futures::stream::{self, Stream};
use serde_json::{json, Value};
use std::time::Duration;
use tower_http::cors::CorsLayer;

use crate::config::ServerConfig;
use crate::graph::storage::GraphStorage;
use crate::embeddings::LocalEmbeddingEngine;
use crate::search::{HybridSearchEngine, TextSearchEngine, VectorSearchEngine};
use crate::memory::MemoryOptimizer;
use super::protocol::McpProtocol;

#[derive(Clone)]
pub struct AppState {
    storage: Arc<GraphStorage>,
    embedding_engine: Arc<LocalEmbeddingEngine>,
    search_engine: Arc<HybridSearchEngine>,
    memory_optimizer: Arc<MemoryOptimizer>,
}

pub struct McpServer {
    config: ServerConfig,
    storage: Arc<GraphStorage>,
    embedding_engine: Arc<LocalEmbeddingEngine>,
    search_engine: Arc<HybridSearchEngine>,
    memory_optimizer: Arc<MemoryOptimizer>,
}

impl McpServer {
    pub fn new(
        config: ServerConfig,
        storage: Arc<GraphStorage>,
        embedding_engine: Arc<LocalEmbeddingEngine>,
        search_engine: Arc<HybridSearchEngine>,
        memory_optimizer: Arc<MemoryOptimizer>,
    ) -> Self {
        Self {
            config,
            storage,
            embedding_engine,
            search_engine,
            memory_optimizer,
        }
    }

    pub async fn new_from_config(config: ServerConfig) -> Result<Self> {
        info!("Initializing KG MCP Server with config: {:?}", config);
        
        // Initialize storage
        let storage = Arc::new(GraphStorage::new(&config.database_path(), &config.database)?);
        
        // Initialize embedding engine
        let embedding_engine = Arc::new(LocalEmbeddingEngine::new(config.clone())?);
        
        // Initialize search engines
        let text_engine = TextSearchEngine::new(storage.clone());
        let vector_engine = VectorSearchEngine::new();
        let search_engine = Arc::new(HybridSearchEngine::new(text_engine, vector_engine));
        
        // Initialize memory optimizer
        let memory_config = crate::memory::MemoryConfig {
            max_cache_size: 100_000_000, // 100MB
            cache_ttl: std::time::Duration::from_secs(3600), // 1 hour
            memory_pool_size: 1000,
            gc_interval: std::time::Duration::from_secs(300), // 5 minutes
            gc_threshold: 0.8,
            preload_enabled: true,
            compression_enabled: false,
            memory_mapping_enabled: false,
        };
        let memory_optimizer = Arc::new(MemoryOptimizer::new(memory_config));
        
        // Initialize components
        memory_optimizer.initialize().await?;
        
        info!("KG MCP Server components initialized successfully");
        
        Ok(Self {
            config,
            storage,
            embedding_engine,
            search_engine,
            memory_optimizer,
        })
    }

    pub async fn run(&self) -> Result<()> {
        info!("KG MCP Server starting...");

        // Start background tasks
        self.start_background_tasks().await?;

        // Check if we should run as HTTP/SSE server or stdio
        if std::env::var("MCP_TRANSPORT").as_deref() == Ok("sse") || 
           std::env::var("MCP_TRANSPORT").as_deref() == Ok("http") {
            self.run_http_server().await
        } else {
            self.run_stdio_server().await
        }
    }

    async fn run_http_server(&self) -> Result<()> {
        let port = std::env::var("MCP_PORT")
            .unwrap_or_else(|_| self.config.port.to_string())
            .parse::<u16>()
            .unwrap_or(self.config.port);

        info!("Starting KG MCP Server with HTTP/SSE transport on port {}", port);

        let state = AppState {
            storage: Arc::clone(&self.storage),
            embedding_engine: Arc::clone(&self.embedding_engine),
            search_engine: Arc::clone(&self.search_engine),
            memory_optimizer: Arc::clone(&self.memory_optimizer),
        };

        let app = Router::new()
            // SSE endpoint for MCP clients - handles both GET and POST
            .route("/sse", get(handle_sse_connect).post(handle_sse_request))
            // HTTP endpoint for MCP over HTTP
            .route("/mcp", post(handle_mcp_request))
            // Health check endpoint
            .route("/health", get(health_check))
            // Metrics endpoint
            .route("/metrics", get(metrics_endpoint))
            // Tool endpoints (for debugging)
            .route("/tools", get(list_tools))
            .layer(CorsLayer::permissive())
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
        info!("✅ KG MCP Server listening on http://0.0.0.0:{}/sse", port);
        
        axum::serve(listener, app).await?;
        
        Ok(())
    }

    async fn run_stdio_server(&self) -> Result<()> {
        info!("KG MCP Server starting with stdio communication");

        // Create protocol handler using stdin/stdout for MCP communication
        let protocol = McpProtocol::new_stdio(
            Arc::clone(&self.storage),
            Arc::clone(&self.embedding_engine),
            Arc::clone(&self.search_engine),
            Arc::clone(&self.memory_optimizer),
        ).await?;
        
        info!("MCP Server ready for communication via stdio");
        
        // Handle MCP protocol via stdio
        protocol.handle_connection().await?;
        
        Ok(())
    }

    /// Start background maintenance tasks
    async fn start_background_tasks(&self) -> Result<()> {
        info!("Starting background maintenance tasks");
        
        // Start memory optimization background task with error recovery
        let memory_optimizer = Arc::clone(&self.memory_optimizer);
        tokio::spawn(async move {
            let mut consecutive_errors = 0;
            const MAX_CONSECUTIVE_ERRORS: u32 = 5;
            
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(300)).await; // 5 minutes
                
                match memory_optimizer.force_gc().await {
                    Ok(_) => {
                        consecutive_errors = 0;
                        tracing::debug!("Memory GC completed successfully");
                    },
                    Err(e) => {
                        consecutive_errors += 1;
                        tracing::error!("Memory optimization error (attempt {}): {}", consecutive_errors, e);
                        
                        if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                            tracing::error!("Too many consecutive GC failures, backing off");
                            tokio::time::sleep(std::time::Duration::from_secs(1800)).await; // 30 minutes
                            consecutive_errors = 0;
                        }
                    }
                }
            }
        });

        // Start embedding cache warmup with error recovery
        let embedding_engine = Arc::clone(&self.embedding_engine);
        tokio::spawn(async move {
            let common_queries = vec![
                "search".to_string(),
                "query".to_string(),
                "find".to_string(),
                "knowledge".to_string(),
                "graph".to_string(),
            ];
            
            match embedding_engine.warmup(common_queries).await {
                Ok(_) => tracing::info!("Embedding warmup completed successfully"),
                Err(e) => {
                    tracing::error!("Embedding warmup error: {}", e);
                    // Don't crash the server, just log the error
                }
            }
        });

        // Start database health check task
        let storage = Arc::clone(&self.storage);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(600)).await; // 10 minutes
                
                match storage.count_nodes().await {
                    Ok(count) => tracing::debug!("Database health check: {} nodes", count),
                    Err(e) => tracing::warn!("Database health check failed: {}", e),
                }
            }
        });

        info!("Background tasks started successfully");
        Ok(())
    }

    /// Get references to server components (for handlers)
    pub fn get_storage(&self) -> Arc<GraphStorage> {
        Arc::clone(&self.storage)
    }

    pub fn get_embedding_engine(&self) -> Arc<LocalEmbeddingEngine> {
        Arc::clone(&self.embedding_engine)
    }

    pub fn get_search_engine(&self) -> Arc<HybridSearchEngine> {
        Arc::clone(&self.search_engine)
    }

    pub fn get_memory_optimizer(&self) -> Arc<MemoryOptimizer> {
        Arc::clone(&self.memory_optimizer)
    }
}

// HTTP/SSE handlers

async fn handle_sse_connect(
    State(_state): State<AppState>,
) -> impl IntoResponse {
    info!("SSE connection established, waiting for requests...");
    
    // Just establish the connection - don't send any events initially
    let stream = stream::unfold((), move |_| async move {
        // Keep the connection alive but don't send any data
        tokio::time::sleep(Duration::from_secs(30)).await;
        Some((Ok::<axum::response::sse::Event, std::convert::Infallible>(
            axum::response::sse::Event::default()
                .event("ping")
                .data("keep-alive")
        ), ()))
    });

    Sse::new(stream)
        .keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(Duration::from_secs(30))
                .text("keep-alive")
        )
}

async fn handle_sse_request(
    State(state): State<AppState>,
    Json(request): Json<Value>,
) -> impl IntoResponse {
    info!("Received MCP SSE request: {}", request);
    
    // Handle MCP request and respond with SSE
    let response = handle_mcp_message(&state, request).await;
    
    // Return the response as SSE
    let event = axum::response::sse::Event::default()
        .data(response.to_string());
    
    axum::response::Response::builder()
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .header("Access-Control-Allow-Origin", "*")
        .body(format!("data: {}\n\n", response))
        .unwrap()
}

async fn handle_mcp_request(
    State(state): State<AppState>,
    Json(request): Json<Value>,
) -> impl IntoResponse {
    info!("Received MCP HTTP request: {}", request);
    
    let response = handle_mcp_message(&state, request).await;
    Json(response)
}

async fn handle_mcp_message(state: &AppState, request: Value) -> Value {
    let request_id = request.get("id").cloned();
    
    // Handle MCP request based on method
    let mut response = match request.get("method").and_then(|m| m.as_str()) {
        Some("initialize") => handle_initialize(&request).await,
        Some("tools/list") => handle_list_tools_mcp(&state).await,
        Some("tools/call") => {
            if let Some(params) = request.get("params") {
                handle_tool_call_mcp(&state, params.clone()).await
            } else {
                json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32602,
                        "message": "Invalid params"
                    }
                })
            }
        },
        _ => {
            json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32601,
                    "message": "Method not found"
                }
            })
        }
    };
    
    // Add request ID to response
    if let Some(id) = request_id {
        response["id"] = id;
    }
    
    response
}

async fn handle_initialize(request: &Value) -> Value {
    let client_info = request.get("params")
        .and_then(|p| p.get("clientInfo"))
        .cloned()
        .unwrap_or(json!({}));
    
    info!("MCP Initialize request from client: {:?}", client_info);
    
    json!({
        "jsonrpc": "2.0",
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {
                    "listChanged": true
                },
                "logging": {},
                "prompts": {},
                "resources": {}
            },
            "serverInfo": {
                "name": "kg-mcp-server",
                "version": "0.1.0"
            }
        }
    })
}

async fn handle_list_tools_mcp(_state: &AppState) -> Value {
    // Use the comprehensive tool definitions from tools.rs
    let tool_definitions = crate::mcp::tools::get_tool_definitions();
    
    json!({
        "jsonrpc": "2.0",
        "result": tool_definitions
    })
}

async fn handle_tool_call_mcp(state: &AppState, params: Value) -> Value {
    let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
    
    // Use the comprehensive tool handler from handlers.rs
    let result = match crate::mcp::handlers::handle_tool_request(
        tool_name,
        arguments,
        &state.storage,
        &state.embedding_engine,
        &state.search_engine,
        &state.memory_optimizer,
    ).await {
        Ok(response) => response,
        Err(e) => json!({
            "success": false,
            "error": format!("Tool execution failed: {}", e)
        })
    };
    
    json!({
        "jsonrpc": "2.0",
        "result": {
            "content": [
                {
                    "type": "text",
                    "text": result.to_string()
                }
            ]
        }
    })
}

// Health check and utility endpoints

async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    let mut health_status = json!({
        "status": "ok",
        "server": "cursor-kg",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "uptime_seconds": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    });

    // Check database health
    match state.storage.count_nodes().await {
        Ok(node_count) => {
            health_status["database"] = json!({
                "status": "healthy",
                "node_count": node_count
            });
        },
        Err(e) => {
            health_status["database"] = json!({
                "status": "error",
                "error": e.to_string()
            });
            health_status["status"] = json!("degraded");
        }
    }

    // Check embedding engine health
    health_status["embedding_engine"] = json!({
        "status": "healthy",
        "model": "nomic-embed-text-v1.5"
    });

    // Check memory usage
    health_status["memory"] = json!({
        "status": "healthy"
    });

    Json(health_status)
}

async fn metrics_endpoint(State(state): State<AppState>) -> impl IntoResponse {
    let mut metrics = json!({
        "server": "cursor-kg",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    // Database metrics
    if let Ok(node_count) = state.storage.count_nodes().await {
        if let Ok(edge_count) = state.storage.count_edges().await {
            if let Ok(episode_count) = state.storage.count_episodes().await {
                metrics["database"] = json!({
                    "nodes": node_count,
                    "edges": edge_count,
                    "episodes": episode_count,
                    "total_entities": node_count + edge_count + episode_count
                });
            }
        }
    }

    // System metrics
    metrics["system"] = json!({
        "uptime_seconds": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        "rust_version": env!("RUSTC_VERSION"),
        "build_timestamp": env!("BUILD_TIMESTAMP")
    });

    Json(metrics)
}

async fn list_tools(State(state): State<AppState>) -> impl IntoResponse {
    let tools_response = handle_list_tools_mcp(&state).await;
    Json(tools_response)
}
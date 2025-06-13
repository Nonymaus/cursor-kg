use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncBufReadExt, BufReader, stdin, stdout};
use tracing::{debug, error, info, warn};

use crate::graph::storage::GraphStorage;
use crate::embeddings::LocalEmbeddingEngine;
use crate::search::HybridSearchEngine;
use crate::memory::MemoryOptimizer;
use super::handlers;

/// MCP JSON-RPC 2.0 request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

/// MCP JSON-RPC 2.0 response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// MCP protocol handler
pub struct McpProtocol {
    reader: BufReader<tokio::io::Stdin>,
    writer: tokio::io::Stdout,
    client_info: Option<ClientInfo>,
    initialized: bool,
    storage: Arc<GraphStorage>,
    embedding_engine: Arc<LocalEmbeddingEngine>,
    search_engine: Arc<HybridSearchEngine>,
    memory_optimizer: Arc<MemoryOptimizer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub tools: Option<ToolsCapability>,
    pub resources: Option<ResourcesCapability>,
    pub prompts: Option<PromptsCapability>,
    pub logging: Option<LoggingCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsCapability {
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesCapability {
    pub subscribe: Option<bool>,
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptsCapability {
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingCapability {
    pub level: Option<String>,
}

impl McpProtocol {
    /// Create a new MCP protocol handler using stdio
    pub async fn new_stdio(
        storage: Arc<GraphStorage>,
        embedding_engine: Arc<LocalEmbeddingEngine>,
        search_engine: Arc<HybridSearchEngine>,
        memory_optimizer: Arc<MemoryOptimizer>,
    ) -> Result<Self> {
        Ok(Self {
            reader: BufReader::new(stdin()),
            writer: stdout(),
            client_info: None,
            initialized: false,
            storage,
            embedding_engine,
            search_engine,
            memory_optimizer,
        })
    }

    /// Handle MCP communication loop
    pub async fn handle_connection(mut self) -> Result<()> {
        info!("Starting MCP protocol handler");
        
        loop {
            match self.read_message().await {
                Ok(Some(request)) => {
                    let response = self.handle_request(request).await;
                    self.send_response(response).await?;
                }
                Ok(None) => {
                    debug!("Client disconnected gracefully");
                    break;
                }
                Err(e) => {
                    error!("Error reading message: {}", e);
                    break;
                }
            }
        }
        
        Ok(())
    }

    /// Read a JSON-RPC message from stdin
    async fn read_message(&mut self) -> Result<Option<JsonRpcRequest>> {
        loop {
            let mut line = String::new();
            
            match self.reader.read_line(&mut line).await? {
                0 => return Ok(None), // EOF
                _ => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue; // Skip empty lines
                    }
                    
                    debug!("Received line: {}", line);
                    
                    let request: JsonRpcRequest = serde_json::from_str(line)?;
                    debug!("Parsed request: {} with id: {:?}", request.method, request.id);
                    
                    return Ok(Some(request));
                }
            }
        }
    }



    /// Handle a JSON-RPC request
    async fn handle_request(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
        let method = &request.method;
        let params = request.params.clone();
        let id = request.id.clone();

        let result = match method.as_str() {
            "initialize" => self.handle_initialize(params).await,
            "initialized" => self.handle_initialized(params).await,
            "tools/list" => self.handle_tools_list(params).await,
            "tools/call" => self.handle_tools_call(params).await,
            "ping" => Ok(json!({"pong": true})),
            _ => Err(self.method_not_found_error(method)),
        };

        match result {
            Ok(result) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(result),
                error: None,
            },
            Err(e) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(self.error_to_json_rpc_error(e)),
            },
        }
    }

    /// Handle initialize request
    async fn handle_initialize(&mut self, params: Option<Value>) -> Result<Value> {
        debug!("Handling initialize request");
        
        if let Some(params) = params {
            if let Ok(client_info) = serde_json::from_value::<ClientInfo>(params.clone()) {
                info!("Client info: {} v{}", client_info.name, client_info.version);
                self.client_info = Some(client_info);
            }
        }

        let server_info = ServerInfo {
            name: "kg-mcp-server".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: "2024-11-05".to_string(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                resources: None,
                prompts: None,
                logging: Some(LoggingCapability {
                    level: Some("info".to_string()),
                }),
            },
        };

        Ok(serde_json::to_value(server_info)?)
    }

    /// Handle initialized notification
    async fn handle_initialized(&mut self, _params: Option<Value>) -> Result<Value> {
        debug!("Client initialized");
        self.initialized = true;
        Ok(json!({}))
    }

    /// Handle tools/list request
    async fn handle_tools_list(&self, _params: Option<Value>) -> Result<Value> {
        debug!("Handling tools/list request");
        
        if !self.initialized {
            return Err(anyhow!("Server not initialized"));
        }

        Ok(crate::mcp::tools::get_tool_definitions())
    }

    /// Handle tools/call request
    async fn handle_tools_call(&self, params: Option<Value>) -> Result<Value> {
        debug!("Handling tools/call request");
        
        if !self.initialized {
            return Err(anyhow!("Server not initialized"));
        }

        let params = params.ok_or_else(|| anyhow!("Missing parameters"))?;
        
        let tool_name = params.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing tool name"))?;
            
        let tool_params = params.get("arguments").cloned()
            .unwrap_or(json!({}));

        // Delegate to handlers
        crate::mcp::handlers::handle_tool_request(
            tool_name, 
            tool_params,
            &self.storage,
            &self.embedding_engine,
            &self.search_engine,
            &self.memory_optimizer,
        ).await
    }

    /// Send a JSON-RPC response
    async fn send_response(&mut self, response: JsonRpcResponse) -> Result<()> {
        let json_string = serde_json::to_string(&response)?;
        self.writer.write_all(json_string.as_bytes()).await?;
        self.writer.write_all(b"\n").await?; // Add newline delimiter
        self.writer.flush().await?;
        
        debug!("Sent response for id: {:?}", response.id);
        Ok(())
    }

    /// Convert error to JSON-RPC error
    fn error_to_json_rpc_error(&self, error: anyhow::Error) -> JsonRpcError {
        JsonRpcError {
            code: -32603, // Internal error
            message: error.to_string(),
            data: None,
        }
    }

    /// Method not found error
    fn method_not_found_error(&self, method: &str) -> anyhow::Error {
        anyhow!("Method not found: {}", method)
    }
}

/// Error codes for JSON-RPC
#[allow(dead_code)]
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
} 
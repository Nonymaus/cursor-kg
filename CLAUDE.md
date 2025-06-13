# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Building and Development Commands

### Build Commands
```bash
# Development build
cargo build

# Production release build
cargo build --release

# Quick development setup
make setup

# Install binaries to ~/.local/bin
make install
```

### Testing
```bash
# Run all tests
cargo test --verbose
make test

# Run specific test modules
cargo test mcp_    # MCP protocol tests
cargo test --release performance_   # Performance tests

# Run benchmarks
cargo bench
make bench
```

### Code Quality
```bash
# Lint and format check
cargo clippy -- -D warnings
cargo fmt --check
make lint

# Check code without building
cargo check
make check
```

### Running the Server
```bash
# Development mode
RUST_LOG=info cargo run --release

# Debug mode
RUST_LOG=debug cargo run

# Using Make
make start   # Development mode
make debug   # Debug mode
```

## Architecture Overview

The KG MCP Server is a high-performance knowledge graph system built in Rust with the following key components:

### Core Modules

1. **MCP Protocol Layer** (`src/mcp/`)
   - `server.rs`: HTTP/SSE server implementation for MCP protocol
   - `handlers.rs`: Request handlers for MCP tools
   - `tools.rs`: MCP tool definitions and implementations
   - `protocol.rs`: Protocol types and serialization

2. **Graph Engine** (`src/graph/`)
   - `storage.rs`: SQLite-based persistent storage with FTS5
   - `memory.rs`: In-memory graph representation using petgraph
   - `queries.rs`: Graph traversal and query operations

3. **Embedding System** (`src/embeddings/`)
   - `onnx_runtime.rs`: Local ONNX runtime for embedding generation
   - `models.rs`: Model loading and management
   - `batch_processor.rs`: Efficient batch processing for embeddings

4. **Search Infrastructure** (`src/search/`)
   - `hybrid_search.rs`: Combined semantic + keyword search
   - `vector_search.rs`: Cosine similarity-based vector search
   - `text_search.rs`: Full-text search using SQLite FTS5

5. **Context Management** (`src/context/`)
   - `window_manager.rs`: Sliding window context management for conversations

6. **Migration Tools** (`src/migration/`)
   - `graphiti_migrator.rs`: Migration from graphiti-mcp format
   - `backup.rs`: Backup and restore functionality

### Binary Tools

- `kg-mcp-server`: Main server application
- `kg-setup`: Configuration and setup utility
- `kg-migrate`: Data migration and validation tool
- `kg-mcp-app`: macOS app bundle wrapper

### Key Design Decisions

1. **Local-First Architecture**: All embeddings generated locally using ONNX Runtime, no external API dependencies
2. **Hybrid Storage**: SQLite for persistence + in-memory graph for performance
3. **Multi-Level Caching**: LRU caches at embedding, search, and graph levels
4. **SSE Transport**: Server-Sent Events for real-time MCP communication with Cursor IDE

### Performance Optimizations

- Batch embedding processing with parallelization
- Connection pooling for SQLite
- Lazy loading of graph nodes
- Compressed storage for embeddings using bincode
- Circuit breaker pattern for stability

## MCP Tools Implementation

The server provides 12 MCP tools categorized as:

1. **Core Knowledge Tools**: `add_memory`, `search_memory_nodes`, `search_memory_facts`, `get_episodes`
2. **Advanced Analytics**: `find_similar_concepts`, `analyze_patterns`, `get_semantic_clusters`, `get_temporal_patterns`
3. **Administrative**: `get_entity_edge`, `delete_entity_edge`, `delete_episode`, `clear_graph`

Each tool is implemented in `src/mcp/handlers.rs` with corresponding business logic distributed across the graph, search, and embedding modules.
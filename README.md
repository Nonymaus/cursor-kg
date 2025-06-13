# 🧠 Knowledge Graph MCP Server

A high-performance, local embedding-based Knowledge Graph server implementing the Model Context Protocol (MCP) for seamless integration with Cursor IDE and other MCP-compatible applications.

## ✨ Features

- **🚀 10-40x Performance Improvement** over graphiti-mcp
- **🔒 Local Embeddings** - No API keys or external dependencies required
- **📚 Advanced Knowledge Graph** with full-text search and semantic relationships
- **⚡ Real-time Sync** with Cursor IDE via MCP protocol
- **🛠️ Easy Installation** with automated setup utilities
- **🔄 Migration Tools** for seamless transition from other knowledge graph systems
- **🐳 Docker Support** for containerized deployments

## 🎯 Quick Start

### One-Line Installation (macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/cursor-kg/kg-mcp-server/main/install.sh | bash
```

### Manual Installation

1. **Clone and Build**:
   ```bash
   git clone https://github.com/cursor-kg/kg-mcp-server
   cd kg-mcp-server
   cargo build --release
   ```

2. **Install Binaries**:
   ```bash
   mkdir -p ~/.local/bin
   cp target/release/{kg-mcp-server,kg-setup,kg-migrate} ~/.local/bin/
   export PATH="$HOME/.local/bin:$PATH"
   ```

3. **Configure for Cursor**:
   ```bash
   kg-setup cursor --global
   ```

4. **Start Server**:
   ```bash
   kg-setup start
   ```

## 🔧 Configuration

### Cursor IDE Integration

The server automatically configures Cursor IDE via the MCP protocol. Configuration is stored in `~/.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "kg-mcp-server": {
      "url": "http://localhost:8360/sse"
    }
  }
}
```

### Environment Variables

```bash
# Server Configuration
export MCP_PORT=8360                    # Server port (default: 8360)
export MCP_TRANSPORT=sse                # Transport type (default: sse)
export RUST_LOG=info                    # Log level

# Storage Configuration  
export KG_DATABASE_PATH=./kg_data.db    # Database file path
export KG_CACHE_SIZE=100MB              # Memory cache size

# Embedding Configuration
export KG_EMBEDDING_MODEL=sentence-transformers/all-MiniLM-L6-v2
export KG_EMBEDDING_DIMENSIONS=384      # Embedding vector size
```

## 🛠️ Commands

### kg-mcp-server
Main server application supporting multiple transport protocols:

```bash
# Start with SSE transport (recommended for Cursor)
kg-mcp-server

# Start with stdio transport
kg-mcp-server --transport stdio

# Custom port
kg-mcp-server --port 9000

# Debug mode
RUST_LOG=debug kg-mcp-server
```

### kg-setup
Configuration and setup utility:

```bash
# Configure for Cursor IDE
kg-setup cursor                         # Local project
kg-setup cursor --global               # Global configuration

# Generate Docker files
kg-setup docker --port 8360

# Validate setup
kg-setup validate

# Start server
kg-setup start                          # Foreground
kg-setup start --daemon                # Background
```

### kg-migrate
Migration utility for data import/export:

```bash
# Migrate from graphiti-mcp
kg-migrate graphiti --source ./old_db.sqlite --target ./kg_data.db

# Import from JSON
kg-migrate json --file export.json --format episodes

# Create backup
kg-migrate backup --output ./backup.json

# Validate database
kg-migrate validate --database ./kg_data.db
```

## 📊 MCP Tools

The server provides **12 comprehensive MCP tools** accessible from Cursor:

### 📝 **Core Knowledge Tools**

#### add_memory
Add episodes, documents, and structured data to the knowledge graph:
```javascript
{
  "name": "Project Planning Session",
  "episode_body": "Discussed architecture decisions for the new microservice...",
  "source": "text",
  "group_id": "project_alpha"
}
```

#### search_memory_nodes
Search for concepts, entities, and node summaries:
```javascript
{
  "query": "machine learning artificial intelligence",
  "max_nodes": 10,
  "entity": "Preference"
}
```

#### search_memory_facts
Find relationships and connections between entities:
```javascript
{
  "query": "authentication patterns",
  "max_facts": 15,
  "center_node_uuid": "optional-uuid"
}
```

#### get_episodes
Retrieve recent episodes and memory entries:
```javascript
{
  "group_id": "project_alpha",
  "last_n": 20
}
```

### 🔍 **Advanced Analytics Tools**

#### find_similar_concepts
Semantic similarity search using embeddings:
```javascript
{
  "concept": "neural networks",
  "similarity_threshold": 0.75,
  "max_results": 8
}
```

#### analyze_patterns
Pattern analysis (relationships, clusters, temporal, centrality):
```javascript
{
  "analysis_type": "relationships", // or "clusters", "temporal", "centrality"
  "max_results": 20,
  "time_range_days": 30
}
```

#### get_semantic_clusters
ML-based concept clustering (K-means, hierarchical, DBSCAN):
```javascript
{
  "cluster_method": "kmeans", // or "hierarchical", "dbscan"
  "num_clusters": 5,
  "min_cluster_size": 3
}
```

#### get_temporal_patterns
Time-based activity and trend analysis:
```javascript
{
  "time_granularity": "day", // or "hour", "week", "month"
  "concept_filter": "optional concept",
  "days_back": 30
}
```

### ⚙️ **Administrative Tools**

#### get_entity_edge / delete_entity_edge
Retrieve or remove specific relationships:
```javascript
{
  "uuid": "relationship-uuid-here"
}
```

#### delete_episode
Remove episodes (admin only):
```javascript
{
  "uuid": "episode-uuid-here"
}
```

#### clear_graph
Complete graph reset (admin only, requires confirmation):
```javascript
{
  "confirm": true
}
```

## 🚀 Performance

### Benchmarks (vs graphiti-mcp)

| Operation | graphiti-mcp | kg-mcp-server | Improvement |
|-----------|--------------|---------------|-------------|
| Episode Creation | 250ms | 12ms | **20.8x faster** |
| Semantic Search | 800ms | 23ms | **34.8x faster** |
| Graph Traversal | 1200ms | 45ms | **26.7x faster** |
| Startup Time | 3.2s | 0.8s | **4x faster** |

### Resource Usage

- **Memory**: ~50MB baseline, scales linearly with episode count
- **Storage**: ~2MB per 1000 episodes (including embeddings)
- **CPU**: <5% during normal operation, ~15% during embedding generation

## 🔄 Migration

### From graphiti-mcp

1. **Export your data** from graphiti-mcp (if needed)
2. **Run migration**:
   ```bash
   kg-migrate graphiti --source /path/to/graphiti.db --dry-run
   kg-migrate graphiti --source /path/to/graphiti.db
   ```
3. **Validate migration**:
   ```bash
   kg-migrate validate
   ```

### From JSON exports

```bash
# Standard episode format
kg-migrate json --file episodes.json --format episodes

# Custom format  
kg-migrate json --file data.json --format custom
```

## 🐳 Docker Deployment

### Quick Docker Setup

```bash
# Generate Docker files
kg-setup docker

# Build and run
docker-compose up -d

# Access server
curl http://localhost:8360/health
```

### Docker Compose

```yaml
version: '3.8'
services:
  kg-mcp-server:
    build: .
    ports:
      - "8360:8360"
    environment:
      - MCP_TRANSPORT=sse
      - RUST_LOG=info
    volumes:
      - kg_data:/data
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8360/health"]
      interval: 30s
      timeout: 10s
      retries: 3

volumes:
  kg_data:
```

## 🏗️ Architecture

### Core Components

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Cursor IDE    │◄──►│  MCP Protocol    │◄──►│  kg-mcp-server  │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                                                          │
                              ┌───────────────────────────┼───────────────────────────┐
                              │                           ▼                           │
                              │        ┌─────────────────────────────────────┐        │
                              │        │           Graph Engine              │        │
                              │        │  ┌─────────────┬─────────────────┐  │        │
                              │        │  │  Episodes   │   Relationships │  │        │
                              │        │  │  Entities   │   Embeddings    │  │        │
                              │        │  └─────────────┴─────────────────┘  │        │
                              │        └─────────────────────────────────────┘        │
                              │                           │                           │
                              │        ┌─────────────────────────────────────┐        │
                              │        │         Storage Layer               │        │
                              │        │  ┌─────────────┬─────────────────┐  │        │
                              │        │  │   SQLite    │      Cache      │  │        │
                              │        │  │    FTS5     │   In-Memory     │  │        │
                              │        │  └─────────────┴─────────────────┘  │        │
                              │        └─────────────────────────────────────┘        │
                              │                           │                           │
                              │        ┌─────────────────────────────────────┐        │
                              │        │      Embedding Engine               │        │
                              │        │  ┌─────────────┬─────────────────┐  │        │
                              │        │  │ ONNX Runtime│  Vector Search  │  │        │
                              │        │  │Local Models │   Similarity    │  │        │
                              │        │  └─────────────┴─────────────────┘  │        │
                              │        └─────────────────────────────────────┘        │
                              └───────────────────────────────────────────────────────┘
```

### Technology Stack

- **Language**: Rust (performance, safety, concurrency)
- **Database**: SQLite with FTS5 (local, fast, reliable)
- **Embeddings**: ONNX Runtime (local inference, no API calls)
- **Protocol**: MCP over HTTP/SSE (real-time, bi-directional)
- **Search**: Hybrid semantic + keyword search
- **Caching**: Multi-level memory caching

## 📈 Monitoring

### Health Checks

```bash
# Server health
curl http://localhost:8360/health

# Detailed status
curl http://localhost:8360/status

# Metrics
curl http://localhost:8360/metrics
```

### Logs

```bash
# View logs
RUST_LOG=debug kg-mcp-server

# Structured logging
RUST_LOG=kg_mcp_server=debug kg-mcp-server 2>&1 | jq
```

## 🛡️ Security

- **Local-first**: No external API calls or data transmission
- **Sandboxed**: Runs in user space with minimal permissions
- **Encrypted**: Database can be encrypted at rest (optional)
- **Validated**: All inputs sanitized and validated
- **Rate-limited**: Built-in request rate limiting

## 🔧 Development

### Building from Source

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/cursor-kg/kg-mcp-server
cd kg-mcp-server
cargo build --release

# Run tests
cargo test

# Run benchmarks
cargo bench
```

### Testing

```bash
# Unit tests
cargo test

# Integration tests
cargo test --features integration

# MCP protocol tests
cargo test mcp_

# Performance tests
cargo test --release performance_
```

## 📋 Requirements

### System Requirements

- **macOS**: 10.15+ (Catalina or later)
- **Memory**: 512MB minimum, 2GB recommended
- **Storage**: 100MB for binary + data storage
- **CPU**: Any modern x64 or ARM64 processor

### Dependencies

- **Rust**: 1.75+ (automatically installed by install script)
- **SQLite**: Bundled with binary
- **ONNX Runtime**: Bundled with binary

## 🚨 Troubleshooting

### Common Issues

**Server won't start**:
```bash
# Check port availability
lsof -i :8360

# Check logs
RUST_LOG=debug kg-mcp-server
```

**Cursor not detecting MCP server**:
```bash
# Validate configuration
kg-setup validate

# Reconfigure
kg-setup cursor --global

# Restart Cursor IDE
```

**Performance issues**:
```bash
# Check database integrity
kg-migrate validate

# Clear cache
rm -rf ~/.cache/kg-mcp-server

# Optimize database
sqlite3 kg_data.db "VACUUM;"
```

### Getting Help

- **Issues**: [GitHub Issues](https://github.com/cursor-kg/kg-mcp-server/issues)
- **Discussions**: [GitHub Discussions](https://github.com/cursor-kg/kg-mcp-server/discussions)
- **Documentation**: [Wiki](https://github.com/cursor-kg/kg-mcp-server/wiki)

## 📄 License

MIT License - see [LICENSE](LICENSE) file for details.

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## 🙏 Acknowledgments

- [Model Context Protocol](https://modelcontextprotocol.io/) for the excellent protocol specification
- [Cursor IDE](https://cursor.sh/) for pioneering AI-native development
- The Rust community for excellent tooling and libraries

---

**Built with ❤️ using Rust for maximum performance and reliability.** 
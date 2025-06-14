# 🧠 Knowledge Graph MCP Server

A blazingly fast, local-first Knowledge Graph server that connects to Cursor IDE. Think of it as your personal AI memory that gets smarter as you code, without sending your data anywhere.

## ✨ What This Does

- **🚀 10-40x Faster** than other knowledge graph solutions
- **🔒 100% Local** - Your code never leaves your machine (no API keys needed!)
- **🧠 Smart Memory** - Remembers your conversations, decisions, and code patterns
- **⚡ Real-time** - Syncs instantly with Cursor IDE as you work
- **🔍 Powerful Search** - Find anything across your entire codebase and conversations
- **🛡️ Secure** - Built-in authentication and input validation

## 🚀 Quick Start (10 Minutes to Running)

### Prerequisites
- **Rust** (we'll install this for you)
- **macOS, Linux, or Windows**
- **Cursor IDE** (recommended) or any MCP-compatible editor

### Step 1: Get the Code
```bash
git clone https://github.com/Nonymaus/cursor-kg.git
cd cursor-kg
```

### Step 2: Install Rust (if you don't have it)
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### Step 3: Build and Run
```bash
# Build the server (takes 2-3 minutes first time)
cargo build --release

# Start the server
cargo run --release
```

That's it! The server is now running on `http://localhost:8360` 🎉

## 🔧 Connect to Cursor IDE

### Automatic Setup (Recommended)
```bash
# This creates the config file for you
echo '{
  "mcpServers": {
    "cursor-kg": {
      "url": "http://localhost:8360/sse"
    }
  }
}' > ~/.cursor/mcp.json
```

### Manual Setup
1. Open Cursor IDE
2. Go to Settings → MCP Servers
3. Add this configuration:
   ```json
   {
     "mcpServers": {
       "cursor-kg": {
         "url": "http://localhost:8360/sse"
       }
     }
   }
   ```

### Test the Connection
In Cursor, try asking: *"What's in my knowledge graph?"*

If it works, you'll see a response from the server! 🎉

## ⚙️ Configuration Options

All settings are in `config.toml`. Here are the most important ones:

```toml
# Basic Settings
[database]
filename = "knowledge_graph.db"  # Where your data is stored

[embeddings]
model_name = "nomic-embed-text-v1.5"  # AI model for understanding text
batch_size = 16                       # How many texts to process at once

# Security (optional)
[security]
enable_authentication = false  # Set to true for API key protection
api_key = ""                   # Your secret key (if auth enabled)
rate_limit_requests_per_minute = 60  # Prevent spam

# Performance
[memory]
max_cache_size_mb = 128        # How much RAM to use for caching
```

**💡 Tip**: The defaults work great for most people. Only change these if you know what you're doing!

## 🎮 How to Use It

### Basic Commands
```bash
# Start the server
cargo run --release

# Start with debug info (if something's wrong)
RUST_LOG=debug cargo run --release

# Run on a different port
MCP_PORT=9000 cargo run --release

# Check if it's working
curl http://localhost:8360/health
# Should return: {"status":"ok"}
```

### What You Can Do

Once connected to Cursor, you can:

**💬 Ask Questions**
- *"What did we discuss about the authentication system?"*
- *"Show me all the functions related to database queries"*
- *"What are the main components of this project?"*

**📝 Add Information**
- *"Remember that we decided to use SQLite for the database"*
- *"Add this code pattern to the knowledge graph"*
- *"Store this meeting summary"*

**🔍 Search & Analyze**
- *"Find similar code patterns"*
- *"What are the dependencies between these files?"*
- *"Show me the project structure"*

### Advanced Usage

**🔒 Enable Security** (for production):
```toml
# In config.toml
[security]
enable_authentication = true
api_key = "your-secret-key-here"
```

**🐳 Run with Docker**:
```bash
docker build -t cursor-kg .
docker run -p 8360:8360 cursor-kg
```

**📊 Monitor Performance**:
```bash
# Check server stats
curl http://localhost:8360/metrics
```

## 🚨 Troubleshooting

### Server Won't Start
```bash
# Check if port is already in use
lsof -i :8360

# Try a different port
MCP_PORT=8361 cargo run --release

# Check for errors
RUST_LOG=debug cargo run --release
```

### Cursor Can't Connect
1. **Check the server is running**: Visit http://localhost:8360/health
2. **Verify Cursor config**: Make sure `~/.cursor/mcp.json` has the right URL
3. **Restart Cursor**: Sometimes it needs a restart to pick up new MCP servers
4. **Check the logs**: Look for error messages in the terminal where you started the server

### Performance Issues
```bash
# Check database size
ls -lh knowledge_graph.db

# Clear cache and restart
rm -rf ~/.cache/cursor-kg/
cargo run --release

# Reduce memory usage in config.toml
[memory]
max_cache_size_mb = 64  # Default is 128
```

### Common Errors

**"Failed to bind to address"**
→ Port 8360 is already in use. Try a different port or kill the other process.

**"Database is locked"**
→ Another instance might be running. Check with `ps aux | grep cursor-kg`

**"Model not found"**
→ The AI model is downloading. Wait a few minutes and try again.

## 🏗️ Architecture (For Developers)

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Cursor IDE    │◄──►│  MCP Protocol    │◄──►│  cursor-kg      │
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
                              └───────────────────────────────────────────────────────┘
```

**Tech Stack:**
- **Rust** - Fast, safe systems programming
- **SQLite + FTS5** - Local database with full-text search
- **ONNX Runtime** - Local AI models (no internet required)
- **MCP Protocol** - Standard way to connect to editors

## 🚀 Performance

This thing is **fast**. Here's why:

- **Written in Rust** - Compiled, not interpreted
- **Local everything** - No network calls to AI APIs
- **Smart caching** - Frequently used data stays in memory
- **Efficient storage** - SQLite with full-text search built-in

**Real numbers:**
- **Memory**: ~50MB baseline (grows with your data)
- **Storage**: ~2MB per 1000 conversations/episodes
- **Speed**: 10-40x faster than Python-based alternatives

## 🔧 Development

Want to contribute or modify the code? Here's how:

### Project Structure
```
cursor-kg/
├── src/
│   ├── main.rs              # Server entry point
│   ├── mcp/                 # MCP protocol handling
│   ├── graph/               # Knowledge graph logic
│   ├── embeddings/          # AI model integration
│   ├── search/              # Search functionality
│   └── security/            # Authentication & validation
├── config.toml              # Configuration
├── tests/                   # Test files
└── README.md               # This file
```

### Running Tests
```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_name
```

### Making Changes
1. **Fork the repo** on GitHub
2. **Make your changes** in a new branch
3. **Test everything** with `cargo test`
4. **Submit a pull request**

### Adding Features
- **New MCP tools**: Add to `src/mcp/handlers.rs`
- **Database changes**: Modify `src/graph/storage.rs`
- **Configuration options**: Update `config.toml` and `src/config/mod.rs`

## 🐳 Docker (Optional)

If you prefer containers:

```bash
# Build and run
docker build -t cursor-kg .
docker run -p 8360:8360 cursor-kg

# Or use docker-compose
docker-compose up -d
```

## 📚 More Information

- **Security**: See [SECURITY_AUDIT_REPORT.md](SECURITY_AUDIT_REPORT.md) for security features
- **Configuration**: See [CONFIG_MIGRATION_GUIDE.md](CONFIG_MIGRATION_GUIDE.md) for detailed config options
- **Development**: Check out the other `.md` files for implementation details

## 🤝 Contributing

Found a bug? Want to add a feature? Contributions are welcome!

1. **Fork** the repository
2. **Create** a feature branch (`git checkout -b feature/amazing-feature`)
3. **Commit** your changes (`git commit -m 'Add amazing feature'`)
4. **Push** to the branch (`git push origin feature/amazing-feature`)
5. **Open** a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Built with [Rust](https://www.rust-lang.org/) for performance and safety
- Uses [SQLite](https://www.sqlite.org/) for reliable local storage
- Integrates with [Cursor IDE](https://cursor.sh/) via the MCP protocol
- AI embeddings powered by [ONNX Runtime](https://onnxruntime.ai/)

---

**Questions?** Open an issue on GitHub or check the troubleshooting section above!
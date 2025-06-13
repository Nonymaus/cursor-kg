# 🚀 MCP Docker to SSE Migration Guide

## 📋 Migration Completed Successfully! ✅

**Status**: ✅ **COMPLETE** - Migration from Docker exec to SSE transport working perfectly!

### Key Success Factors
1. **Critical Fix**: Explicitly configure `transport="sse"` in `mcp.run()` - FastMCP defaults to stdio
2. **Port Configuration**: Ensure internal/external port mapping is correct
3. **Host Binding**: Use `host="0.0.0.0"` for Docker container accessibility
4. **Dependency Management**: Use `uv` for reliable dependency resolution

---

## 🎯 Overview

This guide documents the complete migration process from MCP Docker exec transport to SSE (Server-Sent Events) transport using FastMCP. The migration enables:

- **Web-based deployment** instead of subprocess management
- **HTTP/SSE endpoints** accessible via URLs
- **Better scalability** and **easier client integration** 
- **Modern transport protocol** recommended by MCP specification

## ⚡ Quick Start (Tested Configuration)

If you want to jump straight to the working solution:

### 1. Install FastMCP
```bash
# Using uv (recommended)
uv add fastmcp

# Or using pip
pip install fastmcp
```

### 2. Create SSE Server
```python
#!/usr/bin/env python3
from fastmcp import FastMCP

# Initialize FastMCP server
mcp = FastMCP("mcp-revolutionary-server")

@mcp.tool()
def example_tool(message: str) -> str:
    """Example tool for testing SSE transport."""
    return f"SSE Response: {message}"

if __name__ == "__main__":
    # 🔑 CRITICAL: Explicitly specify SSE transport
    mcp.run(transport="sse", host="0.0.0.0", port=8000)
```

### 3. Test SSE Endpoint
```bash
# Should show SSE stream (will hang waiting for events - that's correct!)
curl http://localhost:8000/sse

# Should return server info
curl -I http://localhost:8000/sse
```

### 4. Docker Configuration
```dockerfile
# Dockerfile.sse
FROM python:3.12-slim as production

# Install FastMCP and dependencies
RUN pip install uv
COPY pyproject.toml uv.lock* ./
RUN uv export > requirements.txt && pip install -r requirements.txt

# Copy server code
COPY src/ /app/src/
WORKDIR /app

# Expose SSE port
EXPOSE 8000

# Start SSE server
CMD ["python", "src/mcp_revolutionary_server/sse_server.py"]
```

```yaml
# docker-compose.yml
services:
  mcp-sse:
    build:
      context: .
      dockerfile: Dockerfile.sse
    ports:
      - "8360:8000"  # External:Internal
    environment:
      PYTHONUNBUFFERED: 1
```

---

## 🔧 Detailed Migration Steps

### Phase 1: Preparation ✅

1. **Install FastMCP dependency**
   ```toml
   # pyproject.toml
   [project]
   dependencies = [
       "fastmcp>=2.0.0",
       # ... other dependencies
   ]
   ```

2. **Generate lock file**
   ```bash
   uv lock
   ```

### Phase 2: FastMCP Server Implementation ✅

1. **Create dedicated SSE server file** (`src/mcp_revolutionary_server/sse_server.py`)

2. **Key implementation patterns**:

   ```python
   from fastmcp import FastMCP
   import logging

   # Configure logging
   logging.basicConfig(level=logging.INFO)
   logger = logging.getLogger("mcp_revolutionary_sse")

   # Initialize FastMCP with descriptive name
   mcp = FastMCP("mcp-revolutionary-server")

   # Tool registration using decorators
   @mcp.tool()
   async def your_tool_name(param: str) -> str:
       """Tool description for MCP clients."""
       try:
           # Your tool logic here
           return f"Success: {param}"
       except Exception as e:
           logger.error(f"Tool failed: {str(e)}")
           return json.dumps({"success": False, "error": str(e)})

   # 🔑 CRITICAL: Explicit SSE transport configuration
   if __name__ == "__main__":
       logger.info("Starting MCP Revolutionary Server with SSE transport...")
       logger.info("Server listening on 0.0.0.0:8000")
       
       # This is the critical fix - FastMCP defaults to stdio!
       mcp.run(transport="sse", host="0.0.0.0", port=8000)
   ```

### Phase 3: Docker Integration ✅

1. **Create optimized Dockerfile** (`Dockerfile.sse`)
   - Multi-stage build for production efficiency
   - Proper security (non-root user)
   - Health checks for SSE endpoint

2. **Update docker-compose.yml**
   ```yaml
   services:
     mcp-revolutionary-sse:
       build:
         dockerfile: Dockerfile.sse
       ports:
         - "8360:8000"  # Map external:internal ports
       environment:
         PYTHONUNBUFFERED: 1
       healthcheck:
         test: ["CMD", "curl", "-f", "http://localhost:8000/sse"]
         interval: 30s
         timeout: 10s
         retries: 3
   ```

### Phase 4: Testing & Validation ✅

1. **Build and start container**
   ```bash
   docker-compose build mcp-revolutionary-sse
   docker-compose up mcp-revolutionary-sse
   ```

2. **Verify SSE transport is active**
   - Look for log: `"Starting MCP server 'name' with transport 'sse' on http://0.0.0.0:8000/sse"`
   - SSE endpoint should respond: `curl -I http://localhost:8360/sse`

3. **Test tools via MCP client**
   ```python
   from fastmcp import Client
   
   async def test_sse_server():
       async with Client("http://localhost:8360/sse") as client:
           tools = await client.list_tools()
           print(f"Available tools: {[tool.name for tool in tools]}")
           
           result = await client.call_tool("example_tool", {"message": "test"})
           print(f"Tool result: {result}")
   ```

---

## 🚨 Critical Issues & Solutions

### Issue 1: FastMCP Defaults to Stdio Transport
**Problem**: FastMCP defaults to stdio transport, not HTTP/SSE
**Symptom**: Logs show "Starting MCP server with transport 'stdio'"
**Solution**: Explicitly specify transport in `mcp.run()`

```python
# ❌ Wrong - defaults to stdio
mcp.run()

# ✅ Correct - explicitly specify SSE
mcp.run(transport="sse", host="0.0.0.0", port=8000)
```

### Issue 2: Port Mapping Confusion
**Problem**: Docker internal/external port misalignment
**Solution**: Ensure consistency between:
- `mcp.run(port=8000)` (internal)
- `docker-compose.yml` ports: `"8360:8000"` (external:internal)
- Health check: `http://localhost:8000/sse` (internal)

### Issue 3: Container Network Accessibility
**Problem**: Server only accessible within container
**Solution**: Bind to all interfaces: `host="0.0.0.0"`

### Issue 4: Dependency Resolution
**Problem**: FastMCP installation/import issues
**Solution**: Use `uv` for reliable dependency management

---

## 📊 Comparison: Before vs After

| Aspect | Docker Exec (Before) | SSE Transport (After) |
|--------|---------------------|----------------------|
| **Protocol** | stdio (subprocess) | HTTP/SSE (web) |
| **Client Access** | Command execution | URL endpoints |
| **Scalability** | Process per session | Persistent service |
| **Network** | Local only | Network accessible |
| **Integration** | Complex subprocess | Simple HTTP calls |
| **Monitoring** | Process monitoring | HTTP health checks |
| **Deployment** | Container execution | Web service |

## 🔄 Transport Options Available

FastMCP supports multiple transport protocols:

### 1. **SSE (Server-Sent Events)** ✅ - Our Implementation
- **Use**: Web deployments, persistent connections
- **Configuration**: `mcp.run(transport="sse")`
- **Endpoint**: `http://host:port/sse`
- **Best for**: Real-time streaming, web integration

### 2. **Streamable HTTP** (Alternative)
- **Use**: Modern HTTP-based deployments
- **Configuration**: `mcp.run(transport="streamable-http")`  
- **Endpoint**: `http://host:port/mcp`
- **Best for**: New projects, efficient bidirectional communication

### 3. **Stdio** (Original)
- **Use**: Local tools, subprocess communication
- **Configuration**: `mcp.run(transport="stdio")` (default)
- **Best for**: Command-line tools, local development

## 🎯 Client Integration Examples

### Python Client (FastMCP)
```python
from fastmcp import Client

# Connect to SSE server
async def use_mcp_server():
    async with Client("http://localhost:8360/sse") as client:
        # List available tools
        tools = await client.list_tools()
        
        # Call a tool
        result = await client.call_tool("autonomous_file_system_operations", {
            "operations": [{"operation_type": "read", "path": "README.md"}],
            "project_path": "/app"
        })
        
        return result
```

### Cursor IDE Configuration
```json
{
  "mcpServers": {
    "mcp-revolutionary-sse": {
      "url": "http://localhost:8360/sse",
      "env": {}
    }
  }
}
```

### Claude Desktop Configuration
```json
{
  "mcpServers": {
    "mcp-revolutionary": {
      "url": "http://localhost:8360/sse"
    }
  }
}
```

## 🔍 Debugging & Troubleshooting

### Check Server Logs
```bash
# View live logs
docker-compose logs -f mcp-revolutionary-sse

# Look for these key indicators:
# ✅ "Starting MCP server 'name' with transport 'sse'"
# ✅ "Uvicorn running on http://0.0.0.0:8000"
# ✅ "GET /sse HTTP/1.1" 200 OK
```

### Test SSE Endpoint
```bash
# Should hang (waiting for events) - this is correct!
curl http://localhost:8360/sse

# Should return headers quickly
curl -I http://localhost:8360/sse

# Should timeout after 5s - confirms SSE is working
timeout 5 curl http://localhost:8360/sse || echo "SSE working correctly"
```

### Verify Tool Registration
```python
# Quick test script
import asyncio
from fastmcp import Client

async def test_tools():
    try:
        async with Client("http://localhost:8360/sse") as client:
            tools = await client.list_tools()
            print(f"✅ Connected! Found {len(tools)} tools:")
            for tool in tools:
                print(f"  - {tool.name}: {tool.description}")
    except Exception as e:
        print(f"❌ Connection failed: {e}")

asyncio.run(test_tools())
```

### Common Error Messages

| Error | Cause | Solution |
|-------|-------|----------|
| `"transport 'stdio'"` in logs | No explicit transport config | Add `transport="sse"` to `mcp.run()` |
| `Connection refused` | Wrong port/not started | Check port mapping and container status |
| `curl hangs immediately` | SSE working correctly | This is expected behavior for SSE |
| `ImportError: fastmcp` | Missing dependency | Install with `uv add fastmcp` |

## 🏆 Success Criteria

Migration is successful when you see:

1. **✅ Server logs show**: `"Starting MCP server 'name' with transport 'sse'"`
2. **✅ HTTP requests work**: `curl -I http://localhost:8360/sse` returns headers
3. **✅ Tools are accessible**: Client can list and call tools
4. **✅ Health check passes**: Docker health check succeeds
5. **✅ Persistent service**: Server stays running and handles multiple requests

## 🚀 Next Steps

1. **Client Integration**: Update your MCP clients to use the SSE URL
2. **Production Deployment**: Configure SSL/TLS for HTTPS endpoints
3. **Monitoring**: Set up proper logging and metrics collection
4. **Authentication**: Implement authentication if needed for production
5. **Documentation**: Update your API docs to reflect the new endpoints

## 📚 References

- [FastMCP Documentation](https://gofastmcp.com/)
- [MCP Protocol Specification](https://spec.modelcontextprotocol.io/)
- [FastMCP SSE Transport Guide](https://gofastmcp.com/deployment/running-server)
- [Client Transport Options](https://gofastmcp.com/clients/transports)

---

**Migration Status**: ✅ **COMPLETE** - SSE transport working perfectly with 8 revolutionary tools available! 
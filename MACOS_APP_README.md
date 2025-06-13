# 📱 Knowledge Graph MCP Server - macOS App

Your Knowledge Graph MCP Server is now available as a beautiful macOS application with a dock icon! This makes it easy to see when your server is running and provides a native macOS experience.

## 🎯 Features

- **Dock Icon**: Beautiful gradient knowledge graph icon that appears in your dock
- **Native Notifications**: macOS notifications for server status updates
- **Easy Management**: Simple start/stop/status commands
- **Background Operation**: Runs quietly in the background
- **Automatic Startup**: Can be launched from Applications folder

## 🚀 Quick Start

### Option 1: Launch as macOS App (Recommended)
```bash
# Launch with dock icon and notifications
./launch_kg_server.sh app
```

### Option 2: Background Service
```bash
# Start server in background
./launch_kg_server.sh start

# Check status
./launch_kg_server.sh status

# Stop server
./launch_kg_server.sh stop
```

### Option 3: Direct Launch
```bash
# Double-click "KG MCP Server.app" in Finder
# Or from Applications folder
open "/Applications/KG MCP Server.app"
```

## 📋 Available Commands

| Command | Description |
|---------|-------------|
| `./launch_kg_server.sh app` | Launch as macOS app with dock icon |
| `./launch_kg_server.sh start` | Start server in background |
| `./launch_kg_server.sh stop` | Stop the server |
| `./launch_kg_server.sh restart` | Restart the server |
| `./launch_kg_server.sh status` | Check server status |
| `./launch_kg_server.sh logs` | View server logs |
| `./launch_kg_server.sh help` | Show help |

## 🔧 Building the App

If you need to rebuild the app (after code changes):

```bash
# Rebuild the entire app bundle
./build_app.sh
```

This will:
1. Compile the Rust binary in release mode
2. Copy it to the app bundle
3. Set proper permissions
4. Create a symlink in Applications

## 📊 Server Details

- **Endpoint**: `http://localhost:8360/sse`
- **Protocol**: Server-Sent Events (SSE)
- **Port**: 8360
- **Logs**: `~/.kg-mcp-server.log`
- **PID File**: `~/.kg-mcp-server.pid`

## 🔔 Notifications

When running as a macOS app, you'll receive notifications for:

- **Startup**: "Knowledge Graph MCP Server is starting..."
- **Ready**: "Server is ready and running on port 8360"
- **Shutdown**: "Server is shutting down..."

## 🎨 App Icon

The app features a beautiful gradient icon representing a knowledge graph network:
- Purple/blue gradient background
- Connected nodes representing knowledge entities
- Central hub with radiating connections
- "MCP" text at the bottom

## 📁 File Structure

```
KG MCP Server.app/
├── Contents/
│   ├── Info.plist              # App metadata
│   ├── MacOS/
│   │   └── kg-mcp-app          # Main executable
│   └── Resources/
│       ├── AppIcon.icns        # App icon
│       └── AppIcon.iconset/    # Icon source files
```

## 🔧 Configuration

The app uses the same configuration as the command-line version:
- `config.toml` - Main configuration file
- Environment variables for runtime settings
- Database stored in configured location

## 🐛 Troubleshooting

### App Won't Start
```bash
# Check if binary exists and is executable
ls -la "KG MCP Server.app/Contents/MacOS/kg-mcp-app"

# Rebuild if needed
./build_app.sh
```

### No Dock Icon
```bash
# Launch explicitly as app
./launch_kg_server.sh app

# Or open directly
open "KG MCP Server.app"
```

### Check Server Status
```bash
# View current status
./launch_kg_server.sh status

# View recent logs
./launch_kg_server.sh logs
```

### Port Already in Use
```bash
# Stop any existing server
./launch_kg_server.sh stop

# Check what's using port 8360
lsof -i :8360

# Start fresh
./launch_kg_server.sh start
```

## 🔗 Integration with Cursor

The server is designed to work with Cursor's MCP (Model Context Protocol). Once running, you can connect to it from Cursor using the SSE endpoint:

```json
{
  "mcpServers": {
    "kg-server": {
      "command": "curl",
      "args": ["-N", "http://localhost:8360/sse"]
    }
  }
}
```

## 🎯 Next Steps

1. **Launch the app**: `./launch_kg_server.sh app`
2. **Verify it's running**: Check your dock for the KG MCP Server icon
3. **Connect from Cursor**: Use the SSE endpoint in your MCP configuration
4. **Start building**: Begin creating your knowledge graph!

## 📝 Notes

- The app runs the same server code as the command-line version
- All data and configuration are shared between app and CLI modes
- The dock icon will remain visible while the server is running
- Notifications require macOS notification permissions (usually granted automatically)

Enjoy your new macOS Knowledge Graph MCP Server app! 🎉 
#!/bin/bash

# Knowledge Graph MCP Server Launcher
# Provides easy start/stop functionality with status checking

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_PATH="$SCRIPT_DIR/KG MCP Server.app"
BINARY_PATH="$APP_PATH/Contents/MacOS/kg-mcp-app"
PID_FILE="$HOME/.kg-mcp-server.pid"
LOG_FILE="$HOME/.kg-mcp-server.log"

show_help() {
    echo "🧠 Knowledge Graph MCP Server Launcher"
    echo ""
    echo "Usage: $0 [COMMAND]"
    echo ""
    echo "Commands:"
    echo "  start     Start the server (default)"
    echo "  stop      Stop the server"
    echo "  restart   Restart the server"
    echo "  status    Show server status"
    echo "  logs      Show server logs"
    echo "  app       Launch as macOS app (with dock icon)"
    echo "  help      Show this help"
    echo ""
    echo "Examples:"
    echo "  $0              # Start server in background"
    echo "  $0 app          # Launch as macOS app with dock icon"
    echo "  $0 status       # Check if server is running"
    echo "  $0 logs         # View server logs"
}

check_status() {
    if [ -f "$PID_FILE" ]; then
        PID=$(cat "$PID_FILE")
        if ps -p "$PID" > /dev/null 2>&1; then
            echo "✅ Server is running (PID: $PID)"
            echo "🌐 Endpoint: http://localhost:8360/sse"
            return 0
        else
            echo "❌ Server is not running (stale PID file)"
            rm -f "$PID_FILE"
            return 1
        fi
    else
        echo "❌ Server is not running"
        return 1
    fi
}

start_server() {
    if check_status > /dev/null 2>&1; then
        echo "⚠️  Server is already running"
        check_status
        return 1
    fi

    echo "🚀 Starting Knowledge Graph MCP Server..."
    
    # Start server in background
    nohup "$BINARY_PATH" > "$LOG_FILE" 2>&1 &
    PID=$!
    echo $PID > "$PID_FILE"
    
    # Wait a moment and check if it started successfully
    sleep 2
    if ps -p "$PID" > /dev/null 2>&1; then
        echo "✅ Server started successfully (PID: $PID)"
        echo "🌐 Endpoint: http://localhost:8360/sse"
        echo "📋 Logs: $LOG_FILE"
    else
        echo "❌ Failed to start server"
        rm -f "$PID_FILE"
        return 1
    fi
}

stop_server() {
    if [ -f "$PID_FILE" ]; then
        PID=$(cat "$PID_FILE")
        if ps -p "$PID" > /dev/null 2>&1; then
            echo "🛑 Stopping server (PID: $PID)..."
            kill "$PID"
            
            # Wait for graceful shutdown
            for i in {1..10}; do
                if ! ps -p "$PID" > /dev/null 2>&1; then
                    break
                fi
                sleep 1
            done
            
            # Force kill if still running
            if ps -p "$PID" > /dev/null 2>&1; then
                echo "⚡ Force stopping server..."
                kill -9 "$PID"
            fi
            
            rm -f "$PID_FILE"
            echo "✅ Server stopped"
        else
            echo "❌ Server is not running (stale PID file)"
            rm -f "$PID_FILE"
        fi
    else
        echo "❌ Server is not running"
    fi
}

launch_app() {
    echo "📱 Launching KG MCP Server as macOS app..."
    if [ -d "$APP_PATH" ]; then
        open "$APP_PATH"
        echo "✅ App launched! Check your dock for the KG MCP Server icon."
    else
        echo "❌ App bundle not found. Run './build_app.sh' first."
        return 1
    fi
}

show_logs() {
    if [ -f "$LOG_FILE" ]; then
        echo "📋 Server logs (last 50 lines):"
        echo "================================"
        tail -50 "$LOG_FILE"
    else
        echo "❌ No log file found"
    fi
}

# Main command handling
case "${1:-start}" in
    "start")
        start_server
        ;;
    "stop")
        stop_server
        ;;
    "restart")
        stop_server
        sleep 1
        start_server
        ;;
    "status")
        check_status
        ;;
    "logs")
        show_logs
        ;;
    "app")
        launch_app
        ;;
    "help"|"-h"|"--help")
        show_help
        ;;
    *)
        echo "❌ Unknown command: $1"
        echo ""
        show_help
        exit 1
        ;;
esac 
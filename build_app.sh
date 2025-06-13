#!/bin/bash

set -e

echo "🚀 Building Knowledge Graph MCP Server App..."

# Clean previous builds
echo "🧹 Cleaning previous builds..."
rm -rf "KG MCP Server.app/Contents/MacOS/kg-mcp-app"

# Build the Rust binary in release mode
echo "🔨 Building Rust binary..."
cargo build --release --bin kg-mcp-app

# Copy the binary to the app bundle
echo "📦 Setting up app bundle..."
cp target/release/kg-mcp-app "KG MCP Server.app/Contents/MacOS/"

# Make the binary executable
chmod +x "KG MCP Server.app/Contents/MacOS/kg-mcp-app"

# Set proper permissions for the app bundle
echo "🔐 Setting permissions..."
chmod -R 755 "KG MCP Server.app"

# Create a symlink in Applications for easy access (optional)
if [ ! -L "/Applications/KG MCP Server.app" ]; then
    echo "🔗 Creating symlink in Applications..."
    ln -sf "$(pwd)/KG MCP Server.app" "/Applications/KG MCP Server.app"
fi

echo "✅ Build complete!"
echo ""
echo "📱 Your KG MCP Server app is ready!"
echo "   Location: $(pwd)/KG MCP Server.app"
echo "   Symlink:  /Applications/KG MCP Server.app"
echo ""
echo "🚀 To run the app:"
echo "   • Double-click the app in Finder"
echo "   • Or run: open 'KG MCP Server.app'"
echo "   • Or run from terminal: ./KG\\ MCP\\ Server.app/Contents/MacOS/kg-mcp-app"
echo ""
echo "📊 The server will be available at: http://localhost:8360/sse"
echo "🔔 You'll see notifications when the server starts and is ready" 
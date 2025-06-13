#!/bin/bash

# Wrapper script to launch KG MCP Server without dock bouncing
# This temporarily disables dock bouncing system-wide, launches the app, then re-enables it

echo "🚀 Launching KG MCP Server without dock bouncing..."

# Check current dock bouncing setting
CURRENT_BOUNCE=$(defaults read com.apple.dock no-bouncing 2>/dev/null || echo "false")

# Disable dock bouncing temporarily
echo "🔇 Temporarily disabling dock bouncing..."
defaults write com.apple.dock no-bouncing -bool TRUE
killall Dock

# Wait for dock to restart
sleep 2

# Launch the app
echo "📱 Launching KG MCP Server..."
open "/Applications/KG MCP Server.app"

# Wait for app to fully launch
sleep 5

# Restore original dock bouncing setting
echo "🔊 Restoring dock bouncing setting..."
if [ "$CURRENT_BOUNCE" = "true" ]; then
    defaults write com.apple.dock no-bouncing -bool TRUE
else
    defaults delete com.apple.dock no-bouncing 2>/dev/null || true
fi
killall Dock

echo "✅ KG MCP Server launched successfully!"
echo "📊 Server available at: http://localhost:8360/sse" 
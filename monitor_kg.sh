#!/bin/bash

# KG System Health Monitor
# Usage: ./monitor_kg.sh [interval_seconds]

INTERVAL=${1:-30}  # Default 30 seconds
LOG_FILE="kg_monitor.log"
PID_FILE="kg_server.pid"

echo "🔍 Starting KG System Health Monitor (interval: ${INTERVAL}s)"
echo "📝 Logging to: $LOG_FILE"

# Function to log with timestamp
log_with_timestamp() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1" | tee -a "$LOG_FILE"
}

# Function to check memory usage
check_memory() {
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS
        local memory_pressure=$(memory_pressure | grep "System-wide memory free percentage" | awk '{print $5}' | tr -d '%')
        local swap_used=$(sysctl vm.swapusage | grep "used" | awk '{print $7}' | tr -d 'M')
        echo "Memory free: ${memory_pressure}%, Swap used: ${swap_used}M"
    else
        # Linux
        local mem_info=$(free -m | grep "Mem:")
        local used=$(echo $mem_info | awk '{print $3}')
        local total=$(echo $mem_info | awk '{print $2}')
        local percent=$((used * 100 / total))
        echo "Memory used: ${percent}% (${used}M/${total}M)"
    fi
}

# Function to check if KG process is running
check_kg_process() {
    if [ -f "$PID_FILE" ]; then
        local pid=$(cat "$PID_FILE")
        if ps -p "$pid" > /dev/null 2>&1; then
            echo "✅ KG process running (PID: $pid)"
            return 0
        else
            echo "❌ KG process not found (stale PID file)"
            rm -f "$PID_FILE"
            return 1
        fi
    else
        # Try to find by process name
        local kg_pid=$(pgrep -f "kg-mcp-server\|kg_mcp_server" | head -1)
        if [ -n "$kg_pid" ]; then
            echo "✅ KG process found (PID: $kg_pid)"
            echo "$kg_pid" > "$PID_FILE"
            return 0
        else
            echo "❌ KG process not running"
            return 1
        fi
    fi
}

# Function to check database file
check_database() {
    local db_file="knowledge_graph.db"
    if [ -f "$db_file" ]; then
        local size=$(du -h "$db_file" | cut -f1)
        echo "📊 Database size: $size"
        
        # Check if database is locked
        if sqlite3 "$db_file" "SELECT 1;" > /dev/null 2>&1; then
            echo "✅ Database accessible"
        else
            echo "⚠️  Database locked or corrupted"
        fi
    else
        echo "❌ Database file not found"
    fi
}

# Function to check log files for errors
check_recent_errors() {
    local error_count=0
    
    # Check for recent panics or crashes in system logs
    if [[ "$OSTYPE" == "darwin"* ]]; then
        error_count=$(log show --last 5m --predicate 'process CONTAINS "kg"' 2>/dev/null | grep -i "panic\|crash\|fatal" | wc -l)
    else
        error_count=$(journalctl --since "5 minutes ago" | grep -i "kg.*\(panic\|crash\|fatal\)" | wc -l)
    fi
    
    if [ "$error_count" -gt 0 ]; then
        echo "⚠️  Found $error_count recent error(s) in logs"
    else
        echo "✅ No recent errors found"
    fi
}

# Function to test KG server health
test_kg_health() {
    local port=${KG_PORT:-8360}
    
    if command -v curl > /dev/null 2>&1; then
        local response=$(curl -s -w "%{http_code}" -o /dev/null "http://localhost:$port/health" 2>/dev/null)
        if [ "$response" = "200" ]; then
            echo "✅ KG server responding on port $port"
        else
            echo "⚠️  KG server not responding (HTTP: $response)"
        fi
    else
        echo "ℹ️  curl not available, skipping HTTP health check"
    fi
}

# Main monitoring loop
log_with_timestamp "🚀 KG Health Monitor started"

while true; do
    echo "----------------------------------------"
    log_with_timestamp "🔍 Health Check"
    
    # Check if process is running
    if check_kg_process; then
        # Process is running, check other metrics
        echo "$(check_memory)"
        check_database
        check_recent_errors
        test_kg_health
        
        # Check for high memory usage
        if [[ "$OSTYPE" == "darwin"* ]]; then
            local memory_free=$(memory_pressure | grep "System-wide memory free percentage" | awk '{print $5}' | tr -d '%')
            if [ "$memory_free" -lt 10 ]; then
                log_with_timestamp "⚠️  LOW MEMORY WARNING: Only ${memory_free}% free"
            fi
        fi
        
    else
        log_with_timestamp "❌ KG PROCESS DOWN - Attempting restart..."
        
        # Try to restart the KG server
        if [ -f "launch_kg_server.sh" ]; then
            ./launch_kg_server.sh &
            log_with_timestamp "🔄 Restart command issued"
        else
            log_with_timestamp "❌ No restart script found (launch_kg_server.sh)"
        fi
    fi
    
    sleep "$INTERVAL"
done 
# MCP Tool Output Verbosity System

## Overview

The Knowledge Graph MCP Server now includes a configurable verbosity system to optimize context window usage while preserving essential information for memory retrieval. This addresses the issue of verbose tool outputs consuming too much context space.

## Problem Solved

**Before**: MCP tool outputs were always verbose, including all metadata, timestamps, and detailed information, which could quickly consume context windows when dealing with large result sets.

**After**: Three verbosity levels provide optimal output size for different use cases while maintaining all necessary information for effective memory retrieval.

## Verbosity Levels

### 1. Summary Mode (`verbosity: "summary"`)
- **Size Reduction**: ~50-70% smaller output
- **Use Case**: Large result sets, quick overviews
- **Content**: Only essential identifiers and names

**Example Node Output**:
```json
{
  "uuid": "123e4567-e89b-12d3-a456-426614174000",
  "name": "Alice",
  "type": "Person"
}
```

### 2. Compact Mode (`verbosity: "compact"`) - **DEFAULT**
- **Size Reduction**: ~30-40% smaller output
- **Use Case**: Standard memory retrieval operations
- **Content**: Essential information for memory retrieval

**Example Node Output**:
```json
{
  "uuid": "123e4567-e89b-12d3-a456-426614174000",
  "name": "Alice",
  "type": "Person",
  "summary": "Software engineer at TechCorp",
  "group_id": "project-alpha"
}
```

### 3. Full Mode (`verbosity: "full"`)
- **Size Reduction**: No reduction (complete details)
- **Use Case**: Detailed analysis, debugging, complete context needed
- **Content**: All metadata, timestamps, and detailed information

**Example Node Output**:
```json
{
  "uuid": "123e4567-e89b-12d3-a456-426614174000",
  "name": "Alice",
  "node_type": "Person",
  "summary": "Software engineer at TechCorp",
  "created_at": "2024-01-15T10:30:00Z",
  "updated_at": "2024-01-15T10:30:00Z",
  "group_id": "project-alpha",
  "metadata": {
    "source": "employee_directory",
    "confidence": 0.95
  }
}
```

## Usage Examples

### Adding Memory with Verbosity

```python
# Summary mode - minimal output
await mcp.call_tool("mcp_kg-mcp-server_add_memory", {
    "name": "Meeting Notes",
    "episode_body": "Alice discussed the new AI project with Bob...",
    "verbosity": "summary"
})

# Compact mode (default) - balanced output
await mcp.call_tool("mcp_kg-mcp-server_add_memory", {
    "name": "Meeting Notes",
    "episode_body": "Alice discussed the new AI project with Bob...",
    "verbosity": "compact"  # or omit for default
})

# Full mode - complete details
await mcp.call_tool("mcp_kg-mcp-server_add_memory", {
    "name": "Meeting Notes",
    "episode_body": "Alice discussed the new AI project with Bob...",
    "verbosity": "full"
})
```

### Searching Memory with Verbosity

```python
# Search with summary output for large result sets
await mcp.call_tool("mcp_kg-mcp-server_search_memory", {
    "operation": "nodes",
    "query": "software engineer",
    "max_results": 50,
    "verbosity": "summary"
})

# Search with compact output (default)
await mcp.call_tool("mcp_kg-mcp-server_search_memory", {
    "operation": "facts",
    "query": "collaboration",
    "max_results": 10,
    "verbosity": "compact"
})

# Search with full details for analysis
await mcp.call_tool("mcp_kg-mcp-server_search_memory", {
    "operation": "similar_concepts",
    "query": "AI technology",
    "verbosity": "full"
})
```

## Response Format Differences

### Add Memory Response

**Summary Mode**:
```json
{
  "success": true,
  "episode_id": "uuid-here",
  "entities": 3,
  "relationships": 2
}
```

**Compact Mode**:
```json
{
  "success": true,
  "episode_id": "uuid-here",
  "name": "Meeting Notes",
  "entities_created": 3,
  "relationships_created": 2
}
```

**Full Mode**:
```json
{
  "success": true,
  "message": "Episode added successfully",
  "episode_id": "uuid-here",
  "group_id": "project-alpha",
  "name": "Meeting Notes",
  "entities_created": 3,
  "relationships_created": 2,
  "entities": [
    {"uuid": "...", "name": "Alice", "type": "Person"},
    {"uuid": "...", "name": "Bob", "type": "Person"},
    {"uuid": "...", "name": "TechCorp", "type": "Organization"}
  ],
  "relationships": [
    {"uuid": "...", "type": "works_at", "summary": "Alice works at TechCorp"},
    {"uuid": "...", "type": "collaborates_with", "summary": "Alice collaborates with Bob"}
  ]
}
```

### Search Response

**Summary Mode**:
```json
{
  "success": true,
  "op": "nodes",
  "count": 3,
  "query": "Alice",
  "results": [
    {"uuid": "...", "name": "Alice", "type": "Person"},
    {"uuid": "...", "name": "Alice Smith", "type": "Person"},
    {"uuid": "...", "name": "Alice Johnson", "type": "Person"}
  ]
}
```

**Compact Mode**:
```json
{
  "success": true,
  "operation": "nodes",
  "total_found": 3,
  "query": "Alice",
  "results": [
    {
      "uuid": "...",
      "name": "Alice",
      "type": "Person",
      "summary": "Software engineer at TechCorp",
      "group_id": "project-alpha"
    }
  ]
}
```

## Performance Benefits

### Context Window Savings

| Verbosity | Typical Size | Use Case | Context Savings |
|-----------|-------------|----------|-----------------|
| Summary   | 50-100 chars/item | Large result sets | 50-70% |
| Compact   | 100-200 chars/item | Standard operations | 30-40% |
| Full      | 200-500 chars/item | Detailed analysis | 0% |

### Real-World Example

For a search returning 20 nodes:

- **Summary**: ~2KB total output
- **Compact**: ~4KB total output  
- **Full**: ~10KB total output

This means you can fit **5x more results** in the same context window using summary mode.

## Best Practices

### When to Use Each Mode

**Summary Mode**:
- Large result sets (>20 items)
- Quick entity/relationship discovery
- Context window optimization
- Batch processing operations

**Compact Mode** (Default):
- Standard memory retrieval
- Most interactive operations
- Balanced information needs
- General-purpose usage

**Full Mode**:
- Detailed analysis and debugging
- When you need complete metadata
- Timestamp-sensitive operations
- Data export/migration tasks

### Optimization Strategies

1. **Start with Summary**: For exploratory queries, use summary mode to get an overview
2. **Drill Down with Compact**: Use compact mode to get essential details for specific items
3. **Full Details When Needed**: Only use full mode when you need complete information

## Implementation Details

### Code Changes

The verbosity system is implemented in `src/mcp/handlers.rs` with:

1. **OutputVerbosity enum**: Defines the three verbosity levels
2. **Format methods**: Specialized formatting for nodes, edges, and episodes
3. **Response builders**: Consistent response structure across verbosity levels
4. **Parameter parsing**: Automatic detection of verbosity from tool parameters

### Tool Schema Updates

All MCP tools now accept an optional `verbosity` parameter:

```json
{
  "verbosity": {
    "type": "string",
    "enum": ["summary", "compact", "full"],
    "default": "compact",
    "description": "Output verbosity level"
  }
}
```

## Migration Guide

### Existing Code

No changes required! The system defaults to "compact" mode, which provides a good balance of information and size reduction.

### Optimizing for Context Windows

```python
# Before: Always verbose output
result = await mcp.call_tool("mcp_kg-mcp-server_search_memory", {
    "operation": "nodes",
    "query": "employees",
    "max_results": 50
})

# After: Optimized for large result sets
result = await mcp.call_tool("mcp_kg-mcp-server_search_memory", {
    "operation": "nodes",
    "query": "employees", 
    "max_results": 50,
    "verbosity": "summary"  # 50-70% size reduction
})
```

## Conclusion

The verbosity system provides a powerful way to optimize MCP tool outputs for different use cases while preserving all essential information for effective memory retrieval. By choosing the appropriate verbosity level, you can:

- **Save significant context window space**
- **Improve response times**
- **Maintain information quality**
- **Scale to larger result sets**

The default "compact" mode provides an excellent balance for most use cases, while "summary" and "full" modes offer optimization for specific scenarios. 
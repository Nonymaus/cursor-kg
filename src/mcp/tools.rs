use serde_json::{json, Value};

/// Define consolidated MCP tools for the knowledge graph server
/// Reduced from 12 tools to 4 powerful, batch-capable tools
pub fn get_tool_definitions() -> Value {
    json!({
        "tools": [
            {
                "name": "mcp_kg-mcp-server_add_memory",
                "description": "Add an episode to memory. This is the primary way to add information to the graph.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Name of the episode"
                        },
                        "episode_body": {
                            "type": "string",
                            "description": "The content of the episode to persist to memory. When source='json', this must be a properly escaped JSON string, not a raw Python dictionary. The JSON data will be automatically processed to extract entities and relationships."
                        },
                        "group_id": {
                            "type": "string",
                            "description": "A unique ID for this graph. If not provided, uses the default group_id from CLI or a generated one."
                        },
                        "source": {
                            "type": "string",
                            "enum": ["text", "json", "message"],
                            "default": "text",
                            "description": "Source type, must be one of: 'text' for plain text content (default), 'json' for structured data, 'message' for conversation-style content"
                        },
                        "source_description": {
                            "type": "string",
                            "default": "",
                            "description": "Description of the source"
                        },
                        "uuid": {
                            "type": "string",
                            "description": "Optional UUID for the episode"
                        },
                        "verbosity": {
                            "type": "string",
                            "enum": ["summary", "compact", "full"],
                            "default": "compact",
                            "description": "Output verbosity level: 'summary' for minimal output, 'compact' for essential info (default), 'full' for complete details"
                        }
                    },
                    "required": ["name", "episode_body"]
                }
            },
            {
                "name": "mcp_kg-mcp-server_search_memory",
                "description": "Comprehensive search and retrieval operations for the knowledge graph. Supports batch queries and multiple search types.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["nodes", "facts", "episodes", "similar_concepts", "batch"],
                            "description": "Type of search operation: 'nodes' for node summaries, 'facts' for relationships, 'episodes' for recent episodes, 'similar_concepts' for semantic similarity, 'batch' for multiple queries"
                        },
                        "query": {
                            "type": "string",
                            "description": "Primary search query (required for nodes, facts, similar_concepts)"
                        },
                        "queries": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Multiple queries for batch operations"
                        },
                        "group_ids": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Optional list of group IDs to filter results"
                        },
                        "group_id": {
                            "type": "string",
                            "description": "Single group ID for episodes operation"
                        },
                        "max_results": {
                            "type": "integer",
                            "default": 10,
                            "description": "Maximum number of results to return per query (default: 10)"
                        },
                        "center_node_uuid": {
                            "type": "string",
                            "description": "Optional UUID of a node to center the search around"
                        },
                        "entity_filter": {
                            "type": "string",
                            "description": "Optional entity type to filter results"
                        },
                        "similarity_threshold": {
                            "type": "number",
                            "default": 0.7,
                            "description": "Minimum similarity score threshold for similar_concepts (0.0-1.0, default: 0.7)"
                        },
                        "last_n": {
                            "type": "integer",
                            "default": 10,
                            "description": "Number of recent episodes to retrieve (for episodes operation)"
                        },
                        "verbosity": {
                            "type": "string",
                            "enum": ["summary", "compact", "full"],
                            "default": "compact",
                            "description": "Output verbosity level: 'summary' for minimal output, 'compact' for essential info (default), 'full' for complete details"
                        }
                    },
                    "required": ["operation"]
                }
            },
            {
                "name": "mcp_kg-mcp-server_analyze_patterns",
                "description": "Advanced analytics and pattern analysis for the knowledge graph. Includes clustering, temporal analysis, and relationship patterns.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "analysis_type": {
                            "type": "string",
                            "enum": ["relationships", "clusters", "temporal", "centrality", "semantic_clusters"],
                            "description": "Type of pattern analysis: 'relationships' for frequent patterns, 'clusters' for entity clustering, 'temporal' for time-based patterns, 'centrality' for important nodes, 'semantic_clusters' for concept grouping"
                        },
                        "group_ids": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Optional list of group IDs to filter analysis"
                        },
                        "max_results": {
                            "type": "integer",
                            "default": 20,
                            "description": "Maximum number of patterns/clusters to return (default: 20)"
                        },
                        "time_range_days": {
                            "type": "integer",
                            "default": 30,
                            "description": "Number of days to analyze for temporal analysis (default: 30)"
                        },
                        "time_granularity": {
                            "type": "string",
                            "enum": ["hour", "day", "week", "month"],
                            "default": "day",
                            "description": "Time granularity for temporal analysis (default: day)"
                        },
                        "concept_filter": {
                            "type": "string",
                            "description": "Optional concept to filter temporal analysis"
                        },
                        "cluster_method": {
                            "type": "string",
                            "enum": ["kmeans", "hierarchical", "dbscan"],
                            "default": "kmeans",
                            "description": "Clustering algorithm for semantic_clusters (default: kmeans)"
                        },
                        "num_clusters": {
                            "type": "integer",
                            "default": 5,
                            "description": "Number of clusters for kmeans (default: 5)"
                        },
                        "min_cluster_size": {
                            "type": "integer",
                            "default": 3,
                            "description": "Minimum size for clusters (default: 3)"
                        },
                        "verbosity": {
                            "type": "string",
                            "enum": ["summary", "compact", "full"],
                            "default": "compact",
                            "description": "Output verbosity level: 'summary' for minimal output, 'compact' for essential info (default), 'full' for complete details"
                        }
                    },
                    "required": ["analysis_type"]
                }
            },
            {
                "name": "mcp_kg-mcp-server_manage_graph",
                "description": "Graph management operations including deletion, retrieval, and maintenance. Supports batch operations for efficiency.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["get_entity_edge", "delete_entity_edge", "delete_episode", "delete_batch", "clear_graph", "get_episodes"],
                            "description": "Management operation: 'get_entity_edge' to retrieve edge, 'delete_entity_edge' to remove edge, 'delete_episode' to remove episode, 'delete_batch' for multiple deletions, 'clear_graph' to reset, 'get_episodes' to retrieve recent episodes"
                        },
                        "uuid": {
                            "type": "string",
                            "description": "UUID of the entity edge or episode (for single operations)"
                        },
                        "uuids": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Multiple UUIDs for batch delete operations"
                        },
                        "delete_type": {
                            "type": "string",
                            "enum": ["episodes", "edges", "mixed"],
                            "default": "mixed",
                            "description": "Type of items to delete in batch operation"
                        },
                        "confirm": {
                            "type": "boolean",
                            "description": "Required confirmation for destructive operations (clear_graph, delete_batch)"
                        },
                        "group_id": {
                            "type": "string",
                            "description": "Group ID for get_episodes operation"
                        },
                        "last_n": {
                            "type": "integer",
                            "default": 10,
                            "description": "Number of recent episodes to retrieve"
                        },
                        "verbosity": {
                            "type": "string",
                            "enum": ["summary", "compact", "full"],
                            "default": "compact",
                            "description": "Output verbosity level: 'summary' for minimal output, 'compact' for essential info (default), 'full' for complete details"
                        }
                    },
                    "required": ["operation"]
                }
            },
            {
                "name": "mcp_kg-mcp-server_index_codebase",
                "description": "Index and analyze large codebases with parallel processing, multi-language support, and intelligent dependency mapping. Optimized for cursor agent mode with large context windows.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["index", "reindex", "status", "search_code", "get_dependencies", "analyze_structure"],
                            "description": "Indexing operation: 'index' for initial indexing, 'reindex' for full re-indexing, 'status' for indexing progress, 'search_code' for code search, 'get_dependencies' for dependency mapping, 'analyze_structure' for codebase analysis"
                        },
                        "path": {
                            "type": "string",
                            "description": "Root path of the codebase to index (required for index/reindex operations)"
                        },
                        "languages": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Optional list of programming languages to focus on (e.g., ['rust', 'python', 'javascript']). If not specified, auto-detects all supported languages."
                        },
                        "include_patterns": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Glob patterns for files to include (e.g., ['*.rs', '*.py', '*.js'])"
                        },
                        "exclude_patterns": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Glob patterns for files to exclude (e.g., ['target/*', 'node_modules/*', '*.log'])"
                        },
                        "max_file_size": {
                            "type": "integer",
                            "default": 1048576,
                            "description": "Maximum file size to index in bytes (default: 1MB)"
                        },
                        "parallel_workers": {
                            "type": "integer",
                            "default": 4,
                            "description": "Number of parallel workers for indexing (default: 4)"
                        },
                        "incremental": {
                            "type": "boolean",
                            "default": true,
                            "description": "Enable incremental indexing (only process changed files)"
                        },
                        "extract_dependencies": {
                            "type": "boolean",
                            "default": true,
                            "description": "Extract and map dependencies between files"
                        },
                        "extract_symbols": {
                            "type": "boolean",
                            "default": true,
                            "description": "Extract function/class/variable symbols"
                        },
                        "query": {
                            "type": "string",
                            "description": "Search query for search_code operation"
                        },
                        "file_path": {
                            "type": "string",
                            "description": "Specific file path for dependency analysis"
                        },
                        "symbol_type": {
                            "type": "string",
                            "enum": ["function", "class", "variable", "import", "all"],
                            "default": "all",
                            "description": "Type of symbols to search for in search_code operation"
                        },
                        "context_lines": {
                            "type": "integer",
                            "default": 5,
                            "description": "Number of context lines to include around search results"
                        },
                        "group_id": {
                            "type": "string",
                            "description": "Group ID to associate indexed content with"
                        },
                        "max_results": {
                            "type": "integer",
                            "default": 50,
                            "description": "Maximum number of results to return"
                        },
                        "verbosity": {
                            "type": "string",
                            "enum": ["summary", "compact", "full"],
                            "default": "compact",
                            "description": "Output verbosity level: 'summary' for minimal output, 'compact' for essential info (default), 'full' for complete details"
                        }
                    },
                    "required": ["operation"]
                }
            }
        ]
    })
} 
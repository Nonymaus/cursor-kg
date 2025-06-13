# Enhanced KG Features Guide

This document provides a comprehensive overview of the enhanced features implemented in the KG (Knowledge Graph) system to address stability, context window management, large codebase indexing, and hallucination prevention.

## Table of Contents

1. [Stability Enhancements](#stability-enhancements)
2. [Context Window Management](#context-window-management)
3. [Codebase Indexing](#codebase-indexing)
4. [Hallucination Prevention](#hallucination-prevention)
5. [Configuration](#configuration)
6. [Usage Examples](#usage-examples)
7. [Performance Optimizations](#performance-optimizations)
8. [Monitoring and Health Checks](#monitoring-and-health-checks)

## Stability Enhancements

### Circuit Breaker Pattern

The system now implements a comprehensive circuit breaker pattern to prevent cascading failures:

**Location**: `src/stability/circuit_breaker.rs`

**Features**:
- Automatic failure detection and recovery
- Configurable failure thresholds and timeouts
- Half-open state for gradual recovery testing
- Metrics collection for monitoring

**Usage**:
```rust
use crate::stability::{CircuitBreaker, CircuitBreakerConfig};

let config = CircuitBreakerConfig {
    failure_threshold: 5,
    timeout_duration: Duration::from_secs(60),
    half_open_max_calls: 3,
};

let breaker = CircuitBreaker::new("database".to_string(), config);

// Protected operation
let result = breaker.call(|| async {
    // Your database operation here
    database.query("SELECT * FROM nodes").await
}).await;
```

### Fault Tolerance

**Location**: `src/stability/fault_tolerance.rs`

**Features**:
- Retry mechanisms with exponential backoff
- Graceful degradation under load
- Resource exhaustion protection
- Automatic recovery strategies

### Registry Management

**Location**: `src/stability/registry.rs`

**Features**:
- Centralized circuit breaker management
- Health status monitoring
- Automatic cleanup of unused breakers
- Performance metrics aggregation

## Context Window Management

### Intelligent Chunking

**Location**: `src/context/window_manager.rs`

**Features**:
- Adaptive chunking that preserves code structure
- Priority-based content selection
- Large context window support (128k+ tokens)
- Access pattern learning for optimization

**Key Components**:

1. **ContextChunk**: Represents a piece of content with metadata
2. **ChunkType**: Categorizes content (Code, Documentation, Configuration, etc.)
3. **ContextWindowManager**: Main interface for content management

**Usage**:
```rust
use crate::context::{ContextWindowManager, ContextWindowConfig, ChunkType};

let config = ContextWindowConfig {
    max_tokens: 128_000,
    overlap_tokens: 200,
    adaptive_chunking: true,
    preserve_code_blocks: true,
    ..Default::default()
};

let manager = ContextWindowManager::new(config, embedding_engine);

// Add content with intelligent chunking
let chunk_ids = manager.add_content(
    &file_content,
    Some("src/main.rs".to_string()),
    ChunkType::Code,
).await?;

// Get optimal context for a query
let context = manager.get_context_for_query(
    "How does authentication work?",
    Some(32_000), // Max tokens
).await?;
```

### Features

1. **Adaptive Chunking**: Automatically adjusts chunk boundaries based on content type
2. **Priority System**: Ranks content by importance and relevance
3. **Access Tracking**: Learns from usage patterns to improve selection
4. **Token Management**: Precise token counting and limit enforcement
5. **Caching**: Intelligent caching with LRU eviction

## Codebase Indexing

### Comprehensive Language Support

**Location**: `src/indexing/codebase_indexer.rs`

**Supported Languages**:
- Rust, Python, JavaScript, TypeScript
- Java, C++, C, Go, Ruby, PHP
- C#, Swift, Kotlin, Scala, Clojure
- Haskell, OCaml, Elm, Dart
- Markdown, JSON, YAML, TOML, XML, HTML, CSS

### Features

1. **Parallel Processing**: Concurrent file processing with configurable limits
2. **Incremental Indexing**: Only processes changed files
3. **Dependency Mapping**: Tracks imports, includes, and references
4. **Cross-File Analysis**: Analyzes relationships between files
5. **Metadata Extraction**: Complexity scoring, LOC counting, language detection

**Usage**:
```rust
use crate::indexing::{CodebaseIndexer, CodebaseIndexerConfig};

let config = CodebaseIndexerConfig {
    max_concurrent_files: 10,
    supported_extensions: vec!["rs".to_string(), "py".to_string()],
    enable_dependency_mapping: true,
    enable_cross_file_analysis: true,
    ..Default::default()
};

let indexer = CodebaseIndexer::new(
    config,
    context_manager,
    entity_extractor,
    relationship_extractor,
    embedding_engine,
);

// Index entire codebase
let stats = indexer.index_codebase(Path::new("./src")).await?;
println!("Processed {} files, found {} nodes", stats.processed_files, stats.total_nodes);
```

### Dependency Analysis

The indexer automatically detects and maps dependencies:

- **Rust**: `use` statements
- **Python**: `import` and `from` statements
- **JavaScript/TypeScript**: `import` and `require()` calls
- **Generic**: Pattern-based detection for other languages

## Hallucination Prevention

### Multi-Layer Validation

**Location**: `src/validation/hallucination_detector.rs`

**Validation Layers**:

1. **Fact Verification**: Cross-references against known facts
2. **Cross-Reference Validation**: Checks consistency with existing knowledge
3. **Temporal Consistency**: Validates time-based relationships
4. **Contradiction Detection**: Identifies logical inconsistencies
5. **Source Credibility**: Assesses information reliability
6. **Uncertainty Quantification**: Measures confidence levels

**Usage**:
```rust
use crate::validation::{HallucinationDetector, HallucinationDetectorConfig, ValidationContext};

let config = HallucinationDetectorConfig {
    confidence_threshold: 0.7,
    fact_verification_enabled: true,
    contradiction_detection: true,
    uncertainty_quantification: true,
    ..Default::default()
};

let detector = HallucinationDetector::new(config, embedding_engine);

let context = ValidationContext {
    source_type: SourceType::UserGenerated,
    timestamp: Utc::now(),
    ..Default::default()
};

let result = detector.validate_content(
    "The new authentication system uses OAuth 2.0",
    &context,
).await?;

if result.is_valid {
    println!("Content validated with confidence: {:.2}", result.confidence_score);
} else {
    println!("Content failed validation: {:?}", result.contradictions);
}
```

### Validation Features

1. **Pattern-Based Detection**: Identifies common contradiction patterns
2. **Semantic Analysis**: Uses embeddings for deeper understanding
3. **Evidence Tracking**: Maintains supporting evidence for claims
4. **Confidence Scoring**: Provides numerical confidence assessments
5. **Recommendation Engine**: Suggests improvements for low-confidence content

## Configuration

### Enhanced Configuration File

**Location**: `enhanced_config.toml`

The enhanced configuration provides comprehensive settings for all new features:

```toml
[stability]
enable_circuit_breakers = true
default_failure_threshold = 5
default_timeout_seconds = 60
health_check_interval_seconds = 30

[context_window]
max_tokens = 128000
overlap_tokens = 200
priority_boost = 1.5
adaptive_chunking = true
preserve_code_blocks = true

[indexing]
max_concurrent_files = 10
enable_incremental = true
enable_dependency_mapping = true
enable_cross_file_analysis = true
max_file_size_mb = 1

[validation]
confidence_threshold = 0.7
enable_fact_verification = true
enable_contradiction_detection = true
enable_uncertainty_quantification = true

[performance]
enable_parallel_processing = true
max_parallel_operations = 8
memory_limit_mb = 512
cache_size_mb = 128
```

## Usage Examples

### Basic Setup

```rust
use kg_mcp_server::{
    stability::CircuitBreakerRegistry,
    context::ContextWindowManager,
    indexing::CodebaseIndexer,
    validation::HallucinationDetector,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize enhanced components
    let registry = CircuitBreakerRegistry::new();
    let context_manager = ContextWindowManager::new(
        ContextWindowConfig::default(),
        None, // embedding engine
    );
    
    // Index a codebase
    let indexer = CodebaseIndexer::new(
        CodebaseIndexerConfig::default(),
        Arc::new(context_manager),
        Arc::new(entity_extractor),
        Arc::new(relationship_extractor),
        None,
    );
    
    let stats = indexer.index_codebase(Path::new("./src")).await?;
    println!("Indexing complete: {:?}", stats);
    
    Ok(())
}
```

### Advanced Query Processing

```rust
// Get context for a complex query
let context = context_manager.get_context_for_query(
    "How does the authentication system handle OAuth tokens?",
    Some(64_000),
).await?;

// Validate the response before returning
let validation_result = hallucination_detector.validate_content(
    &response_text,
    &validation_context,
).await?;

if validation_result.confidence_score >= 0.8 {
    // High confidence - return response
    Ok(response_text)
} else {
    // Low confidence - request more context or flag for review
    Err(anyhow::anyhow!("Response confidence too low: {:.2}", validation_result.confidence_score))
}
```

## Performance Optimizations

### Parallel Processing

All major operations now support parallel processing:

1. **File Indexing**: Process multiple files concurrently
2. **Context Chunking**: Parallel chunk generation and analysis
3. **Validation**: Concurrent validation across multiple layers
4. **Circuit Breaker Operations**: Non-blocking health checks

### Memory Management

Enhanced memory management features:

1. **Intelligent Caching**: LRU caches with configurable sizes
2. **Memory Pools**: Reusable object pools for common types
3. **Garbage Collection**: Automatic cleanup of unused resources
4. **Memory Monitoring**: Real-time memory usage tracking

### Caching Strategies

1. **Multi-Level Caching**: L1 (in-memory) and L2 (persistent) caches
2. **Smart Eviction**: Priority-based cache eviction
3. **Cache Warming**: Preload frequently accessed content
4. **Cache Coherency**: Automatic invalidation on updates

## Monitoring and Health Checks

### Health Monitoring Script

**Location**: `monitor_kg.sh`

The enhanced monitoring script provides:

1. **Circuit Breaker Status**: Real-time breaker state monitoring
2. **Memory Usage**: Detailed memory consumption tracking
3. **Performance Metrics**: Response times and throughput
4. **Error Rates**: Failure detection and alerting
5. **Automatic Restart**: Intelligent restart on critical failures

**Usage**:
```bash
# Start monitoring with enhanced features
./monitor_kg.sh --enhanced --interval 30 --auto-restart
```

### Metrics Collection

The system collects comprehensive metrics:

1. **Stability Metrics**: Circuit breaker states, failure rates
2. **Performance Metrics**: Response times, throughput, memory usage
3. **Quality Metrics**: Validation confidence scores, error rates
4. **Usage Metrics**: Access patterns, cache hit rates

### Alerting

Configurable alerting for:

1. **High Error Rates**: Automatic alerts when error rates exceed thresholds
2. **Memory Pressure**: Warnings when memory usage is high
3. **Performance Degradation**: Alerts for slow response times
4. **Validation Failures**: Notifications for low-confidence content

## Best Practices

### Stability

1. **Use Circuit Breakers**: Wrap all external calls with circuit breakers
2. **Configure Timeouts**: Set appropriate timeouts for all operations
3. **Monitor Health**: Regularly check system health metrics
4. **Graceful Degradation**: Design fallback mechanisms for failures

### Context Management

1. **Optimize Chunk Size**: Balance between context and performance
2. **Use Priority Scoring**: Implement domain-specific priority algorithms
3. **Monitor Access Patterns**: Analyze and optimize based on usage
4. **Cache Strategically**: Cache frequently accessed content

### Indexing

1. **Incremental Updates**: Use incremental indexing for large codebases
2. **Parallel Processing**: Configure appropriate concurrency levels
3. **Filter Effectively**: Use exclude patterns to skip irrelevant files
4. **Monitor Progress**: Track indexing progress and performance

### Validation

1. **Set Appropriate Thresholds**: Configure confidence thresholds based on use case
2. **Use Multiple Layers**: Enable all validation layers for critical content
3. **Track Evidence**: Maintain supporting evidence for all claims
4. **Regular Updates**: Keep fact databases current and accurate

## Troubleshooting

### Common Issues

1. **Memory Pressure**: Reduce cache sizes or enable more aggressive garbage collection
2. **Slow Indexing**: Increase parallel processing or exclude large files
3. **Low Validation Confidence**: Improve fact databases or adjust thresholds
4. **Circuit Breaker Trips**: Check underlying service health and adjust thresholds

### Debug Mode

Enable debug logging for detailed troubleshooting:

```toml
[logging]
level = "debug"
enable_performance_logging = true
enable_validation_logging = true
```

## Future Enhancements

Planned improvements include:

1. **Advanced ML Models**: Integration with larger language models for validation
2. **Distributed Processing**: Support for distributed indexing and processing
3. **Real-time Updates**: Live indexing of code changes
4. **Advanced Analytics**: Deeper insights into code relationships and patterns
5. **API Extensions**: Enhanced APIs for external integrations

## Conclusion

The enhanced KG system provides enterprise-grade stability, intelligent context management, comprehensive codebase indexing, and robust hallucination prevention. These features work together to create a reliable, scalable, and accurate knowledge graph system suitable for production use.

For additional support or questions, refer to the individual module documentation or contact the development team. 
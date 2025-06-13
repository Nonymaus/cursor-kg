pub mod optimization;

// Re-export the main memory optimization components
pub use optimization::{
    MemoryOptimizer, 
    MemoryConfig, 
    MemoryStats, 
    GcResult, 
    OptimizationResult
}; 
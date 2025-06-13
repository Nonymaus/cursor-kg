pub mod codebase_indexer;
pub mod language_support;
pub mod dependency_mapper;

// Re-export main types
pub use codebase_indexer::{
    CodebaseIndexer,
    CodebaseIndexerConfig,
    FileIndexResult,
    IndexingStats,
    Dependency,
    FileMetadata,
    ProgrammingLanguage,
    IndexingConfig,
    IndexingResult,
    CodeSearchResult,
    FileDependency,
    CodebaseAnalysis,
};
pub use language_support::{LanguageDetector, SupportedLanguage};
pub use dependency_mapper::{DependencyMapper, DependencyType}; 
pub mod hybrid_search;
pub mod text_search;
pub mod vector_search;

// Re-export the main engines for easier access
pub use hybrid_search::{HybridSearchEngine, FusionAlgorithm, HybridSearchOptions, UserFeedback, SearchContext, PreviousQuery};
pub use text_search::{TextSearchEngine, BoostFactors, SearchOptions};
pub use vector_search::{VectorSearchEngine, DistanceMetric};

use anyhow::Result;
use uuid::Uuid;

use crate::graph::{KGNode, KGEdge, Episode, SearchResult};

/// Search engine interface
pub trait SearchEngine {
    async fn search_nodes(&self, query: &str, limit: usize) -> Result<Vec<KGNode>>;
    async fn search_episodes(&self, query: &str, limit: usize) -> Result<Vec<Episode>>;
    async fn hybrid_search(&self, query: &str, limit: usize) -> Result<SearchResult>;
} 
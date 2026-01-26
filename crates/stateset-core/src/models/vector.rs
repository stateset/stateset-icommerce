//! Vector search models for semantic search capabilities

use serde::{Deserialize, Serialize};
use std::fmt;

/// Result of a vector similarity search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchResult<T> {
    /// The matched entity
    pub entity: T,
    /// Distance from query vector (lower = more similar)
    pub distance: f32,
    /// Similarity score (1.0 - normalized distance, higher = more similar)
    pub score: f32,
}

impl<T> VectorSearchResult<T> {
    /// Create a new search result from distance
    pub fn new(entity: T, distance: f32) -> Self {
        // Normalize distance to score (assuming cosine distance in [0, 2])
        let score = 1.0 - (distance / 2.0).min(1.0);
        Self { entity, distance, score }
    }
}

/// Entity types that support vector embeddings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Product,
    Customer,
    Order,
    InventoryItem,
}

impl EntityType {
    /// Get the table name for this entity type's embeddings
    pub fn embedding_table(&self) -> &'static str {
        match self {
            EntityType::Product => "product_embeddings",
            EntityType::Customer => "customer_embeddings",
            EntityType::Order => "order_embeddings",
            EntityType::InventoryItem => "inventory_embeddings",
        }
    }

    /// Get the ID column name for this entity type
    pub fn id_column(&self) -> &'static str {
        match self {
            EntityType::Product => "product_id",
            EntityType::Customer => "customer_id",
            EntityType::Order => "order_id",
            EntityType::InventoryItem => "item_id",
        }
    }

    /// Get the source table name for this entity type
    pub fn source_table(&self) -> &'static str {
        match self {
            EntityType::Product => "products",
            EntityType::Customer => "customers",
            EntityType::Order => "orders",
            EntityType::InventoryItem => "inventory_items",
        }
    }
}

impl fmt::Display for EntityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EntityType::Product => write!(f, "product"),
            EntityType::Customer => write!(f, "customer"),
            EntityType::Order => write!(f, "order"),
            EntityType::InventoryItem => write!(f, "inventory_item"),
        }
    }
}

impl std::str::FromStr for EntityType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "product" | "products" => Ok(EntityType::Product),
            "customer" | "customers" => Ok(EntityType::Customer),
            "order" | "orders" => Ok(EntityType::Order),
            "inventory_item" | "inventory" | "inventory_items" => Ok(EntityType::InventoryItem),
            _ => Err(format!("Unknown entity type: {}", s)),
        }
    }
}

/// Metadata about a stored embedding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingMetadata {
    /// Type of entity
    pub entity_type: EntityType,
    /// Entity ID
    pub entity_id: String,
    /// Model used to generate embedding
    pub model: String,
    /// Hash of text used to generate embedding (for cache invalidation)
    pub text_hash: String,
    /// When the embedding was created
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Request to generate and store an embedding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    /// Type of entity
    pub entity_type: EntityType,
    /// Entity ID
    pub entity_id: String,
    /// Text to embed
    pub text: String,
}

/// Configuration for the embedding service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// API key for embedding service
    pub api_key: String,
    /// Model to use (default: text-embedding-3-small)
    pub model: String,
    /// Number of dimensions (default: 1536 for text-embedding-3-small)
    pub dimensions: usize,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "text-embedding-3-small".to_string(),
            dimensions: 1536,
        }
    }
}

/// Vector search query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchQuery {
    /// The query text (will be embedded)
    pub query: String,
    /// Entity type to search
    pub entity_type: EntityType,
    /// Maximum results to return
    pub limit: usize,
    /// Minimum similarity score (0.0 - 1.0)
    pub min_score: Option<f32>,
}

impl Default for VectorSearchQuery {
    fn default() -> Self {
        Self {
            query: String::new(),
            entity_type: EntityType::Product,
            limit: 10,
            min_score: None,
        }
    }
}

/// Statistics about vector embeddings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingStats {
    /// Total number of embeddings by entity type
    pub counts: std::collections::HashMap<EntityType, u64>,
    /// Model used
    pub model: String,
    /// Embedding dimensions
    pub dimensions: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_type_parsing() {
        assert_eq!("product".parse::<EntityType>().unwrap(), EntityType::Product);
        assert_eq!("products".parse::<EntityType>().unwrap(), EntityType::Product);
        assert_eq!("customer".parse::<EntityType>().unwrap(), EntityType::Customer);
        assert_eq!("inventory".parse::<EntityType>().unwrap(), EntityType::InventoryItem);
    }

    #[test]
    fn test_vector_search_result_score() {
        let result = VectorSearchResult::new("test", 0.0);
        assert_eq!(result.score, 1.0);

        let result = VectorSearchResult::new("test", 2.0);
        assert_eq!(result.score, 0.0);

        let result = VectorSearchResult::new("test", 1.0);
        assert_eq!(result.score, 0.5);
    }
}

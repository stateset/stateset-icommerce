//! Vector search operations for semantic similarity search
//!
//! This module provides hybrid semantic + lexical search using embeddings
//! and BM25 (SQLite FTS5) for improved relevance.
//! If FTS5 isn't available, it transparently falls back to embedding-only search.
//!
//! # Example
//!
//! ```rust,ignore
//! use stateset_embedded::Commerce;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Initialize vector search with OpenAI API key
//! let vector = commerce.vector(std::env::var("OPENAI_API_KEY")?);
//!
//! // Index a product for search
//! vector.index_product(&product)?;
//!
//! // Search for similar products
//! let results = vector.search_products("wireless bluetooth headphones", 10)?;
//! for result in results {
//!     println!("{}: {} (score: {:.2})", result.entity.name, result.entity.id, result.score);
//! }
//! ```

use stateset_core::{
    Customer, EmbeddingService, EmbeddingStats, EntityType, InventoryItem, Order, Product, Result,
    VectorRepository, VectorSearchResult,
};
use stateset_db::sqlite::SqliteVectorRepository;
use std::sync::Arc;

/// Vector search operations
pub struct Vector {
    repo: SqliteVectorRepository,
    embedding_service: Arc<EmbeddingService>,
}

impl Vector {
    /// Create a new Vector instance
    pub(crate) fn new(repo: SqliteVectorRepository, api_key: String) -> Self {
        Self {
            repo,
            embedding_service: Arc::new(EmbeddingService::new(api_key)),
        }
    }

    /// Create with custom embedding model
    pub fn with_model(
        repo: SqliteVectorRepository,
        api_key: String,
        model: String,
        dimensions: usize,
    ) -> Self {
        Self {
            repo,
            embedding_service: Arc::new(EmbeddingService::with_model(api_key, model, dimensions)),
        }
    }

    // ========================================================================
    // Product Operations
    // ========================================================================

    /// Index a product for vector search
    pub fn index_product(&self, product: &Product) -> Result<()> {
        let text = EmbeddingService::product_text(product);
        let result = self.embedding_service.embed(&text)?;
        self.repo.store_embedding(
            EntityType::Product,
            &product.id.to_string(),
            &result.embedding,
            &result.text_hash,
            self.embedding_service.model(),
        )
    }

    /// Search for products similar to the query text
    pub fn search_products(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<Product>>> {
        let result = self.embedding_service.embed(query)?;
        self.repo
            .search_products_hybrid(&result.embedding, query, limit)
    }

    /// Search products using a pre-computed embedding
    pub fn search_products_by_embedding(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<Product>>> {
        self.repo.search_products(embedding, limit)
    }

    /// Remove a product from the vector index
    pub fn unindex_product(&self, product_id: &str) -> Result<()> {
        self.repo.delete_embedding(EntityType::Product, product_id)
    }

    /// Index multiple products in batch
    pub fn index_products(&self, products: &[Product]) -> Result<usize> {
        let mut indexed = 0;

        // Process in batches of 100 for API efficiency
        for chunk in products.chunks(100) {
            let texts: Vec<String> = chunk.iter().map(EmbeddingService::product_text).collect();

            let results = self.embedding_service.embed_batch(&texts)?;

            for (product, result) in chunk.iter().zip(results.iter()) {
                self.repo.store_embedding(
                    EntityType::Product,
                    &product.id.to_string(),
                    &result.embedding,
                    &result.text_hash,
                    self.embedding_service.model(),
                )?;
                indexed += 1;
            }
        }

        Ok(indexed)
    }

    // ========================================================================
    // Customer Operations
    // ========================================================================

    /// Index a customer for vector search
    pub fn index_customer(&self, customer: &Customer) -> Result<()> {
        let text = EmbeddingService::customer_text(customer);
        let result = self.embedding_service.embed(&text)?;
        self.repo.store_embedding(
            EntityType::Customer,
            &customer.id.to_string(),
            &result.embedding,
            &result.text_hash,
            self.embedding_service.model(),
        )
    }

    /// Search for customers similar to the query text
    pub fn search_customers(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<Customer>>> {
        let result = self.embedding_service.embed(query)?;
        self.repo
            .search_customers_hybrid(&result.embedding, query, limit)
    }

    /// Remove a customer from the vector index
    pub fn unindex_customer(&self, customer_id: &str) -> Result<()> {
        self.repo
            .delete_embedding(EntityType::Customer, customer_id)
    }

    /// Index multiple customers in batch
    pub fn index_customers(&self, customers: &[Customer]) -> Result<usize> {
        let mut indexed = 0;

        for chunk in customers.chunks(100) {
            let texts: Vec<String> = chunk.iter().map(EmbeddingService::customer_text).collect();

            let results = self.embedding_service.embed_batch(&texts)?;

            for (customer, result) in chunk.iter().zip(results.iter()) {
                self.repo.store_embedding(
                    EntityType::Customer,
                    &customer.id.to_string(),
                    &result.embedding,
                    &result.text_hash,
                    self.embedding_service.model(),
                )?;
                indexed += 1;
            }
        }

        Ok(indexed)
    }

    // ========================================================================
    // Order Operations
    // ========================================================================

    /// Index an order for vector search
    pub fn index_order(&self, order: &Order) -> Result<()> {
        let text = EmbeddingService::order_text(order);
        let result = self.embedding_service.embed(&text)?;
        self.repo.store_embedding(
            EntityType::Order,
            &order.id.to_string(),
            &result.embedding,
            &result.text_hash,
            self.embedding_service.model(),
        )
    }

    /// Search for orders similar to the query text
    pub fn search_orders(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<Order>>> {
        let result = self.embedding_service.embed(query)?;
        self.repo
            .search_orders_hybrid(&result.embedding, query, limit)
    }

    /// Remove an order from the vector index
    pub fn unindex_order(&self, order_id: &str) -> Result<()> {
        self.repo.delete_embedding(EntityType::Order, order_id)
    }

    /// Index multiple orders in batch
    pub fn index_orders(&self, orders: &[Order]) -> Result<usize> {
        let mut indexed = 0;

        for chunk in orders.chunks(100) {
            let texts: Vec<String> = chunk.iter().map(EmbeddingService::order_text).collect();

            let results = self.embedding_service.embed_batch(&texts)?;

            for (order, result) in chunk.iter().zip(results.iter()) {
                self.repo.store_embedding(
                    EntityType::Order,
                    &order.id.to_string(),
                    &result.embedding,
                    &result.text_hash,
                    self.embedding_service.model(),
                )?;
                indexed += 1;
            }
        }

        Ok(indexed)
    }

    // ========================================================================
    // Inventory Operations
    // ========================================================================

    /// Index an inventory item for vector search
    pub fn index_inventory_item(&self, item: &InventoryItem) -> Result<()> {
        let text = EmbeddingService::inventory_item_text(item);
        let result = self.embedding_service.embed(&text)?;
        self.repo.store_embedding(
            EntityType::InventoryItem,
            &item.id.to_string(),
            &result.embedding,
            &result.text_hash,
            self.embedding_service.model(),
        )
    }

    /// Search for inventory items similar to the query text
    pub fn search_inventory(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<InventoryItem>>> {
        let result = self.embedding_service.embed(query)?;
        self.repo
            .search_inventory_hybrid(&result.embedding, query, limit)
    }

    /// Remove an inventory item from the vector index
    pub fn unindex_inventory_item(&self, item_id: &str) -> Result<()> {
        self.repo
            .delete_embedding(EntityType::InventoryItem, item_id)
    }

    /// Index multiple inventory items in batch
    pub fn index_inventory_items(&self, items: &[InventoryItem]) -> Result<usize> {
        let mut indexed = 0;

        for chunk in items.chunks(100) {
            let texts: Vec<String> = chunk
                .iter()
                .map(EmbeddingService::inventory_item_text)
                .collect();

            let results = self.embedding_service.embed_batch(&texts)?;

            for (item, result) in chunk.iter().zip(results.iter()) {
                self.repo.store_embedding(
                    EntityType::InventoryItem,
                    &item.id.to_string(),
                    &result.embedding,
                    &result.text_hash,
                    self.embedding_service.model(),
                )?;
                indexed += 1;
            }
        }

        Ok(indexed)
    }

    // ========================================================================
    // Utility Operations
    // ========================================================================

    /// Get embedding statistics
    pub fn stats(&self) -> Result<EmbeddingStats> {
        self.repo.get_stats()
    }

    /// Check if an entity has been indexed
    pub fn is_indexed(&self, entity_type: EntityType, entity_id: &str) -> Result<bool> {
        self.repo.has_embedding(entity_type, entity_id)
    }

    /// Clear all embeddings for an entity type
    pub fn clear(&self, entity_type: EntityType) -> Result<u64> {
        self.repo.clear_embeddings(entity_type)
    }

    /// Clear all embeddings
    pub fn clear_all(&self) -> Result<u64> {
        let mut total = 0;
        total += self.repo.clear_embeddings(EntityType::Product)?;
        total += self.repo.clear_embeddings(EntityType::Customer)?;
        total += self.repo.clear_embeddings(EntityType::Order)?;
        total += self.repo.clear_embeddings(EntityType::InventoryItem)?;
        Ok(total)
    }

    /// Generate an embedding for text without storing it
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let result = self.embedding_service.embed(text)?;
        Ok(result.embedding)
    }
}

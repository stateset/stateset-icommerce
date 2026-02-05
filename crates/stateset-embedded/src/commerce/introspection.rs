use super::Commerce;

use stateset_db::Database;

#[cfg(all(feature = "sqlite", feature = "vector"))]
use stateset_core::CommerceError;

#[cfg(all(feature = "sqlite", feature = "vector"))]
use crate::Vector;

impl Commerce {
    /// Get the underlying database (for advanced use cases).
    pub fn database(&self) -> &dyn Database {
        &*self.db
    }

    /// Access vector search operations.
    ///
    /// Requires the `vector` feature and an OpenAI API key for embedding generation.
    ///
    /// # Arguments
    ///
    /// * `api_key` - OpenAI API key for generating embeddings
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    /// let api_key = std::env::var("OPENAI_API_KEY")?;
    ///
    /// let vector = commerce.vector(api_key)?;
    ///
    /// // Index products for search
    /// for product in commerce.products().list(Default::default())? {
    ///     vector.index_product(&product)?;
    /// }
    ///
    /// // Semantic search
    /// let results = vector.search_products("wireless bluetooth headphones", 10)?;
    /// for result in results {
    ///     println!("{}: {} (score: {:.2})", result.entity.name, result.entity.id, result.score);
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(all(feature = "sqlite", feature = "vector"))]
    pub fn vector(&self, api_key: String) -> Result<Vector, CommerceError> {
        match &self.sqlite_db {
            Some(db) => Ok(Vector::new(db.vector(), api_key)),
            None => Err(CommerceError::NotPermitted(
                "Vector search requires SQLite database. Use Commerce::new() instead of with_database() or with_postgres().".to_string()
            )),
        }
    }
}

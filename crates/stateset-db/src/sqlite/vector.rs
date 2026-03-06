//! SQLite vector search repository implementation
//!
//! Uses pure Rust cosine similarity computation for vector search.
//! Embeddings are stored as BLOBs in regular SQLite tables.

use super::{
    map_db_error, parse_datetime_row, parse_decimal_row, parse_enum_row, parse_json_opt_row,
    parse_json_row, parse_uuid_row,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use stateset_core::{
    CommerceError, Customer, EmbeddingMetadata, EmbeddingStats, EntityType, InventoryItem, Order,
    Product, Result, VectorRepository, VectorSearchResult,
};
use std::collections::HashMap;

/// SQLite implementation of `VectorRepository` using pure Rust cosine similarity.
#[derive(Debug)]
pub struct SqliteVectorRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteVectorRepository {
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    /// Convert f32 slice to blob for storage
    fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
        embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
    }

    /// Convert blob back to f32 vec
    fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
        blob.chunks_exact(4)
            .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
            .collect()
    }

    /// Compute cosine similarity between two vectors
    /// Returns a value between -1 and 1 (1 = identical, 0 = orthogonal, -1 = opposite)
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let mut dot_product = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for i in 0..a.len() {
            dot_product += a[i] * b[i];
            norm_a += a[i] * a[i];
            norm_b += b[i] * b[i];
        }

        let denominator = norm_a.sqrt() * norm_b.sqrt();
        if denominator == 0.0 { 0.0 } else { dot_product / denominator }
    }

    /// Convert cosine similarity to distance (lower = more similar)
    fn similarity_to_distance(similarity: f32) -> f32 {
        1.0 - similarity
    }

    /// Reciprocal Rank Fusion constant (higher = more smoothing)
    const RRF_K: f32 = 60.0;

    fn rrf_score(rank: usize) -> f32 {
        1.0 / (Self::RRF_K + rank as f32)
    }

    fn merge_rrf<T, F>(
        vector_results: Vec<VectorSearchResult<T>>,
        bm25_results: Vec<(T, f32)>,
        limit: usize,
        id_fn: F,
    ) -> Vec<VectorSearchResult<T>>
    where
        T: Clone,
        F: Fn(&T) -> String,
    {
        if bm25_results.is_empty() {
            return vector_results;
        }

        #[derive(Clone)]
        struct Entry<T> {
            entity: T,
            rrf: f32,
            vector_distance: Option<f32>,
        }

        let mut entries: HashMap<String, Entry<T>> = HashMap::new();

        for (idx, result) in vector_results.iter().enumerate() {
            let id = id_fn(&result.entity);
            let entry = entries.entry(id).or_insert(Entry {
                entity: result.entity.clone(),
                rrf: 0.0,
                vector_distance: None,
            });
            entry.rrf += Self::rrf_score(idx + 1);
            entry.vector_distance = Some(result.distance);
        }

        for (idx, (entity, _bm25)) in bm25_results.iter().enumerate() {
            let id = id_fn(entity);
            let entry = entries.entry(id).or_insert(Entry {
                entity: entity.clone(),
                rrf: 0.0,
                vector_distance: None,
            });
            entry.rrf += Self::rrf_score(idx + 1);
        }

        let max_rrf = entries.values().fold(0.0f32, |max, entry| max.max(entry.rrf));

        let mut merged: Vec<VectorSearchResult<T>> = entries
            .into_values()
            .map(|entry| {
                let score = if max_rrf > 0.0 { entry.rrf / max_rrf } else { 0.0 };
                let distance = 2.0 * (1.0 - score);
                VectorSearchResult { entity: entry.entity, distance, score }
            })
            .collect();

        merged.sort_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal).then_with(|| {
                a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal)
            })
        });
        merged.truncate(limit);
        merged
    }

    fn row_to_product(row: &rusqlite::Row<'_>) -> rusqlite::Result<Product> {
        let attributes_json: String = row.get("attributes")?;
        let seo_json: Option<String> = row.get("seo")?;

        Ok(Product {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "product", "id")?.into(),
            name: row.get("name")?,
            slug: row.get("slug")?,
            description: row.get("description")?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "product", "status")?,
            product_type: parse_enum_row(
                &row.get::<_, String>("product_type")?,
                "product",
                "product_type",
            )?,
            attributes: parse_json_row(&attributes_json, "product", "attributes")?,
            seo: parse_json_opt_row(seo_json, "product", "seo")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "product",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "product",
                "updated_at",
            )?,
        })
    }

    fn row_to_customer(row: &rusqlite::Row<'_>) -> rusqlite::Result<Customer> {
        let tags_json: String = row.get("tags")?;
        let metadata_json: Option<String> = row.get("metadata")?;

        Ok(Customer {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "customer", "id")?.into(),
            email: row.get("email")?,
            first_name: row.get("first_name")?,
            last_name: row.get("last_name")?,
            phone: row.get("phone")?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "customer", "status")?,
            accepts_marketing: row.get::<_, i32>("accepts_marketing")? != 0,
            email_verified: row.get::<_, i32>("email_verified")? != 0,
            tags: parse_json_row(&tags_json, "customer", "tags")?,
            metadata: parse_json_opt_row(metadata_json, "customer", "metadata")?,
            default_shipping_address_id: row
                .get::<_, Option<String>>("default_shipping_address_id")?
                .and_then(|s| s.parse().ok()),
            default_billing_address_id: row
                .get::<_, Option<String>>("default_billing_address_id")?
                .and_then(|s| s.parse().ok()),
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "customer",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "customer",
                "updated_at",
            )?,
        })
    }

    fn row_to_order(row: &rusqlite::Row<'_>) -> rusqlite::Result<Order> {
        let shipping_address_json: Option<String> = row.get("shipping_address")?;
        let billing_address_json: Option<String> = row.get("billing_address")?;

        Ok(Order {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "order", "id")?.into(),
            order_number: row.get("order_number")?,
            customer_id: parse_uuid_row(
                &row.get::<_, String>("customer_id")?,
                "order",
                "customer_id",
            )?
            .into(),
            status: parse_enum_row(&row.get::<_, String>("status")?, "order", "status")?,
            order_date: parse_datetime_row(
                &row.get::<_, String>("order_date")?,
                "order",
                "order_date",
            )?,
            total_amount: parse_decimal_row(
                &row.get::<_, String>("total_amount")?,
                "order",
                "total_amount",
            )?,
            currency: row.get("currency")?,
            payment_status: parse_enum_row(
                &row.get::<_, String>("payment_status")?,
                "order",
                "payment_status",
            )?,
            fulfillment_status: parse_enum_row(
                &row.get::<_, String>("fulfillment_status")?,
                "order",
                "fulfillment_status",
            )?,
            payment_method: row.get("payment_method")?,
            shipping_method: row.get("shipping_method")?,
            tracking_number: row.get("tracking_number")?,
            notes: row.get("notes")?,
            shipping_address: parse_json_opt_row(
                shipping_address_json,
                "order",
                "shipping_address",
            )?,
            billing_address: parse_json_opt_row(billing_address_json, "order", "billing_address")?,
            items: Vec::new(), // Items not loaded in vector search results
            version: row.get::<_, Option<i32>>("version")?.unwrap_or(0),
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "order",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "order",
                "updated_at",
            )?,
        })
    }

    fn row_to_inventory_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<InventoryItem> {
        Ok(InventoryItem {
            id: row.get("id")?,
            sku: row.get("sku")?,
            name: row.get("name")?,
            description: row.get("description")?,
            unit_of_measure: row.get("unit_of_measure")?,
            is_active: row.get::<_, i32>("is_active")? != 0,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "inventory_item",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "inventory_item",
                "updated_at",
            )?,
        })
    }

    fn infer_dimensions(conn: &rusqlite::Connection) -> Result<Option<usize>> {
        for entity_type in [
            EntityType::Product,
            EntityType::Customer,
            EntityType::Order,
            EntityType::InventoryItem,
        ] {
            let table = entity_type.embedding_table();
            let length: Option<i64> = conn
                .query_row(&format!("SELECT length(embedding) FROM {} LIMIT 1", table), [], |row| {
                    row.get(0)
                })
                .optional()
                .map_err(map_db_error)?;

            if let Some(len) = length {
                if len > 0 {
                    return Ok(Some((len as usize) / 4));
                }
            }
        }

        Ok(None)
    }

    fn search_products_vector_only(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<Product>>> {
        let conn = self.conn()?;

        let mut stmt = conn
            .prepare(
                "SELECT p.*, pe.embedding
                 FROM product_embeddings pe
                 JOIN products p ON p.id = pe.product_id",
            )
            .map_err(map_db_error)?;

        let mut results: Vec<VectorSearchResult<Product>> = stmt
            .query_map([], |row| {
                let product = Self::row_to_product(row)?;
                let embedding_blob: Vec<u8> = row.get("embedding")?;
                let stored_embedding = Self::blob_to_embedding(&embedding_blob);
                let similarity = Self::cosine_similarity(embedding, &stored_embedding);
                let distance = Self::similarity_to_distance(similarity);
                Ok(VectorSearchResult::new(product, distance))
            })
            .map_err(map_db_error)?
            .filter_map(|r| r.ok())
            .collect();

        results.sort_by(|a, b| {
            a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        Ok(results)
    }

    fn search_customers_vector_only(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<Customer>>> {
        let conn = self.conn()?;

        let mut stmt = conn
            .prepare(
                "SELECT c.*, ce.embedding
                 FROM customer_embeddings ce
                 JOIN customers c ON c.id = ce.customer_id",
            )
            .map_err(map_db_error)?;

        let mut results: Vec<VectorSearchResult<Customer>> = stmt
            .query_map([], |row| {
                let customer = Self::row_to_customer(row)?;
                let embedding_blob: Vec<u8> = row.get("embedding")?;
                let stored_embedding = Self::blob_to_embedding(&embedding_blob);
                let similarity = Self::cosine_similarity(embedding, &stored_embedding);
                let distance = Self::similarity_to_distance(similarity);
                Ok(VectorSearchResult::new(customer, distance))
            })
            .map_err(map_db_error)?
            .filter_map(|r| r.ok())
            .collect();

        results.sort_by(|a, b| {
            a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        Ok(results)
    }

    fn search_orders_vector_only(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<Order>>> {
        let conn = self.conn()?;

        let mut stmt = conn
            .prepare(
                "SELECT o.*, oe.embedding
                 FROM order_embeddings oe
                 JOIN orders o ON o.id = oe.order_id",
            )
            .map_err(map_db_error)?;

        let mut results: Vec<VectorSearchResult<Order>> = stmt
            .query_map([], |row| {
                let order = Self::row_to_order(row)?;
                let embedding_blob: Vec<u8> = row.get("embedding")?;
                let stored_embedding = Self::blob_to_embedding(&embedding_blob);
                let similarity = Self::cosine_similarity(embedding, &stored_embedding);
                let distance = Self::similarity_to_distance(similarity);
                Ok(VectorSearchResult::new(order, distance))
            })
            .map_err(map_db_error)?
            .filter_map(|r| r.ok())
            .collect();

        results.sort_by(|a, b| {
            a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        Ok(results)
    }

    fn search_inventory_vector_only(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<InventoryItem>>> {
        let conn = self.conn()?;

        let mut stmt = conn
            .prepare(
                "SELECT i.*, ie.embedding
                 FROM inventory_embeddings ie
                 JOIN inventory_items i ON i.id = ie.item_id",
            )
            .map_err(map_db_error)?;

        let mut results: Vec<VectorSearchResult<InventoryItem>> = stmt
            .query_map([], |row| {
                let item = Self::row_to_inventory_item(row)?;
                let embedding_blob: Vec<u8> = row.get("embedding")?;
                let stored_embedding = Self::blob_to_embedding(&embedding_blob);
                let similarity = Self::cosine_similarity(embedding, &stored_embedding);
                let distance = Self::similarity_to_distance(similarity);
                Ok(VectorSearchResult::new(item, distance))
            })
            .map_err(map_db_error)?
            .filter_map(|r| r.ok())
            .collect();

        results.sort_by(|a, b| {
            a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        Ok(results)
    }

    fn search_products_bm25(&self, query: &str, limit: usize) -> Result<Vec<(Product, f32)>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT p.*, bm25(product_fts) AS bm25_score
                 FROM product_fts
                 JOIN products p ON p.id = product_fts.entity_id
                 WHERE product_fts MATCH ?
                 ORDER BY bm25_score
                 LIMIT ?",
            )
            .map_err(map_db_error)?;

        let results = stmt
            .query_map(rusqlite::params![query, limit as i64], |row| {
                let product = Self::row_to_product(row)?;
                let score: f64 = row.get("bm25_score")?;
                Ok((product, score as f32))
            })
            .map_err(map_db_error)?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    fn search_customers_bm25(&self, query: &str, limit: usize) -> Result<Vec<(Customer, f32)>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT c.*, bm25(customer_fts) AS bm25_score
                 FROM customer_fts
                 JOIN customers c ON c.id = customer_fts.entity_id
                 WHERE customer_fts MATCH ?
                 ORDER BY bm25_score
                 LIMIT ?",
            )
            .map_err(map_db_error)?;

        let results = stmt
            .query_map(rusqlite::params![query, limit as i64], |row| {
                let customer = Self::row_to_customer(row)?;
                let score: f64 = row.get("bm25_score")?;
                Ok((customer, score as f32))
            })
            .map_err(map_db_error)?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    fn search_orders_bm25(&self, query: &str, limit: usize) -> Result<Vec<(Order, f32)>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT o.*, bm25(order_fts) AS bm25_score
                 FROM order_fts
                 JOIN orders o ON o.id = order_fts.entity_id
                 WHERE order_fts MATCH ?
                 ORDER BY bm25_score
                 LIMIT ?",
            )
            .map_err(map_db_error)?;

        let results = stmt
            .query_map(rusqlite::params![query, limit as i64], |row| {
                let order = Self::row_to_order(row)?;
                let score: f64 = row.get("bm25_score")?;
                Ok((order, score as f32))
            })
            .map_err(map_db_error)?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    fn search_inventory_bm25(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(InventoryItem, f32)>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT i.*, bm25(inventory_fts) AS bm25_score
                 FROM inventory_fts
                 JOIN inventory_items i ON i.id = CAST(inventory_fts.entity_id AS INTEGER)
                 WHERE inventory_fts MATCH ?
                 ORDER BY bm25_score
                 LIMIT ?",
            )
            .map_err(map_db_error)?;

        let results = stmt
            .query_map(rusqlite::params![query, limit as i64], |row| {
                let item = Self::row_to_inventory_item(row)?;
                let score: f64 = row.get("bm25_score")?;
                Ok((item, score as f32))
            })
            .map_err(map_db_error)?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    pub fn search_products_hybrid(
        &self,
        embedding: &[f32],
        query: &str,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<Product>>> {
        let vector_results = self.search_products_vector_only(embedding, limit)?;
        if query.trim().is_empty() {
            return Ok(vector_results);
        }

        let bm25_results = match self.search_products_bm25(query, limit) {
            Ok(results) => results,
            Err(_) => return Ok(vector_results),
        };

        Ok(Self::merge_rrf(vector_results, bm25_results, limit, |product| product.id.to_string()))
    }

    pub fn search_customers_hybrid(
        &self,
        embedding: &[f32],
        query: &str,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<Customer>>> {
        let vector_results = self.search_customers_vector_only(embedding, limit)?;
        if query.trim().is_empty() {
            return Ok(vector_results);
        }

        let bm25_results = match self.search_customers_bm25(query, limit) {
            Ok(results) => results,
            Err(_) => return Ok(vector_results),
        };

        Ok(Self::merge_rrf(vector_results, bm25_results, limit, |customer| customer.id.to_string()))
    }

    pub fn search_orders_hybrid(
        &self,
        embedding: &[f32],
        query: &str,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<Order>>> {
        let vector_results = self.search_orders_vector_only(embedding, limit)?;
        if query.trim().is_empty() {
            return Ok(vector_results);
        }

        let bm25_results = match self.search_orders_bm25(query, limit) {
            Ok(results) => results,
            Err(_) => return Ok(vector_results),
        };

        Ok(Self::merge_rrf(vector_results, bm25_results, limit, |order| order.id.to_string()))
    }

    pub fn search_inventory_hybrid(
        &self,
        embedding: &[f32],
        query: &str,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<InventoryItem>>> {
        let vector_results = self.search_inventory_vector_only(embedding, limit)?;
        if query.trim().is_empty() {
            return Ok(vector_results);
        }

        let bm25_results = match self.search_inventory_bm25(query, limit) {
            Ok(results) => results,
            Err(_) => return Ok(vector_results),
        };

        Ok(Self::merge_rrf(vector_results, bm25_results, limit, |item| item.id.to_string()))
    }
}

impl VectorRepository for SqliteVectorRepository {
    fn store_embedding(
        &self,
        entity_type: EntityType,
        entity_id: &str,
        embedding: &[f32],
        text_hash: &str,
        model: &str,
    ) -> Result<()> {
        let conn = self.conn()?;
        let embedding_blob = Self::embedding_to_blob(embedding);
        let now = Utc::now();

        // Insert or replace into the appropriate embedding table
        let table = entity_type.embedding_table();
        let id_col = entity_type.id_column();

        conn.execute(
            &format!(
                "INSERT OR REPLACE INTO {} ({}, embedding, created_at, updated_at) VALUES (?, ?, ?, ?)",
                table, id_col
            ),
            rusqlite::params![entity_id, embedding_blob, now.to_rfc3339(), now.to_rfc3339()],
        )
        .map_err(map_db_error)?;

        // Update metadata
        conn.execute(
            "INSERT OR REPLACE INTO embedding_metadata (entity_type, entity_id, model, text_hash, created_at)
             VALUES (?, ?, ?, ?, ?)",
            rusqlite::params![
                entity_type.to_string(),
                entity_id,
                model,
                text_hash,
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        Ok(())
    }

    fn search_products(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<Product>>> {
        self.search_products_vector_only(embedding, limit)
    }

    fn search_customers(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<Customer>>> {
        self.search_customers_vector_only(embedding, limit)
    }

    fn search_orders(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<Order>>> {
        self.search_orders_vector_only(embedding, limit)
    }

    fn search_inventory(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<InventoryItem>>> {
        self.search_inventory_vector_only(embedding, limit)
    }

    fn delete_embedding(&self, entity_type: EntityType, entity_id: &str) -> Result<()> {
        let conn = self.conn()?;
        let table = entity_type.embedding_table();
        let id_col = entity_type.id_column();

        conn.execute(&format!("DELETE FROM {} WHERE {} = ?", table, id_col), [entity_id])
            .map_err(map_db_error)?;

        conn.execute(
            "DELETE FROM embedding_metadata WHERE entity_type = ? AND entity_id = ?",
            [&entity_type.to_string(), entity_id],
        )
        .map_err(map_db_error)?;

        Ok(())
    }

    fn has_embedding(&self, entity_type: EntityType, entity_id: &str) -> Result<bool> {
        let conn = self.conn()?;

        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM embedding_metadata WHERE entity_type = ? AND entity_id = ?",
                [&entity_type.to_string(), entity_id],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        Ok(count > 0)
    }

    fn get_embedding_metadata(
        &self,
        entity_type: EntityType,
        entity_id: &str,
    ) -> Result<Option<EmbeddingMetadata>> {
        let conn = self.conn()?;

        let result = conn.query_row(
            "SELECT entity_type, entity_id, model, text_hash, created_at
             FROM embedding_metadata
             WHERE entity_type = ? AND entity_id = ?",
            [&entity_type.to_string(), entity_id],
            |row| {
                Ok(EmbeddingMetadata {
                    entity_type,
                    entity_id: row.get("entity_id")?,
                    model: row.get("model")?,
                    text_hash: row.get("text_hash")?,
                    created_at: parse_datetime_row(
                        &row.get::<_, String>("created_at")?,
                        "embedding_metadata",
                        "created_at",
                    )?,
                })
            },
        );

        match result {
            Ok(meta) => Ok(Some(meta)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_stats(&self) -> Result<EmbeddingStats> {
        let conn = self.conn()?;
        let mut counts = HashMap::new();

        for entity_type in [
            EntityType::Product,
            EntityType::Customer,
            EntityType::Order,
            EntityType::InventoryItem,
        ] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM embedding_metadata WHERE entity_type = ?",
                    [entity_type.to_string()],
                    |row| row.get(0),
                )
                .map_err(map_db_error)?;

            counts.insert(entity_type, count as u64);
        }

        let model: String = conn
            .query_row(
                "SELECT model FROM embedding_metadata ORDER BY created_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_db_error)?
            .unwrap_or_else(|| "text-embedding-3-small".to_string());

        let dimensions = Self::infer_dimensions(&conn)?.unwrap_or(1536);

        Ok(EmbeddingStats { counts, model, dimensions })
    }

    fn clear_embeddings(&self, entity_type: EntityType) -> Result<u64> {
        let conn = self.conn()?;
        let table = entity_type.embedding_table();

        // Get count before deleting
        let count: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {}", table), [], |row| row.get(0))
            .map_err(map_db_error)?;

        // Clear embedding table
        conn.execute(&format!("DELETE FROM {}", table), []).map_err(map_db_error)?;

        // Clear metadata
        conn.execute(
            "DELETE FROM embedding_metadata WHERE entity_type = ?",
            [entity_type.to_string()],
        )
        .map_err(map_db_error)?;

        Ok(count as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_blob_conversion() {
        let embedding = vec![1.0f32, 2.0, 3.0, -1.5];
        let blob = SqliteVectorRepository::embedding_to_blob(&embedding);
        let recovered = SqliteVectorRepository::blob_to_embedding(&blob);
        assert_eq!(embedding, recovered);
    }

    #[test]
    fn test_cosine_similarity() {
        // Identical vectors should have similarity 1.0
        let a = vec![1.0f32, 0.0, 0.0];
        let b = vec![1.0f32, 0.0, 0.0];
        let sim = SqliteVectorRepository::cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.001);

        // Orthogonal vectors should have similarity 0.0
        let a = vec![1.0f32, 0.0, 0.0];
        let b = vec![0.0f32, 1.0, 0.0];
        let sim = SqliteVectorRepository::cosine_similarity(&a, &b);
        assert!(sim.abs() < 0.001);

        // Opposite vectors should have similarity -1.0
        let a = vec![1.0f32, 0.0, 0.0];
        let b = vec![-1.0f32, 0.0, 0.0];
        let sim = SqliteVectorRepository::cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 0.001);
    }

    #[test]
    fn test_similarity_to_distance() {
        // Similarity 1.0 -> distance 0.0
        assert!((SqliteVectorRepository::similarity_to_distance(1.0) - 0.0).abs() < 0.001);

        // Similarity 0.0 -> distance 1.0
        assert!((SqliteVectorRepository::similarity_to_distance(0.0) - 1.0).abs() < 0.001);

        // Similarity -1.0 -> distance 2.0
        assert!((SqliteVectorRepository::similarity_to_distance(-1.0) - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_rrf_merge_prefers_overlap() {
        let vector_results = vec![
            VectorSearchResult::new("alpha".to_string(), 0.2),
            VectorSearchResult::new("bravo".to_string(), 0.3),
        ];
        let bm25_results = vec![("bravo".to_string(), -1.0), ("charlie".to_string(), -2.0)];

        let merged = SqliteVectorRepository::merge_rrf(vector_results, bm25_results, 3, |value| {
            value.clone()
        });

        assert_eq!(merged.first().unwrap().entity, "bravo");
    }
}

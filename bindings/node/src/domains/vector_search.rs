//! Vector Search API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Vector Search API
// ============================================================================

/// Kept for the generated declarations; the search path returns `serde_json::Value`.
#[allow(dead_code)]
#[napi(object)]
#[derive(Serialize, Clone)]
pub struct VectorSearchResultOutput {
    pub id: String,
    pub name: String,
    pub distance: f64,
    pub score: f64,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct ProductSearchResultOutput {
    pub product: ProductOutput,
    pub distance: f64,
    pub score: f64,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct CustomerSearchResultOutput {
    pub customer: CustomerOutput,
    pub distance: f64,
    pub score: f64,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct OrderSearchResultOutput {
    pub order: OrderOutput,
    pub distance: f64,
    pub score: f64,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct InventorySearchResultOutput {
    pub item: InventoryItemOutput,
    pub distance: f64,
    pub score: f64,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct EmbeddingStatsOutput {
    pub product_count: u32,
    pub customer_count: u32,
    pub order_count: u32,
    pub inventory_count: u32,
    pub model: String,
    pub dimensions: u32,
}

/// Vector search operations for semantic similarity search
#[napi]
pub struct VectorSearch {
    pub(crate) commerce: Handle,
    pub(crate) api_key: String,
}

#[napi]
impl VectorSearch {
    /// Search products using natural language query
    #[napi]
    pub async fn search_products(
        &self,
        query: String,
        limit: Option<u32>,
    ) -> Result<Vec<ProductSearchResultOutput>> {
        let vector = {
            let commerce = self.commerce.get()?;
            commerce
                .vector(self.api_key.clone())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to initialize vector search", e))?
        };

        let results = vector
            .search_products(&query, limit.unwrap_or(10) as usize)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to search products", e))?;

        Ok(results
            .into_iter()
            .map(|r| ProductSearchResultOutput {
                product: r.entity.into(),
                distance: r.distance as f64,
                score: r.score as f64,
            })
            .collect())
    }

    /// Search customers using natural language query
    #[napi]
    pub async fn search_customers(
        &self,
        query: String,
        limit: Option<u32>,
    ) -> Result<Vec<CustomerSearchResultOutput>> {
        let vector = {
            let commerce = self.commerce.get()?;
            commerce
                .vector(self.api_key.clone())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to initialize vector search", e))?
        };

        let results = vector
            .search_customers(&query, limit.unwrap_or(10) as usize)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to search customers", e))?;

        Ok(results
            .into_iter()
            .map(|r| CustomerSearchResultOutput {
                customer: r.entity.into(),
                distance: r.distance as f64,
                score: r.score as f64,
            })
            .collect())
    }

    /// Search orders using natural language query
    #[napi]
    pub async fn search_orders(
        &self,
        query: String,
        limit: Option<u32>,
    ) -> Result<Vec<OrderSearchResultOutput>> {
        let vector = {
            let commerce = self.commerce.get()?;
            commerce
                .vector(self.api_key.clone())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to initialize vector search", e))?
        };

        let results = vector
            .search_orders(&query, limit.unwrap_or(10) as usize)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to search orders", e))?;

        results
            .into_iter()
            .map(|r| {
                Ok(OrderSearchResultOutput {
                    order: convert_output(r.entity)?,
                    distance: r.distance as f64,
                    score: r.score as f64,
                })
            })
            .collect()
    }

    /// Search inventory items using natural language query
    #[napi]
    pub async fn search_inventory(
        &self,
        query: String,
        limit: Option<u32>,
    ) -> Result<Vec<InventorySearchResultOutput>> {
        let vector = {
            let commerce = self.commerce.get()?;
            commerce
                .vector(self.api_key.clone())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to initialize vector search", e))?
        };

        let results = vector
            .search_inventory(&query, limit.unwrap_or(10) as usize)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to search inventory", e))?;

        Ok(results
            .into_iter()
            .map(|r| InventorySearchResultOutput {
                item: r.entity.into(),
                distance: r.distance as f64,
                score: r.score as f64,
            })
            .collect())
    }

    /// Index a product for vector search
    #[napi]
    pub async fn index_product(&self, product_id: String) -> Result<()> {
        let uuid: uuid::Uuid =
            product_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let (product, vector) = {
            let commerce = self.commerce.get()?;
            let product = commerce
                .products()
                .get(uuid.into())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to get product", e))?
                .ok_or_else(|| coded(ErrCode::NotFound, "Product not found"))?;

            let vector = commerce
                .vector(self.api_key.clone())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to initialize vector search", e))?;

            (product, vector)
        };

        vector
            .index_product(&product)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to index product", e))?;

        Ok(())
    }

    /// Index a customer for vector search
    #[napi]
    pub async fn index_customer(&self, customer_id: String) -> Result<()> {
        let uuid: uuid::Uuid =
            customer_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let (customer, vector) = {
            let commerce = self.commerce.get()?;
            let customer = commerce
                .customers()
                .get(uuid.into())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to get customer", e))?
                .ok_or_else(|| coded(ErrCode::NotFound, "Customer not found"))?;

            let vector = commerce
                .vector(self.api_key.clone())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to initialize vector search", e))?;

            (customer, vector)
        };

        vector
            .index_customer(&customer)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to index customer", e))?;

        Ok(())
    }

    /// Index an order for vector search
    #[napi]
    pub async fn index_order(&self, order_id: String) -> Result<()> {
        let uuid: uuid::Uuid =
            order_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let (order, vector) = {
            let commerce = self.commerce.get()?;
            let order = commerce
                .orders()
                .get(uuid.into())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to get order", e))?
                .ok_or_else(|| coded(ErrCode::NotFound, "Order not found"))?;

            let vector = commerce
                .vector(self.api_key.clone())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to initialize vector search", e))?;

            (order, vector)
        };

        vector
            .index_order(&order)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to index order", e))?;

        Ok(())
    }

    /// Index an inventory item for vector search
    #[napi]
    pub async fn index_inventory_item(&self, item_id: String) -> Result<()> {
        let item_id = item_id
            .parse::<i64>()
            .map_err(|_| coded(ErrCode::Validation, "Invalid inventory item ID"))?;

        let (item, vector) = {
            let commerce = self.commerce.get()?;
            let item = commerce
                .inventory()
                .get_item(item_id)
                .map_err(|e| wrap(ErrCode::Internal, "Failed to get inventory item", e))?
                .ok_or_else(|| coded(ErrCode::NotFound, "Inventory item not found"))?;

            let vector = commerce
                .vector(self.api_key.clone())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to initialize vector search", e))?;

            (item, vector)
        };

        vector
            .index_inventory_item(&item)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to index inventory item", e))?;

        Ok(())
    }

    /// Index all products for vector search
    #[napi]
    pub async fn index_all_products(&self) -> Result<u32> {
        let (products, vector) = {
            let commerce = self.commerce.get()?;
            let products = commerce
                .products()
                .list(Default::default())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to list products", e))?;

            let vector = commerce
                .vector(self.api_key.clone())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to initialize vector search", e))?;

            (products, vector)
        };

        let count = vector
            .index_products(&products)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to index products", e))?;

        Ok(count as u32)
    }

    /// Index all customers for vector search
    #[napi]
    pub async fn index_all_customers(&self) -> Result<u32> {
        let (customers, vector) = {
            let commerce = self.commerce.get()?;
            let customers = commerce
                .customers()
                .list(Default::default())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to list customers", e))?;

            let vector = commerce
                .vector(self.api_key.clone())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to initialize vector search", e))?;

            (customers, vector)
        };

        let count = vector
            .index_customers(&customers)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to index customers", e))?;

        Ok(count as u32)
    }

    /// Index all orders for vector search
    #[napi]
    pub async fn index_all_orders(&self) -> Result<u32> {
        let (orders, vector) = {
            let commerce = self.commerce.get()?;
            let orders = commerce
                .orders()
                .list(Default::default())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to list orders", e))?;

            let vector = commerce
                .vector(self.api_key.clone())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to initialize vector search", e))?;

            (orders, vector)
        };

        let count = vector
            .index_orders(&orders)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to index orders", e))?;

        Ok(count as u32)
    }

    /// Index all inventory items for vector search
    #[napi]
    pub async fn index_all_inventory(&self) -> Result<u32> {
        let (items, vector) = {
            let commerce = self.commerce.get()?;
            let items = commerce
                .inventory()
                .list(Default::default())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to list inventory items", e))?;

            let vector = commerce
                .vector(self.api_key.clone())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to initialize vector search", e))?;

            (items, vector)
        };

        let count = vector
            .index_inventory_items(&items)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to index inventory items", e))?;

        Ok(count as u32)
    }

    /// Get embedding statistics
    #[napi]
    pub async fn stats(&self) -> Result<EmbeddingStatsOutput> {
        let vector = {
            let commerce = self.commerce.get()?;
            commerce
                .vector(self.api_key.clone())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to initialize vector search", e))?
        };

        let stats =
            vector.stats().map_err(|e| wrap(ErrCode::Internal, "Failed to get stats", e))?;

        Ok(EmbeddingStatsOutput {
            product_count: *stats.counts.get(&stateset_core::EntityType::Product).unwrap_or(&0)
                as u32,
            customer_count: *stats.counts.get(&stateset_core::EntityType::Customer).unwrap_or(&0)
                as u32,
            order_count: *stats.counts.get(&stateset_core::EntityType::Order).unwrap_or(&0) as u32,
            inventory_count: *stats
                .counts
                .get(&stateset_core::EntityType::InventoryItem)
                .unwrap_or(&0) as u32,
            model: stats.model,
            dimensions: stats.dimensions as u32,
        })
    }

    /// Clear all embeddings for a specific entity type
    #[napi(ts_args_type = "entityType: VectorEntityType")]
    pub async fn clear(&self, entity_type: String) -> Result<u32> {
        let vector = {
            let commerce = self.commerce.get()?;
            commerce
                .vector(self.api_key.clone())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to initialize vector search", e))?
        };

        let et: stateset_core::EntityType =
            entity_type.parse().map_err(|e: String| coded(ErrCode::Validation, e))?;

        let count = vector
            .clear(et)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to clear embeddings", e))?;

        Ok(count as u32)
    }

    /// Clear all embeddings
    #[napi]
    pub async fn clear_all(&self) -> Result<u32> {
        let vector = {
            let commerce = self.commerce.get()?;
            commerce
                .vector(self.api_key.clone())
                .map_err(|e| wrap(ErrCode::Internal, "Failed to initialize vector search", e))?
        };

        let count = vector
            .clear_all()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to clear all embeddings", e))?;

        Ok(count as u32)
    }
}

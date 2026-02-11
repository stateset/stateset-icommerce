//! Product operations

use stateset_core::{
    CreateProduct, CreateProductVariant, Product, ProductFilter, ProductStatus, ProductVariant,
    Result, UpdateProduct,
};
use stateset_db::Database;
use stateset_observability::Metrics;
use std::sync::Arc;
use uuid::Uuid;

#[cfg(feature = "events")]
use crate::events::EventSystem;
#[cfg(feature = "events")]
use stateset_core::CommerceEvent;

/// Product operations interface.
pub struct Products {
    db: Arc<dyn Database>,
    metrics: Metrics,
    #[cfg(feature = "events")]
    event_system: Arc<EventSystem>,
}

impl Products {
    #[cfg(feature = "events")]
    pub(crate) fn new(
        db: Arc<dyn Database>,
        event_system: Arc<EventSystem>,
        metrics: Metrics,
    ) -> Self {
        Self {
            db,
            metrics,
            event_system,
        }
    }

    #[cfg(not(feature = "events"))]
    pub(crate) fn new(db: Arc<dyn Database>, metrics: Metrics) -> Self {
        Self { db, metrics }
    }

    #[cfg(feature = "events")]
    fn emit(&self, event: CommerceEvent) {
        self.event_system.emit(event);
    }

    #[cfg(feature = "events")]
    fn fields_changed(input: &UpdateProduct) -> Vec<String> {
        let mut fields = Vec::new();
        if input.name.is_some() {
            fields.push("name".to_string());
        }
        if input.slug.is_some() {
            fields.push("slug".to_string());
        }
        if input.description.is_some() {
            fields.push("description".to_string());
        }
        if input.status.is_some() {
            fields.push("status".to_string());
        }
        if input.attributes.is_some() {
            fields.push("attributes".to_string());
        }
        if input.seo.is_some() {
            fields.push("seo".to_string());
        }
        fields
    }

    /// Create a new product.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # use rust_decimal_macros::dec;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let product = commerce.products().create(CreateProduct {
    ///     name: "Premium Widget".into(),
    ///     description: Some("A high-quality widget for all your needs".into()),
    ///     variants: Some(vec![CreateProductVariant {
    ///         sku: "WIDGET-001".into(),
    ///         price: dec!(49.99),
    ///         ..Default::default()
    ///     }]),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn create(&self, input: CreateProduct) -> Result<Product> {
        let product = self.db.products().create(input)?;
        self.metrics.record_product_created(&product.id.to_string());
        #[cfg(feature = "events")]
        {
            self.emit(CommerceEvent::ProductCreated {
                product_id: product.id,
                name: product.name.clone(),
                slug: product.slug.clone(),
                timestamp: product.created_at,
            });
        }
        Ok(product)
    }

    /// Get a product by ID.
    pub fn get(&self, id: Uuid) -> Result<Option<Product>> {
        self.db.products().get(id)
    }

    /// Get a product by slug.
    pub fn get_by_slug(&self, slug: &str) -> Result<Option<Product>> {
        self.db.products().get_by_slug(slug)
    }

    /// Update a product.
    pub fn update(&self, id: Uuid, input: UpdateProduct) -> Result<Product> {
        #[cfg(feature = "events")]
        let previous = self.db.products().get(id)?;
        #[cfg(feature = "events")]
        let fields_changed = Self::fields_changed(&input);

        let updated = self.db.products().update(id, input)?;

        #[cfg(feature = "events")]
        {
            if !fields_changed.is_empty() {
                self.emit(CommerceEvent::ProductUpdated {
                    product_id: updated.id,
                    fields_changed,
                    timestamp: updated.updated_at,
                });
            }

            if let Some(previous) = previous {
                if previous.status != updated.status {
                    self.emit(CommerceEvent::ProductStatusChanged {
                        product_id: updated.id,
                        from_status: previous.status.to_string(),
                        to_status: updated.status.to_string(),
                        timestamp: updated.updated_at,
                    });
                }
            }
        }

        Ok(updated)
    }

    /// List products with optional filtering.
    pub fn list(&self, filter: ProductFilter) -> Result<Vec<Product>> {
        self.db.products().list(filter)
    }

    /// List active products.
    pub fn list_active(&self) -> Result<Vec<Product>> {
        self.db.products().list(ProductFilter {
            status: Some(ProductStatus::Active),
            ..Default::default()
        })
    }

    /// Delete a product (archives it).
    pub fn delete(&self, id: Uuid) -> Result<()> {
        self.db.products().delete(id)
    }

    /// Add a variant to a product.
    pub fn add_variant(
        &self,
        product_id: Uuid,
        variant: CreateProductVariant,
    ) -> Result<ProductVariant> {
        let variant = self.db.products().add_variant(product_id, variant)?;
        #[cfg(feature = "events")]
        {
            self.emit(CommerceEvent::ProductVariantAdded {
                product_id,
                variant_id: variant.id,
                sku: variant.sku.clone(),
                timestamp: variant.created_at,
            });
        }
        Ok(variant)
    }

    /// Get a variant by ID.
    pub fn get_variant(&self, id: Uuid) -> Result<Option<ProductVariant>> {
        self.db.products().get_variant(id)
    }

    /// Get a variant by SKU.
    pub fn get_variant_by_sku(&self, sku: &str) -> Result<Option<ProductVariant>> {
        self.db.products().get_variant_by_sku(sku)
    }

    /// Update a variant.
    pub fn update_variant(
        &self,
        id: Uuid,
        variant: CreateProductVariant,
    ) -> Result<ProductVariant> {
        let variant = self.db.products().update_variant(id, variant)?;
        #[cfg(feature = "events")]
        {
            self.emit(CommerceEvent::ProductVariantUpdated {
                variant_id: variant.id,
                sku: variant.sku.clone(),
                timestamp: variant.updated_at,
            });
        }
        Ok(variant)
    }

    /// Delete a variant.
    pub fn delete_variant(&self, id: Uuid) -> Result<()> {
        self.db.products().delete_variant(id)
    }

    /// Get all variants for a product.
    pub fn get_variants(&self, product_id: Uuid) -> Result<Vec<ProductVariant>> {
        self.db.products().get_variants(product_id)
    }

    /// Count products matching a filter.
    pub fn count(&self, filter: ProductFilter) -> Result<u64> {
        self.db.products().count(filter)
    }

    /// Activate a product (make it available for purchase).
    pub fn activate(&self, id: Uuid) -> Result<Product> {
        self.update(
            id,
            UpdateProduct {
                status: Some(ProductStatus::Active),
                ..Default::default()
            },
        )
    }

    /// Archive a product.
    pub fn archive(&self, id: Uuid) -> Result<Product> {
        self.update(
            id,
            UpdateProduct {
                status: Some(ProductStatus::Archived),
                ..Default::default()
            },
        )
    }

    /// Search products by name or description.
    pub fn search(&self, query: &str) -> Result<Vec<Product>> {
        self.db.products().list(ProductFilter {
            search: Some(query.to_string()),
            status: Some(ProductStatus::Active),
            ..Default::default()
        })
    }
}

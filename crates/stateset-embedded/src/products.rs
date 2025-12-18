//! Product operations

use stateset_core::{
    CreateProduct, CreateProductVariant, Product, ProductFilter, ProductStatus,
    ProductVariant, Result, UpdateProduct,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

/// Product operations interface.
pub struct Products {
    db: Arc<dyn Database>,
}

impl Products {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
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
        self.db.products().create(input)
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
        self.db.products().update(id, input)
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
    pub fn add_variant(&self, product_id: Uuid, variant: CreateProductVariant) -> Result<ProductVariant> {
        self.db.products().add_variant(product_id, variant)
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
    pub fn update_variant(&self, id: Uuid, variant: CreateProductVariant) -> Result<ProductVariant> {
        self.db.products().update_variant(id, variant)
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
        self.db.products().update(
            id,
            UpdateProduct {
                status: Some(ProductStatus::Active),
                ..Default::default()
            },
        )
    }

    /// Archive a product.
    pub fn archive(&self, id: Uuid) -> Result<Product> {
        self.db.products().update(
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

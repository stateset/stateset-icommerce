//! PostgreSQL product repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::FromRow;
use stateset_core::{
    CommerceError, CreateProduct, CreateProductVariant, Product, ProductAttribute, ProductFilter,
    ProductRepository, ProductStatus, ProductType, ProductVariant, Result, SeoMetadata,
    UpdateProduct, VariantOption,
};
use uuid::Uuid;

/// PostgreSQL implementation of ProductRepository
#[derive(Clone)]
pub struct PgProductRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct ProductRow {
    id: Uuid,
    name: String,
    slug: String,
    description: String,
    status: String,
    product_type: String,
    attributes: serde_json::Value,
    seo: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct VariantRow {
    id: Uuid,
    product_id: Uuid,
    sku: String,
    name: String,
    price: Decimal,
    compare_at_price: Option<Decimal>,
    cost: Option<Decimal>,
    barcode: Option<String>,
    weight: Option<Decimal>,
    weight_unit: Option<String>,
    options: serde_json::Value,
    is_default: bool,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgProductRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_product(row: ProductRow) -> Product {
        Product {
            id: row.id,
            name: row.name,
            slug: row.slug,
            description: row.description,
            status: parse_product_status(&row.status),
            product_type: parse_product_type(&row.product_type),
            attributes: serde_json::from_value(row.attributes).unwrap_or_default(),
            seo: row.seo.and_then(|v| serde_json::from_value(v).ok()),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    fn row_to_variant(row: VariantRow) -> ProductVariant {
        ProductVariant {
            id: row.id,
            product_id: row.product_id,
            sku: row.sku,
            name: row.name,
            price: row.price,
            compare_at_price: row.compare_at_price,
            cost: row.cost,
            barcode: row.barcode,
            weight: row.weight,
            weight_unit: row.weight_unit,
            options: serde_json::from_value(row.options).unwrap_or_default(),
            is_default: row.is_default,
            is_active: row.is_active,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    /// Create a product (async)
    pub async fn create_async(&self, input: CreateProduct) -> Result<Product> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let slug = input.slug.clone().unwrap_or_else(|| Product::generate_slug(&input.name));
        let description = input.description.clone().unwrap_or_default();
        let product_type = input.product_type.unwrap_or_default();
        let attributes = input.attributes.clone().unwrap_or_default();

        let attributes_json = serde_json::to_value(&attributes).unwrap_or_default();
        let seo_json = input.seo.as_ref().map(|s| serde_json::to_value(s).unwrap_or_default());

        sqlx::query(
            r#"
            INSERT INTO products (id, name, slug, description, status, product_type, attributes, seo, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(id)
        .bind(&input.name)
        .bind(&slug)
        .bind(&description)
        .bind("draft")
        .bind(format!("{:?}", product_type).to_lowercase())
        .bind(&attributes_json)
        .bind(&seo_json)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        // Create variants if provided
        if let Some(variant_inputs) = &input.variants {
            for (i, vi) in variant_inputs.iter().enumerate() {
                self.add_variant_async(id, vi.clone(), i == 0).await?;
            }
        }

        Ok(Product {
            id,
            name: input.name,
            slug,
            description,
            status: ProductStatus::Draft,
            product_type,
            attributes,
            seo: input.seo,
            created_at: now,
            updated_at: now,
        })
    }

    /// Get a product by ID (async)
    pub async fn get_async(&self, id: Uuid) -> Result<Option<Product>> {
        let row = sqlx::query_as::<_, ProductRow>("SELECT * FROM products WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_product))
    }

    /// Get product by slug (async)
    pub async fn get_by_slug_async(&self, slug: &str) -> Result<Option<Product>> {
        let row = sqlx::query_as::<_, ProductRow>("SELECT * FROM products WHERE slug = $1")
            .bind(slug)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_product))
    }

    /// Update a product (async)
    pub async fn update_async(&self, id: Uuid, input: UpdateProduct) -> Result<Product> {
        let now = Utc::now();

        let existing = self.get_async(id).await?.ok_or(CommerceError::ProductNotFound(id))?;

        let new_name = input.name.unwrap_or(existing.name);
        let new_slug = input.slug.unwrap_or(existing.slug);
        let new_description = input.description.unwrap_or(existing.description);
        let new_status = input.status.unwrap_or(existing.status);
        let new_attributes = input.attributes.unwrap_or(existing.attributes);
        let new_seo = input.seo.or(existing.seo);

        let attributes_json = serde_json::to_value(&new_attributes).unwrap_or_default();
        let seo_json = new_seo.as_ref().map(|s| serde_json::to_value(s).unwrap_or_default());

        sqlx::query(
            r#"
            UPDATE products
            SET name = $1, slug = $2, description = $3, status = $4,
                attributes = $5, seo = $6, updated_at = $7
            WHERE id = $8
            "#,
        )
        .bind(&new_name)
        .bind(&new_slug)
        .bind(&new_description)
        .bind(new_status.to_string())
        .bind(&attributes_json)
        .bind(&seo_json)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::ProductNotFound(id))
    }

    /// List products (async)
    pub async fn list_async(&self, filter: ProductFilter) -> Result<Vec<Product>> {
        let limit = filter.limit.unwrap_or(100) as i64;
        let offset = filter.offset.unwrap_or(0) as i64;

        let rows = sqlx::query_as::<_, ProductRow>(
            "SELECT * FROM products ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_product).collect())
    }

    /// Delete a product (async)
    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        sqlx::query("UPDATE products SET status = 'archived', updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    /// Add variant to product (async)
    async fn add_variant_async(
        &self,
        product_id: Uuid,
        input: CreateProductVariant,
        is_default: bool,
    ) -> Result<ProductVariant> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let options = input.options.clone().unwrap_or_default();
        let options_json = serde_json::to_value(&options).unwrap_or_default();

        sqlx::query(
            r#"
            INSERT INTO product_variants (id, product_id, sku, name, price, compare_at_price, cost,
                                          barcode, weight, weight_unit, options, is_default, is_active,
                                          created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            "#,
        )
        .bind(id)
        .bind(product_id)
        .bind(&input.sku)
        .bind(input.name.as_deref().unwrap_or(&input.sku))
        .bind(input.price)
        .bind(input.compare_at_price)
        .bind(input.cost)
        .bind(&input.barcode)
        .bind(input.weight)
        .bind(&input.weight_unit)
        .bind(&options_json)
        .bind(input.is_default.unwrap_or(is_default))
        .bind(true)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(ProductVariant {
            id,
            product_id,
            sku: input.sku,
            name: input.name.unwrap_or_default(),
            price: input.price,
            compare_at_price: input.compare_at_price,
            cost: input.cost,
            barcode: input.barcode,
            weight: input.weight,
            weight_unit: input.weight_unit,
            options,
            is_default: input.is_default.unwrap_or(is_default),
            is_active: true,
            created_at: now,
            updated_at: now,
        })
    }

    /// Add variant (public async)
    pub async fn add_variant_public_async(
        &self,
        product_id: Uuid,
        input: CreateProductVariant,
    ) -> Result<ProductVariant> {
        self.add_variant_async(product_id, input, false).await
    }

    /// Get variant by ID (async)
    pub async fn get_variant_async(&self, id: Uuid) -> Result<Option<ProductVariant>> {
        let row = sqlx::query_as::<_, VariantRow>("SELECT * FROM product_variants WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_variant))
    }

    /// Get variant by SKU (async)
    pub async fn get_variant_by_sku_async(&self, sku: &str) -> Result<Option<ProductVariant>> {
        let row = sqlx::query_as::<_, VariantRow>("SELECT * FROM product_variants WHERE sku = $1")
            .bind(sku)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_variant))
    }

    /// Update variant (async)
    pub async fn update_variant_async(
        &self,
        id: Uuid,
        input: CreateProductVariant,
    ) -> Result<ProductVariant> {
        let now = Utc::now();
        let options_json = serde_json::to_value(&input.options.clone().unwrap_or_default()).unwrap_or_default();

        sqlx::query(
            r#"
            UPDATE product_variants
            SET sku = $1, name = $2, price = $3, compare_at_price = $4, cost = $5,
                barcode = $6, weight = $7, weight_unit = $8, options = $9, updated_at = $10
            WHERE id = $11
            "#,
        )
        .bind(&input.sku)
        .bind(input.name.as_deref().unwrap_or(&input.sku))
        .bind(input.price)
        .bind(input.compare_at_price)
        .bind(input.cost)
        .bind(&input.barcode)
        .bind(input.weight)
        .bind(&input.weight_unit)
        .bind(&options_json)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_variant_async(id)
            .await?
            .ok_or(CommerceError::ProductVariantNotFound(id))
    }

    /// Delete variant (async)
    pub async fn delete_variant_async(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM product_variants WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    /// Get all variants for product (async)
    pub async fn get_variants_async(&self, product_id: Uuid) -> Result<Vec<ProductVariant>> {
        let rows =
            sqlx::query_as::<_, VariantRow>("SELECT * FROM product_variants WHERE product_id = $1")
                .bind(product_id)
                .fetch_all(&self.pool)
                .await
                .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_variant).collect())
    }

    /// Count products (async)
    pub async fn count_async(&self, _filter: ProductFilter) -> Result<u64> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM products WHERE status != 'archived'")
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(count.0 as u64)
    }
}

impl ProductRepository for PgProductRepository {
    fn create(&self, input: CreateProduct) -> Result<Product> {
        tokio::runtime::Handle::current().block_on(self.create_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<Product>> {
        tokio::runtime::Handle::current().block_on(self.get_async(id))
    }

    fn get_by_slug(&self, slug: &str) -> Result<Option<Product>> {
        tokio::runtime::Handle::current().block_on(self.get_by_slug_async(slug))
    }

    fn update(&self, id: Uuid, input: UpdateProduct) -> Result<Product> {
        tokio::runtime::Handle::current().block_on(self.update_async(id, input))
    }

    fn list(&self, filter: ProductFilter) -> Result<Vec<Product>> {
        tokio::runtime::Handle::current().block_on(self.list_async(filter))
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        tokio::runtime::Handle::current().block_on(self.delete_async(id))
    }

    fn add_variant(&self, product_id: Uuid, variant: CreateProductVariant) -> Result<ProductVariant> {
        tokio::runtime::Handle::current().block_on(self.add_variant_public_async(product_id, variant))
    }

    fn get_variant(&self, id: Uuid) -> Result<Option<ProductVariant>> {
        tokio::runtime::Handle::current().block_on(self.get_variant_async(id))
    }

    fn get_variant_by_sku(&self, sku: &str) -> Result<Option<ProductVariant>> {
        tokio::runtime::Handle::current().block_on(self.get_variant_by_sku_async(sku))
    }

    fn update_variant(&self, id: Uuid, variant: CreateProductVariant) -> Result<ProductVariant> {
        tokio::runtime::Handle::current().block_on(self.update_variant_async(id, variant))
    }

    fn delete_variant(&self, id: Uuid) -> Result<()> {
        tokio::runtime::Handle::current().block_on(self.delete_variant_async(id))
    }

    fn get_variants(&self, product_id: Uuid) -> Result<Vec<ProductVariant>> {
        tokio::runtime::Handle::current().block_on(self.get_variants_async(product_id))
    }

    fn count(&self, filter: ProductFilter) -> Result<u64> {
        tokio::runtime::Handle::current().block_on(self.count_async(filter))
    }
}

fn parse_product_status(s: &str) -> ProductStatus {
    match s {
        "draft" => ProductStatus::Draft,
        "active" => ProductStatus::Active,
        "archived" => ProductStatus::Archived,
        _ => ProductStatus::Draft,
    }
}

fn parse_product_type(s: &str) -> ProductType {
    match s {
        "simple" => ProductType::Simple,
        "variable" => ProductType::Variable,
        "bundle" => ProductType::Bundle,
        "digital" => ProductType::Digital,
        _ => ProductType::Simple,
    }
}

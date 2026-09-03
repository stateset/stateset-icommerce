//! PostgreSQL product repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::{PgConnection, PgPool};
use sqlx::{FromRow, QueryBuilder};
use stateset_core::{
    BatchResult, CommerceError, CreateProduct, CreateProductVariant, Product, ProductFilter,
    ProductId, ProductRepository, ProductStatus, ProductType, ProductVariant, Result,
    UpdateProduct, Validate, VariantPurchasability, validate_batch_size, validate_sku,
};
use uuid::Uuid;

/// Order statuses that still count as "open" for the purpose of withdrawing a
/// SKU from sale (mirrors the SQLite backend).
pub(crate) const OPEN_ORDER_STATUSES: &str =
    "'pending','confirmed','processing','partially_shipped'";

/// Reservation statuses that still hold stock against a SKU.
pub(crate) const ACTIVE_RESERVATION_STATUSES: &str = "'pending','confirmed','allocated'";

/// Live references to one or more SKUs from the cart / order / reservation
/// tables. A product or variant may only be withdrawn from sale when all
/// three counts are zero (see [`SkuReferenceCounts::ensure_none`]).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SkuReferenceCounts {
    /// Lines in carts whose status is `active`.
    pub active_cart_lines: u64,
    /// Lines on orders in an open status.
    pub open_order_lines: u64,
    /// Inventory reservations in pending / confirmed / allocated status.
    pub active_reservations: u64,
}

impl SkuReferenceCounts {
    /// Whether nothing live references the SKU set.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.active_cart_lines == 0 && self.open_order_lines == 0 && self.active_reservations == 0
    }

    /// Refuse with a [`CommerceError::Conflict`] naming every non-zero count.
    ///
    /// # Errors
    ///
    /// Returns `Conflict` when any count is non-zero.
    pub fn ensure_none(&self, action: &str, subject: &str) -> Result<()> {
        if self.is_empty() {
            return Ok(());
        }
        Err(CommerceError::Conflict(format!(
            "cannot {action} {subject}: {} active cart line(s), {} open order line(s) and {} active reservation(s) still reference its SKU(s)",
            self.active_cart_lines, self.open_order_lines, self.active_reservations
        )))
    }
}

/// Count live references to the SKUs selected by `sku_predicate` (a SQL
/// fragment over the column `sku` using the single bind `$1`).
async fn sku_reference_counts<A>(
    conn: &mut PgConnection,
    sku_predicate: &str,
    param: A,
) -> Result<SkuReferenceCounts>
where
    A: for<'q> sqlx::Encode<'q, sqlx::Postgres> + sqlx::Type<sqlx::Postgres> + Clone + Send + Sync,
{
    let cart_sql = format!(
        "SELECT COUNT(*) FROM cart_items ci JOIN carts c ON c.id = ci.cart_id \
         WHERE c.status = 'active' AND ci.sku {sku_predicate}"
    );
    let order_sql = format!(
        "SELECT COUNT(*) FROM order_items oi JOIN orders o ON o.id = oi.order_id \
         WHERE o.status IN ({OPEN_ORDER_STATUSES}) AND oi.sku {sku_predicate}"
    );
    let reservation_sql = format!(
        "SELECT COUNT(*) FROM inventory_reservations r JOIN inventory_items ii ON ii.id = r.item_id \
         WHERE r.status IN ({ACTIVE_RESERVATION_STATUSES}) AND ii.sku {sku_predicate}"
    );
    let cart: i64 = sqlx::query_scalar(&cart_sql)
        .bind(param.clone())
        .fetch_one(&mut *conn)
        .await
        .map_err(map_db_error)?;
    let order: i64 = sqlx::query_scalar(&order_sql)
        .bind(param.clone())
        .fetch_one(&mut *conn)
        .await
        .map_err(map_db_error)?;
    let reservation: i64 = sqlx::query_scalar(&reservation_sql)
        .bind(param)
        .fetch_one(&mut *conn)
        .await
        .map_err(map_db_error)?;
    Ok(SkuReferenceCounts {
        active_cart_lines: u64::try_from(cart).unwrap_or_default(),
        open_order_lines: u64::try_from(order).unwrap_or_default(),
        active_reservations: u64::try_from(reservation).unwrap_or_default(),
    })
}

/// Live references to every SKU of `product_id` (active and inactive
/// variants alike).
pub(crate) async fn product_reference_counts_pg(
    conn: &mut PgConnection,
    product_id: ProductId,
) -> Result<SkuReferenceCounts> {
    sku_reference_counts(
        conn,
        "IN (SELECT sku FROM product_variants WHERE product_id = $1)",
        product_id.into_uuid(),
    )
    .await
}

/// Live references to a single SKU.
pub(crate) async fn sku_reference_counts_for_pg(
    conn: &mut PgConnection,
    sku: &str,
) -> Result<SkuReferenceCounts> {
    sku_reference_counts(conn, "= $1", sku.to_string()).await
}

/// Whether `sku` may be sold right now, on an existing connection /
/// transaction. Postgres twin of the SQLite `variant_is_purchasable_with_conn`;
/// intended for `carts.rs::add_item_internal` and order line creation.
pub(crate) async fn variant_is_purchasable_with_conn_pg(
    conn: &mut PgConnection,
    sku: &str,
) -> Result<VariantPurchasability> {
    let row: Option<(bool, String)> = sqlx::query_as(
        "SELECT pv.is_active, p.status FROM product_variants pv \
         JOIN products p ON p.id = pv.product_id WHERE pv.sku = $1",
    )
    .bind(sku)
    .fetch_optional(conn)
    .await
    .map_err(map_db_error)?;
    let Some((is_active, status)) = row else {
        return Ok(VariantPurchasability::NotInCatalog);
    };
    if !is_active {
        return Ok(VariantPurchasability::VariantInactive);
    }
    let status: ProductStatus = status.parse().map_err(|e| {
        CommerceError::DatabaseError(format!("Invalid product.status '{status}': {e}"))
    })?;
    if status == ProductStatus::Active {
        Ok(VariantPurchasability::Purchasable)
    } else {
        Ok(VariantPurchasability::ProductNotActive(status))
    }
}

/// PostgreSQL implementation of `ProductRepository`
#[derive(Debug, Clone)]
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
    version: i32,
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
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_product(row: ProductRow) -> Result<Product> {
        let ProductRow {
            id,
            name,
            slug,
            description,
            status,
            product_type,
            attributes,
            seo,
            created_at,
            updated_at,
            version: _,
        } = row;

        let status: ProductStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid product.status '{}': {}", status, e))
        })?;
        let product_type: ProductType = product_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid product.product_type '{}': {}",
                product_type, e
            ))
        })?;
        let attributes = serde_json::from_value(attributes).map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid JSON for product.attributes: {}", e))
        })?;
        let seo = seo.map(serde_json::from_value).transpose().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid JSON for product.seo: {}", e))
        })?;

        Ok(Product {
            id: ProductId::from(id),
            name,
            slug,
            description,
            status,
            product_type,
            attributes,
            seo,
            created_at,
            updated_at,
        })
    }

    fn row_to_variant(row: VariantRow) -> Result<ProductVariant> {
        let VariantRow {
            id,
            product_id,
            sku,
            name,
            price,
            compare_at_price,
            cost,
            barcode,
            weight,
            weight_unit,
            options,
            is_default,
            is_active,
            created_at,
            updated_at,
        } = row;

        let options = serde_json::from_value(options).map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid JSON for product_variant.options: {}", e))
        })?;

        Ok(ProductVariant {
            id,
            product_id: ProductId::from(product_id),
            sku,
            name,
            price,
            compare_at_price,
            cost,
            barcode,
            weight,
            weight_unit,
            options,
            is_default,
            is_active,
            created_at,
            updated_at,
        })
    }

    /// Whether `sku` may currently be sold (see `variant_is_purchasable_with_conn_pg`).
    pub async fn variant_purchasability_async(&self, sku: &str) -> Result<VariantPurchasability> {
        let mut conn = self.pool.acquire().await.map_err(map_db_error)?;
        variant_is_purchasable_with_conn_pg(&mut conn, sku).await
    }

    /// Live cart / order / reservation references to the product's SKUs.
    pub async fn reference_counts_async(
        &self,
        product_id: ProductId,
    ) -> Result<SkuReferenceCounts> {
        let mut conn = self.pool.acquire().await.map_err(map_db_error)?;
        product_reference_counts_pg(&mut conn, product_id).await
    }

    /// All variants including soft-deleted (`is_active = false`) rows.
    pub async fn get_variants_including_inactive_async(
        &self,
        product_id: ProductId,
    ) -> Result<Vec<ProductVariant>> {
        let rows = sqlx::query_as::<_, VariantRow>(
            "SELECT * FROM product_variants WHERE product_id = $1 ORDER BY is_default DESC, sku",
        )
        .bind(product_id.into_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_variant).collect()
    }

    /// Insert one variant on an open connection / transaction.
    ///
    /// Validates the SKU, requires the parent product to exist (typed
    /// `ProductNotFound` rather than a foreign-key failure) and checks SKU
    /// uniqueness up front; the UNIQUE index (mapped to `DuplicateSku` by
    /// `map_db_error`) backstops the race window.
    async fn insert_variant_tx(
        conn: &mut PgConnection,
        product_id: ProductId,
        input: &CreateProductVariant,
        default_if_unset: bool,
        now: DateTime<Utc>,
    ) -> Result<ProductVariant> {
        // SKU format, amounts and the compare-at relationship. The embedded
        // facade validates too, but the repository is a public API of its own.
        // `validate_sku` runs first so a malformed SKU keeps its historical
        // `ValidationError`.
        validate_sku(&input.sku)?;
        input.validate()?;

        let product_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM products WHERE id = $1)")
                .bind(product_id.into_uuid())
                .fetch_one(&mut *conn)
                .await
                .map_err(map_db_error)?;
        if !product_exists {
            return Err(CommerceError::ProductNotFound(product_id.into_uuid()));
        }

        let sku_taken: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM product_variants WHERE sku = $1)")
                .bind(&input.sku)
                .fetch_one(&mut *conn)
                .await
                .map_err(map_db_error)?;
        if sku_taken {
            return Err(CommerceError::DuplicateSku(input.sku.clone()));
        }

        let id = Uuid::new_v4();
        let name = input.name.clone().unwrap_or_else(|| input.sku.clone());
        let options = input.options.clone().unwrap_or_default();
        let options_json = serde_json::to_value(&options).unwrap_or_default();
        let is_default = input.is_default.unwrap_or(default_if_unset);

        sqlx::query(
            r#"
            INSERT INTO product_variants (id, product_id, sku, name, price, compare_at_price, cost,
                                          barcode, weight, weight_unit, options, is_default, is_active,
                                          created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            "#,
        )
        .bind(id)
        .bind(product_id.into_uuid())
        .bind(&input.sku)
        .bind(&name)
        .bind(input.price)
        .bind(input.compare_at_price)
        .bind(input.cost)
        .bind(&input.barcode)
        .bind(input.weight)
        .bind(&input.weight_unit)
        .bind(&options_json)
        .bind(is_default)
        .bind(true)
        .bind(now)
        .bind(now)
        .execute(&mut *conn)
        .await
        .map_err(map_db_error)?;

        Ok(ProductVariant {
            id,
            product_id,
            sku: input.sku.clone(),
            name,
            price: input.price,
            compare_at_price: input.compare_at_price,
            cost: input.cost,
            barcode: input.barcode.clone(),
            weight: input.weight,
            weight_unit: input.weight_unit.clone(),
            options,
            is_default,
            is_active: true,
            created_at: now,
            updated_at: now,
        })
    }

    /// Insert one product and its inline variants on an open transaction.
    /// Shared by `create_async` and `create_batch_atomic_async` so a failing
    /// second variant rolls the product back instead of leaving a half-product.
    async fn insert_product_tx(conn: &mut PgConnection, input: &CreateProduct) -> Result<Product> {
        let id = ProductId::new();
        let now = Utc::now();
        let slug = input.slug.clone().unwrap_or_else(|| Product::generate_slug(&input.name));
        let description = input.description.clone().unwrap_or_default();
        let product_type = input.product_type.unwrap_or_default();
        let attributes = input.attributes.clone().unwrap_or_default();

        // Validate every inline variant (SKU, amounts and the compare-at
        // relationship) before writing anything, so a bad later variant cannot
        // leave a half-created product behind and a caller holding the
        // repository directly cannot store money the catalogue cannot honour.
        if let Some(variants) = &input.variants {
            for variant in variants {
                // `validate_sku` first so a malformed SKU keeps its historical
                // `ValidationError`; `validate` then covers the amounts.
                validate_sku(&variant.sku)?;
                variant.validate()?;
            }
        }

        let slug_taken: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM products WHERE slug = $1)")
                .bind(&slug)
                .fetch_one(&mut *conn)
                .await
                .map_err(map_db_error)?;
        if slug_taken {
            return Err(CommerceError::DuplicateSlug(slug));
        }

        let attributes_json = serde_json::to_value(&attributes).unwrap_or_default();
        let seo_json = input.seo.as_ref().map(|s| serde_json::to_value(s).unwrap_or_default());

        sqlx::query(
            r#"
            INSERT INTO products (id, name, slug, description, status, product_type, attributes, seo, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(id.into_uuid())
        .bind(&input.name)
        .bind(&slug)
        .bind(&description)
        .bind(ProductStatus::Draft.to_string())
        .bind(product_type.to_string())
        .bind(&attributes_json)
        .bind(&seo_json)
        .bind(now)
        .bind(now)
        .execute(&mut *conn)
        .await
        .map_err(map_db_error)?;

        if let Some(variant_inputs) = &input.variants {
            for (i, vi) in variant_inputs.iter().enumerate() {
                Self::insert_variant_tx(conn, id, vi, i == 0, now).await?;
            }
        }

        Ok(Product {
            id,
            name: input.name.clone(),
            slug,
            description,
            status: ProductStatus::Draft,
            product_type,
            attributes,
            seo: input.seo.clone(),
            created_at: now,
            updated_at: now,
        })
    }

    /// Apply a partial update on an open transaction (row locked with
    /// `FOR UPDATE`). Only the supplied fields are written, matching the
    /// SQLite backend, and the [`ProductStatus`] state machine plus the
    /// live-reference guard for archiving are enforced.
    async fn update_product_tx(
        conn: &mut PgConnection,
        id: ProductId,
        input: &UpdateProduct,
    ) -> Result<Product> {
        let now = Utc::now();
        let existing_row =
            sqlx::query_as::<_, ProductRow>("SELECT * FROM products WHERE id = $1 FOR UPDATE")
                .bind(id.into_uuid())
                .fetch_optional(&mut *conn)
                .await
                .map_err(map_db_error)?
                .ok_or(CommerceError::ProductNotFound(id.into_uuid()))?;
        let current_version = existing_row.version;
        let existing = Self::row_to_product(existing_row)?;

        if let Some(slug) = &input.slug {
            let owner: Option<Uuid> = sqlx::query_scalar("SELECT id FROM products WHERE slug = $1")
                .bind(slug)
                .fetch_optional(&mut *conn)
                .await
                .map_err(map_db_error)?;
            if owner.is_some_and(|owner| owner != id.into_uuid()) {
                return Err(CommerceError::DuplicateSlug(slug.clone()));
            }
        }
        if let Some(status) = input.status {
            existing.status.ensure_can_transition_to(status)?;
            // Any move out of `Active` — archiving OR unpublishing back to
            // `Draft` — withdraws the SKUs from sale, so both need the same
            // live-reference guard; without it, unpublishing left carts
            // holding a SKU checkout would still accept.
            if let Some(action) = existing.status.withdrawal_action(status) {
                product_reference_counts_pg(conn, id)
                    .await?
                    .ensure_none(action, &format!("product {id}"))?;
            }
        }

        let mut builder = QueryBuilder::new("UPDATE products SET updated_at = ");
        builder.push_bind(now);
        if let Some(name) = &input.name {
            builder.push(", name = ").push_bind(name.clone());
        }
        if let Some(slug) = &input.slug {
            builder.push(", slug = ").push_bind(slug.clone());
        }
        if let Some(description) = &input.description {
            builder.push(", description = ").push_bind(description.clone());
        }
        if let Some(status) = input.status {
            builder.push(", status = ").push_bind(status.to_string());
        }
        if let Some(attributes) = &input.attributes {
            let json = serde_json::to_value(attributes).unwrap_or_default();
            builder.push(", attributes = ").push_bind(json);
        }
        if let Some(seo) = &input.seo {
            let json = serde_json::to_value(seo).unwrap_or_default();
            builder.push(", seo = ").push_bind(json);
        }
        builder.push(", version = version + 1 WHERE id = ").push_bind(id.into_uuid());
        builder.push(" AND version = ").push_bind(current_version);

        let result = builder.build().execute(&mut *conn).await.map_err(map_db_error)?;
        if result.rows_affected() == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "product".to_string(),
                id: id.to_string(),
                expected_version: current_version,
            });
        }

        let row = sqlx::query_as::<_, ProductRow>("SELECT * FROM products WHERE id = $1")
            .bind(id.into_uuid())
            .fetch_optional(&mut *conn)
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::ProductNotFound(id.into_uuid()))?;
        Self::row_to_product(row)
    }

    /// Archive one product, refusing while live references exist. Unknown or
    /// already-archived products are a no-op so deletes stay idempotent.
    async fn archive_product_tx(conn: &mut PgConnection, id: ProductId) -> Result<()> {
        let status: Option<String> =
            sqlx::query_scalar("SELECT status FROM products WHERE id = $1 FOR UPDATE")
                .bind(id.into_uuid())
                .fetch_optional(&mut *conn)
                .await
                .map_err(map_db_error)?;
        let Some(status) = status else {
            return Ok(());
        };
        if status == ProductStatus::Archived.to_string() {
            return Ok(());
        }
        product_reference_counts_pg(conn, id)
            .await?
            .ensure_none("archive", &format!("product {id}"))?;
        sqlx::query(
            "UPDATE products SET status = 'archived', updated_at = $1, version = version + 1 WHERE id = $2",
        )
        .bind(Utc::now())
        .bind(id.into_uuid())
        .execute(&mut *conn)
        .await
        .map_err(map_db_error)?;
        Ok(())
    }

    /// Create a product (async) — product and inline variants in one transaction.
    pub async fn create_async(&self, input: CreateProduct) -> Result<Product> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let product = Self::insert_product_tx(tx.as_mut(), &input).await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(product)
    }

    /// Get a product by ID (async)
    pub async fn get_async(&self, id: ProductId) -> Result<Option<Product>> {
        let row = sqlx::query_as::<_, ProductRow>("SELECT * FROM products WHERE id = $1")
            .bind(id.into_uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_product).transpose()
    }

    /// Get product by slug (async)
    pub async fn get_by_slug_async(&self, slug: &str) -> Result<Option<Product>> {
        let row = sqlx::query_as::<_, ProductRow>("SELECT * FROM products WHERE slug = $1")
            .bind(slug)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_product).transpose()
    }

    /// Update a product (async)
    pub async fn update_async(&self, id: ProductId, input: UpdateProduct) -> Result<Product> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let product = Self::update_product_tx(tx.as_mut(), id, &input).await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(product)
    }

    fn push_list_filters(builder: &mut QueryBuilder<'_, sqlx::Postgres>, filter: &ProductFilter) {
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        } else {
            builder.push(" AND status != 'archived'");
        }
        if let Some(product_type) = filter.product_type {
            builder.push(" AND product_type = ").push_bind(product_type.to_string());
        }
        if let Some(search) = &filter.search {
            let pattern = format!("%{}%", search);
            builder
                .push(" AND (name ILIKE ")
                .push_bind(pattern.clone())
                .push(" OR description ILIKE ")
                .push_bind(pattern)
                .push(')');
        }
        if let Some(category) = &filter.category {
            builder.push(
                " AND EXISTS (SELECT 1 FROM jsonb_array_elements(attributes) attr \
                 WHERE (attr->>'name' = 'category' OR attr->>'group' = 'category') \
                   AND attr->>'value' = ",
            );
            builder.push_bind(category.clone()).push(')');
        }
        if filter.min_price.is_some() || filter.max_price.is_some() {
            builder.push(
                " AND EXISTS (SELECT 1 FROM product_variants pv \
                 WHERE pv.product_id = products.id AND pv.is_active = true",
            );
            if let Some(min_price) = filter.min_price {
                builder.push(" AND pv.price >= ").push_bind(min_price);
            }
            if let Some(max_price) = filter.max_price {
                builder.push(" AND pv.price <= ").push_bind(max_price);
            }
            builder.push(')');
        }
        if let Some(in_stock) = filter.in_stock {
            let clause = if in_stock { " AND EXISTS" } else { " AND NOT EXISTS" };
            builder.push(clause).push(
                " (SELECT 1 FROM product_variants pv_stock \
                 JOIN inventory_items ii ON ii.sku = pv_stock.sku \
                 JOIN inventory_balances ib ON ib.item_id = ii.id \
                 WHERE pv_stock.product_id = products.id \
                   AND pv_stock.is_active = true \
                   AND ib.quantity_available > 0)",
            );
        }
    }

    /// List products (async).
    ///
    /// Ordered by `(name ASC, id ASC)` and paginated by the same keyset cursor
    /// as the SQLite backend (`after_cursor = (name, id)`); `offset` applies
    /// only when no cursor is supplied.
    pub async fn list_async(&self, filter: ProductFilter) -> Result<Vec<Product>> {
        let mut builder = QueryBuilder::new("SELECT * FROM products WHERE 1=1");
        Self::push_list_filters(&mut builder, &filter);

        if let Some((cursor_name, cursor_id)) = &filter.after_cursor {
            let cursor_id: Uuid = cursor_id.parse().map_err(|e| {
                CommerceError::ValidationError(format!(
                    "invalid after_cursor id '{cursor_id}': {e}"
                ))
            })?;
            builder
                .push(" AND (name > ")
                .push_bind(cursor_name.clone())
                .push(" OR (name = ")
                .push_bind(cursor_name.clone())
                .push(" AND id > ")
                .push_bind(cursor_id)
                .push("))");
        }

        builder.push(" ORDER BY name ASC, id ASC");
        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if filter.after_cursor.is_none() {
            if let Some(offset) = filter.offset {
                builder.push(" OFFSET ").push_bind(i64::from(offset));
            }
        }

        let rows = builder
            .build_query_as::<ProductRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_product).collect()
    }

    /// Delete a product (async) — archives; refused while live references exist.
    pub async fn delete_async(&self, id: ProductId) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        Self::archive_product_tx(tx.as_mut(), id).await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    /// Add variant (public async)
    pub async fn add_variant_public_async(
        &self,
        product_id: ProductId,
        input: CreateProductVariant,
    ) -> Result<ProductVariant> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let variant =
            Self::insert_variant_tx(tx.as_mut(), product_id, &input, false, Utc::now()).await?;
        tx.commit().await.map_err(map_db_error)?;
        Ok(variant)
    }

    /// Get variant by ID (async)
    pub async fn get_variant_async(&self, id: Uuid) -> Result<Option<ProductVariant>> {
        let row = sqlx::query_as::<_, VariantRow>("SELECT * FROM product_variants WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_variant).transpose()
    }

    /// Get variant by SKU (async)
    pub async fn get_variant_by_sku_async(&self, sku: &str) -> Result<Option<ProductVariant>> {
        let row = sqlx::query_as::<_, VariantRow>("SELECT * FROM product_variants WHERE sku = $1")
            .bind(sku)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_variant).transpose()
    }

    /// Update variant (async)
    pub async fn update_variant_async(
        &self,
        id: Uuid,
        input: CreateProductVariant,
    ) -> Result<ProductVariant> {
        // Same contract as `insert_variant_tx`: SKU, amounts and compare-at.
        validate_sku(&input.sku)?;
        input.validate()?;
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let current: (i32, String) =
            sqlx::query_as("SELECT version, sku FROM product_variants WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_one(tx.as_mut())
                .await
                .map_err(|e| match e {
                    sqlx::Error::RowNotFound => CommerceError::ProductVariantNotFound(id),
                    e => map_db_error(e),
                })?;
        let (current_version, current_sku) = current;

        if current_sku != input.sku {
            let taken: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM product_variants WHERE sku = $1 AND id != $2)",
            )
            .bind(&input.sku)
            .bind(id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(map_db_error)?;
            if taken {
                return Err(CommerceError::DuplicateSku(input.sku));
            }
        }

        let options_json =
            serde_json::to_value(input.options.clone().unwrap_or_default()).unwrap_or_default();

        let result = sqlx::query(
            r#"
            UPDATE product_variants
            SET sku = $1, name = $2, price = $3, compare_at_price = $4, cost = $5,
                barcode = $6, weight = $7, weight_unit = $8, options = $9, updated_at = $10, version = version + 1
            WHERE id = $11 AND version = $12
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
        .bind(current_version)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if result.rows_affected() == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "product_variant".to_string(),
                id: id.to_string(),
                expected_version: current_version,
            });
        }

        let row = sqlx::query_as::<_, VariantRow>("SELECT * FROM product_variants WHERE id = $1")
            .bind(id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?
            .ok_or(CommerceError::ProductVariantNotFound(id))?;
        tx.commit().await.map_err(map_db_error)?;
        Self::row_to_variant(row)
    }

    /// Delete variant (async) — soft delete (`is_active = false`), matching
    /// SQLite; refused while live cart / order / reservation references exist.
    pub async fn delete_variant_async(&self, id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let row: Option<(String, bool)> =
            sqlx::query_as("SELECT sku, is_active FROM product_variants WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        let Some((sku, is_active)) = row else {
            return Ok(());
        };
        if !is_active {
            return Ok(());
        }

        sku_reference_counts_for_pg(tx.as_mut(), &sku)
            .await?
            .ensure_none("delete", &format!("variant {id} (SKU {sku})"))?;

        sqlx::query(
            "UPDATE product_variants SET is_active = false, updated_at = $1, version = version + 1 WHERE id = $2",
        )
        .bind(Utc::now())
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    /// Get all live variants for product (async)
    pub async fn get_variants_async(&self, product_id: ProductId) -> Result<Vec<ProductVariant>> {
        let rows = sqlx::query_as::<_, VariantRow>(
            "SELECT * FROM product_variants WHERE product_id = $1 AND is_active = true \
             ORDER BY is_default DESC, sku",
        )
        .bind(product_id.into_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_variant).collect()
    }

    /// Count products (async)
    pub async fn count_async(&self, filter: ProductFilter) -> Result<u64> {
        let mut builder = QueryBuilder::new("SELECT COUNT(*) FROM products WHERE 1=1");
        Self::push_list_filters(&mut builder, &filter);

        let count: (i64,) =
            builder.build_query_as().fetch_one(&self.pool).await.map_err(map_db_error)?;

        Ok(count.0 as u64)
    }

    // === Batch Operations (async) ===

    /// Create multiple products - partial success allowed (async)
    pub async fn create_batch_async(
        &self,
        inputs: Vec<CreateProduct>,
    ) -> Result<BatchResult<Product>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_async(input).await {
                Ok(product) => result.record_success(product),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    /// Create multiple products - atomic (all-or-nothing) (async)
    pub async fn create_batch_atomic_async(
        &self,
        inputs: Vec<CreateProduct>,
    ) -> Result<Vec<Product>> {
        validate_batch_size(&inputs)?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut products = Vec::with_capacity(inputs.len());

        for input in &inputs {
            products.push(Self::insert_product_tx(tx.as_mut(), input).await?);
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(products)
    }

    /// Update multiple products - partial success allowed (async)
    pub async fn update_batch_async(
        &self,
        updates: Vec<(ProductId, UpdateProduct)>,
    ) -> Result<BatchResult<Product>> {
        validate_batch_size(&updates)?;
        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update_async(id, input).await {
                Ok(product) => result.record_success(product),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Update multiple products - atomic (all-or-nothing) (async)
    pub async fn update_batch_atomic_async(
        &self,
        updates: Vec<(ProductId, UpdateProduct)>,
    ) -> Result<Vec<Product>> {
        validate_batch_size(&updates)?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut products = Vec::with_capacity(updates.len());

        for (id, input) in &updates {
            products.push(Self::update_product_tx(tx.as_mut(), *id, input).await?);
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(products)
    }

    /// Delete multiple products - partial success allowed (async)
    pub async fn delete_batch_async(&self, ids: Vec<ProductId>) -> Result<BatchResult<ProductId>> {
        validate_batch_size(&ids)?;
        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            match self.delete_async(id).await {
                Ok(()) => result.record_success(id),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Delete multiple products - atomic (all-or-nothing) (async)
    pub async fn delete_batch_atomic_async(&self, ids: Vec<ProductId>) -> Result<()> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        for id in &ids {
            Self::archive_product_tx(tx.as_mut(), *id).await?;
        }
        tx.commit().await.map_err(map_db_error)?;
        Ok(())
    }

    /// Get multiple products by ID (async)
    pub async fn get_batch_async(&self, ids: Vec<ProductId>) -> Result<Vec<Product>> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let uuid_ids: Vec<Uuid> = ids.iter().map(|id| id.into_uuid()).collect();
        let rows = sqlx::query_as::<_, ProductRow>("SELECT * FROM products WHERE id = ANY($1)")
            .bind(&uuid_ids)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_product).collect()
    }
}

impl ProductRepository for PgProductRepository {
    fn create(&self, input: CreateProduct) -> Result<Product> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: ProductId) -> Result<Option<Product>> {
        super::block_on(self.get_async(id))
    }

    fn get_by_slug(&self, slug: &str) -> Result<Option<Product>> {
        super::block_on(self.get_by_slug_async(slug))
    }

    fn update(&self, id: ProductId, input: UpdateProduct) -> Result<Product> {
        super::block_on(self.update_async(id, input))
    }

    fn list(&self, filter: ProductFilter) -> Result<Vec<Product>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: ProductId) -> Result<()> {
        super::block_on(self.delete_async(id))
    }

    fn add_variant(
        &self,
        product_id: ProductId,
        variant: CreateProductVariant,
    ) -> Result<ProductVariant> {
        super::block_on(self.add_variant_public_async(product_id, variant))
    }

    fn get_variant(&self, id: Uuid) -> Result<Option<ProductVariant>> {
        super::block_on(self.get_variant_async(id))
    }

    fn get_variant_by_sku(&self, sku: &str) -> Result<Option<ProductVariant>> {
        super::block_on(self.get_variant_by_sku_async(sku))
    }

    fn update_variant(&self, id: Uuid, variant: CreateProductVariant) -> Result<ProductVariant> {
        super::block_on(self.update_variant_async(id, variant))
    }

    fn delete_variant(&self, id: Uuid) -> Result<()> {
        super::block_on(self.delete_variant_async(id))
    }

    fn get_variants(&self, product_id: ProductId) -> Result<Vec<ProductVariant>> {
        super::block_on(self.get_variants_async(product_id))
    }

    fn count(&self, filter: ProductFilter) -> Result<u64> {
        super::block_on(self.count_async(filter))
    }

    // === Batch Operations ===

    fn create_batch(&self, inputs: Vec<CreateProduct>) -> Result<BatchResult<Product>> {
        super::block_on(self.create_batch_async(inputs))
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateProduct>) -> Result<Vec<Product>> {
        super::block_on(self.create_batch_atomic_async(inputs))
    }

    fn update_batch(
        &self,
        updates: Vec<(ProductId, UpdateProduct)>,
    ) -> Result<BatchResult<Product>> {
        super::block_on(self.update_batch_async(updates))
    }

    fn update_batch_atomic(
        &self,
        updates: Vec<(ProductId, UpdateProduct)>,
    ) -> Result<Vec<Product>> {
        super::block_on(self.update_batch_atomic_async(updates))
    }

    fn delete_batch(&self, ids: Vec<ProductId>) -> Result<BatchResult<ProductId>> {
        super::block_on(self.delete_batch_async(ids))
    }

    fn delete_batch_atomic(&self, ids: Vec<ProductId>) -> Result<()> {
        super::block_on(self.delete_batch_atomic_async(ids))
    }

    fn get_batch(&self, ids: Vec<ProductId>) -> Result<Vec<Product>> {
        super::block_on(self.get_batch_async(ids))
    }
}

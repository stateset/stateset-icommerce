//! SQLite product repository implementation

use super::{
    ConflictValues, build_in_clause, escape_like, json1_available, map_db_error, map_db_error_with,
    params_refs, parse_datetime_row, parse_decimal_opt_row, parse_decimal_row, parse_enum_row,
    parse_json_opt_row, parse_json_row, parse_uuid_row, uuid_params,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use stateset_core::{
    BatchResult, CommerceError, CreateProduct, CreateProductVariant, Product, ProductFilter,
    ProductId, ProductRepository, ProductStatus, ProductVariant, Result, UpdateProduct, Validate,
    VariantPurchasability, validate_batch_size, validate_sku,
};
use uuid::Uuid;

/// Order statuses that still count as "open" for the purpose of withdrawing a
/// SKU from sale: units have been promised but not all have left the building.
/// Mirrors `OrderStatus::allows_line_changes` plus `PartiallyShipped`.
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
    /// Lines on orders in an open status (pending / confirmed / processing /
    /// partially shipped).
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
/// fragment over the alias-free column `sku` with exactly one `?` parameter).
fn sku_reference_counts(
    conn: &rusqlite::Connection,
    sku_predicate: &str,
    param: &str,
) -> Result<SkuReferenceCounts> {
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
    let count = |sql: &str| -> Result<u64> {
        let n: i64 = conn.query_row(sql, [param], |row| row.get(0)).map_err(map_db_error)?;
        Ok(u64::try_from(n).unwrap_or_default())
    };
    Ok(SkuReferenceCounts {
        active_cart_lines: count(&cart_sql)?,
        open_order_lines: count(&order_sql)?,
        active_reservations: count(&reservation_sql)?,
    })
}

/// Live references to every SKU of `product_id` (active and inactive
/// variants alike — an inactive variant can still sit on an open order).
pub(crate) fn product_reference_counts(
    conn: &rusqlite::Connection,
    product_id: ProductId,
) -> Result<SkuReferenceCounts> {
    sku_reference_counts(
        conn,
        "IN (SELECT sku FROM product_variants WHERE product_id = ?)",
        &product_id.to_string(),
    )
}

/// Live references to a single SKU.
pub(crate) fn sku_reference_counts_for(
    conn: &rusqlite::Connection,
    sku: &str,
) -> Result<SkuReferenceCounts> {
    sku_reference_counts(conn, "= ?", sku)
}

/// Whether `sku` may be sold right now, using an existing connection so the
/// check can run inside the caller's transaction.
///
/// Intended for `carts.rs::add_item` (and order line creation): call it with
/// the write transaction before inserting the line and refuse via
/// [`VariantPurchasability::ensure_sellable`]. A SKU that is not in the
/// catalogue at all reports `NotInCatalog`, which `is_sellable()` treats as
/// allowed so ad-hoc lines keep working.
pub(crate) fn variant_is_purchasable_with_conn(
    conn: &rusqlite::Connection,
    sku: &str,
) -> Result<VariantPurchasability> {
    let row: Option<(i32, String)> = conn
        .query_row(
            "SELECT pv.is_active, p.status FROM product_variants pv \
             JOIN products p ON p.id = pv.product_id WHERE pv.sku = ?",
            [sku],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_db_error)?;
    let Some((is_active, status)) = row else {
        return Ok(VariantPurchasability::NotInCatalog);
    };
    if is_active == 0 {
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

/// Convert a price-filter bound to the `f64` the SQL predicate compares
/// against.
///
/// Only ever used for a range comparison against `CAST(price AS REAL)`; the
/// prices themselves are read back as exact `Decimal`s from the TEXT column.
/// A bound that does not fit a float at all (a `Decimal` whose magnitude
/// exceeds `f64`) is a bad request rather than a silent `inf`.
fn price_bound(bound: rust_decimal::Decimal, field: &str) -> Result<f64> {
    use rust_decimal::prelude::ToPrimitive;
    bound.to_f64().filter(|value| value.is_finite()).ok_or_else(|| {
        CommerceError::ValidationError(format!("{field} {bound} is not a comparable amount"))
    })
}

/// SQLite implementation of `ProductRepository`
#[derive(Debug)]
pub struct SqliteProductRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteProductRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    /// Whether `sku` may currently be sold (see `variant_is_purchasable_with_conn`).
    pub fn variant_purchasability(&self, sku: &str) -> Result<VariantPurchasability> {
        let conn = self.conn()?;
        variant_is_purchasable_with_conn(&conn, sku)
    }

    /// Live cart / order / reservation references to the product's SKUs.
    pub fn reference_counts(&self, product_id: ProductId) -> Result<SkuReferenceCounts> {
        let conn = self.conn()?;
        product_reference_counts(&conn, product_id)
    }

    /// All variants of a product including soft-deleted (`is_active = 0`) rows.
    ///
    /// [`ProductRepository::get_variants`] returns live variants only; use this
    /// when reconciling historical order lines against withdrawn SKUs.
    pub fn get_variants_including_inactive(
        &self,
        product_id: ProductId,
    ) -> Result<Vec<ProductVariant>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT * FROM product_variants WHERE product_id = ? ORDER BY is_default DESC, sku",
            )
            .map_err(map_db_error)?;
        let variants = stmt
            .query_map([product_id.to_string()], Self::row_to_variant)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;
        Ok(variants)
    }

    fn row_to_product(row: &rusqlite::Row<'_>) -> rusqlite::Result<Product> {
        let attributes_json: String = row.get("attributes")?;
        let seo_json: Option<String> = row.get("seo")?;

        Ok(Product {
            id: ProductId::from(parse_uuid_row(&row.get::<_, String>("id")?, "product", "id")?),
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

    fn row_to_variant(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProductVariant> {
        let options_json: String = row.get("options")?;

        Ok(ProductVariant {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "product_variant", "id")?,
            product_id: ProductId::from(parse_uuid_row(
                &row.get::<_, String>("product_id")?,
                "product_variant",
                "product_id",
            )?),
            sku: row.get("sku")?,
            name: row.get("name")?,
            price: parse_decimal_row(&row.get::<_, String>("price")?, "product_variant", "price")?,
            compare_at_price: parse_decimal_opt_row(
                row.get::<_, Option<String>>("compare_at_price")?,
                "product_variant",
                "compare_at_price",
            )?,
            cost: parse_decimal_opt_row(
                row.get::<_, Option<String>>("cost")?,
                "product_variant",
                "cost",
            )?,
            barcode: row.get("barcode")?,
            weight: parse_decimal_opt_row(
                row.get::<_, Option<String>>("weight")?,
                "product_variant",
                "weight",
            )?,
            weight_unit: row.get("weight_unit")?,
            options: parse_json_row(&options_json, "product_variant", "options")?,
            is_default: row.get::<_, i32>("is_default")? != 0,
            is_active: row.get::<_, i32>("is_active")? != 0,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "product_variant",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "product_variant",
                "updated_at",
            )?,
        })
    }

    /// Insert one product (and its inline variants) on an open transaction.
    ///
    /// Shared by [`ProductRepository::create`] and
    /// [`ProductRepository::create_batch_atomic`] so both paths perform the
    /// same slug / SKU checks inside the same write lock.
    fn insert_product_tx(tx: &rusqlite::Transaction<'_>, input: &CreateProduct) -> Result<Product> {
        let id = ProductId::new();
        let now = Utc::now();
        let slug = input.slug.clone().unwrap_or_else(|| Product::generate_slug(&input.name));
        let name = input.name.clone();
        let description = input.description.clone().unwrap_or_default();
        let product_type = input.product_type.unwrap_or_default();
        let attributes = input.attributes.clone().unwrap_or_default();
        let seo = input.seo.clone();

        // Validate every inline variant before touching the database so a bad
        // second variant cannot leave a half-created product behind. The full
        // `Validate` impl (not just the SKU) runs here so a caller holding the
        // repository directly cannot store a negative price, a compare-at
        // price below the price, or an amount finer than
        // `VARIANT_MONEY_SCALE`.
        if let Some(variants) = &input.variants {
            for variant in variants {
                // `validate_sku` first so a malformed SKU keeps its historical
                // `ValidationError`; `validate` then covers the amounts.
                validate_sku(&variant.sku)?;
                variant.validate()?;
            }
        }

        // Check slug uniqueness
        let exists: i32 = tx
            .query_row("SELECT COUNT(*) FROM products WHERE slug = ?", [&slug], |row| row.get(0))
            .map_err(map_db_error)?;

        if exists > 0 {
            return Err(CommerceError::DuplicateSlug(slug));
        }

        let attributes_json = serde_json::to_string(&attributes).unwrap_or_default();
        let seo_json = seo.as_ref().map(|s| serde_json::to_string(s).unwrap_or_default());

        tx.execute(
            "INSERT INTO products (id, name, slug, description, status, product_type, attributes, seo, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                &name,
                &slug,
                &description,
                ProductStatus::Draft.to_string(),
                product_type.to_string(),
                attributes_json,
                seo_json,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(|e| map_db_error_with(e, ConflictValues::slug(&slug)))?;

        // Create variants inline if provided (using the same connection)
        if let Some(variants) = &input.variants {
            for (i, variant) in variants.iter().enumerate() {
                let variant_id = Uuid::new_v4();

                // Check SKU uniqueness
                let sku_exists: i32 = tx
                    .query_row(
                        "SELECT COUNT(*) FROM product_variants WHERE sku = ?",
                        [&variant.sku],
                        |row| row.get(0),
                    )
                    .map_err(map_db_error)?;

                if sku_exists > 0 {
                    return Err(CommerceError::DuplicateSku(variant.sku.clone()));
                }

                let options_json =
                    serde_json::to_string(&variant.options.clone().unwrap_or_default())
                        .unwrap_or_default();

                tx.execute(
                    "INSERT INTO product_variants (id, product_id, sku, name, price, compare_at_price, cost,
                                                   barcode, weight, weight_unit, options, is_default, is_active,
                                                   created_at, updated_at)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
                    rusqlite::params![
                        variant_id.to_string(),
                        id.to_string(),
                        &variant.sku,
                        variant.name.as_ref().unwrap_or(&variant.sku),
                        variant.price.to_string(),
                        variant.compare_at_price.map(|d| d.to_string()),
                        variant.cost.map(|d| d.to_string()),
                        &variant.barcode,
                        variant.weight.map(|d| d.to_string()),
                        &variant.weight_unit,
                        options_json,
                        i32::from(variant.is_default.unwrap_or(i == 0)), // First variant is default
                        now.to_rfc3339(),
                        now.to_rfc3339(),
                    ],
                )
                .map_err(|e| map_db_error_with(e, ConflictValues::sku(&variant.sku)))?;
            }
        }

        Ok(Product {
            id,
            name,
            slug,
            description,
            status: ProductStatus::Draft,
            product_type,
            attributes,
            seo,
            created_at: now,
            updated_at: now,
        })
    }

    /// Apply a partial update on an open transaction.
    ///
    /// Shared by [`ProductRepository::update`] and
    /// [`ProductRepository::update_batch_atomic`]. Enforces the
    /// [`ProductStatus`] state machine and refuses to archive a product whose
    /// SKUs are still referenced by an active cart, open order or reservation.
    fn update_product_tx(
        tx: &rusqlite::Transaction<'_>,
        id: ProductId,
        input: &UpdateProduct,
    ) -> Result<Product> {
        let now = Utc::now();
        let (current_version, current_status): (i32, String) = tx
            .query_row(
                "SELECT version, status FROM products WHERE id = ?",
                [id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    CommerceError::ProductNotFound(id.into_uuid())
                }
                e => map_db_error(e),
            })?;
        let current_status: ProductStatus =
            parse_enum_row(&current_status, "product", "status").map_err(map_db_error)?;

        let mut updates = vec!["updated_at = ?"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

        if let Some(name) = &input.name {
            updates.push("name = ?");
            params.push(Box::new(name.clone()));
        }
        if let Some(slug) = &input.slug {
            let existing_id: Option<String> = tx
                .query_row("SELECT id FROM products WHERE slug = ?", [slug], |row| row.get(0))
                .optional()
                .map_err(map_db_error)?;
            if let Some(existing_id) = existing_id {
                if existing_id != id.to_string() {
                    return Err(CommerceError::DuplicateSlug(slug.clone()));
                }
            }
            updates.push("slug = ?");
            params.push(Box::new(slug.clone()));
        }
        if let Some(description) = &input.description {
            updates.push("description = ?");
            params.push(Box::new(description.clone()));
        }
        if let Some(status) = input.status {
            current_status.ensure_can_transition_to(status)?;
            // Any move out of `Active` — archiving OR unpublishing back to
            // `Draft` — withdraws the SKUs from sale, so both need the same
            // live-reference guard; without it, unpublishing left carts
            // holding a SKU checkout would still accept.
            if let Some(action) = current_status.withdrawal_action(status) {
                product_reference_counts(tx, id)?.ensure_none(action, &format!("product {id}"))?;
            }
            updates.push("status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(attributes) = &input.attributes {
            updates.push("attributes = ?");
            params.push(Box::new(serde_json::to_string(attributes).unwrap_or_default()));
        }
        if let Some(seo) = &input.seo {
            updates.push("seo = ?");
            params.push(Box::new(serde_json::to_string(seo).unwrap_or_default()));
        }

        updates.push("version = version + 1");
        params.push(Box::new(id.to_string()));
        params.push(Box::new(current_version));

        let sql =
            format!("UPDATE products SET {} WHERE id = ? AND version = ?", updates.join(", "));
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();

        let rows_affected = tx.execute(&sql, params_refs.as_slice()).map_err(|e| {
            map_db_error_with(
                e,
                ConflictValues { slug: input.slug.as_deref(), ..ConflictValues::default() },
            )
        })?;
        if rows_affected == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "product".to_string(),
                id: id.to_string(),
                expected_version: current_version,
            });
        }

        tx.query_row("SELECT * FROM products WHERE id = ?", [id.to_string()], Self::row_to_product)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    CommerceError::ProductNotFound(id.into_uuid())
                }
                e => map_db_error(e),
            })
    }

    /// Append the shared [`ProductFilter`] predicates — everything except
    /// ordering and pagination — so `list` and `count` can never disagree
    /// about what a filter means.
    ///
    /// Every predicate is expressed in SQL (including the price range, which
    /// used to be applied in Rust after loading every matching row), so the
    /// database can use its indexes and pagination applies to the filtered
    /// set.
    fn push_list_filters(
        sql: &mut String,
        params: &mut Vec<Box<dyn rusqlite::ToSql>>,
        filter: &ProductFilter,
        use_json: bool,
    ) -> Result<()> {
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        } else {
            sql.push_str(" AND status != 'archived'");
        }
        if let Some(product_type) = filter.product_type {
            sql.push_str(" AND product_type = ?");
            params.push(Box::new(product_type.to_string()));
        }
        if let Some(search) = &filter.search {
            sql.push_str(" AND (name LIKE ? ESCAPE '\\' OR description LIKE ? ESCAPE '\\')");
            let escaped = format!("%{}%", escape_like(search));
            params.push(Box::new(escaped.clone()));
            params.push(Box::new(escaped));
        }
        if let Some(category) = &filter.category {
            if use_json {
                sql.push_str(
                    " AND EXISTS (SELECT 1 FROM json_each(attributes) attr \
                     WHERE (json_extract(attr.value, '$.name') = 'category' \
                        OR json_extract(attr.value, '$.group') = 'category') \
                       AND json_extract(attr.value, '$.value') = ?)",
                );
                params.push(Box::new(category.clone()));
            } else {
                sql.push_str(" AND (attributes LIKE ? OR attributes LIKE ?)");
                params.push(Box::new(format!("%\"name\":\"category\",\"value\":\"{category}\"%")));
                params.push(Box::new(format!("%\"value\":\"{category}\",\"group\":\"category\"%")));
            }
        }
        // Price range, filtered in SQL exactly as the Postgres backend does.
        //
        // `price` is exact TEXT on this backend, so the *predicate* casts to
        // REAL. That is a range comparison only — no money value is ever read
        // back through the cast — and both sides go through the same
        // `Decimal -> f64` rounding, so a bound equal to a stored price still
        // matches. `CreateProductVariant::validate` caps amounts at
        // `VARIANT_MONEY_SCALE` (4 dp), well inside the range where that
        // rounding is order-preserving.
        if filter.min_price.is_some() || filter.max_price.is_some() {
            sql.push_str(
                " AND EXISTS (SELECT 1 FROM product_variants pv_price \
                 WHERE pv_price.product_id = products.id AND pv_price.is_active = 1",
            );
            if let Some(min_price) = filter.min_price {
                sql.push_str(" AND CAST(pv_price.price AS REAL) >= ?");
                params.push(Box::new(price_bound(min_price, "min_price")?));
            }
            if let Some(max_price) = filter.max_price {
                sql.push_str(" AND CAST(pv_price.price AS REAL) <= ?");
                params.push(Box::new(price_bound(max_price, "max_price")?));
            }
            sql.push(')');
        }
        if let Some(in_stock) = filter.in_stock {
            let stock_clause = if in_stock { "EXISTS" } else { "NOT EXISTS" };
            sql.push_str(&format!(
                " AND {stock_clause} (SELECT 1 FROM product_variants pv_stock \
                 JOIN inventory_items ii ON ii.sku = pv_stock.sku \
                 JOIN inventory_balances ib ON ib.item_id = ii.id \
                 WHERE pv_stock.product_id = products.id \
                   AND pv_stock.is_active = 1 \
                   AND CAST(ib.quantity_available AS REAL) > 0)"
            ));
        }
        Ok(())
    }

    /// Archive one product on an open transaction, refusing while live
    /// references exist. Archiving an already-archived or unknown product is a
    /// no-op so deletes stay idempotent.
    fn archive_product_tx(tx: &rusqlite::Transaction<'_>, id: ProductId) -> Result<()> {
        let status: Option<String> = tx
            .query_row("SELECT status FROM products WHERE id = ?", [id.to_string()], |row| {
                row.get(0)
            })
            .optional()
            .map_err(map_db_error)?;
        let Some(status) = status else {
            return Ok(());
        };
        if status == ProductStatus::Archived.to_string() {
            return Ok(());
        }
        product_reference_counts(tx, id)?.ensure_none("archive", &format!("product {id}"))?;
        tx.execute(
            "UPDATE products SET status = 'archived', updated_at = ?, version = version + 1 WHERE id = ?",
            rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;
        Ok(())
    }
}

impl ProductRepository for SqliteProductRepository {
    fn create(&self, input: CreateProduct) -> Result<Product> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let product = Self::insert_product_tx(&tx, &input)?;
        tx.commit().map_err(map_db_error)?;
        Ok(product)
    }

    fn get(&self, id: ProductId) -> Result<Option<Product>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM products WHERE id = ?",
            [id.to_string()],
            Self::row_to_product,
        );

        match result {
            Ok(product) => Ok(Some(product)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_by_slug(&self, slug: &str) -> Result<Option<Product>> {
        let conn = self.conn()?;
        let result =
            conn.query_row("SELECT * FROM products WHERE slug = ?", [slug], Self::row_to_product);

        match result {
            Ok(product) => Ok(Some(product)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: ProductId, input: UpdateProduct) -> Result<Product> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let product = Self::update_product_tx(&tx, id, &input)?;
        tx.commit().map_err(map_db_error)?;
        Ok(product)
    }

    fn list(&self, filter: ProductFilter) -> Result<Vec<Product>> {
        let conn = self.conn()?;
        let use_json = json1_available(&conn);
        let mut sql = "SELECT * FROM products WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];
        Self::push_list_filters(&mut sql, &mut params, &filter, use_json)?;

        // Keyset cursor: (name, id) for stable ASC ordering
        if let Some((cursor_name, cursor_id)) = &filter.after_cursor {
            sql.push_str(" AND (name > ? OR (name = ? AND id > ?))");
            params.push(Box::new(cursor_name.clone()));
            params.push(Box::new(cursor_name.clone()));
            params.push(Box::new(cursor_id.clone()));
        }

        sql.push_str(" ORDER BY name ASC, id ASC");

        // Offset pagination applies only in non-cursor mode — the cursor
        // already encodes the position, and re-applying the offset on top of
        // it skipped a whole page. The helper emits `LIMIT -1 OFFSET n` when
        // an offset is set without a limit, because SQLite rejects a bare
        // OFFSET.
        let page_offset = if filter.after_cursor.is_none() { filter.offset } else { None };
        crate::sqlite::append_limit_offset(&mut sql, filter.limit, page_offset);

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;

        let products = stmt
            .query_map(params_refs.as_slice(), Self::row_to_product)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(products)
    }

    fn delete(&self, id: ProductId) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        Self::archive_product_tx(&tx, id)?;
        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn add_variant(
        &self,
        product_id: ProductId,
        variant: CreateProductVariant,
    ) -> Result<ProductVariant> {
        // Validate SKU format, amounts and the compare-at relationship. The
        // embedded facade validates too, but the repository is a public API of
        // its own and must not accept money it cannot honour. `validate_sku`
        // runs first so a malformed SKU keeps its historical `ValidationError`.
        validate_sku(&variant.sku)?;
        variant.validate()?;

        let mut conn = self.conn()?;
        let id = Uuid::new_v4();
        let now = Utc::now();
        let sku = variant.sku.clone();
        let name = variant.name.clone().unwrap_or_else(|| sku.clone());
        let options = variant.options.clone().unwrap_or_default();

        // Check + insert under one IMMEDIATE transaction so two concurrent
        // callers cannot both pass the pre-check; the UNIQUE index (mapped to
        // `DuplicateSku` by `map_db_error`) is the backstop.
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        let product_exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM products WHERE id = ?)",
                [product_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;
        if !product_exists {
            return Err(CommerceError::ProductNotFound(product_id.into_uuid()));
        }

        // Check SKU uniqueness
        let exists: i32 = tx
            .query_row("SELECT COUNT(*) FROM product_variants WHERE sku = ?", [&sku], |row| {
                row.get(0)
            })
            .map_err(map_db_error)?;

        if exists > 0 {
            return Err(CommerceError::DuplicateSku(sku));
        }

        let options_json = serde_json::to_string(&options).unwrap_or_default();

        tx.execute(
            "INSERT INTO product_variants (id, product_id, sku, name, price, compare_at_price, cost,
                                           barcode, weight, weight_unit, options, is_default, is_active,
                                           created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
            rusqlite::params![
                id.to_string(),
                product_id.to_string(),
                &sku,
                &name,
                variant.price.to_string(),
                variant.compare_at_price.map(|d| d.to_string()),
                variant.cost.map(|d| d.to_string()),
                &variant.barcode,
                variant.weight.map(|d| d.to_string()),
                &variant.weight_unit,
                options_json,
                i32::from(variant.is_default.unwrap_or(false)),
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(|e| map_db_error_with(e, ConflictValues::sku(&sku)))?;

        tx.commit().map_err(map_db_error)?;

        Ok(ProductVariant {
            id,
            product_id,
            sku,
            name,
            price: variant.price,
            compare_at_price: variant.compare_at_price,
            cost: variant.cost,
            barcode: variant.barcode,
            weight: variant.weight,
            weight_unit: variant.weight_unit,
            options,
            is_default: variant.is_default.unwrap_or(false),
            is_active: true,
            created_at: now,
            updated_at: now,
        })
    }

    fn get_variant(&self, id: Uuid) -> Result<Option<ProductVariant>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM product_variants WHERE id = ?",
            [id.to_string()],
            Self::row_to_variant,
        );

        match result {
            Ok(variant) => Ok(Some(variant)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_variant_by_sku(&self, sku: &str) -> Result<Option<ProductVariant>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM product_variants WHERE sku = ?",
            [sku],
            Self::row_to_variant,
        );

        match result {
            Ok(variant) => Ok(Some(variant)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update_variant(&self, id: Uuid, variant: CreateProductVariant) -> Result<ProductVariant> {
        // Same contract as `add_variant`: SKU, amounts and compare-at.
        validate_sku(&variant.sku)?;
        variant.validate()?;

        let mut conn = self.conn()?;
        let now = Utc::now();
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        let (current_version, current_sku): (i32, String) = tx
            .query_row(
                "SELECT version, sku FROM product_variants WHERE id = ?",
                [id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => CommerceError::ProductVariantNotFound(id),
                e => map_db_error(e),
            })?;

        // A SKU change must not collide with another variant (parity with the
        // Postgres backend, which always writes the SKU column).
        if current_sku != variant.sku {
            let taken: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM product_variants WHERE sku = ? AND id != ?)",
                    rusqlite::params![&variant.sku, id.to_string()],
                    |row| row.get(0),
                )
                .map_err(map_db_error)?;
            if taken {
                return Err(CommerceError::DuplicateSku(variant.sku));
            }
        }

        let options_json =
            serde_json::to_string(&variant.options.clone().unwrap_or_default()).unwrap_or_default();

        let rows_affected = tx.execute(
            "UPDATE product_variants SET sku = ?, name = ?, price = ?, compare_at_price = ?, cost = ?,
                     barcode = ?, weight = ?, weight_unit = ?, options = ?, updated_at = ?, version = version + 1
             WHERE id = ? AND version = ?",
            rusqlite::params![
                &variant.sku,
                variant.name.as_ref().unwrap_or(&variant.sku),
                variant.price.to_string(),
                variant.compare_at_price.map(|d| d.to_string()),
                variant.cost.map(|d| d.to_string()),
                &variant.barcode,
                variant.weight.map(|d| d.to_string()),
                &variant.weight_unit,
                options_json,
                now.to_rfc3339(),
                id.to_string(),
                current_version,
            ],
        )
        .map_err(|e| map_db_error_with(e, ConflictValues::sku(&variant.sku)))?;
        if rows_affected == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "product_variant".to_string(),
                id: id.to_string(),
                expected_version: current_version,
            });
        }

        let updated = tx
            .query_row(
                "SELECT * FROM product_variants WHERE id = ?",
                [id.to_string()],
                Self::row_to_variant,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => CommerceError::ProductVariantNotFound(id),
                e => map_db_error(e),
            })?;
        tx.commit().map_err(map_db_error)?;
        Ok(updated)
    }

    fn delete_variant(&self, id: Uuid) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        let row: Option<(String, i32)> = tx
            .query_row(
                "SELECT sku, is_active FROM product_variants WHERE id = ?",
                [id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(map_db_error)?;
        // Deleting an unknown or already-inactive variant is a no-op.
        let Some((sku, is_active)) = row else {
            return Ok(());
        };
        if is_active == 0 {
            return Ok(());
        }

        sku_reference_counts_for(&tx, &sku)?
            .ensure_none("delete", &format!("variant {id} (SKU {sku})"))?;

        tx.execute(
            "UPDATE product_variants SET is_active = 0, updated_at = ?, version = version + 1 WHERE id = ?",
            rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;
        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn get_variants(&self, product_id: ProductId) -> Result<Vec<ProductVariant>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM product_variants WHERE product_id = ? AND is_active = 1 ORDER BY is_default DESC, sku")
            .map_err(map_db_error)?;

        let variants = stmt
            .query_map([product_id.to_string()], Self::row_to_variant)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(variants)
    }

    fn count(&self, filter: ProductFilter) -> Result<u64> {
        let conn = self.conn()?;
        let use_json = json1_available(&conn);
        let mut sql = "SELECT COUNT(*) FROM products WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];
        // Same predicates as `list` (pagination excluded), counted in SQL
        // instead of by loading every id and its variants.
        Self::push_list_filters(&mut sql, &mut params, &filter, use_json)?;

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let count: i64 =
            conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0)).map_err(map_db_error)?;
        Ok(u64::try_from(count).unwrap_or_default())
    }

    // === Batch Operations ===

    fn create_batch(&self, inputs: Vec<CreateProduct>) -> Result<BatchResult<Product>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create(input) {
                Ok(product) => result.record_success(product),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateProduct>) -> Result<Vec<Product>> {
        validate_batch_size(&inputs)?;
        if inputs.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let mut results = Vec::with_capacity(inputs.len());

        for input in &inputs {
            results.push(Self::insert_product_tx(&tx, input)?);
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn update_batch(
        &self,
        updates: Vec<(ProductId, UpdateProduct)>,
    ) -> Result<BatchResult<Product>> {
        validate_batch_size(&updates)?;
        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update(id, input) {
                Ok(product) => result.record_success(product),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn update_batch_atomic(
        &self,
        updates: Vec<(ProductId, UpdateProduct)>,
    ) -> Result<Vec<Product>> {
        validate_batch_size(&updates)?;
        if updates.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let mut results = Vec::with_capacity(updates.len());

        for (id, input) in &updates {
            results.push(Self::update_product_tx(&tx, *id, input)?);
        }

        tx.commit().map_err(map_db_error)?;
        Ok(results)
    }

    fn delete_batch(&self, ids: Vec<ProductId>) -> Result<BatchResult<ProductId>> {
        validate_batch_size(&ids)?;
        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            match self.delete(id) {
                Ok(()) => result.record_success(id),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn delete_batch_atomic(&self, ids: Vec<ProductId>) -> Result<()> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.conn()?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;

        // Archive products (soft delete) one by one so every member gets the
        // same live-reference guard; the transaction keeps it all-or-nothing.
        for id in &ids {
            Self::archive_product_tx(&tx, *id)?;
        }

        tx.commit().map_err(map_db_error)?;
        Ok(())
    }

    fn get_batch(&self, ids: Vec<ProductId>) -> Result<Vec<Product>> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let conn = self.conn()?;
        let placeholders = build_in_clause(ids.len());
        let sql = format!("SELECT * FROM products WHERE id IN ({placeholders})");

        let uuid_ids: Vec<Uuid> = ids.iter().map(|id| id.into_uuid()).collect();
        let params = uuid_params(&uuid_ids);
        let params_refs = params_refs(&params);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let products = stmt
            .query_map(params_refs.as_slice(), Self::row_to_product)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(products)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::{
        CreateProduct, CreateProductVariant, ProductFilter, ProductRepository, ProductStatus,
        UpdateProduct,
    };

    fn fresh_repo() -> SqliteProductRepository {
        SqliteDatabase::in_memory().expect("in-memory").products()
    }

    fn make_product(repo: &SqliteProductRepository, name: &str, slug: &str) -> Product {
        repo.create(CreateProduct {
            name: name.into(),
            slug: Some(slug.into()),
            description: Some(format!("Description for {name}")),
            product_type: None,
            attributes: None,
            seo: None,
            variants: Some(vec![CreateProductVariant {
                sku: format!("SKU-{slug}"),
                name: Some("Default".into()),
                price: dec!(19.99),
                is_default: Some(true),
                ..Default::default()
            }]),
        })
        .expect("create product")
    }

    #[test]
    fn create_product_round_trips_with_default_variant() {
        let repo = fresh_repo();
        let p = make_product(&repo, "Widget", "widget");
        assert_eq!(p.name, "Widget");
        assert_eq!(p.slug, "widget");

        let by_id = repo.get(p.id).expect("ok").expect("found");
        assert_eq!(by_id.id, p.id);
        let by_slug = repo.get_by_slug("widget").expect("ok").expect("found");
        assert_eq!(by_slug.id, p.id);
        assert!(repo.get_by_slug("missing-slug").expect("ok").is_none());

        let variants = repo.get_variants(p.id).expect("variants");
        assert_eq!(variants.len(), 1);
    }

    #[test]
    fn update_product_changes_name_and_status() {
        let repo = fresh_repo();
        let p = make_product(&repo, "Original", "original");
        let updated = repo
            .update(
                p.id,
                UpdateProduct {
                    name: Some("Renamed".into()),
                    status: Some(ProductStatus::Archived),
                    ..Default::default()
                },
            )
            .expect("update");
        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.status, ProductStatus::Archived);
    }

    #[test]
    fn list_filters_by_status() {
        let repo = fresh_repo();
        // Newly-created products default to ProductStatus::Draft.
        let draft = make_product(&repo, "Draft", "draft-prod");
        let to_archive = make_product(&repo, "ToArchive", "to-archive");
        repo.update(
            to_archive.id,
            UpdateProduct { status: Some(ProductStatus::Archived), ..Default::default() },
        )
        .expect("archive");

        let drafts = repo
            .list(ProductFilter { status: Some(ProductStatus::Draft), ..Default::default() })
            .expect("draft");
        let archived = repo
            .list(ProductFilter { status: Some(ProductStatus::Archived), ..Default::default() })
            .expect("archived");
        assert!(drafts.iter().any(|p| p.id == draft.id));
        assert!(archived.iter().any(|p| p.id == to_archive.id));
    }

    #[test]
    fn delete_removes_product() {
        let repo = fresh_repo();
        let p = make_product(&repo, "DelMe", "del-me");
        repo.delete(p.id).expect("delete");
        if let Some(found) = repo.get(p.id).expect("ok") {
            assert_ne!(found.status, ProductStatus::Active, "deleted product should not be Active");
        }
    }

    #[test]
    fn get_variant_by_sku_round_trips() {
        let repo = fresh_repo();
        let p = make_product(&repo, "VarTest", "var-test");
        let variants = repo.get_variants(p.id).expect("ok");
        let v = variants.first().expect("default variant exists");
        let by_sku = repo.get_variant_by_sku(&v.sku).expect("ok").expect("found");
        assert_eq!(by_sku.id, v.id);
        assert!(repo.get_variant_by_sku("missing-sku").expect("ok").is_none());
    }

    #[test]
    fn create_batch_returns_per_input_results() {
        let repo = fresh_repo();
        let mk = |name: &str, slug: &str| CreateProduct {
            name: name.into(),
            slug: Some(slug.into()),
            description: None,
            product_type: None,
            attributes: None,
            seo: None,
            variants: Some(vec![CreateProductVariant {
                sku: format!("SKU-B-{slug}"),
                price: dec!(1),
                is_default: Some(true),
                ..Default::default()
            }]),
        };
        let result = repo
            .create_batch(vec![mk("A", "a-batch"), mk("B", "b-batch"), mk("C", "c-batch")])
            .expect("batch");
        assert_eq!(result.success_count, 3);
        assert_eq!(result.failure_count, 0);
    }

    #[test]
    fn get_unknown_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get(stateset_core::ProductId::new()).expect("ok").is_none());
    }

    // ------------------------------------------------------------------
    // Round-5 hardening: state machine, references, atomicity, typed conflicts
    // ------------------------------------------------------------------

    #[test]
    fn archived_product_cannot_be_reactivated() {
        let repo = fresh_repo();
        let p = make_product(&repo, "Gone", "gone");
        repo.delete(p.id).expect("archive");
        let err = repo
            .update(
                p.id,
                UpdateProduct { status: Some(ProductStatus::Active), ..Default::default() },
            )
            .expect_err("Archived -> Active must be refused");
        assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
        // Idempotent re-archive is fine.
        repo.delete(p.id).expect("re-archive is a no-op");
    }

    #[test]
    fn add_variant_to_unknown_product_is_product_not_found() {
        let repo = fresh_repo();
        let err = repo
            .add_variant(
                ProductId::new(),
                CreateProductVariant {
                    sku: "ORPHAN-1".into(),
                    price: dec!(1),
                    ..Default::default()
                },
            )
            .expect_err("missing product");
        assert!(matches!(err, CommerceError::ProductNotFound(_)), "{err:?}");
    }

    #[test]
    fn add_variant_duplicate_sku_is_typed() {
        let repo = fresh_repo();
        let p = make_product(&repo, "Dup", "dup");
        let err = repo
            .add_variant(
                p.id,
                CreateProductVariant {
                    sku: "SKU-dup".into(),
                    price: dec!(1),
                    ..Default::default()
                },
            )
            .expect_err("duplicate sku");
        assert!(matches!(err, CommerceError::DuplicateSku(_)), "{err:?}");
    }

    #[test]
    fn inline_variant_with_invalid_sku_leaves_no_half_product() {
        let repo = fresh_repo();
        let err = repo
            .create(CreateProduct {
                name: "Half".into(),
                slug: Some("half".into()),
                variants: Some(vec![
                    CreateProductVariant {
                        sku: "GOOD-1".into(),
                        price: dec!(1),
                        ..Default::default()
                    },
                    CreateProductVariant {
                        sku: "bad sku".into(),
                        price: dec!(1),
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            })
            .expect_err("invalid sku");
        assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
        assert!(repo.get_by_slug("half").expect("ok").is_none(), "product must be rolled back");
        assert!(repo.get_variant_by_sku("GOOD-1").expect("ok").is_none());
    }

    #[test]
    fn delete_variant_is_soft_and_filtered_from_get_variants() {
        let repo = fresh_repo();
        let p = make_product(&repo, "Soft", "soft");
        let v = repo.get_variants(p.id).expect("ok").remove(0);
        repo.delete_variant(v.id).expect("delete");
        assert!(repo.get_variants(p.id).expect("ok").is_empty());
        let all = repo.get_variants_including_inactive(p.id).expect("ok");
        assert_eq!(all.len(), 1);
        assert!(!all[0].is_active);
        assert_eq!(
            repo.variant_purchasability("SKU-soft").expect("ok"),
            VariantPurchasability::VariantInactive
        );
        // Idempotent.
        repo.delete_variant(v.id).expect("second delete is a no-op");
    }

    fn seed_active_cart_line(db: &SqliteDatabase, sku: &str) {
        let conn = db.pool().get().expect("conn");
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO carts (id, cart_number, status, created_at, updated_at) VALUES ('cart-1', 'C-1', 'active', ?, ?)",
            [&now, &now],
        )
        .expect("cart");
        conn.execute(
            "INSERT INTO cart_items (id, cart_id, sku, name, quantity, unit_price, total, created_at, updated_at)
             VALUES ('ci-1', 'cart-1', ?, 'x', 1, '1', '1', ?, ?)",
            rusqlite::params![sku, &now, &now],
        )
        .expect("cart item");
    }

    #[test]
    fn archive_refuses_while_active_cart_references_sku() {
        let db = SqliteDatabase::in_memory().expect("db");
        let repo = db.products();
        let p = make_product(&repo, "InCart", "in-cart");
        seed_active_cart_line(&db, "SKU-in-cart");

        let err = repo.delete(p.id).expect_err("archive must be refused");
        match err {
            CommerceError::Conflict(msg) => assert!(msg.contains("1 active cart line"), "{msg}"),
            other => panic!("expected Conflict, got {other:?}"),
        }
        let err = repo
            .update(
                p.id,
                UpdateProduct { status: Some(ProductStatus::Archived), ..Default::default() },
            )
            .expect_err("status update to archived must be refused too");
        assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
        let v = repo.get_variants(p.id).expect("ok").remove(0);
        let err = repo.delete_variant(v.id).expect_err("variant delete refused");
        assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
        assert_eq!(repo.get(p.id).expect("ok").expect("found").status, ProductStatus::Draft);

        // Abandon the cart: archive now succeeds.
        db.pool()
            .get()
            .expect("conn")
            .execute("UPDATE carts SET status = 'abandoned'", [])
            .expect("abandon");
        repo.delete(p.id).expect("archive after cart abandoned");
    }

    #[test]
    fn variant_purchasability_reflects_product_status() {
        let repo = fresh_repo();
        let p = make_product(&repo, "Purch", "purch");
        assert_eq!(
            repo.variant_purchasability("SKU-purch").expect("ok"),
            VariantPurchasability::ProductNotActive(ProductStatus::Draft)
        );
        repo.update(
            p.id,
            UpdateProduct { status: Some(ProductStatus::Active), ..Default::default() },
        )
        .expect("activate");
        assert_eq!(
            repo.variant_purchasability("SKU-purch").expect("ok"),
            VariantPurchasability::Purchasable
        );
        assert_eq!(
            repo.variant_purchasability("NOT-IN-CATALOG").expect("ok"),
            VariantPurchasability::NotInCatalog
        );
        assert!(VariantPurchasability::NotInCatalog.is_sellable());
        assert!(
            VariantPurchasability::ProductNotActive(ProductStatus::Draft)
                .ensure_sellable("x")
                .is_err()
        );
    }

    #[test]
    fn update_variant_sku_collision_is_typed() {
        let repo = fresh_repo();
        let a = make_product(&repo, "A", "a-sku");
        let _b = make_product(&repo, "B", "b-sku");
        let va = repo.get_variants(a.id).expect("ok").remove(0);
        let err = repo
            .update_variant(
                va.id,
                CreateProductVariant {
                    sku: "SKU-b-sku".into(),
                    price: dec!(1),
                    ..Default::default()
                },
            )
            .expect_err("sku collision");
        assert!(matches!(err, CommerceError::DuplicateSku(_)), "{err:?}");
    }

    #[test]
    fn concurrent_slug_and_sku_creation_yields_exactly_one_winner() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("race.db");
        let db = SqliteDatabase::new(&crate::DatabaseConfig {
            url: path.to_str().expect("utf8").to_string(),
            max_connections: 8,
        })
        .expect("open");
        let db = std::sync::Arc::new(db);
        let threads = 8;
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(threads));
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let db = std::sync::Arc::clone(&db);
                let barrier = std::sync::Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    db.products().create(CreateProduct {
                        name: "Raced".into(),
                        slug: Some("raced".into()),
                        variants: Some(vec![CreateProductVariant {
                            sku: "RACED-1".into(),
                            price: dec!(1),
                            ..Default::default()
                        }]),
                        ..Default::default()
                    })
                })
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("thread")).collect();
        let winners = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(winners, 1, "exactly one create may win: {results:?}");
        for r in results.iter().filter(|r| r.is_err()) {
            assert!(
                matches!(r, Err(CommerceError::DuplicateSlug(_) | CommerceError::DuplicateSku(_))),
                "losers must get a typed conflict: {r:?}"
            );
        }
        assert_eq!(
            db.products().list(ProductFilter::default()).expect("list").len(),
            1,
            "no half-products may survive"
        );
    }
}

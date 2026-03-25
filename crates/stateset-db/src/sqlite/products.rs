//! SQLite product repository implementation

use super::{
    build_in_clause, escape_like, json1_available, map_db_error, params_refs, parse_datetime_row,
    parse_decimal_opt_row, parse_decimal_row, parse_enum_row, parse_json_opt_row, parse_json_row,
    parse_uuid, parse_uuid_row, uuid_params,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use stateset_core::{
    BatchResult, CommerceError, CreateProduct, CreateProductVariant, Product, ProductFilter,
    ProductId, ProductRepository, ProductStatus, ProductVariant, Result, UpdateProduct,
    validate_batch_size, validate_sku,
};
use uuid::Uuid;

/// SQLite implementation of `ProductRepository`
#[derive(Debug)]
pub struct SqliteProductRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteProductRepository {
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
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
}

impl ProductRepository for SqliteProductRepository {
    fn create(&self, input: CreateProduct) -> Result<Product> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let id = ProductId::new();
        let now = Utc::now();
        let slug = input.slug.clone().unwrap_or_else(|| Product::generate_slug(&input.name));
        let name = input.name.clone();
        let description = input.description.clone().unwrap_or_default();
        let product_type = input.product_type.unwrap_or_default();
        let attributes = input.attributes.clone().unwrap_or_default();
        let seo = input.seo.clone();

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
        .map_err(map_db_error)?;

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
                        (i == 0) as i32,  // First variant is default
                        now.to_rfc3339(),
                        now.to_rfc3339(),
                    ],
                )
                .map_err(map_db_error)?;
            }
        }

        tx.commit().map_err(map_db_error)?;

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
        let conn = self.conn()?;
        let now = Utc::now();
        let current_version: i32 = conn
            .query_row("SELECT version FROM products WHERE id = ?", [id.to_string()], |row| {
                row.get(0)
            })
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    CommerceError::ProductNotFound(id.into_uuid())
                }
                e => map_db_error(e),
            })?;

        let mut updates = vec!["updated_at = ?"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

        if let Some(name) = &input.name {
            updates.push("name = ?");
            params.push(Box::new(name.clone()));
        }
        if let Some(slug) = &input.slug {
            let existing_id: Option<String> = conn
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
        if let Some(status) = &input.status {
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
        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let rows_affected = conn.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;
        if rows_affected == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "product".to_string(),
                id: id.to_string(),
                expected_version: current_version,
            });
        }

        // Fetch the updated product with the same connection
        let result = conn.query_row(
            "SELECT * FROM products WHERE id = ?",
            [id.to_string()],
            Self::row_to_product,
        );

        match result {
            Ok(product) => Ok(product),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                Err(CommerceError::ProductNotFound(id.into_uuid()))
            }
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list(&self, filter: ProductFilter) -> Result<Vec<Product>> {
        let ProductFilter {
            status,
            product_type,
            search,
            category,
            min_price,
            max_price,
            in_stock,
            limit,
            offset,
            after_cursor,
        } = filter;
        let conn = self.conn()?;
        let use_json = json1_available(&conn);
        let mut sql = "SELECT * FROM products WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        } else {
            sql.push_str(" AND status != 'archived'");
        }
        if let Some(product_type) = product_type {
            sql.push_str(" AND product_type = ?");
            params.push(Box::new(product_type.to_string()));
        }
        if let Some(search) = search {
            sql.push_str(" AND (name LIKE ? ESCAPE '\\' OR description LIKE ? ESCAPE '\\')");
            let escaped = format!("%{}%", escape_like(&search));
            params.push(Box::new(escaped.clone()));
            params.push(Box::new(escaped));
        }
        if let Some(category) = category {
            if use_json {
                sql.push_str(
                    " AND EXISTS (SELECT 1 FROM json_each(attributes) attr \
                     WHERE (json_extract(attr.value, '$.name') = 'category' \
                        OR json_extract(attr.value, '$.group') = 'category') \
                       AND json_extract(attr.value, '$.value') = ?)",
                );
                params.push(Box::new(category));
            } else {
                sql.push_str(" AND (attributes LIKE ? OR attributes LIKE ?)");
                params
                    .push(Box::new(format!("%\"name\":\"category\",\"value\":\"{}\"%", category)));
                params
                    .push(Box::new(format!("%\"value\":\"{}\",\"group\":\"category\"%", category)));
            }
        }
        if let Some(in_stock) = in_stock {
            let stock_clause = if in_stock { "EXISTS" } else { "NOT EXISTS" };
            sql.push_str(&format!(
                " AND {} (SELECT 1 FROM product_variants pv_stock \
                 JOIN inventory_items ii ON ii.sku = pv_stock.sku \
                 JOIN inventory_balances ib ON ib.item_id = ii.id \
                 WHERE pv_stock.product_id = products.id \
                   AND pv_stock.is_active = 1 \
                   AND CAST(ib.quantity_available AS REAL) > 0)",
                stock_clause
            ));
        }

        // Keyset cursor: (name, id) for stable ASC ordering
        if let Some((cursor_name, cursor_id)) = &after_cursor {
            sql.push_str(" AND (name > ? OR (name = ? AND id > ?))");
            params.push(Box::new(cursor_name.clone()));
            params.push(Box::new(cursor_name.clone()));
            params.push(Box::new(cursor_id.clone()));
        }

        sql.push_str(" ORDER BY name ASC, id ASC");

        let apply_price_filter = min_price.is_some() || max_price.is_some();
        if !apply_price_filter {
            if let Some(limit) = limit {
                sql.push_str(&format!(" LIMIT {}", limit));
            }
            if after_cursor.is_none() {
                if let Some(offset) = offset {
                    sql.push_str(&format!(" OFFSET {}", offset));
                }
            }
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;

        let products = stmt
            .query_map(params_refs.as_slice(), Self::row_to_product)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        if apply_price_filter {
            let min_price = min_price.as_ref();
            let max_price = max_price.as_ref();
            let mut filtered = Vec::with_capacity(products.len());
            for product in products {
                let variants = self.get_variants(product.id)?;
                let mut matches = false;
                for variant in variants {
                    if !variant.is_active {
                        continue;
                    }
                    if let Some(min) = min_price {
                        if variant.price < *min {
                            continue;
                        }
                    }
                    if let Some(max) = max_price {
                        if variant.price > *max {
                            continue;
                        }
                    }
                    matches = true;
                    break;
                }
                if matches {
                    filtered.push(product);
                }
            }
            if let Some(offset) = offset {
                filtered = filtered.into_iter().skip(offset as usize).collect();
            }
            if let Some(limit) = limit {
                filtered.truncate(limit as usize);
            }
            return Ok(filtered);
        }

        Ok(products)
    }

    fn delete(&self, id: ProductId) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE products SET status = 'archived', updated_at = ? WHERE id = ?",
            rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;
        Ok(())
    }

    fn add_variant(
        &self,
        product_id: ProductId,
        variant: CreateProductVariant,
    ) -> Result<ProductVariant> {
        // Validate SKU format
        validate_sku(&variant.sku)?;

        let conn = self.conn()?;
        let id = Uuid::new_v4();
        let now = Utc::now();
        let sku = variant.sku.clone();
        let name = variant.name.clone().unwrap_or_else(|| sku.clone());
        let options = variant.options.clone().unwrap_or_default();

        // Check SKU uniqueness
        let exists: i32 = conn
            .query_row("SELECT COUNT(*) FROM product_variants WHERE sku = ?", [&sku], |row| {
                row.get(0)
            })
            .map_err(map_db_error)?;

        if exists > 0 {
            return Err(CommerceError::DuplicateSku(sku));
        }

        let options_json = serde_json::to_string(&options).unwrap_or_default();

        conn.execute(
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
                variant.is_default.unwrap_or(false) as i32,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

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
        let conn = self.conn()?;
        let now = Utc::now();
        let current_version: i32 = conn
            .query_row(
                "SELECT version FROM product_variants WHERE id = ?",
                [id.to_string()],
                |row| row.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => CommerceError::ProductVariantNotFound(id),
                e => map_db_error(e),
            })?;

        let options_json =
            serde_json::to_string(&variant.options.clone().unwrap_or_default()).unwrap_or_default();

        let rows_affected = conn.execute(
            "UPDATE product_variants SET name = ?, price = ?, compare_at_price = ?, cost = ?,
                     barcode = ?, weight = ?, weight_unit = ?, options = ?, updated_at = ?, version = version + 1
             WHERE id = ? AND version = ?",
            rusqlite::params![
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
        .map_err(map_db_error)?;
        if rows_affected == 0 {
            return Err(CommerceError::VersionConflict {
                entity: "product_variant".to_string(),
                id: id.to_string(),
                expected_version: current_version,
            });
        }

        // Fetch the updated variant with the same connection
        let result = conn.query_row(
            "SELECT * FROM product_variants WHERE id = ?",
            [id.to_string()],
            Self::row_to_variant,
        );

        match result {
            Ok(v) => Ok(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                Err(CommerceError::ProductVariantNotFound(id))
            }
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn delete_variant(&self, id: Uuid) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE product_variants SET is_active = 0, updated_at = ? WHERE id = ?",
            rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;
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
        let ProductFilter {
            status,
            product_type,
            search,
            category,
            min_price,
            max_price,
            in_stock,
            limit: _,
            offset: _,
            after_cursor: _,
        } = filter;

        let conn = self.conn()?;
        let use_json = json1_available(&conn);
        let mut sql = "SELECT id FROM products WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        } else {
            sql.push_str(" AND status != 'archived'");
        }
        if let Some(product_type) = product_type {
            sql.push_str(" AND product_type = ?");
            params.push(Box::new(product_type.to_string()));
        }
        if let Some(search) = search {
            sql.push_str(" AND (name LIKE ? ESCAPE '\\' OR description LIKE ? ESCAPE '\\')");
            let escaped = format!("%{}%", escape_like(&search));
            params.push(Box::new(escaped.clone()));
            params.push(Box::new(escaped));
        }
        if let Some(category) = category {
            if use_json {
                sql.push_str(
                    " AND EXISTS (SELECT 1 FROM json_each(attributes) attr \
                     WHERE (json_extract(attr.value, '$.name') = 'category' \
                        OR json_extract(attr.value, '$.group') = 'category') \
                       AND json_extract(attr.value, '$.value') = ?)",
                );
                params.push(Box::new(category));
            } else {
                sql.push_str(" AND (attributes LIKE ? OR attributes LIKE ?)");
                params
                    .push(Box::new(format!("%\"name\":\"category\",\"value\":\"{}\"%", category)));
                params
                    .push(Box::new(format!("%\"value\":\"{}\",\"group\":\"category\"%", category)));
            }
        }
        if let Some(in_stock) = in_stock {
            let stock_clause = if in_stock { "EXISTS" } else { "NOT EXISTS" };
            sql.push_str(&format!(
                " AND {} (SELECT 1 FROM product_variants pv_stock \
                 JOIN inventory_items ii ON ii.sku = pv_stock.sku \
                 JOIN inventory_balances ib ON ib.item_id = ii.id \
                 WHERE pv_stock.product_id = products.id \
                   AND pv_stock.is_active = 1 \
                   AND CAST(ib.quantity_available AS REAL) > 0)",
                stock_clause
            ));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let ids = stmt
            .query_map(params_refs.as_slice(), |row| row.get::<_, String>(0))
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?
            .into_iter()
            .map(|id_str| parse_uuid(&id_str, "product", "id").map(ProductId::from))
            .collect::<Result<Vec<_>>>()?;

        if min_price.is_some() || max_price.is_some() {
            let min_price = min_price.as_ref();
            let max_price = max_price.as_ref();
            let mut count = 0u64;
            for id in ids {
                let variants = self.get_variants(id)?;
                let mut matches = false;
                for variant in variants {
                    if !variant.is_active {
                        continue;
                    }
                    if let Some(min) = min_price {
                        if variant.price < *min {
                            continue;
                        }
                    }
                    if let Some(max) = max_price {
                        if variant.price > *max {
                            continue;
                        }
                    }
                    matches = true;
                    break;
                }
                if matches {
                    count += 1;
                }
            }
            return Ok(count);
        }

        Ok(ids.len() as u64)
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
        let tx = conn.transaction().map_err(map_db_error)?;
        let mut results = Vec::with_capacity(inputs.len());

        for input in inputs {
            let id = ProductId::new();
            let now = Utc::now();
            let slug = input.slug.clone().unwrap_or_else(|| Product::generate_slug(&input.name));
            let name = input.name.clone();
            let description = input.description.clone().unwrap_or_default();
            let product_type = input.product_type.unwrap_or_default();
            let attributes = input.attributes.clone().unwrap_or_default();
            let seo = input.seo.clone();

            // Check slug uniqueness
            let exists: i32 = tx
                .query_row("SELECT COUNT(*) FROM products WHERE slug = ?", [&slug], |row| {
                    row.get(0)
                })
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
            .map_err(map_db_error)?;

            // Create variants inline if provided
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
                            (i == 0) as i32,  // First variant is default
                            now.to_rfc3339(),
                            now.to_rfc3339(),
                        ],
                    )
                    .map_err(map_db_error)?;
                }
            }

            results.push(Product {
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
            });
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
        let tx = conn.transaction().map_err(map_db_error)?;
        let mut results = Vec::with_capacity(updates.len());

        for (id, input) in updates {
            let now = Utc::now();
            let current_version: i32 = tx
                .query_row("SELECT version FROM products WHERE id = ?", [id.to_string()], |row| {
                    row.get(0)
                })
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        CommerceError::ProductNotFound(id.into_uuid())
                    }
                    e => map_db_error(e),
                })?;

            let mut update_parts = vec!["updated_at = ?"];
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

            if let Some(name) = &input.name {
                update_parts.push("name = ?");
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
                update_parts.push("slug = ?");
                params.push(Box::new(slug.clone()));
            }
            if let Some(description) = &input.description {
                update_parts.push("description = ?");
                params.push(Box::new(description.clone()));
            }
            if let Some(status) = &input.status {
                update_parts.push("status = ?");
                params.push(Box::new(status.to_string()));
            }
            if let Some(attributes) = &input.attributes {
                update_parts.push("attributes = ?");
                params.push(Box::new(serde_json::to_string(attributes).unwrap_or_default()));
            }
            if let Some(seo) = &input.seo {
                update_parts.push("seo = ?");
                params.push(Box::new(serde_json::to_string(seo).unwrap_or_default()));
            }

            update_parts.push("version = version + 1");
            params.push(Box::new(id.to_string()));
            params.push(Box::new(current_version));

            let sql = format!(
                "UPDATE products SET {} WHERE id = ? AND version = ?",
                update_parts.join(", ")
            );

            let params_refs: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            let rows_affected = tx.execute(&sql, params_refs.as_slice()).map_err(map_db_error)?;
            if rows_affected == 0 {
                return Err(CommerceError::VersionConflict {
                    entity: "product".to_string(),
                    id: id.to_string(),
                    expected_version: current_version,
                });
            }

            let product = tx
                .query_row(
                    "SELECT * FROM products WHERE id = ?",
                    [id.to_string()],
                    Self::row_to_product,
                )
                .map_err(map_db_error)?;

            results.push(product);
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
        let tx = conn.transaction().map_err(map_db_error)?;

        let placeholders = build_in_clause(ids.len());

        // Archive products (soft delete) with IN clause
        let sql = format!(
            "UPDATE products SET status = 'archived', updated_at = ? WHERE id IN ({})",
            placeholders
        );

        // Build params with timestamp first, then IDs
        let now = Utc::now().to_rfc3339();
        let mut all_params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];
        for id in &ids {
            all_params.push(Box::new(id.to_string()));
        }
        let all_params_refs: Vec<&dyn rusqlite::ToSql> =
            all_params.iter().map(|p| p.as_ref()).collect();

        tx.execute(&sql, all_params_refs.as_slice()).map_err(map_db_error)?;

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
        let sql = format!("SELECT * FROM products WHERE id IN ({})", placeholders);

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

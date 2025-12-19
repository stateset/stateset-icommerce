//! SQLite product repository implementation

use super::{map_db_error, parse_decimal};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CommerceError, CreateProduct, CreateProductVariant, Product, ProductFilter,
    ProductRepository, ProductStatus, ProductType, ProductVariant, Result, UpdateProduct,
};
use uuid::Uuid;

/// SQLite implementation of ProductRepository
pub struct SqliteProductRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteProductRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_product(row: &rusqlite::Row) -> rusqlite::Result<Product> {
        let attributes_json: String = row.get("attributes")?;
        let seo_json: Option<String> = row.get("seo")?;

        Ok(Product {
            id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
            name: row.get("name")?,
            slug: row.get("slug")?,
            description: row.get("description")?,
            status: parse_product_status(&row.get::<_, String>("status")?),
            product_type: parse_product_type(&row.get::<_, String>("product_type")?),
            attributes: serde_json::from_str(&attributes_json).unwrap_or_default(),
            seo: seo_json.and_then(|s| serde_json::from_str(&s).ok()),
            created_at: row.get::<_, String>("created_at")?.parse().unwrap_or_else(|_| Utc::now()),
            updated_at: row.get::<_, String>("updated_at")?.parse().unwrap_or_else(|_| Utc::now()),
        })
    }

    fn row_to_variant(row: &rusqlite::Row) -> rusqlite::Result<ProductVariant> {
        let options_json: String = row.get("options")?;

        Ok(ProductVariant {
            id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
            product_id: row.get::<_, String>("product_id")?.parse().unwrap_or_default(),
            sku: row.get("sku")?,
            name: row.get("name")?,
            price: parse_decimal(&row.get::<_, String>("price")?),
            compare_at_price: row.get::<_, Option<String>>("compare_at_price")?.map(|s| parse_decimal(&s)),
            cost: row.get::<_, Option<String>>("cost")?.map(|s| parse_decimal(&s)),
            barcode: row.get("barcode")?,
            weight: row.get::<_, Option<String>>("weight")?.map(|s| parse_decimal(&s)),
            weight_unit: row.get("weight_unit")?,
            options: serde_json::from_str(&options_json).unwrap_or_default(),
            is_default: row.get::<_, i32>("is_default")? != 0,
            is_active: row.get::<_, i32>("is_active")? != 0,
            created_at: row.get::<_, String>("created_at")?.parse().unwrap_or_else(|_| Utc::now()),
            updated_at: row.get::<_, String>("updated_at")?.parse().unwrap_or_else(|_| Utc::now()),
        })
    }
}

impl ProductRepository for SqliteProductRepository {
    fn create(&self, input: CreateProduct) -> Result<Product> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let id = Uuid::new_v4();
        let now = Utc::now();
        let slug = input.slug.clone().unwrap_or_else(|| Product::generate_slug(&input.name));
        let name = input.name.clone();
        let description = input.description.clone().unwrap_or_default();
        let product_type = input.product_type.unwrap_or_default();
        let attributes = input.attributes.clone().unwrap_or_default();
        let seo = input.seo.clone();

        // Check slug uniqueness
        let exists: i32 = tx
            .query_row(
                "SELECT COUNT(*) FROM products WHERE slug = ?",
                [&slug],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        if exists > 0 {
            return Err(CommerceError::DuplicateSlug(slug));
        }

        let attributes_json = serde_json::to_string(&attributes).unwrap_or_default();
        let seo_json = seo.as_ref().map(|s| serde_json::to_string(s).unwrap_or_default());

        tx.execute(
            "INSERT INTO products (id, name, slug, description, status, product_type, attributes, seo, created_at, updated_at)
             VALUES (?, ?, ?, ?, 'draft', ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                &name,
                &slug,
                &description,
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

                let options_json = serde_json::to_string(&variant.options.clone().unwrap_or_default()).unwrap_or_default();

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

    fn get(&self, id: Uuid) -> Result<Option<Product>> {
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
        let result = conn.query_row(
            "SELECT * FROM products WHERE slug = ?",
            [slug],
            Self::row_to_product,
        );

        match result {
            Ok(product) => Ok(Some(product)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: Uuid, input: UpdateProduct) -> Result<Product> {
        let conn = self.conn()?;
        let now = Utc::now();

        let mut updates = vec!["updated_at = ?"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

        if let Some(name) = &input.name {
            updates.push("name = ?");
            params.push(Box::new(name.clone()));
        }
        if let Some(slug) = &input.slug {
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

        params.push(Box::new(id.to_string()));

        let sql = format!("UPDATE products SET {} WHERE id = ?", updates.join(", "));
        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        conn.execute(&sql, params_refs.as_slice())
            .map_err(map_db_error)?;

        // Fetch the updated product with the same connection
        let result = conn.query_row(
            "SELECT * FROM products WHERE id = ?",
            [id.to_string()],
            Self::row_to_product,
        );

        match result {
            Ok(product) => Ok(product),
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(CommerceError::ProductNotFound(id)),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list(&self, filter: ProductFilter) -> Result<Vec<Product>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM products WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(product_type) = &filter.product_type {
            sql.push_str(" AND product_type = ?");
            params.push(Box::new(product_type.to_string()));
        }
        if let Some(search) = &filter.search {
            sql.push_str(" AND (name LIKE ? OR description LIKE ?)");
            params.push(Box::new(format!("%{}%", search)));
            params.push(Box::new(format!("%{}%", search)));
        }

        sql.push_str(" ORDER BY name");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;

        let products = stmt
            .query_map(params_refs.as_slice(), Self::row_to_product)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(products)
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE products SET status = 'archived', updated_at = ? WHERE id = ?",
            rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;
        Ok(())
    }

    fn add_variant(&self, product_id: Uuid, variant: CreateProductVariant) -> Result<ProductVariant> {
        let conn = self.conn()?;
        let id = Uuid::new_v4();
        let now = Utc::now();
        let sku = variant.sku.clone();
        let name = variant.name.clone().unwrap_or_else(|| sku.clone());
        let options = variant.options.clone().unwrap_or_default();

        // Check SKU uniqueness
        let exists: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM product_variants WHERE sku = ?",
                [&sku],
                |row| row.get(0),
            )
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

        let options_json = serde_json::to_string(&variant.options.clone().unwrap_or_default()).unwrap_or_default();

        conn.execute(
            "UPDATE product_variants SET name = ?, price = ?, compare_at_price = ?, cost = ?,
                     barcode = ?, weight = ?, weight_unit = ?, options = ?, updated_at = ? WHERE id = ?",
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
            ],
        )
        .map_err(map_db_error)?;

        // Fetch the updated variant with the same connection
        let result = conn.query_row(
            "SELECT * FROM product_variants WHERE id = ?",
            [id.to_string()],
            Self::row_to_variant,
        );

        match result {
            Ok(v) => Ok(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(CommerceError::ProductVariantNotFound(id)),
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

    fn get_variants(&self, product_id: Uuid) -> Result<Vec<ProductVariant>> {
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
        let mut sql = "SELECT COUNT(*) FROM products WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let count: i64 = conn
            .query_row(&sql, params_refs.as_slice(), |row| row.get(0))
            .map_err(map_db_error)?;

        Ok(count as u64)
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

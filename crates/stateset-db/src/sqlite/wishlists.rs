//! SQLite implementation of wishlist repository

use super::{map_db_error, parse_datetime_row, parse_uuid_row, with_immediate_transaction};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    AddWishlistItem, CommerceError, CreateWishlist, ProductId, Result, UpdateWishlist, Wishlist,
    WishlistFilter, WishlistId, WishlistItem, WishlistRepository,
};

#[derive(Debug)]
pub struct SqliteWishlistRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteWishlistRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_wishlist(row: &rusqlite::Row<'_>) -> rusqlite::Result<Wishlist> {
        Ok(Wishlist {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "wishlist", "id")?.into(),
            customer_id: parse_uuid_row(
                &row.get::<_, String>("customer_id")?,
                "wishlist",
                "customer_id",
            )?
            .into(),
            name: row.get("name")?,
            is_public: row.get::<_, i32>("is_public")? != 0,
            items: Vec::new(),
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "wishlist",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "wishlist",
                "updated_at",
            )?,
        })
    }

    fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<WishlistItem> {
        Ok(WishlistItem {
            product_id: parse_uuid_row(
                &row.get::<_, String>("product_id")?,
                "wishlist_item",
                "product_id",
            )?
            .into(),
            variant_id: row.get("variant_id")?,
            added_at: parse_datetime_row(
                &row.get::<_, String>("added_at")?,
                "wishlist_item",
                "added_at",
            )?,
            note: row.get("notes")?,
            quantity: row.get::<_, i64>("quantity")? as u32,
            priority: row.get("priority")?,
        })
    }

    fn load_items(
        conn: &rusqlite::Connection,
        wishlist_id: &str,
    ) -> rusqlite::Result<Vec<WishlistItem>> {
        let mut stmt =
            conn.prepare("SELECT * FROM wishlist_items WHERE wishlist_id = ? ORDER BY added_at")?;
        let items = stmt
            .query_map([wishlist_id], Self::row_to_item)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(items)
    }
}

impl WishlistRepository for SqliteWishlistRepository {
    fn create(&self, input: CreateWishlist) -> Result<Wishlist> {
        let id = WishlistId::new();
        let now = Utc::now();
        let id_str = id.to_string();
        let now_str = now.to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO wishlists (id, customer_id, name, is_public, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    input.customer_id.to_string(),
                    &input.name,
                    input.is_public as i32,
                    &now_str,
                    &now_str,
                ],
            )?;

            tx.query_row("SELECT * FROM wishlists WHERE id = ?", [&id_str], Self::row_to_wishlist)
        })
    }

    fn get(&self, id: WishlistId) -> Result<Option<Wishlist>> {
        let conn = self.conn()?;
        let id_str = id.to_string();
        match conn.query_row(
            "SELECT * FROM wishlists WHERE id = ?",
            [&id_str],
            Self::row_to_wishlist,
        ) {
            Ok(mut wishlist) => {
                wishlist.items = Self::load_items(&conn, &id_str).map_err(map_db_error)?;
                Ok(Some(wishlist))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: WishlistId, input: UpdateWishlist) -> Result<Wishlist> {
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            let mut sets = vec!["updated_at = ?".to_string()];
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now_str.clone())];

            if let Some(ref name) = input.name {
                sets.push("name = ?".into());
                params.push(Box::new(name.clone()));
            }
            if let Some(is_public) = input.is_public {
                sets.push("is_public = ?".into());
                params.push(Box::new(is_public as i32));
            }

            let sql = format!("UPDATE wishlists SET {} WHERE id = ?", sets.join(", "));
            params.push(Box::new(id_str.clone()));

            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            tx.execute(&sql, param_refs.as_slice())?;

            let mut wishlist = tx.query_row(
                "SELECT * FROM wishlists WHERE id = ?",
                [&id_str],
                Self::row_to_wishlist,
            )?;
            wishlist.items = Self::load_items(tx, &id_str)?;
            Ok(wishlist)
        })
    }

    fn list(&self, filter: WishlistFilter) -> Result<Vec<Wishlist>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM wishlists WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

        if let Some(customer_id) = filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params.push(Box::new(customer_id.to_string()));
        }
        if let Some(is_public) = filter.is_public {
            sql.push_str(" AND is_public = ?");
            params.push(Box::new(is_public as i32));
        }

        sql.push_str(" ORDER BY created_at DESC");

        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let wishlists = stmt
            .query_map(param_refs.as_slice(), Self::row_to_wishlist)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;

        // Load items for each wishlist
        let mut result = Vec::with_capacity(wishlists.len());
        for mut wl in wishlists {
            let id_str = wl.id.to_string();
            wl.items = Self::load_items(&conn, &id_str).map_err(map_db_error)?;
            result.push(wl);
        }
        Ok(result)
    }

    fn delete(&self, id: WishlistId) -> Result<()> {
        let conn = self.conn()?;
        let id_str = id.to_string();
        // Delete items first (foreign-key-like cleanup)
        conn.execute("DELETE FROM wishlist_items WHERE wishlist_id = ?", [&id_str])
            .map_err(map_db_error)?;
        conn.execute("DELETE FROM wishlists WHERE id = ?", [&id_str]).map_err(map_db_error)?;
        Ok(())
    }

    fn add_item(&self, wishlist_id: WishlistId, item: AddWishlistItem) -> Result<WishlistItem> {
        let wl_id_str = wishlist_id.to_string();
        let item_id = uuid::Uuid::new_v4().to_string();
        let now_str = Utc::now().to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            // Verify wishlist exists
            tx.query_row("SELECT id FROM wishlists WHERE id = ?", [&wl_id_str], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => rusqlite::Error::QueryReturnedNoRows,
                other => other,
            })?;

            let product_id_str = item.product_id.to_string();
            let quantity = i64::from(item.quantity.unwrap_or(1));

            tx.execute(
                "INSERT INTO wishlist_items (id, wishlist_id, product_id, variant_id, priority, quantity, added_at, notes)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &item_id,
                    &wl_id_str,
                    &product_id_str,
                    &item.variant_id,
                    &item.priority,
                    quantity,
                    &now_str,
                    &item.note,
                ],
            )?;

            // Update wishlist updated_at
            tx.execute(
                "UPDATE wishlists SET updated_at = ? WHERE id = ?",
                rusqlite::params![&now_str, &wl_id_str],
            )?;

            tx.query_row("SELECT * FROM wishlist_items WHERE id = ?", [&item_id], Self::row_to_item)
        })
    }

    fn remove_item(&self, wishlist_id: WishlistId, product_id: ProductId) -> Result<()> {
        let conn = self.conn()?;
        let wl_id_str = wishlist_id.to_string();
        let product_id_str = product_id.to_string();
        let now_str = Utc::now().to_rfc3339();

        conn.execute(
            "DELETE FROM wishlist_items WHERE wishlist_id = ? AND product_id = ?",
            rusqlite::params![&wl_id_str, &product_id_str],
        )
        .map_err(map_db_error)?;

        // Update wishlist updated_at
        conn.execute(
            "UPDATE wishlists SET updated_at = ? WHERE id = ?",
            rusqlite::params![&now_str, &wl_id_str],
        )
        .map_err(map_db_error)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;
    use stateset_core::CustomerId;

    fn test_repo() -> SqliteWishlistRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        let conn = db.conn().expect("conn");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS wishlists (
                id TEXT PRIMARY KEY,
                customer_id TEXT NOT NULL,
                name TEXT NOT NULL DEFAULT 'My Wishlist',
                is_public INTEGER NOT NULL DEFAULT 0,
                created_at TEXT,
                updated_at TEXT
            );
            CREATE TABLE IF NOT EXISTS wishlist_items (
                id TEXT PRIMARY KEY,
                wishlist_id TEXT NOT NULL,
                product_id TEXT NOT NULL,
                added_at TEXT NOT NULL DEFAULT (datetime('now')),
                notes TEXT,
                UNIQUE(wishlist_id, product_id)
            );",
        )
        .expect("create tables");
        SqliteWishlistRepository::new(db.pool().clone())
    }

    #[test]
    fn create_and_get() {
        let repo = test_repo();
        let customer_id = CustomerId::new();
        let wishlist = repo
            .create(CreateWishlist { customer_id, name: "Birthday Ideas".into(), is_public: true })
            .expect("create");

        assert_eq!(wishlist.name, "Birthday Ideas");
        assert!(wishlist.is_public);
        assert!(wishlist.items.is_empty());

        let fetched = repo.get(wishlist.id).expect("get").expect("Some");
        assert_eq!(fetched.id, wishlist.id);
        assert_eq!(fetched.customer_id, customer_id);
        assert_eq!(fetched.name, "Birthday Ideas");
        assert!(fetched.is_public);
    }

    #[test]
    fn add_item() {
        let repo = test_repo();
        let wishlist = repo
            .create(CreateWishlist {
                customer_id: CustomerId::new(),
                name: "Gadgets".into(),
                is_public: false,
            })
            .expect("create");

        let product_id = ProductId::new();
        let item = repo
            .add_item(
                wishlist.id,
                AddWishlistItem {
                    product_id,
                    variant_id: Some("VAR-1".into()),
                    note: Some("Love this one".into()),
                    quantity: Some(3),
                    priority: Some(2),
                },
            )
            .expect("add_item");

        assert_eq!(item.product_id, product_id);
        assert_eq!(item.note.as_deref(), Some("Love this one"));
        assert_eq!(item.variant_id.as_deref(), Some("VAR-1"));
        assert_eq!(item.quantity, 3);
        assert_eq!(item.priority, Some(2));

        // Re-read from the database: variant_id, quantity, and priority must
        // survive the round-trip (previously they were dropped and quantity
        // always read back as 1).
        let fetched = repo.get(wishlist.id).expect("get").expect("Some");
        assert_eq!(fetched.items.len(), 1);
        let stored = &fetched.items[0];
        assert_eq!(stored.product_id, product_id);
        assert_eq!(stored.variant_id.as_deref(), Some("VAR-1"));
        assert_eq!(stored.quantity, 3, "quantity must survive persistence");
        assert_eq!(stored.priority, Some(2));
    }

    #[test]
    fn delete() {
        let repo = test_repo();
        let wishlist = repo
            .create(CreateWishlist {
                customer_id: CustomerId::new(),
                name: "Temp".into(),
                is_public: false,
            })
            .expect("create");

        // Add an item so we also verify cascade-like cleanup
        repo.add_item(
            wishlist.id,
            AddWishlistItem {
                product_id: ProductId::new(),
                variant_id: None,
                note: None,
                quantity: None,
                priority: None,
            },
        )
        .expect("add_item");

        repo.delete(wishlist.id).expect("delete");
        assert!(repo.get(wishlist.id).expect("get after delete").is_none());
    }
}

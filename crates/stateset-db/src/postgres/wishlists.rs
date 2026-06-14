//! PostgreSQL implementation of wishlist repository

use super::map_db_error;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    AddWishlistItem, CommerceError, CreateWishlist, ProductId, Result, UpdateWishlist, Wishlist,
    WishlistFilter, WishlistId, WishlistItem, WishlistRepository,
};
use uuid::Uuid;

/// PostgreSQL wishlist repository
#[derive(Debug, Clone)]
pub struct PgWishlistRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct WishlistRow {
    id: Uuid,
    customer_id: Uuid,
    name: String,
    is_public: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct WishlistItemRow {
    #[allow(dead_code)]
    id: Uuid,
    #[allow(dead_code)]
    wishlist_id: Uuid,
    product_id: Uuid,
    variant_id: Option<String>,
    priority: Option<i32>,
    notes: Option<String>,
    added_at: DateTime<Utc>,
}

impl PgWishlistRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_wishlist(row: WishlistRow, items: Vec<WishlistItem>) -> Wishlist {
        Wishlist {
            id: WishlistId::from(row.id),
            customer_id: row.customer_id.into(),
            name: row.name,
            is_public: row.is_public,
            items,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    fn row_to_item(row: WishlistItemRow) -> WishlistItem {
        WishlistItem {
            product_id: ProductId::from(row.product_id),
            variant_id: row.variant_id,
            added_at: row.added_at,
            note: row.notes,
            quantity: 1,
            priority: row.priority,
        }
    }

    async fn load_items_async(&self, wishlist_id: Uuid) -> Result<Vec<WishlistItem>> {
        let rows = sqlx::query_as::<_, WishlistItemRow>(
            "SELECT id, wishlist_id, product_id, variant_id, priority, notes, added_at
             FROM wishlist_items WHERE wishlist_id = $1 ORDER BY added_at",
        )
        .bind(wishlist_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_item).collect())
    }

    // ---- async helpers ----

    async fn create_async(&self, input: CreateWishlist) -> Result<Wishlist> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO wishlists (id, customer_id, name, is_public, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(id)
        .bind(input.customer_id.into_uuid())
        .bind(&input.name)
        .bind(input.is_public)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    async fn get_async(&self, id: Uuid) -> Result<Option<Wishlist>> {
        let row = sqlx::query_as::<_, WishlistRow>(
            "SELECT id, customer_id, name, is_public, created_at, updated_at
             FROM wishlists WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        match row {
            Some(r) => {
                let items = self.load_items_async(r.id).await?;
                Ok(Some(Self::row_to_wishlist(r, items)))
            }
            None => Ok(None),
        }
    }

    async fn update_async(&self, id: Uuid, input: UpdateWishlist) -> Result<Wishlist> {
        let now = Utc::now();

        let mut query = String::from("UPDATE wishlists SET updated_at = $1");
        let mut param_idx = 2u32;
        let mut has_name = false;
        let mut has_is_public = false;

        if input.name.is_some() {
            query.push_str(&format!(", name = ${param_idx}"));
            param_idx += 1;
            has_name = true;
        }
        if input.is_public.is_some() {
            query.push_str(&format!(", is_public = ${param_idx}"));
            param_idx += 1;
            has_is_public = true;
        }

        query.push_str(&format!(" WHERE id = ${param_idx}"));

        let mut q = sqlx::query(&query).bind(now);

        if has_name {
            q = q.bind(input.name.expect("checked above"));
        }
        if has_is_public {
            q = q.bind(input.is_public.expect("checked above"));
        }

        q = q.bind(id);

        q.execute(&self.pool).await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    async fn list_async(&self, filter: WishlistFilter) -> Result<Vec<Wishlist>> {
        let mut query = String::from(
            "SELECT id, customer_id, name, is_public, created_at, updated_at
             FROM wishlists WHERE 1=1",
        );
        let mut param_idx = 1u32;
        let mut binds: Vec<WishlistBindValue> = Vec::new();

        if let Some(customer_id) = filter.customer_id {
            query.push_str(&format!(" AND customer_id = ${param_idx}"));
            param_idx += 1;
            binds.push(WishlistBindValue::Uuid(customer_id.into_uuid()));
        }
        if let Some(is_public) = filter.is_public {
            query.push_str(&format!(" AND is_public = ${param_idx}"));
            param_idx += 1;
            binds.push(WishlistBindValue::Bool(is_public));
        }
        let _ = param_idx;

        query.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            query.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = filter.offset {
            query.push_str(&format!(" OFFSET {offset}"));
        }

        let mut q = sqlx::query_as::<_, WishlistRow>(&query);
        for bind in &binds {
            q = match bind {
                WishlistBindValue::Uuid(v) => q.bind(*v),
                WishlistBindValue::Bool(v) => q.bind(*v),
            };
        }

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;

        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            let items = self.load_items_async(row.id).await?;
            result.push(Self::row_to_wishlist(row, items));
        }
        Ok(result)
    }

    async fn delete_async(&self, id: Uuid) -> Result<()> {
        // Items are cascade-deleted via foreign key, but we explicitly delete for safety
        sqlx::query("DELETE FROM wishlist_items WHERE wishlist_id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        sqlx::query("DELETE FROM wishlists WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    async fn add_item_async(
        &self,
        wishlist_id: Uuid,
        item: AddWishlistItem,
    ) -> Result<WishlistItem> {
        let item_id = Uuid::new_v4();
        let now = Utc::now();

        // Verify wishlist exists
        let exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM wishlists WHERE id = $1")
            .bind(wishlist_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        if exists.is_none() {
            return Err(CommerceError::NotFound);
        }

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query(
            "INSERT INTO wishlist_items (id, wishlist_id, product_id, variant_id, priority, notes, added_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(item_id)
        .bind(wishlist_id)
        .bind(item.product_id.into_uuid())
        .bind(&item.variant_id)
        .bind(item.priority)
        .bind(&item.note)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        // Update wishlist updated_at
        sqlx::query("UPDATE wishlists SET updated_at = $1 WHERE id = $2")
            .bind(now)
            .bind(wishlist_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(WishlistItem {
            product_id: item.product_id,
            variant_id: item.variant_id,
            added_at: now,
            note: item.note,
            quantity: item.quantity.unwrap_or(1),
            priority: item.priority,
        })
    }

    async fn remove_item_async(&self, wishlist_id: Uuid, product_id: Uuid) -> Result<()> {
        let now = Utc::now();

        sqlx::query("DELETE FROM wishlist_items WHERE wishlist_id = $1 AND product_id = $2")
            .bind(wishlist_id)
            .bind(product_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        // Update wishlist updated_at
        sqlx::query("UPDATE wishlists SET updated_at = $1 WHERE id = $2")
            .bind(now)
            .bind(wishlist_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }
}

/// Internal enum for heterogeneous bind parameters
enum WishlistBindValue {
    Uuid(Uuid),
    Bool(bool),
}

impl WishlistRepository for PgWishlistRepository {
    fn create(&self, input: CreateWishlist) -> Result<Wishlist> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: WishlistId) -> Result<Option<Wishlist>> {
        super::block_on(self.get_async(id.into_uuid()))
    }

    fn update(&self, id: WishlistId, input: UpdateWishlist) -> Result<Wishlist> {
        super::block_on(self.update_async(id.into_uuid(), input))
    }

    fn list(&self, filter: WishlistFilter) -> Result<Vec<Wishlist>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: WishlistId) -> Result<()> {
        super::block_on(self.delete_async(id.into_uuid()))
    }

    fn add_item(&self, wishlist_id: WishlistId, item: AddWishlistItem) -> Result<WishlistItem> {
        super::block_on(self.add_item_async(wishlist_id.into_uuid(), item))
    }

    fn remove_item(&self, wishlist_id: WishlistId, product_id: ProductId) -> Result<()> {
        super::block_on(self.remove_item_async(wishlist_id.into_uuid(), product_id.into_uuid()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_to_wishlist_maps_fields_correctly() {
        let now = Utc::now();
        let id = Uuid::new_v4();
        let customer_id = Uuid::new_v4();

        let row = WishlistRow {
            id,
            customer_id,
            name: "Birthday Ideas".into(),
            is_public: true,
            created_at: now,
            updated_at: now,
        };

        let wishlist = PgWishlistRepository::row_to_wishlist(row, vec![]);
        assert_eq!(wishlist.name, "Birthday Ideas");
        assert!(wishlist.is_public);
        assert!(wishlist.items.is_empty());
        assert_eq!(wishlist.id.into_uuid(), id);
    }

    #[test]
    fn row_to_item_maps_fields_correctly() {
        let now = Utc::now();
        let product_id = Uuid::new_v4();

        let row = WishlistItemRow {
            id: Uuid::new_v4(),
            wishlist_id: Uuid::new_v4(),
            product_id,
            variant_id: Some("size-L".into()),
            priority: Some(1),
            notes: Some("Red color".into()),
            added_at: now,
        };

        let item = PgWishlistRepository::row_to_item(row);
        assert_eq!(item.product_id.into_uuid(), product_id);
        assert_eq!(item.variant_id.as_deref(), Some("size-L"));
        assert_eq!(item.priority, Some(1));
        assert_eq!(item.note.as_deref(), Some("Red color"));
        assert_eq!(item.quantity, 1);
    }

    #[test]
    fn row_to_item_handles_nullable_fields() {
        let now = Utc::now();
        let row = WishlistItemRow {
            id: Uuid::new_v4(),
            wishlist_id: Uuid::new_v4(),
            product_id: Uuid::new_v4(),
            variant_id: None,
            priority: None,
            notes: None,
            added_at: now,
        };

        let item = PgWishlistRepository::row_to_item(row);
        assert!(item.variant_id.is_none());
        assert!(item.priority.is_none());
        assert!(item.note.is_none());
    }
}

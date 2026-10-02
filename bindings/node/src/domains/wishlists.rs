//! Wishlists.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Wishlists
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateWishlistInput {
    pub customer_id: String,
    pub name: String,
    pub is_public: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateWishlistInput {
    pub name: Option<String>,
    pub is_public: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AddWishlistItemInput {
    pub product_id: String,
    pub variant_id: Option<String>,
    pub note: Option<String>,
    pub quantity: Option<u32>,
    pub priority: Option<i32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct WishlistFilterInput {
    pub customer_id: Option<String>,
    pub is_public: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct WishlistItemOutput {
    pub product_id: String,
    pub variant_id: Option<String>,
    pub added_at: String,
    pub note: Option<String>,
    pub quantity: u32,
    pub priority: Option<i32>,
}

impl From<stateset_core::WishlistItem> for WishlistItemOutput {
    fn from(i: stateset_core::WishlistItem) -> Self {
        Self {
            product_id: i.product_id.to_string(),
            variant_id: i.variant_id,
            added_at: i.added_at.to_rfc3339(),
            note: i.note,
            quantity: i.quantity,
            priority: i.priority,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct WishlistOutput {
    pub id: String,
    pub customer_id: String,
    pub name: String,
    pub is_public: bool,
    pub items: Vec<WishlistItemOutput>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Wishlist> for WishlistOutput {
    fn from(w: stateset_core::Wishlist) -> Self {
        Self {
            id: w.id.to_string(),
            customer_id: w.customer_id.to_string(),
            name: w.name,
            is_public: w.is_public,
            items: w.items.into_iter().map(Into::into).collect(),
            created_at: w.created_at.to_rfc3339(),
            updated_at: w.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct Wishlists {
    pub(crate) commerce: Handle,
}

#[napi]
impl Wishlists {
    /// Whether the wishlists backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.wishlists().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateWishlistInput) -> Result<WishlistOutput> {
        let commerce = self.commerce.get()?;
        let customer_id: uuid::Uuid = input
            .customer_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;
        let wishlist = commerce
            .wishlists()
            .create(stateset_core::CreateWishlist {
                customer_id: customer_id.into(),
                name: input.name,
                is_public: input.is_public.unwrap_or(false),
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create wishlist", e))?;
        Ok(wishlist.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<WishlistOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let wishlist = commerce
            .wishlists()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get wishlist", e))?;
        Ok(wishlist.map(Into::into))
    }

    #[napi]
    pub async fn update(&self, id: String, input: UpdateWishlistInput) -> Result<WishlistOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let wishlist = commerce
            .wishlists()
            .update(
                uuid.into(),
                stateset_core::UpdateWishlist { name: input.name, is_public: input.is_public },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update wishlist", e))?;
        Ok(wishlist.into())
    }

    #[napi]
    pub async fn list(&self, filter: Option<WishlistFilterInput>) -> Result<Vec<WishlistOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.unwrap_or_default();
        let customer_id = match filter.customer_id.as_deref() {
            Some(s) => Some(
                s.parse::<uuid::Uuid>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?
                    .into(),
            ),
            None => None,
        };
        let wishlists = commerce
            .wishlists()
            .list(stateset_core::WishlistFilter {
                customer_id,
                is_public: filter.is_public,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list wishlists", e))?;
        Ok(wishlists.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        commerce
            .wishlists()
            .delete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete wishlist", e))?;
        Ok(())
    }

    /// Add a product to a wishlist, returning the added item.
    #[napi]
    pub async fn add_item(
        &self,
        wishlist_id: String,
        item: AddWishlistItemInput,
    ) -> Result<WishlistItemOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            wishlist_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid wishlist UUID"))?;
        let product_id: uuid::Uuid = item
            .product_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid product UUID"))?;
        let added = commerce
            .wishlists()
            .add_item(
                uuid.into(),
                stateset_core::AddWishlistItem {
                    product_id: product_id.into(),
                    variant_id: item.variant_id,
                    note: item.note,
                    quantity: item.quantity,
                    priority: item.priority,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to add wishlist item", e))?;
        Ok(added.into())
    }

    #[napi]
    pub async fn remove_item(&self, wishlist_id: String, product_id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            wishlist_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid wishlist UUID"))?;
        let product_uuid: uuid::Uuid =
            product_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid product UUID"))?;
        commerce
            .wishlists()
            .remove_item(uuid.into(), product_uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to remove wishlist item", e))?;
        Ok(())
    }
}

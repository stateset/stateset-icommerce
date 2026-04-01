//! Wishlist domain models
//!
//! Allows customers to save products for later purchase, with optional
//! public sharing and cart conversion.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use stateset_primitives::{CustomerId, ProductId, WishlistId};

/// A customer wishlist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wishlist {
    /// Unique wishlist ID
    pub id: WishlistId,
    /// Customer who owns this wishlist
    pub customer_id: CustomerId,
    /// Wishlist name (e.g., "Birthday Ideas", "Home Office")
    pub name: String,
    /// Whether this wishlist is publicly visible
    pub is_public: bool,
    /// Items in this wishlist
    pub items: Vec<WishlistItem>,
    /// When the wishlist was created
    pub created_at: DateTime<Utc>,
    /// When the wishlist was last updated
    pub updated_at: DateTime<Utc>,
}

/// An item in a wishlist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WishlistItem {
    /// Product ID
    pub product_id: ProductId,
    /// Optional variant ID (e.g., size/color)
    pub variant_id: Option<String>,
    /// When the item was added
    pub added_at: DateTime<Utc>,
    /// Optional customer note
    pub note: Option<String>,
    /// Desired quantity
    pub quantity: u32,
    /// Priority (lower = higher priority)
    pub priority: Option<i32>,
}

/// Input for creating a wishlist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWishlist {
    /// Customer who owns the wishlist
    pub customer_id: CustomerId,
    /// Wishlist name
    pub name: String,
    /// Whether to make it public
    pub is_public: bool,
}

/// Input for adding an item to a wishlist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddWishlistItem {
    /// Product to add
    pub product_id: ProductId,
    /// Optional variant
    pub variant_id: Option<String>,
    /// Optional note
    pub note: Option<String>,
    /// Desired quantity (default 1)
    pub quantity: Option<u32>,
    /// Priority
    pub priority: Option<i32>,
}

/// Input for updating a wishlist
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateWishlist {
    /// Updated name
    pub name: Option<String>,
    /// Updated visibility
    pub is_public: Option<bool>,
}

/// Filter for listing wishlists
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WishlistFilter {
    /// Filter by customer
    pub customer_id: Option<CustomerId>,
    /// Filter by public/private
    pub is_public: Option<bool>,
    /// Maximum results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

impl Wishlist {
    /// Number of items in this wishlist
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Whether this wishlist is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Check if a product is already in this wishlist
    #[must_use]
    pub fn contains_product(&self, product_id: &ProductId) -> bool {
        self.items.iter().any(|item| item.product_id == *product_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use stateset_primitives::{CustomerId, ProductId, WishlistId};

    fn make_item(product_id: ProductId) -> WishlistItem {
        WishlistItem {
            product_id,
            variant_id: None,
            added_at: Utc::now(),
            note: None,
            quantity: 1,
            priority: None,
        }
    }

    fn make_wishlist(items: Vec<WishlistItem>) -> Wishlist {
        Wishlist {
            id: WishlistId::new(),
            customer_id: CustomerId::new(),
            name: "My Wishlist".to_string(),
            is_public: false,
            items,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    // ---- item_count ----

    #[test]
    fn item_count_returns_zero_for_empty_wishlist() {
        let wl = make_wishlist(vec![]);
        assert_eq!(wl.item_count(), 0);
    }

    #[test]
    fn item_count_returns_correct_count() {
        let wl = make_wishlist(vec![make_item(ProductId::new()), make_item(ProductId::new())]);
        assert_eq!(wl.item_count(), 2);
    }

    // ---- is_empty ----

    #[test]
    fn is_empty_returns_true_for_empty_wishlist() {
        let wl = make_wishlist(vec![]);
        assert!(wl.is_empty());
    }

    #[test]
    fn is_empty_returns_false_when_items_present() {
        let wl = make_wishlist(vec![make_item(ProductId::new())]);
        assert!(!wl.is_empty());
    }

    // ---- contains_product ----

    #[test]
    fn contains_product_returns_true_when_present() {
        let product_id = ProductId::new();
        let wl = make_wishlist(vec![make_item(product_id)]);
        assert!(wl.contains_product(&product_id));
    }

    #[test]
    fn contains_product_returns_false_when_absent() {
        let wl = make_wishlist(vec![make_item(ProductId::new())]);
        let other_id = ProductId::new();
        assert!(!wl.contains_product(&other_id));
    }

    #[test]
    fn contains_product_returns_false_for_empty_wishlist() {
        let wl = make_wishlist(vec![]);
        assert!(!wl.contains_product(&ProductId::new()));
    }
}

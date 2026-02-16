//! Strongly-typed entity identifiers.
//!
//! Each ID type is a newtype around [`Uuid`] that prevents accidentally mixing up
//! identifiers from different domains. All ID types are `Copy`, `Eq`, `Hash`, and
//! support serialization via `serde`.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Generate a strongly-typed ID newtype around `Uuid`.
///
/// The generated type implements:
/// - `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Hash`
/// - `Display` (delegates to `Uuid::to_string()`)
/// - `FromStr` (parses via `Uuid::parse_str`)
/// - `From<Uuid>`, `From<IdType> for Uuid`
/// - `AsRef<Uuid>`
/// - `Serialize`, `Deserialize` (transparent)
/// - `new()` to generate a random v4 ID
/// - `nil()` for a zero/nil ID
macro_rules! define_id {
    (
        $(#[$meta:meta])*
        $name:ident
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Create a new random ID (UUID v4).
            #[inline]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Create a nil (all-zeros) ID.
            #[inline]
            pub const fn nil() -> Self {
                Self(Uuid::nil())
            }

            /// Create from an existing [`Uuid`].
            #[inline]
            pub const fn from_uuid(id: Uuid) -> Self {
                Self(id)
            }

            /// Get the inner [`Uuid`].
            #[inline]
            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }

            /// Consume and return the inner [`Uuid`].
            #[inline]
            pub const fn into_uuid(self) -> Uuid {
                self.0
            }

            /// Returns `true` if this is a nil (all-zeros) ID.
            #[inline]
            pub fn is_nil(&self) -> bool {
                self.0.is_nil()
            }
        }

        impl Default for $name {
            #[inline]
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;

            #[inline]
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(s).map(Self)
            }
        }

        impl From<Uuid> for $name {
            #[inline]
            fn from(id: Uuid) -> Self {
                Self(id)
            }
        }

        impl From<$name> for Uuid {
            #[inline]
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl AsRef<Uuid> for $name {
            #[inline]
            fn as_ref(&self) -> &Uuid {
                &self.0
            }
        }

        #[cfg(any(test, feature = "arbitrary"))]
        impl proptest::arbitrary::Arbitrary for $name {
            type Parameters = ();
            type Strategy = proptest::strategy::MapInto<
                proptest::arbitrary::StrategyFor<[u8; 16]>,
                Self,
            >;

            fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
                use proptest::strategy::Strategy;
                proptest::arbitrary::any::<[u8; 16]>().prop_map_into()
            }
        }

        #[cfg(any(test, feature = "arbitrary"))]
        impl From<[u8; 16]> for $name {
            fn from(bytes: [u8; 16]) -> Self {
                Self(Uuid::from_bytes(bytes))
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Entity ID types
// ---------------------------------------------------------------------------

define_id! {
    /// Strongly-typed order identifier.
    OrderId
}

define_id! {
    /// Strongly-typed customer identifier.
    CustomerId
}

define_id! {
    /// Strongly-typed product identifier.
    ProductId
}

define_id! {
    /// Strongly-typed invoice identifier.
    InvoiceId
}

define_id! {
    /// Strongly-typed shipment identifier.
    ShipmentId
}

define_id! {
    /// Strongly-typed return/RMA identifier.
    ReturnId
}

define_id! {
    /// Strongly-typed warehouse identifier.
    WarehouseId
}

define_id! {
    /// Strongly-typed payment identifier.
    PaymentId
}

define_id! {
    /// Strongly-typed inventory item identifier.
    InventoryItemId
}

define_id! {
    /// Strongly-typed subscription identifier.
    SubscriptionId
}

define_id! {
    /// Strongly-typed shopping cart identifier.
    CartId
}

define_id! {
    /// Strongly-typed fulfillment identifier.
    FulfillmentId
}

define_id! {
    /// Strongly-typed order line item identifier.
    OrderItemId
}

define_id! {
    /// Strongly-typed purchase order identifier.
    PurchaseOrderId
}

define_id! {
    /// Strongly-typed promotion identifier.
    PromotionId
}

define_id! {
    /// Strongly-typed warranty identifier.
    WarrantyId
}

define_id! {
    /// Strongly-typed credit memo identifier.
    CreditId
}

define_id! {
    /// Strongly-typed agent identifier (A2A commerce).
    AgentId
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_types_are_distinct() {
        let order_id = OrderId::new();
        let customer_id = CustomerId::new();

        // Both are UUIDs internally, but they're different types
        let _: Uuid = order_id.into();
        let _: Uuid = customer_id.into();

        // This would NOT compile (which is the point):
        // let _: OrderId = customer_id;
    }

    #[test]
    fn roundtrip_display_parse() {
        let id = OrderId::new();
        let s = id.to_string();
        let parsed: OrderId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn serde_roundtrip() {
        let id = ProductId::new();
        let json = serde_json::to_string(&id).unwrap();
        let parsed: ProductId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);

        // Serializes as a plain UUID string
        let uuid_json = serde_json::to_string(id.as_uuid()).unwrap();
        assert_eq!(json, uuid_json);
    }

    #[test]
    fn nil_id() {
        let id = OrderId::nil();
        assert!(id.is_nil());
        assert_eq!(id.to_string(), "00000000-0000-0000-0000-000000000000");
    }

    #[test]
    fn debug_includes_type_name() {
        let id = CustomerId::nil();
        let debug = format!("{:?}", id);
        assert!(debug.starts_with("CustomerId("));
    }

    #[test]
    fn from_uuid_roundtrip() {
        let uuid = Uuid::new_v4();
        let order_id = OrderId::from(uuid);
        assert_eq!(Uuid::from(order_id), uuid);
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn order_id_display_parse_roundtrip(id: OrderId) {
                let s = id.to_string();
                let parsed: OrderId = s.parse().unwrap();
                prop_assert_eq!(id, parsed);
            }

            #[test]
            fn customer_id_display_parse_roundtrip(id: CustomerId) {
                let s = id.to_string();
                let parsed: CustomerId = s.parse().unwrap();
                prop_assert_eq!(id, parsed);
            }

            #[test]
            fn product_id_serde_roundtrip(id: ProductId) {
                let json = serde_json::to_string(&id).unwrap();
                let parsed: ProductId = serde_json::from_str(&json).unwrap();
                prop_assert_eq!(id, parsed);
            }
        }
    }
}

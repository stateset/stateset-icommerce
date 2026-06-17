//! Strongly-typed entity identifiers.
//!
//! Each ID type is a newtype around [`Uuid`] that prevents accidentally mixing up
//! identifiers from different domains. All ID types are `Copy`, `Eq`, `Hash`, and
//! support serialization via `serde`.

use core::fmt;
use serde::{Deserialize, Serialize};
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
        #[must_use]
        pub struct $name(Uuid);

        impl $name {
            /// Create a new random ID (UUID v4).
            #[inline]
            #[cfg(feature = "std")]
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
            pub const fn is_nil(&self) -> bool {
                self.0.is_nil()
            }
        }

        #[cfg(feature = "std")]
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

        impl core::str::FromStr for $name {
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

        #[cfg(feature = "sqlx-postgres")]
        impl sqlx::Type<sqlx::Postgres> for $name {
            fn type_info() -> sqlx::postgres::PgTypeInfo {
                <Uuid as sqlx::Type<sqlx::Postgres>>::type_info()
            }
        }

        #[cfg(feature = "sqlx-postgres")]
        impl sqlx::postgres::PgHasArrayType for $name {
            fn array_type_info() -> sqlx::postgres::PgTypeInfo {
                <Uuid as sqlx::postgres::PgHasArrayType>::array_type_info()
            }
        }

        #[cfg(feature = "sqlx-postgres")]
        impl<'q> sqlx::Encode<'q, sqlx::Postgres> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut sqlx::postgres::PgArgumentBuffer,
            ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
                <Uuid as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&self.0, buf)
            }
        }

        #[cfg(feature = "sqlx-postgres")]
        impl<'r> sqlx::Decode<'r, sqlx::Postgres> for $name {
            fn decode(value: sqlx::postgres::PgValueRef<'r>)
            -> Result<Self, sqlx::error::BoxDynError> {
                <Uuid as sqlx::Decode<'r, sqlx::Postgres>>::decode(value).map(Self)
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

        #[cfg(feature = "rusqlite")]
        impl rusqlite::types::ToSql for $name {
            #[inline]
            fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
                // Write UUID to stack buffer (no heap allocation)
                Ok(rusqlite::types::ToSqlOutput::Owned(
                    rusqlite::types::Value::Text(self.0.to_string()),
                ))
            }
        }

        #[cfg(feature = "rusqlite")]
        impl rusqlite::types::FromSql for $name {
            #[inline]
            fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
                let text = value.as_str()?;
                Uuid::parse_str(text)
                    .map(Self)
                    .map_err(|e| rusqlite::types::FromSqlError::Other(Box::new(e)))
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

define_id! {
    /// Strongly-typed gift card identifier.
    GiftCardId
}

define_id! {
    /// Strongly-typed store credit identifier.
    StoreCreditId
}

define_id! {
    /// Strongly-typed customer segment identifier.
    SegmentId
}

define_id! {
    /// Strongly-typed shipping zone identifier.
    ShippingZoneId
}

define_id! {
    /// Strongly-typed shipping method identifier.
    ShippingMethodId
}

define_id! {
    /// Strongly-typed product review identifier.
    ReviewId
}

define_id! {
    /// Strongly-typed wishlist identifier.
    WishlistId
}

define_id! {
    /// Strongly-typed loyalty program identifier.
    LoyaltyProgramId
}

define_id! {
    /// Strongly-typed reward identifier.
    RewardId
}

define_id! {
    /// Strongly-typed gift card transaction identifier.
    GiftCardTransactionId
}

define_id! {
    /// Strongly-typed store credit transaction identifier.
    StoreCreditTransactionId
}

define_id! {
    /// Strongly-typed loyalty transaction identifier.
    LoyaltyTransactionId
}

define_id! {
    /// Strongly-typed fraud rule identifier.
    FraudRuleId
}

define_id! {
    /// Strongly-typed search configuration identifier.
    SearchConfigId
}

define_id! {
    /// Strongly-typed loyalty account identifier.
    LoyaltyAccountId
}

define_id! {
    /// Strongly-typed sales/fulfillment channel identifier.
    ChannelId
}

define_id! {
    /// Strongly-typed B2B company (account) identifier.
    CompanyId
}

define_id! {
    /// Strongly-typed contact identifier.
    ContactId
}

define_id! {
    /// Strongly-typed company shipping address identifier.
    CompanyAddressId
}

define_id! {
    /// Strongly-typed transfer order identifier.
    TransferOrderId
}

define_id! {
    /// Strongly-typed transfer order line item identifier.
    TransferOrderItemId
}

define_id! {
    /// Strongly-typed unit class identifier (e.g. Length, Weight, Volume).
    UnitClassId
}

define_id! {
    /// Strongly-typed unit of measure identifier.
    UnitOfMeasureId
}

define_id! {
    /// Strongly-typed unit conversion rule identifier.
    UnitConversionRuleId
}

define_id! {
    /// Strongly-typed production batch identifier.
    ProductionBatchId
}

define_id! {
    /// Strongly-typed supplier SKU identifier.
    SupplierSkuId
}

define_id! {
    /// Strongly-typed vendor return identifier.
    VendorReturnId
}

define_id! {
    /// Strongly-typed vendor return line item identifier.
    VendorReturnItemId
}

define_id! {
    /// Strongly-typed vendor credit identifier.
    VendorCreditId
}

define_id! {
    /// Strongly-typed vendor credit application identifier.
    VendorCreditApplicationId
}

define_id! {
    /// Strongly-typed payment obligation identifier.
    PaymentObligationId
}

define_id! {
    /// Strongly-typed price level identifier.
    PriceLevelId
}

define_id! {
    /// Strongly-typed prepayment identifier.
    PrepaymentId
}

define_id! {
    /// Strongly-typed prepayment application identifier.
    PrepaymentApplicationId
}

define_id! {
    /// Strongly-typed price schedule identifier.
    PriceScheduleId
}

define_id! {
    /// Strongly-typed activity log entry identifier.
    ActivityLogId
}

define_id! {
    /// Strongly-typed integration field-mapping identifier.
    IntegrationMappingId
}

define_id! {
    /// Strongly-typed inbound shipment identifier.
    InboundShipmentId
}

define_id! {
    /// Strongly-typed inbound shipment line item identifier.
    InboundShipmentItemId
}

define_id! {
    /// Strongly-typed purgatory (non-posted) order identifier.
    PurgatoryOrderId
}

define_id! {
    /// Strongly-typed purgatory order line item identifier.
    PurgatoryLineItemId
}

define_id! {
    /// Strongly-typed print station identifier.
    PrintStationId
}

define_id! {
    /// Strongly-typed print job identifier.
    PrintJobId
}

define_id! {
    /// Strongly-typed EDI document identifier.
    EdiDocumentId
}

define_id! {
    /// Strongly-typed integration field-path mapping identifier.
    IntegrationFieldMappingId
}

define_id! {
    /// Strongly-typed operational topology snapshot identifier.
    TopologySnapshotId
}

define_id! {
    /// Strongly-typed stock snapshot identifier.
    StockSnapshotId
}

define_id! {
    /// Strongly-typed stock snapshot line identifier.
    StockSnapshotLineId
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_types_are_distinct() {
        let order_id = OrderId::from_uuid(Uuid::from_bytes([1; 16]));
        let customer_id = CustomerId::from_uuid(Uuid::from_bytes([2; 16]));

        // Both are UUIDs internally, but they're different types
        let _: Uuid = order_id.into();
        let _: Uuid = customer_id.into();

        // This would NOT compile (which is the point):
        // let _: OrderId = customer_id;
    }

    #[test]
    fn roundtrip_display_parse() {
        let id = OrderId::from_uuid(Uuid::from_bytes([3; 16]));
        let s = id.to_string();
        let parsed: OrderId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn serde_roundtrip() {
        let id = ProductId::from_uuid(Uuid::from_bytes([4; 16]));
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
        let uuid = Uuid::from_bytes([5; 16]);
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

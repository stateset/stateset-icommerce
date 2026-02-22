//! Conversion traits between domain types and their FFI-safe counterparts.
//!
//! The [`IntoFfi`] and [`FromFfi`] traits centralize conversion logic and
//! make it easy to add new domain ↔ FFI pairs.

use crate::error::FfiErrorCode;

/// Convert a domain type into its FFI-safe representation.
#[allow(clippy::wrong_self_convention)]
pub trait IntoFfi<F> {
    /// Perform the conversion.
    ///
    /// This is infallible because we control both sides of the conversion
    /// and domain types are always representable in the FFI layer.
    fn into_ffi(&self) -> F;
}

/// Convert an FFI-safe type back into a domain type.
pub trait FromFfi<F>: Sized {
    /// Attempt the conversion.
    ///
    /// This can fail (e.g. invalid currency bytes, null string pointers).
    fn from_ffi(ffi: &F) -> Result<Self, FfiErrorCode>;
}

// ---------------------------------------------------------------------------
// IntoFfi implementations
// ---------------------------------------------------------------------------

use stateset_core::models::order::Order;
use stateset_core::models::customer::Customer;
use stateset_core::models::product::Product;
use stateset_core::models::inventory::StockLevel;
use stateset_primitives::Money;

use crate::types::{
    FfiOrder, FfiCustomer, FfiProduct, FfiInventoryLevel, FfiMoney, FfiUuid,
};

impl IntoFfi<FfiOrder> for Order {
    fn into_ffi(&self) -> FfiOrder {
        FfiOrder::from(self)
    }
}

impl IntoFfi<FfiCustomer> for Customer {
    fn into_ffi(&self) -> FfiCustomer {
        FfiCustomer::from_domain(self)
    }
}

impl IntoFfi<FfiProduct> for Product {
    fn into_ffi(&self) -> FfiProduct {
        FfiProduct::from_domain(self)
    }
}

impl IntoFfi<FfiInventoryLevel> for StockLevel {
    fn into_ffi(&self) -> FfiInventoryLevel {
        FfiInventoryLevel::from_stock_level(self)
    }
}

impl IntoFfi<FfiMoney> for Money {
    fn into_ffi(&self) -> FfiMoney {
        FfiMoney::from(*self)
    }
}

impl IntoFfi<FfiUuid> for uuid::Uuid {
    fn into_ffi(&self) -> FfiUuid {
        FfiUuid::from(*self)
    }
}

// ---------------------------------------------------------------------------
// FromFfi implementations
// ---------------------------------------------------------------------------

impl FromFfi<FfiMoney> for Money {
    fn from_ffi(ffi: &FfiMoney) -> Result<Self, FfiErrorCode> {
        Self::try_from(*ffi).map_err(|_| FfiErrorCode::InvalidArgument)
    }
}

impl FromFfi<FfiUuid> for uuid::Uuid {
    fn from_ffi(ffi: &FfiUuid) -> Result<Self, FfiErrorCode> {
        Ok(Self::from(*ffi))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------


#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use stateset_core::models::customer::CustomerStatus;
    use stateset_core::models::order::{
        FulfillmentStatus, OrderItem, OrderStatus, PaymentStatus,
    };
    use stateset_core::models::product::{ProductStatus, ProductType};
    use stateset_primitives::{
        CurrencyCode, CustomerId, OrderId, OrderItemId, ProductId,
    };
    use uuid::Uuid;

    fn make_order() -> Order {
        let now = Utc::now();
        Order {
            id: OrderId::new(),
            order_number: "ORD-1".to_string(),
            customer_id: CustomerId::new(),
            status: OrderStatus::Pending,
            order_date: now,
            total_amount: dec!(100),
            currency: "USD".to_string(),
            payment_status: PaymentStatus::Pending,
            fulfillment_status: FulfillmentStatus::Unfulfilled,
            payment_method: None,
            shipping_method: None,
            tracking_number: None,
            notes: None,
            shipping_address: None,
            billing_address: None,
            items: vec![OrderItem {
                id: OrderItemId::new(),
                order_id: OrderId::new(),
                product_id: ProductId::new(),
                variant_id: None,
                sku: "S".into(),
                name: "N".into(),
                quantity: 1,
                unit_price: dec!(100),
                discount: dec!(0),
                tax_amount: dec!(0),
                total: dec!(100),
            }],
            version: 1,
            created_at: now,
            updated_at: now,
        }
    }

    fn make_customer() -> Customer {
        let now = Utc::now();
        Customer {
            id: CustomerId::new(),
            email: "bob@example.com".to_string(),
            first_name: "Bob".to_string(),
            last_name: "Jones".to_string(),
            phone: None,
            status: CustomerStatus::Active,
            accepts_marketing: false,
            email_verified: true,
            tags: vec![],
            metadata: None,
            default_shipping_address_id: None,
            default_billing_address_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn make_product() -> Product {
        let now = Utc::now();
        Product {
            id: ProductId::new(),
            name: "Gadget".to_string(),
            slug: "gadget".to_string(),
            description: "A gadget".to_string(),
            status: ProductStatus::Active,
            product_type: ProductType::Simple,
            attributes: vec![],
            seo: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn make_stock_level() -> StockLevel {
        StockLevel {
            sku: "G-001".to_string(),
            name: "Gadget".to_string(),
            total_on_hand: dec!(50),
            total_allocated: dec!(10),
            total_available: dec!(40),
            locations: vec![],
        }
    }

    #[test]
    fn order_into_ffi() {
        let order = make_order();
        let ffi: FfiOrder = order.into_ffi();
        assert_eq!(ffi.id, FfiUuid::from(order.id));
        assert_eq!(ffi.item_count, 1);
    }

    #[test]
    fn customer_into_ffi() {
        let customer = make_customer();
        let ffi: FfiCustomer = customer.into_ffi();
        assert_eq!(ffi.id, FfiUuid::from(customer.id));
        crate::types::customer::stateset_customer_free(ffi);
    }

    #[test]
    fn product_into_ffi() {
        let product = make_product();
        let ffi: FfiProduct = product.into_ffi();
        assert_eq!(ffi.id, FfiUuid::from(product.id));
        crate::types::product::stateset_product_free(ffi);
    }

    #[test]
    fn inventory_into_ffi() {
        let stock = make_stock_level();
        let ffi: FfiInventoryLevel = stock.into_ffi();
        assert_eq!(ffi.quantity, 50);
        assert_eq!(ffi.available, 40);
    }

    #[test]
    fn money_into_ffi() {
        let money = Money::new(dec!(9.99), CurrencyCode::USD);
        let ffi: FfiMoney = money.into_ffi();
        assert_eq!(ffi.amount_cents, 999);
    }

    #[test]
    fn money_from_ffi() {
        let ffi = FfiMoney { amount_cents: 1500, currency: *b"EUR" };
        let money = Money::from_ffi(&ffi).unwrap();
        assert_eq!(money.amount(), dec!(15));
        assert_eq!(money.currency(), CurrencyCode::EUR);
    }

    #[test]
    fn money_from_ffi_invalid_currency() {
        let ffi = FfiMoney { amount_cents: 100, currency: [0, 0, 0] };
        assert!(Money::from_ffi(&ffi).is_err());
    }

    #[test]
    fn uuid_into_ffi() {
        let uuid = Uuid::new_v4();
        let ffi: FfiUuid = uuid.into_ffi();
        assert_eq!(ffi.bytes, *uuid.as_bytes());
    }

    #[test]
    fn uuid_from_ffi() {
        let ffi = FfiUuid::from(Uuid::new_v4());
        let uuid = Uuid::from_ffi(&ffi).unwrap();
        assert_eq!(*uuid.as_bytes(), ffi.bytes);
    }
}

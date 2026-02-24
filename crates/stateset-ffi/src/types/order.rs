//! ABI-safe order types.

use stateset_core::models::order::{Order, OrderStatus};

use super::ids::FfiUuid;
use super::money::FfiMoney;

/// ABI-safe order status.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum FfiOrderStatus {
    /// Order is pending.
    #[default]
    Pending = 0,
    /// Order is confirmed.
    Confirmed = 1,
    /// Order is being processed.
    Processing = 2,
    /// Order has been shipped.
    Shipped = 3,
    /// Order has been delivered.
    Delivered = 4,
    /// Order was cancelled.
    Cancelled = 5,
    /// Order was refunded.
    Refunded = 6,
}

impl From<OrderStatus> for FfiOrderStatus {
    fn from(s: OrderStatus) -> Self {
        match s {
            OrderStatus::Pending => Self::Pending,
            OrderStatus::Confirmed => Self::Confirmed,
            OrderStatus::Processing => Self::Processing,
            OrderStatus::Shipped => Self::Shipped,
            OrderStatus::Delivered => Self::Delivered,
            OrderStatus::Cancelled => Self::Cancelled,
            OrderStatus::Refunded => Self::Refunded,
            // non_exhaustive fallback
            _ => Self::Pending,
        }
    }
}

impl From<FfiOrderStatus> for OrderStatus {
    fn from(s: FfiOrderStatus) -> Self {
        match s {
            FfiOrderStatus::Pending => Self::Pending,
            FfiOrderStatus::Confirmed => Self::Confirmed,
            FfiOrderStatus::Processing => Self::Processing,
            FfiOrderStatus::Shipped => Self::Shipped,
            FfiOrderStatus::Delivered => Self::Delivered,
            FfiOrderStatus::Cancelled => Self::Cancelled,
            FfiOrderStatus::Refunded => Self::Refunded,
        }
    }
}

/// ABI-safe order summary.
///
/// This is a flattened, C-compatible projection of the full [`Order`] type.
/// String fields (order number, notes) are intentionally omitted — callers
/// can retrieve them through JSON serialization if needed.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FfiOrder {
    /// Order UUID.
    pub id: FfiUuid,
    /// Customer UUID.
    pub customer_id: FfiUuid,
    /// Current order status.
    pub status: FfiOrderStatus,
    /// Total amount in minor units + currency.
    pub total: FfiMoney,
    /// Number of line items.
    pub item_count: u32,
    /// Unix timestamp in milliseconds when the order was created.
    pub created_at_epoch_ms: i64,
}

impl From<&Order> for FfiOrder {
    fn from(order: &Order) -> Self {
        let cents = (order.total_amount * rust_decimal::Decimal::from(100)).to_i64().unwrap_or(0);

        let mut currency = [0u8; 3];
        let code_bytes = order.currency.as_bytes();
        let len = code_bytes.len().min(3);
        currency[..len].copy_from_slice(&code_bytes[..len]);

        Self {
            id: FfiUuid::from(order.id),
            customer_id: FfiUuid::from(order.customer_id),
            status: FfiOrderStatus::from(order.status),
            total: FfiMoney { amount_cents: cents, currency },
            item_count: order.items.len() as u32,
            created_at_epoch_ms: order.created_at.timestamp_millis(),
        }
    }
}

use rust_decimal::prelude::ToPrimitive;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use stateset_core::models::order::{FulfillmentStatus, OrderItem, PaymentStatus};
    use stateset_primitives::{CustomerId, OrderId, OrderItemId, ProductId};

    fn make_test_order(status: OrderStatus) -> Order {
        let now = Utc::now();
        let item = OrderItem {
            id: OrderItemId::new(),
            order_id: OrderId::new(),
            product_id: ProductId::new(),
            variant_id: None,
            sku: "SKU-001".to_string(),
            name: "Widget".to_string(),
            quantity: 2,
            unit_price: dec!(29.99),
            discount: dec!(0),
            tax_amount: dec!(4.80),
            total: dec!(64.78),
        };

        Order {
            id: OrderId::new(),
            order_number: "ORD-001".to_string(),
            customer_id: CustomerId::new(),
            status,
            order_date: now,
            total_amount: dec!(64.78),
            currency: "USD".to_string(),
            payment_status: PaymentStatus::Pending,
            fulfillment_status: FulfillmentStatus::Unfulfilled,
            payment_method: None,
            shipping_method: None,
            tracking_number: None,
            notes: None,
            shipping_address: None,
            billing_address: None,
            items: vec![item],
            version: 1,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn order_status_roundtrip_pending() {
        let ffi: FfiOrderStatus = OrderStatus::Pending.into();
        let back: OrderStatus = ffi.into();
        assert_eq!(back, OrderStatus::Pending);
    }

    #[test]
    fn order_status_roundtrip_all() {
        let statuses = [
            OrderStatus::Pending,
            OrderStatus::Confirmed,
            OrderStatus::Processing,
            OrderStatus::Shipped,
            OrderStatus::Delivered,
            OrderStatus::Cancelled,
            OrderStatus::Refunded,
        ];
        for s in statuses {
            let ffi: FfiOrderStatus = s.into();
            let back: OrderStatus = ffi.into();
            assert_eq!(s, back);
        }
    }

    #[test]
    fn order_status_values() {
        assert_eq!(FfiOrderStatus::Pending as i32, 0);
        assert_eq!(FfiOrderStatus::Confirmed as i32, 1);
        assert_eq!(FfiOrderStatus::Processing as i32, 2);
        assert_eq!(FfiOrderStatus::Shipped as i32, 3);
        assert_eq!(FfiOrderStatus::Delivered as i32, 4);
        assert_eq!(FfiOrderStatus::Cancelled as i32, 5);
        assert_eq!(FfiOrderStatus::Refunded as i32, 6);
    }

    #[test]
    fn order_status_default() {
        assert_eq!(FfiOrderStatus::default(), FfiOrderStatus::Pending);
    }

    #[test]
    fn ffi_order_from_domain() {
        let order = make_test_order(OrderStatus::Processing);
        let ffi = FfiOrder::from(&order);

        assert_eq!(ffi.id, FfiUuid::from(order.id));
        assert_eq!(ffi.customer_id, FfiUuid::from(order.customer_id));
        assert_eq!(ffi.status, FfiOrderStatus::Processing);
        assert_eq!(ffi.total.amount_cents, 6478);
        assert_eq!(&ffi.total.currency, b"USD");
        assert_eq!(ffi.item_count, 1);
        assert!(ffi.created_at_epoch_ms > 0);
    }

    #[test]
    fn ffi_order_default() {
        let ffi = FfiOrder::default();
        assert!(ffi.id.is_nil());
        assert_eq!(ffi.status, FfiOrderStatus::Pending);
        assert_eq!(ffi.total.amount_cents, 0);
        assert_eq!(ffi.item_count, 0);
    }

    #[test]
    fn ffi_order_preserves_customer_id() {
        let order = make_test_order(OrderStatus::Pending);
        let ffi = FfiOrder::from(&order);
        let back_customer: CustomerId = ffi.customer_id.into();
        assert_eq!(back_customer, order.customer_id);
    }

    #[test]
    fn ffi_order_debug() {
        let order = make_test_order(OrderStatus::Shipped);
        let ffi = FfiOrder::from(&order);
        let debug = format!("{:?}", ffi);
        assert!(debug.contains("FfiOrder"));
        assert!(debug.contains("Shipped"));
    }

    #[test]
    fn ffi_order_status_eq_and_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(FfiOrderStatus::Pending);
        set.insert(FfiOrderStatus::Shipped);
        assert_eq!(set.len(), 2);
        assert!(set.contains(&FfiOrderStatus::Pending));
    }
}

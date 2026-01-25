//! Prelude module for the orders domain
//! Re-exports common types for convenient imports

pub use crate::orders::Orders;
pub use stateset_core::{
    Address, CreateOrder, CreateOrderItem, FulfillmentStatus, Order, OrderFilter, OrderItem,
    OrderStatus, PaymentStatus, UpdateOrder,
};

/// Cart types related to orders
pub use stateset_core::{
    AddCartItem, Cart, CartAddress, CartItem, CartPaymentStatus, CartStatus, CreateCart,
    SetCartShipping, ShippingRate, UpdateCartItem,
};

#[cfg(feature = "events")]
pub use crate::events::CommerceEvent;

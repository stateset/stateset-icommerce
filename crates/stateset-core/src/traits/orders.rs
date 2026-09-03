//! Order, cart, and backorder repositories.

use super::*;

/// Order repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait OrderRepository: Send + Sync {
    /// Create a new order
    fn create(&self, input: CreateOrder) -> Result<Order>;

    /// Create an order for a cart, returning the existing order when that cart
    /// has already been checked out. This is the durable idempotency boundary
    /// for retryable checkout adapters.
    fn create_from_cart(&self, cart_id: CartId, input: CreateOrder) -> Result<Order>;

    /// Get order by ID
    fn get(&self, id: OrderId) -> Result<Option<Order>>;

    /// Get order by order number
    fn get_by_number(&self, order_number: &str) -> Result<Option<Order>>;

    /// Update an order (field patches and/or a status transition) in one
    /// transaction, with every side effect of the transition.
    ///
    /// Cancelling (`status: Some(Cancelled)`) releases the order's inventory
    /// reservations and backorders, and is subject to the money rule: while
    /// any payment against the order still holds money (in flight or
    /// captured, see [`crate::PaymentRepository::open_captures_for_order`]) the
    /// cancel is refused with a `ValidationError` naming the outstanding
    /// amount, unless [`crate::UpdateOrder::void_payments`] is set — then in-flight
    /// payments are voided in the same transaction and settled ones are left
    /// for an explicit refund.
    fn update(&self, id: OrderId, input: UpdateOrder) -> Result<Order>;

    /// Ship an order, fully or per line.
    ///
    /// Increments each line's `shipped_quantity` (all remaining units when
    /// `input.lines` is `None`), confirms the shipped portion of the order's
    /// inventory reservations, and moves the order to
    /// [`crate::OrderStatus::PartiallyShipped`] or
    /// [`crate::OrderStatus::Shipped`] — all in
    /// one transaction. Fails with
    /// [`crate::CommerceError::ShipmentExceedsOrdered`] when a line would overship.
    fn ship(&self, id: OrderId, input: ShipOrder) -> Result<Order>;

    /// List orders with filter
    fn list(&self, filter: OrderFilter) -> Result<Vec<Order>>;

    /// Delete an order and everything it holds (lines, reservations,
    /// backorders). A missing id is a no-op.
    ///
    /// Refused with `Conflict` when the order is a fulfilment record
    /// (shipped/delivered/refunded, [`crate::OrderStatus::allows_delete`]) or a
    /// financial record: its `payment_status` holds money
    /// ([`crate::PaymentStatus::holds_money`]) or any payment row references it.
    fn delete(&self, id: OrderId) -> Result<()>;

    /// Add a line to a pre-fulfilment order: inserts the line, reserves its
    /// stock (backordering any shortfall), recomputes the total and writes an
    /// `orders.item_added.v1` outbox event, all in one transaction.
    fn add_item(&self, order_id: OrderId, item: CreateOrderItem) -> Result<OrderItem>;

    /// Remove a line from a pre-fulfilment order: releases the line's own
    /// reservations, cancels its backorder, recomputes the total and writes
    /// an `orders.item_removed.v1` outbox event, all in one transaction.
    ///
    /// Subject to the money rule of [`crate::RemoveOrderItem`]: refused with
    /// [`crate::CommerceError::OrderTotalBelowCaptured`] when the order's new
    /// total would fall below the money already captured against it.
    /// [`Self::remove_item_with`] can opt out.
    fn remove_item(&self, order_id: OrderId, item_id: OrderItemId) -> Result<()> {
        self.remove_item_with(order_id, item_id, RemoveOrderItem::default())
    }

    /// [`Self::remove_item`] with explicit handling of the order's captured
    /// money; see [`crate::RemoveOrderItem::allow_overpayment`].
    fn remove_item_with(
        &self,
        order_id: OrderId,
        item_id: OrderItemId,
        input: RemoveOrderItem,
    ) -> Result<()>;

    /// Count orders matching filter
    fn count(&self, filter: OrderFilter) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple orders - partial success allowed
    fn create_batch(&self, inputs: Vec<CreateOrder>) -> Result<BatchResult<Order>>;

    /// Create multiple orders - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreateOrder>) -> Result<Vec<Order>>;

    /// Update multiple orders - partial success allowed
    fn update_batch(&self, updates: Vec<(OrderId, UpdateOrder)>) -> Result<BatchResult<Order>>;

    /// Update multiple orders - atomic (all-or-nothing)
    fn update_batch_atomic(&self, updates: Vec<(OrderId, UpdateOrder)>) -> Result<Vec<Order>>;

    /// Delete multiple orders - partial success allowed
    fn delete_batch(&self, ids: Vec<OrderId>) -> Result<BatchResult<OrderId>>;

    /// Delete multiple orders - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<OrderId>) -> Result<()>;

    /// Get multiple orders by ID
    fn get_batch(&self, ids: Vec<OrderId>) -> Result<Vec<Order>>;
}

/// Cart/Checkout repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait CartRepository: Send + Sync {
    /// Create a new cart/checkout session
    fn create(&self, input: CreateCart) -> Result<Cart>;

    /// Get cart by ID
    fn get(&self, id: CartId) -> Result<Option<Cart>>;

    /// Get cart by cart number
    fn get_by_number(&self, cart_number: &str) -> Result<Option<Cart>>;

    /// Update a cart
    fn update(&self, id: CartId, input: UpdateCart) -> Result<Cart>;

    /// List carts with filter
    fn list(&self, filter: CartFilter) -> Result<Vec<Cart>>;

    /// Get carts for a customer
    fn for_customer(&self, customer_id: CustomerId) -> Result<Vec<Cart>>;

    /// Delete a cart (or mark as cancelled)
    fn delete(&self, id: CartId) -> Result<()>;

    // Item operations
    /// Add item to cart
    fn add_item(&self, cart_id: CartId, item: AddCartItem) -> Result<CartItem>;

    /// Update a cart item (quantity, etc)
    fn update_item(&self, item_id: Uuid, input: UpdateCartItem) -> Result<CartItem>;

    /// Remove item from cart
    fn remove_item(&self, item_id: Uuid) -> Result<()>;

    /// Get items for a cart
    fn get_items(&self, cart_id: CartId) -> Result<Vec<CartItem>>;

    /// Get a single cart line by its id (any cart)
    fn get_item(&self, item_id: Uuid) -> Result<Option<CartItem>>;

    /// Clear all items from cart
    fn clear_items(&self, cart_id: CartId) -> Result<()>;

    // Address operations
    /// Set shipping address
    fn set_shipping_address(&self, id: CartId, address: CartAddress) -> Result<Cart>;

    /// Set billing address
    fn set_billing_address(&self, id: CartId, address: CartAddress) -> Result<Cart>;

    // Shipping operations
    /// Set shipping method
    fn set_shipping(&self, id: CartId, shipping: SetCartShipping) -> Result<Cart>;

    /// Get available shipping rates for cart
    fn get_shipping_rates(&self, id: CartId) -> Result<Vec<ShippingRate>>;

    // Payment operations
    /// Set payment method/token
    fn set_payment(&self, id: CartId, payment: SetCartPayment) -> Result<Cart>;

    /// Set x402 payment method (stablecoin)
    fn set_x402_payment(&self, id: CartId, payment: SetCartX402Payment) -> Result<Cart>;

    /// Complete checkout with x402 payment
    /// Returns `PaymentRequired` if no intent exists, `IntentCreated` if awaiting signature,
    /// `AwaitingSettlement` if signed but not settled, or Completed if settled
    fn complete_with_x402(&self, id: CartId, payee_address: &str) -> Result<X402CheckoutResult>;

    // Discount operations
    /// Apply coupon/discount code
    fn apply_discount(&self, id: CartId, coupon_code: &str) -> Result<Cart>;

    /// Remove discount
    fn remove_discount(&self, id: CartId) -> Result<Cart>;

    // Status transitions
    /// Mark cart as ready for payment (validates all requirements met)
    fn mark_ready_for_payment(&self, id: CartId) -> Result<Cart>;

    /// Begin checkout/payment process
    fn begin_checkout(&self, id: CartId) -> Result<Cart>;

    /// Complete checkout (creates order, returns checkout result).
    ///
    /// The minted order is `Confirmed` with payment left `Pending`; record the
    /// payment through the payments API (or use
    /// [`complete_settled_externally`](Self::complete_settled_externally) when
    /// settlement genuinely happened out of band).
    fn complete(&self, id: CartId) -> Result<CheckoutResult>;

    /// Complete checkout for a cart whose payment was settled outside the
    /// engine (ACP, external PSP). Explicitly opts in to minting an order that
    /// is `Confirmed` + `Paid` with no engine-side payment record.
    fn complete_settled_externally(&self, id: CartId) -> Result<CheckoutResult>;

    /// Cancel a cart
    fn cancel(&self, id: CartId) -> Result<Cart>;

    /// Mark cart as abandoned
    fn abandon(&self, id: CartId) -> Result<Cart>;

    /// Expire a cart
    fn expire(&self, id: CartId) -> Result<Cart>;

    // Inventory operations
    /// Reserve inventory for cart items
    fn reserve_inventory(&self, id: CartId) -> Result<Cart>;

    /// Release inventory reservations
    fn release_inventory(&self, id: CartId) -> Result<Cart>;

    // Totals
    /// Recalculate cart totals
    fn recalculate(&self, id: CartId) -> Result<Cart>;

    /// Set tax amount
    fn set_tax(&self, id: CartId, tax_amount: rust_decimal::Decimal) -> Result<Cart>;

    // Queries
    /// Get abandoned carts (for recovery campaigns)
    fn get_abandoned(&self) -> Result<Vec<Cart>>;

    /// Get expired carts
    fn get_expired(&self) -> Result<Vec<Cart>>;

    /// Count carts matching filter
    fn count(&self, filter: CartFilter) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple carts - partial success allowed
    fn create_batch(&self, inputs: Vec<CreateCart>) -> Result<BatchResult<Cart>>;

    /// Create multiple carts - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreateCart>) -> Result<Vec<Cart>>;

    /// Update multiple carts - partial success allowed
    fn update_batch(&self, updates: Vec<(CartId, UpdateCart)>) -> Result<BatchResult<Cart>>;

    /// Update multiple carts - atomic (all-or-nothing)
    fn update_batch_atomic(&self, updates: Vec<(CartId, UpdateCart)>) -> Result<Vec<Cart>>;

    /// Delete multiple carts - partial success allowed
    fn delete_batch(&self, ids: Vec<CartId>) -> Result<BatchResult<CartId>>;

    /// Delete multiple carts - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<CartId>) -> Result<()>;

    /// Get multiple carts by ID
    fn get_batch(&self, ids: Vec<CartId>) -> Result<Vec<Cart>>;
}

/// Backorder repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait BackorderRepository: Send + Sync {
    // Backorder operations
    /// Create a backorder
    fn create_backorder(&self, input: CreateBackorder) -> Result<Backorder>;

    /// Get backorder by ID
    fn get_backorder(&self, id: Uuid) -> Result<Option<Backorder>>;

    /// Get backorder by number
    fn get_backorder_by_number(&self, number: &str) -> Result<Option<Backorder>>;

    /// Update backorder
    fn update_backorder(&self, id: Uuid, input: UpdateBackorder) -> Result<Backorder>;

    /// List backorders
    fn list_backorders(&self, filter: BackorderFilter) -> Result<Vec<Backorder>>;

    /// Cancel backorder
    fn cancel_backorder(&self, id: Uuid) -> Result<Backorder>;

    /// Get backorders for order
    fn get_backorders_for_order(&self, order_id: Uuid) -> Result<Vec<Backorder>>;

    /// Get backorders for customer
    fn get_backorders_for_customer(&self, customer_id: Uuid) -> Result<Vec<Backorder>>;

    /// Get backorders for SKU
    fn get_backorders_for_sku(&self, sku: &str) -> Result<Vec<Backorder>>;

    // Fulfillment operations
    /// Fulfill backorder (partial or full)
    fn fulfill_backorder(&self, input: FulfillBackorder) -> Result<Backorder>;

    /// Get fulfillment history for backorder
    fn get_fulfillment_history(&self, backorder_id: Uuid) -> Result<Vec<BackorderFulfillment>>;

    // Allocation operations
    /// Allocate inventory to backorder
    fn allocate_backorder(&self, input: AllocateBackorder) -> Result<BackorderAllocation>;

    /// Get allocations for backorder
    fn get_allocations(&self, backorder_id: Uuid) -> Result<Vec<BackorderAllocation>>;

    /// Release allocation
    fn release_allocation(&self, allocation_id: Uuid) -> Result<BackorderAllocation>;

    /// Confirm allocation
    fn confirm_allocation(&self, allocation_id: Uuid) -> Result<BackorderAllocation>;

    /// Expire old allocations
    fn expire_allocations(&self) -> Result<u32>;

    // Auto-allocation
    /// Auto-allocate available inventory to pending backorders
    fn auto_allocate_inventory(&self, sku: &str) -> Result<Vec<BackorderAllocation>>;

    // Analytics
    /// Get backorder summary
    fn get_summary(&self) -> Result<BackorderSummary>;

    /// Get SKU backorder summary
    fn get_sku_summary(&self, sku: &str) -> Result<Option<SkuBackorderSummary>>;

    /// Get overdue backorders
    fn get_overdue_backorders(&self) -> Result<Vec<Backorder>>;

    /// Count pending backorders
    fn count_pending(&self) -> Result<u64>;
}

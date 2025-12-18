"""Type stubs for stateset_embedded"""

from typing import List, Optional

__version__: str

# ============================================================================
# Commerce
# ============================================================================

class Commerce:
    """Main Commerce instance for local commerce operations."""

    def __init__(self, db_path: str) -> None:
        """Create a new Commerce instance with a database path.

        Args:
            db_path: Path to SQLite database file, or ":memory:" for in-memory.
        """
        ...

    @property
    def customers(self) -> Customers:
        """Get the customers API."""
        ...

    @property
    def orders(self) -> Orders:
        """Get the orders API."""
        ...

    @property
    def products(self) -> Products:
        """Get the products API."""
        ...

    @property
    def inventory(self) -> Inventory:
        """Get the inventory API."""
        ...

    @property
    def returns(self) -> Returns:
        """Get the returns API."""
        ...

    @property
    def payments(self) -> Payments:
        """Get the payments API."""
        ...

    @property
    def shipments(self) -> Shipments:
        """Get the shipments API."""
        ...

    @property
    def warranties(self) -> Warranties:
        """Get the warranties API."""
        ...

    @property
    def purchase_orders(self) -> PurchaseOrders:
        """Get the purchase orders API."""
        ...

    @property
    def invoices(self) -> Invoices:
        """Get the invoices API."""
        ...

    @property
    def bom(self) -> BomApi:
        """Get the bill of materials API."""
        ...

    @property
    def work_orders(self) -> WorkOrders:
        """Get the work orders API."""
        ...

    @property
    def carts(self) -> Carts:
        """Get the carts API."""
        ...

    @property
    def analytics(self) -> Analytics:
        """Get the analytics API."""
        ...

    @property
    def currency(self) -> CurrencyOperations:
        """Get the currency API."""
        ...

# ============================================================================
# Customers
# ============================================================================

class Customer:
    """Customer data returned from operations."""

    id: str
    email: str
    first_name: str
    last_name: str
    phone: Optional[str]
    status: str
    accepts_marketing: bool
    created_at: str
    updated_at: str

    @property
    def full_name(self) -> str:
        """Get the full name."""
        ...

class Customers:
    """Customer management operations."""

    def create(
        self,
        email: str,
        first_name: str,
        last_name: str,
        phone: Optional[str] = None,
        accepts_marketing: Optional[bool] = None,
    ) -> Customer:
        """Create a new customer.

        Args:
            email: Customer email address
            first_name: First name
            last_name: Last name
            phone: Phone number (optional)
            accepts_marketing: Marketing opt-in (optional)

        Returns:
            The created customer
        """
        ...

    def get(self, id: str) -> Optional[Customer]:
        """Get a customer by ID."""
        ...

    def get_by_email(self, email: str) -> Optional[Customer]:
        """Get a customer by email."""
        ...

    def list(self) -> List[Customer]:
        """List all customers."""
        ...

    def count(self) -> int:
        """Count customers."""
        ...

# ============================================================================
# Orders
# ============================================================================

class OrderItem:
    """Order line item."""

    id: str
    sku: str
    name: str
    quantity: int
    unit_price: float
    total: float

class Order:
    """Order data returned from operations."""

    id: str
    order_number: str
    customer_id: str
    status: str
    total_amount: float
    currency: str
    payment_status: str
    fulfillment_status: str
    tracking_number: Optional[str]
    items: List[OrderItem]
    created_at: str
    updated_at: str

    @property
    def item_count(self) -> int:
        """Get the number of items in the order."""
        ...

class CreateOrderItemInput:
    """Input for creating an order item."""

    sku: str
    name: str
    quantity: int
    unit_price: float
    product_id: Optional[str]
    variant_id: Optional[str]

    def __init__(
        self,
        sku: str,
        name: str,
        quantity: int,
        unit_price: float,
        product_id: Optional[str] = None,
        variant_id: Optional[str] = None,
    ) -> None: ...

class Orders:
    """Order management operations."""

    def create(
        self,
        customer_id: str,
        items: List[CreateOrderItemInput],
        currency: Optional[str] = None,
        notes: Optional[str] = None,
    ) -> Order:
        """Create a new order."""
        ...

    def get(self, id: str) -> Optional[Order]:
        """Get an order by ID."""
        ...

    def list(self) -> List[Order]:
        """List all orders."""
        ...

    def update_status(self, id: str, status: str) -> Order:
        """Update order status."""
        ...

    def ship(self, id: str, tracking_number: Optional[str] = None) -> Order:
        """Ship an order."""
        ...

    def cancel(self, id: str) -> Order:
        """Cancel an order."""
        ...

    def count(self) -> int:
        """Count orders."""
        ...

# ============================================================================
# Products
# ============================================================================

class Product:
    """Product data returned from operations."""

    id: str
    name: str
    slug: str
    description: str
    status: str
    created_at: str
    updated_at: str

class ProductVariant:
    """Product variant data."""

    id: str
    product_id: str
    sku: str
    name: str
    price: float
    compare_at_price: Optional[float]
    is_default: bool

class CreateProductVariantInput:
    """Input for creating a product variant."""

    sku: str
    name: Optional[str]
    price: float
    compare_at_price: Optional[float]

    def __init__(
        self,
        sku: str,
        price: float,
        name: Optional[str] = None,
        compare_at_price: Optional[float] = None,
    ) -> None: ...

class Products:
    """Product catalog operations."""

    def create(
        self,
        name: str,
        description: Optional[str] = None,
        variants: Optional[List[CreateProductVariantInput]] = None,
    ) -> Product:
        """Create a new product."""
        ...

    def get(self, id: str) -> Optional[Product]:
        """Get a product by ID."""
        ...

    def get_variant_by_sku(self, sku: str) -> Optional[ProductVariant]:
        """Get a product variant by SKU."""
        ...

    def list(self) -> List[Product]:
        """List all products."""
        ...

    def count(self) -> int:
        """Count products."""
        ...

# ============================================================================
# Inventory
# ============================================================================

class InventoryItem:
    """Inventory item data."""

    id: int
    sku: str
    name: str
    description: Optional[str]
    unit_of_measure: str
    is_active: bool

class StockLevel:
    """Stock level information."""

    sku: str
    name: str
    total_on_hand: float
    total_allocated: float
    total_available: float

class Reservation:
    """Inventory reservation."""

    id: str
    item_id: int
    quantity: float
    status: str

class Inventory:
    """Inventory management operations."""

    def create_item(
        self,
        sku: str,
        name: str,
        description: Optional[str] = None,
        initial_quantity: Optional[float] = None,
        reorder_point: Optional[float] = None,
    ) -> InventoryItem:
        """Create a new inventory item."""
        ...

    def get_stock(self, sku: str) -> Optional[StockLevel]:
        """Get stock level for a SKU."""
        ...

    def adjust(self, sku: str, quantity: float, reason: str) -> None:
        """Adjust inventory quantity."""
        ...

    def reserve(
        self,
        sku: str,
        quantity: float,
        reference_type: str,
        reference_id: str,
        expires_in_seconds: Optional[int] = None,
    ) -> Reservation:
        """Reserve inventory for an order."""
        ...

    def confirm_reservation(self, reservation_id: str) -> None:
        """Confirm a reservation."""
        ...

    def release_reservation(self, reservation_id: str) -> None:
        """Release a reservation."""
        ...

# ============================================================================
# Returns
# ============================================================================

class Return:
    """Return request data."""

    id: str
    order_id: str
    status: str
    reason: str
    created_at: str

class CreateReturnItemInput:
    """Input for creating a return item."""

    order_item_id: str
    quantity: int

    def __init__(self, order_item_id: str, quantity: int) -> None: ...

class Returns:
    """Return processing operations."""

    def create(
        self,
        order_id: str,
        reason: str,
        items: List[CreateReturnItemInput],
        reason_details: Optional[str] = None,
    ) -> Return:
        """Create a new return request."""
        ...

    def get(self, id: str) -> Optional[Return]:
        """Get a return by ID."""
        ...

    def approve(self, id: str) -> Return:
        """Approve a return request."""
        ...

    def reject(self, id: str, reason: str) -> Return:
        """Reject a return request."""
        ...

    def list(self) -> List[Return]:
        """List all returns."""
        ...

    def count(self) -> int:
        """Count returns."""
        ...

# ============================================================================
# Payments
# ============================================================================

class Payment:
    """Payment data returned from operations."""

    id: str
    payment_number: str
    order_id: Optional[str]
    invoice_id: Optional[str]
    customer_id: Optional[str]
    amount: float
    currency: str
    status: str
    payment_method: str
    created_at: str
    updated_at: str

class Refund:
    """Refund data returned from operations."""

    id: str
    payment_id: str
    amount: float
    status: str
    reason: Optional[str]
    created_at: str

class Payments:
    """Payment processing operations."""

    def create(
        self,
        amount: float,
        currency: Optional[str] = None,
        order_id: Optional[str] = None,
        customer_id: Optional[str] = None,
        payment_method: Optional[str] = None,
    ) -> Payment: ...

    def get(self, id: str) -> Optional[Payment]: ...

    def list(self) -> List[Payment]: ...

    def complete(self, id: str) -> Payment: ...

    def mark_failed(self, id: str, reason: str, code: Optional[str] = None) -> Payment: ...

    def create_refund(self, payment_id: str, amount: float, reason: Optional[str] = None) -> Refund: ...

    def count(self) -> int: ...

# ============================================================================
# Shipments
# ============================================================================

class Shipment:
    """Shipment data returned from operations."""

    id: str
    shipment_number: str
    order_id: str
    status: str
    carrier: str
    shipping_method: str
    tracking_number: Optional[str]
    tracking_url: Optional[str]
    recipient_name: str
    shipping_address: str
    created_at: str
    updated_at: str

class Shipments:
    """Shipment management operations."""

    def create(
        self,
        order_id: str,
        recipient_name: str,
        shipping_address: str,
        carrier: Optional[str] = None,
        shipping_method: Optional[str] = None,
        tracking_number: Optional[str] = None,
    ) -> Shipment: ...

    def get(self, id: str) -> Optional[Shipment]: ...

    def list(self) -> List[Shipment]: ...

    def ship(self, id: str, tracking_number: Optional[str] = None) -> Shipment: ...

    def mark_delivered(self, id: str) -> Shipment: ...

    def cancel(self, id: str) -> Shipment: ...

    def count(self) -> int: ...

# ============================================================================
# Warranties
# ============================================================================

class Warranty:
    """Warranty data returned from operations."""

    id: str
    warranty_number: str
    customer_id: str
    product_id: Optional[str]
    order_id: Optional[str]
    status: str
    warranty_type: str
    start_date: str
    end_date: str

class WarrantyClaim:
    """Warranty claim data."""

    id: str
    claim_number: str
    warranty_id: str
    status: str
    issue_description: str
    resolution: Optional[str]
    created_at: str

class Warranties:
    """Warranty management operations."""

    def create(
        self,
        customer_id: str,
        product_id: Optional[str] = None,
        order_id: Optional[str] = None,
        warranty_type: Optional[str] = None,
        duration_months: Optional[int] = None,
        serial_number: Optional[str] = None,
    ) -> Warranty: ...

    def get(self, id: str) -> Optional[Warranty]: ...

    def list(self) -> List[Warranty]: ...

    def create_claim(self, warranty_id: str, issue_description: str) -> WarrantyClaim: ...

    def approve_claim(self, id: str) -> WarrantyClaim: ...

    def deny_claim(self, id: str, reason: str) -> WarrantyClaim: ...

    def complete_claim(self, id: str, resolution: str) -> WarrantyClaim: ...

    def count(self) -> int: ...

# ============================================================================
# Purchase Orders
# ============================================================================

class Supplier:
    """Supplier data."""

    id: str
    supplier_code: str
    name: str
    email: Optional[str]
    phone: Optional[str]
    status: str

class PurchaseOrder:
    """Purchase order data."""

    id: str
    po_number: str
    supplier_id: str
    status: str
    total_amount: float
    currency: str
    created_at: str
    updated_at: str

class PurchaseOrders:
    """Purchase order management operations."""

    def create_supplier(self, name: str, email: Optional[str] = None, phone: Optional[str] = None) -> Supplier: ...

    def get_supplier(self, id: str) -> Optional[Supplier]: ...

    def list_suppliers(self) -> List[Supplier]: ...

    def create(self, supplier_id: str) -> PurchaseOrder: ...

    def get(self, id: str) -> Optional[PurchaseOrder]: ...

    def list(self) -> List[PurchaseOrder]: ...

    def submit(self, id: str) -> PurchaseOrder: ...

    def approve(self, id: str, approved_by: str) -> PurchaseOrder: ...

    def send(self, id: str) -> PurchaseOrder: ...

    def cancel(self, id: str) -> PurchaseOrder: ...

    def count(self) -> int: ...

# ============================================================================
# Invoices
# ============================================================================

class Invoice:
    """Invoice data returned from operations."""

    id: str
    invoice_number: str
    status: str
    total_amount: float
    balance_due: float
    currency: str
    created_at: str
    updated_at: str

class Invoices:
    """Invoice management operations."""

    def create(self, customer_id: str, invoice_type: Optional[str] = None) -> Invoice: ...

    def get(self, id: str) -> Optional[Invoice]: ...

    def list(self) -> List[Invoice]: ...

    def send(self, id: str) -> Invoice: ...

    def void(self, id: str) -> Invoice: ...

    def record_payment(self, id: str, amount: float, payment_method: str) -> Invoice: ...

    def get_overdue(self) -> List[Invoice]: ...

    def count(self) -> int: ...

# ============================================================================
# Bill of Materials
# ============================================================================

class Bom:
    """Bill of materials data."""

    id: str
    bom_number: str
    product_id: str
    status: str

class BomComponent:
    """BOM component data."""

    id: str
    bom_id: str
    component_sku: str
    quantity: float

class BomApi:
    """Bill of materials operations."""

    def create(self, product_id: str) -> Bom: ...

    def get(self, id: str) -> Optional[Bom]: ...

    def list(self) -> List[Bom]: ...

    def add_component(self, bom_id: str, component_sku: str, quantity: float) -> BomComponent: ...

    def get_components(self, bom_id: str) -> List[BomComponent]: ...

    def activate(self, id: str) -> Bom: ...

    def count(self) -> int: ...

# ============================================================================
# Work Orders
# ============================================================================

class WorkOrder:
    """Work order data."""

    id: str
    work_order_number: str
    status: str
    quantity_planned: float
    quantity_completed: float

class WorkOrders:
    """Work order operations."""

    def create(self, bom_id: str, quantity_planned: float) -> WorkOrder: ...

    def get(self, id: str) -> Optional[WorkOrder]: ...

    def list(self) -> List[WorkOrder]: ...

    def start(self, id: str) -> WorkOrder: ...

    def complete(self, id: str, quantity_completed: float) -> WorkOrder: ...

    def cancel(self, id: str) -> WorkOrder: ...

    def count(self) -> int: ...

# ============================================================================
# Carts
# ============================================================================

class CartAddress:
    """Cart address."""

    first_name: str
    last_name: str
    company: Optional[str]
    line1: str
    line2: Optional[str]
    city: str
    state: Optional[str]
    postal_code: str
    country: str
    phone: Optional[str]
    email: Optional[str]

class CartItem:
    """Cart item."""

    id: str
    sku: str
    name: str
    quantity: int
    unit_price: float
    total: float

class Cart:
    """Cart data."""

    id: str
    cart_number: str
    status: str
    currency: str
    grand_total: float
    item_count: int

    @property
    def shipping_address(self) -> Optional[CartAddress]: ...

    @property
    def billing_address(self) -> Optional[CartAddress]: ...

    @property
    def items(self) -> List[CartItem]: ...

class AddCartItemInput:
    """Input for adding cart item."""

    def __init__(
        self,
        sku: str,
        name: str,
        quantity: int,
        unit_price: float,
        product_id: Optional[str] = None,
        variant_id: Optional[str] = None,
        description: Optional[str] = None,
        image_url: Optional[str] = None,
        original_price: Optional[float] = None,
        weight: Optional[float] = None,
        requires_shipping: Optional[bool] = None,
    ) -> None: ...

class ShippingRate:
    """Available shipping option."""

    id: str
    carrier: str
    service: str
    description: Optional[str]
    price: float
    currency: str
    estimated_days: Optional[int]

class CheckoutResult:
    """Checkout completion result."""

    cart_id: str
    order_id: str
    order_number: str
    payment_id: Optional[str]
    total_charged: float
    currency: str

class Carts:
    """Cart and checkout operations."""

    def create(
        self,
        customer_id: Optional[str] = None,
        customer_email: Optional[str] = None,
        customer_name: Optional[str] = None,
        currency: Optional[str] = None,
        expires_in_minutes: Optional[int] = None,
    ) -> Cart: ...

    def get(self, id: str) -> Optional[Cart]: ...

    def get_by_number(self, cart_number: str) -> Optional[Cart]: ...

    def update(
        self,
        id: str,
        customer_email: Optional[str] = None,
        customer_phone: Optional[str] = None,
        customer_name: Optional[str] = None,
        shipping_method: Optional[str] = None,
        coupon_code: Optional[str] = None,
        notes: Optional[str] = None,
    ) -> Cart: ...

    def list(self) -> List[Cart]: ...

    def for_customer(self, customer_id: str) -> List[Cart]: ...

    def delete(self, id: str) -> None: ...

    def add_item(self, cart_id: str, item: AddCartItemInput) -> CartItem: ...

    def update_item(self, item_id: str, quantity: Optional[int] = None) -> CartItem: ...

    def remove_item(self, item_id: str) -> None: ...

    def get_items(self, cart_id: str) -> List[CartItem]: ...

    def clear_items(self, cart_id: str) -> None: ...

    def set_shipping_address(self, id: str, address: CartAddress) -> Cart: ...

    def set_billing_address(self, id: str, address: CartAddress) -> Cart: ...

    def set_shipping(
        self,
        id: str,
        address: CartAddress,
        shipping_method: Optional[str] = None,
        shipping_carrier: Optional[str] = None,
        shipping_amount: Optional[float] = None,
    ) -> Cart: ...

    def get_shipping_rates(self, id: str) -> List[ShippingRate]: ...

    def set_payment(self, id: str, payment_method: str, payment_token: Optional[str] = None) -> Cart: ...

    def apply_discount(self, id: str, coupon_code: str) -> Cart: ...

    def remove_discount(self, id: str) -> Cart: ...

    def mark_ready_for_payment(self, id: str) -> Cart: ...

    def begin_checkout(self, id: str) -> Cart: ...

    def complete(self, id: str) -> CheckoutResult: ...

    def cancel(self, id: str) -> Cart: ...

    def abandon(self, id: str) -> Cart: ...

    def expire(self, id: str) -> Cart: ...

    def reserve_inventory(self, id: str) -> Cart: ...

    def release_inventory(self, id: str) -> Cart: ...

    def recalculate(self, id: str) -> Cart: ...

    def set_tax(self, id: str, tax_amount: float) -> Cart: ...

    def get_abandoned(self) -> List[Cart]: ...

    def get_expired(self) -> List[Cart]: ...

    def count(self) -> int: ...

# ============================================================================
# Analytics
# ============================================================================

class SalesSummary:
    total_revenue: float
    order_count: int
    average_order_value: float
    items_sold: int
    unique_customers: int

class RevenueByPeriod:
    period: str
    revenue: float
    order_count: int
    period_start: str

class TopProduct:
    product_id: Optional[str]
    sku: str
    name: str
    units_sold: int
    revenue: float
    order_count: int

class ProductPerformance:
    product_id: str
    sku: str
    name: str
    units_sold: int
    revenue: float
    previous_units_sold: int
    previous_revenue: float
    units_growth_percent: float
    revenue_growth_percent: float

class CustomerMetrics:
    total_customers: int
    new_customers: int
    returning_customers: int
    average_lifetime_value: float
    average_orders_per_customer: float

class TopCustomer:
    customer_id: str
    name: str
    email: str
    order_count: int
    total_spent: float
    average_order_value: float

class InventoryHealth:
    total_skus: int
    in_stock_skus: int
    low_stock_skus: int
    out_of_stock_skus: int
    total_value: float

class LowStockItem:
    sku: str
    name: str
    on_hand: float
    allocated: float
    available: float
    reorder_point: Optional[float]
    average_daily_sales: Optional[float]
    days_of_stock: Optional[float]

class InventoryMovement:
    sku: str
    name: str
    units_sold: int
    units_received: int
    units_returned: int
    units_adjusted: int
    net_change: int

class OrderStatusBreakdown:
    pending: int
    confirmed: int
    processing: int
    shipped: int
    delivered: int
    cancelled: int
    refunded: int

class FulfillmentMetrics:
    avg_time_to_ship_hours: Optional[float]
    avg_time_to_deliver_hours: Optional[float]
    on_time_shipping_percent: Optional[float]
    on_time_delivery_percent: Optional[float]
    shipped_today: int
    awaiting_shipment: int

class ReturnMetrics:
    total_returns: int
    return_rate_percent: float
    total_refunded: float

class DemandForecast:
    sku: str
    name: str
    average_daily_demand: float
    forecasted_demand: float
    confidence: float
    current_stock: float
    days_until_stockout: Optional[int]
    recommended_reorder_qty: Optional[float]
    trend: str

class RevenueForecast:
    period: str
    forecasted_revenue: float
    lower_bound: float
    upper_bound: float
    confidence_level: float
    based_on_periods: int

class Analytics:
    def sales_summary(self, period: Optional[str] = None, limit: Optional[int] = None) -> SalesSummary: ...

    def revenue_by_period(self, period: Optional[str] = None, granularity: Optional[str] = None) -> List[RevenueByPeriod]: ...

    def top_products(self, period: Optional[str] = None, limit: Optional[int] = None) -> List[TopProduct]: ...

    def product_performance(self, period: Optional[str] = None, limit: Optional[int] = None) -> List[ProductPerformance]: ...

    def customer_metrics(self, period: Optional[str] = None) -> CustomerMetrics: ...

    def top_customers(self, period: Optional[str] = None, limit: Optional[int] = None) -> List[TopCustomer]: ...

    def inventory_health(self) -> InventoryHealth: ...

    def low_stock_items(self, threshold: Optional[float] = None) -> List[LowStockItem]: ...

    def inventory_movement(self, period: Optional[str] = None) -> List[InventoryMovement]: ...

    def order_status_breakdown(self, period: Optional[str] = None) -> OrderStatusBreakdown: ...

    def fulfillment_metrics(self, period: Optional[str] = None) -> FulfillmentMetrics: ...

    def return_metrics(self, period: Optional[str] = None) -> ReturnMetrics: ...

    def demand_forecast(self, skus: Optional[List[str]] = None, days_ahead: Optional[int] = None) -> List[DemandForecast]: ...

    def revenue_forecast(self, periods_ahead: Optional[int] = None, granularity: Optional[str] = None) -> List[RevenueForecast]: ...

# ============================================================================
# Currency
# ============================================================================

class ExchangeRate:
    id: str
    base_currency: str
    quote_currency: str
    rate: float
    source: str
    rate_at: str
    created_at: str
    updated_at: str

class ConversionResult:
    original_amount: float
    original_currency: str
    converted_amount: float
    target_currency: str
    rate: float
    inverse_rate: float
    rate_at: str

class StoreCurrencySettings:
    base_currency: str
    enabled_currencies: List[str]
    auto_convert: bool
    rounding_mode: str

class SetExchangeRateInput:
    base_currency: str
    quote_currency: str
    rate: float
    source: Optional[str]

    def __init__(self, base_currency: str, quote_currency: str, rate: float, source: Optional[str] = None) -> None: ...

class CurrencyOperations:
    def get_rate(self, from_currency: str, to_currency: str) -> Optional[ExchangeRate]: ...

    def get_rates_for(self, base_currency: str) -> List[ExchangeRate]: ...

    def list_rates(self, base_currency: Optional[str] = None, quote_currency: Optional[str] = None) -> List[ExchangeRate]: ...

    def set_rate(self, base_currency: str, quote_currency: str, rate: float, source: Optional[str] = None) -> ExchangeRate: ...

    def set_rates(self, rates: List[SetExchangeRateInput]) -> List[ExchangeRate]: ...

    def delete_rate(self, id: str) -> None: ...

    def convert(self, from_currency: str, to_currency: str, amount: float) -> ConversionResult: ...

    def get_settings(self) -> StoreCurrencySettings: ...

    def update_settings(
        self,
        base_currency: str,
        enabled_currencies: List[str],
        auto_convert: Optional[bool] = None,
        rounding_mode: Optional[str] = None,
    ) -> StoreCurrencySettings: ...

    def set_base_currency(self, currency_code: str) -> StoreCurrencySettings: ...

    def enable_currencies(self, currency_codes: List[str]) -> StoreCurrencySettings: ...

    def is_enabled(self, currency_code: str) -> bool: ...

    def base_currency(self) -> str: ...

    def enabled_currencies(self) -> List[str]: ...

    def format(self, amount: float, currency_code: str) -> str: ...

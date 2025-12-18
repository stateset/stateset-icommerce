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

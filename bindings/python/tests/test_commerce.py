"""Tests for stateset_embedded Python bindings."""

import pytest
from stateset_embedded import (
    Commerce,
    CreateOrderItemInput,
    CreateProductVariantInput,
    CreateReturnItemInput,
)


@pytest.fixture
def commerce():
    """Create an in-memory Commerce instance for testing."""
    return Commerce(":memory:")


class TestCustomers:
    """Customer API tests."""

    def test_create_customer(self, commerce):
        """Test creating a customer."""
        customer = commerce.customers.create(
            email="alice@example.com",
            first_name="Alice",
            last_name="Smith",
        )

        assert customer.id is not None
        assert customer.email == "alice@example.com"
        assert customer.first_name == "Alice"
        assert customer.last_name == "Smith"
        assert customer.status == "active"

    def test_create_customer_with_optional_fields(self, commerce):
        """Test creating a customer with optional fields."""
        customer = commerce.customers.create(
            email="bob@example.com",
            first_name="Bob",
            last_name="Jones",
            phone="+1234567890",
            accepts_marketing=True,
        )

        assert customer.phone == "+1234567890"
        assert customer.accepts_marketing is True

    def test_get_customer_by_id(self, commerce):
        """Test getting a customer by ID."""
        created = commerce.customers.create(
            email="test@example.com",
            first_name="Test",
            last_name="User",
        )

        found = commerce.customers.get(created.id)
        assert found is not None
        assert found.id == created.id
        assert found.email == "test@example.com"

    def test_get_customer_by_email(self, commerce):
        """Test getting a customer by email."""
        created = commerce.customers.create(
            email="email@example.com",
            first_name="Email",
            last_name="User",
        )

        found = commerce.customers.get_by_email("email@example.com")
        assert found is not None
        assert found.id == created.id

    def test_get_nonexistent_customer(self, commerce):
        """Test getting a customer that doesn't exist."""
        result = commerce.customers.get("00000000-0000-0000-0000-000000000000")
        assert result is None

    def test_list_customers(self, commerce):
        """Test listing customers."""
        commerce.customers.create(
            email="list1@example.com",
            first_name="List",
            last_name="One",
        )
        commerce.customers.create(
            email="list2@example.com",
            first_name="List",
            last_name="Two",
        )

        customers = commerce.customers.list()
        assert len(customers) >= 2

    def test_count_customers(self, commerce):
        """Test counting customers."""
        initial = commerce.customers.count()

        commerce.customers.create(
            email="count@example.com",
            first_name="Count",
            last_name="Test",
        )

        assert commerce.customers.count() == initial + 1

    def test_customer_full_name(self, commerce):
        """Test customer full_name property."""
        customer = commerce.customers.create(
            email="fullname@example.com",
            first_name="Full",
            last_name="Name",
        )

        assert customer.full_name == "Full Name"


class TestOrders:
    """Order API tests."""

    def test_create_order(self, commerce):
        """Test creating an order."""
        customer = commerce.customers.create(
            email="order@example.com",
            first_name="Order",
            last_name="Customer",
        )

        order = commerce.orders.create(
            customer_id=customer.id,
            items=[
                CreateOrderItemInput(
                    sku="SKU-001",
                    name="Test Product",
                    quantity=2,
                    unit_price=29.99,
                )
            ],
        )

        assert order.id is not None
        assert order.order_number is not None
        assert order.customer_id == customer.id
        assert order.status == "pending"
        assert len(order.items) == 1
        assert order.items[0].quantity == 2

    def test_order_total_calculation(self, commerce):
        """Test that order total is calculated correctly."""
        customer = commerce.customers.create(
            email="total@example.com",
            first_name="Total",
            last_name="Test",
        )

        order = commerce.orders.create(
            customer_id=customer.id,
            items=[
                CreateOrderItemInput(
                    sku="SKU-A",
                    name="Product A",
                    quantity=2,
                    unit_price=10.00,
                ),
                CreateOrderItemInput(
                    sku="SKU-B",
                    name="Product B",
                    quantity=1,
                    unit_price=15.00,
                ),
            ],
        )

        # 2 * 10 + 1 * 15 = 35
        assert order.total_amount == 35.00

    def test_ship_order(self, commerce):
        """Test shipping an order."""
        customer = commerce.customers.create(
            email="ship@example.com",
            first_name="Ship",
            last_name="Test",
        )

        order = commerce.orders.create(
            customer_id=customer.id,
            items=[
                CreateOrderItemInput(
                    sku="SKU-001",
                    name="Test",
                    quantity=1,
                    unit_price=10.00,
                )
            ],
        )

        shipped = commerce.orders.ship(order.id, tracking_number="1Z123ABC")
        assert shipped.status == "shipped"
        assert shipped.tracking_number == "1Z123ABC"

    def test_cancel_order(self, commerce):
        """Test cancelling an order."""
        customer = commerce.customers.create(
            email="cancel@example.com",
            first_name="Cancel",
            last_name="Test",
        )

        order = commerce.orders.create(
            customer_id=customer.id,
            items=[
                CreateOrderItemInput(
                    sku="SKU-001",
                    name="Test",
                    quantity=1,
                    unit_price=10.00,
                )
            ],
        )

        cancelled = commerce.orders.cancel(order.id)
        assert cancelled.status == "cancelled"

    def test_update_order_status(self, commerce):
        """Test updating order status."""
        customer = commerce.customers.create(
            email="status@example.com",
            first_name="Status",
            last_name="Test",
        )

        order = commerce.orders.create(
            customer_id=customer.id,
            items=[
                CreateOrderItemInput(
                    sku="SKU-001",
                    name="Test",
                    quantity=1,
                    unit_price=10.00,
                )
            ],
        )

        updated = commerce.orders.update_status(order.id, "processing")
        assert updated.status == "processing"


class TestProducts:
    """Product API tests."""

    def test_create_product(self, commerce):
        """Test creating a product."""
        product = commerce.products.create(
            name="Test Product",
            description="A test product",
        )

        assert product.id is not None
        assert product.name == "Test Product"
        assert product.description == "A test product"
        assert product.status == "draft"  # Products start in draft status

    def test_create_product_with_variants(self, commerce):
        """Test creating a product with variants."""
        product = commerce.products.create(
            name="Widget",
            variants=[
                CreateProductVariantInput(
                    sku="WIDGET-SM",
                    price=19.99,
                    name="Small",
                ),
                CreateProductVariantInput(
                    sku="WIDGET-LG",
                    price=29.99,
                    name="Large",
                ),
            ],
        )

        assert product.id is not None

        # Get variants by SKU
        small = commerce.products.get_variant_by_sku("WIDGET-SM")
        large = commerce.products.get_variant_by_sku("WIDGET-LG")

        assert small is not None
        assert small.price == 19.99

        assert large is not None
        assert large.price == 29.99


class TestInventory:
    """Inventory API tests."""

    def test_create_inventory_item(self, commerce):
        """Test creating an inventory item."""
        item = commerce.inventory.create_item(
            sku="INV-001",
            name="Inventory Item",
            initial_quantity=100,
        )

        assert item.id is not None
        assert item.sku == "INV-001"
        assert item.name == "Inventory Item"

    def test_get_stock(self, commerce):
        """Test getting stock levels."""
        commerce.inventory.create_item(
            sku="STOCK-001",
            name="Stock Item",
            initial_quantity=50,
        )

        stock = commerce.inventory.get_stock("STOCK-001")

        assert stock is not None
        assert stock.sku == "STOCK-001"
        assert stock.total_on_hand == 50
        assert stock.total_available == 50
        assert stock.total_allocated == 0

    def test_adjust_inventory(self, commerce):
        """Test adjusting inventory."""
        commerce.inventory.create_item(
            sku="ADJ-001",
            name="Adjust Item",
            initial_quantity=100,
        )

        # Remove some stock
        commerce.inventory.adjust("ADJ-001", -10, "Sold 10 units")

        stock = commerce.inventory.get_stock("ADJ-001")
        assert stock.total_on_hand == 90

        # Add more stock
        commerce.inventory.adjust("ADJ-001", 25, "Received shipment")

        stock = commerce.inventory.get_stock("ADJ-001")
        assert stock.total_on_hand == 115

    def test_reserve_inventory(self, commerce):
        """Test reserving inventory."""
        commerce.inventory.create_item(
            sku="RES-001",
            name="Reserve Item",
            initial_quantity=100,
        )

        reservation = commerce.inventory.reserve(
            sku="RES-001",
            quantity=10,
            reference_type="order",
            reference_id="test-order-123",
        )

        assert reservation.id is not None
        assert reservation.quantity == 10
        assert reservation.status == "pending"

        stock = commerce.inventory.get_stock("RES-001")
        assert stock.total_allocated == 10
        assert stock.total_available == 90

    def test_confirm_reservation(self, commerce):
        """Test confirming a reservation."""
        commerce.inventory.create_item(
            sku="CONF-001",
            name="Confirm Item",
            initial_quantity=100,
        )

        reservation = commerce.inventory.reserve(
            sku="CONF-001",
            quantity=10,
            reference_type="order",
            reference_id="test-order-456",
        )

        # Confirm just updates the reservation status - allocation remains
        # until the reservation is processed/released
        commerce.inventory.confirm_reservation(reservation.id)

        stock = commerce.inventory.get_stock("CONF-001")
        assert stock.total_on_hand == 100
        assert stock.total_allocated == 10  # Still allocated
        assert stock.total_available == 90  # Still reduced

    def test_release_reservation(self, commerce):
        """Test releasing a reservation."""
        commerce.inventory.create_item(
            sku="REL-001",
            name="Release Item",
            initial_quantity=100,
        )

        reservation = commerce.inventory.reserve(
            sku="REL-001",
            quantity=10,
            reference_type="order",
            reference_id="test-order-789",
        )

        commerce.inventory.release_reservation(reservation.id)

        stock = commerce.inventory.get_stock("REL-001")
        assert stock.total_on_hand == 100  # Not deducted
        assert stock.total_allocated == 0  # Released
        assert stock.total_available == 100  # Back to available


class TestReturns:
    """Return API tests."""

    def test_create_return(self, commerce):
        """Test creating a return."""
        # Create order first
        customer = commerce.customers.create(
            email="return@example.com",
            first_name="Return",
            last_name="Test",
        )

        order = commerce.orders.create(
            customer_id=customer.id,
            items=[
                CreateOrderItemInput(
                    sku="RET-001",
                    name="Return Item",
                    quantity=2,
                    unit_price=25.00,
                )
            ],
        )

        # Create return
        ret = commerce.returns.create(
            order_id=order.id,
            reason="defective",
            items=[
                CreateReturnItemInput(
                    order_item_id=order.items[0].id,
                    quantity=1,
                )
            ],
            reason_details="Product stopped working",
        )

        assert ret.id is not None
        assert ret.order_id == order.id
        assert ret.status == "requested"
        assert ret.reason == "defective"

    def test_approve_return(self, commerce):
        """Test approving a return."""
        customer = commerce.customers.create(
            email="approve@example.com",
            first_name="Approve",
            last_name="Test",
        )

        order = commerce.orders.create(
            customer_id=customer.id,
            items=[
                CreateOrderItemInput(
                    sku="APP-001",
                    name="Approve Item",
                    quantity=1,
                    unit_price=50.00,
                )
            ],
        )

        ret = commerce.returns.create(
            order_id=order.id,
            reason="damaged",
            items=[
                CreateReturnItemInput(
                    order_item_id=order.items[0].id,
                    quantity=1,
                )
            ],
        )

        approved = commerce.returns.approve(ret.id)
        assert approved.status == "approved"

    def test_reject_return(self, commerce):
        """Test rejecting a return."""
        customer = commerce.customers.create(
            email="reject@example.com",
            first_name="Reject",
            last_name="Test",
        )

        order = commerce.orders.create(
            customer_id=customer.id,
            items=[
                CreateOrderItemInput(
                    sku="REJ-001",
                    name="Reject Item",
                    quantity=1,
                    unit_price=30.00,
                )
            ],
        )

        ret = commerce.returns.create(
            order_id=order.id,
            reason="changed_mind",
            items=[
                CreateReturnItemInput(
                    order_item_id=order.items[0].id,
                    quantity=1,
                )
            ],
        )

        rejected = commerce.returns.reject(ret.id, "Item was used")
        assert rejected.status == "rejected"

    def test_list_returns(self, commerce):
        """Test listing returns."""
        customer = commerce.customers.create(
            email="list-returns@example.com",
            first_name="List",
            last_name="Returns",
        )

        order = commerce.orders.create(
            customer_id=customer.id,
            items=[
                CreateOrderItemInput(
                    sku="LIST-001",
                    name="List Item",
                    quantity=1,
                    unit_price=20.00,
                )
            ],
        )

        commerce.returns.create(
            order_id=order.id,
            reason="defective",
            items=[
                CreateReturnItemInput(
                    order_item_id=order.items[0].id,
                    quantity=1,
                )
            ],
        )

        returns = commerce.returns.list()
        assert len(returns) >= 1


class TestRepr:
    """Test __repr__ methods."""

    def test_customer_repr(self, commerce):
        """Test Customer.__repr__."""
        customer = commerce.customers.create(
            email="repr@example.com",
            first_name="Repr",
            last_name="Test",
        )

        repr_str = repr(customer)
        assert "Customer" in repr_str
        assert "repr@example.com" in repr_str
        assert "Repr Test" in repr_str

    def test_order_repr(self, commerce):
        """Test Order.__repr__."""
        customer = commerce.customers.create(
            email="orderrepr@example.com",
            first_name="Order",
            last_name="Repr",
        )

        order = commerce.orders.create(
            customer_id=customer.id,
            items=[
                CreateOrderItemInput(
                    sku="REPR-001",
                    name="Test",
                    quantity=1,
                    unit_price=10.00,
                )
            ],
        )

        repr_str = repr(order)
        assert "Order" in repr_str
        assert order.order_number in repr_str

    def test_product_repr(self, commerce):
        """Test Product.__repr__."""
        product = commerce.products.create(
            name="Repr Product",
        )

        repr_str = repr(product)
        assert "Product" in repr_str
        assert "Repr Product" in repr_str

    def test_stock_level_repr(self, commerce):
        """Test StockLevel.__repr__."""
        commerce.inventory.create_item(
            sku="REPR-INV",
            name="Repr Inventory",
            initial_quantity=50,
        )

        stock = commerce.inventory.get_stock("REPR-INV")
        repr_str = repr(stock)
        assert "StockLevel" in repr_str
        assert "REPR-INV" in repr_str

    def test_return_repr(self, commerce):
        """Test Return.__repr__."""
        customer = commerce.customers.create(
            email="returnrepr@example.com",
            first_name="Return",
            last_name="Repr",
        )

        order = commerce.orders.create(
            customer_id=customer.id,
            items=[
                CreateOrderItemInput(
                    sku="RR-001",
                    name="Test",
                    quantity=1,
                    unit_price=10.00,
                )
            ],
        )

        ret = commerce.returns.create(
            order_id=order.id,
            reason="defective",
            items=[
                CreateReturnItemInput(
                    order_item_id=order.items[0].id,
                    quantity=1,
                )
            ],
        )

        repr_str = repr(ret)
        assert "Return" in repr_str
        assert "defective" in repr_str


class TestAnalyticsAndCurrency:
    def test_sales_summary_empty(self, commerce):
        summary = commerce.analytics.sales_summary(period="last30days")
        assert summary.order_count == 0
        assert summary.total_revenue == 0.0

    def test_currency_set_rate_and_convert(self, commerce):
        commerce.currency.set_rate("USD", "EUR", 0.9, source="test")
        result = commerce.currency.convert("USD", "EUR", 100.0)
        assert abs(result.converted_amount - 90.0) < 1e-6


class TestCartsExtended:
    def test_update_and_set_shipping(self, commerce):
        cart = commerce.carts.create(customer_email="cart@example.com")

        updated = commerce.carts.update(
            cart.id,
            customer_email="updated@example.com",
            shipping_method="standard",
            notes="test",
        )
        assert updated.customer_email == "updated@example.com"
        assert updated.shipping_method == "standard"
        assert updated.notes == "test"

        from stateset_embedded import CartAddress

        address = CartAddress(
            first_name="Alice",
            last_name="Smith",
            line1="123 Main St",
            city="San Francisco",
            postal_code="94105",
            country="US",
        )

        shipped = commerce.carts.set_shipping(
            updated.id,
            address,
            shipping_method="standard",
            shipping_carrier="ups",
            shipping_amount=9.99,
        )
        assert shipped.shipping_method == "standard"
        assert shipped.shipping_address is not None
        assert shipped.shipping_address.city == "San Francisco"
        assert abs(shipped.shipping_amount - 9.99) < 1e-6

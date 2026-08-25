package com.stateset.embedded;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class CommerceTests {
    private Commerce commerce;

    @BeforeEach
    void setUp() {
        commerce = new Commerce(":memory:");
    }

    @AfterEach
    void tearDown() {
        commerce.close();
    }

    @Test
    void customersCrud() {
        Customer customer = commerce.customers().create(
            "test@example.com",
            "Test",
            "User",
            null
        );

        assertNotNull(customer);
        assertFalse(customer.getId().isEmpty());
        assertEquals("test@example.com", customer.getEmail());

        assertTrue(commerce.customers().get(customer.getId()).isPresent());
        assertTrue(commerce.customers().getByEmail("test@example.com").isPresent());
        assertTrue(commerce.customers().list().size() >= 1);
        assertTrue(commerce.customers().count() >= 1);
    }

    @Test
    void productsCreateAndList() {
        Product product = commerce.products().create(
            "Test Product",
            "A test product",
            "StateSet",
            "Widgets"
        );

        assertNotNull(product);
        assertFalse(product.getId().isEmpty());
        assertEquals("Test Product", product.getName());

        assertTrue(commerce.products().list().size() >= 1);
        assertTrue(commerce.products().count() >= 1);
    }

    @Test
    void inventoryAdjustReserveRelease() {
        InventoryItem item = commerce.inventory().create("SKU-001", 10, 0, 0);
        assertNotNull(item);
        assertEquals("SKU-001", item.getSku());
        assertEquals(10, item.getQuantityAvailable());

        InventoryItem adjusted = commerce.inventory().adjust(item.getId(), 5, "restock");
        assertEquals(15, adjusted.getQuantityOnHand());
        assertEquals(15, adjusted.getQuantityAvailable());

        InventoryItem reserved = commerce.inventory().reserve(item.getId(), 3, "ORDER-1");
        assertEquals(3, reserved.getQuantityReserved());
        assertEquals(12, reserved.getQuantityAvailable());

        InventoryItem released = commerce.inventory().release(item.getId(), 2);
        assertEquals(1, released.getQuantityReserved());
        assertEquals(14, released.getQuantityAvailable());
    }

    @Test
    void ordersCreateGetListCount() {
        Customer customer = commerce.customers().create(
            "order@example.com",
            "Order",
            "User",
            null
        );

        String itemsJson = "[{\"sku\":\"SKU-001\",\"name\":\"Widget\",\"quantity\":2,\"unit_price\":29.99}]";
        Order order = commerce.orders().create(customer.getId(), itemsJson, "USD");

        assertNotNull(order);
        assertFalse(order.getId().isEmpty());
        assertEquals(customer.getId(), order.getCustomerId());

        assertTrue(commerce.orders().get(order.getId()).isPresent());
        assertTrue(commerce.orders().list().size() >= 1);
        assertTrue(commerce.orders().count() >= 1);
    }

    @Test
    void cartsCheckoutFlow() {
        Cart cart = commerce.carts().create(null, "USD");
        assertNotNull(cart);
        assertFalse(cart.getId().isEmpty());

        Cart updated = commerce.carts().addItem(
            cart.getId(),
            "CART-001",
            "Cart Item",
            2,
            12.50
        );
        assertNotNull(updated);

        Order order = commerce.carts().checkout(cart.getId());
        assertNotNull(order);
        assertFalse(order.getId().isEmpty());
    }

    @Test
    void paymentsRecord() {
        Customer customer = commerce.customers().create(
            "pay@example.com",
            "Pay",
            "User",
            null
        );

        String itemsJson = "[{\"sku\":\"PAY-001\",\"name\":\"Payment Item\",\"quantity\":1,\"unit_price\":49.99}]";
        Order order = commerce.orders().create(customer.getId(), itemsJson, "USD");

        boolean recorded = commerce.payments().record(order.getId(), 49.99, "credit_card");
        assertTrue(recorded);
    }

    @Test
    void returnsLifecycle() {
        Customer customer = commerce.customers().create(
            "returns@example.com",
            "Return",
            "User",
            null
        );

        String itemsJson = "[{\"sku\":\"RET-001\",\"name\":\"Return Item\",\"quantity\":1,\"unit_price\":19.99}]";
        Order order = commerce.orders().create(customer.getId(), itemsJson, "USD");
        // A return requires a shipped order (the engine enforces this).
        commerce.orders().ship(order.getId(), "TRACK-RET-0001", null);

        ReturnRequest toApprove = commerce.returns().create(order.getId(), "Damaged");
        assertNotNull(toApprove);
        assertFalse(toApprove.getId().isEmpty());
        assertEquals(order.getId(), toApprove.getOrderId());

        ReturnRequest approved = commerce.returns().approve(toApprove.getId(), 19.99);
        assertNotNull(approved);

        String itemsJson2 = "[{\"sku\":\"RET-002\",\"name\":\"Return Item 2\",\"quantity\":1,\"unit_price\":9.99}]";
        Order order2 = commerce.orders().create(customer.getId(), itemsJson2, "USD");
        commerce.orders().ship(order2.getId(), "TRACK-RET-0002", null);
        ReturnRequest toReject = commerce.returns().create(order2.getId(), "Wrong size");
        ReturnRequest rejected = commerce.returns().reject(toReject.getId(), "Invalid");
        assertNotNull(rejected);

        assertTrue(commerce.returns().get(toApprove.getId()).isPresent());
        assertTrue(commerce.returns().list().size() >= 1);
    }

    @Test
    void analyticsSummary() {
        SalesSummary summary = commerce.analytics().salesSummary(0);
        assertNotNull(summary);
        assertTrue(summary.getTotalRevenue() >= 0.0);
        assertTrue(summary.getTotalOrders() >= 0);
    }
}

package com.stateset.embedded

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import kotlin.test.assertNull

class StateSetCommerceTest {

    @Test
    fun customersCrud() {
        StateSetCommerce(":memory:").use { commerce ->
            val customer = commerce.customers.create(
                email = "test@example.com",
                firstName = "Test",
                lastName = "User"
            )

            assertTrue(customer.id.isNotEmpty())
            assertEquals("test@example.com", customer.email)

            val fetched = commerce.customers.get(customer.id)
            assertNotNull(fetched)
            assertEquals(customer.id, fetched.id)

            val list = commerce.customers.list()
            assertTrue(list.isNotEmpty())

            val deleted = commerce.customers.delete(customer.id)
            assertTrue(deleted)

            val missing = commerce.customers.get(customer.id)
            assertNull(missing)
        }
    }

    @Test
    fun productsCreateAndList() {
        StateSetCommerce(":memory:").use { commerce ->
            val product = commerce.products.create(
                name = "Test Product",
                sku = "PROD-001",
                price = 19.99,
                description = "A test product"
            )

            assertTrue(product.id.isNotEmpty())
            assertEquals("Test Product", product.name)

            val fetched = commerce.products.get(product.id)
            assertNotNull(fetched)

            val list = commerce.products.list()
            assertTrue(list.isNotEmpty())
        }
    }

    @Test
    fun inventoryAdjustAndGetLevel() {
        StateSetCommerce(":memory:").use { commerce ->
            val item = commerce.inventory.createItem(
                sku = "INV-001",
                name = "Inventory Item",
                initialQuantity = 10.0
            )

            assertTrue(item.id.isNotEmpty())
            assertEquals("INV-001", item.sku)

            val adjusted = commerce.inventory.adjust(
                sku = "INV-001",
                quantityDelta = 5.0,
                reason = "restock"
            )
            assertTrue(adjusted)

            val level = commerce.inventory.getLevel("INV-001")
            assertNotNull(level)
            assertTrue(level.inventoryItemId.isNotEmpty())
        }
    }

    @Test
    fun ordersLifecycle() {
        StateSetCommerce(":memory:").use { commerce ->
            val customer = commerce.customers.create(
                email = "order@example.com",
                firstName = "Order",
                lastName = "User"
            )

            val order = commerce.orders.create(
                customerId = customer.id,
                items = listOf(
                    OrderItem(sku = "SKU-001", name = "Widget", quantity = 2, unitPrice = 29.99)
                ),
                currency = "USD"
            )

            assertTrue(order.id.isNotEmpty())
            assertEquals(customer.id, order.customerId)

            val updated = commerce.orders.updateStatus(order.id, OrderStatus.Confirmed)
            assertEquals("confirmed", updated.status)

            val shipped = commerce.orders.ship(order.id)
            assertEquals(order.id, shipped.id)

            val listed = commerce.orders.list()
            assertTrue(listed.isNotEmpty())
        }
    }

    @Test
    fun returnsFlow() {
        StateSetCommerce(":memory:").use { commerce ->
            val customer = commerce.customers.create(
                email = "returns@example.com",
                firstName = "Return",
                lastName = "User"
            )

            val order = commerce.orders.create(
                customerId = customer.id,
                items = listOf(
                    OrderItem(sku = "RET-001", name = "Return Item", quantity = 1, unitPrice = 9.99)
                ),
                currency = "USD"
            )

            // A return requires a shipped order (the engine enforces this).
            commerce.orders.ship(order.id)

            val ret = commerce.returns.create(order.id, ReturnReason.Defective)
            assertTrue(ret.id.isNotEmpty())
            assertEquals(order.id, ret.orderId)

            val approved = commerce.returns.approve(ret.id)
            assertEquals(ret.id, approved.id)

            val list = commerce.returns.list()
            assertTrue(list.isNotEmpty())
        }
    }

    @Test
    fun paymentsAndAnalytics() {
        StateSetCommerce(":memory:").use { commerce ->
            val customer = commerce.customers.create(
                email = "pay@example.com",
                firstName = "Pay",
                lastName = "User"
            )

            val order = commerce.orders.create(
                customerId = customer.id,
                items = listOf(
                    OrderItem(sku = "PAY-001", name = "Payment Item", quantity = 1, unitPrice = 49.99)
                ),
                currency = "USD"
            )

            val payment = commerce.payments.create(
                orderId = order.id,
                amount = 49.99,
                currency = "USD",
                method = PaymentMethod.CreditCard
            )
            assertTrue(payment.id.isNotEmpty())
            assertEquals(order.id, payment.orderId)

            val fetched = commerce.payments.get(payment.id)
            assertNotNull(fetched)

            val list = commerce.payments.list()
            assertTrue(list.isNotEmpty())

            val summary = commerce.analytics.salesSummary(TimePeriod.AllTime)
            assertNotNull(summary)
        }
    }

    @Test
    fun shipmentsCreateAndList() {
        StateSetCommerce(":memory:").use { commerce ->
            val customer = commerce.customers.create(
                email = "ship@example.com",
                firstName = "Ship",
                lastName = "User"
            )

            val order = commerce.orders.create(
                customerId = customer.id,
                items = listOf(
                    OrderItem(sku = "SHIP-001", name = "Ship Item", quantity = 1, unitPrice = 12.50)
                ),
                currency = "USD"
            )

            val shipment = commerce.shipments.create(
                orderId = order.id,
                recipientName = "Ship User",
                shippingAddress = "123 Main St, City, ST 12345",
                carrier = "ups"
            )

            assertTrue(shipment.id.isNotEmpty())
            assertEquals(order.id, shipment.orderId)

            val fetched = commerce.shipments.get(shipment.id)
            assertNotNull(fetched)

            val shipped = commerce.shipments.ship(shipment.id, "1Z999AA10123456784")
            assertEquals("shipped", shipped.status)

            val delivered = commerce.shipments.deliver(shipment.id)
            assertEquals("delivered", delivered.status)

            val secondOrder = commerce.orders.create(
                customerId = customer.id,
                items = listOf(
                    OrderItem(sku = "SHIP-002", name = "Ship Item 2", quantity = 1, unitPrice = 8.25)
                ),
                currency = "USD"
            )

            val toCancel = commerce.shipments.create(
                orderId = secondOrder.id,
                recipientName = "Ship User",
                shippingAddress = "456 Market St, City, ST 12345",
                carrier = "ups"
            )

            val cancelled = commerce.shipments.cancel(toCancel.id)
            assertEquals("cancelled", cancelled.status)

            val list = commerce.shipments.list()
            assertTrue(list.isNotEmpty())
        }
    }
}

import Foundation
import XCTest
@testable import StateSet

/// End-to-end tests against the REAL Rust engine through the C ABI. There is
/// no Swift-side store: if the native library is missing, nothing links.
final class StateSetTests: XCTestCase {
    var commerce: StateSetCommerce!

    override func setUpWithError() throws {
        commerce = try StateSetCommerce(dbPath: ":memory:")
    }

    override func tearDown() {
        commerce?.close()
        commerce = nil
    }

    private func dec(_ s: String) -> Decimal { Decimal(string: s, locale: Locale(identifier: "en_US_POSIX"))! }

    private func newCustomer(_ email: String = "alice@example.com") throws -> Customer {
        try commerce.customers.create(email: email, firstName: "Alice", lastName: "Smith")
    }

    private func line(_ c: Customer, _ sku: String, _ qty: Int32, _ price: String) -> CreateOrderItem {
        CreateOrderItem(productId: c.id, sku: sku, name: sku, quantity: qty, unitPrice: dec(price))
    }

    private func deliveredOrder(_ c: Customer, sku: String, qty: Int32 = 1) throws -> Order {
        let order = try commerce.orders.create(customerId: c.id, items: [line(c, sku, qty, "15.00")])
        _ = try commerce.orders.updateStatus(id: order.id, status: .confirmed)
        _ = try commerce.orders.ship(id: order.id)
        return try commerce.orders.deliver(id: order.id)
    }

    private func assertEngineError(_ code: StateSetError.Code? = nil, _ body: () throws -> Void,
                                   file: StaticString = #filePath, line: UInt = #line) {
        do {
            try body()
            XCTFail("expected the engine to refuse", file: file, line: line)
        } catch let e as StateSetError {
            if let code = code { XCTAssertEqual(e.code, code, e.message, file: file, line: line) }
        } catch {
            XCTFail("unexpected error \(error)", file: file, line: line)
        }
    }

    // MARK: - Proof that this is not an in-memory fake

    func testDataPersistsAcrossInstancesOnTheSameFile() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("stateset-swift-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let path = dir.appendingPathComponent("store.db").path

        var customerId = ""
        var orderId = ""
        do {
            let first = try StateSetCommerce(dbPath: path)
            let c = try first.customers.create(email: "persist@example.com", firstName: "Per", lastName: "Sist")
            let o = try first.orders.create(customerId: c.id, items: [
                CreateOrderItem(productId: c.id, sku: "P-1", name: "Persisted", quantity: 3, unitPrice: dec("0.10")),
            ])
            customerId = c.id
            orderId = o.id
            first.close()
        }

        let size = try FileManager.default.attributesOfItem(atPath: path)[.size] as? NSNumber
        XCTAssertGreaterThan(size?.intValue ?? 0, 0, "the SQLite file must exist and be non-empty")

        let second = try StateSetCommerce(dbPath: path)
        defer { second.close() }
        XCTAssertEqual(try second.customers.get(id: customerId)?.email, "persist@example.com")
        let order = try XCTUnwrap(second.orders.get(id: orderId))
        XCTAssertEqual(order.totalAmount, dec("0.30"))
        XCTAssertEqual(try second.customers.list().count, 1)
    }

    func testSeparateInMemoryStoresDoNotShareData() throws {
        _ = try newCustomer()
        let other = try StateSetCommerce(dbPath: ":memory:")
        defer { other.close() }
        XCTAssertEqual(try other.customers.count(), 0)
        XCTAssertEqual(try commerce.customers.count(), 1)
    }

    func testNativeSurfaceReportsVersionsAndMethods() throws {
        let v = try commerce.version()
        XCTAssertEqual(v["json_api_version"] as? Int, 1)
        XCTAssertEqual(v["abi_version"] as? Int, 1)
        let methods = try commerce.nativeMethodNames()
        XCTAssertTrue(methods.contains("orders.create"))
        XCTAssertTrue(methods.contains("payments.create_refund"))
    }

    // MARK: - Customers

    func testCustomerCrud() throws {
        let c = try commerce.customers.create(email: "bob@example.com", firstName: "Bob", lastName: "Jones",
                                              phone: "+15550001")
        XCTAssertFalse(c.id.isEmpty)
        XCTAssertEqual(c.status, .active)
        XCTAssertEqual(c.fullName, "Bob Jones")
        XCTAssertEqual(try commerce.customers.get(id: c.id)?.id, c.id)
        XCTAssertEqual(try commerce.customers.get(email: "bob@example.com")?.id, c.id)
        XCTAssertEqual(try commerce.customers.update(id: c.id, firstName: "Robert").firstName, "Robert")
        XCTAssertEqual(try commerce.customers.count(), 1)
        try commerce.customers.delete(id: c.id)
        let after = try commerce.customers.get(id: c.id)
        XCTAssertTrue(after == nil || after?.status == .deleted)
    }

    func testDuplicateEmailIsRefused() throws {
        _ = try newCustomer("dup@example.com")
        assertEngineError(.invalidArgument) { _ = try self.newCustomer("dup@example.com") }
    }

    func testLookupsOfMissingEntitiesReturnNil() throws {
        let id = UUID().uuidString.lowercased()
        XCTAssertNil(try commerce.customers.get(id: id))
        XCTAssertNil(try commerce.orders.get(id: id))
        XCTAssertNil(try commerce.products.get(id: id))
        XCTAssertNil(try commerce.carts.get(id: id))
        XCTAssertNil(try commerce.payments.get(id: id))
        XCTAssertNil(try commerce.returns.get(id: id))
        XCTAssertNil(try commerce.shipments.get(id: id))
        XCTAssertNil(try commerce.inventory.getStock(sku: "NO-SUCH-SKU"))
    }

    func testMalformedIdsAreRefusedNotCoerced() {
        assertEngineError(.invalidArgument) { _ = try self.commerce.orders.get(id: "not-a-uuid") }
    }

    func testMutatingAMissingEntityThrowsNotFound() {
        assertEngineError(.notFound) { _ = try self.commerce.orders.cancel(id: UUID().uuidString) }
    }

    func testClosedInstanceRefusesCalls() throws {
        let c = try StateSetCommerce(dbPath: ":memory:")
        c.close()
        c.close()
        assertEngineError { _ = try c.customers.list() }
    }

    // MARK: - Products

    func testProductWithExactDecimalPrice() throws {
        let p = try commerce.products.create(name: "Premium Widget", sku: "WIDGET-001", price: dec("29.99"),
                                             description: "A widget")
        let variant = try XCTUnwrap(commerce.products.variant(sku: "WIDGET-001"))
        XCTAssertEqual(variant.price, dec("29.99"))
        XCTAssertEqual(variant.productId, p.id)
        _ = try commerce.products.addVariant(productId: p.id,
                                             variant: CreateProductVariant(sku: "WIDGET-002", price: dec("31.50")))
        XCTAssertEqual(try commerce.products.variants(productId: p.id).count, 2)
        XCTAssertEqual(try commerce.products.search("Premium").count, 0) // active products only
        XCTAssertEqual(try commerce.products.activate(id: p.id).status, .active)
        XCTAssertEqual(try commerce.products.search("Premium").count, 1)
    }

    func testNegativePriceIsRefused() {
        assertEngineError { _ = try self.commerce.products.create(name: "Bad", sku: "BAD-1", price: self.dec("-1")) }
    }

    // MARK: - Inventory

    func testInventoryAdjustAndReserve() throws {
        _ = try commerce.inventory.createItem(sku: "INV-1", name: "Bolt", initialQuantity: 100)
        XCTAssertEqual(try commerce.inventory.getStock(sku: "INV-1")?.totalOnHand, 100)
        let tx = try commerce.inventory.adjust(sku: "INV-1", quantityDelta: dec("-5.5"), reason: "damaged")
        XCTAssertEqual(tx.quantity, dec("-5.5"))
        XCTAssertEqual(try commerce.inventory.getStock(sku: "INV-1")?.totalOnHand, dec("94.5"))

        let res = try commerce.inventory.reserve(sku: "INV-1", quantity: 10, referenceType: "order", referenceId: "O-1")
        XCTAssertEqual(try commerce.inventory.getStock(sku: "INV-1")?.totalAvailable, dec("84.5"))
        XCTAssertTrue(try commerce.inventory.hasStock(sku: "INV-1", quantity: dec("84.5")))
        XCTAssertFalse(try commerce.inventory.hasStock(sku: "INV-1", quantity: dec("84.6")))
        try commerce.inventory.releaseReservation(id: res.id)
        XCTAssertEqual(try commerce.inventory.getStock(sku: "INV-1")?.totalAvailable, dec("94.5"))
    }

    func testOverReservationIsRefused() throws {
        _ = try commerce.inventory.createItem(sku: "INV-2", name: "Nut", initialQuantity: 1)
        assertEngineError {
            _ = try self.commerce.inventory.reserve(sku: "INV-2", quantity: 2, referenceType: "order", referenceId: "X")
        }
    }

    // MARK: - Orders

    func testOrderTotalsAreExactDecimals() throws {
        let c = try newCustomer()
        let order = try commerce.orders.create(customerId: c.id, items: [line(c, "A", 3, "0.10"), line(c, "B", 1, "19.99")])
        XCTAssertEqual(order.totalAmount, dec("20.29"))
        XCTAssertEqual(order.items.count, 2)
        XCTAssertEqual(order.currency, "USD")
        XCTAssertEqual(order.status, .pending)
        XCTAssertEqual(try commerce.orders.get(orderNumber: order.orderNumber)?.id, order.id)
        XCTAssertEqual(try commerce.orders.list(customerId: c.id).count, 1)
    }

    func testOrderLifecycleFollowsTheEngineStateMachine() throws {
        let c = try newCustomer()
        var order = try commerce.orders.create(customerId: c.id, items: [line(c, "A", 1, "10")])
        order = try commerce.orders.updateStatus(id: order.id, status: .confirmed)
        order = try commerce.orders.ship(id: order.id, trackingNumber: "1Z999")
        XCTAssertEqual(order.status, .shipped)
        XCTAssertEqual(order.trackingNumber, "1Z999")
        order = try commerce.orders.deliver(id: order.id)
        XCTAssertEqual(order.status, .delivered)
        let id = order.id
        assertEngineError { _ = try self.commerce.orders.cancel(id: id) }
    }

    func testExplicitCurrencyIsValidated() throws {
        let c = try newCustomer()
        XCTAssertEqual(try commerce.orders.create(customerId: c.id, items: [line(c, "A", 1, "1")], currency: "eur").currency,
                       "EUR")
        assertEngineError(.invalidArgument) {
            _ = try self.commerce.orders.create(customerId: c.id, items: [self.line(c, "A", 1, "1")], currency: "EURO")
        }
    }

    // MARK: - Carts / checkout

    private let addr = CartAddress(firstName: "Alice", lastName: "Smith", line1: "1 Main St", city: "Austin",
                                   postalCode: "78701", country: "US", state: "TX")

    func testCartCheckoutCreatesARealOrder() throws {
        let c = try newCustomer()
        _ = try commerce.inventory.createItem(sku: "CART-1", name: "Mug", initialQuantity: 10)
        let cart = try commerce.carts.create(customerId: c.id, customerEmail: c.email)
        XCTAssertEqual(cart.status, .active)
        let item = try commerce.carts.addItem(cartId: cart.id, sku: "CART-1", name: "Mug", quantity: 2,
                                              unitPrice: dec("12.50"))
        XCTAssertEqual(item.total, dec("25.00"))
        try commerce.carts.setShipping(cartId: cart.id, address: addr, method: "standard", amount: dec("5.00"))
        try commerce.carts.setPayment(cartId: cart.id, paymentMethod: "credit_card", paymentToken: "tok_test")
        let result = try commerce.carts.complete(cartId: cart.id)
        let order = try XCTUnwrap(commerce.orders.get(id: result.orderId))
        XCTAssertEqual(order.totalAmount, result.totalCharged)
        XCTAssertEqual(try commerce.carts.get(id: cart.id)?.status, .completed)
    }

    // MARK: - Payments / refunds

    func testPaymentAndPartialRefund() throws {
        let c = try newCustomer()
        let order = try commerce.orders.create(customerId: c.id, items: [line(c, "A", 1, "100")])
        var payment = try commerce.payments.create(orderId: order.id, amount: dec("100.00"), customerId: c.id)
        XCTAssertEqual(payment.status, .pending)
        payment = try commerce.payments.complete(id: payment.id)
        XCTAssertEqual(payment.status, .completed)
        let refund = try commerce.payments.refund(paymentId: payment.id, amount: dec("30.01"), reason: "partial")
        XCTAssertEqual(refund.amount, dec("30.01"))
        _ = try commerce.payments.completeRefund(id: refund.id)
        XCTAssertEqual(try commerce.payments.get(id: payment.id)?.amountRefunded, dec("30.01"))
        let pid = payment.id
        assertEngineError { _ = try self.commerce.payments.refund(paymentId: pid, amount: self.dec("70.00")) }
        XCTAssertEqual(try commerce.payments.list(orderId: order.id).count, 1)
    }

    // MARK: - Returns

    func testReturnLifecycle() throws {
        let c = try newCustomer()
        let order = try deliveredOrder(c, sku: "R-1", qty: 2)
        let ret = try commerce.returns.create(orderId: order.id, reason: .defective,
                                              items: [CreateReturnItem(orderItemId: order.items[0].id, quantity: 1,
                                                                       condition: .damaged)],
                                              reasonDetails: "cracked")
        XCTAssertEqual(ret.status, .requested)
        XCTAssertEqual(try commerce.returns.approve(id: ret.id).status, .approved)
        XCTAssertEqual(try commerce.returns.addTracking(id: ret.id, trackingNumber: "RT-1").trackingNumber, "RT-1")
        XCTAssertEqual(try commerce.returns.markReceived(id: ret.id).status, .received)
        let rid = ret.id
        assertEngineError { _ = try self.commerce.returns.complete(id: rid) } // items need a disposition first
        let item = try commerce.returns.setItemDisposition(id: ret.id, itemId: ret.items[0].id, disposition: .scrap)
        XCTAssertEqual(item.disposition, .scrap)
        XCTAssertEqual(try commerce.returns.complete(id: ret.id).status, .completed)
        XCTAssertEqual(try commerce.returns.list(orderId: order.id).count, 1)
    }

    // MARK: - Shipments

    func testShipmentLifecycle() throws {
        let c = try newCustomer()
        let order = try commerce.orders.create(customerId: c.id, items: [line(c, "S-1", 1, "15")])
        _ = try commerce.orders.updateStatus(id: order.id, status: .confirmed)
        var s = try commerce.shipments.create(orderId: order.id, recipientName: "Alice Smith",
                                              shippingAddress: "1 Main St, Austin TX", carrier: .ups,
                                              method: .ground, shippingCost: dec("7.25"))
        XCTAssertEqual(s.status, .pending)
        XCTAssertEqual(s.carrier, .ups)
        XCTAssertEqual(s.shippingCost, dec("7.25"))
        let sid = s.id
        assertEngineError(.invalidArgument) { _ = try self.commerce.shipments.ship(id: sid) } // pending -> shipped is illegal
        XCTAssertEqual(try commerce.shipments.markProcessing(id: s.id).status, .processing)
        XCTAssertEqual(try commerce.shipments.markReady(id: s.id).status, .readyToShip)
        s = try commerce.shipments.ship(id: s.id, trackingNumber: "1ZTRACK")
        XCTAssertEqual(s.status, .shipped)
        XCTAssertEqual(try commerce.shipments.get(trackingNumber: "1ZTRACK")?.id, s.id)
        XCTAssertEqual(try commerce.shipments.markInTransit(id: s.id).status, .inTransit)
        XCTAssertEqual(try commerce.shipments.markOutForDelivery(id: s.id).status, .outForDelivery)
        XCTAssertEqual(try commerce.shipments.deliver(id: s.id).status, .delivered)
        XCTAssertEqual(try commerce.shipments.list(orderId: order.id).count, 1)
    }

    // MARK: - Concurrency

    func testConcurrentCallsOnOneHandle() throws {
        let commerce = self.commerce!
        DispatchQueue.concurrentPerform(iterations: 16) { i in
            _ = try? commerce.customers.create(email: "p\(i)@example.com", firstName: "P", lastName: "\(i)")
        }
        XCTAssertEqual(try commerce.customers.count(), 16)
    }
}

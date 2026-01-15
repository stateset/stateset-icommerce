import XCTest
@testable import StateSet

final class StateSetTests: XCTestCase {
    var commerce: StateSetCommerce!

    override func setUpWithError() throws {
        commerce = try StateSetCommerce(dbPath: ":memory:")
    }

    override func tearDownWithError() throws {
        commerce.close()
        commerce = nil
    }

    // MARK: - API Availability Tests

    func testAllAPIsAvailable() {
        XCTAssertNotNil(commerce.customers)
        XCTAssertNotNil(commerce.products)
        XCTAssertNotNil(commerce.orders)
        XCTAssertNotNil(commerce.inventory)
        XCTAssertNotNil(commerce.carts)
        XCTAssertNotNil(commerce.returns)
        XCTAssertNotNil(commerce.payments)
        XCTAssertNotNil(commerce.analytics)
        XCTAssertNotNil(commerce.shipments)
        XCTAssertNotNil(commerce.warranties)
        XCTAssertNotNil(commerce.suppliers)
        XCTAssertNotNil(commerce.purchaseOrders)
        XCTAssertNotNil(commerce.invoices)
        XCTAssertNotNil(commerce.bom)
        XCTAssertNotNil(commerce.workOrders)
        XCTAssertNotNil(commerce.currency)
        XCTAssertNotNil(commerce.subscriptions)
        XCTAssertNotNil(commerce.promotions)
        XCTAssertNotNil(commerce.tax)
        XCTAssertNotNil(commerce.quality)
        XCTAssertNotNil(commerce.lots)
        XCTAssertNotNil(commerce.serials)
        XCTAssertNotNil(commerce.warehouse)
        XCTAssertNotNil(commerce.receiving)
        XCTAssertNotNil(commerce.fulfillment)
        XCTAssertNotNil(commerce.accountsPayable)
        XCTAssertNotNil(commerce.accountsReceivable)
        XCTAssertNotNil(commerce.costAccounting)
        XCTAssertNotNil(commerce.credit)
        XCTAssertNotNil(commerce.backorders)
        XCTAssertNotNil(commerce.generalLedger)
    }

    // MARK: - Customer Tests

    func testCreateCustomer() throws {
        let customer = try commerce.customers.create(
            email: "test@example.com",
            firstName: "Test",
            lastName: "User"
        )

        XCTAssertFalse(customer.id.isEmpty)
        XCTAssertEqual(customer.email, "test@example.com")
        XCTAssertEqual(customer.firstName, "Test")
        XCTAssertEqual(customer.lastName, "User")
    }

    func testGetCustomer() throws {
        let created = try commerce.customers.create(
            email: "get@example.com",
            firstName: "Get",
            lastName: "Test"
        )

        let retrieved = try commerce.customers.get(id: created.id)
        XCTAssertNotNil(retrieved)
        XCTAssertEqual(retrieved?.id, created.id)
        XCTAssertEqual(retrieved?.email, "get@example.com")
    }

    func testListCustomers() throws {
        _ = try commerce.customers.create(
            email: "list1@example.com",
            firstName: "List",
            lastName: "One"
        )
        _ = try commerce.customers.create(
            email: "list2@example.com",
            firstName: "List",
            lastName: "Two"
        )

        let customers = try commerce.customers.list()
        XCTAssertGreaterThanOrEqual(customers.count, 2)
    }

    func testDeleteCustomer() throws {
        let customer = try commerce.customers.create(
            email: "delete@example.com",
            firstName: "Delete",
            lastName: "Me"
        )

        let deleted = try commerce.customers.delete(id: customer.id)
        XCTAssertTrue(deleted)

        let retrieved = try commerce.customers.get(id: customer.id)
        XCTAssertNil(retrieved)
    }

    // MARK: - Product Tests

    func testCreateProduct() throws {
        let product = try commerce.products.create(
            name: "Test Product",
            sku: "TEST-001",
            price: 29.99,
            description: "A test product"
        )

        XCTAssertFalse(product.id.isEmpty)
        XCTAssertEqual(product.name, "Test Product")
        XCTAssertEqual(product.sku, "TEST-001")
    }

    func testListProducts() throws {
        _ = try commerce.products.create(
            name: "Product A",
            sku: "PROD-A",
            price: 10.00
        )
        _ = try commerce.products.create(
            name: "Product B",
            sku: "PROD-B",
            price: 20.00
        )

        let products = try commerce.products.list()
        XCTAssertGreaterThanOrEqual(products.count, 2)
    }

    // MARK: - Inventory Tests

    func testCreateInventoryItem() throws {
        let item = try commerce.inventory.createItem(
            sku: "INV-001",
            name: "Inventory Item",
            initialQuantity: 100
        )

        XCTAssertEqual(item.sku, "INV-001")
    }

    func testAdjustInventory() throws {
        _ = try commerce.inventory.createItem(
            sku: "ADJ-001",
            name: "Adjust Test",
            initialQuantity: 50
        )

        let adjusted = try commerce.inventory.adjust(
            sku: "ADJ-001",
            quantityDelta: 10,
            reason: "Received shipment"
        )
        XCTAssertTrue(adjusted)

        let level = try commerce.inventory.getLevel(sku: "ADJ-001")
        XCTAssertNotNil(level)
        XCTAssertEqual(level?.available, 60)
    }

    // MARK: - Order Tests

    func testCreateOrder() throws {
        let customer = try commerce.customers.create(
            email: "order@example.com",
            firstName: "Order",
            lastName: "Test"
        )

        let order = try commerce.orders.create(
            customerId: customer.id,
            items: [
                OrderItem(sku: "TEST-SKU", name: "Test Item", quantity: 2, unitPrice: 19.99)
            ],
            currency: "USD"
        )

        XCTAssertFalse(order.id.isEmpty)
        XCTAssertEqual(order.customerId, customer.id)
    }

    func testOrderLifecycle() throws {
        let customer = try commerce.customers.create(
            email: "lifecycle@example.com",
            firstName: "Lifecycle",
            lastName: "Test"
        )

        let order = try commerce.orders.create(
            customerId: customer.id,
            items: [
                OrderItem(sku: "LIFE-001", name: "Lifecycle Item", quantity: 1, unitPrice: 49.99)
            ]
        )

        // Ship order
        let shipped = try commerce.orders.ship(id: order.id)
        XCTAssertEqual(shipped.status, "shipped")
    }

    // MARK: - Cart Tests

    func testCreateCart() throws {
        let cart = try commerce.carts.create(currency: "USD")

        XCTAssertFalse(cart.id.isEmpty)
        XCTAssertEqual(cart.currency, "USD")
    }

    // MARK: - Analytics Tests

    func testSalesSummary() throws {
        let summary = try commerce.analytics.salesSummary()
        XCTAssertNotNil(summary)
    }

    // MARK: - Currency Tests

    func testSetExchangeRate() throws {
        let rate = try commerce.currency.setRate(
            from: .usd,
            to: .eur,
            rate: 0.85
        )

        XCTAssertEqual(rate.fromCurrency, "USD")
        XCTAssertEqual(rate.toCurrency, "EUR")
        XCTAssertEqual(rate.rate, 0.85)
    }

    func testConvertCurrency() throws {
        _ = try commerce.currency.setRate(from: .usd, to: .eur, rate: 0.85)

        let result = try commerce.currency.convert(
            amount: 100.0,
            from: .usd,
            to: .eur
        )

        XCTAssertEqual(result.convertedAmount, 85.0, accuracy: 0.01)
    }

    // MARK: - Subscription Tests

    func testCreateSubscriptionPlan() throws {
        let plan = try commerce.subscriptions.createPlan(
            code: "BASIC",
            name: "Basic Plan",
            interval: "month",
            intervalCount: 1,
            price: 9.99,
            currency: "USD"
        )

        XCTAssertFalse(plan.id.isEmpty)
        XCTAssertEqual(plan.code, "BASIC")
    }

    func testListSubscriptionPlans() throws {
        _ = try commerce.subscriptions.createPlan(
            code: "LIST-PLAN",
            name: "List Plan",
            interval: "month",
            intervalCount: 1,
            price: 19.99
        )

        let plans = try commerce.subscriptions.listPlans()
        XCTAssertGreaterThanOrEqual(plans.count, 1)
    }

    // MARK: - Promotion Tests

    func testCreatePromotion() throws {
        let promo = try commerce.promotions.create(
            code: "TEST20",
            name: "Test Discount",
            discountType: "percentage",
            discountValue: 20.0
        )

        XCTAssertFalse(promo.id.isEmpty)
        XCTAssertEqual(promo.code, "TEST20")
    }

    func testGetActivePromotions() throws {
        let promo = try commerce.promotions.create(
            code: "ACTIVE10",
            name: "Active Promo",
            discountType: "percentage",
            discountValue: 10.0
        )
        _ = try commerce.promotions.activate(id: promo.id)

        let active = try commerce.promotions.getActive()
        XCTAssertGreaterThanOrEqual(active.count, 1)
    }

    // MARK: - Tax Tests

    func testGetTaxSettings() throws {
        let settings = try commerce.tax.getSettings()
        XCTAssertNotNil(settings)
    }

    // MARK: - Warehouse Tests

    func testCreateWarehouse() throws {
        let warehouse = try commerce.warehouse.createWarehouse(
            code: "WH-TEST",
            name: "Test Warehouse",
            warehouseType: "distribution"
        )

        XCTAssertEqual(warehouse.code, "WH-TEST")
        XCTAssertEqual(warehouse.name, "Test Warehouse")
    }

    func testListWarehouses() throws {
        _ = try commerce.warehouse.createWarehouse(
            code: "WH-LIST",
            name: "List Warehouse"
        )

        let warehouses = try commerce.warehouse.listWarehouses()
        XCTAssertGreaterThanOrEqual(warehouses.count, 1)
    }

    // MARK: - General Ledger Tests

    func testCreateGlAccount() throws {
        let account = try commerce.generalLedger.createAccount(
            accountNumber: "1000",
            name: "Cash",
            accountType: "asset"
        )

        XCTAssertFalse(account.id.isEmpty)
        XCTAssertEqual(account.accountNumber, "1000")
        XCTAssertEqual(account.name, "Cash")
    }

    func testListGlAccounts() throws {
        _ = try commerce.generalLedger.createAccount(
            accountNumber: "2000",
            name: "Accounts Payable",
            accountType: "liability"
        )

        let accounts = try commerce.generalLedger.listAccounts()
        XCTAssertGreaterThanOrEqual(accounts.count, 1)
    }
}

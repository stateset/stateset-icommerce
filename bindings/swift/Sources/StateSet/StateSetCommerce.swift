import Foundation
import StateSetC

/// StateSet Embedded Commerce - The SQLite of Commerce
///
/// A zero-dependency, local-first commerce engine for Swift/iOS applications.
///
/// Example usage:
/// ```swift
/// let commerce = try StateSetCommerce(dbPath: "store.db")
///
/// let customer = try commerce.customers.create(
///     email: "alice@example.com",
///     firstName: "Alice",
///     lastName: "Smith"
/// )
///
/// let product = try commerce.products.create(
///     name: "Premium Widget",
///     sku: "WIDGET-001",
///     price: 29.99
/// )
///
/// let order = try commerce.orders.create(
///     customerId: customer.id,
///     items: [OrderItem(sku: "WIDGET-001", name: "Widget", quantity: 2, unitPrice: 29.99)],
///     currency: "USD"
/// )
/// ```
public final class StateSetCommerce {
    private var handle: StateSetHandle?

    /// Customers API
    public private(set) lazy var customers = CustomersAPI(commerce: self)

    /// Products API
    public private(set) lazy var products = ProductsAPI(commerce: self)

    /// Orders API
    public private(set) lazy var orders = OrdersAPI(commerce: self)

    /// Inventory API
    public private(set) lazy var inventory = InventoryAPI(commerce: self)

    /// Carts API
    public private(set) lazy var carts = CartsAPI(commerce: self)

    /// Returns API
    public private(set) lazy var returns = ReturnsAPI(commerce: self)

    /// Payments API
    public private(set) lazy var payments = PaymentsAPI(commerce: self)

    /// Analytics API
    public private(set) lazy var analytics = AnalyticsAPI(commerce: self)

    /// Shipments API
    public private(set) lazy var shipments = ShipmentsAPI(commerce: self)

    /// Warranties API
    public private(set) lazy var warranties = WarrantiesAPI(commerce: self)

    /// Suppliers API
    public private(set) lazy var suppliers = SuppliersAPI(commerce: self)

    /// Purchase Orders API
    public private(set) lazy var purchaseOrders = PurchaseOrdersAPI(commerce: self)

    /// Invoices API
    public private(set) lazy var invoices = InvoicesAPI(commerce: self)

    /// Bill of Materials API
    public private(set) lazy var bom = BOMAPI(commerce: self)

    /// Work Orders API
    public private(set) lazy var workOrders = WorkOrdersAPI(commerce: self)

    /// Currency API
    public private(set) lazy var currency = CurrencyAPI(commerce: self)

    /// Subscriptions API
    public private(set) lazy var subscriptions = SubscriptionsAPI(commerce: self)

    /// Promotions API
    public private(set) lazy var promotions = PromotionsAPI(commerce: self)

    /// Tax API
    public private(set) lazy var tax = TaxAPI(commerce: self)

    /// Quality API
    public private(set) lazy var quality = QualityAPI(commerce: self)

    /// Lots API
    public private(set) lazy var lots = LotsAPI(commerce: self)

    /// Serials API
    public private(set) lazy var serials = SerialsAPI(commerce: self)

    /// Warehouse API
    public private(set) lazy var warehouse = WarehouseAPI(commerce: self)

    /// Receiving API
    public private(set) lazy var receiving = ReceivingAPI(commerce: self)

    /// Fulfillment API
    public private(set) lazy var fulfillment = FulfillmentAPI(commerce: self)

    /// Accounts Payable API
    public private(set) lazy var accountsPayable = AccountsPayableAPI(commerce: self)

    /// Accounts Receivable API
    public private(set) lazy var accountsReceivable = AccountsReceivableAPI(commerce: self)

    /// Cost Accounting API
    public private(set) lazy var costAccounting = CostAccountingAPI(commerce: self)

    /// Credit API
    public private(set) lazy var credit = CreditAPI(commerce: self)

    /// Backorders API
    public private(set) lazy var backorders = BackordersAPI(commerce: self)

    /// General Ledger API
    public private(set) lazy var generalLedger = GeneralLedgerAPI(commerce: self)

    /// Create a new Commerce instance
    /// - Parameter dbPath: Path to SQLite database file, or ":memory:" for in-memory database
    /// - Throws: StateSetError if database initialization fails
    public init(dbPath: String) throws {
        handle = stateset_commerce_new(dbPath)
        if handle == nil {
            throw StateSetError.initializationFailed("Failed to create commerce instance")
        }
    }

    deinit {
        close()
    }

    /// Close the commerce instance and release native resources
    public func close() {
        if let h = handle {
            stateset_commerce_free(h)
            handle = nil
        }
    }

    internal func getHandle() throws -> StateSetHandle {
        guard let h = handle else {
            throw StateSetError.invalidHandle
        }
        return h
    }

    internal func parseJSON<T: Decodable>(_ ptr: UnsafeMutablePointer<CChar>?) throws -> T {
        guard let ptr = ptr else {
            throw StateSetError.nullPointer
        }
        defer { stateset_string_free(ptr) }

        let jsonString = String(cString: ptr)
        guard let data = jsonString.data(using: .utf8) else {
            throw StateSetError.invalidJSON("Failed to convert string to data")
        }

        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try decoder.decode(T.self, from: data)
    }

    internal func parseOptionalJSON<T: Decodable>(_ ptr: UnsafeMutablePointer<CChar>?) -> T? {
        guard let ptr = ptr else { return nil }
        defer { stateset_string_free(ptr) }

        let jsonString = String(cString: ptr)
        guard let data = jsonString.data(using: .utf8) else { return nil }

        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try? decoder.decode(T.self, from: data)
    }
}

/// StateSet error types
public enum StateSetError: Error, LocalizedError {
    case initializationFailed(String)
    case invalidHandle
    case nullPointer
    case invalidJSON(String)
    case invalidUUID
    case operationFailed(String)

    public var errorDescription: String? {
        switch self {
        case .initializationFailed(let msg): return "Initialization failed: \(msg)"
        case .invalidHandle: return "Invalid commerce handle"
        case .nullPointer: return "Null pointer returned from native code"
        case .invalidJSON(let msg): return "Invalid JSON: \(msg)"
        case .invalidUUID: return "Invalid UUID format"
        case .operationFailed(let msg): return "Operation failed: \(msg)"
        }
    }
}

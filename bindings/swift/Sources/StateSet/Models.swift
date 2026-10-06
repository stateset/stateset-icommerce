import Foundation

// These types mirror the JSON the Rust engine (stateset-embedded) emits
// through stateset-ffi's JSON call surface. Every money/quantity field is a
// `Decimal` decoded from an exact decimal STRING -- never a Double.

// MARK: - Exact decimal decoding

/// Decodes a `Decimal` from the engine's exact decimal string ("19.99").
@propertyWrapper
public struct DecimalString: Codable, Equatable, Hashable, Sendable {
    public var wrappedValue: Decimal

    public init(wrappedValue: Decimal) { self.wrappedValue = wrappedValue }

    public init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        wrappedValue = try DecimalString.decode(c)
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        try c.encode(JSONArgs.decimalString(wrappedValue))
    }

    static func decode(_ c: SingleValueDecodingContainer) throws -> Decimal {
        if let s = try? c.decode(String.self) {
            guard let d = Decimal(string: s, locale: Locale(identifier: "en_US_POSIX")) else {
                throw DecodingError.dataCorruptedError(in: c, debugDescription: "'\(s)' is not a decimal")
            }
            return d
        }
        throw DecodingError.dataCorruptedError(in: c, debugDescription: "expected a decimal string")
    }
}

/// Optional counterpart of `DecimalString`; a missing key or `null` is `nil`.
@propertyWrapper
public struct OptionalDecimalString: Codable, Equatable, Hashable, Sendable {
    public var wrappedValue: Decimal?

    public init(wrappedValue: Decimal?) { self.wrappedValue = wrappedValue }

    public init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        wrappedValue = c.decodeNil() ? nil : try DecimalString.decode(c)
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        if let v = wrappedValue { try c.encode(JSONArgs.decimalString(v)) } else { try c.encodeNil() }
    }
}

extension KeyedDecodingContainer {
    /// Lets `@OptionalDecimalString` properties tolerate an absent key.
    public func decode(_ type: OptionalDecimalString.Type, forKey key: Key) throws -> OptionalDecimalString {
        try decodeIfPresent(type, forKey: key) ?? OptionalDecimalString(wrappedValue: nil)
    }
}

// MARK: - Enums

public enum CustomerStatus: String, Codable, Sendable { case active, inactive, suspended, deleted }
public enum ProductStatus: String, Codable, Sendable { case draft, active, archived }
public enum ProductType: String, Codable, Sendable { case simple, variable, bundle, digital }

public enum OrderStatus: String, Codable, Sendable {
    case pending, confirmed, processing
    case partiallyShipped = "partially_shipped"
    case shipped, delivered, cancelled, refunded
}

public enum OrderPaymentStatus: String, Codable, Sendable {
    case pending, authorized, paid
    case partiallyPaid = "partially_paid"
    case refunded
    case partiallyRefunded = "partially_refunded"
    case failed
}

public enum FulfillmentStatus: String, Codable, Sendable {
    case unfulfilled
    case partiallyFulfilled = "partially_fulfilled"
    case fulfilled, shipped, delivered
}

public enum CartStatus: String, Codable, Sendable {
    case active
    case readyForPayment = "ready_for_payment"
    case paymentPending = "payment_pending"
    case completed, abandoned, cancelled, expired
}

public enum CartPaymentStatus: String, Codable, Sendable {
    case none
    case methodSelected = "method_selected"
    case authorized, captured, failed, refunded
}

public enum PaymentMethod: String, Codable, Sendable {
    case creditCard = "credit_card"
    case debitCard = "debit_card"
    case bankTransfer = "bank_transfer"
    case payPal = "pay_pal"
    case applePay = "apple_pay"
    case googlePay = "google_pay"
    case crypto, stablecoin
    case storeCredit = "store_credit"
    case giftCard = "gift_card"
    case cashOnDelivery = "cash_on_delivery"
    case invoice, other
}

public enum PaymentStatus: String, Codable, Sendable {
    case pending, processing
    case requiresAction = "requires_action"
    case completed, failed, cancelled, refunded
    case partiallyRefunded = "partially_refunded"
    case disputed
}

public enum RefundStatus: String, Codable, Sendable { case pending, processing, completed, failed, cancelled }

public enum ReturnReason: String, Codable, Sendable {
    case defective
    case wrongItem = "wrong_item"
    case notAsDescribed = "not_as_described"
    case changedMind = "changed_mind"
    case betterPriceFound = "better_price_found"
    case noLongerNeeded = "no_longer_needed"
    case damaged, other
}

public enum ReturnStatus: String, Codable, Sendable {
    case requested, approved, rejected
    case inTransit = "in_transit"
    case received, inspecting, completed, cancelled
}

public enum ItemCondition: String, Codable, Sendable { case new, opened, used, damaged, defective }

public enum ReturnDisposition: String, Codable, Sendable {
    case restock, refurbish, scrap
    case returnToVendor = "return_to_vendor"
    case quarantine
}

public enum ShipmentStatus: String, Codable, Sendable {
    case pending, processing
    case readyToShip = "ready_to_ship"
    case shipped
    case inTransit = "in_transit"
    case outForDelivery = "out_for_delivery"
    case delivered, failed, returned, cancelled
    case onHold = "on_hold"
}

public enum ShippingCarrier: String, Codable, Sendable {
    case other, ups
    case fedEx = "fed_ex"
    case usps, dhl
    case onTrac = "on_trac"
    case laserShip = "laser_ship"
}

public enum ShippingMethod: String, Codable, Sendable {
    case standard, express, overnight
    case twoDay = "two_day"
    case ground, international
    case sameDay = "same_day"
    case freight
}

public enum ReservationStatus: String, Codable, Sendable {
    case pending, confirmed, allocated, cancelled, released, expired, fulfilled
}

// MARK: - Customers

public struct Customer: Codable, Sendable {
    public let id: String
    public let email: String
    public let firstName: String
    public let lastName: String
    public let phone: String?
    public let status: CustomerStatus
    public let acceptsMarketing: Bool
    public let emailVerified: Bool
    public let tags: [String]
    public let createdAt: Date
    public let updatedAt: Date

    public var fullName: String { "\(firstName) \(lastName)".trimmingCharacters(in: .whitespaces) }
}

// MARK: - Products

public struct Product: Codable, Sendable {
    public let id: String
    public let name: String
    public let slug: String
    public let description: String
    public let status: ProductStatus
    public let productType: ProductType
    public let createdAt: Date
    public let updatedAt: Date
}

public struct ProductVariant: Codable, Sendable {
    public let id: String
    public let productId: String
    public let sku: String
    public let name: String
    @DecimalString public var price: Decimal
    @OptionalDecimalString public var compareAtPrice: Decimal?
    @OptionalDecimalString public var cost: Decimal?
    public let barcode: String?
    public let isDefault: Bool
    public let isActive: Bool
}

/// Input for a product variant.
public struct CreateProductVariant: JSONArgConvertible, Sendable {
    public var sku: String
    public var price: Decimal
    public var name: String?
    public var compareAtPrice: Decimal?
    public var cost: Decimal?
    public var isDefault: Bool?

    public init(sku: String, price: Decimal, name: String? = nil, compareAtPrice: Decimal? = nil,
                cost: Decimal? = nil, isDefault: Bool? = nil) {
        self.sku = sku; self.price = price; self.name = name
        self.compareAtPrice = compareAtPrice; self.cost = cost; self.isDefault = isDefault
    }

    var jsonArg: Any {
        ["sku": sku, "price": price, "name": name, "compare_at_price": compareAtPrice,
         "cost": cost, "is_default": isDefault] as [String: Any?]
    }
}

// MARK: - Inventory

public struct InventoryItem: Codable, Sendable {
    public let id: Int64
    public let sku: String
    public let name: String
    public let description: String?
    public let unitOfMeasure: String
    public let isActive: Bool
}

public struct LocationStock: Codable, Sendable {
    public let locationId: Int32
    public let locationName: String?
    @DecimalString public var onHand: Decimal
    @DecimalString public var allocated: Decimal
    @DecimalString public var available: Decimal
}

public struct StockLevel: Codable, Sendable {
    public let sku: String
    public let name: String
    @DecimalString public var totalOnHand: Decimal
    @DecimalString public var totalAllocated: Decimal
    @DecimalString public var totalAvailable: Decimal
    public let locations: [LocationStock]
}

public struct InventoryTransaction: Codable, Sendable {
    public let id: Int64
    public let itemId: Int64
    public let locationId: Int32
    public let transactionType: String
    @DecimalString public var quantity: Decimal
    public let reason: String?
    public let createdAt: Date
}

public struct InventoryReservation: Codable, Sendable {
    public let id: String
    public let itemId: Int64
    public let locationId: Int32
    @DecimalString public var quantity: Decimal
    public let status: ReservationStatus
    public let referenceType: String
    public let referenceId: String
    public let expiresAt: Date?
    public let createdAt: Date
}

// MARK: - Carts

public struct CartAddress: Codable, Sendable, JSONArgConvertible {
    public var firstName: String
    public var lastName: String
    public var company: String?
    public var line1: String
    public var line2: String?
    public var city: String
    public var state: String?
    public var postalCode: String
    public var country: String
    public var phone: String?
    public var email: String?

    public init(firstName: String, lastName: String, line1: String, city: String, postalCode: String,
                country: String, state: String? = nil, line2: String? = nil, company: String? = nil,
                phone: String? = nil, email: String? = nil) {
        self.firstName = firstName; self.lastName = lastName; self.company = company
        self.line1 = line1; self.line2 = line2; self.city = city; self.state = state
        self.postalCode = postalCode; self.country = country; self.phone = phone; self.email = email
    }

    var jsonArg: Any {
        ["first_name": firstName, "last_name": lastName, "company": company, "line1": line1, "line2": line2,
         "city": city, "state": state, "postal_code": postalCode, "country": country, "phone": phone,
         "email": email] as [String: Any?]
    }
}

public struct Cart: Codable, Sendable {
    public let id: String
    public let cartNumber: String
    public let customerId: String?
    public let status: CartStatus
    public let currency: String
    public let items: [CartItem]
    @DecimalString public var subtotal: Decimal
    @DecimalString public var taxAmount: Decimal
    @DecimalString public var shippingAmount: Decimal
    @DecimalString public var discountAmount: Decimal
    @DecimalString public var grandTotal: Decimal
    public let customerEmail: String?
    public let shippingAddress: CartAddress?
    public let billingAddress: CartAddress?
    public let shippingMethod: String?
    public let paymentMethod: String?
    public let paymentStatus: CartPaymentStatus
    public let couponCode: String?
    public let orderId: String?
    public let orderNumber: String?
    public let createdAt: Date
    public let updatedAt: Date
}

public struct CartItem: Codable, Sendable {
    public let id: String
    public let cartId: String
    public let productId: String?
    public let variantId: String?
    public let sku: String
    public let name: String
    public let quantity: Int32
    @DecimalString public var unitPrice: Decimal
    @DecimalString public var discountAmount: Decimal
    @DecimalString public var taxAmount: Decimal
    @DecimalString public var total: Decimal
    public let requiresShipping: Bool
}

/// Input for a cart line.
public struct AddCartItem: JSONArgConvertible, Sendable {
    public var sku: String
    public var name: String
    public var quantity: Int32
    public var unitPrice: Decimal
    public var productId: String?
    public var variantId: String?

    public init(sku: String, name: String, quantity: Int32 = 1, unitPrice: Decimal,
                productId: String? = nil, variantId: String? = nil) {
        self.sku = sku; self.name = name; self.quantity = quantity; self.unitPrice = unitPrice
        self.productId = productId; self.variantId = variantId
    }

    var jsonArg: Any {
        ["sku": sku, "name": name, "quantity": Int(quantity), "unit_price": unitPrice,
         "product_id": productId, "variant_id": variantId] as [String: Any?]
    }
}

public struct CheckoutResult: Codable, Sendable {
    public let cartId: String
    public let orderId: String
    public let orderNumber: String
    public let paymentId: String?
    @DecimalString public var totalCharged: Decimal
    public let currency: String
}

// MARK: - Orders

public struct Address: Codable, Sendable, JSONArgConvertible {
    public var line1: String
    public var line2: String?
    public var city: String
    public var state: String?
    public var postalCode: String
    public var country: String

    public init(line1: String, city: String, postalCode: String, country: String,
                state: String? = nil, line2: String? = nil) {
        self.line1 = line1; self.line2 = line2; self.city = city; self.state = state
        self.postalCode = postalCode; self.country = country
    }

    var jsonArg: Any {
        ["line1": line1, "line2": line2, "city": city, "state": state, "postal_code": postalCode,
         "country": country] as [String: Any?]
    }
}

public struct Order: Codable, Sendable {
    public let id: String
    public let orderNumber: String
    public let customerId: String
    public let status: OrderStatus
    @DecimalString public var totalAmount: Decimal
    @DecimalString public var taxAmount: Decimal
    @DecimalString public var shippingAmount: Decimal
    @DecimalString public var discountAmount: Decimal
    public let currency: String
    public let paymentStatus: OrderPaymentStatus
    public let fulfillmentStatus: FulfillmentStatus
    public let paymentMethod: String?
    public let trackingNumber: String?
    public let notes: String?
    public let shippingAddress: Address?
    public let items: [OrderItem]
    public let version: Int32
    public let createdAt: Date
    public let updatedAt: Date
}

public struct OrderItem: Codable, Sendable {
    public let id: String
    public let orderId: String
    public let productId: String
    public let variantId: String?
    public let sku: String
    public let name: String
    public let quantity: Int32
    public let shippedQuantity: Int32
    @DecimalString public var unitPrice: Decimal
    @DecimalString public var discount: Decimal
    @DecimalString public var taxAmount: Decimal
    @DecimalString public var total: Decimal
}

/// Input for an order line.
public struct CreateOrderItem: JSONArgConvertible, Sendable {
    public var productId: String
    public var sku: String
    public var name: String
    public var quantity: Int32
    public var unitPrice: Decimal
    public var variantId: String?
    public var discount: Decimal?
    public var taxAmount: Decimal?

    public init(productId: String, sku: String, name: String, quantity: Int32 = 1, unitPrice: Decimal,
                variantId: String? = nil, discount: Decimal? = nil, taxAmount: Decimal? = nil) {
        self.productId = productId; self.sku = sku; self.name = name; self.quantity = quantity
        self.unitPrice = unitPrice; self.variantId = variantId; self.discount = discount; self.taxAmount = taxAmount
    }

    var jsonArg: Any {
        ["product_id": productId, "variant_id": variantId, "sku": sku, "name": name, "quantity": Int(quantity),
         "unit_price": unitPrice, "discount": discount, "tax_amount": taxAmount] as [String: Any?]
    }
}

// MARK: - Payments

public struct Payment: Codable, Sendable {
    public let id: String
    public let paymentNumber: String
    public let orderId: String?
    public let customerId: String?
    public let status: PaymentStatus
    public let paymentMethod: PaymentMethod
    @DecimalString public var amount: Decimal
    public let currency: String
    @DecimalString public var amountRefunded: Decimal
    public let externalId: String?
    public let failureReason: String?
    public let failureCode: String?
    public let paidAt: Date?
    public let version: Int32
    public let createdAt: Date
    public let updatedAt: Date
}

public struct Refund: Codable, Sendable {
    public let id: String
    public let refundNumber: String
    public let paymentId: String
    public let status: RefundStatus
    @DecimalString public var amount: Decimal
    public let currency: String
    public let reason: String?
    public let failureReason: String?
    public let refundedAt: Date?
    public let createdAt: Date
}

// MARK: - Returns

public struct Return: Codable, Sendable {
    public let id: String
    public let orderId: String
    public let customerId: String
    public let status: ReturnStatus
    public let reason: ReturnReason
    public let reasonDetails: String?
    @OptionalDecimalString public var refundAmount: Decimal?
    public let trackingNumber: String?
    public let items: [ReturnItem]
    public let notes: String?
    public let version: Int32
    public let createdAt: Date
    public let updatedAt: Date
}

public struct ReturnItem: Codable, Sendable {
    public let id: String
    public let returnId: String
    public let orderItemId: String
    public let sku: String
    public let name: String
    public let quantity: Int32
    public let condition: ItemCondition
    @DecimalString public var refundAmount: Decimal
    public let disposition: ReturnDisposition?
}

/// Input for a return line.
public struct CreateReturnItem: JSONArgConvertible, Sendable {
    public var orderItemId: String
    public var quantity: Int32
    public var condition: ItemCondition?

    public init(orderItemId: String, quantity: Int32 = 1, condition: ItemCondition? = nil) {
        self.orderItemId = orderItemId; self.quantity = quantity; self.condition = condition
    }

    var jsonArg: Any {
        ["order_item_id": orderItemId, "quantity": Int(quantity), "condition": condition?.rawValue] as [String: Any?]
    }
}

// MARK: - Shipments

public struct Shipment: Codable, Sendable {
    public let id: String
    public let shipmentNumber: String
    public let orderId: String
    public let status: ShipmentStatus
    public let carrier: ShippingCarrier
    public let shippingMethod: ShippingMethod
    public let trackingNumber: String?
    public let trackingUrl: String?
    public let recipientName: String
    public let shippingAddress: String
    @OptionalDecimalString public var shippingCost: Decimal?
    @OptionalDecimalString public var weightKg: Decimal?
    public let signatureRequired: Bool
    public let shippedAt: Date?
    public let deliveredAt: Date?
    public let items: [ShipmentItem]
    public let version: Int32
    public let createdAt: Date
    public let updatedAt: Date
}

public struct ShipmentItem: Codable, Sendable {
    public let id: String
    public let shipmentId: String
    public let orderItemId: String?
    public let productId: String?
    public let sku: String
    public let name: String
    public let quantity: Int32
}

/// Input for a shipment line.
public struct CreateShipmentItem: JSONArgConvertible, Sendable {
    public var sku: String
    public var name: String
    public var quantity: Int32
    public var orderItemId: String?
    public var productId: String?

    public init(sku: String, name: String, quantity: Int32 = 1, orderItemId: String? = nil, productId: String? = nil) {
        self.sku = sku; self.name = name; self.quantity = quantity
        self.orderItemId = orderItemId; self.productId = productId
    }

    var jsonArg: Any {
        ["sku": sku, "name": name, "quantity": Int(quantity), "order_item_id": orderItemId,
         "product_id": productId] as [String: Any?]
    }
}

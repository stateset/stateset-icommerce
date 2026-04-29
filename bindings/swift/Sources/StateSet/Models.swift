import Foundation

// MARK: - Customer

public struct Customer: Codable, Identifiable, Sendable {
    public let id: String
    public let email: String
    public let firstName: String
    public let lastName: String
    public let phone: String?
    public let createdAt: String?
    public let updatedAt: String?
}

// MARK: - Product

public struct Product: Codable, Identifiable, Sendable {
    public let id: String
    public let name: String
    public let sku: String
    public let price: Double?
    public let slug: String?
    public let description: String?
    public let isActive: Bool?
    public let createdAt: String?
    public let updatedAt: String?
}

public struct ProductVariant: Codable, Identifiable, Sendable {
    public let id: String
    public let productId: String
    public let sku: String
    public let name: String?
    public let price: String
    public let compareAtPrice: String?
    public let isDefault: Bool?
    public let createdAt: String?
}

// MARK: - Order

public struct Order: Codable, Identifiable, Sendable {
    public let id: String
    public let orderNumber: String
    public let customerId: String
    public let status: String
    public let totalAmount: String
    public let currency: String
    public let createdAt: String?
    public let updatedAt: String?
}

public struct OrderItem: Codable, Sendable {
    public let sku: String
    public let name: String
    public let quantity: Int
    public let unitPrice: Double

    public init(sku: String, name: String, quantity: Int, unitPrice: Double) {
        self.sku = sku
        self.name = name
        self.quantity = quantity
        self.unitPrice = unitPrice
    }
}

// MARK: - Inventory

public struct InventoryItem: Codable, Identifiable, Sendable {
    public let id: String
    public let sku: String
    public let name: String
    public let description: String?
    public let unitOfMeasure: String?
    public let createdAt: String?
}

public struct StockLevel: Codable, Identifiable, Sendable {
    public let id: String
    public let inventoryItemId: String
    public let locationId: String?
    public let available: Int
    public let reserved: Int
    public let incoming: Int?
    public let updatedAt: String?
}

// MARK: - Cart

public struct Cart: Codable, Identifiable, Sendable {
    public let id: String
    public let customerId: String?
    public let status: String
    public let grandTotal: String
    public let currency: String
    public let createdAt: String?
}

public struct CartItem: Codable, Identifiable, Sendable {
    public let id: String
    public let cartId: String
    public let variantId: String
    public let quantity: Int
    public let unitPrice: String
    public let lineTotal: String
}

// MARK: - Return

public struct Return: Codable, Identifiable, Sendable {
    public let id: String
    public let orderId: String
    public let reason: String
    public let status: String
    public let refundAmount: String?
    public let notes: String?
    public let createdAt: String?
}

// MARK: - Payment

public struct Payment: Codable, Identifiable, Sendable {
    public let id: String
    public let orderId: String
    public let amount: String
    public let currency: String
    public let method: String
    public let status: String
    public let createdAt: String?
}

public struct Refund: Codable, Identifiable, Sendable {
    public let id: String
    public let paymentId: String
    public let amount: String
    public let currency: String
    public let status: String
    public let reason: String?
    public let createdAt: String?
}

// MARK: - Analytics

public struct SalesSummary: Codable, Sendable {
    public let totalRevenue: String
    public let orderCount: Int
    public let averageOrderValue: String
}

public struct TopProduct: Codable, Sendable {
    public let productId: String
    public let productName: String
    public let totalQuantity: Int
    public let totalRevenue: String
}

public struct TopCustomer: Codable, Sendable {
    public let customerId: String
    public let customerName: String
    public let orderCount: Int
    public let totalSpent: String
}

// MARK: - Shipment

public struct Shipment: Codable, Identifiable, Sendable {
    public let id: String
    public let shipmentNumber: String
    public let orderId: String
    public let status: String
    public let carrier: String?
    public let trackingNumber: String?
    public let trackingUrl: String?
    public let recipientName: String
    public let recipientEmail: String?
    public let shippingAddress: String
    public let shippedAt: String?
    public let deliveredAt: String?
    public let estimatedDelivery: String?
    public let weight: String?
    public let notes: String?
    public let createdAt: String?
    public let updatedAt: String?
}

// MARK: - Currency

public struct ExchangeRate: Codable, Identifiable, Sendable {
    public let id: String
    public let fromCurrency: String
    public let toCurrency: String
    public let rate: Double
    public let source: String?
    public let validFrom: String
    public let validTo: String?
    public let createdAt: String?
}

public struct ConversionResult: Codable, Sendable {
    public let fromCurrency: String
    public let toCurrency: String
    public let originalAmount: Double
    public let convertedAmount: Double
    public let rate: Double
    public let rateAt: String
}

public struct StoreCurrencySettings: Codable, Sendable {
    public let baseCurrency: String
    public let enabledCurrencies: [String]
    public let autoConvert: Bool
    public let roundingMode: String
}

// MARK: - Subscription

public struct SubscriptionPlan: Codable, Identifiable, Sendable {
    public let id: String
    public let code: String
    public let name: String
    public let interval: String
    public let intervalCount: Int
    public let price: Double
    public let currency: String
    public let status: String
    public let createdAt: String?
}

public struct Subscription: Codable, Identifiable, Sendable {
    public let id: String
    public let customerId: String
    public let planId: String
    public let status: String
    public let createdAt: String?
}

// MARK: - Promotion

public struct Promotion: Codable, Identifiable, Sendable {
    public let id: String
    public let code: String
    public let name: String
    public let discountType: String
    public let discountValue: Double
    public let isActive: Bool
    public let createdAt: String?
}

public struct Coupon: Codable, Identifiable, Sendable {
    public let id: String
    public let promotionId: String
    public let code: String
    public let maxUses: Int?
    public let usedCount: Int
    public let isActive: Bool
    public let createdAt: String?
}

// MARK: - Tax

public struct TaxCalculation: Codable, Sendable {
    public let subtotal: Double
    public let taxAmount: Double
    public let total: Double
    public let currency: String
}

public struct TaxJurisdiction: Codable, Identifiable, Sendable {
    public let id: String
    public let name: String
    public let code: String
    public let countryCode: String
    public let stateCode: String?
}

public struct TaxRate: Codable, Identifiable, Sendable {
    public let id: String
    public let jurisdictionId: String
    public let name: String
    public let rate: Double
}

public struct TaxExemption: Codable, Identifiable, Sendable {
    public let id: String
    public let customerId: String
    public let exemptionType: String
    public let effectiveFrom: String
}

public struct TaxSettings: Codable, Sendable {
    public let enabled: Bool
    public let defaultCountry: String
    public let pricesIncludeTax: Bool
}

// MARK: - Warehouse

public struct Warehouse: Codable, Identifiable, Sendable {
    public let id: Int
    public let code: String
    public let name: String
    public let warehouseType: String
    public let isActive: Bool
    public let createdAt: String?
}

public struct Location: Codable, Identifiable, Sendable {
    public let id: Int
    public let warehouseId: Int
    public let locationType: String
    public let zone: String?
    public let aisle: String?
}

// MARK: - General Ledger

public struct GlAccount: Codable, Identifiable, Sendable {
    public let id: String
    public let accountNumber: String
    public let name: String
    public let accountType: String
    public let isActive: Bool
    public let createdAt: String?
}

// MARK: - Enums

public enum OrderStatus: String, Codable, Sendable {
    case pending = "pending"
    case confirmed = "confirmed"
    case processing = "processing"
    case shipped = "shipped"
    case delivered = "delivered"
    case cancelled = "cancelled"
    case refunded = "refunded"
}

public enum ReturnReason: String, Codable, Sendable {
    case defective = "defective"
    case wrongItem = "wrong_item"
    case notAsDescribed = "not_as_described"
    case changedMind = "changed_mind"
    case arrivedLate = "arrived_late"
    case other = "other"
}

public enum ReturnStatus: String, Codable, Sendable {
    case requested = "requested"
    case approved = "approved"
    case rejected = "rejected"
    case inTransit = "in_transit"
    case received = "received"
    case completed = "completed"
    case cancelled = "cancelled"
}

public enum PaymentMethod: String, Codable, Sendable {
    case creditCard = "credit_card"
    case debitCard = "debit_card"
    case bankTransfer = "bank_transfer"
    case paypal = "paypal"
    case stripe = "stripe"
    case crypto = "crypto"
    case cash = "cash"
    case other = "other"
}

public enum TimePeriod: String, Sendable {
    case today = "today"
    case thisWeek = "week"
    case thisMonth = "month"
    case thisQuarter = "quarter"
    case thisYear = "year"
    case allTime = "all"
}

public enum ShipmentStatus: String, Codable, Sendable {
    case pending = "pending"
    case processing = "processing"
    case ready = "ready"
    case shipped = "shipped"
    case inTransit = "in_transit"
    case outForDelivery = "out_for_delivery"
    case delivered = "delivered"
    case failed = "failed"
    case cancelled = "cancelled"
}

public enum ShippingCarrier: String, Codable, Sendable {
    case ups = "ups"
    case fedex = "fedex"
    case usps = "usps"
    case dhl = "dhl"
    case other = "other"
}

public enum Currency: String, CaseIterable, Codable, Sendable {
    case usd = "USD"
    case eur = "EUR"
    case gbp = "GBP"
    case jpy = "JPY"
    case cad = "CAD"
    case aud = "AUD"
    case chf = "CHF"
    case cny = "CNY"
}

public enum RefundStatus: String, Codable, Sendable {
    case pending = "pending"
    case completed = "completed"
    case failed = "failed"
}

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

    enum CodingKeys: String, CodingKey {
        case sku, name, quantity
        case unitPrice = "unit_price"
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
    public let available: String
    public let reserved: String
    public let incoming: String?
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

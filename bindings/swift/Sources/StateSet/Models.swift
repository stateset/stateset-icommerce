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

// MARK: - Warranty

public struct Warranty: Codable, Identifiable, Sendable {
    public let id: String
    public let warrantyNumber: String
    public let customerId: String
    public let productId: String?
    public let orderId: String?
    public let orderItemId: String?
    public let serialNumber: String?
    public let status: String
    public let warrantyType: String
    public let durationMonths: Int
    public let coverageDescription: String?
    public let startDate: String
    public let endDate: String
    public let purchaseDate: String?
    public let notes: String?
    public let createdAt: String?
    public let updatedAt: String?
}

public struct WarrantyClaim: Codable, Identifiable, Sendable {
    public let id: String
    public let claimNumber: String
    public let warrantyId: String
    public let status: String
    public let issueDescription: String
    public let resolution: String?
    public let resolutionNotes: String?
    public let contactEmail: String?
    public let contactPhone: String?
    public let denialReason: String?
    public let resolvedAt: String?
    public let createdAt: String?
    public let updatedAt: String?
}

public enum WarrantyType: String, Codable, Sendable {
    case standard = "standard"
    case extended = "extended"
    case limited = "limited"
    case lifetime = "lifetime"
}

public enum WarrantyStatus: String, Codable, Sendable {
    case active = "active"
    case expired = "expired"
    case voided = "voided"
}

public enum ClaimStatus: String, Codable, Sendable {
    case pending = "pending"
    case approved = "approved"
    case denied = "denied"
    case completed = "completed"
    case cancelled = "cancelled"
}

public enum ClaimResolution: String, Codable, Sendable {
    case repair = "repair"
    case replacement = "replacement"
    case refund = "refund"
    case storeCredit = "store_credit"
}

// MARK: - Supplier

public struct Supplier: Codable, Identifiable, Sendable {
    public let id: String
    public let supplierCode: String?
    public let name: String
    public let email: String?
    public let phone: String?
    public let address: String?
    public let contactName: String?
    public let paymentTerms: String?
    public let leadTimeDays: Int?
    public let isActive: Bool?
    public let notes: String?
    public let createdAt: String?
    public let updatedAt: String?
}

// MARK: - Purchase Order

public struct PurchaseOrder: Codable, Identifiable, Sendable {
    public let id: String
    public let poNumber: String
    public let supplierId: String
    public let status: String
    public let subtotal: String
    public let taxAmount: String
    public let shippingCost: String
    public let total: String
    public let currency: String
    public let shipToAddress: String?
    public let expectedDate: String?
    public let receivedDate: String?
    public let approvedBy: String?
    public let approvedAt: String?
    public let supplierReference: String?
    public let notes: String?
    public let createdAt: String?
    public let updatedAt: String?
}

public struct PurchaseOrderItem: Codable, Sendable {
    public let sku: String
    public let name: String
    public let quantity: Double
    public let unitCost: Double

    public init(sku: String, name: String, quantity: Double, unitCost: Double) {
        self.sku = sku
        self.name = name
        self.quantity = quantity
        self.unitCost = unitCost
    }

    enum CodingKeys: String, CodingKey {
        case sku, name, quantity
        case unitCost = "unit_cost"
    }
}

public enum PurchaseOrderStatus: String, Codable, Sendable {
    case draft = "draft"
    case pendingApproval = "pending_approval"
    case approved = "approved"
    case sent = "sent"
    case acknowledged = "acknowledged"
    case partiallyReceived = "partially_received"
    case received = "received"
    case completed = "completed"
    case cancelled = "cancelled"
    case onHold = "on_hold"
}

// MARK: - Invoice

public struct Invoice: Codable, Identifiable, Sendable {
    public let id: String
    public let invoiceNumber: String
    public let customerId: String
    public let orderId: String?
    public let status: String
    public let invoiceType: String
    public let subtotal: String
    public let taxAmount: String
    public let total: String
    public let amountPaid: String
    public let currency: String
    public let billingEmail: String?
    public let billingName: String?
    public let billingAddress: String?
    public let dueDate: String?
    public let sentAt: String?
    public let viewedAt: String?
    public let paidAt: String?
    public let notes: String?
    public let createdAt: String?
    public let updatedAt: String?
}

public struct InvoiceItem: Codable, Sendable {
    public let description: String
    public let quantity: Double
    public let unitPrice: Double
    public let sku: String?

    public init(description: String, quantity: Double, unitPrice: Double, sku: String? = nil) {
        self.description = description
        self.quantity = quantity
        self.unitPrice = unitPrice
        self.sku = sku
    }

    enum CodingKeys: String, CodingKey {
        case description, quantity, sku
        case unitPrice = "unit_price"
    }
}

public enum InvoiceStatus: String, Codable, Sendable {
    case draft = "draft"
    case sent = "sent"
    case viewed = "viewed"
    case partiallyPaid = "partially_paid"
    case paid = "paid"
    case overdue = "overdue"
    case voided = "voided"
    case writtenOff = "written_off"
    case disputed = "disputed"
}

// MARK: - Bill of Materials

public struct BillOfMaterials: Codable, Identifiable, Sendable {
    public let id: String
    public let bomNumber: String
    public let productId: String
    public let name: String
    public let description: String?
    public let version: String
    public let status: String
    public let notes: String?
    public let createdAt: String?
    public let updatedAt: String?
}

public struct BOMComponent: Codable, Identifiable, Sendable {
    public let id: String
    public let bomId: String
    public let componentSku: String?
    public let name: String
    public let description: String?
    public let quantity: String
    public let unitOfMeasure: String?
    public let position: String?
    public let isOptional: Bool?
    public let notes: String?
}

public enum BOMStatus: String, Codable, Sendable {
    case draft = "draft"
    case active = "active"
    case obsolete = "obsolete"
}

// MARK: - Work Order

public struct WorkOrder: Codable, Identifiable, Sendable {
    public let id: String
    public let workOrderNumber: String
    public let productId: String
    public let bomId: String?
    public let status: String
    public let priority: String
    public let quantityToBuild: String
    public let quantityCompleted: String
    public let plannedStart: String?
    public let plannedEnd: String?
    public let actualStart: String?
    public let actualEnd: String?
    public let notes: String?
    public let createdAt: String?
    public let updatedAt: String?
}

public enum WorkOrderStatus: String, Codable, Sendable {
    case planned = "planned"
    case inProgress = "in_progress"
    case onHold = "on_hold"
    case completed = "completed"
    case partiallyCompleted = "partially_completed"
    case cancelled = "cancelled"
}

public enum WorkOrderPriority: String, Codable, Sendable {
    case low = "low"
    case normal = "normal"
    case high = "high"
    case urgent = "urgent"
}

// MARK: - Currency

public struct ExchangeRate: Codable, Identifiable, Sendable {
    public let id: String
    public let baseCurrency: String
    public let quoteCurrency: String
    public let rate: String
    public let source: String?
    public let validFrom: String
    public let validTo: String?
    public let createdAt: String?
}

public struct ConversionResult: Codable, Sendable {
    public let fromCurrency: String
    public let toCurrency: String
    public let originalAmount: String
    public let convertedAmount: String
    public let rate: String
    public let rateAt: String
}

public struct StoreCurrencySettings: Codable, Sendable {
    public let baseCurrency: String
    public let enabledCurrencies: [String]
    public let autoConvert: Bool
    public let roundingMode: String
}

public enum Currency: String, Codable, Sendable {
    case usd = "USD"
    case eur = "EUR"
    case gbp = "GBP"
    case jpy = "JPY"
    case cad = "CAD"
    case aud = "AUD"
    case chf = "CHF"
    case cny = "CNY"
}

// MARK: - Refund

public struct Refund: Codable, Identifiable, Sendable {
    public let id: String
    public let refundNumber: String
    public let paymentId: String
    public let amount: String
    public let currency: String
    public let status: String
    public let reason: String?
    public let externalId: String?
    public let failureReason: String?
    public let refundedAt: String?
    public let createdAt: String?
}

public enum RefundStatus: String, Codable, Sendable {
    case pending = "pending"
    case completed = "completed"
    case failed = "failed"
}

// MARK: - Return Status

public enum ReturnStatus: String, Codable, Sendable {
    case requested = "requested"
    case approved = "approved"
    case rejected = "rejected"
    case inTransit = "in_transit"
    case received = "received"
    case completed = "completed"
    case cancelled = "cancelled"
}

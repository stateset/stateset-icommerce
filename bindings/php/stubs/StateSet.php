<?php
/**
 * StateSet Embedded Commerce - PHP Stubs
 *
 * This file provides IDE autocompletion for the native PHP extension.
 * Do not include this file at runtime - the extension provides the actual classes.
 *
 * @package StateSet\Embedded
 * @version 0.9.9
 */

namespace StateSet;

/**
 * Main Commerce instance for local commerce operations.
 */
class Commerce
{
    /**
     * Create a new Commerce instance.
     *
     * @param string $dbPath Path to SQLite database (use ':memory:' for in-memory)
     * @throws \RuntimeException If database initialization fails
     */
    public function __construct(string $dbPath) {}

    /** @return Customers */
    public function customers(): Customers {}

    /** @return Orders */
    public function orders(): Orders {}

    /** @return Products */
    public function products(): Products {}

    /** @return Inventory */
    public function inventory(): Inventory {}

    /** @return Returns */
    public function returns(): Returns {}

    /** @return Payments */
    public function payments(): Payments {}

    /** @return Shipments */
    public function shipments(): Shipments {}

    /** @return Warranties */
    public function warranties(): Warranties {}

    /** @return PurchaseOrders */
    public function purchase_orders(): PurchaseOrders {}

    /** @return Invoices */
    public function invoices(): Invoices {}

    /** @return BomApi */
    public function bom(): BomApi {}

    /** @return WorkOrders */
    public function work_orders(): WorkOrders {}

    /** @return Carts */
    public function carts(): Carts {}

    /** @return Analytics */
    public function analytics(): Analytics {}

    /** @return CurrencyOps */
    public function currency(): CurrencyOps {}

    /** @return Subscriptions */
    public function subscriptions(): Subscriptions {}

    /** @return Promotions */
    public function promotions(): Promotions {}

    /** @return Tax */
    public function tax(): Tax {}
}

/**
 * Customer entity.
 */
class Customer
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string */
    public function getEmail(): string {}

    /** @return string */
    public function getFirstName(): string {}

    /** @return string */
    public function getLastName(): string {}

    /** @return string|null */
    public function getPhone(): ?string {}

    /** @return string */
    public function getStatus(): string {}

    /** @return bool */
    public function getAcceptsMarketing(): bool {}

    /** @return string ISO 8601 datetime */
    public function getCreatedAt(): string {}

    /** @return string ISO 8601 datetime */
    public function getUpdatedAt(): string {}

    /** @return string Full name (first + last) */
    public function getFullName(): string {}

    /** @return string */
    public function __toString(): string {}
}

/**
 * Customers API for managing customer records.
 */
class Customers
{
    /**
     * Create a new customer.
     *
     * @param string $email Customer email address
     * @param string $firstName First name
     * @param string $lastName Last name
     * @param string|null $phone Phone number
     * @param bool|null $acceptsMarketing Marketing opt-in
     * @return Customer
     */
    public function create(
        string $email,
        string $firstName,
        string $lastName,
        ?string $phone = null,
        ?bool $acceptsMarketing = null
    ): Customer {}

    /**
     * Get a customer by ID.
     *
     * @param string $id Customer UUID
     * @return Customer|null
     */
    public function get(string $id): ?Customer {}

    /**
     * Get a customer by email.
     *
     * @param string $email Customer email
     * @return Customer|null
     */
    public function getByEmail(string $email): ?Customer {}

    /**
     * List all customers.
     *
     * @return Customer[]
     */
    public function list(): array {}

    /**
     * Count customers.
     *
     * @return int
     */
    public function count(): int {}
}

/**
 * Order line item.
 */
class OrderItem
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string */
    public function getSku(): string {}

    /** @return string */
    public function getName(): string {}

    /** @return int */
    public function getQuantity(): int {}

    /** @return float */
    public function getUnitPrice(): float {}

    /** @return float */
    public function getTotal(): float {}

    /** @return string */
    public function __toString(): string {}
}

/**
 * Order entity.
 */
class Order
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string Human-readable order number */
    public function getOrderNumber(): string {}

    /** @return string Customer UUID */
    public function getCustomerId(): string {}

    /** @return string Order status */
    public function getStatus(): string {}

    /** @return float */
    public function getTotalAmount(): float {}

    /** @return string Currency code (e.g., 'USD') */
    public function getCurrency(): string {}

    /** @return string Payment status */
    public function getPaymentStatus(): string {}

    /** @return string Fulfillment status */
    public function getFulfillmentStatus(): string {}

    /** @return string|null Shipping tracking number */
    public function getTrackingNumber(): ?string {}

    /** @return OrderItem[] */
    public function getItems(): array {}

    /** @return int Version for optimistic locking */
    public function getVersion(): int {}

    /** @return string ISO 8601 datetime */
    public function getCreatedAt(): string {}

    /** @return string ISO 8601 datetime */
    public function getUpdatedAt(): string {}

    /** @return int Number of line items */
    public function getItemCount(): int {}

    /** @return string */
    public function __toString(): string {}
}

/**
 * Orders API for managing orders.
 */
class Orders
{
    /**
     * Create a new order.
     *
     * @param string $customerId Customer UUID
     * @param array $items Array of items: [['sku' => 'SKU', 'name' => 'Name', 'quantity' => 1, 'unit_price' => 9.99], ...]
     * @param string|null $currency Currency code (default: 'USD')
     * @param string|null $notes Order notes
     * @return Order
     */
    public function create(
        string $customerId,
        array $items,
        ?string $currency = null,
        ?string $notes = null
    ): Order {}

    /**
     * Get an order by ID.
     *
     * @param string $id Order UUID
     * @return Order|null
     */
    public function get(string $id): ?Order {}

    /**
     * List all orders.
     *
     * @return Order[]
     */
    public function list(): array {}

    /**
     * Count orders.
     *
     * @return int
     */
    public function count(): int {}

    /**
     * Mark order as shipped.
     *
     * @param string $id Order UUID
     * @param string|null $trackingNumber Tracking number
     * @param string|null $carrier Shipping carrier
     * @return Order
     */
    public function ship(string $id, ?string $trackingNumber = null, ?string $carrier = null): Order {}

    /**
     * Cancel an order.
     *
     * @param string $id Order UUID
     * @param string|null $reason Cancellation reason
     * @return Order
     */
    public function cancel(string $id, ?string $reason = null): Order {}

    /**
     * Confirm an order.
     *
     * @param string $id Order UUID
     * @return Order
     */
    public function confirm(string $id): Order {}

    /**
     * Mark order as delivered.
     *
     * @param string $id Order UUID
     * @return Order
     */
    public function deliver(string $id): Order {}
}

/**
 * Product variant entity.
 */
class ProductVariant
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string */
    public function getSku(): string {}

    /** @return string */
    public function getName(): string {}

    /** @return float */
    public function getPrice(): float {}

    /** @return float|null */
    public function getCompareAtPrice(): ?float {}

    /** @return int */
    public function getInventoryQuantity(): int {}

    /** @return float|null */
    public function getWeight(): ?float {}

    /** @return string|null */
    public function getBarcode(): ?string {}
}

/**
 * Product entity.
 */
class Product
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string */
    public function getName(): string {}

    /** @return string|null */
    public function getDescription(): ?string {}

    /** @return string|null */
    public function getVendor(): ?string {}

    /** @return string|null */
    public function getProductType(): ?string {}

    /** @return string */
    public function getStatus(): string {}

    /** @return string[] */
    public function getTags(): array {}

    /** @return ProductVariant[] */
    public function getVariants(): array {}

    /** @return string ISO 8601 datetime */
    public function getCreatedAt(): string {}

    /** @return string ISO 8601 datetime */
    public function getUpdatedAt(): string {}

    /** @return string */
    public function __toString(): string {}
}

/**
 * Products API for managing products.
 */
class Products
{
    /**
     * Create a new product.
     *
     * @param string $name Product name
     * @param string|null $description Product description
     * @param string|null $vendor Vendor name
     * @param string|null $productType Product type/category
     * @return Product
     */
    public function create(
        string $name,
        ?string $description = null,
        ?string $vendor = null,
        ?string $productType = null
    ): Product {}

    /**
     * Get a product by ID.
     *
     * @param string $id Product UUID
     * @return Product|null
     */
    public function get(string $id): ?Product {}

    /**
     * List all products.
     *
     * @return Product[]
     */
    public function list(): array {}

    /**
     * Count products.
     *
     * @return int
     */
    public function count(): int {}

    /**
     * Get a product variant by SKU.
     *
     * @param string $sku SKU code
     * @return ProductVariant|null
     */
    public function getBySku(string $sku): ?ProductVariant {}
}

/**
 * Inventory item entity.
 */
class InventoryItem
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string */
    public function getSku(): string {}

    /** @return int */
    public function getQuantityOnHand(): int {}

    /** @return int */
    public function getQuantityReserved(): int {}

    /** @return int */
    public function getQuantityAvailable(): int {}

    /** @return int|null */
    public function getReorderPoint(): ?int {}

    /** @return int|null */
    public function getReorderQuantity(): ?int {}

    /** @return string|null */
    public function getLocationId(): ?string {}

    /** @return string */
    public function __toString(): string {}
}

/**
 * Inventory API for managing inventory.
 */
class Inventory
{
    /**
     * Create a new inventory item.
     *
     * @param string $sku SKU code
     * @param int $quantity Initial quantity
     * @param int|null $reorderPoint Low stock threshold
     * @param int|null $reorderQuantity Reorder quantity
     * @return InventoryItem
     */
    public function create(
        string $sku,
        int $quantity,
        ?int $reorderPoint = null,
        ?int $reorderQuantity = null
    ): InventoryItem {}

    /**
     * Get an inventory item by ID.
     *
     * @param string $id Inventory item UUID
     * @return InventoryItem|null
     */
    public function get(string $id): ?InventoryItem {}

    /**
     * Get an inventory item by SKU.
     *
     * @param string $sku SKU code
     * @return InventoryItem|null
     */
    public function getBySku(string $sku): ?InventoryItem {}

    /**
     * List all inventory items.
     *
     * @return InventoryItem[]
     */
    public function list(): array {}

    /**
     * Adjust inventory quantity.
     *
     * @param string $id Inventory item UUID
     * @param int $adjustment Quantity change (+/-)
     * @param string|null $reason Adjustment reason
     * @return InventoryItem
     */
    public function adjust(string $id, int $adjustment, ?string $reason = null): InventoryItem {}

    /**
     * Reserve inventory for an order.
     *
     * @param string $id Inventory item UUID
     * @param int $quantity Quantity to reserve
     * @param string|null $orderId Order UUID
     * @return InventoryItem
     */
    public function reserve(string $id, int $quantity, ?string $orderId = null): InventoryItem {}

    /**
     * Release reserved inventory.
     *
     * @param string $id Inventory item UUID
     * @param int $quantity Quantity to release
     * @return InventoryItem
     */
    public function release(string $id, int $quantity): InventoryItem {}
}

/**
 * Return request entity.
 */
class ReturnRequest
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string Order UUID */
    public function getOrderId(): string {}

    /** @return string Customer UUID */
    public function getCustomerId(): string {}

    /** @return string Return status */
    public function getStatus(): string {}

    /** @return string Return reason */
    public function getReason(): string {}

    /** @return float */
    public function getRefundAmount(): float {}

    /** @return string ISO 8601 datetime */
    public function getCreatedAt(): string {}

    /** @return string ISO 8601 datetime */
    public function getUpdatedAt(): string {}

    /** @return string */
    public function __toString(): string {}
}

/**
 * Returns API for managing return requests.
 */
class Returns
{
    /**
     * Create a return request.
     *
     * @param string $orderId Order UUID
     * @param string $reason Return reason
     * @return ReturnRequest
     */
    public function create(string $orderId, string $reason): ReturnRequest {}

    /**
     * Get a return request by ID.
     *
     * @param string $id Return request UUID
     * @return ReturnRequest|null
     */
    public function get(string $id): ?ReturnRequest {}

    /**
     * List all return requests.
     *
     * @return ReturnRequest[]
     */
    public function list(): array {}

    /**
     * Approve a return request.
     *
     * @param string $id Return request UUID
     * @param float|null $refundAmount Override refund amount
     * @return ReturnRequest
     */
    public function approve(string $id, ?float $refundAmount = null): ReturnRequest {}

    /**
     * Reject a return request.
     *
     * @param string $id Return request UUID
     * @param string|null $reason Rejection reason
     * @return ReturnRequest
     */
    public function reject(string $id, ?string $reason = null): ReturnRequest {}
}

/**
 * Payments API for recording payments.
 */
class Payments
{
    /**
     * Record a payment for an order.
     *
     * @param string $orderId Order UUID
     * @param float $amount Payment amount
     * @param string|null $method Payment method
     * @return bool
     */
    public function record(string $orderId, float $amount, ?string $method = null): bool {}
}

/**
 * Shopping cart item entity.
 */
class CartItem
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string */
    public function getSku(): string {}

    /** @return string */
    public function getName(): string {}

    /** @return int */
    public function getQuantity(): int {}

    /** @return float */
    public function getUnitPrice(): float {}

    /** @return float */
    public function getTotal(): float {}
}

/**
 * Shopping cart entity.
 */
class Cart
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string|null Customer UUID */
    public function getCustomerId(): ?string {}

    /** @return string Cart status */
    public function getStatus(): string {}

    /** @return CartItem[] */
    public function getItems(): array {}

    /** @return float */
    public function getSubtotal(): float {}

    /** @return float */
    public function getTotal(): float {}

    /** @return string Currency code */
    public function getCurrency(): string {}

    /** @return string ISO 8601 datetime */
    public function getCreatedAt(): string {}

    /** @return string ISO 8601 datetime */
    public function getUpdatedAt(): string {}

    /** @return string */
    public function __toString(): string {}
}

/**
 * Carts API for shopping cart management.
 */
class Carts
{
    /**
     * Create a new cart.
     *
     * @param string|null $customerId Customer UUID (optional for guest carts)
     * @param string|null $currency Currency code (default: 'USD')
     * @return Cart
     */
    public function create(?string $customerId = null, ?string $currency = null): Cart {}

    /**
     * Get a cart by ID.
     *
     * @param string $id Cart UUID
     * @return Cart|null
     */
    public function get(string $id): ?Cart {}

    /**
     * List all carts.
     *
     * @return Cart[]
     */
    public function list(): array {}

    /**
     * Add an item to a cart.
     *
     * @param string $cartId Cart UUID
     * @param string $sku SKU code
     * @param string $name Item name
     * @param int $quantity Quantity
     * @param float $unitPrice Price per unit
     * @return Cart
     */
    public function addItem(
        string $cartId,
        string $sku,
        string $name,
        int $quantity,
        float $unitPrice
    ): Cart {}

    /**
     * Checkout a cart, creating an order.
     *
     * @param string $cartId Cart UUID
     * @return Order
     */
    public function checkout(string $cartId): Order {}
}

/**
 * Sales summary statistics.
 */
class SalesSummary
{
    /** @return float */
    public function getTotalRevenue(): float {}

    /** @return int */
    public function getTotalOrders(): int {}

    /** @return float */
    public function getAverageOrderValue(): float {}

    /** @return int */
    public function getTotalItemsSold(): int {}
}

/**
 * Analytics API for sales reports.
 */
class Analytics
{
    /**
     * Get sales summary for a time period.
     *
     * @param int|null $days Number of days (default: 30)
     * @return SalesSummary
     */
    public function salesSummary(?int $days = null): SalesSummary {}
}

// ============================================================================
// Shipments API
// ============================================================================

/**
 * Shipment entity.
 */
class Shipment
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string Order UUID */
    public function getOrderId(): string {}

    /** @return string|null */
    public function getTrackingNumber(): ?string {}

    /** @return string|null */
    public function getCarrier(): ?string {}

    /** @return string */
    public function getStatus(): string {}

    /** @return string|null ISO 8601 datetime */
    public function getShippedAt(): ?string {}

    /** @return string|null ISO 8601 datetime */
    public function getDeliveredAt(): ?string {}

    /** @return string ISO 8601 datetime */
    public function getCreatedAt(): string {}

    /** @return string ISO 8601 datetime */
    public function getUpdatedAt(): string {}

    /** @return string */
    public function __toString(): string {}
}

/**
 * Shipments API for managing shipments.
 */
class Shipments
{
    /**
     * Create a new shipment.
     *
     * @param string $orderId Order UUID
     * @param string|null $trackingNumber Tracking number
     * @param string|null $carrier Shipping carrier
     * @return Shipment
     */
    public function create(string $orderId, ?string $trackingNumber = null, ?string $carrier = null): Shipment {}

    /**
     * Get a shipment by ID.
     *
     * @param string $id Shipment UUID
     * @return Shipment|null
     */
    public function get(string $id): ?Shipment {}

    /**
     * Get a shipment by tracking number.
     *
     * @param string $trackingNumber Tracking number
     * @return Shipment|null
     */
    public function getByTracking(string $trackingNumber): ?Shipment {}

    /**
     * List all shipments.
     *
     * @return Shipment[]
     */
    public function list(): array {}

    /**
     * Get shipments for an order.
     *
     * @param string $orderId Order UUID
     * @return Shipment[]
     */
    public function forOrder(string $orderId): array {}

    /**
     * Mark shipment as shipped.
     *
     * @param string $id Shipment UUID
     * @return Shipment
     */
    public function ship(string $id): Shipment {}

    /**
     * Mark shipment as delivered.
     *
     * @param string $id Shipment UUID
     * @return Shipment
     */
    public function markDelivered(string $id): Shipment {}

    /**
     * Cancel a shipment.
     *
     * @param string $id Shipment UUID
     * @return Shipment
     */
    public function cancel(string $id): Shipment {}

    /**
     * Count shipments.
     *
     * @return int
     */
    public function count(): int {}
}

// ============================================================================
// Warranties API
// ============================================================================

/**
 * Warranty entity.
 */
class Warranty
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string Product UUID */
    public function getProductId(): string {}

    /** @return string|null Order UUID */
    public function getOrderId(): ?string {}

    /** @return string Customer UUID */
    public function getCustomerId(): string {}

    /** @return string */
    public function getWarrantyType(): string {}

    /** @return string */
    public function getStatus(): string {}

    /** @return string ISO 8601 datetime */
    public function getStartDate(): string {}

    /** @return string ISO 8601 datetime */
    public function getEndDate(): string {}

    /** @return string ISO 8601 datetime */
    public function getCreatedAt(): string {}

    /** @return string */
    public function __toString(): string {}
}

/**
 * Warranty claim entity.
 */
class WarrantyClaim
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string Warranty UUID */
    public function getWarrantyId(): string {}

    /** @return string */
    public function getDescription(): string {}

    /** @return string */
    public function getStatus(): string {}

    /** @return string|null */
    public function getResolution(): ?string {}

    /** @return string ISO 8601 datetime */
    public function getCreatedAt(): string {}
}

/**
 * Warranties API for managing warranties and claims.
 */
class Warranties
{
    /**
     * Create a new warranty.
     *
     * @param string $productId Product UUID
     * @param string $customerId Customer UUID
     * @param string $warrantyType Warranty type
     * @param int $durationMonths Duration in months
     * @return Warranty
     */
    public function create(string $productId, string $customerId, string $warrantyType, int $durationMonths): Warranty {}

    /**
     * Get a warranty by ID.
     *
     * @param string $id Warranty UUID
     * @return Warranty|null
     */
    public function get(string $id): ?Warranty {}

    /**
     * List all warranties.
     *
     * @return Warranty[]
     */
    public function list(): array {}

    /**
     * Get warranties for a customer.
     *
     * @param string $customerId Customer UUID
     * @return Warranty[]
     */
    public function forCustomer(string $customerId): array {}

    /**
     * Check if a warranty is valid.
     *
     * @param string $id Warranty UUID
     * @return bool
     */
    public function isValid(string $id): bool {}

    /**
     * Create a warranty claim.
     *
     * @param string $warrantyId Warranty UUID
     * @param string $description Claim description
     * @return WarrantyClaim
     */
    public function createClaim(string $warrantyId, string $description): WarrantyClaim {}

    /**
     * Approve a warranty claim.
     *
     * @param string $claimId Claim UUID
     * @param string|null $resolution Resolution details
     * @return WarrantyClaim
     */
    public function approveClaim(string $claimId, ?string $resolution = null): WarrantyClaim {}

    /**
     * Deny a warranty claim.
     *
     * @param string $claimId Claim UUID
     * @param string|null $reason Denial reason
     * @return WarrantyClaim
     */
    public function denyClaim(string $claimId, ?string $reason = null): WarrantyClaim {}

    /**
     * Count warranties.
     *
     * @return int
     */
    public function count(): int {}
}

// ============================================================================
// PurchaseOrders API
// ============================================================================

/**
 * Supplier entity.
 */
class Supplier
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string */
    public function getName(): string {}

    /** @return string|null */
    public function getEmail(): ?string {}

    /** @return string|null */
    public function getPhone(): ?string {}

    /** @return string */
    public function getStatus(): string {}

    /** @return string ISO 8601 datetime */
    public function getCreatedAt(): string {}
}

/**
 * Purchase order entity.
 */
class PurchaseOrder
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string PO number */
    public function getPoNumber(): string {}

    /** @return string Supplier UUID */
    public function getSupplierId(): string {}

    /** @return string */
    public function getStatus(): string {}

    /** @return float */
    public function getTotalAmount(): float {}

    /** @return string Currency code */
    public function getCurrency(): string {}

    /** @return string|null ISO 8601 datetime */
    public function getExpectedDate(): ?string {}

    /** @return string ISO 8601 datetime */
    public function getCreatedAt(): string {}

    /** @return string ISO 8601 datetime */
    public function getUpdatedAt(): string {}

    /** @return string */
    public function __toString(): string {}
}

/**
 * PurchaseOrders API for managing suppliers and purchase orders.
 */
class PurchaseOrders
{
    /**
     * Create a new supplier.
     *
     * @param string $name Supplier name
     * @param string|null $email Email address
     * @param string|null $phone Phone number
     * @return Supplier
     */
    public function createSupplier(string $name, ?string $email = null, ?string $phone = null): Supplier {}

    /**
     * Get a supplier by ID.
     *
     * @param string $id Supplier UUID
     * @return Supplier|null
     */
    public function getSupplier(string $id): ?Supplier {}

    /**
     * List all suppliers.
     *
     * @return Supplier[]
     */
    public function listSuppliers(): array {}

    /**
     * Create a new purchase order.
     *
     * @param string $supplierId Supplier UUID
     * @param string|null $currency Currency code
     * @return PurchaseOrder
     */
    public function create(string $supplierId, ?string $currency = null): PurchaseOrder {}

    /**
     * Get a purchase order by ID.
     *
     * @param string $id PO UUID
     * @return PurchaseOrder|null
     */
    public function get(string $id): ?PurchaseOrder {}

    /**
     * List all purchase orders.
     *
     * @return PurchaseOrder[]
     */
    public function list(): array {}

    /**
     * Submit a purchase order.
     *
     * @param string $id PO UUID
     * @return PurchaseOrder
     */
    public function submit(string $id): PurchaseOrder {}

    /**
     * Approve a purchase order.
     *
     * @param string $id PO UUID
     * @return PurchaseOrder
     */
    public function approve(string $id): PurchaseOrder {}

    /**
     * Cancel a purchase order.
     *
     * @param string $id PO UUID
     * @return PurchaseOrder
     */
    public function cancel(string $id): PurchaseOrder {}

    /**
     * Complete a purchase order.
     *
     * @param string $id PO UUID
     * @return PurchaseOrder
     */
    public function complete(string $id): PurchaseOrder {}

    /**
     * Count purchase orders.
     *
     * @return int
     */
    public function count(): int {}
}

// ============================================================================
// Invoices API
// ============================================================================

/**
 * Invoice entity.
 */
class Invoice
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string Invoice number */
    public function getInvoiceNumber(): string {}

    /** @return string Customer UUID */
    public function getCustomerId(): string {}

    /** @return string|null Order UUID */
    public function getOrderId(): ?string {}

    /** @return string */
    public function getStatus(): string {}

    /** @return float */
    public function getSubtotal(): float {}

    /** @return float */
    public function getTax(): float {}

    /** @return float */
    public function getTotal(): float {}

    /** @return string Currency code */
    public function getCurrency(): string {}

    /** @return string|null ISO 8601 datetime */
    public function getDueDate(): ?string {}

    /** @return string|null ISO 8601 datetime */
    public function getPaidAt(): ?string {}

    /** @return string ISO 8601 datetime */
    public function getCreatedAt(): string {}

    /** @return string ISO 8601 datetime */
    public function getUpdatedAt(): string {}

    /** @return string */
    public function __toString(): string {}
}

/**
 * Invoices API for managing invoices.
 */
class Invoices
{
    /**
     * Create a new invoice.
     *
     * @param string $customerId Customer UUID
     * @param string|null $orderId Order UUID
     * @param int|null $dueDays Days until due
     * @return Invoice
     */
    public function create(string $customerId, ?string $orderId = null, ?int $dueDays = null): Invoice {}

    /**
     * Get an invoice by ID.
     *
     * @param string $id Invoice UUID
     * @return Invoice|null
     */
    public function get(string $id): ?Invoice {}

    /**
     * List all invoices.
     *
     * @return Invoice[]
     */
    public function list(): array {}

    /**
     * Get invoices for a customer.
     *
     * @param string $customerId Customer UUID
     * @return Invoice[]
     */
    public function forCustomer(string $customerId): array {}

    /**
     * Send an invoice.
     *
     * @param string $id Invoice UUID
     * @return Invoice
     */
    public function send(string $id): Invoice {}

    /**
     * Record a payment on an invoice.
     *
     * @param string $id Invoice UUID
     * @param float $amount Payment amount
     * @return Invoice
     */
    public function recordPayment(string $id, float $amount): Invoice {}

    /**
     * Void an invoice.
     *
     * @param string $id Invoice UUID
     * @return Invoice
     */
    public function void(string $id): Invoice {}

    /**
     * Get overdue invoices.
     *
     * @return Invoice[]
     */
    public function getOverdue(): array {}

    /**
     * Get customer's outstanding balance.
     *
     * @param string $customerId Customer UUID
     * @return float
     */
    public function customerBalance(string $customerId): float {}

    /**
     * Count invoices.
     *
     * @return int
     */
    public function count(): int {}
}

// ============================================================================
// BOM API
// ============================================================================

/**
 * BOM component entity.
 */
class BomComponent
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string BOM UUID */
    public function getBomId(): string {}

    /** @return string */
    public function getComponentSku(): string {}

    /** @return int */
    public function getQuantity(): int {}

    /** @return float */
    public function getUnitCost(): float {}
}

/**
 * Bill of Materials entity.
 */
class BillOfMaterials
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string Product UUID */
    public function getProductId(): string {}

    /** @return string */
    public function getName(): string {}

    /** @return string */
    public function getVersion(): string {}

    /** @return string */
    public function getStatus(): string {}

    /** @return float */
    public function getTotalCost(): float {}

    /** @return string ISO 8601 datetime */
    public function getCreatedAt(): string {}

    /** @return string ISO 8601 datetime */
    public function getUpdatedAt(): string {}

    /** @return string */
    public function __toString(): string {}
}

/**
 * BOM API for managing bills of materials.
 */
class BomApi
{
    /**
     * Create a new BOM.
     *
     * @param string $productId Product UUID
     * @param string $name BOM name
     * @param string|null $version Version string
     * @return BillOfMaterials
     */
    public function create(string $productId, string $name, ?string $version = null): BillOfMaterials {}

    /**
     * Get a BOM by ID.
     *
     * @param string $id BOM UUID
     * @return BillOfMaterials|null
     */
    public function get(string $id): ?BillOfMaterials {}

    /**
     * List all BOMs.
     *
     * @return BillOfMaterials[]
     */
    public function list(): array {}

    /**
     * Add a component to a BOM.
     *
     * @param string $bomId BOM UUID
     * @param string $componentSku Component SKU
     * @param int $quantity Quantity required
     * @param float $unitCost Cost per unit
     * @return BomComponent
     */
    public function addComponent(string $bomId, string $componentSku, int $quantity, float $unitCost): BomComponent {}

    /**
     * Get components for a BOM.
     *
     * @param string $bomId BOM UUID
     * @return BomComponent[]
     */
    public function getComponents(string $bomId): array {}

    /**
     * Remove a component from a BOM.
     *
     * @param string $componentId Component UUID
     * @return bool
     */
    public function removeComponent(string $componentId): bool {}

    /**
     * Activate a BOM.
     *
     * @param string $id BOM UUID
     * @return BillOfMaterials
     */
    public function activate(string $id): BillOfMaterials {}

    /**
     * Delete a BOM.
     *
     * @param string $id BOM UUID
     * @return bool
     */
    public function delete(string $id): bool {}

    /**
     * Count BOMs.
     *
     * @return int
     */
    public function count(): int {}
}

// ============================================================================
// WorkOrders API
// ============================================================================

/**
 * Work order entity.
 */
class WorkOrder
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string Work order number */
    public function getWorkOrderNumber(): string {}

    /** @return string BOM UUID */
    public function getBomId(): string {}

    /** @return int */
    public function getQuantity(): int {}

    /** @return string */
    public function getStatus(): string {}

    /** @return string */
    public function getPriority(): string {}

    /** @return string|null ISO 8601 datetime */
    public function getStartedAt(): ?string {}

    /** @return string|null ISO 8601 datetime */
    public function getCompletedAt(): ?string {}

    /** @return string ISO 8601 datetime */
    public function getCreatedAt(): string {}

    /** @return string ISO 8601 datetime */
    public function getUpdatedAt(): string {}

    /** @return string */
    public function __toString(): string {}
}

/**
 * WorkOrders API for managing work orders.
 */
class WorkOrders
{
    /**
     * Create a new work order.
     *
     * @param string $bomId BOM UUID
     * @param int $quantity Quantity to produce
     * @param string|null $priority Priority level
     * @return WorkOrder
     */
    public function create(string $bomId, int $quantity, ?string $priority = null): WorkOrder {}

    /**
     * Get a work order by ID.
     *
     * @param string $id Work order UUID
     * @return WorkOrder|null
     */
    public function get(string $id): ?WorkOrder {}

    /**
     * List all work orders.
     *
     * @return WorkOrder[]
     */
    public function list(): array {}

    /**
     * Start a work order.
     *
     * @param string $id Work order UUID
     * @return WorkOrder
     */
    public function start(string $id): WorkOrder {}

    /**
     * Complete a work order.
     *
     * @param string $id Work order UUID
     * @return WorkOrder
     */
    public function complete(string $id): WorkOrder {}

    /**
     * Put a work order on hold.
     *
     * @param string $id Work order UUID
     * @return WorkOrder
     */
    public function hold(string $id): WorkOrder {}

    /**
     * Resume a work order.
     *
     * @param string $id Work order UUID
     * @return WorkOrder
     */
    public function resume(string $id): WorkOrder {}

    /**
     * Cancel a work order.
     *
     * @param string $id Work order UUID
     * @return WorkOrder
     */
    public function cancel(string $id): WorkOrder {}

    /**
     * Count work orders.
     *
     * @return int
     */
    public function count(): int {}
}

// ============================================================================
// CurrencyOps API
// ============================================================================

/**
 * Exchange rate entity.
 */
class ExchangeRate
{
    /** @return string */
    public function getFromCurrency(): string {}

    /** @return string */
    public function getToCurrency(): string {}

    /** @return float */
    public function getRate(): float {}

    /** @return string ISO 8601 datetime */
    public function getUpdatedAt(): string {}
}

/**
 * CurrencyOps API for currency operations.
 */
class CurrencyOps
{
    /**
     * Get exchange rate between currencies.
     *
     * @param string $from Source currency code
     * @param string $to Target currency code
     * @return ExchangeRate|null
     */
    public function getRate(string $from, string $to): ?ExchangeRate {}

    /**
     * List all exchange rates.
     *
     * @return ExchangeRate[]
     */
    public function listRates(): array {}

    /**
     * Set an exchange rate.
     *
     * @param string $from Source currency code
     * @param string $to Target currency code
     * @param float $rate Exchange rate
     * @return ExchangeRate
     */
    public function setRate(string $from, string $to, float $rate): ExchangeRate {}

    /**
     * Convert amount between currencies.
     *
     * @param float $amount Amount to convert
     * @param string $from Source currency code
     * @param string $to Target currency code
     * @return float
     */
    public function convert(float $amount, string $from, string $to): float {}

    /**
     * Get base currency.
     *
     * @return string
     */
    public function baseCurrency(): string {}

    /**
     * Get enabled currencies.
     *
     * @return string[]
     */
    public function enabledCurrencies(): array {}

    /**
     * Format amount in currency.
     *
     * @param float $amount Amount
     * @param string $currency Currency code
     * @return string
     */
    public function format(float $amount, string $currency): string {}
}

// ============================================================================
// Subscriptions API
// ============================================================================

/**
 * Subscription plan entity.
 */
class SubscriptionPlan
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string */
    public function getName(): string {}

    /** @return string|null */
    public function getDescription(): ?string {}

    /** @return float */
    public function getPrice(): float {}

    /** @return string Currency code */
    public function getCurrency(): string {}

    /** @return string Billing interval (day, week, month, year) */
    public function getInterval(): string {}

    /** @return int Interval count */
    public function getIntervalCount(): int {}

    /** @return int|null Trial period in days */
    public function getTrialDays(): ?int {}

    /** @return string */
    public function getStatus(): string {}

    /** @return string ISO 8601 datetime */
    public function getCreatedAt(): string {}
}

/**
 * Subscription entity.
 */
class Subscription
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string Plan UUID */
    public function getPlanId(): string {}

    /** @return string Customer UUID */
    public function getCustomerId(): string {}

    /** @return string */
    public function getStatus(): string {}

    /** @return string ISO 8601 datetime */
    public function getCurrentPeriodStart(): string {}

    /** @return string ISO 8601 datetime */
    public function getCurrentPeriodEnd(): string {}

    /** @return string|null ISO 8601 datetime */
    public function getCanceledAt(): ?string {}

    /** @return string ISO 8601 datetime */
    public function getCreatedAt(): string {}

    /** @return string ISO 8601 datetime */
    public function getUpdatedAt(): string {}

    /** @return string */
    public function __toString(): string {}
}

/**
 * Subscriptions API for managing subscriptions.
 */
class Subscriptions
{
    /**
     * Create a subscription plan.
     *
     * @param string $name Plan name
     * @param float $price Price per interval
     * @param string $interval Billing interval (day, week, month, year)
     * @param int|null $intervalCount Interval count
     * @param int|null $trialDays Trial period in days
     * @return SubscriptionPlan
     */
    public function createPlan(string $name, float $price, string $interval, ?int $intervalCount = null, ?int $trialDays = null): SubscriptionPlan {}

    /**
     * Get a plan by ID.
     *
     * @param string $id Plan UUID
     * @return SubscriptionPlan|null
     */
    public function getPlan(string $id): ?SubscriptionPlan {}

    /**
     * List all plans.
     *
     * @return SubscriptionPlan[]
     */
    public function listPlans(): array {}

    /**
     * Subscribe a customer to a plan.
     *
     * @param string $planId Plan UUID
     * @param string $customerId Customer UUID
     * @return Subscription
     */
    public function subscribe(string $planId, string $customerId): Subscription {}

    /**
     * Get a subscription by ID.
     *
     * @param string $id Subscription UUID
     * @return Subscription|null
     */
    public function get(string $id): ?Subscription {}

    /**
     * List all subscriptions.
     *
     * @return Subscription[]
     */
    public function list(): array {}

    /**
     * Pause a subscription.
     *
     * @param string $id Subscription UUID
     * @return Subscription
     */
    public function pause(string $id): Subscription {}

    /**
     * Resume a subscription.
     *
     * @param string $id Subscription UUID
     * @return Subscription
     */
    public function resume(string $id): Subscription {}

    /**
     * Cancel a subscription.
     *
     * @param string $id Subscription UUID
     * @param bool|null $atPeriodEnd Cancel at period end
     * @return Subscription
     */
    public function cancel(string $id, ?bool $atPeriodEnd = null): Subscription {}

    /**
     * Get subscriptions for a customer.
     *
     * @param string $customerId Customer UUID
     * @return Subscription[]
     */
    public function forCustomer(string $customerId): array {}

    /**
     * Check if a subscription is active.
     *
     * @param string $id Subscription UUID
     * @return bool
     */
    public function isActive(string $id): bool {}
}

// ============================================================================
// Promotions API
// ============================================================================

/**
 * Promotion entity.
 */
class Promotion
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string Promo code */
    public function getCode(): string {}

    /** @return string */
    public function getName(): string {}

    /** @return string|null */
    public function getDescription(): ?string {}

    /** @return string Discount type (percentage, fixed) */
    public function getDiscountType(): string {}

    /** @return float */
    public function getDiscountValue(): float {}

    /** @return float|null Minimum purchase amount */
    public function getMinPurchase(): ?float {}

    /** @return int|null Maximum uses */
    public function getMaxUses(): ?int {}

    /** @return int Current use count */
    public function getUsesCount(): int {}

    /** @return string|null ISO 8601 datetime */
    public function getStartsAt(): ?string {}

    /** @return string|null ISO 8601 datetime */
    public function getEndsAt(): ?string {}

    /** @return string */
    public function getStatus(): string {}

    /** @return string ISO 8601 datetime */
    public function getCreatedAt(): string {}

    /** @return string */
    public function __toString(): string {}
}

/**
 * Promotions API for managing promotions.
 */
class Promotions
{
    /**
     * Create a new promotion.
     *
     * @param string $code Promo code
     * @param string $name Promotion name
     * @param string $discountType Discount type (percentage, fixed)
     * @param float $discountValue Discount value
     * @param float|null $minPurchase Minimum purchase amount
     * @param int|null $maxUses Maximum uses
     * @return Promotion
     */
    public function create(string $code, string $name, string $discountType, float $discountValue, ?float $minPurchase = null, ?int $maxUses = null): Promotion {}

    /**
     * Get a promotion by ID.
     *
     * @param string $id Promotion UUID
     * @return Promotion|null
     */
    public function get(string $id): ?Promotion {}

    /**
     * Get a promotion by code.
     *
     * @param string $code Promo code
     * @return Promotion|null
     */
    public function getByCode(string $code): ?Promotion {}

    /**
     * List all promotions.
     *
     * @return Promotion[]
     */
    public function list(): array {}

    /**
     * Activate a promotion.
     *
     * @param string $id Promotion UUID
     * @return Promotion
     */
    public function activate(string $id): Promotion {}

    /**
     * Deactivate a promotion.
     *
     * @param string $id Promotion UUID
     * @return Promotion
     */
    public function deactivate(string $id): Promotion {}

    /**
     * Get active promotions.
     *
     * @return Promotion[]
     */
    public function getActive(): array {}

    /**
     * Check if a promo code is valid.
     *
     * @param string $code Promo code
     * @param float|null $orderTotal Order total for min purchase check
     * @return bool
     */
    public function isValid(string $code, ?float $orderTotal = null): bool {}

    /**
     * Delete a promotion.
     *
     * @param string $id Promotion UUID
     * @return bool
     */
    public function delete(string $id): bool {}
}

// ============================================================================
// Tax API
// ============================================================================

/**
 * Tax jurisdiction entity.
 */
class TaxJurisdiction
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string */
    public function getName(): string {}

    /** @return string Country code */
    public function getCountry(): string {}

    /** @return string|null State/province */
    public function getState(): ?string {}

    /** @return string|null City */
    public function getCity(): ?string {}

    /** @return string|null Postal code */
    public function getPostalCode(): ?string {}

    /** @return string */
    public function getStatus(): string {}
}

/**
 * Tax rate entity.
 */
class TaxRate
{
    /** @return string UUID */
    public function getId(): string {}

    /** @return string Jurisdiction UUID */
    public function getJurisdictionId(): string {}

    /** @return string */
    public function getName(): string {}

    /** @return float Rate as decimal (e.g., 0.08 for 8%) */
    public function getRate(): float {}

    /** @return string Tax type */
    public function getTaxType(): string {}

    /** @return string */
    public function getStatus(): string {}
}

/**
 * Tax API for tax calculations.
 */
class Tax
{
    /**
     * Calculate tax for an amount.
     *
     * @param float $amount Amount to tax
     * @param string $jurisdictionId Jurisdiction UUID
     * @return float Tax amount
     */
    public function calculate(float $amount, string $jurisdictionId): float {}

    /**
     * Get effective tax rate for a jurisdiction.
     *
     * @param string $jurisdictionId Jurisdiction UUID
     * @return float Rate as decimal
     */
    public function getEffectiveRate(string $jurisdictionId): float {}

    /**
     * List all tax jurisdictions.
     *
     * @return TaxJurisdiction[]
     */
    public function listJurisdictions(): array {}

    /**
     * Create a tax jurisdiction.
     *
     * @param string $name Jurisdiction name
     * @param string $country Country code
     * @param string|null $state State/province
     * @param string|null $city City
     * @return TaxJurisdiction
     */
    public function createJurisdiction(string $name, string $country, ?string $state = null, ?string $city = null): TaxJurisdiction {}

    /**
     * List tax rates for a jurisdiction.
     *
     * @param string $jurisdictionId Jurisdiction UUID
     * @return TaxRate[]
     */
    public function listRates(string $jurisdictionId): array {}

    /**
     * Create a tax rate.
     *
     * @param string $jurisdictionId Jurisdiction UUID
     * @param string $name Rate name
     * @param float $rate Rate as decimal
     * @param string $taxType Tax type
     * @return TaxRate
     */
    public function createRate(string $jurisdictionId, string $name, float $rate, string $taxType): TaxRate {}

    /**
     * Check if tax is enabled.
     *
     * @return bool
     */
    public function isEnabled(): bool {}

    /**
     * Enable or disable tax.
     *
     * @param bool $enabled Enable state
     * @return bool
     */
    public function setEnabled(bool $enabled): bool {}
}

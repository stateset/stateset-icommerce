"""
StateSet Embedded Commerce - Local-first commerce library

A native Rust commerce library with Python bindings for managing
customers, orders, products, inventory, and returns using SQLite.

Example:
    >>> from stateset_embedded import Commerce
    >>>
    >>> commerce = Commerce("./store.db")
    >>> customer = commerce.customers.create(
    ...     email="alice@example.com",
    ...     first_name="Alice",
    ...     last_name="Smith"
    ... )
    >>> print(customer.id)
"""

from stateset_embedded.stateset_embedded import (
    Commerce,
    Customers,
    Customer,
    Orders,
    Order,
    OrderItem,
    CreateOrderItemInput,
    Products,
    Product,
    ProductVariant,
    CreateProductVariantInput,
    Inventory,
    InventoryItem,
    StockLevel,
    Reservation,
    Returns,
    Return,
    CreateReturnItemInput,
    Payments,
    Payment,
    Refund,
    Shipments,
    Shipment,
    Warranties,
    Warranty,
    WarrantyClaim,
    PurchaseOrders,
    Supplier,
    PurchaseOrder,
    Invoices,
    Invoice,
    BomApi,
    Bom,
    BomComponent,
    WorkOrders,
    WorkOrder,
    Carts,
    Cart,
    CartItem,
    CartAddress,
    AddCartItemInput,
    ShippingRate,
    CheckoutResult,
    Analytics,
    SalesSummary,
    RevenueByPeriod,
    TopProduct,
    ProductPerformance,
    CustomerMetrics,
    TopCustomer,
    InventoryHealth,
    LowStockItem,
    InventoryMovement,
    OrderStatusBreakdown,
    FulfillmentMetrics,
    ReturnMetrics,
    DemandForecast,
    RevenueForecast,
    CurrencyOperations,
    ExchangeRate,
    ConversionResult,
    StoreCurrencySettings,
    SetExchangeRateInput,
)

__version__ = "0.1.1"

__all__ = [
    # Main entry point
    "Commerce",
    # Customers
    "Customers",
    "Customer",
    # Orders
    "Orders",
    "Order",
    "OrderItem",
    "CreateOrderItemInput",
    # Products
    "Products",
    "Product",
    "ProductVariant",
    "CreateProductVariantInput",
    # Inventory
    "Inventory",
    "InventoryItem",
    "StockLevel",
    "Reservation",
    # Returns
    "Returns",
    "Return",
    "CreateReturnItemInput",
    # Payments
    "Payments",
    "Payment",
    "Refund",
    # Shipments
    "Shipments",
    "Shipment",
    # Warranties
    "Warranties",
    "Warranty",
    "WarrantyClaim",
    # Purchase Orders
    "PurchaseOrders",
    "Supplier",
    "PurchaseOrder",
    # Invoices
    "Invoices",
    "Invoice",
    # Bill of Materials
    "BomApi",
    "Bom",
    "BomComponent",
    # Work Orders
    "WorkOrders",
    "WorkOrder",
    # Carts
    "Carts",
    "Cart",
    "CartItem",
    "CartAddress",
    "AddCartItemInput",
    "ShippingRate",
    "CheckoutResult",
    # Analytics
    "Analytics",
    "SalesSummary",
    "RevenueByPeriod",
    "TopProduct",
    "ProductPerformance",
    "CustomerMetrics",
    "TopCustomer",
    "InventoryHealth",
    "LowStockItem",
    "InventoryMovement",
    "OrderStatusBreakdown",
    "FulfillmentMetrics",
    "ReturnMetrics",
    "DemandForecast",
    "RevenueForecast",
    # Currency
    "CurrencyOperations",
    "ExchangeRate",
    "ConversionResult",
    "StoreCurrencySettings",
    "SetExchangeRateInput",
    # Version
    "__version__",
]

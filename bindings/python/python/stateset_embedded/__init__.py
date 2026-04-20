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
    SyncRuntime,
    SyncEvent,
    SyncStatus,
    SyncRemoteHead,
    SyncAcknowledgement,
    SyncRejection,
    SyncPushResult,
    SyncConfirmation,
    SyncDeadLetter,
    SyncPullResult,
    SyncSnapshot,
    SyncFullSyncResult,
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
    CustomObjectsApi,
    CustomObjectType,
    CustomFieldDefinition,
    CustomFieldDefinitionInput,
    CustomObject,
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
    VectorSearch,
    ProductSearchResult,
    CustomerSearchResult,
    EmbeddingStats,
)
from stateset_embedded.agent_toolkit import (
    AgentToolDescriptor,
    EmbeddedAgentToolkit,
    FrameworkToolFactory,
    create_embedded_agent_toolkit,
)
from stateset_embedded.autogen import create_autogen_tools
from stateset_embedded.crewai import create_crewai_tools
from stateset_embedded.generic import (
    create_callable_registry,
    create_tool_descriptors,
    execute_tool,
    execute_tool_calls,
)
from stateset_embedded.langchain import create_langchain_tools
from stateset_embedded.openai import (
    create_openai_tools,
    execute_openai_tool_call,
    execute_openai_tool_calls,
)

__version__ = "0.9.9"

__all__ = [
    # Main entry point
    "Commerce",
    "SyncRuntime",
    "SyncEvent",
    "SyncStatus",
    "SyncRemoteHead",
    "SyncAcknowledgement",
    "SyncRejection",
    "SyncPushResult",
    "SyncConfirmation",
    "SyncDeadLetter",
    "SyncPullResult",
    "SyncSnapshot",
    "SyncFullSyncResult",
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
    # Custom Objects
    "CustomObjectsApi",
    "CustomObjectType",
    "CustomFieldDefinition",
    "CustomFieldDefinitionInput",
    "CustomObject",
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
    # Vector Search
    "VectorSearch",
    "ProductSearchResult",
    "CustomerSearchResult",
    "EmbeddingStats",
    # Agent toolkit
    "AgentToolDescriptor",
    "EmbeddedAgentToolkit",
    "FrameworkToolFactory",
    "create_embedded_agent_toolkit",
    "create_tool_descriptors",
    "create_callable_registry",
    "execute_tool",
    "execute_tool_calls",
    "create_openai_tools",
    "execute_openai_tool_call",
    "execute_openai_tool_calls",
    "create_langchain_tools",
    "create_crewai_tools",
    "create_autogen_tools",
    # Version
    "__version__",
]

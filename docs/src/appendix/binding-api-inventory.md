# Binding API Inventory

This page is generated from the language binding manifests and exported package surfaces under `bindings/`.
Do not edit it by hand. Regenerate it with:

```bash
node ./scripts/ci/generate_binding_api_inventory.mjs
```

Machine-readable output lives at `artifacts/compatibility/binding-api-inventory.json`.

## Summary

| Metric | Value |
| --- | --- |
| Binding packages | 10 |
| Detailed surfaces | 5 |
| Ecosystems | 8 |

## Ecosystem Counts

| Ecosystem | Bindings |
| --- | --- |
| Composer | 1 |
| Go modules | 1 |
| Maven | 2 |
| npm | 2 |
| NuGet | 1 |
| PyPI | 1 |
| RubyGems | 1 |
| SwiftPM | 1 |

## Binding Overview

| Language | Ecosystem | Package | Version | Coverage | Summary |
| --- | --- | --- | --- | --- | --- |
| .NET | NuGet | `StateSet.Embedded` | `1.37.0` | `detailed` | 96 API methods |
| Go | Go modules | `github.com/stateset/stateset-icommerce/bindings/go/stateset` | — | `detailed` | 81 API methods |
| Java | Maven | `com.stateset:embedded` | `1.37.0` | `package-manifest` | manifest coverage |
| Kotlin | Maven | `com.stateset:embedded-kotlin` | `1.37.0` | `package-manifest` | manifest coverage |
| Node.js | npm | `@stateset/embedded` | `1.37.0` | `detailed` | 10 export entrypoints |
| PHP | Composer | `stateset/embedded` | `1.37.0` | `package-manifest` | manifest coverage |
| Python | PyPI | `stateset-embedded` | `1.37.0` | `detailed` | 257 public symbols |
| Ruby | RubyGems | `stateset_embedded` | `1.37.0` | `package-manifest` | manifest coverage |
| Swift | SwiftPM | `StateSet` | — | `detailed` | 82 API methods |
| WASM | npm | `@stateset/embedded-wasm` | `1.37.0` | `package-manifest` | manifest coverage |

## Node.js Exports

| Subpath | Runtime entry | Types entry |
| --- | --- | --- |
| `.` | `./index.js` | `./index.d.ts` |
| `./agent-toolkit` | `./agent-toolkit.mjs` | `./agent-toolkit.d.ts` |
| `./canonical-json` | `./canonical-json.mjs` | `./canonical-json.d.ts` |
| `./generic` | `./generic.mjs` | `./generic.d.ts` |
| `./langchain` | `./langchain.mjs` | `./langchain.d.ts` |
| `./native-toolkit` | `./native-toolkit.mjs` | `./native-toolkit.d.ts` |
| `./openai` | `./openai.mjs` | `./openai.d.ts` |
| `./purchase-runtime` | `./purchase-runtime.mjs` | `./purchase-runtime.d.ts` |
| `./vercel-ai` | `./vercel-ai.mjs` | `./vercel-ai.d.ts` |
| `./webmcp` | `./webmcp.mjs` | `./webmcp.d.ts` |

## Go Surface Summary

| Metric | Value |
| --- | --- |
| Exported types | 73 |
| API types | 16 |
| Root accessors | 16 |
| API methods | 81 |

## Go Exported Types

| Type |
| --- |
| `AnalyticsAPI` |
| `BillOfMaterials` |
| `BOMAPI` |
| `BOMComponent` |
| `BOMStatus` |
| `Cart` |
| `CartsAPI` |
| `ClaimResolution` |
| `ClaimStatus` |
| `Commerce` |
| `ConversionResult` |
| `Currency` |
| `CurrencyAPI` |
| `Customer` |
| `CustomersAPI` |
| `ExchangeRate` |
| `InventoryAPI` |
| `InventoryItem` |
| `InventoryReservation` |
| `Invoice` |
| `InvoiceItem` |
| `InvoicesAPI` |
| `InvoiceStatus` |
| `InvoiceType` |
| `LocationStock` |
| `Order` |
| `OrderItem` |
| `OrdersAPI` |
| `OrderStatus` |
| `Payment` |
| `PaymentMethod` |
| `PaymentsAPI` |
| `PaymentTerms` |
| `Product` |
| `ProductsAPI` |
| `ProductVariant` |
| `PurchaseOrder` |
| `PurchaseOrderItem` |
| `PurchaseOrdersAPI` |
| `PurchaseOrderStatus` |
| `Refund` |
| `RefundStatus` |
| `ReservationStatus` |
| `Return` |
| `ReturnReason` |
| `ReturnsAPI` |
| `ReturnStatus` |
| `SalesSummary` |
| `Shipment` |
| `ShipmentEvent` |
| `ShipmentItem` |
| `ShipmentsAPI` |
| `ShipmentStatus` |
| `ShippingCarrier` |
| `StockLevel` |
| `StoreCurrencySettings` |
| `Supplier` |
| `SuppliersAPI` |
| `TaskStatus` |
| `TimePeriod` |
| `TopCustomer` |
| `TopProduct` |
| `WarrantiesAPI` |
| `Warranty` |
| `WarrantyClaim` |
| `WarrantyStatus` |
| `WarrantyType` |
| `WorkOrder` |
| `WorkOrderMaterial` |
| `WorkOrderPriority` |
| `WorkOrdersAPI` |
| `WorkOrderStatus` |
| `WorkOrderTask` |

## Go Commerce Accessors

| Accessor |
| --- |
| `Analytics` |
| `BOM` |
| `Carts` |
| `Currency` |
| `Customers` |
| `Inventory` |
| `Invoices` |
| `Orders` |
| `Payments` |
| `Products` |
| `PurchaseOrders` |
| `Returns` |
| `Shipments` |
| `Suppliers` |
| `Warranties` |
| `WorkOrders` |

## Go API Methods

| Receiver | Method |
| --- | --- |
| `AnalyticsAPI` | `GetSalesSummary` |
| `AnalyticsAPI` | `GetTopCustomers` |
| `AnalyticsAPI` | `GetTopProducts` |
| `BOMAPI` | `Activate` |
| `BOMAPI` | `AddComponent` |
| `BOMAPI` | `Create` |
| `BOMAPI` | `Get` |
| `BOMAPI` | `GetComponents` |
| `BOMAPI` | `List` |
| `CartsAPI` | `AddItem` |
| `CartsAPI` | `Create` |
| `CartsAPI` | `Get` |
| `CurrencyAPI` | `Convert` |
| `CurrencyAPI` | `GetRate` |
| `CurrencyAPI` | `GetSettings` |
| `CurrencyAPI` | `SetRate` |
| `CustomersAPI` | `Create` |
| `CustomersAPI` | `Delete` |
| `CustomersAPI` | `Get` |
| `CustomersAPI` | `List` |
| `InventoryAPI` | `Adjust` |
| `InventoryAPI` | `CreateItem` |
| `InventoryAPI` | `GetLevel` |
| `InvoicesAPI` | `Create` |
| `InvoicesAPI` | `Get` |
| `InvoicesAPI` | `GetOverdue` |
| `InvoicesAPI` | `List` |
| `InvoicesAPI` | `RecordPayment` |
| `InvoicesAPI` | `Send` |
| `InvoicesAPI` | `Void` |
| `OrdersAPI` | `Cancel` |
| `OrdersAPI` | `Create` |
| `OrdersAPI` | `Get` |
| `OrdersAPI` | `List` |
| `OrdersAPI` | `Ship` |
| `OrdersAPI` | `UpdateStatus` |
| `PaymentsAPI` | `Complete` |
| `PaymentsAPI` | `Create` |
| `PaymentsAPI` | `Fail` |
| `PaymentsAPI` | `Get` |
| `PaymentsAPI` | `List` |
| `PaymentsAPI` | `Refund` |
| `ProductsAPI` | `Create` |
| `ProductsAPI` | `Get` |
| `ProductsAPI` | `List` |
| `ProductsAPI` | `Publish` |
| `PurchaseOrdersAPI` | `Approve` |
| `PurchaseOrdersAPI` | `Cancel` |
| `PurchaseOrdersAPI` | `Create` |
| `PurchaseOrdersAPI` | `Get` |
| `PurchaseOrdersAPI` | `List` |
| `PurchaseOrdersAPI` | `Send` |
| `PurchaseOrdersAPI` | `Submit` |
| `ReturnsAPI` | `Approve` |
| `ReturnsAPI` | `Complete` |
| `ReturnsAPI` | `Create` |
| `ReturnsAPI` | `Get` |
| `ReturnsAPI` | `List` |
| `ReturnsAPI` | `Reject` |
| `ShipmentsAPI` | `Cancel` |
| `ShipmentsAPI` | `Create` |
| `ShipmentsAPI` | `Deliver` |
| `ShipmentsAPI` | `Get` |
| `ShipmentsAPI` | `List` |
| `ShipmentsAPI` | `Ship` |
| `SuppliersAPI` | `Create` |
| `SuppliersAPI` | `Get` |
| `SuppliersAPI` | `List` |
| `WarrantiesAPI` | `ApproveClaim` |
| `WarrantiesAPI` | `CompleteClaim` |
| `WarrantiesAPI` | `Create` |
| `WarrantiesAPI` | `CreateClaim` |
| `WarrantiesAPI` | `DenyClaim` |
| `WarrantiesAPI` | `Get` |
| `WarrantiesAPI` | `List` |
| `WorkOrdersAPI` | `Cancel` |
| `WorkOrdersAPI` | `Complete` |
| `WorkOrdersAPI` | `Create` |
| `WorkOrdersAPI` | `Get` |
| `WorkOrdersAPI` | `List` |
| `WorkOrdersAPI` | `Start` |

## .NET Surface Summary

| Metric | Value |
| --- | --- |
| Public types | 60 |
| API types | 8 |
| Facade properties | 8 |
| API methods | 96 |
| Target frameworks |  |

## .NET Public Types

| Type |
| --- |
| `AddCartItem` |
| `Address` |
| `Cart` |
| `CartAddress` |
| `CartItem` |
| `CartPaymentStatus` |
| `CartsApi` |
| `CartStatus` |
| `CheckoutResult` |
| `CreateOrderItem` |
| `CreateProductVariant` |
| `CreateReturnItem` |
| `CreateShipmentItem` |
| `Customer` |
| `CustomersApi` |
| `CustomerStatus` |
| `FulfillmentStatus` |
| `InventoryApi` |
| `InventoryItem` |
| `InventoryReservation` |
| `InventoryTransaction` |
| `ItemCondition` |
| `LocationStock` |
| `Order` |
| `OrderItem` |
| `OrderPaymentStatus` |
| `OrdersApi` |
| `OrderStatus` |
| `Payment` |
| `PaymentMethod` |
| `PaymentsApi` |
| `PaymentStatus` |
| `Product` |
| `ProductsApi` |
| `ProductStatus` |
| `ProductType` |
| `ProductVariant` |
| `Refund` |
| `RefundStatus` |
| `ReservationStatus` |
| `Return` |
| `ReturnDisposition` |
| `ReturnItem` |
| `ReturnReason` |
| `ReturnsApi` |
| `ReturnStatus` |
| `Shipment` |
| `ShipmentItem` |
| `ShipmentsApi` |
| `ShipmentStatus` |
| `ShippingCarrier` |
| `ShippingMethod` |
| `StateSetCommerce` |
| `StateSetErrorCode` |
| `StateSetException` |
| `StateSetNotFoundException` |
| `StateSetValidationException` |
| `StockLevel` |
| `UpdateCustomer` |
| `UpdateProduct` |

## .NET Facade Properties

| Property | Type |
| --- | --- |
| `Carts` | `CartsApi` |
| `Customers` | `CustomersApi` |
| `Inventory` | `InventoryApi` |
| `Orders` | `OrdersApi` |
| `Payments` | `PaymentsApi` |
| `Products` | `ProductsApi` |
| `Returns` | `ReturnsApi` |
| `Shipments` | `ShipmentsApi` |

## .NET API Methods

| API type | Method |
| --- | --- |
| `CartsApi` | `Abandon` |
| `CartsApi` | `AddItem` |
| `CartsApi` | `ApplyDiscount` |
| `CartsApi` | `BeginCheckout` |
| `CartsApi` | `Cancel` |
| `CartsApi` | `ClearItems` |
| `CartsApi` | `Complete` |
| `CartsApi` | `Create` |
| `CartsApi` | `Get` |
| `CartsApi` | `GetItems` |
| `CartsApi` | `List` |
| `CartsApi` | `MarkReadyForPayment` |
| `CartsApi` | `Recalculate` |
| `CartsApi` | `RemoveDiscount` |
| `CartsApi` | `RemoveItem` |
| `CartsApi` | `SetBillingAddress` |
| `CartsApi` | `SetPayment` |
| `CartsApi` | `SetShipping` |
| `CartsApi` | `SetShippingAddress` |
| `CartsApi` | `UpdateItemQuantity` |
| `CustomersApi` | `Count` |
| `CustomersApi` | `Create` |
| `CustomersApi` | `Delete` |
| `CustomersApi` | `Get` |
| `CustomersApi` | `GetByEmail` |
| `CustomersApi` | `List` |
| `CustomersApi` | `Update` |
| `InventoryApi` | `Adjust` |
| `InventoryApi` | `ConfirmReservation` |
| `InventoryApi` | `CreateItem` |
| `InventoryApi` | `GetItem` |
| `InventoryApi` | `GetReservation` |
| `InventoryApi` | `GetStock` |
| `InventoryApi` | `HasStock` |
| `InventoryApi` | `List` |
| `InventoryApi` | `ReleaseReservation` |
| `InventoryApi` | `Reserve` |
| `OrdersApi` | `Cancel` |
| `OrdersApi` | `Count` |
| `OrdersApi` | `Create` |
| `OrdersApi` | `Deliver` |
| `OrdersApi` | `Get` |
| `OrdersApi` | `GetByNumber` |
| `OrdersApi` | `List` |
| `OrdersApi` | `ListForCustomer` |
| `OrdersApi` | `Ship` |
| `OrdersApi` | `UpdateStatus` |
| `PaymentsApi` | `Cancel` |
| `PaymentsApi` | `Complete` |
| `PaymentsApi` | `CompleteRefund` |
| `PaymentsApi` | `Create` |
| `PaymentsApi` | `Fail` |
| `PaymentsApi` | `FailRefund` |
| `PaymentsApi` | `ForOrder` |
| `PaymentsApi` | `Get` |
| `PaymentsApi` | `GetRefund` |
| `PaymentsApi` | `GetRefunds` |
| `PaymentsApi` | `List` |
| `PaymentsApi` | `MarkProcessing` |
| `PaymentsApi` | `Refund` |
| `ProductsApi` | `Activate` |
| `ProductsApi` | `AddVariant` |
| `ProductsApi` | `Archive` |
| `ProductsApi` | `Count` |
| `ProductsApi` | `Create` |
| `ProductsApi` | `Delete` |
| `ProductsApi` | `Get` |
| `ProductsApi` | `GetBySlug` |
| `ProductsApi` | `GetVariantBySku` |
| `ProductsApi` | `GetVariants` |
| `ProductsApi` | `List` |
| `ProductsApi` | `Search` |
| `ProductsApi` | `Update` |
| `ReturnsApi` | `AddTracking` |
| `ReturnsApi` | `Approve` |
| `ReturnsApi` | `Cancel` |
| `ReturnsApi` | `Complete` |
| `ReturnsApi` | `Create` |
| `ReturnsApi` | `Get` |
| `ReturnsApi` | `List` |
| `ReturnsApi` | `ListForOrder` |
| `ReturnsApi` | `MarkReceived` |
| `ReturnsApi` | `Reject` |
| `ReturnsApi` | `SetItemDisposition` |
| `ShipmentsApi` | `Cancel` |
| `ShipmentsApi` | `Create` |
| `ShipmentsApi` | `Deliver` |
| `ShipmentsApi` | `ForOrder` |
| `ShipmentsApi` | `Get` |
| `ShipmentsApi` | `GetByTracking` |
| `ShipmentsApi` | `List` |
| `ShipmentsApi` | `MarkInTransit` |
| `ShipmentsApi` | `MarkOutForDelivery` |
| `ShipmentsApi` | `MarkProcessing` |
| `ShipmentsApi` | `MarkReady` |
| `ShipmentsApi` | `Ship` |

## Python Helper Modules

| Module |
| --- |
| `agent_toolkit` |
| `autogen` |
| `crewai` |
| `generic` |
| `langchain` |
| `openai` |

## Python Public Symbols

| Symbol |
| --- |
| `__version__` |
| `ActivityLogEntry` |
| `ActivityLogs` |
| `AddCartItemInput` |
| `AgentFeedback` |
| `AgentIdentity` |
| `AgentToolDescriptor` |
| `AgentValidationRequest` |
| `AgentValidationResponse` |
| `AgentValidationStatus` |
| `Analytics` |
| `AssetDisposal` |
| `Bom` |
| `BomApi` |
| `BomComponent` |
| `BoostRule` |
| `BoostRuleInput` |
| `CanadianTaxInfo` |
| `CaptureStockLineInput` |
| `Cart` |
| `CartAddress` |
| `CartItem` |
| `Carts` |
| `Channel` |
| `ChannelProductMapping` |
| `ChannelProductSyncItem` |
| `Channels` |
| `CheckoutResult` |
| `CloseMonthReport` |
| `CloseMonthStep` |
| `Commerce` |
| `Companies` |
| `Company` |
| `CompanyPriceOverride` |
| `CompanyShippingAddress` |
| `Contact` |
| `ConversionResult` |
| `create_autogen_tools` |
| `create_callable_registry` |
| `create_crewai_tools` |
| `create_embedded_agent_toolkit` |
| `create_langchain_tools` |
| `create_openai_tools` |
| `create_tool_descriptors` |
| `CreateIntegrationMappingInput` |
| `CreateInvoiceItemInput` |
| `CreateOrderItemInput` |
| `CreateProductVariantInput` |
| `CreatePurchaseOrderItemInput` |
| `CreateReturnItemInput` |
| `CurrencyOperations` |
| `Customer` |
| `CustomerMetrics` |
| `Customers` |
| `CustomerSearchResult` |
| `CustomFieldDefinition` |
| `CustomFieldDefinitionInput` |
| `CustomObject` |
| `CustomObjectsApi` |
| `CustomObjectType` |
| `CycleCount` |
| `CycleCountLine` |
| `CycleCountLineInput` |
| `CycleCounts` |
| `DemandForecast` |
| `DepreciationEntry` |
| `DepreciationSchedule` |
| `EmbeddedAgentToolkit` |
| `EmbeddingStats` |
| `Erc8004` |
| `ExchangeRate` |
| `execute_openai_tool_call` |
| `execute_openai_tool_calls` |
| `execute_tool` |
| `execute_tool_calls` |
| `FacetConfig` |
| `FacetConfigInput` |
| `FeedbackSummary` |
| `FixedAsset` |
| `FixedAssets` |
| `FrameworkToolFactory` |
| `Fraud` |
| `FraudAssessment` |
| `FraudRule` |
| `FraudSignal` |
| `FraudSignalInput` |
| `FulfillmentMetrics` |
| `GiftCard` |
| `GiftCards` |
| `GiftCardTransaction` |
| `GlPeriod` |
| `InboundShipment` |
| `InboundShipmentItem` |
| `InboundShipmentItemInput` |
| `InboundShipments` |
| `IngestLineItemInput` |
| `IntegrationFieldMapping` |
| `IntegrationFieldMappings` |
| `IntegrationMapping` |
| `IntegrationMappings` |
| `Inventory` |
| `InventoryHealth` |
| `InventoryItem` |
| `InventoryMovement` |
| `Invoice` |
| `Invoices` |
| `jcs_canonicalize` |
| `LowStockItem` |
| `Loyalty` |
| `LoyaltyAccount` |
| `LoyaltyProgram` |
| `LoyaltyTier` |
| `LoyaltyTierInput` |
| `LoyaltyTransaction` |
| `merkle_root` |
| `NewIntegrationFieldMapping` |
| `Order` |
| `OrderItem` |
| `Orders` |
| `OrderStatusBreakdown` |
| `PairStationResult` |
| `payload_plain_hash` |
| `Payment` |
| `PaymentObligation` |
| `PaymentObligationDashboard` |
| `PaymentObligations` |
| `Payments` |
| `PerformanceObligation` |
| `PerformanceObligationInput` |
| `Prepayment` |
| `PrepaymentApplication` |
| `Prepayments` |
| `PriceLevel` |
| `PriceLevelEntry` |
| `PriceLevels` |
| `PriceSchedule` |
| `PriceScheduleEntry` |
| `PriceSchedules` |
| `PrintJob` |
| `PrintStation` |
| `PrintStations` |
| `Product` |
| `ProductionBatch` |
| `ProductionBatches` |
| `ProductPerformance` |
| `Products` |
| `ProductSearchResult` |
| `ProductVariant` |
| `PromotionCondition` |
| `PromotionConditionInput` |
| `PromotionLineItemInput` |
| `PurchaseOrder` |
| `PurchaseOrders` |
| `Purgatory` |
| `PurgatoryLineItem` |
| `PurgatoryOrder` |
| `RecordCycleCountLineInput` |
| `Refund` |
| `Reservation` |
| `Return` |
| `ReturnMetrics` |
| `Returns` |
| `RevaluationLine` |
| `RevaluationResult` |
| `RevenueByPeriod` |
| `RevenueContract` |
| `RevenueForecast` |
| `RevenueRecognition` |
| `RevenueSchedule` |
| `RevenueScheduleEntry` |
| `Review` |
| `Reviews` |
| `ReviewSummary` |
| `Reward` |
| `SalesSummary` |
| `SearchConfig` |
| `SearchConfigs` |
| `SearchField` |
| `SearchFieldInput` |
| `Segment` |
| `SegmentMembership` |
| `SegmentRule` |
| `SegmentRuleInput` |
| `Segments` |
| `SetExchangeRateInput` |
| `Shipment` |
| `Shipments` |
| `ShippingCondition` |
| `ShippingRate` |
| `ShippingZone` |
| `ShippingZones` |
| `StockLevel` |
| `StockSnapshot` |
| `StockSnapshotLine` |
| `StockSnapshots` |
| `StoreCredit` |
| `StoreCredits` |
| `StoreCreditTransaction` |
| `StoreCurrencySettings` |
| `Supplier` |
| `SupplierSku` |
| `SupplierSkuBulkItemInput` |
| `SupplierSkus` |
| `SyncAcknowledgement` |
| `SyncConfirmation` |
| `SyncDeadLetter` |
| `SyncEvent` |
| `SyncFullSyncResult` |
| `SyncPullResult` |
| `SyncPushResult` |
| `SyncRejection` |
| `SyncRemoteHead` |
| `SyncRuntime` |
| `SyncSnapshot` |
| `SyncStatus` |
| `SynonymGroup` |
| `SynonymGroupInput` |
| `TaxApi` |
| `TaxCalculationResult` |
| `TaxExemption` |
| `TaxJurisdiction` |
| `TaxRate` |
| `TaxSettings` |
| `ThreeWayMatchLine` |
| `ThreeWayMatchResult` |
| `TopCustomer` |
| `TopologySnapshot` |
| `TopologySnapshots` |
| `TopProduct` |
| `TransferOrder` |
| `TransferOrderItem` |
| `TransferOrderItemInput` |
| `TransferOrders` |
| `UnitClass` |
| `UnitConversionRule` |
| `UnitOfMeasure` |
| `UnitsOfMeasure` |
| `UsStateTaxInfo` |
| `ValidationSummary` |
| `VectorSearch` |
| `VendorCredit` |
| `VendorCreditApplication` |
| `VendorCredits` |
| `VendorReturn` |
| `VendorReturnItem` |
| `VendorReturnItemInput` |
| `VendorReturns` |
| `Warranties` |
| `Warranty` |
| `WarrantyClaim` |
| `Wishlist` |
| `WishlistItem` |
| `Wishlists` |
| `WorkOrder` |
| `WorkOrders` |
| `ZoneShippingMethod` |
| `ZoneShippingRate` |

## Swift Surface Summary

| Metric | Value |
| --- | --- |
| Public types | 58 |
| API types | 8 |
| Facade properties | 8 |
| API methods | 82 |
| Targets | `StateSet`, `StateSetTests` |

## Swift Public Types

| Type |
| --- |
| `AddCartItem` |
| `Address` |
| `Cart` |
| `CartAddress` |
| `CartItem` |
| `CartPaymentStatus` |
| `CartsAPI` |
| `CartStatus` |
| `CheckoutResult` |
| `Code` |
| `CreateOrderItem` |
| `CreateProductVariant` |
| `CreateReturnItem` |
| `CreateShipmentItem` |
| `Customer` |
| `CustomersAPI` |
| `CustomerStatus` |
| `DecimalString` |
| `FulfillmentStatus` |
| `InventoryAPI` |
| `InventoryItem` |
| `InventoryReservation` |
| `InventoryTransaction` |
| `ItemCondition` |
| `LocationStock` |
| `OptionalDecimalString` |
| `Order` |
| `OrderItem` |
| `OrderPaymentStatus` |
| `OrdersAPI` |
| `OrderStatus` |
| `Payment` |
| `PaymentMethod` |
| `PaymentsAPI` |
| `PaymentStatus` |
| `Product` |
| `ProductsAPI` |
| `ProductStatus` |
| `ProductType` |
| `ProductVariant` |
| `Refund` |
| `RefundStatus` |
| `ReservationStatus` |
| `Return` |
| `ReturnDisposition` |
| `ReturnItem` |
| `ReturnReason` |
| `ReturnsAPI` |
| `ReturnStatus` |
| `Shipment` |
| `ShipmentItem` |
| `ShipmentsAPI` |
| `ShipmentStatus` |
| `ShippingCarrier` |
| `ShippingMethod` |
| `StateSetCommerce` |
| `StateSetError` |
| `StockLevel` |

## Swift Facade Properties

| Property | Type |
| --- | --- |
| `carts` | `CartsAPI` |
| `customers` | `CustomersAPI` |
| `inventory` | `InventoryAPI` |
| `orders` | `OrdersAPI` |
| `payments` | `PaymentsAPI` |
| `products` | `ProductsAPI` |
| `returns` | `ReturnsAPI` |
| `shipments` | `ShipmentsAPI` |

## Swift API Methods

| API type | Method |
| --- | --- |
| `CartsAPI` | `abandon` |
| `CartsAPI` | `addItem` |
| `CartsAPI` | `applyDiscount` |
| `CartsAPI` | `cancel` |
| `CartsAPI` | `clearItems` |
| `CartsAPI` | `complete` |
| `CartsAPI` | `create` |
| `CartsAPI` | `get` |
| `CartsAPI` | `items` |
| `CartsAPI` | `list` |
| `CartsAPI` | `recalculate` |
| `CartsAPI` | `removeItem` |
| `CartsAPI` | `setBillingAddress` |
| `CartsAPI` | `setPayment` |
| `CartsAPI` | `setShipping` |
| `CartsAPI` | `setShippingAddress` |
| `CartsAPI` | `updateItemQuantity` |
| `CustomersAPI` | `count` |
| `CustomersAPI` | `create` |
| `CustomersAPI` | `delete` |
| `CustomersAPI` | `get` |
| `CustomersAPI` | `list` |
| `CustomersAPI` | `update` |
| `InventoryAPI` | `adjust` |
| `InventoryAPI` | `confirmReservation` |
| `InventoryAPI` | `createItem` |
| `InventoryAPI` | `getStock` |
| `InventoryAPI` | `hasStock` |
| `InventoryAPI` | `item` |
| `InventoryAPI` | `list` |
| `InventoryAPI` | `releaseReservation` |
| `InventoryAPI` | `reserve` |
| `OrdersAPI` | `cancel` |
| `OrdersAPI` | `count` |
| `OrdersAPI` | `create` |
| `OrdersAPI` | `deliver` |
| `OrdersAPI` | `get` |
| `OrdersAPI` | `list` |
| `OrdersAPI` | `ship` |
| `OrdersAPI` | `updateStatus` |
| `PaymentsAPI` | `cancel` |
| `PaymentsAPI` | `complete` |
| `PaymentsAPI` | `completeRefund` |
| `PaymentsAPI` | `create` |
| `PaymentsAPI` | `fail` |
| `PaymentsAPI` | `failRefund` |
| `PaymentsAPI` | `get` |
| `PaymentsAPI` | `list` |
| `PaymentsAPI` | `markProcessing` |
| `PaymentsAPI` | `refund` |
| `PaymentsAPI` | `refunds` |
| `ProductsAPI` | `activate` |
| `ProductsAPI` | `addVariant` |
| `ProductsAPI` | `archive` |
| `ProductsAPI` | `count` |
| `ProductsAPI` | `create` |
| `ProductsAPI` | `delete` |
| `ProductsAPI` | `get` |
| `ProductsAPI` | `list` |
| `ProductsAPI` | `search` |
| `ProductsAPI` | `variant` |
| `ProductsAPI` | `variants` |
| `ReturnsAPI` | `addTracking` |
| `ReturnsAPI` | `approve` |
| `ReturnsAPI` | `cancel` |
| `ReturnsAPI` | `complete` |
| `ReturnsAPI` | `create` |
| `ReturnsAPI` | `get` |
| `ReturnsAPI` | `list` |
| `ReturnsAPI` | `markReceived` |
| `ReturnsAPI` | `reject` |
| `ReturnsAPI` | `setItemDisposition` |
| `ShipmentsAPI` | `cancel` |
| `ShipmentsAPI` | `create` |
| `ShipmentsAPI` | `deliver` |
| `ShipmentsAPI` | `get` |
| `ShipmentsAPI` | `list` |
| `ShipmentsAPI` | `markInTransit` |
| `ShipmentsAPI` | `markOutForDelivery` |
| `ShipmentsAPI` | `markProcessing` |
| `ShipmentsAPI` | `markReady` |
| `ShipmentsAPI` | `ship` |

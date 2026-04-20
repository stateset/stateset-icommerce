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
| .NET | NuGet | `StateSet.Embedded` | `0.9.9` | `detailed` | 233 API methods |
| Go | Go modules | `github.com/stateset/stateset-icommerce/bindings/go/stateset` | — | `detailed` | 80 API methods |
| Java | Maven | `com.stateset:embedded` | `0.9.9` | `package-manifest` | manifest coverage |
| Kotlin | Maven | `com.stateset:embedded-kotlin` | `0.9.9` | `package-manifest` | manifest coverage |
| Node.js | npm | `@stateset/embedded` | `0.9.9` | `detailed` | 6 export entrypoints |
| PHP | Composer | `stateset/embedded` | `0.9.9` | `package-manifest` | manifest coverage |
| Python | PyPI | `stateset-embedded` | `0.9.9` | `detailed` | 99 public symbols |
| Ruby | RubyGems | `stateset_embedded` | `0.9.9` | `package-manifest` | manifest coverage |
| Swift | SwiftPM | `StateSet` | — | `detailed` | 232 API methods |
| WASM | npm | `@stateset/embedded-wasm` | `0.9.9` | `package-manifest` | manifest coverage |

## Node.js Exports

| Subpath | Runtime entry | Types entry |
| --- | --- | --- |
| `.` | `./index.js` | `./index.d.ts` |
| `./agent-toolkit` | `./agent-toolkit.mjs` | `./agent-toolkit.d.ts` |
| `./generic` | `./generic.mjs` | `./generic.d.ts` |
| `./langchain` | `./langchain.mjs` | `./langchain.d.ts` |
| `./openai` | `./openai.mjs` | `./openai.d.ts` |
| `./vercel-ai` | `./vercel-ai.mjs` | `./vercel-ai.d.ts` |

## Go Surface Summary

| Metric | Value |
| --- | --- |
| Exported types | 73 |
| API types | 16 |
| Root accessors | 16 |
| API methods | 80 |

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
| Public types | 79 |
| API types | 31 |
| Facade properties | 31 |
| API methods | 233 |
| Target frameworks | `net6.0`, `net7.0`, `net8.0` |

## .NET Public Types

| Type |
| --- |
| `AccountsPayableApi` |
| `AccountsReceivableApi` |
| `AnalyticsApi` |
| `BackordersApi` |
| `BillOfMaterials` |
| `BomApi` |
| `BomComponent` |
| `BomStatus` |
| `Cart` |
| `CartsApi` |
| `ClaimResolution` |
| `ClaimStatus` |
| `ConversionResult` |
| `CostAccountingApi` |
| `CreditApi` |
| `CurrencyApi` |
| `CurrencyCode` |
| `Customer` |
| `CustomersApi` |
| `ExchangeRate` |
| `FulfillmentApi` |
| `GeneralLedgerApi` |
| `InventoryApi` |
| `InventoryItem` |
| `Invoice` |
| `InvoiceItem` |
| `InvoicesApi` |
| `InvoiceStatus` |
| `LotsApi` |
| `Order` |
| `OrderItem` |
| `OrdersApi` |
| `OrderStatus` |
| `Payment` |
| `PaymentMethod` |
| `PaymentsApi` |
| `Product` |
| `ProductsApi` |
| `ProductVariant` |
| `PromotionsApi` |
| `PurchaseOrder` |
| `PurchaseOrderItem` |
| `PurchaseOrdersApi` |
| `PurchaseOrderStatus` |
| `QualityApi` |
| `ReceivingApi` |
| `Refund` |
| `RefundStatus` |
| `Return` |
| `ReturnReason` |
| `ReturnsApi` |
| `ReturnStatus` |
| `SalesSummary` |
| `SerialsApi` |
| `Shipment` |
| `ShipmentsApi` |
| `ShipmentStatus` |
| `ShippingCarrier` |
| `StateSetCommerce` |
| `StateSetException` |
| `StockLevel` |
| `StoreCurrencySettings` |
| `SubscriptionsApi` |
| `Supplier` |
| `SuppliersApi` |
| `TaxApi` |
| `TimePeriod` |
| `TopCustomer` |
| `TopProduct` |
| `WarehouseApi` |
| `WarrantiesApi` |
| `Warranty` |
| `WarrantyClaim` |
| `WarrantyStatus` |
| `WarrantyType` |
| `WorkOrder` |
| `WorkOrderPriority` |
| `WorkOrdersApi` |
| `WorkOrderStatus` |

## .NET Facade Properties

| Property | Type |
| --- | --- |
| `AccountsPayable` | `AccountsPayableApi` |
| `AccountsReceivable` | `AccountsReceivableApi` |
| `Analytics` | `AnalyticsApi` |
| `Backorders` | `BackordersApi` |
| `Bom` | `BomApi` |
| `Carts` | `CartsApi` |
| `CostAccounting` | `CostAccountingApi` |
| `Credit` | `CreditApi` |
| `Currency` | `CurrencyApi` |
| `Customers` | `CustomersApi` |
| `Fulfillment` | `FulfillmentApi` |
| `GeneralLedger` | `GeneralLedgerApi` |
| `Inventory` | `InventoryApi` |
| `Invoices` | `InvoicesApi` |
| `Lots` | `LotsApi` |
| `Orders` | `OrdersApi` |
| `Payments` | `PaymentsApi` |
| `Products` | `ProductsApi` |
| `Promotions` | `PromotionsApi` |
| `PurchaseOrders` | `PurchaseOrdersApi` |
| `Quality` | `QualityApi` |
| `Receiving` | `ReceivingApi` |
| `Returns` | `ReturnsApi` |
| `Serials` | `SerialsApi` |
| `Shipments` | `ShipmentsApi` |
| `Subscriptions` | `SubscriptionsApi` |
| `Suppliers` | `SuppliersApi` |
| `Tax` | `TaxApi` |
| `Warehouse` | `WarehouseApi` |
| `Warranties` | `WarrantiesApi` |
| `WorkOrders` | `WorkOrdersApi` |

## .NET API Methods

| API type | Method |
| --- | --- |
| `AccountsPayableApi` | `ApproveBill` |
| `AccountsPayableApi` | `CancelBill` |
| `AccountsPayableApi` | `CreateBill` |
| `AccountsPayableApi` | `GetAgingSummary` |
| `AccountsPayableApi` | `GetBill` |
| `AccountsPayableApi` | `GetBillByNumber` |
| `AccountsPayableApi` | `GetBillsDueSoon` |
| `AccountsPayableApi` | `GetOverdueBills` |
| `AccountsPayableApi` | `GetTotalOutstanding` |
| `AccountsPayableApi` | `ListBills` |
| `AccountsReceivableApi` | `CreateCreditMemo` |
| `AccountsReceivableApi` | `GetAgingSummary` |
| `AccountsReceivableApi` | `GetCreditMemo` |
| `AccountsReceivableApi` | `GetDso` |
| `AccountsReceivableApi` | `GetTotalOutstanding` |
| `AccountsReceivableApi` | `GetUnappliedCredits` |
| `AccountsReceivableApi` | `ListCreditMemos` |
| `AccountsReceivableApi` | `VoidCreditMemo` |
| `AnalyticsApi` | `GetSalesSummary` |
| `AnalyticsApi` | `GetTopCustomers` |
| `AnalyticsApi` | `GetTopProducts` |
| `BackordersApi` | `CancelBackorder` |
| `BackordersApi` | `CountPending` |
| `BackordersApi` | `CreateBackorder` |
| `BackordersApi` | `GetBackorder` |
| `BackordersApi` | `GetBackorderByNumber` |
| `BackordersApi` | `GetBackordersForOrder` |
| `BackordersApi` | `GetBackordersForSku` |
| `BackordersApi` | `GetOverdueBackorders` |
| `BackordersApi` | `GetSummary` |
| `BackordersApi` | `ListBackorders` |
| `BomApi` | `Activate` |
| `BomApi` | `AddComponent` |
| `BomApi` | `Create` |
| `BomApi` | `Get` |
| `BomApi` | `GetComponents` |
| `BomApi` | `List` |
| `CartsApi` | `AddItem` |
| `CartsApi` | `Create` |
| `CartsApi` | `Get` |
| `CostAccountingApi` | `GetItemCost` |
| `CostAccountingApi` | `GetTotalInventoryValue` |
| `CostAccountingApi` | `ListItemCosts` |
| `CostAccountingApi` | `SetItemCost` |
| `CostAccountingApi` | `UpdateAverageCost` |
| `CreditApi` | `AdjustCreditLimit` |
| `CreditApi` | `CheckCredit` |
| `CreditApi` | `CreateCreditAccount` |
| `CreditApi` | `GetCreditAccount` |
| `CreditApi` | `GetCreditAccountByCustomer` |
| `CreditApi` | `GetOverLimitCustomers` |
| `CreditApi` | `ListCreditAccounts` |
| `CreditApi` | `ReactivateCreditAccount` |
| `CreditApi` | `SuspendCreditAccount` |
| `CurrencyApi` | `Convert` |
| `CurrencyApi` | `GetRate` |
| `CurrencyApi` | `GetSettings` |
| `CurrencyApi` | `SetRate` |
| `CustomersApi` | `Count` |
| `CustomersApi` | `Create` |
| `CustomersApi` | `Delete` |
| `CustomersApi` | `Get` |
| `CustomersApi` | `List` |
| `FulfillmentApi` | `AssignPick` |
| `FulfillmentApi` | `CancelPick` |
| `FulfillmentApi` | `CancelWave` |
| `FulfillmentApi` | `CompleteWave` |
| `FulfillmentApi` | `CreateWave` |
| `FulfillmentApi` | `GetPick` |
| `FulfillmentApi` | `GetWave` |
| `FulfillmentApi` | `IsOrderReadyToPack` |
| `FulfillmentApi` | `IsOrderReadyToShip` |
| `FulfillmentApi` | `ListPicks` |
| `FulfillmentApi` | `ListWaves` |
| `FulfillmentApi` | `ReleaseWave` |
| `FulfillmentApi` | `StartPick` |
| `GeneralLedgerApi` | `CreateAccount` |
| `GeneralLedgerApi` | `GetAccount` |
| `GeneralLedgerApi` | `GetAccountBalance` |
| `GeneralLedgerApi` | `GetAccountByNumber` |
| `GeneralLedgerApi` | `GetBalanceSheet` |
| `GeneralLedgerApi` | `GetIncomeStatement` |
| `GeneralLedgerApi` | `GetJournalEntry` |
| `GeneralLedgerApi` | `GetTrialBalance` |
| `GeneralLedgerApi` | `InitializeChartOfAccounts` |
| `GeneralLedgerApi` | `ListAccounts` |
| `GeneralLedgerApi` | `ListJournalEntries` |
| `GeneralLedgerApi` | `PostJournalEntry` |
| `GeneralLedgerApi` | `VoidJournalEntry` |
| `InventoryApi` | `Adjust` |
| `InventoryApi` | `CreateItem` |
| `InventoryApi` | `GetLevel` |
| `InvoicesApi` | `Create` |
| `InvoicesApi` | `Get` |
| `InvoicesApi` | `GetOverdue` |
| `InvoicesApi` | `List` |
| `InvoicesApi` | `RecordPayment` |
| `InvoicesApi` | `Send` |
| `InvoicesApi` | `Void` |
| `LotsApi` | `Create` |
| `LotsApi` | `Get` |
| `LotsApi` | `GetActiveLots` |
| `LotsApi` | `GetByNumber` |
| `LotsApi` | `GetExpiredLots` |
| `LotsApi` | `GetExpiringLots` |
| `LotsApi` | `GetQuarantined` |
| `LotsApi` | `List` |
| `LotsApi` | `Quarantine` |
| `LotsApi` | `ReleaseQuarantine` |
| `OrdersApi` | `Cancel` |
| `OrdersApi` | `Create` |
| `OrdersApi` | `Get` |
| `OrdersApi` | `List` |
| `OrdersApi` | `Ship` |
| `OrdersApi` | `UpdateStatus` |
| `PaymentsApi` | `Complete` |
| `PaymentsApi` | `Create` |
| `PaymentsApi` | `Fail` |
| `PaymentsApi` | `Get` |
| `PaymentsApi` | `List` |
| `PaymentsApi` | `Refund` |
| `ProductsApi` | `Create` |
| `ProductsApi` | `Get` |
| `ProductsApi` | `List` |
| `PromotionsApi` | `Activate` |
| `PromotionsApi` | `Create` |
| `PromotionsApi` | `CreateCoupon` |
| `PromotionsApi` | `Deactivate` |
| `PromotionsApi` | `Delete` |
| `PromotionsApi` | `Get` |
| `PromotionsApi` | `GetActive` |
| `PromotionsApi` | `GetByCode` |
| `PromotionsApi` | `GetCouponByCode` |
| `PromotionsApi` | `List` |
| `PromotionsApi` | `ValidateCoupon` |
| `PurchaseOrdersApi` | `Approve` |
| `PurchaseOrdersApi` | `Cancel` |
| `PurchaseOrdersApi` | `Create` |
| `PurchaseOrdersApi` | `Get` |
| `PurchaseOrdersApi` | `List` |
| `PurchaseOrdersApi` | `Send` |
| `PurchaseOrdersApi` | `Submit` |
| `QualityApi` | `CloseNcr` |
| `QualityApi` | `CompleteInspection` |
| `QualityApi` | `CreateHold` |
| `QualityApi` | `CreateInspection` |
| `QualityApi` | `CreateNcr` |
| `QualityApi` | `GetActiveHolds` |
| `QualityApi` | `GetHold` |
| `QualityApi` | `GetInspection` |
| `QualityApi` | `GetNcr` |
| `QualityApi` | `ListHolds` |
| `QualityApi` | `ListInspections` |
| `QualityApi` | `ListNcrs` |
| `QualityApi` | `ReleaseHold` |
| `QualityApi` | `StartInspection` |
| `ReceivingApi` | `CancelReceipt` |
| `ReceivingApi` | `CompleteReceiving` |
| `ReceivingApi` | `CreateReceipt` |
| `ReceivingApi` | `CreateReceiptFromPo` |
| `ReceivingApi` | `GetReceipt` |
| `ReceivingApi` | `GetReceiptByNumber` |
| `ReceivingApi` | `ListReceipts` |
| `ReceivingApi` | `StartReceiving` |
| `ReturnsApi` | `Approve` |
| `ReturnsApi` | `Complete` |
| `ReturnsApi` | `Create` |
| `ReturnsApi` | `Get` |
| `ReturnsApi` | `List` |
| `ReturnsApi` | `Reject` |
| `SerialsApi` | `Create` |
| `SerialsApi` | `Get` |
| `SerialsApi` | `GetAvailable` |
| `SerialsApi` | `GetBySerial` |
| `SerialsApi` | `IsAvailable` |
| `SerialsApi` | `List` |
| `SerialsApi` | `MarkSold` |
| `SerialsApi` | `Quarantine` |
| `ShipmentsApi` | `Cancel` |
| `ShipmentsApi` | `Create` |
| `ShipmentsApi` | `Deliver` |
| `ShipmentsApi` | `Get` |
| `ShipmentsApi` | `List` |
| `ShipmentsApi` | `Ship` |
| `SubscriptionsApi` | `ActivatePlan` |
| `SubscriptionsApi` | `ArchivePlan` |
| `SubscriptionsApi` | `Cancel` |
| `SubscriptionsApi` | `CreatePlan` |
| `SubscriptionsApi` | `Get` |
| `SubscriptionsApi` | `GetPlan` |
| `SubscriptionsApi` | `List` |
| `SubscriptionsApi` | `ListPlans` |
| `SubscriptionsApi` | `Pause` |
| `SubscriptionsApi` | `Resume` |
| `SubscriptionsApi` | `Subscribe` |
| `SuppliersApi` | `Create` |
| `SuppliersApi` | `Get` |
| `SuppliersApi` | `List` |
| `TaxApi` | `Calculate` |
| `TaxApi` | `CreateExemption` |
| `TaxApi` | `CreateJurisdiction` |
| `TaxApi` | `CreateRate` |
| `TaxApi` | `CustomerIsExempt` |
| `TaxApi` | `GetCustomerExemptions` |
| `TaxApi` | `GetEffectiveRate` |
| `TaxApi` | `GetJurisdiction` |
| `TaxApi` | `GetRate` |
| `TaxApi` | `GetSettings` |
| `TaxApi` | `ListJurisdictions` |
| `TaxApi` | `ListRates` |
| `TaxApi` | `SetEnabled` |
| `WarehouseApi` | `CreateLocation` |
| `WarehouseApi` | `CreateWarehouse` |
| `WarehouseApi` | `GetLocation` |
| `WarehouseApi` | `GetPickableLocations` |
| `WarehouseApi` | `GetTotalAvailable` |
| `WarehouseApi` | `GetWarehouse` |
| `WarehouseApi` | `GetWarehouseByCode` |
| `WarehouseApi` | `ListLocations` |
| `WarehouseApi` | `ListWarehouses` |
| `WarrantiesApi` | `ApproveClaim` |
| `WarrantiesApi` | `CompleteClaim` |
| `WarrantiesApi` | `Create` |
| `WarrantiesApi` | `CreateClaim` |
| `WarrantiesApi` | `DenyClaim` |
| `WarrantiesApi` | `Get` |
| `WarrantiesApi` | `List` |
| `WorkOrdersApi` | `Cancel` |
| `WorkOrdersApi` | `Complete` |
| `WorkOrdersApi` | `Create` |
| `WorkOrdersApi` | `Get` |
| `WorkOrdersApi` | `List` |
| `WorkOrdersApi` | `Start` |

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
| `AddCartItemInput` |
| `AgentToolDescriptor` |
| `Analytics` |
| `Bom` |
| `BomApi` |
| `BomComponent` |
| `Cart` |
| `CartAddress` |
| `CartItem` |
| `Carts` |
| `CheckoutResult` |
| `Commerce` |
| `ConversionResult` |
| `create_autogen_tools` |
| `create_callable_registry` |
| `create_crewai_tools` |
| `create_embedded_agent_toolkit` |
| `create_langchain_tools` |
| `create_openai_tools` |
| `create_tool_descriptors` |
| `CreateOrderItemInput` |
| `CreateProductVariantInput` |
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
| `DemandForecast` |
| `EmbeddedAgentToolkit` |
| `EmbeddingStats` |
| `ExchangeRate` |
| `execute_openai_tool_call` |
| `execute_openai_tool_calls` |
| `execute_tool` |
| `execute_tool_calls` |
| `FrameworkToolFactory` |
| `FulfillmentMetrics` |
| `Inventory` |
| `InventoryHealth` |
| `InventoryItem` |
| `InventoryMovement` |
| `Invoice` |
| `Invoices` |
| `LowStockItem` |
| `Order` |
| `OrderItem` |
| `Orders` |
| `OrderStatusBreakdown` |
| `Payment` |
| `Payments` |
| `Product` |
| `ProductPerformance` |
| `Products` |
| `ProductSearchResult` |
| `ProductVariant` |
| `PurchaseOrder` |
| `PurchaseOrders` |
| `Refund` |
| `Reservation` |
| `Return` |
| `ReturnMetrics` |
| `Returns` |
| `RevenueByPeriod` |
| `RevenueForecast` |
| `SalesSummary` |
| `SetExchangeRateInput` |
| `Shipment` |
| `Shipments` |
| `ShippingRate` |
| `StockLevel` |
| `StoreCurrencySettings` |
| `Supplier` |
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
| `TopCustomer` |
| `TopProduct` |
| `VectorSearch` |
| `Warranties` |
| `Warranty` |
| `WarrantyClaim` |
| `WorkOrder` |
| `WorkOrders` |

## Swift Surface Summary

| Metric | Value |
| --- | --- |
| Public types | 80 |
| API types | 31 |
| Facade properties | 31 |
| API methods | 232 |
| Targets | `StateSet`, `StateSetC`, `StateSetTests` |

## Swift Public Types

| Type |
| --- |
| `AccountsPayableAPI` |
| `AccountsReceivableAPI` |
| `AnalyticsAPI` |
| `BackordersAPI` |
| `BillOfMaterials` |
| `BOMAPI` |
| `BOMComponent` |
| `BOMStatus` |
| `Cart` |
| `CartItem` |
| `CartsAPI` |
| `ClaimResolution` |
| `ClaimStatus` |
| `ConversionResult` |
| `CostAccountingAPI` |
| `CreditAPI` |
| `Currency` |
| `CurrencyAPI` |
| `Customer` |
| `CustomersAPI` |
| `ExchangeRate` |
| `FulfillmentAPI` |
| `GeneralLedgerAPI` |
| `InventoryAPI` |
| `InventoryItem` |
| `Invoice` |
| `InvoiceItem` |
| `InvoicesAPI` |
| `InvoiceStatus` |
| `LotsAPI` |
| `Order` |
| `OrderItem` |
| `OrdersAPI` |
| `OrderStatus` |
| `Payment` |
| `PaymentMethod` |
| `PaymentsAPI` |
| `Product` |
| `ProductsAPI` |
| `ProductVariant` |
| `PromotionsAPI` |
| `PurchaseOrder` |
| `PurchaseOrderItem` |
| `PurchaseOrdersAPI` |
| `PurchaseOrderStatus` |
| `QualityAPI` |
| `ReceivingAPI` |
| `Refund` |
| `RefundStatus` |
| `Return` |
| `ReturnReason` |
| `ReturnsAPI` |
| `ReturnStatus` |
| `SalesSummary` |
| `SerialsAPI` |
| `Shipment` |
| `ShipmentsAPI` |
| `ShipmentStatus` |
| `ShippingCarrier` |
| `StateSetCommerce` |
| `StateSetError` |
| `StockLevel` |
| `StoreCurrencySettings` |
| `SubscriptionsAPI` |
| `Supplier` |
| `SuppliersAPI` |
| `TaxAPI` |
| `TimePeriod` |
| `TopCustomer` |
| `TopProduct` |
| `WarehouseAPI` |
| `WarrantiesAPI` |
| `Warranty` |
| `WarrantyClaim` |
| `WarrantyStatus` |
| `WarrantyType` |
| `WorkOrder` |
| `WorkOrderPriority` |
| `WorkOrdersAPI` |
| `WorkOrderStatus` |

## Swift Facade Properties

| Property | Type |
| --- | --- |
| `accountsPayable` | `AccountsPayableAPI` |
| `accountsReceivable` | `AccountsReceivableAPI` |
| `analytics` | `AnalyticsAPI` |
| `backorders` | `BackordersAPI` |
| `bom` | `BOMAPI` |
| `carts` | `CartsAPI` |
| `costAccounting` | `CostAccountingAPI` |
| `credit` | `CreditAPI` |
| `currency` | `CurrencyAPI` |
| `customers` | `CustomersAPI` |
| `fulfillment` | `FulfillmentAPI` |
| `generalLedger` | `GeneralLedgerAPI` |
| `inventory` | `InventoryAPI` |
| `invoices` | `InvoicesAPI` |
| `lots` | `LotsAPI` |
| `orders` | `OrdersAPI` |
| `payments` | `PaymentsAPI` |
| `products` | `ProductsAPI` |
| `promotions` | `PromotionsAPI` |
| `purchaseOrders` | `PurchaseOrdersAPI` |
| `quality` | `QualityAPI` |
| `receiving` | `ReceivingAPI` |
| `returns` | `ReturnsAPI` |
| `serials` | `SerialsAPI` |
| `shipments` | `ShipmentsAPI` |
| `subscriptions` | `SubscriptionsAPI` |
| `suppliers` | `SuppliersAPI` |
| `tax` | `TaxAPI` |
| `warehouse` | `WarehouseAPI` |
| `warranties` | `WarrantiesAPI` |
| `workOrders` | `WorkOrdersAPI` |

## Swift API Methods

| API type | Method |
| --- | --- |
| `AccountsPayableAPI` | `approveBill` |
| `AccountsPayableAPI` | `cancelBill` |
| `AccountsPayableAPI` | `createBill` |
| `AccountsPayableAPI` | `getAgingSummary` |
| `AccountsPayableAPI` | `getBill` |
| `AccountsPayableAPI` | `getBillByNumber` |
| `AccountsPayableAPI` | `getBillsDueSoon` |
| `AccountsPayableAPI` | `getOverdueBills` |
| `AccountsPayableAPI` | `getTotalOutstanding` |
| `AccountsPayableAPI` | `listBills` |
| `AccountsReceivableAPI` | `createCreditMemo` |
| `AccountsReceivableAPI` | `getAgingSummary` |
| `AccountsReceivableAPI` | `getCreditMemo` |
| `AccountsReceivableAPI` | `getDso` |
| `AccountsReceivableAPI` | `getTotalOutstanding` |
| `AccountsReceivableAPI` | `getUnappliedCredits` |
| `AccountsReceivableAPI` | `listCreditMemos` |
| `AccountsReceivableAPI` | `voidCreditMemo` |
| `AnalyticsAPI` | `salesSummary` |
| `AnalyticsAPI` | `topCustomers` |
| `AnalyticsAPI` | `topProducts` |
| `BackordersAPI` | `cancelBackorder` |
| `BackordersAPI` | `countPending` |
| `BackordersAPI` | `createBackorder` |
| `BackordersAPI` | `getBackorder` |
| `BackordersAPI` | `getBackorderByNumber` |
| `BackordersAPI` | `getBackordersForOrder` |
| `BackordersAPI` | `getBackordersForSku` |
| `BackordersAPI` | `getOverdueBackorders` |
| `BackordersAPI` | `getSummary` |
| `BackordersAPI` | `listBackorders` |
| `BOMAPI` | `activate` |
| `BOMAPI` | `addComponent` |
| `BOMAPI` | `create` |
| `BOMAPI` | `get` |
| `BOMAPI` | `getComponents` |
| `BOMAPI` | `list` |
| `CartsAPI` | `addItem` |
| `CartsAPI` | `create` |
| `CartsAPI` | `get` |
| `CostAccountingAPI` | `getItemCost` |
| `CostAccountingAPI` | `getTotalInventoryValue` |
| `CostAccountingAPI` | `listItemCosts` |
| `CostAccountingAPI` | `setItemCost` |
| `CostAccountingAPI` | `updateAverageCost` |
| `CreditAPI` | `adjustCreditLimit` |
| `CreditAPI` | `checkCredit` |
| `CreditAPI` | `createCreditAccount` |
| `CreditAPI` | `getCreditAccount` |
| `CreditAPI` | `getCreditAccountByCustomer` |
| `CreditAPI` | `getOverLimitCustomers` |
| `CreditAPI` | `listCreditAccounts` |
| `CreditAPI` | `reactivateCreditAccount` |
| `CreditAPI` | `suspendCreditAccount` |
| `CurrencyAPI` | `convert` |
| `CurrencyAPI` | `getRate` |
| `CurrencyAPI` | `getSettings` |
| `CurrencyAPI` | `setRate` |
| `CustomersAPI` | `create` |
| `CustomersAPI` | `delete` |
| `CustomersAPI` | `get` |
| `CustomersAPI` | `list` |
| `FulfillmentAPI` | `assignPick` |
| `FulfillmentAPI` | `cancelPick` |
| `FulfillmentAPI` | `cancelWave` |
| `FulfillmentAPI` | `completeWave` |
| `FulfillmentAPI` | `createWave` |
| `FulfillmentAPI` | `getPick` |
| `FulfillmentAPI` | `getWave` |
| `FulfillmentAPI` | `isOrderReadyToPack` |
| `FulfillmentAPI` | `isOrderReadyToShip` |
| `FulfillmentAPI` | `listPicks` |
| `FulfillmentAPI` | `listWaves` |
| `FulfillmentAPI` | `releaseWave` |
| `FulfillmentAPI` | `startPick` |
| `GeneralLedgerAPI` | `createAccount` |
| `GeneralLedgerAPI` | `getAccount` |
| `GeneralLedgerAPI` | `getAccountBalance` |
| `GeneralLedgerAPI` | `getAccountByNumber` |
| `GeneralLedgerAPI` | `getBalanceSheet` |
| `GeneralLedgerAPI` | `getIncomeStatement` |
| `GeneralLedgerAPI` | `getJournalEntry` |
| `GeneralLedgerAPI` | `getTrialBalance` |
| `GeneralLedgerAPI` | `initializeChartOfAccounts` |
| `GeneralLedgerAPI` | `listAccounts` |
| `GeneralLedgerAPI` | `listJournalEntries` |
| `GeneralLedgerAPI` | `postJournalEntry` |
| `GeneralLedgerAPI` | `voidJournalEntry` |
| `InventoryAPI` | `adjust` |
| `InventoryAPI` | `createItem` |
| `InventoryAPI` | `getLevel` |
| `InvoicesAPI` | `create` |
| `InvoicesAPI` | `get` |
| `InvoicesAPI` | `getOverdue` |
| `InvoicesAPI` | `list` |
| `InvoicesAPI` | `recordPayment` |
| `InvoicesAPI` | `send` |
| `InvoicesAPI` | `void` |
| `LotsAPI` | `create` |
| `LotsAPI` | `get` |
| `LotsAPI` | `getActiveLots` |
| `LotsAPI` | `getByNumber` |
| `LotsAPI` | `getExpiredLots` |
| `LotsAPI` | `getExpiringLots` |
| `LotsAPI` | `getQuarantined` |
| `LotsAPI` | `list` |
| `LotsAPI` | `quarantine` |
| `LotsAPI` | `releaseQuarantine` |
| `OrdersAPI` | `cancel` |
| `OrdersAPI` | `create` |
| `OrdersAPI` | `get` |
| `OrdersAPI` | `list` |
| `OrdersAPI` | `ship` |
| `OrdersAPI` | `updateStatus` |
| `PaymentsAPI` | `complete` |
| `PaymentsAPI` | `create` |
| `PaymentsAPI` | `fail` |
| `PaymentsAPI` | `get` |
| `PaymentsAPI` | `list` |
| `PaymentsAPI` | `refund` |
| `ProductsAPI` | `create` |
| `ProductsAPI` | `get` |
| `ProductsAPI` | `list` |
| `PromotionsAPI` | `activate` |
| `PromotionsAPI` | `create` |
| `PromotionsAPI` | `createCoupon` |
| `PromotionsAPI` | `deactivate` |
| `PromotionsAPI` | `delete` |
| `PromotionsAPI` | `get` |
| `PromotionsAPI` | `getActive` |
| `PromotionsAPI` | `getByCode` |
| `PromotionsAPI` | `getCouponByCode` |
| `PromotionsAPI` | `list` |
| `PromotionsAPI` | `validateCoupon` |
| `PurchaseOrdersAPI` | `approve` |
| `PurchaseOrdersAPI` | `cancel` |
| `PurchaseOrdersAPI` | `create` |
| `PurchaseOrdersAPI` | `get` |
| `PurchaseOrdersAPI` | `list` |
| `PurchaseOrdersAPI` | `send` |
| `PurchaseOrdersAPI` | `submit` |
| `QualityAPI` | `closeNcr` |
| `QualityAPI` | `completeInspection` |
| `QualityAPI` | `createHold` |
| `QualityAPI` | `createInspection` |
| `QualityAPI` | `createNcr` |
| `QualityAPI` | `getActiveHolds` |
| `QualityAPI` | `getHold` |
| `QualityAPI` | `getInspection` |
| `QualityAPI` | `getNcr` |
| `QualityAPI` | `listHolds` |
| `QualityAPI` | `listInspections` |
| `QualityAPI` | `listNcrs` |
| `QualityAPI` | `releaseHold` |
| `QualityAPI` | `startInspection` |
| `ReceivingAPI` | `cancelReceipt` |
| `ReceivingAPI` | `completeReceiving` |
| `ReceivingAPI` | `createReceipt` |
| `ReceivingAPI` | `createReceiptFromPo` |
| `ReceivingAPI` | `getReceipt` |
| `ReceivingAPI` | `getReceiptByNumber` |
| `ReceivingAPI` | `listReceipts` |
| `ReceivingAPI` | `startReceiving` |
| `ReturnsAPI` | `approve` |
| `ReturnsAPI` | `complete` |
| `ReturnsAPI` | `create` |
| `ReturnsAPI` | `get` |
| `ReturnsAPI` | `list` |
| `ReturnsAPI` | `reject` |
| `SerialsAPI` | `create` |
| `SerialsAPI` | `get` |
| `SerialsAPI` | `getAvailable` |
| `SerialsAPI` | `getBySerial` |
| `SerialsAPI` | `isAvailable` |
| `SerialsAPI` | `list` |
| `SerialsAPI` | `markSold` |
| `SerialsAPI` | `quarantine` |
| `ShipmentsAPI` | `cancel` |
| `ShipmentsAPI` | `create` |
| `ShipmentsAPI` | `deliver` |
| `ShipmentsAPI` | `get` |
| `ShipmentsAPI` | `list` |
| `ShipmentsAPI` | `ship` |
| `SubscriptionsAPI` | `activatePlan` |
| `SubscriptionsAPI` | `archivePlan` |
| `SubscriptionsAPI` | `cancel` |
| `SubscriptionsAPI` | `createPlan` |
| `SubscriptionsAPI` | `get` |
| `SubscriptionsAPI` | `getPlan` |
| `SubscriptionsAPI` | `list` |
| `SubscriptionsAPI` | `listPlans` |
| `SubscriptionsAPI` | `pause` |
| `SubscriptionsAPI` | `resume` |
| `SubscriptionsAPI` | `subscribe` |
| `SuppliersAPI` | `create` |
| `SuppliersAPI` | `get` |
| `SuppliersAPI` | `list` |
| `TaxAPI` | `calculate` |
| `TaxAPI` | `createExemption` |
| `TaxAPI` | `createJurisdiction` |
| `TaxAPI` | `createRate` |
| `TaxAPI` | `customerIsExempt` |
| `TaxAPI` | `getCustomerExemptions` |
| `TaxAPI` | `getEffectiveRate` |
| `TaxAPI` | `getJurisdiction` |
| `TaxAPI` | `getRate` |
| `TaxAPI` | `getSettings` |
| `TaxAPI` | `listJurisdictions` |
| `TaxAPI` | `listRates` |
| `TaxAPI` | `setEnabled` |
| `WarehouseAPI` | `createLocation` |
| `WarehouseAPI` | `createWarehouse` |
| `WarehouseAPI` | `getLocation` |
| `WarehouseAPI` | `getPickableLocations` |
| `WarehouseAPI` | `getTotalAvailable` |
| `WarehouseAPI` | `getWarehouse` |
| `WarehouseAPI` | `getWarehouseByCode` |
| `WarehouseAPI` | `listLocations` |
| `WarehouseAPI` | `listWarehouses` |
| `WarrantiesAPI` | `approveClaim` |
| `WarrantiesAPI` | `completeClaim` |
| `WarrantiesAPI` | `create` |
| `WarrantiesAPI` | `createClaim` |
| `WarrantiesAPI` | `denyClaim` |
| `WarrantiesAPI` | `get` |
| `WarrantiesAPI` | `list` |
| `WorkOrdersAPI` | `cancel` |
| `WorkOrdersAPI` | `complete` |
| `WorkOrdersAPI` | `create` |
| `WorkOrdersAPI` | `get` |
| `WorkOrdersAPI` | `list` |
| `WorkOrdersAPI` | `start` |

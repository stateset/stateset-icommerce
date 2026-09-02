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
| .NET | NuGet | `StateSet.Embedded` | `1.28.5` | `detailed` | 245 API methods |
| Go | Go modules | `github.com/stateset/stateset-icommerce/bindings/go/stateset` | — | `detailed` | 80 API methods |
| Java | Maven | `com.stateset:embedded` | `1.28.5` | `package-manifest` | manifest coverage |
| Kotlin | Maven | `com.stateset:embedded-kotlin` | `1.28.5` | `package-manifest` | manifest coverage |
| Node.js | npm | `@stateset/embedded` | `1.28.5` | `detailed` | 6 export entrypoints |
| PHP | Composer | `stateset/embedded` | `1.28.5` | `package-manifest` | manifest coverage |
| Python | PyPI | `stateset-embedded` | `1.28.5` | `detailed` | 244 public symbols |
| Ruby | RubyGems | `stateset_embedded` | `1.28.5` | `package-manifest` | manifest coverage |
| Swift | SwiftPM | `StateSet` | — | `detailed` | 71 API methods |
| WASM | npm | `@stateset/embedded-wasm` | `1.28.5` | `package-manifest` | manifest coverage |

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
| Public types | 112 |
| API types | 31 |
| Facade properties | 31 |
| API methods | 245 |
| Target frameworks | `net6.0`, `net7.0`, `net8.0` |

## .NET Public Types

| Type |
| --- |
| `AccountsPayableApi` |
| `AccountsReceivableApi` |
| `AnalyticsApi` |
| `ApAgingSummary` |
| `ArAgingSummary` |
| `Backorder` |
| `BackordersApi` |
| `BackorderSummary` |
| `BalanceSheet` |
| `Bill` |
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
| `Coupon` |
| `CreditAccount` |
| `CreditApi` |
| `CreditCheck` |
| `CreditMemo` |
| `CurrencyApi` |
| `CurrencyCode` |
| `Customer` |
| `CustomersApi` |
| `ExchangeRate` |
| `FulfillmentApi` |
| `GeneralLedgerApi` |
| `GlAccount` |
| `IncomeStatement` |
| `Inspection` |
| `InventoryApi` |
| `InventoryItem` |
| `Invoice` |
| `InvoiceItem` |
| `InvoicesApi` |
| `InvoiceStatus` |
| `ItemCost` |
| `JournalEntry` |
| `Location` |
| `Lot` |
| `LotsApi` |
| `Ncr` |
| `Order` |
| `OrderItem` |
| `OrdersApi` |
| `OrderStatus` |
| `Payment` |
| `PaymentMethod` |
| `PaymentsApi` |
| `PickTask` |
| `Product` |
| `ProductsApi` |
| `ProductVariant` |
| `Promotion` |
| `PromotionsApi` |
| `PurchaseOrder` |
| `PurchaseOrderItem` |
| `PurchaseOrdersApi` |
| `PurchaseOrderStatus` |
| `QualityApi` |
| `QualityHold` |
| `Receipt` |
| `ReceivingApi` |
| `Refund` |
| `RefundStatus` |
| `Return` |
| `ReturnReason` |
| `ReturnsApi` |
| `ReturnStatus` |
| `SalesSummary` |
| `Serial` |
| `SerialsApi` |
| `Shipment` |
| `ShipmentsApi` |
| `ShipmentStatus` |
| `ShippingCarrier` |
| `StateSetCommerce` |
| `StateSetException` |
| `StockLevel` |
| `StoreCurrencySettings` |
| `Subscription` |
| `SubscriptionPlan` |
| `SubscriptionsApi` |
| `Supplier` |
| `SuppliersApi` |
| `TaxApi` |
| `TaxCalculation` |
| `TaxExemption` |
| `TaxJurisdiction` |
| `TaxRate` |
| `TaxSettings` |
| `TimePeriod` |
| `TopCustomer` |
| `TopProduct` |
| `TrialBalance` |
| `Warehouse` |
| `WarehouseApi` |
| `WarrantiesApi` |
| `Warranty` |
| `WarrantyClaim` |
| `WarrantyStatus` |
| `WarrantyType` |
| `Wave` |
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
| `AccountsReceivableApi` | `ListReceivables` |
| `AccountsReceivableApi` | `VoidCreditMemo` |
| `AnalyticsApi` | `GetSalesSummary` |
| `AnalyticsApi` | `GetTopCustomers` |
| `AnalyticsApi` | `GetTopProducts` |
| `AnalyticsApi` | `SalesSummary` |
| `AnalyticsApi` | `TopCustomers` |
| `AnalyticsApi` | `TopProducts` |
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
| `CostAccountingApi` | `ListCostEntries` |
| `CostAccountingApi` | `ListItemCosts` |
| `CostAccountingApi` | `SetItemCost` |
| `CostAccountingApi` | `UpdateAverageCost` |
| `CreditApi` | `AdjustCreditLimit` |
| `CreditApi` | `CheckCredit` |
| `CreditApi` | `CreateCreditAccount` |
| `CreditApi` | `GetCreditAccount` |
| `CreditApi` | `GetCreditAccountByCustomer` |
| `CreditApi` | `GetCreditLimit` |
| `CreditApi` | `GetOverLimitCustomers` |
| `CreditApi` | `ListCreditAccounts` |
| `CreditApi` | `ReactivateCreditAccount` |
| `CreditApi` | `SetCreditLimit` |
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
| `FulfillmentApi` | `ListPickLists` |
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
| `LotsApi` | `CreateLot` |
| `LotsApi` | `Get` |
| `LotsApi` | `GetActiveLots` |
| `LotsApi` | `GetByNumber` |
| `LotsApi` | `GetExpiredLots` |
| `LotsApi` | `GetExpiringLots` |
| `LotsApi` | `GetQuarantined` |
| `LotsApi` | `List` |
| `LotsApi` | `ListLots` |
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
| `SerialsApi` | `ListSerials` |
| `SerialsApi` | `MarkSold` |
| `SerialsApi` | `Quarantine` |
| `SerialsApi` | `RegisterSerial` |
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
| Public types | 73 |
| API types | 31 |
| Facade properties | 31 |
| API methods | 71 |
| Targets | `StateSet`, `StateSetC`, `StateSetTests` |

## Swift Public Types

| Type |
| --- |
| `AccountsPayableAPI` |
| `AccountsReceivableAPI` |
| `AnalyticsAPI` |
| `BackordersAPI` |
| `BOMAPI` |
| `Cart` |
| `CartItem` |
| `CartsAPI` |
| `ConversionResult` |
| `CostAccountingAPI` |
| `Coupon` |
| `CreditAPI` |
| `Currency` |
| `CurrencyAPI` |
| `Customer` |
| `CustomersAPI` |
| `ExchangeRate` |
| `FulfillmentAPI` |
| `GeneralLedgerAPI` |
| `GlAccount` |
| `InventoryAPI` |
| `InventoryItem` |
| `InvoicesAPI` |
| `Location` |
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
| `Promotion` |
| `PromotionsAPI` |
| `PurchaseOrdersAPI` |
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
| `Subscription` |
| `SubscriptionPlan` |
| `SubscriptionsAPI` |
| `SuppliersAPI` |
| `TaxAPI` |
| `TaxCalculation` |
| `TaxExemption` |
| `TaxJurisdiction` |
| `TaxRate` |
| `TaxSettings` |
| `TimePeriod` |
| `TopCustomer` |
| `TopProduct` |
| `Warehouse` |
| `WarehouseAPI` |
| `WarrantiesAPI` |
| `WorkOrdersAPI` |

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
| `AnalyticsAPI` | `salesSummary` |
| `AnalyticsAPI` | `topCustomers` |
| `AnalyticsAPI` | `topProducts` |
| `CartsAPI` | `create` |
| `CurrencyAPI` | `convert` |
| `CurrencyAPI` | `getRate` |
| `CurrencyAPI` | `getSettings` |
| `CurrencyAPI` | `setRate` |
| `CustomersAPI` | `create` |
| `CustomersAPI` | `delete` |
| `CustomersAPI` | `get` |
| `CustomersAPI` | `list` |
| `GeneralLedgerAPI` | `createAccount` |
| `GeneralLedgerAPI` | `listAccounts` |
| `InventoryAPI` | `adjust` |
| `InventoryAPI` | `createItem` |
| `InventoryAPI` | `getLevel` |
| `OrdersAPI` | `cancel` |
| `OrdersAPI` | `create` |
| `OrdersAPI` | `get` |
| `OrdersAPI` | `list` |
| `OrdersAPI` | `ship` |
| `OrdersAPI` | `updateStatus` |
| `PaymentsAPI` | `create` |
| `PaymentsAPI` | `get` |
| `PaymentsAPI` | `list` |
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
| `ReturnsAPI` | `approve` |
| `ReturnsAPI` | `complete` |
| `ReturnsAPI` | `create` |
| `ReturnsAPI` | `get` |
| `ReturnsAPI` | `list` |
| `ReturnsAPI` | `reject` |
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
| `TaxAPI` | `calculate` |
| `TaxAPI` | `getEffectiveRate` |
| `TaxAPI` | `getSettings` |
| `TaxAPI` | `setEnabled` |
| `WarehouseAPI` | `createWarehouse` |
| `WarehouseAPI` | `getWarehouse` |
| `WarehouseAPI` | `getWarehouseByCode` |
| `WarehouseAPI` | `listWarehouses` |

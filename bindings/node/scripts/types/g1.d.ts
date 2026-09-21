// Literal unions and payload shapes for the customers, orders, products,
// custom objects, inventory, returns, payments, shipments, warranties,
// purchase orders, invoices, BOM, work orders, carts, analytics and currency
// APIs. Referenced from Rust through `#[napi(ts_type = "...")]`.
//
// Every value is the exact wire string: output unions are what the engine's
// `Display` renders (strum `snake_case`; where a variant carries several
// spellings the longest one is rendered), input unions are every spelling the
// binding's parser accepts. Parsers are ASCII case-insensitive; the lowercase
// canonical forms are listed.

// ---- customers -------------------------------------------------------------

/** Customer account status (`CustomerOutput.status`, `UpdateCustomerInput.status`, `CustomerFilterInput.status`). */
export type CustomerStatus = 'active' | 'inactive' | 'suspended' | 'deleted'

/** Which role(s) an address serves (`CustomerAddressOutput.addressType`, `CreateCustomerAddressInput.addressType`, `setDefaultAddress`). */
export type AddressType = 'shipping' | 'billing' | 'both'

// ---- orders ----------------------------------------------------------------

/** Order lifecycle status as rendered on `OrderOutput.status` and accepted by `OrderFilterInput.status`. */
export type OrderStatus =
  | 'pending'
  | 'confirmed'
  | 'processing'
  | 'partially_shipped'
  | 'shipped'
  | 'delivered'
  | 'cancelled'
  | 'refunded'

/** The statuses `orders.updateStatus` accepts (`partially_shipped` is derived by `ship` and cannot be set directly). */
export type OrderStatusUpdate =
  | 'pending'
  | 'confirmed'
  | 'processing'
  | 'shipped'
  | 'delivered'
  | 'cancelled'
  | 'refunded'

/** Order-level payment status (`OrderOutput.paymentStatus`, `OrderFilterInput.paymentStatus`). */
export type PaymentStatus =
  | 'pending'
  | 'authorized'
  | 'paid'
  | 'partially_paid'
  | 'refunded'
  | 'partially_refunded'
  | 'failed'

/** Order-level fulfillment status (`OrderOutput.fulfillmentStatus`, `OrderFilterInput.fulfillmentStatus`). */
export type FulfillmentStatus =
  | 'unfulfilled'
  | 'partially_fulfilled'
  | 'fulfilled'
  | 'shipped'
  | 'delivered'

/** What order creation does when a line cannot be fully reserved (`CreateOrderInput.stockPolicy`); hyphenated spellings are accepted too. */
export type StockPolicy =
  | 'allow_backorder'
  | 'allow-backorder'
  | 'reject_if_insufficient'
  | 'reject-if-insufficient'

// ---- products --------------------------------------------------------------

/** Catalogue status (`ProductOutput.status`, `UpdateProductInput.status`, `ProductFilterInput.status`). */
export type ProductStatus = 'draft' | 'active' | 'archived'

// ---- custom objects --------------------------------------------------------

/** Custom object field type as rendered on `CustomFieldDefinitionOutput.fieldType`. */
export type CustomFieldType = 'string' | 'integer' | 'decimal' | 'boolean' | 'date_time' | 'uuid' | 'json'

/** Every spelling `CustomFieldDefinitionInput.fieldType` accepts (the short aliases normalise to the canonical form). */
export type CustomFieldTypeInput = CustomFieldType | 'int' | 'number' | 'bool' | 'datetime'

// ---- inventory -------------------------------------------------------------

/** Inventory reservation status (`ReservationOutput.status`). */
export type ReservationStatus =
  | 'pending'
  | 'confirmed'
  | 'allocated'
  | 'cancelled'
  | 'released'
  | 'expired'
  | 'fulfilled'

// ---- returns ---------------------------------------------------------------

/** Return lifecycle status (`ReturnOutput.status`, `ReturnFilterInput.status`). */
export type ReturnStatus =
  | 'requested'
  | 'approved'
  | 'rejected'
  | 'in_transit'
  | 'received'
  | 'inspecting'
  | 'completed'
  | 'cancelled'

/** Why a return was requested (`ReturnOutput.reason`, `CreateReturnInput.reason`, `ReturnFilterInput.reason`); an unrecognised reason is stored as `other`. */
export type ReturnReason =
  | 'defective'
  | 'wrong_item'
  | 'not_as_described'
  | 'changed_mind'
  | 'better_price_found'
  | 'no_longer_needed'
  | 'damaged'
  | 'other'

// ---- payments --------------------------------------------------------------

/** Payment processing status (`PaymentOutput.status`, `PaymentFilterInput.status`). */
export type PaymentTransactionStatus =
  | 'pending'
  | 'processing'
  | 'requires_action'
  | 'completed'
  | 'failed'
  | 'cancelled'
  | 'refunded'
  | 'partially_refunded'
  | 'disputed'

/** Payment method accepted by `CreatePaymentInput.paymentMethod` and `PaymentFilterInput.paymentMethod`, aliases included (`ach`, `cod`, `usdc`, ...). */
export type PaymentMethodType =
  | 'credit_card'
  | 'debit_card'
  | 'bank_transfer'
  | 'ach'
  | 'paypal'
  | 'apple_pay'
  | 'google_pay'
  | 'crypto'
  | 'cryptocurrency'
  | 'stablecoin'
  | 'usdc'
  | 'usdt'
  | 'ssusd'
  | 'store_credit'
  | 'gift_card'
  | 'cash_on_delivery'
  | 'cod'
  | 'invoice'
  | 'other'

/** Refund processing status (`RefundOutput.status`). */
export type RefundStatus = 'pending' | 'processing' | 'completed' | 'failed' | 'cancelled'

// ---- shipments -------------------------------------------------------------

/** Shipment lifecycle status (`ShipmentOutput.status`, `ShipmentFilterInput.status`). */
export type ShipmentStatus =
  | 'pending'
  | 'processing'
  | 'ready_to_ship'
  | 'shipped'
  | 'in_transit'
  | 'out_for_delivery'
  | 'delivered'
  | 'failed'
  | 'returned'
  | 'cancelled'
  | 'on_hold'

/** Carrier as rendered on `ShipmentOutput.carrier` (the engine renders the underscored spelling of multi-word carriers). */
export type ShippingCarrier = 'other' | 'ups' | 'fed_ex' | 'usps' | 'dhl' | 'on_trac' | 'laser_ship'

/** Carriers `CreateShipmentInput.carrier` recognises; anything else is stored as `other`. */
export type ShippingCarrierInput = 'ups' | 'fedex' | 'usps' | 'dhl' | 'other'

/** Carrier spellings `ShipmentFilterInput.carrier` accepts. */
export type ShippingCarrierFilter = ShippingCarrier | 'fedex' | 'ontrac' | 'lasership'

/** Shipping service level as rendered on `ShipmentOutput.shippingMethod`. */
export type ShipmentMethod =
  | 'standard'
  | 'express'
  | 'overnight'
  | 'two_day'
  | 'ground'
  | 'international'
  | 'same_day'
  | 'freight'

/** Every spelling `CreateShipmentInput.shippingMethod` accepts; anything else is refused with `VALIDATION`. */
export type ShipmentMethodInput = ShipmentMethod | 'twoday' | 'sameday'

// ---- warranties ------------------------------------------------------------

/** Warranty status (`WarrantyOutput.status`, `WarrantyFilterInput.status`). */
export type WarrantyStatus = 'active' | 'expired' | 'voided' | 'transferred'

/** Warranty tier as rendered on `WarrantyOutput.warrantyType` and accepted by `WarrantyFilterInput.warrantyType`. */
export type WarrantyType = 'standard' | 'extended' | 'limited' | 'lifetime' | 'accidental_damage' | 'comprehensive'

/** Tiers `CreateWarrantyInput.warrantyType` recognises; anything else uses the engine default (`standard`). */
export type WarrantyTypeInput = 'standard' | 'extended' | 'limited' | 'lifetime'

/** Warranty claim status (`WarrantyClaimOutput.status`). */
export type WarrantyClaimStatus =
  | 'submitted'
  | 'under_review'
  | 'info_requested'
  | 'approved'
  | 'denied'
  | 'in_progress'
  | 'completed'
  | 'cancelled'

/** How a warranty claim was resolved (`WarrantyClaimOutput.resolution`). */
export type WarrantyClaimResolution = 'none' | 'repair' | 'replacement' | 'refund' | 'store_credit' | 'denied'

/** Resolutions `warranties.completeClaim` accepts; anything else records `none`. */
export type WarrantyClaimResolutionInput = WarrantyClaimResolution | 'storecredit'

// ---- purchase orders -------------------------------------------------------

/** Purchase order status (`PurchaseOrderOutput.status`, `PurchaseOrderFilterInput.status`). */
export type PurchaseOrderStatus =
  | 'draft'
  | 'pending_approval'
  | 'approved'
  | 'sent'
  | 'acknowledged'
  | 'partially_received'
  | 'received'
  | 'completed'
  | 'cancelled'
  | 'on_hold'

// ---- invoices --------------------------------------------------------------

/** Invoice status (`InvoiceOutput.status`, `InvoiceFilterInput.status`). */
export type InvoiceStatus =
  | 'draft'
  | 'sent'
  | 'viewed'
  | 'partially_paid'
  | 'paid'
  | 'overdue'
  | 'voided'
  | 'written_off'
  | 'disputed'

// ---- bill of materials -----------------------------------------------------

/** BOM status (`BomOutput.status`, `BomFilterInput.status`). */
export type BomStatus = 'draft' | 'active' | 'obsolete'

// ---- work orders -----------------------------------------------------------

/** Work order status (`WorkOrderOutput.status`, `WorkOrderFilterInput.status`). */
export type WorkOrderStatus =
  | 'planned'
  | 'in_progress'
  | 'completed'
  | 'partially_completed'
  | 'cancelled'
  | 'on_hold'

/** Work order priority (`WorkOrderOutput.priority`, `CreateWorkOrderInput.priority`, `WorkOrderFilterInput.priority`). */
export type WorkOrderPriority = 'low' | 'normal' | 'high' | 'urgent'

// ---- carts -----------------------------------------------------------------

/** Cart lifecycle status (`CartOutput.status`, `CartFilterInput.status`). */
export type CartStatus =
  | 'active'
  | 'ready_for_payment'
  | 'payment_pending'
  | 'completed'
  | 'abandoned'
  | 'cancelled'
  | 'expired'

/** Cart-level payment progress (`CartOutput.paymentStatus`). */
export type CartPaymentStatus = 'none' | 'method_selected' | 'authorized' | 'captured' | 'failed' | 'refunded'

/** How a cart will be fulfilled (`CartOutput.fulfillmentType`; the engine renders pickup as `pick-up`). */
export type FulfillmentType = 'shipping' | 'pick-up' | 'digital'

// ---- analytics -------------------------------------------------------------

/** Reporting window accepted by `AnalyticsQueryInput.period`; an unrecognised value is refused with `VALIDATION`. */
export type AnalyticsPeriod =
  | 'today'
  | 'yesterday'
  | 'last7days'
  | 'last_7_days'
  | 'last30days'
  | 'last_30_days'
  | 'this_month'
  | 'thismonth'
  | 'last_month'
  | 'lastmonth'
  | 'this_quarter'
  | 'thisquarter'
  | 'last_quarter'
  | 'lastquarter'
  | 'this_year'
  | 'thisyear'
  | 'last_year'
  | 'lastyear'
  | 'all_time'
  | 'alltime'
  | 'all'

/** Bucket size accepted by `AnalyticsQueryInput.granularity` and `revenueForecast`; an unrecognised value is refused with `VALIDATION`. */
export type AnalyticsGranularity =
  | 'hour'
  | 'hourly'
  | 'day'
  | 'daily'
  | 'week'
  | 'weekly'
  | 'month'
  | 'monthly'
  | 'quarter'
  | 'quarterly'
  | 'year'
  | 'yearly'

/** Demand direction on `DemandForecastOutput.trend` (rendered in the engine's `Debug` casing). */
export type DemandTrend = 'Rising' | 'Stable' | 'Falling'

// ---- currency --------------------------------------------------------------

/** Conversion rounding mode (`StoreCurrencySettingsOutput.roundingMode`, `StoreCurrencySettingsInput.roundingMode`); an unrecognised input is refused with `VALIDATION`. */
export type RoundingMode = 'half_up' | 'half_down' | 'up' | 'down' | 'half_even'

// ---- payloads --------------------------------------------------------------

/** Free-form JSON metadata attached to a customer. Any JSON object; the engine stores it verbatim. */
export type CustomerMetadata = Record<string, unknown>

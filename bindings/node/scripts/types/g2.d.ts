// Literal unions for the group-2 domains: subscriptions, promotions, tax,
// quality, lots, serials, warehouse, receiving, fulfillment, accounts payable /
// receivable, cost accounting, credit, backorders, general ledger, x402,
// vector search, VES crypto, gift cards, store credits, reviews, wishlists,
// segments and loyalty.
//
// Every value below is the exact wire string: OUTPUT unions list what the
// binding renders (several of these domains render the Rust `Debug` form, so
// they are PascalCase), INPUT unions list every spelling the parser accepts.
// Where the sets coincide one name serves both directions.

// ---------------------------------------------------------------------------
// Lots / serials
// ---------------------------------------------------------------------------

/** Lot status as rendered on `LotOutput.status` (Rust `Debug` form). */
export type LotStatus = 'Active' | 'Quarantine' | 'Expired' | 'Consumed' | 'OnHold' | 'Recalled' | 'Scrapped'
/** Lot status accepted by `LotFilterInput.status`: the rendered form or the engine's snake_case. */
export type LotStatusInput = LotStatus | 'active' | 'quarantine' | 'expired' | 'consumed' | 'on_hold' | 'recalled' | 'scrapped'

/** Serial-number status as rendered on `SerialOutput.status` (Rust `Debug` form). */
export type SerialStatus = 'InProduction' | 'Available' | 'Reserved' | 'Shipped' | 'Sold' | 'Returned' | 'InService' | 'InWarranty' | 'Quarantined' | 'Scrapped' | 'Recalled' | 'Lost' | 'Transferred'
/** Serial-number status accepted by `SerialFilterInput.status`: the rendered form or the engine's snake_case. */
export type SerialStatusInput = SerialStatus | 'in_production' | 'available' | 'reserved' | 'shipped' | 'sold' | 'returned' | 'in_service' | 'in_warranty' | 'quarantined' | 'scrapped' | 'recalled' | 'lost' | 'transferred'

// ---------------------------------------------------------------------------
// Quality
// ---------------------------------------------------------------------------

/** Inspection type as rendered on `InspectionOutput.inspectionType` (Rust `Debug` form). */
export type InspectionType = 'Incoming' | 'Receiving' | 'InProcess' | 'Final' | 'Random' | 'Return'
/** Inspection type accepted by `CreateInspectionInput.inspectionType` (case-insensitive). */
export type InspectionTypeInput = 'incoming' | 'receiving' | 'in_process' | 'inprocess' | 'final' | 'random' | 'return'
/** Inspection status as rendered on `InspectionOutput.status` (Rust `Debug` form). */
export type InspectionStatus = 'Pending' | 'Scheduled' | 'InProgress' | 'Passed' | 'Failed' | 'PartialPass' | 'OnHold' | 'Cancelled'
/** Non-conformance source as rendered on `NcrOutput.source` (Rust `Debug` form). */
export type NcrSource = 'Inspection' | 'CustomerComplaint' | 'InternalAudit' | 'SupplierIssue' | 'ProductionDefect' | 'ShippingDamage'
/** Non-conformance source accepted by `CreateNcrInput.source` (case-insensitive). */
export type NcrSourceInput = 'inspection' | 'production' | 'production_defect' | 'customer' | 'customer_complaint' | 'supplier' | 'supplier_issue' | 'internal_audit' | 'shipping_damage'
/** Non-conformance severity as rendered on `NcrOutput.severity` (Rust `Debug` form). */
export type NcrSeverity = 'Critical' | 'Major' | 'Minor' | 'Observation'
/** Non-conformance severity accepted by `CreateNcrInput.severity` (case-insensitive). */
export type NcrSeverityInput = 'critical' | 'major' | 'minor' | 'observation'
/** Non-conformance report status as rendered on `NcrOutput.status` (Rust `Debug` form). */
export type NcrStatus = 'Open' | 'UnderReview' | 'PendingDisposition' | 'CorrectiveAction' | 'PreventiveAction' | 'Verification' | 'Closed' | 'Cancelled'
/** Quality hold type as rendered on `QualityHoldOutput.holdType` (Rust `Debug` form). */
export type QualityHoldType = 'QualityInspection' | 'CustomerReturn' | 'Recall' | 'Damaged' | 'Expired' | 'Quarantine' | 'RegulatoryHold' | 'InvestigationHold'
/** Quality hold type accepted by `CreateQualityHoldInput.holdType` (case-insensitive). */
export type QualityHoldTypeInput = 'quality_inspection' | 'qualityinspection' | 'damage' | 'damaged' | 'regulatory' | 'regulatory_hold' | 'customer_return' | 'customerreturn' | 'recall' | 'expired' | 'quarantine' | 'investigation' | 'investigation_hold'
/** Quality hold type accepted by `QualityHoldFilterInput.holdType`: the rendered form or the engine's snake_case (strict). */
export type QualityHoldTypeFilter = QualityHoldType | 'quality_inspection' | 'customer_return' | 'recall' | 'damaged' | 'expired' | 'quarantine' | 'regulatory_hold' | 'investigation_hold'
/** Whether a quality hold is still in force, as rendered on `QualityHoldOutput.status`. */
export type QualityHoldStatus = 'held' | 'released'

// ---------------------------------------------------------------------------
// Warehouse / receiving / fulfillment
// ---------------------------------------------------------------------------

/** Warehouse type as rendered on `WarehouseOutput.warehouseType` (Rust `Debug` form). */
export type WarehouseType = 'Distribution' | 'Manufacturing' | 'Retail' | 'ThirdParty' | 'Consignment' | 'Returns'
/** Warehouse type accepted by `CreateWarehouseInput.warehouseType` (case-insensitive). */
export type WarehouseTypeInput = 'distribution' | 'manufacturing' | 'retail' | 'thirdparty' | 'third_party'
/** Warehouse type accepted by `WarehouseFilterInput.warehouseType`: the rendered form or the engine's snake_case (strict). */
export type WarehouseTypeFilter = WarehouseType | 'distribution' | 'manufacturing' | 'retail' | 'third_party' | 'thirdparty' | 'consignment' | 'returns'
/** Warehouse location type as rendered on `LocationOutput.locationType` (Rust `Debug` form). */
export type WarehouseLocationType = 'Bulk' | 'Pick' | 'Staging' | 'Receiving' | 'Shipping' | 'Quarantine' | 'Returns' | 'Production' | 'Packing' | 'CrossDock'
/** Warehouse location type accepted by `CreateLocationInput.locationType` (case-insensitive). */
export type WarehouseLocationTypeInput = 'pick' | 'bulk' | 'receiving' | 'shipping' | 'staging' | 'quarantine' | 'returns'

/** Receipt type as rendered on `ReceiptOutput.receiptType` (Rust `Debug` form). */
export type ReceiptType = 'PurchaseOrder' | 'Transfer' | 'Return' | 'Adjustment' | 'Production' | 'Other'
/** Receipt type accepted by `CreateReceiptInput.receiptType` (case-insensitive). */
export type ReceiptTypeInput = 'purchase_order' | 'purchaseorder' | 'po' | 'return' | 'customer_return' | 'transfer' | 'adjustment'
/** Receipt type accepted by `ReceiptFilterInput.receiptType`: the rendered form or the engine's snake_case (strict). */
export type ReceiptTypeFilter = ReceiptType | 'purchase_order' | 'purchaseorder' | 'po' | 'transfer' | 'return' | 'returns' | 'adjustment' | 'production' | 'other'
/** Receipt status as rendered on `ReceiptOutput.status` (Rust `Debug` form). */
export type ReceiptStatus = 'Expected' | 'InProgress' | 'Received' | 'Inspecting' | 'PuttingAway' | 'Completed' | 'Cancelled'
/** Receipt status accepted by `ReceiptFilterInput.status`: the rendered form or the engine's snake_case (strict). */
export type ReceiptStatusInput = ReceiptStatus | 'expected' | 'in_progress' | 'received' | 'inspecting' | 'putting_away' | 'completed' | 'cancelled' | 'canceled'

/** Wave status as rendered on `WaveOutput.status` (Rust `Debug` form). */
export type WaveStatus = 'Draft' | 'Released' | 'InProgress' | 'Completed' | 'Cancelled'
/** Wave status accepted by `WaveFilterInput.status`: the rendered form or the engine's snake_case (strict). */
export type WaveStatusInput = WaveStatus | 'draft' | 'released' | 'in_progress' | 'completed' | 'cancelled' | 'canceled'
/** Pick task status as rendered on `PickTaskOutput.status` (Rust `Debug` form). */
export type PickTaskStatus = 'Pending' | 'Assigned' | 'InProgress' | 'Completed' | 'Short' | 'Cancelled'
/** Pick task status accepted by `PickTaskFilterInput.status`: the rendered form or the engine's snake_case (strict). */
export type PickTaskStatusInput = PickTaskStatus | 'pending' | 'assigned' | 'in_progress' | 'completed' | 'short' | 'cancelled' | 'canceled'

// ---------------------------------------------------------------------------
// Finance: AP / AR / cost accounting / credit / backorders / general ledger
// ---------------------------------------------------------------------------

/** Bill status as rendered on `BillOutput.status` (Rust `Debug` form). */
export type BillStatus = 'Draft' | 'Pending' | 'Approved' | 'PartiallyPaid' | 'Paid' | 'Overdue' | 'Cancelled' | 'Disputed'
/** Bill status accepted by `BillFilterInput.status`: the rendered form or the engine's snake_case (strict). */
export type BillStatusInput = BillStatus | 'draft' | 'pending' | 'approved' | 'partially_paid' | 'paid' | 'overdue' | 'cancelled' | 'canceled' | 'disputed'
/** Three-way match outcome on `ThreeWayMatchOutput.matchStatus`. */
export type ThreeWayMatchStatus = 'not_required' | 'pending' | 'matched' | 'variance' | 'unknown'

/** Credit memo status as rendered on `CreditMemoOutput.status` (Rust `Debug` form). */
export type CreditMemoStatus = 'Open' | 'PartiallyApplied' | 'FullyApplied' | 'Voided'
/** Credit memo status accepted by `CreditMemoFilterInput.status`: the rendered form or the engine's snake_case (strict). */
export type CreditMemoStatusInput = CreditMemoStatus | 'open' | 'partially_applied' | 'fully_applied' | 'voided'
/** Credit memo reason as rendered on `CreditMemoOutput.reason` (Rust `Debug` form). */
export type CreditMemoReason = 'ReturnedGoods' | 'PricingError' | 'Overpayment' | 'Damaged' | 'ServiceCredit' | 'GoodwillAdjustment' | 'Other'
/** Credit memo reason accepted by `CreateCreditMemoInput.reason` (case-insensitive). */
export type CreditMemoReasonInput = 'returned_goods' | 'returnedgoods' | 'return' | 'pricing_error' | 'pricingerror' | 'billing_error' | 'overpayment' | 'damaged' | 'service_credit' | 'servicecredit' | 'goodwill' | 'goodwill_adjustment' | 'other'
/** Credit memo reason accepted by `CreditMemoFilterInput.reason`: the rendered form or the engine's snake_case (strict). */
export type CreditMemoReasonFilter = CreditMemoReason | 'returned_goods' | 'pricing_error' | 'overpayment' | 'damaged' | 'service_credit' | 'goodwill_adjustment' | 'other'

/** Inventory costing method as rendered on `ItemCostOutput.costMethod` (Rust `Debug` form). */
export type CostMethod = 'Average' | 'Fifo' | 'Lifo' | 'Standard' | 'Specific'
/** Costing method accepted by `SetItemCostInput.costMethod` (case-insensitive). */
export type CostMethodInput = 'standard' | 'average' | 'fifo' | 'lifo'
/** Costing method accepted by `ItemCostFilterInput.costMethod`: the rendered form or the engine's snake_case (strict). */
export type CostMethodFilter = CostMethod | 'average' | 'avg' | 'fifo' | 'lifo' | 'standard' | 'std' | 'specific'

/** Customer credit account status as rendered on `CreditAccountOutput.status` (Rust `Debug` form). */
export type CreditAccountStatus = 'Active' | 'Suspended' | 'OnHold' | 'Closed' | 'PendingReview'
/** Credit account status accepted by `CreditAccountFilterInput.status`: the rendered form or the engine's snake_case (strict). */
export type CreditAccountStatusInput = CreditAccountStatus | 'active' | 'suspended' | 'on_hold' | 'onhold' | 'closed' | 'pending_review' | 'pendingreview'

/** Backorder fulfilment source as rendered on `BackorderFulfillmentOutput.sourceType` (Rust `Debug` form). */
export type BackorderFulfillmentSource = 'Inventory' | 'PurchaseOrder' | 'Transfer' | 'Production'
/** Fulfilment source accepted by `FulfillBackorderInput.sourceType`: the rendered form or the engine's snake_case (strict). */
export type BackorderFulfillmentSourceInput = BackorderFulfillmentSource | 'inventory' | 'purchase_order' | 'transfer' | 'production'

/** Backorder allocation status as rendered on `BackorderAllocationOutput.status` (Rust `Debug` form). */
export type BackorderAllocationStatus = 'Reserved' | 'Confirmed' | 'Released' | 'Expired' | 'Fulfilled'

/** Backorder status as rendered on `BackorderOutput.status` (Rust `Debug` form). */
export type BackorderStatus = 'Pending' | 'PartiallyFulfilled' | 'Allocated' | 'ReadyToShip' | 'Fulfilled' | 'Cancelled'
/** Backorder status accepted by `BackorderFilterInput.status`: the rendered form or the engine's snake_case (strict). */
export type BackorderStatusInput = BackorderStatus | 'pending' | 'partially_fulfilled' | 'allocated' | 'ready_to_ship' | 'fulfilled' | 'cancelled' | 'canceled'
/** Backorder priority as rendered on `BackorderOutput.priority` (Rust `Debug` form). */
export type BackorderPriority = 'Low' | 'Normal' | 'High' | 'Critical'
/** Backorder priority accepted by `CreateBackorderInput.priority` (case-insensitive). */
export type BackorderPriorityInput = 'critical' | 'high' | 'normal' | 'low'
/** Backorder priority accepted by `BackorderFilterInput.priority`: the rendered form or lowercase (strict). */
export type BackorderPriorityFilter = BackorderPriority | BackorderPriorityInput

/** General-ledger account type as rendered on `GlAccountOutput.accountType` (Rust `Debug` form). */
export type GlAccountType = 'Asset' | 'Liability' | 'Equity' | 'Revenue' | 'Expense'
/** Account type accepted by `CreateGlAccountInput.accountType` (case-insensitive). */
export type GlAccountTypeInput = 'asset' | 'liability' | 'equity' | 'revenue' | 'expense'
/** Account type accepted by `GlAccountFilterInput.accountType`: the rendered form or lowercase (strict). */
export type GlAccountTypeFilter = GlAccountType | GlAccountTypeInput
/** General-ledger account status as rendered on `GlAccountOutput.status` (Rust `Debug` form). */
export type GlAccountStatus = 'Active' | 'Inactive' | 'Archived'
/** Account status accepted by `GlAccountFilterInput.status`: the rendered form or lowercase (strict). */
export type GlAccountStatusInput = GlAccountStatus | 'active' | 'inactive' | 'archived'
/** Journal entry status as rendered on `JournalEntryOutput.status` (Rust `Debug` form). */
export type GlJournalEntryStatus = 'Draft' | 'Pending' | 'Posted' | 'Voided' | 'Reversed'
/** Journal entry status accepted by `JournalEntryFilterInput.status`: the rendered form or lowercase (strict). */
export type GlJournalEntryStatusInput = GlJournalEntryStatus | 'draft' | 'pending' | 'posted' | 'voided' | 'reversed'
/** Accounting period status (`GlPeriodOutput.status`, `CloseMonthReportOutput.periodStatus`, `GlPeriodFilterInput.status`). */
export type GlPeriodStatus = 'future' | 'open' | 'closed' | 'locked'
/** Outcome of one month-end close step on `CloseMonthStepOutput.status`. */
export type CloseMonthStepStatus = 'executed' | 'skipped' | 'dry_run'
/** Normal balance side on `RevaluationLineOutput.normalBalance`. */
export type GlBalanceSide = 'debit' | 'credit' | 'unknown'

// ---------------------------------------------------------------------------
// Subscriptions
// ---------------------------------------------------------------------------

/** Subscription plan status (`SubscriptionPlanOutput.status`, `SubscriptionPlanFilterInput.status`). */
export type SubscriptionPlanStatus = 'draft' | 'active' | 'archived'
/** Billing interval as rendered on `SubscriptionPlanOutput` / `SubscriptionOutput` `.billingInterval`. */
export type SubscriptionBillingInterval = 'weekly' | 'bi-weekly' | 'monthly' | 'bi-monthly' | 'quarterly' | 'semi-annual' | 'yearly' | 'custom'
/** Billing interval accepted on input (`CreateSubscriptionPlanInput`, `SubscriptionPlanFilterInput`); case-insensitive. */
export type SubscriptionBillingIntervalInput = 'weekly' | 'biweekly' | 'monthly' | 'bimonthly' | 'quarterly' | 'semiannual' | 'annual' | 'custom'
/** Subscription status, shared by `SubscriptionOutput.status` and the update/filter inputs (case-insensitive on input). */
export type SubscriptionStatus = 'pending' | 'trial' | 'active' | 'paused' | 'past_due' | 'cancelled' | 'expired'
/** Billing cycle status, shared by `BillingCycleOutput.status` and `BillingCycleFilterInput.status` (case-insensitive on input). */
export type BillingCycleStatus = 'scheduled' | 'processing' | 'paid' | 'failed' | 'skipped' | 'refunded' | 'voided'
/** Subscription event type as rendered on `SubscriptionEventOutput.eventType` (lower-cased Rust `Debug` form). */
export type SubscriptionEventType = 'created' | 'activated' | 'trialstarted' | 'trialended' | 'renewed' | 'paymentfailed' | 'paymentretrysucceeded' | 'paused' | 'resumed' | 'skipped' | 'cancelled' | 'expired' | 'planchanged' | 'itemsmodified' | 'quantitychanged' | 'addressupdated' | 'paymentmethodupdated' | 'discountapplied' | 'discountremoved' | 'refunded'

// ---------------------------------------------------------------------------
// Promotions
// ---------------------------------------------------------------------------

/** Promotion type as rendered on `PromotionOutput.promotionType` and `AppliedPromotionOutput.discountType` (lower-cased Rust `Debug` form). */
export type PromotionType = 'percentageoff' | 'fixedamountoff' | 'buyxgety' | 'freeshipping' | 'tiereddiscount' | 'bundlediscount' | 'firstorderdiscount' | 'giftwithpurchase'
/** Promotion type accepted on input (case-insensitive). */
export type PromotionTypeInput = 'percentage_off' | 'percentageoff' | 'fixed_amount_off' | 'fixedamountoff' | 'buy_x_get_y' | 'buyxgety' | 'bogo' | 'free_shipping' | 'freeshipping' | 'tiered_discount' | 'tiereddiscount' | 'bundle' | 'bundle_discount' | 'bundlediscount'
/** Promotion trigger as rendered on `PromotionOutput.trigger` (lower-cased Rust `Debug` form). */
export type PromotionTrigger = 'automatic' | 'couponcode' | 'both'
/** Promotion trigger accepted on input (case-insensitive). */
export type PromotionTriggerInput = 'automatic' | 'auto' | 'coupon_code' | 'couponcode' | 'coupon' | 'both'
/** Promotion target as rendered on `PromotionOutput.target` (lower-cased Rust `Debug` form). */
export type PromotionTarget = 'order' | 'product' | 'category' | 'shipping' | 'lineitem'
/** Promotion target accepted on input (case-insensitive). */
export type PromotionTargetInput = 'order' | 'product' | 'category' | 'shipping' | 'line_item' | 'lineitem'
/** Stacking behaviour as rendered on `PromotionOutput.stacking` (lower-cased Rust `Debug` form). */
export type PromotionStacking = 'stackable' | 'exclusive' | 'selectivestack'
/** Stacking behaviour accepted on input (case-insensitive). */
export type PromotionStackingInput = 'stackable' | 'exclusive' | 'selective_stack' | 'selectivestack'
/** Promotion status, shared by `PromotionOutput.status` and the update/filter inputs (case-insensitive on input). */
export type PromotionStatus = 'draft' | 'scheduled' | 'active' | 'paused' | 'expired' | 'exhausted' | 'archived'
/** Coupon status, shared by `CouponOutput.status` and `CouponFilterInput.status` (case-insensitive on input). */
export type CouponStatus = 'active' | 'disabled' | 'exhausted' | 'expired'

// ---------------------------------------------------------------------------
// Tax
// ---------------------------------------------------------------------------

/** Tax type, shared by outputs and inputs (case-insensitive on input). */
export type TaxType = 'sales_tax' | 'vat' | 'gst' | 'hst' | 'pst' | 'qst' | 'consumption_tax' | 'custom'
/** Product tax category, shared by outputs and inputs (case-insensitive on input). */
export type ProductTaxCategory = 'standard' | 'reduced' | 'super_reduced' | 'zero_rated' | 'exempt' | 'digital' | 'clothing' | 'food' | 'prepared_food' | 'medical' | 'educational' | 'luxury'
/** Tax jurisdiction level, shared by outputs and inputs (case-insensitive on input). */
export type TaxJurisdictionLevel = 'country' | 'state' | 'county' | 'city' | 'district' | 'special'
/** Tax exemption type as rendered on outputs (lower-cased Rust `Debug` form). */
export type TaxExemptionType = 'resale' | 'nonprofit' | 'government' | 'educational' | 'religious' | 'medical' | 'manufacturing' | 'agricultural' | 'export' | 'diplomatic' | 'other'
/** Tax exemption type accepted by `CreateExemptionInput.exemptionType` (case-insensitive). */
export type TaxExemptionTypeInput = TaxExemptionType | 'non_profit'
/** Whether prices carry tax (`TaxSettingsOutput.calculationMethod`; on input, anything but `inclusive` means `exclusive`). */
export type TaxCalculationMethod = 'exclusive' | 'inclusive'
/** How stacked taxes combine (`TaxSettingsOutput.compoundMethod`; on input,). */
export type TaxCompoundMethod = 'combined' | 'compound' | 'separate'

// ---------------------------------------------------------------------------
// x402 / agent cards
// ---------------------------------------------------------------------------

/** x402 network as rendered on outputs. */
export type X402Network = 'set_chain' | 'set_chain_testnet' | 'base' | 'arc' | 'arc-testnet' | 'base_sepolia' | 'ethereum' | 'ethereum_sepolia' | 'arbitrum' | 'optimism'
/** x402 network accepted on input: the rendered form or any alias (case-insensitive). */
export type X402NetworkInput = X402Network | 'set' | 'ssc' | 'set_testnet' | 'arc_testnet' | 'eth' | 'mainnet' | 'sepolia' | 'arb' | 'op'
/** x402 asset as rendered on outputs (lower-cased). */
export type X402Asset = 'usdc' | 'tether' | 'ss_usd' | 'wss_usd' | 'dai' | 'ether'
/** x402 asset accepted on input: the rendered form or the ticker aliases (case-insensitive). */
export type X402AssetInput = X402Asset | 'USDC' | 'USDT' | 'TETHER' | 'usdt' | 'ssUSD' | 'SSUSD' | 'SS_USD' | 'ssusd' | 'wssUSD' | 'WSSUSD' | 'WSS_USD' | 'wssusd' | 'DAI' | 'ETH' | 'ETHER' | 'eth'
/** Payment intent status as rendered on `X402IntentOutput.status`. */
export type X402IntentStatus = 'created' | 'signed' | 'sequenced' | 'batched' | 'settled' | 'expired' | 'failed' | 'cancelled'
/** Payment intent status accepted by `X402IntentFilterInput.status` (case-insensitive). */
export type X402IntentStatusInput = X402IntentStatus | 'canceled'
/** x402 signature scheme, shared by outputs and inputs (case-insensitive on input). */
export type X402SignatureScheme = 'ed25519' | 'ml_dsa65' | 'ed25519_ml_dsa65'
/** Agent trust level as rendered on `X402AgentCardOutput.trustLevel`. */
export type X402TrustLevel = 'sandbox' | 'standard' | 'verified' | 'enterprise'
/** Agent trust level accepted on input: the rendered form or an alias (case-insensitive). */
export type X402TrustLevelInput = X402TrustLevel | 'test' | 'default' | 'business'
/** A2A commerce skill as rendered on `X402AgentCardOutput.a2ASkills`. */
export type X402A2ASkill = 'commerce.sell' | 'commerce.buy' | 'commerce.quote' | 'commerce.request_quote' | 'commerce.fulfill' | 'commerce.ship' | 'commerce.digital_deliver' | 'commerce.process_return' | 'commerce.refund' | 'commerce.support'
/** A2A commerce skill accepted on input: the rendered form or the bare skill name. */
export type X402A2ASkillInput = X402A2ASkill | 'sell' | 'buy' | 'quote' | 'request_quote' | 'fulfill' | 'ship' | 'digital_deliver' | 'process_return' | 'refund' | 'support'
/** x402 credit ledger direction as rendered on `X402CreditTransactionOutput.direction`. */
export type X402CreditDirection = 'credit' | 'debit'
/** x402 credit ledger direction accepted by `X402CreditTransactionFilterInput.direction` (case-insensitive). */
export type X402CreditDirectionInput = X402CreditDirection | 'cr' | 'dr'

// ---------------------------------------------------------------------------
// Vector search / VES crypto
// ---------------------------------------------------------------------------

/** Entity type accepted by `VectorSearch.clear` (case-insensitive). */
export type VectorEntityType = 'product' | 'products' | 'customer' | 'customers' | 'order' | 'orders' | 'inventory_item' | 'inventory' | 'inventory_items'
/** Hash domain accepted by `domainHash` (exact, upper-case). */
export type VesHashDomain = 'PAYLOAD_PLAIN' | 'PAYLOAD_AAD' | 'PAYLOAD_CIPHER' | 'RECIPIENTS' | 'EVENTSIG' | 'LEAF' | 'NODE' | 'PAD_LEAF' | 'STREAM' | 'RECEIPT'

// ---------------------------------------------------------------------------
// Growth: gift cards, store credits, reviews, segments, loyalty
// ---------------------------------------------------------------------------

/** Gift card status, shared by outputs and inputs (case-insensitive on input). */
export type GiftCardStatus = 'active' | 'depleted' | 'expired' | 'disabled'
/** Gift card ledger entry kind on `GiftCardTransactionOutput.transactionType`. */
export type GiftCardTransactionType = 'charge' | 'refund' | 'adjustment'
/** Store credit status, shared by outputs and inputs (case-insensitive on input). */
export type StoreCreditStatus = 'active' | 'depleted' | 'expired' | 'voided'
/** Store credit reason, shared by outputs and inputs (case-insensitive on input). */
export type StoreCreditReason = 'return' | 'loyalty' | 'compensation' | 'promotion' | 'manual' | 'gift_card'
/** Store credit ledger entry kind on `StoreCreditTransactionOutput.transactionType`. */
export type StoreCreditTransactionType = 'issue' | 'apply' | 'adjust' | 'void' | 'expire'
/** Review moderation status, shared by outputs and inputs (case-insensitive on input). */
export type ReviewStatus = 'pending' | 'approved' | 'rejected' | 'flagged'
/** Segment membership model, shared by outputs and inputs (case-insensitive on input). */
export type SegmentType = 'static' | 'dynamic'
/** Segment rule comparison operator, shared by outputs and inputs (case-insensitive on input). */
export type SegmentOperator = 'eq' | 'neq' | 'gt' | 'gte' | 'lt' | 'lte' | 'contains' | 'in' | 'between' | 'starts_with' | 'ends_with'
/** Loyalty program status on `LoyaltyProgramOutput.status`. */
export type LoyaltyProgramStatus = 'active' | 'paused' | 'archived'
/** Loyalty points ledger entry kind, shared by outputs and `AdjustPointsInput.transactionType` (case-insensitive on input). */
export type LoyaltyTransactionType = 'earn' | 'redeem' | 'adjust' | 'expire' | 'bonus' | 'refund'
/** Loyalty reward kind, shared by outputs and inputs (case-insensitive on input). */
export type LoyaltyRewardType = 'discount' | 'free_shipping' | 'free_product' | 'store_credit' | 'exclusive_access'

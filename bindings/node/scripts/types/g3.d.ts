// Literal unions and payload shapes for the finance / procurement / logistics /
// operations / agentic modules (fixed assets, revenue recognition, cycle
// counts, EDI, prepayments, vendor credits, price schedules & levels, transfer
// orders, production batches, supplier SKUs, inbound shipments, activity logs,
// channels, companies, units of measure, shipping zones, stock & topology
// snapshots, print stations, integration mappings, payment obligations,
// purgatory, vendor returns, fraud, search config, ERC-8004, maintenance).
//
// Every union below mirrors a `stateset-core` enum. Output fields carry the
// rendered (snake_case unless noted) form. Input fields accept the same
// spellings; unless a type says otherwise the native parser matches them
// ASCII case-insensitively, so `'DRAFT'` is accepted where `'draft'` is
// listed, but only the listed form is what a record reads back as.

/** Fixed-asset category (`FixedAssetCategory`). */
export type FixedAssetCategory =
  | 'land'
  | 'building'
  | 'machinery'
  | 'equipment'
  | 'vehicle'
  | 'furniture_and_fixtures'
  | 'computer_hardware'
  | 'software'
  | 'leasehold_improvement'
  | 'other'
/** Fixed-asset lifecycle status (`FixedAssetStatus`). */
export type FixedAssetStatus = 'draft' | 'in_service' | 'fully_depreciated' | 'disposed' | 'written_off'
/** Depreciation method accepted on input (exact, lowercase). `declining_balance` also needs `decliningBalanceRate`. */
export type DepreciationMethod = 'straight_line' | 'declining_balance' | 'units_of_production'
/** Depreciation method as rendered on a record; `unknown` only for a method this binding predates. */
export type DepreciationMethodOutput = DepreciationMethod | 'unknown'
/** Depreciation schedule entry status (`DepreciationEntryStatus`). */
export type DepreciationEntryStatus = 'scheduled' | 'posted'

/** Revenue recognition method accepted on input (exact, lowercase). `ratable_over_time` also needs `recognitionStart`/`recognitionEnd`. */
export type RecognitionMethod = 'point_in_time' | 'ratable_over_time' | 'milestone'
/** Recognition method as rendered on a record; `unknown` only for a method this binding predates. */
export type RecognitionMethodOutput = RecognitionMethod | 'unknown'
/** Revenue contract lifecycle status (`RevenueContractStatus`); transitions are guarded. */
export type RevenueContractStatus = 'draft' | 'active' | 'completed' | 'cancelled'
/** Revenue schedule entry status (`RevenueEntryStatus`). */
export type RevenueEntryStatus = 'deferred' | 'recognized'

/** Cycle count lifecycle status (`CycleCountStatus`). */
export type CycleCountStatus = 'draft' | 'in_progress' | 'completed' | 'cancelled'

/** EDI document direction relative to this system (`EdiDirection`). */
export type EdiDirection = 'inbound' | 'outbound'
/** EDI document processing status (`EdiStatus`). */
export type EdiStatus = 'pending' | 'sent' | 'acknowledged' | 'processed' | 'error'

/** Prepayment lifecycle status (`PrepaymentStatus`). */
export type PrepaymentStatus = 'open' | 'applied' | 'refunded' | 'cancelled'
/** What a prepayment application is applied against (`PrepaymentTargetType`). */
export type PrepaymentTargetType = 'bill' | 'payment_obligation'
/** Vendor credit lifecycle status (`VendorCreditStatus`). */
export type VendorCreditStatus = 'open' | 'applied' | 'cancelled'
/** What a vendor credit application is applied against (`VendorCreditTargetType`). */
export type VendorCreditTargetType = 'bill' | 'payment_obligation'

/** Catalog-wide adjustment a price level applies by default (`PriceAdjustmentType`). */
export type PriceAdjustmentType = 'none' | 'percentage_discount' | 'percentage_markup'

/** Transfer order lifecycle status (`TransferOrderStatus`). */
export type TransferOrderStatus = 'draft' | 'pending' | 'in_transit' | 'partially_received' | 'received' | 'cancelled'
/** Production batch lifecycle status (`ProductionBatchStatus`). */
export type ProductionBatchStatus = 'planned' | 'in_progress' | 'completed' | 'cancelled'
/** Inbound shipment (ASN) lifecycle status (`InboundShipmentStatus`). */
export type InboundShipmentStatus = 'pending' | 'in_transit' | 'arrived' | 'partially_received' | 'received' | 'cancelled'

/** Kind of actor that produced an activity log entry (`ActorKind`). */
export type ActorKind = 'user' | 'system' | 'integration' | 'agent'

/** Direction of order flow through a channel (`ChannelType`); fixed once set. */
export type ChannelType = 'sales_channel' | 'fulfillment_channel' | 'end_to_end_channel'
/** Channel lifecycle status (`ChannelStatus`). */
export type ChannelStatus = 'active' | 'paused' | 'deleted'
/** B2B company account status (`CompanyStatus`). */
export type CompanyStatus = 'active' | 'inactive'

/** Scope of a unit conversion rule (`ConversionRuleType`); rendered UPPERCASE, parsed case-insensitively. */
export type ConversionRuleType = 'SYSTEM' | 'SKU'

/** How a zone shipping method prices a shipment (`ShippingMethodType`). */
export type ShippingMethodType = 'flat' | 'weight_based' | 'price_based' | 'calculated' | 'free'

/** Print job payload encoding (`PrintPayloadKind`). */
export type PrintPayloadKind = 'zpl' | 'pdf'
/** Print job lifecycle status (`PrintJobStatus`). */
export type PrintJobStatus = 'queued' | 'picked_up' | 'printed' | 'failed'

/** Value transform applied by an integration field mapping (`FieldTransform`). */
export type FieldTransform = 'none' | 'uppercase' | 'lowercase' | 'trim'

/** Payment obligation lifecycle status (`PaymentObligationStatus`). */
export type PaymentObligationStatus = 'pending' | 'scheduled' | 'partially_paid' | 'paid' | 'cancelled'

/** Operational health grade derived from a topology snapshot (`HealthGrade`). */
export type HealthGrade = 'unknown' | 'healthy' | 'degraded' | 'critical'

/** Vendor return lifecycle status (`VendorReturnStatus`). */
export type VendorReturnStatus = 'draft' | 'pending' | 'processed' | 'cancelled'
/** Reason a line is returned to the vendor (`VendorReturnReason`). */
export type VendorReturnReason = 'defective' | 'overage' | 'wrong_item' | 'other'

/** Fraud assessment decision, also a rule's action (`FraudDecision`). */
export type FraudDecision = 'accept' | 'review' | 'reject'
/** Kind of fraud signal detected on an order (`FraudSignalType`). */
export type FraudSignalType =
  | 'velocity_spike'
  | 'address_mismatch'
  | 'high_value_first_order'
  | 'geo_ip_anomaly'
  | 'bin_country_mismatch'
  | 'device_fingerprint'
  | 'proxy_vpn'
  | 'disposable_email'
  | 'payment_retries'
  | 'unusual_time'

/** Tokenizer strategy for a searchable field (`Tokenizer`). */
export type SearchTokenizer = 'standard' | 'ngram' | 'edge' | 'keyword'
/** Facet type for search refinement (`FacetType`). */
export type FacetType = 'value' | 'range' | 'hierarchical'

/** ERC-8004 wallet proof type as rendered on an identity (`AgentWalletProofType`). */
export type AgentWalletProofType = 'eip_712' | 'erc_1271'
/** ERC-8004 wallet proof type spellings the parser accepts (exact case); records read back as `AgentWalletProofType`. */
export type AgentWalletProofTypeInput = 'eip712' | 'eip_712' | 'erc1271' | 'erc_1271'

/** How a structured import treats a record that already exists (`maintenance::ConflictPolicy`); exact, lowercase. */
export type ImportConflictPolicy = 'skip' | 'fail'

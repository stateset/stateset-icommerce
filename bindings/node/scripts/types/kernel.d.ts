// Governed-write ("kernel") surface: the wire shapes behind
// `Commerce.executeKernelCommand`, `provisionEconomicBudget`,
// `economicBudgetStatus` and `checkoutSnapshot`.
//
// These mirror the serde contracts in `crates/stateset-core/src/kernel.rs`
// (commands, policy, receipts, budgets) and `crates/stateset-core/src/models/`
// (command payloads, the cart snapshot). The kernel speaks snake_case and
// exact decimals: every money, quantity and price field is a base-10 string
// such as `"12.34"`, never a JavaScript number. Timestamps are RFC 3339 strings
// and identifiers are UUID strings unless a comment says otherwise.
//
// Optional fields on inputs may be omitted or sent as `null`; the same fields
// come back as `null` on outputs.

// ---------------------------------------------------------------------------
// Money and identity primitives
// ---------------------------------------------------------------------------

/** Exact money on the wire (`stateset_primitives::MoneyWire`). */
export interface MoneyWire {
  /** Base-10 decimal amount, for example `"29.99"`. */
  amount: string
  /** ISO 4217 code that fixes the permitted minor-unit scale, e.g. `"USD"`. */
  currency: string
}

/**
 * Exact non-fiat amount identified by an asset symbol or chain-qualified
 * token id (`AssetAmountWire`). The asset id is case-sensitive.
 */
export interface AssetAmountWire {
  amount: string
  asset: string
}

/** Identity class responsible for a command (`PrincipalKind`). */
export type KernelPrincipalKind = 'human' | 'agent' | 'system' | 'integration'

/**
 * Authenticated identity and delegation context (`KernelPrincipal`). Comes
 * from trusted host configuration, never from model-generated arguments.
 */
export interface KernelPrincipal {
  /** Stable subject identifier, e.g. `"agent:buyer-7"`. */
  id: string
  kind: KernelPrincipalKind
  /** Tenant boundary; required by default policy (`requires_tenant`). */
  tenant_id?: string | null
  /** Principal that delegated authority; required for agents by default. */
  delegated_by?: string | null
  /** Capabilities the caller asserts; policy checks them. */
  capabilities?: string[]
}

// ---------------------------------------------------------------------------
// Command envelope
// ---------------------------------------------------------------------------

/** Non-mutating preview (the safe default) or an authorized mutation. */
export type KernelExecutionMode = 'preview' | 'apply'

/** Evidence for an approval required by policy (`ApprovalEvidence`). */
export interface KernelApprovalEvidence {
  approval_id: string
  approved_by: string
  /** Must equal the command's `command_type`. */
  scope: string
  tenant_id?: string | null
  store_id?: string | null
  /** Must equal the command's `idempotency_key`. */
  idempotency_key?: string | null
  approved_at: string
  expires_at?: string | null
}

/**
 * Proof that a trusted issuer signed the semantic command
 * (`AuthorityEvidence`). `signature` is hex-encoded Ed25519 over the
 * canonical command hash; `key_id` must appear in
 * `KernelPolicy.trusted_authority_keys`.
 */
export interface KernelAuthorityEvidence {
  issuer: string
  key_id: string
  issued_at: string
  expires_at: string
  signature: string
}

/** A principal-issued objective an agent acts under (`EconomicMandate`). */
export interface EconomicMandate {
  mandate_id: string
  subject_id: string
  issued_by: string
  objective: string
  /** Commands this mandate permits. Empty is never a wildcard. */
  allowed_commands?: string[]
  tenant_id?: string | null
  store_id?: string | null
  issued_at: string
  expires_at: string
}

/** Resources an economic command proposes to commit (`EconomicCommitment`). */
export interface EconomicCommitment {
  /** Budget provisioned via `provisionEconomicBudget`. */
  budget_id?: string | null
  /** Exact fiat money placed at risk. Mutually exclusive with `asset_amount`. */
  amount?: MoneyWire | null
  asset_amount?: AssetAmountWire | null
  counterparty_id?: string | null
  /** Exact unit commitment as a decimal string; units may be fractional. */
  quantity?: string | null
  /** Evidence identifiers (quotes, tickets, contracts) supporting the action. */
  evidence?: string[]
}

/**
 * Versioned execution request shared by every governed command
 * (`CommandEnvelope<T>`). `command_type` selects the payload shape; see
 * `KernelCommand` for the closed catalog.
 */
export interface KernelCommandEnvelope<TType extends string, TPayload> {
  /** Wire-contract version; currently `"1.0"`. */
  contract_version: string
  /** Unique, non-nil UUID for this invocation. */
  command_id: string
  /** Stable retry key. Retries must reuse this value. */
  idempotency_key: string
  command_type: TType
  principal: KernelPrincipal
  /** Logical store boundary; required by default policy (`requires_store`). */
  store_id?: string | null
  /** Root workflow identifier (UUID). */
  correlation_id?: string | null
  /** Command or event that caused this command (UUID). */
  causation_id?: string | null
  /** Optimistic concurrency version expected by the caller. */
  expected_version?: number | null
  /** Policy revision the caller expects; a mismatch is rejected. */
  policy_version?: string | null
  approval?: KernelApprovalEvidence | null
  authority?: KernelAuthorityEvidence | null
  mandate?: EconomicMandate | null
  commitment?: EconomicCommitment | null
  /** Time after which execution should not begin. */
  deadline?: string | null
  trace_id?: string | null
  mode: KernelExecutionMode
  payload: TPayload
  issued_at: string
}

// ---------------------------------------------------------------------------
// Command payloads (crates/stateset-core/src/models/*)
// ---------------------------------------------------------------------------

/** `inventory.item.create` (`CreateInventoryItem`). */
export interface KernelCreateInventoryItemPayload {
  sku: string
  name: string
  description?: string | null
  unit_of_measure?: string | null
  initial_quantity?: string | null
  location_id?: number | null
  reorder_point?: string | null
  safety_stock?: string | null
}

/** Product attribute as the engine stores it (`ProductAttribute`). */
export interface KernelProductAttribute {
  name: string
  value: string
  group?: string | null
  is_visible: boolean
  is_variation: boolean
}

/** Variant option pair (`VariantOption`). */
export interface KernelVariantOption {
  name: string
  value: string
}

/** Variant input nested in `products.create` (`CreateProductVariant`). */
export interface KernelCreateProductVariantPayload {
  sku: string
  name?: string | null
  price: string
  compare_at_price?: string | null
  cost?: string | null
  barcode?: string | null
  weight?: string | null
  weight_unit?: string | null
  options?: KernelVariantOption[] | null
  is_default?: boolean | null
}

/** `products.create` (`CreateProduct`). */
export interface KernelCreateProductPayload {
  name: string
  slug?: string | null
  description?: string | null
  product_type?: 'simple' | 'variable' | 'bundle' | 'digital' | null
  attributes?: KernelProductAttribute[] | null
  seo?: { title?: string | null; description?: string | null; keywords: string[] } | null
  variants?: KernelCreateProductVariantPayload[] | null
}

/** Payment instrument class (`PaymentMethodType`). */
export type KernelPaymentMethodType =
  | 'credit_card'
  | 'debit_card'
  | 'bank_transfer'
  | 'pay_pal'
  | 'apple_pay'
  | 'google_pay'
  | 'crypto'
  | 'stablecoin'
  | 'store_credit'
  | 'gift_card'
  | 'cash_on_delivery'
  | 'invoice'
  | 'other'

/**
 * `payments.create` (`CreatePayment`). Every field is serde-defaulted, so all
 * are optional on the wire; a real payment still needs `amount` and
 * `payment_method`. `metadata` is a free-form string here, not an object.
 */
export interface KernelCreatePaymentPayload {
  order_id?: string | null
  invoice_id?: string | null
  customer_id?: string | null
  payment_method?: KernelPaymentMethodType
  amount?: string
  currency?: string | null
  external_id?: string | null
  idempotency_key?: string | null
  processor?: string | null
  card_brand?: 'unknown' | 'visa' | 'mastercard' | 'amex' | 'discover' | 'diners_club' | 'jcb' | 'union_pay' | null
  card_last4?: string | null
  card_exp_month?: number | null
  card_exp_year?: number | null
  blockchain_network?: 'solana' | 'solana_devnet' | 'set_chain' | 'set_chain_testnet' | 'ethereum' | 'base' | 'arbitrum' | 'near' | 'cosmos' | null
  stablecoin_type?: 'usdc' | 'usdt' | 'ss_usd' | 'wss_usd' | 'dai' | null
  from_wallet_address?: string | null
  to_wallet_address?: string | null
  token_address?: string | null
  billing_email?: string | null
  billing_name?: string | null
  billing_address?: string | null
  description?: string | null
  metadata?: string | null
}

/** `payments.create_refund` (`CreateRefund`). `payment_id` is required in practice. */
export interface KernelCreateRefundPayload {
  payment_id: string
  /** Defaults to the full payment amount. */
  amount?: string | null
  reason?: string | null
  external_id?: string | null
  idempotency_key?: string | null
  notes?: string | null
}

/** `inventory.reserve` (`ReserveInventory`). */
export interface KernelReserveInventoryPayload {
  sku: string
  location_id?: number | null
  quantity: string
  reference_type: string
  reference_id: string
  expires_in_seconds?: number | null
}

/** `inventory.reservation.confirm` (`ConfirmInventoryReservation`). */
export interface KernelConfirmInventoryReservationPayload {
  reservation_id: string
  /** Omit to confirm the full remaining reservation. */
  quantity?: string | null
}

/** `inventory.reservation.release` (`ReleaseInventoryReservation`). */
export interface KernelReleaseInventoryReservationPayload {
  reservation_id: string
}

/** Order lifecycle status (`OrderStatus`). */
export type KernelOrderStatus =
  | 'pending'
  | 'confirmed'
  | 'processing'
  | 'partially_shipped'
  | 'shipped'
  | 'delivered'
  | 'cancelled'
  | 'refunded'

/** Order payment status (`PaymentStatus`). */
export type KernelOrderPaymentStatus =
  | 'pending'
  | 'authorized'
  | 'paid'
  | 'partially_paid'
  | 'refunded'
  | 'partially_refunded'
  | 'failed'

/** `orders.transition` (`TransitionOrder`). */
export interface KernelTransitionOrderPayload {
  order_id: string
  status: KernelOrderStatus
  payment_status?: KernelOrderPaymentStatus | null
  /**
   * Void in-flight payments atomically with a cancel. Omit (or `false`) to
   * have a cancel rejected while captured money is outstanding.
   */
  void_payments?: boolean
}

/** One order line in a shipment (`ShipmentLineInput`). */
export interface KernelShipmentLine {
  order_item_id: string
  quantity: number
}

/** `orders.ship` (`ShipOrderCommand`). */
export interface KernelShipOrderPayload {
  order_id: string
  tracking_number?: string | null
  /** Omit to ship every remaining unit. */
  lines?: KernelShipmentLine[] | null
}

/** `returns.transition` (`TransitionReturn`). */
export interface KernelTransitionReturnPayload {
  return_id: string
  status: 'requested' | 'approved' | 'rejected' | 'in_transit' | 'received' | 'inspecting' | 'completed' | 'cancelled'
}

/** `ledger.post` (`PostJournalEntry`). */
export interface KernelPostJournalEntryPayload {
  journal_entry_id: string
  posted_by: string
}

/** `x402.settle` (`SettleX402Intent`). */
export interface KernelSettleX402IntentPayload {
  intent_id: string
  tx_hash: string
  block_number: number
}

/** What checkout does when a tracked SKU cannot be fully reserved (`StockPolicy`). */
export type KernelStockPolicy = 'allow_backorder' | 'reject_if_insufficient'

/** `checkout.commit` (`CommitCheckout`). */
export interface KernelCommitCheckoutPayload {
  cart_id: string
  /** Omission preserves historical backorder behaviour. */
  stock_policy?: KernelStockPolicy | null
  /** The `fingerprint` from `checkoutSnapshot`; checked under the checkout transaction. */
  expected_cart_fingerprint?: string | null
}

/** `subscriptions.charge` (`ChargeSubscription`). */
export interface KernelChargeSubscriptionPayload {
  billing_cycle_id: string
  payment_method: KernelPaymentMethodType
  processor?: string | null
}

/** `a2a.escrow.create` (`CreateA2AEscrow`). */
export interface KernelCreateA2AEscrowPayload {
  quote_id?: string | null
  payment_id?: string | null
  buyer_address: string
  seller_address: string
  amount: string
  asset: string
  network: string
  release_conditions?: Array<Record<string, unknown>>
  expires_at: string
  auto_release_after?: string | null
  metadata?: Record<string, unknown> | null
}

/** `a2a.escrow.fund` / `a2a.escrow.release` (`FundA2AEscrow`, `ReleaseA2AEscrow`). */
export interface KernelA2AEscrowRefPayload {
  escrow_id: string
}

/** `a2a.escrow.dispute` (`DisputeA2AEscrow`). */
export interface KernelDisputeA2AEscrowPayload {
  escrow_id: string
  reason: string
  category?: string | null
}

/** `a2a.escrow.refund` (`RefundA2AEscrow`). */
export interface KernelRefundA2AEscrowPayload {
  escrow_id: string
  reason?: string | null
}

/** `a2a.dispute.file` (`FileA2ADispute`). */
export interface KernelFileA2ADisputePayload {
  escrow_id: string
  /** Must be the escrow buyer or seller; the respondent is derived. */
  claimant_address: string
  reason: string
  category: string
  evidence_deadline: string
  review_deadline: string
  metadata?: Record<string, unknown> | null
}

/** `a2a.dispute.evidence.submit` (`SubmitA2ADisputeEvidence`). */
export interface KernelSubmitA2ADisputeEvidencePayload {
  dispute_id: string
  submitted_by: string
  evidence_type: string
  title: string
  description?: string | null
  content: string
}

/** `a2a.dispute.resolve` (`ResolveA2ADispute`). */
export interface KernelResolveA2ADisputePayload {
  dispute_id: string
  resolution_type: 'full_refund' | 'release_to_seller' | 'split' | 'escalated'
  /** Required for `split`; forbidden otherwise. */
  buyer_amount?: string | null
  /** Required for `split`; forbidden otherwise. */
  seller_amount?: string | null
  note?: string | null
}

/**
 * The closed catalog of governed commands `executeKernelCommand` dispatches
 * (`stateset_embedded::Commerce::execute_kernel_command`). Any other
 * `command_type` is rejected before a repository is reached.
 */
export type KernelCommandType = KernelCommand['command_type']

/**
 * A governed command: the envelope discriminated by `command_type`, each
 * variant carrying its own payload shape.
 */
export type KernelCommand =
  | KernelCommandEnvelope<'inventory.item.create', KernelCreateInventoryItemPayload>
  | KernelCommandEnvelope<'products.create', KernelCreateProductPayload>
  | KernelCommandEnvelope<'payments.create', KernelCreatePaymentPayload>
  | KernelCommandEnvelope<'payments.create_refund', KernelCreateRefundPayload>
  | KernelCommandEnvelope<'inventory.reserve', KernelReserveInventoryPayload>
  | KernelCommandEnvelope<'inventory.reservation.confirm', KernelConfirmInventoryReservationPayload>
  | KernelCommandEnvelope<'inventory.reservation.release', KernelReleaseInventoryReservationPayload>
  | KernelCommandEnvelope<'orders.transition', KernelTransitionOrderPayload>
  | KernelCommandEnvelope<'orders.ship', KernelShipOrderPayload>
  | KernelCommandEnvelope<'returns.transition', KernelTransitionReturnPayload>
  | KernelCommandEnvelope<'ledger.post', KernelPostJournalEntryPayload>
  | KernelCommandEnvelope<'x402.settle', KernelSettleX402IntentPayload>
  | KernelCommandEnvelope<'checkout.commit', KernelCommitCheckoutPayload>
  | KernelCommandEnvelope<'subscriptions.charge', KernelChargeSubscriptionPayload>
  | KernelCommandEnvelope<'a2a.escrow.create', KernelCreateA2AEscrowPayload>
  | KernelCommandEnvelope<'a2a.escrow.dispute', KernelDisputeA2AEscrowPayload>
  | KernelCommandEnvelope<'a2a.escrow.fund', KernelA2AEscrowRefPayload>
  | KernelCommandEnvelope<'a2a.escrow.release', KernelA2AEscrowRefPayload>
  | KernelCommandEnvelope<'a2a.escrow.refund', KernelRefundA2AEscrowPayload>
  | KernelCommandEnvelope<'a2a.dispute.file', KernelFileA2ADisputePayload>
  | KernelCommandEnvelope<'a2a.dispute.evidence.submit', KernelSubmitA2ADisputeEvidencePayload>
  | KernelCommandEnvelope<'a2a.dispute.resolve', KernelResolveA2ADisputePayload>

// ---------------------------------------------------------------------------
// Policy
// ---------------------------------------------------------------------------

/**
 * Policy requirements for one namespaced command (`KernelCommandPolicy`).
 * Every field is serde-defaulted. Note the defaults that are `true`:
 * `requires_tenant`, `requires_store` and `requires_agent_delegation`.
 */
export interface KernelCommandPolicy {
  /** Capabilities the principal must hold. Default: none. */
  required_capabilities?: string[]
  /** Default `false`. */
  requires_approval?: boolean
  /** Default `true`. */
  requires_tenant?: boolean
  /** Default `true`. */
  requires_store?: boolean
  /** Empty permits any non-empty tenant when `requires_tenant` is on. */
  allowed_tenant_ids?: string[]
  /** Empty permits any non-empty store when `requires_store` is on. */
  allowed_store_ids?: string[]
  /** Default `true`: agents must name `delegated_by`. */
  requires_agent_delegation?: boolean
  /** Default `false`. */
  requires_signed_authority?: boolean
  /** Default `false`. */
  requires_mandate?: boolean
  /** Default `false`: require `commitment.budget_id` on money-moving commands. */
  requires_budget?: boolean
  /** Maximum exact fiat commitment for one command. */
  max_amount?: MoneyWire | null
  /** Require approval only above this exact fiat amount. */
  approval_above?: MoneyWire | null
  max_asset_amount?: AssetAmountWire | null
  approval_above_asset?: AssetAmountWire | null
  /** Maximum exact unit commitment, as a decimal string. */
  max_quantity?: string | null
  /** Empty permits any counterparty. */
  allowed_counterparty_ids?: string[]
}

/**
 * Deterministic, versioned allow-list evaluated before execution
 * (`KernelPolicy`). Deny by default: a `command_type` absent from `commands`
 * is rejected. Comes from trusted host configuration, never from model output.
 */
export interface KernelPolicy {
  /** Stable revision id recorded in every receipt's policy decision. */
  version: string
  /** Rules keyed by `command_type`. */
  commands?: Record<string, KernelCommandPolicy>
  /** Trusted Ed25519 verifying keys, hex encoded, keyed by `key_id`. */
  trusted_authority_keys?: Record<string, string>
}

// ---------------------------------------------------------------------------
// Receipt
// ---------------------------------------------------------------------------

/** Outcome category recorded in a receipt (`ExecutionStatus`). */
export type KernelExecutionStatus = 'previewed' | 'succeeded' | 'rejected' | 'failed'

/** Machine-readable retry guidance (`RetryDisposition`). */
export type KernelRetryDisposition = 'never' | 'same_key' | 'after_conflict' | 'after_delay'

/** Policy decision captured with a receipt (`PolicyDecisionEvidence`). */
export interface KernelPolicyDecision {
  policy_version: string
  decision_id: string
  allowed: boolean
  /** Stable codes such as `"policy.capability_missing:payments.create"`. */
  reason_codes: string[]
}

/**
 * Accountability data copied from the command into its durable receipt
 * (`EconomicReceiptContext`).
 */
export interface KernelEconomicReceiptContext {
  principal: KernelPrincipal
  store_id: string | null
  correlation_id: string | null
  mandate: EconomicMandate | null
  commitment: EconomicCommitment | null
  approval_id: string | null
  authority_issuer: string | null
}

/**
 * Durable, machine-readable outcome of a governed command
 * (`ExecutionReceipt<T>`). `result` is the applied domain aggregate as the
 * engine serialises it (snake_case, exact decimal strings); it depends on the
 * command type (a `Payment` for `payments.create`, an `Order` for
 * `orders.ship`, ...) and is `null` for previews with no aggregate yet, and
 * for rejections.
 */
export interface KernelReceipt {
  contract_version: string
  receipt_id: string
  command_id: string
  idempotency_key: string
  command_type: KernelCommandType
  status: KernelExecutionStatus
  result: Record<string, unknown> | null
  /** Stable code such as `"kernel.commitment_amount_mismatch"`; never parse prose. */
  error_code: string | null
  error_message: string | null
  retry: KernelRetryDisposition
  /** Affected aggregate category, e.g. `"payment"`. */
  aggregate_type: string | null
  aggregate_id: string | null
  version_before: number | null
  version_after: number | null
  /** Events committed atomically with the mutation (UUIDs). */
  event_ids: string[]
  policy: KernelPolicyDecision | null
  /** Present on every receipt the executor issues; absent only on hand-built ones. */
  economic_context?: KernelEconomicReceiptContext
  /** Hex SHA-256 link into the sealed receipt audit chain; set once sealed. */
  audit_hash: string | null
  started_at: string
  completed_at: string
}

// ---------------------------------------------------------------------------
// Economic budgets
// ---------------------------------------------------------------------------

/**
 * Operator-provisioned monetary authority for one principal
 * (`EconomicBudget`). Immutable once provisioned: re-provisioning the same
 * definition is idempotent, changing it under the same id is rejected.
 */
export interface EconomicBudget {
  budget_id: string
  principal_id: string
  tenant_id?: string | null
  store_id?: string | null
  /** Total exact amount available over the budget lifetime. */
  limit: MoneyWire
  valid_from: string
  expires_at: string
}

/** Exact balances of a provisioned budget (`EconomicBudgetStatus`). */
export interface EconomicBudgetStatus {
  /** The definition as stored; optional scope fields come back as `null`. */
  budget: EconomicBudget & { tenant_id: string | null; store_id: string | null }
  /** Reserved by successfully committed commands. */
  committed: MoneyWire
  /** Still available for new commitments. */
  available: MoneyWire
}

// ---------------------------------------------------------------------------
// Checkout snapshot
// ---------------------------------------------------------------------------

/** Cart address as the engine stores it (`CartAddress`). */
export interface CartAddressSnapshot {
  first_name: string
  last_name: string
  company: string | null
  line1: string
  line2: string | null
  city: string
  state: string | null
  postal_code: string
  country: string
  phone: string | null
  email: string | null
}

/** Cart line as the engine stores it (`CartItem`). */
export interface CartItemSnapshot {
  id: string
  cart_id: string
  product_id: string | null
  variant_id: string | null
  sku: string
  name: string
  description: string | null
  image_url: string | null
  quantity: number
  unit_price: string
  original_price: string | null
  discount_amount: string
  tax_amount: string
  total: string
  weight: string | null
  requires_shipping: boolean
  metadata: Record<string, unknown> | null
  created_at: string
  updated_at: string
}

/** x402 stablecoin payment attached to a cart (`CartX402Payment`). */
export interface CartX402PaymentSnapshot {
  intent_id: string | null
  payer_address: string
  network: 'set_chain' | 'set_chain_testnet' | 'base' | 'arc' | 'arc_testnet' | 'base_sepolia' | 'ethereum' | 'ethereum_sepolia' | 'arbitrum' | 'optimism'
  asset: 'usdc' | 'usdt' | 'ssusd' | 'wssusd' | 'dai' | 'eth'
  status: 'created' | 'signed' | 'sequenced' | 'batched' | 'settled' | 'expired' | 'failed' | 'cancelled'
}

/**
 * The cart exactly as the fingerprint was computed over it
 * (`stateset_core::Cart`, snake_case, exact decimals). This is the engine's
 * own aggregate, not the camelCase `CartOutput` the `carts` API returns.
 */
export interface CartSnapshot {
  id: string
  cart_number: string
  customer_id: string | null
  status: 'active' | 'ready_for_payment' | 'payment_pending' | 'completed' | 'abandoned' | 'cancelled' | 'expired'
  currency: string
  items: CartItemSnapshot[]
  subtotal: string
  tax_amount: string
  shipping_amount: string
  discount_amount: string
  grand_total: string
  customer_email: string | null
  customer_phone: string | null
  customer_name: string | null
  shipping_address: CartAddressSnapshot | null
  billing_address: CartAddressSnapshot | null
  billing_same_as_shipping: boolean
  fulfillment_type: 'shipping' | 'pickup' | 'digital' | null
  shipping_method: string | null
  shipping_carrier: string | null
  estimated_delivery: string | null
  payment_method: string | null
  payment_token: string | null
  payment_status: 'none' | 'method_selected' | 'authorized' | 'captured' | 'failed' | 'refunded'
  coupon_code: string | null
  discount_description: string | null
  order_id: string | null
  order_number: string | null
  notes: string | null
  metadata: Record<string, unknown> | null
  inventory_reserved: boolean
  reservation_expires_at: string | null
  x402_payment: CartX402PaymentSnapshot | null
  expires_at: string | null
  completed_at: string | null
  created_at: string
  updated_at: string
}

/**
 * Exact quote terms and their fingerprint, read from one cart snapshot
 * (`Commerce.checkoutSnapshot`). Keep the result with the issued quote and
 * pass `fingerprint` as `expected_cart_fingerprint` on `checkout.commit`;
 * never recalculate it at acceptance.
 */
export interface CheckoutSnapshot {
  cart: CartSnapshot
  /** `"sha256:<hex>"` over the canonical (JCS) cart snapshot, version-tagged. */
  fingerprint: string
}

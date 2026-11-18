/**
 * Which tool performs each catalogued status move, and which lifecycles the
 * engine has that the catalog does not (yet) publish.
 *
 * Hand-maintained and deliberately small. The generated catalog
 * (`./generated/state-machines.json`, written by a Rust test from the engine's
 * own transition rules) is the only authority on which moves the engine
 * accepts; nothing here adds, removes or reorders a move. This module answers
 * the question every refusal, preview and read leads to next: "which tool do I
 * call to make that move?"
 *
 * test/unit/state-machine-operations.test.js keeps it honest: every tool it
 * names exists, every move it names is in the catalog, and every uncatalogued
 * lifecycle's states equal the binding's generated status type
 * (bindings/node/index.d.ts).
 *
 * Nothing here reads or writes the store.
 */

/**
 * @typedef {object} MoveTool
 * @property {string} tool         MCP tool that makes the move
 * @property {string[]} [from]     only from these states (default: every state the catalog allows)
 * @property {boolean} [only]      a plain status update cannot make this move; the engine
 *                                 accepts it only through `tool`
 * @property {Record<string, unknown[]>} [when]
 *                                 the tool makes this move only when each named parameter
 *                                 has one of the listed values (`undefined` = not passed);
 *                                 e.g. cancel_subscription stores `expired` only with
 *                                 `immediate: true`
 * @property {string} [note]       what the tool needs to make this particular move
 */

const freeze = (value) => {
  if (value && typeof value === 'object') {
    for (const inner of Object.values(value)) freeze(inner);
    Object.freeze(value);
  }
  return value;
};

/**
 * machine -> target status -> the tools that move a record into it.
 * A target with no entry has no tool: the engine reaches it some other way
 * (a provider event, billing, a derived rule) or not through MCP at all.
 *
 * @type {Readonly<Record<string, Record<string, MoveTool[]>>>}
 */
export const TRANSITION_TOOLS = freeze({
  order: {
    confirmed: [{ tool: 'update_order_status' }],
    processing: [{ tool: 'update_order_status' }],
    partially_shipped: [
      { tool: 'ship_order', only: true, note: 'pass `lines` with the units shipped now' },
    ],
    shipped: [{ tool: 'ship_order', only: true }],
    delivered: [{ tool: 'update_order_status' }],
    cancelled: [{ tool: 'cancel_order' }, { tool: 'update_order_status' }],
    refunded: [{ tool: 'update_order_status' }],
  },
  return: {
    approved: [{ tool: 'approve_return' }],
    rejected: [{ tool: 'reject_return' }],
    cancelled: [{ tool: 'cancel_return' }],
    in_transit: [{ tool: 'add_return_tracking' }],
    received: [{ tool: 'mark_return_received' }],
    completed: [{ tool: 'complete_return' }],
  },
  shipment: {
    processing: [{ tool: 'update_shipment' }],
    ready_to_ship: [{ tool: 'update_shipment' }],
    shipped: [{ tool: 'ship_shipment' }, { tool: 'update_shipment' }],
    in_transit: [{ tool: 'update_shipment' }],
    out_for_delivery: [{ tool: 'update_shipment' }],
    delivered: [{ tool: 'deliver_shipment' }, { tool: 'update_shipment' }],
    failed: [{ tool: 'update_shipment' }],
    returned: [{ tool: 'update_shipment' }],
    on_hold: [{ tool: 'update_shipment' }],
    cancelled: [{ tool: 'cancel_shipment', only: true }],
  },
  cart: {
    ready_for_payment: [{ tool: 'mark_cart_ready_for_payment' }],
    payment_pending: [{ tool: 'begin_cart_checkout' }],
    completed: [{ tool: 'complete_checkout' }],
    abandoned: [{ tool: 'abandon_cart' }],
    cancelled: [{ tool: 'cancel_cart' }],
    expired: [{ tool: 'expire_cart' }],
  },
  payment: {
    completed: [{ tool: 'complete_payment', from: ['pending', 'processing', 'requires_action'] }],
    failed: [{ tool: 'mark_failed_payment' }],
    cancelled: [{ tool: 'cancel_payment' }],
  },
  customer: {
    active: [{ tool: 'update_customer' }],
    inactive: [{ tool: 'update_customer' }],
    suspended: [{ tool: 'update_customer' }],
    deleted: [{ tool: 'delete_customer', only: true }],
  },
  product: {
    draft: [{ tool: 'update_product' }],
    active: [{ tool: 'activate_product' }, { tool: 'update_product' }],
    archived: [{ tool: 'archive_product' }, { tool: 'update_product' }],
  },
  subscription: {
    paused: [{ tool: 'pause_subscription' }],
    active: [{ tool: 'resume_subscription', from: ['paused'] }],
    // The engine's cancellation stores `expired` when it is immediate and
    // `cancelled` (ending at the paid-through date) otherwise.
    cancelled: [{ tool: 'cancel_subscription', when: { immediate: [false, undefined] } }],
    expired: [
      {
        tool: 'cancel_subscription',
        when: { immediate: [true] },
        note: 'pass `immediate: true` (the default cancels at period end, as cancelled)',
      },
    ],
  },
  subscription_plan: {
    active: [{ tool: 'activate_subscription_plan' }],
    archived: [{ tool: 'archive_subscription_plan' }],
  },
  purchase_order: {
    pending_approval: [{ tool: 'submit_purchase_order', from: ['draft'] }],
    approved: [{ tool: 'approve_purchase_order', from: ['pending_approval'] }],
    sent: [{ tool: 'send_purchase_order', from: ['approved'] }],
    cancelled: [{ tool: 'cancel_purchase_order' }],
  },
  // Machines whose rule the repositories enforce (stateset_core
  // repository_rules). They have no generic status update, so no move is
  // `only` one tool's.
  vendor_return: {
    pending: [{ tool: 'submit_vendor_return' }],
    processed: [{ tool: 'process_vendor_return', note: 'generates the linked vendor credit' }],
    cancelled: [{ tool: 'cancel_vendor_return' }],
  },
  transfer_order: {
    in_transit: [{ tool: 'ship_transfer_order' }],
    partially_received: [{ tool: 'receive_transfer_order_line' }],
    received: [
      { tool: 'receive_transfer_order_line', note: 'receive the rest of every line' },
      {
        tool: 'complete_transfer_order_receiving',
        note: 'books any unit still in transit as a shortage',
      },
    ],
    cancelled: [{ tool: 'cancel_transfer_order' }],
  },
  edi_document: {
    pending: [{ tool: 'set_edi_document_status' }],
    sent: [{ tool: 'set_edi_document_status' }],
    acknowledged: [{ tool: 'set_edi_document_status' }],
    processed: [{ tool: 'set_edi_document_status' }],
    error: [{ tool: 'set_edi_document_status' }],
  },
  receipt: {
    in_progress: [
      { tool: 'start_receiving', from: ['expected'] },
      { tool: 'receive_receipt_items' },
    ],
    received: [{ tool: 'complete_receiving' }],
    cancelled: [
      {
        tool: 'cancel_receipt',
        note: 'refused for a purchase-order receipt once quantities are received',
      },
    ],
  },
  put_away: {
    assigned: [{ tool: 'assign_put_away' }],
    in_progress: [{ tool: 'start_put_away' }],
    completed: [{ tool: 'complete_put_away' }],
    cancelled: [{ tool: 'cancel_put_away' }],
  },
  wave: {
    released: [{ tool: 'release_fulfillment_wave' }],
    completed: [{ tool: 'complete_fulfillment_wave', note: 'every pick task must be finished' }],
    cancelled: [{ tool: 'cancel_fulfillment_wave' }],
  },
  pick_task: {
    assigned: [{ tool: 'assign_pick_task' }],
    in_progress: [{ tool: 'start_pick_task' }],
    cancelled: [{ tool: 'cancel_pick_task' }],
  },
  bom: {
    active: [{ tool: 'activate_bom' }],
  },
});

/**
 * Other names an agent may use for a catalogued machine (the generated
 * catalog knows only its name and Rust type). Kept from the names these
 * lifecycles answered to before they were catalogued.
 *
 * @type {Readonly<Record<string, string[]>>} machine -> aliases (snake_case)
 */
export const MACHINE_ALIASES = freeze({
  bom: ['bill_of_materials'],
  wave: ['fulfillment_wave'],
});

/**
 * Moves the engine's persistence layer accepts that the generated catalog does
 * not list, each citing the Rust rule it mirrors (`source`: file, then
 * function) and the code that shows it (`evidence`: regular expressions the
 * file must match). Without them an agent told "a pending payment can move to
 * processing, failed, cancelled" would never try complete_payment, which the
 * engine accepts. A unit test fails if the cited rule disappears. Folding them
 * into the catalog generator (a Rust change) would make this list empty.
 *
 * @type {Readonly<Record<string, { from: string, to: string, source: string, evidence: string[] }[]>>}
 */
export const PERSISTENCE_MOVES = freeze({
  payment: [
    {
      from: 'pending',
      to: 'completed',
      source:
        'crates/stateset-db/src/rules/payments.rs payment_transition_allowed (single-shot capture)',
      evidence: [
        'from == PaymentTransactionStatus::Pending && to == PaymentTransactionStatus::Completed',
      ],
    },
  ],
  // An immediate cancellation stores `expired` from any non-terminal status;
  // the catalog moves only trial, active and past_due into expired.
  subscription: ['paused', 'pending'].map((from) => ({
    from,
    to: 'expired',
    source:
      'crates/stateset-db/src/sqlite/subscriptions.rs cancel_subscription (immediate: true; refuses only terminal statuses)',
    evidence: ['if status\\.is_terminal\\(\\)', 'if immediate \\{ \\("expired", now\\) \\}'],
  })),
});

/**
 * Tools that take the target status as a parameter rather than implying it.
 * @type {Readonly<Record<string, string>>} tool -> name of its status parameter
 */
export const STATUS_PARAMETER_TOOLS = freeze({
  update_order_status: 'status',
  update_shipment: 'status',
  update_customer: 'status',
  update_product: 'status',
  set_edi_document_status: 'status',
});

/**
 * Tools that do not move a catalogued record themselves but are refused
 * because of one's status (e.g. a refund needs a completed payment). Their
 * refusals name that record's state, so they get the same guidance.
 * @type {Readonly<Record<string, string>>} tool -> catalog machine
 */
export const STATUS_GATED_TOOLS = freeze({
  create_refund: 'payment',
});

/**
 * @typedef {object} UncataloguedLifecycle
 * @property {string} name        lookup name, snake_case
 * @property {string} domain      tool domain, snake_case
 * @property {string} tsType      the binding's generated status type (bindings/node/index.d.ts)
 * @property {string[]} states    that type's members, snake_case
 * @property {string[]} [aliases] other names an agent may use
 * @property {string} [rustType]  engine enum, when it differs from `tsType`
 * @property {string} [note]
 */

/**
 * Lifecycles the engine has (each has a status enum in the binding) whose
 * transition rules are not in the generated catalog. `explain_state_machine`
 * names them as known-but-uncatalogued instead of "unknown", with their
 * states, and says plainly that it cannot judge their moves.
 *
 * @type {readonly UncataloguedLifecycle[]}
 */
export const UNCATALOGUED_LIFECYCLES = freeze([
  {
    name: 'order_payment',
    domain: 'orders',
    tsType: 'PaymentStatus',
    rustType: 'PaymentStatus',
    aliases: ['order_payment_status'],
    states: [
      'pending',
      'authorized',
      'paid',
      'partially_paid',
      'refunded',
      'partially_refunded',
      'failed',
    ],
    note: "Derived from the order's payments and refunds; it is never set directly.",
  },
  {
    name: 'order_fulfillment',
    domain: 'orders',
    tsType: 'FulfillmentStatus',
    rustType: 'FulfillmentStatus',
    aliases: ['order_fulfilment', 'fulfillment', 'fulfilment', 'order_fulfillment_status'],
    states: ['unfulfilled', 'partially_fulfilled', 'fulfilled', 'shipped', 'delivered'],
    note: 'Follows shipping (ship_order) and delivery of the order.',
  },
  {
    name: 'cart_payment',
    domain: 'carts',
    tsType: 'CartPaymentStatus',
    states: ['none', 'method_selected', 'authorized', 'captured', 'failed', 'refunded'],
  },
  {
    name: 'gift_card',
    domain: 'gift_cards',
    tsType: 'GiftCardStatus',
    states: ['active', 'depleted', 'expired', 'disabled'],
  },
  {
    name: 'store_credit',
    domain: 'store_credits',
    tsType: 'StoreCreditStatus',
    states: ['active', 'depleted', 'expired', 'voided'],
  },
  {
    name: 'promotion',
    domain: 'promotions',
    tsType: 'PromotionStatus',
    states: ['draft', 'scheduled', 'active', 'paused', 'expired', 'exhausted', 'archived'],
  },
  {
    name: 'coupon',
    domain: 'promotions',
    tsType: 'CouponStatus',
    states: ['active', 'disabled', 'exhausted', 'expired'],
  },
  {
    name: 'inventory_reservation',
    domain: 'inventory',
    tsType: 'ReservationStatus',
    aliases: ['reservation'],
    states: ['pending', 'confirmed', 'allocated', 'cancelled', 'released', 'expired', 'fulfilled'],
  },
  {
    name: 'invoice',
    domain: 'invoices',
    tsType: 'InvoiceStatus',
    states: [
      'draft',
      'sent',
      'viewed',
      'partially_paid',
      'paid',
      'overdue',
      'voided',
      'written_off',
      'disputed',
    ],
  },
  {
    name: 'backorder',
    domain: 'backorders',
    tsType: 'BackorderStatus',
    states: [
      'pending',
      'partially_fulfilled',
      'allocated',
      'ready_to_ship',
      'fulfilled',
      'cancelled',
    ],
  },
  {
    name: 'backorder_allocation',
    domain: 'backorders',
    tsType: 'BackorderAllocationStatus',
    states: ['reserved', 'confirmed', 'released', 'expired', 'fulfilled'],
  },
  {
    name: 'x402_intent',
    domain: 'x402',
    tsType: 'X402IntentStatus',
    aliases: ['x402_payment_intent', 'payment_intent_x402'],
    states: [
      'created',
      'signed',
      'sequenced',
      'batched',
      'settled',
      'expired',
      'failed',
      'cancelled',
    ],
  },
  {
    name: 'payment_obligation',
    domain: 'payment_obligations',
    tsType: 'PaymentObligationStatus',
    states: ['pending', 'scheduled', 'partially_paid', 'paid', 'cancelled'],
  },
  {
    name: 'gl_period',
    domain: 'general_ledger',
    tsType: 'GlPeriodStatus',
    aliases: ['accounting_period', 'period', 'fiscal_period'],
    states: ['future', 'open', 'closed', 'locked'],
  },
  {
    name: 'journal_entry',
    domain: 'general_ledger',
    tsType: 'GlJournalEntryStatus',
    states: ['draft', 'pending', 'posted', 'voided', 'reversed'],
  },
  {
    name: 'bill',
    domain: 'accounts_payable',
    tsType: 'BillStatus',
    states: [
      'draft',
      'pending',
      'approved',
      'partially_paid',
      'paid',
      'overdue',
      'cancelled',
      'disputed',
    ],
  },
  {
    name: 'credit_memo',
    domain: 'accounts_receivable',
    tsType: 'CreditMemoStatus',
    states: ['open', 'partially_applied', 'fully_applied', 'voided'],
  },
  {
    name: 'credit_account',
    domain: 'credit',
    tsType: 'CreditAccountStatus',
    states: ['active', 'suspended', 'on_hold', 'closed', 'pending_review'],
  },
  {
    name: 'warranty',
    domain: 'warranties',
    tsType: 'WarrantyStatus',
    states: ['active', 'expired', 'voided', 'transferred'],
  },
  {
    name: 'inspection',
    domain: 'quality',
    tsType: 'InspectionStatus',
    states: [
      'pending',
      'scheduled',
      'in_progress',
      'passed',
      'failed',
      'partial_pass',
      'on_hold',
      'cancelled',
    ],
  },
  {
    name: 'ncr',
    domain: 'quality',
    tsType: 'NcrStatus',
    aliases: ['non_conformance_report'],
    states: [
      'open',
      'under_review',
      'pending_disposition',
      'corrective_action',
      'preventive_action',
      'verification',
      'closed',
      'cancelled',
    ],
  },
  {
    name: 'quality_hold',
    domain: 'quality',
    tsType: 'QualityHoldStatus',
    states: ['held', 'released'],
  },
  {
    name: 'inbound_shipment',
    domain: 'inbound_shipments',
    tsType: 'InboundShipmentStatus',
    states: ['pending', 'in_transit', 'arrived', 'partially_received', 'received', 'cancelled'],
  },
  {
    name: 'production_batch',
    domain: 'production_batches',
    tsType: 'ProductionBatchStatus',
    states: ['planned', 'in_progress', 'completed', 'cancelled'],
  },
  {
    name: 'prepayment',
    domain: 'prepayments',
    tsType: 'PrepaymentStatus',
    states: ['open', 'applied', 'refunded', 'cancelled'],
  },
  {
    name: 'vendor_credit',
    domain: 'vendor_credits',
    tsType: 'VendorCreditStatus',
    states: ['open', 'applied', 'cancelled'],
  },
  {
    name: 'depreciation_entry',
    domain: 'fixed_assets',
    tsType: 'DepreciationEntryStatus',
    states: ['scheduled', 'posted'],
  },
  {
    name: 'review',
    domain: 'reviews',
    tsType: 'ReviewStatus',
    states: ['pending', 'approved', 'rejected', 'flagged'],
  },
  {
    name: 'print_job',
    domain: 'print_stations',
    tsType: 'PrintJobStatus',
    states: ['queued', 'picked_up', 'printed', 'failed'],
  },
]);

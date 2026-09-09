// Stub Backend — implements the smallest possible behavior that exercises
// the protocol end-to-end. A real Backend would call into the engine.
//
// The Backend trait from handler-design.md:
//   quote(intent) -> Quote | error
//   accept(quote_id, accept_envelope) -> EscrowFunding | error
//   fulfill(escrow_id, evidence) -> FulfillmentReceipt | error
//   observe(intent_id) -> stream of EscrowEvent
//   dispute(escrow_id, dispute_intent) -> DisputeOutcome | error

import { canonicalJson, signEd25519, newId, newNonceHex } from './codec.mjs';
import { createHash } from 'node:crypto';
import { priceDemoQuote, amount, exactMoney, roundCents, quantity } from './quote-money.mjs';
import { availableInventory, collection } from './state.mjs';

function invalidMoney(error) {
  return { ok: false, error: { type: 'icp.error', code: 'format.invalid_money', message: error.message } };
}

/**
 * Build, sign, and return a Quote for an Intent. The stub:
 *   - sums (quantity * unit_price) per item
 *   - applies a flat 5% "demo handling fee" (fresh pricing)
 *   - OR honors prices from a referenced PriceProposal if from_proposal_id is set (§6.4)
 *   - rejects if total would exceed Intent.max_total
 */
export function stubQuote(intent, merchantSigningKey) {
  // ICPIP-0003: honor an existing PriceProposal if referenced.
  if (intent.from_proposal_id) {
    const proposal = getProposal(intent.from_proposal_id);
    if (!proposal) {
      return {
        ok: false,
        error: { type: 'icp.error', code: 'quote.proposal_not_found', message: `proposal ${intent.from_proposal_id} unknown` },
      };
    }
    if (new Date(proposal.valid_until) < new Date()) {
      return {
        ok: false,
        error: { type: 'icp.error', code: 'quote.proposal_expired', message: `proposal ${intent.from_proposal_id} expired at ${proposal.valid_until}` },
      };
    }
    let exceedsMaximum;
    try {
      exceedsMaximum = proposal.total.currency !== intent.max_total.currency
        || amount(proposal.total.amount) > amount(intent.max_total.amount);
    } catch (error) { return invalidMoney(error); }
    if (exceedsMaximum || proposal.merchant !== intent.merchant) {
      return {
        ok: false,
        error: { type: 'icp.error', code: 'quote.proposal_total_mismatch', message: 'proposal violates the purchase ceiling, currency, or merchant constraints' },
      };
    }
    // Use proposal prices verbatim.
    const now = new Date();
    const exp = new Date(now.getTime() + 5 * 60 * 1000);
    const quote = {
      type: 'quote',
      v: 'icp-1.0',
      quote_id: newId('icp_qt'),
      intent_id: intent.intent_id,
      merchant: intent.merchant,
      total: proposal.total,
      lines: proposal.items.map((li) => ({ sku: li.sku, quantity: li.quantity, unit_price: li.unit_price, line_total: li.line_total })),
      settler: intent.settler,
      escrow_terms: { release_on: 'fulfilled+24h', dispute_window: '168h' },
      expiry: exp.toISOString(),
      from_proposal_id: intent.from_proposal_id,
      nonce: newNonceHex(), // §5.3: every signed payload carries a fresh, unique nonce
      iat: now.toISOString(),
      exp: exp.toISOString(),
    };
    const canonical = canonicalJson(quote);
    const signatureHex = signEd25519(canonical, merchantSigningKey);
    return { ok: true, quote, canonical, signatureHex };
  }

  let priced;
  try {
    priced = priceDemoQuote(intent.items, intent.max_total);
  } catch (error) {
    return { ok: false, error: { type: 'icp.error', code: 'format.invalid_money', message: error.message } };
  }
  const total = priced.amount;

  if (priced.exceedsMaximum) {
    return {
      ok: false,
      error: {
        type: 'icp.error',
        code: 'policy.quote.exceeds_max_total',
        message: `quote total ${total} > max_total ${intent.max_total.amount}`,
      },
    };
  }

  const now = new Date();
  const exp = new Date(now.getTime() + 5 * 60 * 1000); // 5-minute Quote validity

  const quote = {
    type: 'quote',
    v: 'icp-1.0',
    quote_id: newId('icp_qt'),
    intent_id: intent.intent_id,
    merchant: intent.merchant,
    total: { amount: total, currency: intent.max_total.currency },
    lines: priced.lines,
    settler: intent.settler,
    escrow_terms: {
      release_on: 'fulfilled+24h',
      dispute_window: '168h',
    },
    expiry: exp.toISOString(),
    nonce: newNonceHex(), // §5.3: every signed payload carries a fresh, unique nonce
    iat: now.toISOString(),
    exp: exp.toISOString(),
  };

  const canonical = canonicalJson(quote);
  const signatureHex = signEd25519(canonical, merchantSigningKey);
  return { ok: true, quote, canonical, signatureHex };
}

/**
 * Sign a SubscriptionAuthorization for a subscription.create Intent.
 * Returns a SubscriptionAuthorization signed by the merchant key, or an
 * error if the merchant's policy rejects the request.
 *
 * The stub:
 *   - Accepts cadences 1d, 7d, 30d, 1y
 *   - Pro-rated refund policy
 *   - Immediate cancellation OK
 *   - 12-month auto-expiry
 *   - Rejects max_total_per_period > $1000/period (demo policy cap)
 */
export function stubSubscriptionAuthorize(intent, merchantSigningKey, merchantAid) {
  let cap;
  try { cap = amount(intent.max_total_per_period.amount); }
  catch (error) { return invalidMoney(error); }
  if (cap > amount('1000')) {
    return {
      ok: false,
      error: {
        type: 'icp.error',
        code: 'policy.value_above_kyc_floor',
        message: `subscription max_total_per_period $${exactMoney(cap)} exceeds stub policy cap of $1000`,
      },
    };
  }

  const now = new Date();
  const expiry = new Date(now.getTime() + 365 * 86400 * 1000); // 12 months

  const auth = {
    type: 'subscription.authorization',
    v: 'icp-1.0',
    subscription_id: newId('icp_sub'),
    intent_id: intent.intent_id,
    merchant: intent.merchant,
    service_id: intent.service_id,
    cadence: intent.cadence,
    max_total_per_period: intent.max_total_per_period,
    max_occurrences: intent.max_occurrences ?? null,
    first_charge_at: intent.first_charge_at,
    merchant_terms: {
      cancellation_notice_period: '0d',
      refund_policy: 'pro-rated',
      service_grant_per_period: `${intent.service_id}-${intent.cadence}`,
    },
    expiry: expiry.toISOString(),
    iat: now.toISOString(),
  };

  const canonical = canonicalJson(auth);
  const signatureHex = signEd25519(canonical, merchantSigningKey);
  // Signature lives in the outer envelope per ICP-1.0 §5.1 — do NOT embed
  // inside the payload. Embedding makes round-trip verification fail because
  // the client recomputes canonical bytes from the deserialized object,
  // which would then include the signature field.
  return { ok: true, authorization: auth, canonical, signatureHex };
}

/**
 * Sign a ReturnAuthorization for a purchase.return Intent (§6.2).
 *
 * The stub:
 *   - Accepts any original_settlement_id (no persistence layer for the demo)
 *   - Refund outcome only (skips replacement/credit complexity)
 *   - Refund amount: caps at Intent.max_refund if present, else 10x first-item-quantity at $10
 *   - 5-day expected settlement window
 *   - Rejects items where the reason is "no-longer-needed" if max_refund > $500
 *     (demo policy: large no-fault returns require human review)
 */
export function stubReturnAuthorize(intent, merchantSigningKey, merchantAid) {
  // Compute refund amount.
  let refundAmount;
  try {
    if (!Array.isArray(intent.items) || !intent.items.length) throw new Error('return items required');
    refundAmount = intent.items.reduce((sum, it) => sum + quantity(it.quantity) * amount('10'), 0n);
    if (intent.max_refund) {
      const cap = amount(intent.max_refund.amount);
      if (refundAmount > cap) refundAmount = cap;
    }
  } catch (error) { return invalidMoney(error); }

  // Demo policy: large no-fault returns get rejected.
  const hasNoFault = intent.items.some((it) => it.reason === 'no-longer-needed');
  if (hasNoFault && refundAmount > amount('500')) {
    return {
      ok: false,
      error: {
        type: 'icp.error',
        code: 'policy.return.not_eligible',
        message: 'large no-fault returns (>$500) require human review in this demo backend',
      },
    };
  }

  const currency = intent.max_refund?.currency ?? 'USDC';
  const now = new Date();
  const auth = {
    type: 'return.authorization',
    v: 'icp-1.0',
    return_id: newId('icp_ret'),
    intent_id: intent.intent_id,
    original_settlement_id: intent.original_settlement_id,
    merchant: intent.merchant,
    outcome: intent.desired_outcome === 'replacement' ? 'replacement' : 'refund',
    refund:
      intent.desired_outcome === 'replacement'
        ? null
        : {
            amount: { amount: exactMoney(refundAmount), currency },
            rail: 'base-sepolia',
            release_to: '<buyer-wallet-address>',
            expected_settlement_within: '5d',
          },
    merchant_terms: {
      return_shipping_label_url: 'https://example.com/rma/' + intent.intent_id,
      rma_code: 'RMA-' + intent.intent_id.slice(-8),
      must_return_by: new Date(now.getTime() + 14 * 86400 * 1000).toISOString(),
    },
    iat: now.toISOString(),
  };

  const canonical = canonicalJson(auth);
  const signatureHex = signEd25519(canonical, merchantSigningKey);
  // Signature lives in the outer envelope per ICP-1.0 §5.1 — do NOT embed
  // inside the payload. Embedding makes round-trip verification fail because
  // the client recomputes canonical bytes from the deserialized object,
  // which would then include the signature field.
  return { ok: true, authorization: auth, canonical, signatureHex };
}

/**
 * Sign an InventorySnapshot for an inventory.query Intent (§6.3).
 *
 * The stub maintains a small fixed catalog and returns the subset the
 * buyer requested (or the full catalog if `skus` is empty/missing). All
 * prices are in the same currency the buyer named in `settler`.
 */
export function stubInventoryQuery(intent, merchantSigningKey, merchantAid) {
  const CATALOG = {
    'WIDGET-001': { available_quantity: 47, unit_price: { amount: '29.99', currency: 'USDC' }, metadata: { lead_time_days: 2, weight_g: 250 } },
    'WIDGET-002': { available_quantity: 0,  unit_price: { amount: '49.99', currency: 'USDC' }, metadata: { restock_eta: '2026-05-19T00:00:00Z' } },
    'WIDGET-003': { available_quantity: 12, unit_price: { amount: '99.99', currency: 'USDC' }, metadata: { lead_time_days: 5 } },
    'GADGET-A':   { available_quantity: 200, unit_price: { amount: '4.99',  currency: 'USDC' }, metadata: { category: 'consumable' } },
    'GADGET-B':   { available_quantity: 100, unit_price: { amount: '9.99',  currency: 'USDC' }, metadata: { category: 'consumable' } },
  };

  // Report the same balances that acceptance reserves, not catalog seed counts.
  for (const [sku, entry] of Object.entries(CATALOG)) {
    entry.available_quantity = availableInventory(sku);
  }

  // Filter SKUs.
  let skuList;
  if (Array.isArray(intent.skus) && intent.skus.length > 0) {
    skuList = intent.skus.map((s) => s.sku).filter((sku) => CATALOG[sku]);
  } else {
    skuList = Object.keys(CATALOG);
  }

  // Filter by in_stock_only if requested.
  if (intent.filters?.in_stock_only) {
    skuList = skuList.filter((sku) => CATALOG[sku].available_quantity > 0);
  }

  // Apply max_results cap.
  const cap = Math.min(intent.max_results ?? 100, 100);
  const totalMatching = skuList.length;
  skuList = skuList.slice(0, cap);

  const items = skuList.map((sku) => ({ sku, ...CATALOG[sku] }));

  const now = new Date();
  const validUntil = new Date(now.getTime() + 5 * 60 * 1000); // 5-minute validity

  const snapshot = {
    type: 'inventory.snapshot',
    v: 'icp-1.0',
    snapshot_id: newId('icp_inv'),
    intent_id: intent.intent_id,
    merchant: intent.merchant,
    snapshot_taken_at: now.toISOString(),
    valid_until: validUntil.toISOString(),
    items,
    total_matching_skus: totalMatching,
    iat: now.toISOString(),
  };

  const canonical = canonicalJson(snapshot);
  const signatureHex = signEd25519(canonical, merchantSigningKey);
  // Signature lives in the outer envelope per ICP-1.0 §5.1 (see comment above).
  return { ok: true, snapshot, canonical, signatureHex };
}

/**
 * Sign a CancellationAuthorization for a subscription.cancel Intent (§6.5.1).
 *
 * The stub:
 *   - Treats every subscription_id as cancellable (no persistence layer
 *     for the demo, so we can't enforce subscription.not_found).
 *   - Honors the buyer's `effective` preference if "end-of-period"; downgrades
 *     "immediate" → "end-of-period" if the demo policy treats the subscription
 *     as non-refundable (subscription_id ending in "ANNUAL").
 *   - Issues pro-rated refund of $7.50 (half a $15/month subscription) when
 *     the cancellation is immediate.
 */
export function stubSubscriptionCancel(intent, merchantSigningKey, merchantAid) {
  const now = new Date();
  const isAnnual = intent.subscription_id.endsWith('ANNUAL');

  let effectiveAt;
  let proRatedRefund = null;

  if (intent.effective === 'immediate' && !isAnnual) {
    effectiveAt = now.toISOString();
    proRatedRefund = {
      amount: { amount: '7.50', currency: 'USDC' },
      rail: 'base-sepolia',
      release_to: '<buyer-wallet-address>',
      expected_settlement_within: '5d',
    };
  } else {
    // End-of-period — assume 15 days remain in the current cycle.
    effectiveAt = new Date(now.getTime() + 15 * 86400 * 1000).toISOString();
  }

  const auth = {
    type: 'subscription.cancellation',
    v: 'icp-1.0',
    cancellation_id: newId('icp_can'),
    intent_id: intent.intent_id,
    subscription_id: intent.subscription_id,
    merchant: intent.merchant,
    effective_at: effectiveAt,
    final_occurrences: intent.effective === 'immediate' && !isAnnual ? 0 : 1,
    pro_rated_refund: proRatedRefund,
    iat: now.toISOString(),
  };

  const canonical = canonicalJson(auth);
  const signatureHex = signEd25519(canonical, merchantSigningKey);
  return { ok: true, authorization: auth, canonical, signatureHex };
}

// In-memory store of issued PriceProposals (keyed by proposal_id) so the
// from_proposal_id flow can validate non-expired proposals. Production
// handlers persist this via the engine.
const _proposalStore = collection('proposals');

/**
 * Sign a PriceProposal for a quote.request Intent (§6.4 / ICPIP-0003).
 *
 * The stub:
 *   - Catalog same as inventory.query, plus volume-tier discounts:
 *     1–99 units: catalog price
 *     100–499:  10% off
 *     500+:     20% off
 *   - 30-day valid_until window
 *   - Rejects quantities > 10000 with policy.quote.not_available_for_quantity
 */
export function stubQuoteRequest(intent, merchantSigningKey, merchantAid) {
  const CATALOG = {
    'WIDGET-001':    { unit_price: { amount: '29.99', currency: 'USDC' } },
    'WIDGET-002':    { unit_price: { amount: '49.99', currency: 'USDC' } },
    'WIDGET-003':    { unit_price: { amount: '99.99', currency: 'USDC' } },
    'FASTENER-M6X20': { unit_price: { amount: '0.15',  currency: 'USDC' } },
    'GADGET-A':      { unit_price: { amount: '4.99',  currency: 'USDC' } },
    'GADGET-B':      { unit_price: { amount: '9.99',  currency: 'USDC' } },
  };

  let total = 0n;
  const lineItems = [];
  if (!Array.isArray(intent.items) || !intent.items.length) return invalidMoney(new Error('quote items required'));
  for (const item of intent.items) {
    try { quantity(item.quantity); } catch (error) { return invalidMoney(error); }
    if (item.quantity > 10000) {
      return {
        ok: false,
        error: {
          type: 'icp.error',
          code: 'policy.quote.not_available_for_quantity',
          message: `quantity ${item.quantity} for ${item.sku} exceeds quotable range (max 10000)`,
        },
      };
    }
    const catalog = CATALOG[item.sku];
    if (!catalog) {
      return {
        ok: false,
        error: {
          type: 'icp.error',
          code: 'policy.quote.sku_not_quotable',
          message: `SKU ${item.sku} is not in the merchant catalog`,
        },
      };
    }
    // Volume-tier discount.
    const basePrice = amount(catalog.unit_price.amount);
    const percent = item.quantity >= 500 ? 80n : item.quantity >= 100 ? 90n : 100n;
    const tieredUnitPrice = basePrice * percent / 100n;
    const lineTotal = roundCents(tieredUnitPrice * quantity(item.quantity));
    total += lineTotal;
    lineItems.push({
      sku: item.sku,
      quantity: item.quantity,
      unit_price: { amount: exactMoney(tieredUnitPrice), currency: catalog.unit_price.currency },
      line_total: { amount: exactMoney(lineTotal), currency: catalog.unit_price.currency },
      volume_tier: item.quantity >= 500 ? '500+' : item.quantity >= 100 ? '100-499' : '1-99',
    });
  }

  const now = new Date();
  const validUntil = new Date(now.getTime() + 30 * 86400 * 1000); // 30 days

  const proposal = {
    type: 'price.proposal',
    v: 'icp-1.0',
    proposal_id: newId('icp_pp'),
    intent_id: intent.intent_id,
    merchant: intent.merchant,
    issued_at: now.toISOString(),
    valid_until: validUntil.toISOString(),
    items: lineItems,
    total: { amount: exactMoney(total), currency: 'USDC' },
    payment_terms: { net_days: 30, early_pay_discount: { percent: '2', if_paid_within_days: 10 } },
    fulfillment_terms: { lead_time_days: 7, shipping_method: 'ground' },
    return_policy_summary: '30 days, full refund, buyer pays return shipping',
    non_binding_notice:
      'This proposal is informational and does not commit either party. To purchase, submit a purchase.create Intent referencing this proposal_id.',
  };

  // Persist for from_proposal_id lookup.
  _proposalStore.set(proposal.proposal_id, proposal);

  const canonical = canonicalJson(proposal);
  const signatureHex = signEd25519(canonical, merchantSigningKey);
  return { ok: true, proposal, canonical, signatureHex };
}

/** Internal: lookup a proposal by id. Used by the from_proposal_id path in stubQuote. */
export function getProposal(proposal_id) {
  return _proposalStore.get(proposal_id);
}

// In-memory seller balance ledger for payout.request (§6.6 / ICPIP-0004).
// In production this maps to the platform's actual held-funds ledger.
// Demo: every seller starts with $5000 USDC available unless overridden.
const _sellerBalances = collection('seller_balances');

function getSellerBalance(sellerAid) {
  if (!_sellerBalances.has(sellerAid)) {
    _sellerBalances.set(sellerAid, '5000'); // demo balance, never real funds
  }
  return amount(_sellerBalances.get(sellerAid));
}

/**
 * Sign a PayoutAuthorization for a payout.request Intent (§6.6 / ICPIP-0004).
 *
 * The stub:
 *   - Tracks seller balance in-memory; each new seller starts at $5000 USDC
 *   - Rejects if requested amount > available_balance with insufficient_balance
 *   - Honors `max_per_payout` from PrincipalBinding when present
 *   - Applies 3% platform commission + 1% chargeback reserve (released after 90d)
 *   - Computes approved_amount = available_balance - sum(fees)
 */
export function stubPayoutRequest(intent, merchantSigningKey, platformAid) {
  let requested;
  const maxPerPayout = intent.principal_binding?.authority?.max_per_payout;
  let maximum;
  try {
    requested = amount(intent.amount.amount);
    if (requested === 0n || intent.amount.currency !== 'USDC') throw new Error('positive USDC payout required');
    if (maxPerPayout) {
      if (maxPerPayout.currency !== intent.amount.currency) throw new Error('payout authority currency mismatch');
      maximum = amount(maxPerPayout.amount);
    }
  } catch (error) { return invalidMoney(error); }
  const available = getSellerBalance(intent.seller);

  if (requested > available) {
    return {
      ok: false,
      error: {
        type: 'icp.error',
        code: 'policy.payout.insufficient_balance',
        message: `requested ${exactMoney(requested)} ${intent.amount.currency} exceeds available balance ${exactMoney(available)}`,
      },
    };
  }

  // Honor max_per_payout from PrincipalBinding (OPTIONAL field per ICPIP-0004).
  if (maxPerPayout && requested > maximum) {
    return {
      ok: false,
      error: {
        type: 'icp.error',
        code: 'policy.payout.exceeds_max_per_payout',
        message: `requested ${exactMoney(requested)} exceeds principal binding max_per_payout ${maxPerPayout.amount}`,
      },
    };
  }

  // Fees: 3% platform commission + 1% chargeback reserve (release after 90 days).
  const commission = roundCents(requested * 3n / 100n);
  const reserve = roundCents(requested / 100n);
  const approved = requested - commission - reserve;

  const now = new Date();
  const releaseAt = new Date(now.getTime() + 90 * 86400 * 1000);

  const auth = {
    type: 'payout.authorization',
    v: 'icp-1.0',
    payout_id: newId('icp_pay'),
    intent_id: intent.intent_id,
    seller: intent.seller,
    platform: intent.platform,
    available_balance: { amount: exactMoney(available), currency: intent.amount.currency },
    approved_amount: { amount: exactMoney(approved), currency: intent.amount.currency },
    fees: [
      {
        type: 'platform_commission',
        amount: { amount: exactMoney(commission), currency: intent.amount.currency },
        description: 'Standard 3% platform commission',
      },
      {
        type: 'chargeback_reserve',
        amount: { amount: exactMoney(reserve), currency: intent.amount.currency },
        description: '1% chargeback reserve (released after 90 days)',
        release_at: releaseAt.toISOString(),
      },
    ],
    rail: 'base-sepolia',
    rail_initiated_at: now.toISOString(),
    expected_settlement_at: new Date(now.getTime() + 30 * 1000).toISOString(),
    issued_at: now.toISOString(),
  };

  const canonical = canonicalJson(auth);
  const signatureHex = signEd25519(canonical, merchantSigningKey);
  // Do not debit if receipt signing fails.
  _sellerBalances.set(intent.seller, exactMoney(available - requested));
  return { ok: true, authorization: auth, canonical, signatureHex };
}

/** For testing: set a seller's available balance directly. */
export function _seedSellerBalance(sellerAid, value) {
  _sellerBalances.set(sellerAid, exactMoney(amount(value)));
}

/**
 * Sign a ChannelRegistration for a channel.register Intent (ICPIP-0005).
 *
 * Stub semantics:
 *   - Accepts webhook OR sse channels.
 *   - For webhooks, validates that `channel.url` is `https://…` (no
 *     plaintext destinations in production).
 *   - For SSE, mints an opaque subscription token with 1h TTL.
 *   - Echoes the requested `event_filters` (no per-channel allowlist
 *     enforcement in the stub; production would gate by entitlement).
 *   - Stores the registration in the supplied in-memory `channelStore`
 *     and assigns a fresh `channel_id`.
 *
 * Returns `{ ok: true, channel, canonical, signatureHex }` on success
 * or `{ ok: false, error }` on policy rejection.
 */
export function stubChannelRegister(intent, merchantSigningKey, merchantAid, channelStore) {
  const ch = intent.channel;
  if (!ch || typeof ch !== 'object') {
    return {
      ok: false,
      error: {
        type: 'icp.error',
        code: 'format.missing_field',
        message: 'channel.register requires intent.channel',
      },
    };
  }
  if (ch.type !== 'webhook' && ch.type !== 'sse') {
    return {
      ok: false,
      error: {
        type: 'icp.error',
        code: 'format.unknown_channel_type',
        message: `unknown channel.type "${ch.type}" (expected webhook|sse)`,
      },
    };
  }
  if (ch.type === 'webhook') {
    if (!ch.url) {
      return {
        ok: false,
        error: {
          type: 'icp.error',
          code: 'channel.url_unverified',
          message: 'webhook channel.url is required',
        },
      };
    }
    // Production requires https://. Loopback addresses (127.0.0.1, localhost)
    // are permitted for dev / CI / integration tests so a local mock receiver
    // can exercise the end-to-end emit path without TLS.
    const isHttps = ch.url.startsWith('https://');
    const isLoopback =
      ch.url.startsWith('http://127.0.0.1') ||
      ch.url.startsWith('http://localhost');
    if (!isHttps && !isLoopback) {
      return {
        ok: false,
        error: {
          type: 'icp.error',
          code: 'channel.url_unverified',
          message: 'webhook channel.url must be https:// (loopback http:// allowed for dev only)',
        },
      };
    }
  }

  const channelId = newId('icp_ch');
  const now = new Date();
  const expiresAt = new Date(now.getTime() + 30 * 86_400 * 1000); // 30 days

  const channel = {
    type: 'channel.registration',
    v: 'icp-1.0',
    channel_id: channelId,
    intent_id: intent.intent_id,
    agent: intent.buyer,
    merchant: intent.merchant,
    channel_type: ch.type,
    events_registered: Array.isArray(ch.event_filters) ? ch.event_filters : [],
    registered_at: now.toISOString(),
    expires_at: expiresAt.toISOString(),
  };
  if (ch.type === 'webhook') {
    channel.webhook_url = ch.url;
    channel.delivery = ch.delivery ?? {
      max_attempts: 8,
      backoff: 'exponential',
      initial_delay_seconds: 5,
    };
  } else {
    // SSE: mint short-lived subscription token (opaque, server-keyed).
    channel.sse_endpoint = `https://${intent.merchant.replace(/^aid:v1:z/, '')}.example/icp/v1/events/sse`;
    channel.subscription_token = newId('tok');
    channel.token_ttl_seconds = 3600;
  }

  // Persist registration so /icp/v1/channels/:id can return it.
  channelStore.set(channelId, channel);

  const canonical = canonicalJson(channel);
  const signatureHex = signEd25519(canonical, merchantSigningKey);
  return { ok: true, channel, canonical, signatureHex };
}

/** Build EscrowFunding instructions. The stub points at the testnet
 *  ICPEscrow contract; production would compute escrowId and surface
 *  the funding tx encoder. */
export function stubFundingInstructions(quote) {
  const escrowId = '0x' + createHash('sha256')
    .update(canonicalJson(['icp.reference.escrow.v1', quote.intent_id, quote.quote_id])).digest('hex');

  return {
    escrow_id: escrowId,
    settler: quote.settler,
    chain: 'base-sepolia',
    contract: '0x0000000000000000000000000000000000000000', // set after deploy
    function: 'fund',
    args: {
      escrowId,
      buyer: '<buyer-wallet-address>',
      merchant: '<merchant-payout-address>',
      amount: quote.total.amount,
      fulfillmentDeadline: Math.floor(Date.parse(quote.iat) / 1000) + 86400,
      disputeWindow: 7 * 86400,
      quoteHash: '0x' + createHash('sha256').update(canonicalJson(quote)).digest('hex'),
    },
    note: 'STUB. Production handler returns a fully-encoded calldata blob.',
  };
}

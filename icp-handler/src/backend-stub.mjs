// Stub Backend — implements the smallest possible behavior that exercises
// the protocol end-to-end. A real Backend would call into the engine.
//
// The Backend trait from handler-design.md:
//   quote(intent) -> Quote | error
//   accept(quote_id, accept_envelope) -> EscrowFunding | error
//   fulfill(escrow_id, evidence) -> FulfillmentReceipt | error
//   observe(intent_id) -> stream of EscrowEvent
//   dispute(escrow_id, dispute_intent) -> DisputeOutcome | error

import { canonicalJson, signEd25519, newId } from './codec.mjs';

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
    if (proposal.total.amount !== intent.max_total.amount) {
      return {
        ok: false,
        error: { type: 'icp.error', code: 'quote.proposal_total_mismatch', message: `purchase.create max_total ${intent.max_total.amount} does not match proposal.total ${proposal.total.amount}` },
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
      nonce: '00000000000000000000000000000000',
      iat: now.toISOString(),
      exp: exp.toISOString(),
    };
    const canonical = canonicalJson(quote);
    const signatureHex = signEd25519(canonical, merchantSigningKey);
    return { ok: true, quote, canonical, signatureHex };
  }

  let total = 0;
  for (const item of intent.items) {
    total += item.quantity * Number(item.unit_price.amount);
  }
  total = Math.round(total * 1.05 * 100) / 100; // 5% fee, 2dp

  if (total > Number(intent.max_total.amount)) {
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
    total: { amount: total.toFixed(2), currency: intent.max_total.currency },
    lines: intent.items.map((it) => ({
      sku: it.sku,
      quantity: it.quantity,
      unit_price: it.unit_price,
      line_total: {
        amount: (it.quantity * Number(it.unit_price.amount)).toFixed(2),
        currency: it.unit_price.currency,
      },
    })),
    settler: intent.settler,
    escrow_terms: {
      release_on: 'fulfilled+24h',
      dispute_window: '168h',
    },
    expiry: exp.toISOString(),
    nonce: '00000000000000000000000000000000', // demo: deterministic for replay
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
  const cap = Number(intent.max_total_per_period.amount);
  if (cap > 1000) {
    return {
      ok: false,
      error: {
        type: 'icp.error',
        code: 'policy.value_above_kyc_floor',
        message: `subscription max_total_per_period $${cap} exceeds stub policy cap of $1000`,
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
  const itemsTotal = intent.items.reduce((sum, it) => sum + it.quantity * 10, 0); // demo: $10/item
  let refundAmount = itemsTotal;
  if (intent.max_refund) {
    const cap = Number(intent.max_refund.amount);
    if (refundAmount > cap) refundAmount = cap;
  }

  // Demo policy: large no-fault returns get rejected.
  const hasNoFault = intent.items.some((it) => it.reason === 'no-longer-needed');
  if (hasNoFault && refundAmount > 500) {
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
            amount: { amount: refundAmount.toFixed(2), currency },
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
const _proposalStore = new Map();

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

  let total = 0;
  const lineItems = [];
  for (const item of intent.items) {
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
    const basePrice = Number(catalog.unit_price.amount);
    const discount = item.quantity >= 500 ? 0.20 : item.quantity >= 100 ? 0.10 : 0;
    const tieredUnitPrice = basePrice * (1 - discount);
    const lineTotal = tieredUnitPrice * item.quantity;
    total += lineTotal;
    lineItems.push({
      sku: item.sku,
      quantity: item.quantity,
      unit_price: { amount: tieredUnitPrice.toFixed(2), currency: catalog.unit_price.currency },
      line_total: { amount: lineTotal.toFixed(2), currency: catalog.unit_price.currency },
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
    total: { amount: total.toFixed(2), currency: 'USDC' },
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
const _sellerBalances = new Map();

function getSellerBalance(sellerAid) {
  if (!_sellerBalances.has(sellerAid)) {
    _sellerBalances.set(sellerAid, 5000.0); // demo: every seller starts with $5000
  }
  return _sellerBalances.get(sellerAid);
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
  const requested = Number(intent.amount.amount);
  const available = getSellerBalance(intent.seller);

  if (requested > available) {
    return {
      ok: false,
      error: {
        type: 'icp.error',
        code: 'policy.payout.insufficient_balance',
        message: `requested ${requested} ${intent.amount.currency} exceeds available balance ${available.toFixed(2)}`,
      },
    };
  }

  // Honor max_per_payout from PrincipalBinding (OPTIONAL field per ICPIP-0004).
  const maxPerPayout = intent.principal_binding?.authority?.max_per_payout;
  if (maxPerPayout && requested > Number(maxPerPayout.amount)) {
    return {
      ok: false,
      error: {
        type: 'icp.error',
        code: 'policy.payout.exceeds_max_per_payout',
        message: `requested ${requested} exceeds principal binding max_per_payout ${maxPerPayout.amount}`,
      },
    };
  }

  // Fees: 3% platform commission + 1% chargeback reserve (release after 90 days).
  const commission = +(requested * 0.03).toFixed(2);
  const reserve = +(requested * 0.01).toFixed(2);
  const approved = +(requested - commission - reserve).toFixed(2);

  const now = new Date();
  const releaseAt = new Date(now.getTime() + 90 * 86400 * 1000);

  const auth = {
    type: 'payout.authorization',
    v: 'icp-1.0',
    payout_id: newId('icp_pay'),
    intent_id: intent.intent_id,
    seller: intent.seller,
    platform: intent.platform,
    available_balance: { amount: available.toFixed(2), currency: intent.amount.currency },
    approved_amount: { amount: approved.toFixed(2), currency: intent.amount.currency },
    fees: [
      {
        type: 'platform_commission',
        amount: { amount: commission.toFixed(2), currency: intent.amount.currency },
        description: 'Standard 3% platform commission',
      },
      {
        type: 'chargeback_reserve',
        amount: { amount: reserve.toFixed(2), currency: intent.amount.currency },
        description: '1% chargeback reserve (released after 90 days)',
        release_at: releaseAt.toISOString(),
      },
    ],
    rail: 'base-sepolia',
    rail_initiated_at: now.toISOString(),
    expected_settlement_at: new Date(now.getTime() + 30 * 1000).toISOString(),
    issued_at: now.toISOString(),
  };

  // Deduct from seller's balance.
  _sellerBalances.set(intent.seller, available - requested);

  const canonical = canonicalJson(auth);
  const signatureHex = signEd25519(canonical, merchantSigningKey);
  return { ok: true, authorization: auth, canonical, signatureHex };
}

/** For testing: set a seller's available balance directly. */
export function _seedSellerBalance(sellerAid, amount) {
  _sellerBalances.set(sellerAid, amount);
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
  const escrowId = '0x' + Buffer.from(`${quote.intent_id}:${quote.quote_id}`)
    .toString('hex')
    .padEnd(64, '0')
    .slice(0, 64);

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
      fulfillmentDeadline: Math.floor(Date.now() / 1000) + 86400,
      disputeWindow: 7 * 86400,
      quoteHash: '0x' + canonicalJson(quote).split('').reduce((h, c) =>
        ((h << 5) - h + c.charCodeAt(0)) | 0, 0).toString(16).padStart(64, '0').slice(0, 64),
    },
    note: 'STUB. Production handler returns a fully-encoded calldata blob.',
  };
}

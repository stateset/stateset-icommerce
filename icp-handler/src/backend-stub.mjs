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
 *   - applies a flat 5% "demo handling fee"
 *   - rejects if total would exceed Intent.max_total
 */
export function stubQuote(intent, merchantSigningKey) {
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
  auth.signature = { alg: 'ed25519', kid: merchantAid, sig: signatureHex };
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
  auth.signature = { alg: 'ed25519', kid: merchantAid, sig: signatureHex };
  return { ok: true, authorization: auth, canonical, signatureHex };
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

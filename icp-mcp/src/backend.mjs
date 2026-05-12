// Self-contained ICP backend for the MCP server.
//
// Mirrors icp-handler logic but with private per-process state, so the MCP
// server can run standalone without depending on the HTTP server. Same
// codec, same backend-stub semantics — different storage.

import {
  canonicalJson,
  verifyEd25519,
  signEd25519,
  newId,
  newNonceHex,
  pubkeyForAid,
} from '../../icp-handler/src/codec.mjs';
import {
  stubQuote,
  stubFundingInstructions,
  stubSubscriptionAuthorize,
  stubSubscriptionCancel,
  stubReturnAuthorize,
  stubInventoryQuery,
  stubQuoteRequest,
  stubPayoutRequest,
} from '../../icp-handler/src/backend-stub.mjs';

const intents = new Map();
const quotes = new Map();
const escrows = new Map();
const events = new Map();
const settlements = new Map();

export const ALLOWED_SETTLERS = new Set([
  'settler:stateset.usdc.base-sepolia',
  'settler:circle.usdc.base',
]);

const SUPPORTED_VERBS = new Set([
  'purchase.create',
  'subscription.create',
  'subscription.cancel',
  'purchase.return',
  'inventory.query',
  'quote.request',
  'payout.request',
]);

export function submitIntent({ intent, signature, _pubkey_hex }, merchantSigningKey, merchantAid) {
  if (!intent || !signature) return { ok: false, error: { code: 'format.missing_field', message: 'expected { intent, signature }' } };
  if (intent.v !== 'icp-1.0') return { ok: false, error: { code: 'version.unsupported', message: `unknown spec version ${intent.v}` } };
  if (!SUPPORTED_VERBS.has(intent.verb)) return { ok: false, error: { code: 'format.unknown_verb', message: `verb ${intent.verb} not implemented` } };
  if (!ALLOWED_SETTLERS.has(intent.settler)) return { ok: false, error: { code: 'policy.settler.not_allowed', message: `settler ${intent.settler} not in allowlist` } };

  const now = Date.now();
  const iat = Date.parse(intent.iat);
  const exp = Date.parse(intent.exp);
  if (!Number.isFinite(iat) || !Number.isFinite(exp)) return { ok: false, error: { code: 'format.bad_timestamp', message: 'iat/exp must be RFC 3339' } };
  if (exp - iat > 600_000) return { ok: false, error: { code: 'replay.window_too_long', message: 'exp-iat must be <= 600s' } };
  if (now > exp) return { ok: false, error: { code: 'replay.expired', message: 'Intent has expired' } };

  let edPubRaw;
  try {
    edPubRaw = pubkeyForAid(intent.buyer, _pubkey_hex);
  } catch (e) {
    return { ok: false, error: { code: 'auth.aid_resolution_failed', message: e.message } };
  }
  const canonical = canonicalJson(intent);
  if (!verifyEd25519(canonical, signature.sig, edPubRaw)) {
    return { ok: false, error: { code: 'signature.invalid', message: 'Ed25519 verification failed' } };
  }

  // Branch by verb.
  if (intent.verb === 'subscription.create') {
    const result = stubSubscriptionAuthorize(intent, merchantSigningKey, merchantAid);
    if (!result.ok) return { ok: false, error: result.error };
    intents.set(intent.intent_id, { intent, signedAt: new Date().toISOString(), signatureHex: signature.sig });
    return {
      ok: true,
      authorization: result.authorization,
      signature: { alg: 'ed25519', kid: merchantAid, sig: result.signatureHex },
    };
  }

  if (intent.verb === 'purchase.return') {
    const result = stubReturnAuthorize(intent, merchantSigningKey, merchantAid);
    if (!result.ok) return { ok: false, error: result.error };
    intents.set(intent.intent_id, { intent, signedAt: new Date().toISOString(), signatureHex: signature.sig });
    return {
      ok: true,
      authorization: result.authorization,
      signature: { alg: 'ed25519', kid: merchantAid, sig: result.signatureHex },
    };
  }

  if (intent.verb === 'inventory.query') {
    const result = stubInventoryQuery(intent, merchantSigningKey, merchantAid);
    if (!result.ok) return { ok: false, error: result.error };
    intents.set(intent.intent_id, { intent, signedAt: new Date().toISOString(), signatureHex: signature.sig });
    return {
      ok: true,
      snapshot: result.snapshot,
      signature: { alg: 'ed25519', kid: merchantAid, sig: result.signatureHex },
    };
  }

  if (intent.verb === 'subscription.cancel') {
    const result = stubSubscriptionCancel(intent, merchantSigningKey, merchantAid);
    if (!result.ok) return { ok: false, error: result.error };
    intents.set(intent.intent_id, { intent, signedAt: new Date().toISOString(), signatureHex: signature.sig });
    return {
      ok: true,
      authorization: result.authorization,
      signature: { alg: 'ed25519', kid: merchantAid, sig: result.signatureHex },
    };
  }

  if (intent.verb === 'quote.request') {
    const result = stubQuoteRequest(intent, merchantSigningKey, merchantAid);
    if (!result.ok) return { ok: false, error: result.error };
    intents.set(intent.intent_id, { intent, signedAt: new Date().toISOString(), signatureHex: signature.sig });
    return {
      ok: true,
      proposal: result.proposal,
      signature: { alg: 'ed25519', kid: merchantAid, sig: result.signatureHex },
    };
  }

  if (intent.verb === 'payout.request') {
    const result = stubPayoutRequest(intent, merchantSigningKey, merchantAid);
    if (!result.ok) return { ok: false, error: result.error };
    intents.set(intent.intent_id, { intent, signedAt: new Date().toISOString(), signatureHex: signature.sig });
    return {
      ok: true,
      authorization: result.authorization,
      signature: { alg: 'ed25519', kid: merchantAid, sig: result.signatureHex },
    };
  }

  const result = stubQuote(intent, merchantSigningKey);
  if (!result.ok) return { ok: false, error: result.error };

  intents.set(intent.intent_id, { intent, signedAt: new Date().toISOString(), signatureHex: signature.sig });
  quotes.set(result.quote.quote_id, { quote: result.quote, intentId: intent.intent_id });
  return {
    ok: true,
    quote: result.quote,
    signature: { alg: 'ed25519', kid: merchantAid, sig: result.signatureHex },
  };
}

export function acceptQuote(quoteId, merchantSigningKey, merchantAid) {
  const record = quotes.get(quoteId);
  if (!record) return { ok: false, error: { code: 'format.unknown_quote', message: `quote ${quoteId} not found` } };
  const { quote, intentId } = record;
  if (Date.now() > Date.parse(quote.exp)) return { ok: false, error: { code: 'replay.expired', message: 'Quote has expired' } };

  const funding = stubFundingInstructions(quote);
  escrows.set(funding.escrow_id, {
    state: 'pending',
    intent_id: intentId,
    quote_id: quoteId,
    amount: quote.total,
    settler: quote.settler,
    seq: 0,
  });
  events.set(funding.escrow_id, []);
  pushEvent(funding.escrow_id, 'none', 'pending', { kind: 'quote-accepted', quote_id: quoteId }, merchantSigningKey, merchantAid);
  return { ok: true, funding };
}

export function fulfillEscrow(escrowId, evidenceId, merchantSigningKey, merchantAid) {
  const e = escrows.get(escrowId);
  if (!e) return { ok: false, error: { code: 'format.unknown_escrow', message: `escrow ${escrowId} not found` } };
  if (e.state !== 'pending' && e.state !== 'funded') return { ok: false, error: { code: 'escrow.wrong_state', message: `cannot fulfill from state ${e.state}` } };

  if (e.state === 'pending') {
    e.state = 'funded';
    pushEvent(escrowId, 'pending', 'funded', { kind: 'rail-confirmed-mock' }, merchantSigningKey, merchantAid);
  }
  e.state = 'fulfilled';
  pushEvent(escrowId, 'funded', 'fulfilled', { kind: 'fulfillment-evidence-accepted', evidence_id: evidenceId ?? newId('icp_ful') }, merchantSigningKey, merchantAid);
  e.state = 'released';
  pushEvent(escrowId, 'fulfilled', 'released', { kind: 'demo-auto-release' }, merchantSigningKey, merchantAid);

  const receipt = {
    type: 'icp.settlement.receipt',
    v: 'icp-1.0',
    settlement_id: newId('icp_set'),
    escrow_id: escrowId,
    intent_id: e.intent_id,
    final_state: 'released',
    amount: e.amount,
    rail: 'demo-mock',
    rail_txid: '0x' + 'cafe'.repeat(16),
    settled_at: new Date().toISOString(),
    released_to: '<merchant-payout-address>',
  };
  const canonical = canonicalJson(receipt);
  const sigHex = signEd25519(canonical, merchantSigningKey);
  receipt.merchant_signature = { alg: 'ed25519', kid: merchantAid, sig: sigHex };
  receipt.settler_signature = receipt.merchant_signature;
  settlements.set(receipt.settlement_id, receipt);
  return { ok: true, receipt };
}

export function getEscrowState(escrowId) {
  const e = escrows.get(escrowId);
  if (!e) return { ok: false, error: { code: 'format.unknown_escrow', message: `escrow ${escrowId} not found` } };
  return { ok: true, state: e.state, intent_id: e.intent_id, quote_id: e.quote_id, amount: e.amount, settler: e.settler, events: events.get(escrowId) ?? [] };
}

export function getSettlement(settlementId) {
  const s = settlements.get(settlementId);
  if (!s) return { ok: false, error: { code: 'format.unknown_settlement', message: `settlement ${settlementId} not found` } };
  return { ok: true, receipt: s };
}

export function counts() {
  return { intents: intents.size, quotes: quotes.size, escrows: escrows.size, settlements: settlements.size };
}

function pushEvent(escrowId, fromState, toState, trigger, key, kid) {
  const e = escrows.get(escrowId);
  const event = {
    type: 'icp.escrow.event',
    v: 'icp-1.0',
    escrow_id: escrowId,
    intent_id: e.intent_id,
    seq: ++e.seq,
    from_state: fromState,
    to_state: toState,
    trigger,
    iat: new Date().toISOString(),
  };
  const canonical = canonicalJson(event);
  event.settler_signature = { alg: 'ed25519', kid, sig: signEd25519(canonical, key) };
  events.get(escrowId).push(event);
}

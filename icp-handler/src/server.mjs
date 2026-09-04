// ICP-1.0 minimal handler — zero-dep HTTP server.
//
// Implements the surface from icp-spec/handler-design.md §"HTTP surface":
//   POST /icp/v1/intents               — submit signed Intent, get Quote
//   POST /icp/v1/quotes/:id/accept     — accept a Quote (returns funding instructions)
//   POST /icp/v1/escrows/:id/fulfill   — submit fulfillment evidence (stub)
//   POST /icp/v1/escrows/:id/dispute   — open a dispute
//   GET  /icp/v1/escrows/:id/events    — Server-Sent Events stream of EscrowEvents
//   GET  /icp/v1/settlements/:id       — fetch a SettlementReceipt
//   GET  /icp/v1/settlers              — declared Settler allowlist
//   GET  /icp/v1/.well-known/icp       — capabilities advertisement
//   GET  /healthz                      — liveness
//
// Run:
//   node src/server.mjs                 # default port 8787
//   PORT=9000 node src/server.mjs

import { createServer } from 'node:http';
import { once } from 'node:events';
import { generateKeyPairSync } from 'node:crypto';

import {
  canonicalJson,
  verifyEd25519,
  signEd25519,
  newId,
  resolveAidPubkey,
  AidBindingError,
  publicKeyToRaw,
} from './codec.mjs';
import * as state from './state.mjs';
import { ReplayGuard } from './replay-guard.mjs';
import {
  stubQuote,
  stubFundingInstructions,
  stubSubscriptionAuthorize,
  stubSubscriptionCancel,
  stubReturnAuthorize,
  stubInventoryQuery,
  stubQuoteRequest,
  stubPayoutRequest,
  stubChannelRegister,
} from './backend-stub.mjs';
import { publishToSubscribers, fetchChannelEvents } from './channel-emitter.mjs';

// ---------------------------------------------------------------------------
// ICPIP-0005 channel store (in-memory). Keyed by channel_id. Production
// handlers would persist this to durable storage with the same shape.
// Exposed module-scope so handlers and tests can introspect.
// ---------------------------------------------------------------------------
const channelStore = new Map();

const PORT = Number(process.env.PORT ?? 8787);

// ---------------------------------------------------------------------------
// Nonce replay guard — ICP-1.0-DRAFT §5.3. Keyed on (signer AID, nonce).
// Sized via env with §5.3-compliant defaults: a 24h TTL (the floor for
// long-running state transitions) and a 100k-entry LRU bound. Production
// deployments back this with a shared/durable store; the reference impl is
// per-process in-memory, which is correct for a single-instance handler.
// ---------------------------------------------------------------------------
const replayGuard = new ReplayGuard({
  ttlMs: Number(process.env.ICP_NONCE_TTL_MS ?? 86_400_000),
  maxEntries: Number(process.env.ICP_NONCE_MAX_ENTRIES ?? 100_000),
});

// ---------------------------------------------------------------------------
// Merchant identity (this handler's signing key, used to sign Quotes,
// EscrowEvents, and SettlementReceipts on behalf of the merchant Backend).
// In production this comes from a KMS or HSM.
// ---------------------------------------------------------------------------

const merchantKp = generateKeyPairSync('ed25519');
const merchantPubRaw = publicKeyToRaw(merchantKp.publicKey);
const merchantAid = `aid:v1:zMerchantHandlerInstance${process.pid}`;

// ---------------------------------------------------------------------------
// Allowed Settlers (governance allowlist subset).
// ---------------------------------------------------------------------------

const ALLOWED_SETTLERS = new Set([
  'settler:stateset.usdc.base-sepolia', // bootstrap
  'settler:circle.usdc.base',            // future production
]);

// Operator-owned trust roots for co-signing externally settled receipts.
// Shape: { "settler:id": "<32-byte Ed25519 public key hex>" }.
// Never accept a public key from the receipt being authorized.
let TRUSTED_SETTLER_KEYS = new Map();
try {
  TRUSTED_SETTLER_KEYS = new Map(Object.entries(JSON.parse(process.env.ICP_SETTLER_KEYS_JSON ?? '{}')));
} catch (error) {
  throw new Error(`ICP_SETTLER_KEYS_JSON must be a JSON object: ${error.message}`);
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

const routes = [
  ['GET',  '/healthz',                                          handleHealthz],
  ['GET',  '/icp/v1/.well-known/icp',                          handleWellKnown],
  ['GET',  '/icp/v1/settlers',                                  handleSettlers],
  ['POST', '/icp/v1/intents',                                   handleSubmitIntent],
  ['POST', /^\/icp\/v1\/quotes\/([^/]+)\/accept$/,             handleAcceptQuote],
  ['POST', /^\/icp\/v1\/escrows\/([^/]+)\/fulfill$/,           handleFulfill],
  ['POST', /^\/icp\/v1\/escrows\/([^/]+)\/dispute$/,           handleDispute],
  ['POST', '/icp/v1/settlements/cosign',                         handleCosignSettlement],
  ['GET',  /^\/icp\/v1\/escrows\/([^/]+)\/events$/,            handleObserve],
  ['GET',  /^\/icp\/v1\/settlements\/([^/]+)$/,                handleGetSettlement],
  ['GET',  /^\/icp\/v1\/channels\/([^/]+)$/,                   handleGetChannel],
  ['GET',  /^\/icp\/v1\/channels\/([^/]+)\/events$/,           handleGetChannelEvents],
];

const server = createServer(async (req, res) => {
  const url = new URL(req.url, `http://${req.headers.host}`);
  for (const [method, pattern, handler] of routes) {
    if (req.method !== method) continue;
    if (typeof pattern === 'string') {
      if (url.pathname === pattern) return handler(req, res, url, []);
    } else {
      const m = pattern.exec(url.pathname);
      if (m) return handler(req, res, url, m.slice(1));
    }
  }
  return reply(res, 404, { type: 'icp.error', code: 'format.unknown_route', message: req.url });
});

server.listen(PORT, () => {
  const addr = server.address();
  console.error(`icp-handler listening on http://127.0.0.1:${addr.port}`);
  console.error(`  merchant_aid: ${merchantAid}`);
  console.error(`  merchant_pubkey_hex: ${merchantPubRaw.toString('hex')}`);
  console.error(`  allowed_settlers: ${[...ALLOWED_SETTLERS].join(', ')}`);
});

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

function handleHealthz(req, res) {
  reply(res, 200, { ok: true, ...state.counts() });
}

function handleWellKnown(req, res) {
  reply(res, 200, {
    spec: 'icp-1.0',
    handler: 'stateset-icp-handler-stub',
    handler_version: '0.1.0',
    merchant_aid: merchantAid,
    merchant_pubkey: {
      alg: 'ed25519',
      raw_hex: merchantPubRaw.toString('hex'),
    },
    capabilities: {
      verbs: [
        'purchase.create',
        'subscription.create',
        'subscription.cancel',
        'purchase.return',
        'inventory.query',
        'quote.request',
        'payout.request',
        'channel.register',
      ],
      transports: ['http'],
      pqc_hybrid: false,
      push_channels: ['webhook', 'sse'],
    },
    settler_allowlist: [...ALLOWED_SETTLERS],
    docs: 'https://github.com/stateset/icp-spec',
  });
}

function handleSettlers(req, res) {
  reply(res, 200, { settlers: [...ALLOWED_SETTLERS] });
}

async function handleSubmitIntent(req, res) {
  const body = await readJson(req);
  if (!body) return reply(res, 400, errBadJson());

  // Body shape: { intent: <Intent>, signature: { alg, kid, sig }, _pubkey_hex?: hex }
  const { intent, signature } = body;
  if (!intent || !signature) {
    return reply(res, 400, err('format.missing_field', 'expected { intent, signature }'));
  }

  // 1. Spec-shape sanity
  if (intent.v !== 'icp-1.0') return reply(res, 400, err('version.unsupported', `unknown spec version ${intent.v}`));
  const supportedVerbs = new Set([
    'purchase.create',
    'subscription.create',
    'subscription.cancel',
    'purchase.return',
    'inventory.query',
    'quote.request',
    'payout.request',
    'channel.register',
  ]);
  if (!supportedVerbs.has(intent.verb)) {
    return reply(res, 400, err('format.unknown_verb', `verb ${intent.verb} not implemented in stub`));
  }

  // 2. Settler allowlist
  if (!ALLOWED_SETTLERS.has(intent.settler)) {
    return reply(res, 400, err('policy.settler.not_allowed', `settler ${intent.settler} not in allowlist`));
  }

  // 3. Replay window
  const now = Date.now();
  const iat = Date.parse(intent.iat);
  const exp = Date.parse(intent.exp);
  if (!Number.isFinite(iat) || !Number.isFinite(exp)) {
    return reply(res, 400, err('format.bad_timestamp', 'iat/exp must be RFC 3339'));
  }
  if (exp - iat > 600_000) return reply(res, 400, err('replay.window_too_long', 'exp-iat must be <= 600s'));
  if (now > exp) return reply(res, 400, err('replay.expired', 'Intent has expired'));

  // 3b. Nonce presence (§5.3 — every payload MUST carry a nonce).
  if (typeof intent.nonce !== 'string' || intent.nonce.length === 0) {
    return reply(res, 400, err('format.missing_field', 'Intent.nonce is required (§5.3)'));
  }

  // 4. AID→pubkey binding + signature verification.
  //
  // `signature.kid` is the claimed signer identity. We re-derive the AID from
  // the supplied key material (§4.2) and reject any mismatch, then verify the
  // Ed25519 signature under the now-bound key. This closes the hole where any
  // key could verify as any AID.
  const signerAid = signature.kid;
  let edPubRaw;
  try {
    edPubRaw = resolveAidPubkey(signerAid, body._pubkey_hex, body._x_pubkey_hex);
  } catch (e) {
    const code = e instanceof AidBindingError ? e.code : 'auth.aid_resolution_failed';
    return reply(res, 401, err(code, e.message));
  }
  const canonical = canonicalJson(intent);
  if (!verifyEd25519(canonical, signature.sig, edPubRaw)) {
    return reply(res, 401, err('signature.invalid', 'Ed25519 verification failed'));
  }

  // Optional reference resolver for the demo: when the caller supplies the
  // principal's already-resolved public key, verify that the principal signed
  // this exact delegation. Production resolves the DID/organization key from
  // trusted identity infrastructure rather than accepting it from the body.
  if (body._principal_pubkey_hex) {
    const binding = intent.principal_binding;
    if (!binding?.signature?.sig) {
      return reply(res, 401, err('delegation.signature_missing', 'principal binding signature is required'));
    }
    if (binding.agent !== intent.buyer || !binding.authority?.verbs?.includes(intent.verb)) {
      return reply(res, 403, err('delegation.scope_mismatch', 'principal binding does not authorize this agent and verb'));
    }
    if (Date.parse(binding.expiry) <= now) {
      return reply(res, 403, err('delegation.expired', 'principal binding has expired'));
    }
    const { signature: _bindingSignature, ...unsignedBinding } = binding;
    const principalKey = Buffer.from(body._principal_pubkey_hex, 'hex');
    if (principalKey.length !== 32
        || !verifyEd25519(canonicalJson(unsignedBinding), binding.signature.sig, principalKey)) {
      return reply(res, 401, err('delegation.signature_invalid', 'principal binding signature failed'));
    }
  }

  // 4b. Nonce replay (§5.3) — only consume a nonce AFTER the signature is
  // proven valid, so an attacker can't burn a victim's nonce with a forged
  // message. Keyed on the bound signer AID so distinct agents may reuse the
  // same nonce bytes without colliding.
  if (!replayGuard.checkAndRecord(signerAid, intent.nonce)) {
    return reply(res, 400, err('replay.nonce_seen', `nonce already used by ${signerAid} within the replay window`));
  }

  // 5. Hand to backend — branch by verb
  if (intent.verb === 'subscription.create') {
    const result = stubSubscriptionAuthorize(intent, merchantKp.privateKey, merchantAid);
    if (!result.ok) return reply(res, 422, result.error);
    state.recordIntent(intent, signature.sig);
    return reply(res, 200, {
      authorization: result.authorization,
      signature: { alg: 'ed25519', kid: merchantAid, sig: result.signatureHex },
    });
  }

  if (intent.verb === 'purchase.return') {
    const result = stubReturnAuthorize(intent, merchantKp.privateKey, merchantAid);
    if (!result.ok) return reply(res, 422, result.error);
    state.recordIntent(intent, signature.sig);
    return reply(res, 200, {
      authorization: result.authorization,
      signature: { alg: 'ed25519', kid: merchantAid, sig: result.signatureHex },
    });
  }

  if (intent.verb === 'inventory.query') {
    const result = stubInventoryQuery(intent, merchantKp.privateKey, merchantAid);
    if (!result.ok) return reply(res, 422, result.error);
    state.recordIntent(intent, signature.sig);
    return reply(res, 200, {
      snapshot: result.snapshot,
      signature: { alg: 'ed25519', kid: merchantAid, sig: result.signatureHex },
    });
  }

  if (intent.verb === 'subscription.cancel') {
    const result = stubSubscriptionCancel(intent, merchantKp.privateKey, merchantAid);
    if (!result.ok) return reply(res, 422, result.error);
    state.recordIntent(intent, signature.sig);

    // ICPIP-0005: fan out subscription.canceled to every subscribed
    // webhook (same fire-and-forget pattern as fulfill + dispute).
    publishToSubscribers(
      channelStore,
      'subscription.canceled',
      {
        subscription_id: result.authorization.subscription_id,
        intent_id: intent.intent_id,
        effective_at:
          result.authorization.effective_at ?? result.authorization.canceled_at ?? null,
        final_charge_at: result.authorization.final_charge_at ?? null,
        refund_amount: result.authorization.refund_amount ?? null,
      },
      { signingKey: merchantKp.privateKey, sourceAid: merchantAid },
    ).catch((err) => {
      console.error(`publishToSubscribers(subscription.canceled) failed: ${err?.message ?? err}`);
    });

    return reply(res, 200, {
      authorization: result.authorization,
      signature: { alg: 'ed25519', kid: merchantAid, sig: result.signatureHex },
    });
  }

  if (intent.verb === 'quote.request') {
    const result = stubQuoteRequest(intent, merchantKp.privateKey, merchantAid);
    if (!result.ok) return reply(res, 422, result.error);
    state.recordIntent(intent, signature.sig);
    return reply(res, 200, {
      proposal: result.proposal,
      signature: { alg: 'ed25519', kid: merchantAid, sig: result.signatureHex },
    });
  }

  if (intent.verb === 'payout.request') {
    const result = stubPayoutRequest(intent, merchantKp.privateKey, merchantAid);
    if (!result.ok) return reply(res, 422, result.error);
    state.recordIntent(intent, signature.sig);
    return reply(res, 200, {
      authorization: result.authorization,
      signature: { alg: 'ed25519', kid: merchantAid, sig: result.signatureHex },
    });
  }

  if (intent.verb === 'channel.register') {
    const result = stubChannelRegister(intent, merchantKp.privateKey, merchantAid, channelStore);
    if (!result.ok) return reply(res, 422, result.error);
    state.recordIntent(intent, signature.sig);
    return reply(res, 200, {
      channel: result.channel,
      signature: { alg: 'ed25519', kid: merchantAid, sig: result.signatureHex },
    });
  }

  const result = stubQuote(intent, merchantKp.privateKey);
  if (!result.ok) return reply(res, 422, result.error);

  state.recordIntent(intent, signature.sig);
  state.recordQuote(result.quote, intent.intent_id, result.signatureHex);

  return reply(res, 200, {
    quote: result.quote,
    signature: { alg: 'ed25519', kid: merchantAid, sig: result.signatureHex },
  });
}

async function handleAcceptQuote(req, res, _url, [quoteId]) {
  const body = await readJson(req);
  if (!body) return reply(res, 400, errBadJson());
  const record = state.getQuote(quoteId);
  if (!record) return reply(res, 404, err('format.unknown_quote', `quote ${quoteId} not found`));
  const { quote, intentId } = record;
  if (Date.now() > Date.parse(quote.exp)) {
    return reply(res, 410, err('replay.expired', 'Quote has expired'));
  }
  // (In prod we'd verify the buyer's Accept signature; the stub trusts the body shape.)

  const funding = stubFundingInstructions(quote);
  const existingEscrow = state.getEscrow(funding.escrow_id);
  if (existingEscrow) {
    return reply(res, 200, {
      funding,
      order: {
        order_id: existingEscrow.order_id,
        status: 'authorized',
        quote_id: quoteId,
      },
      inventory_reservation: state.getInventoryReservation(funding.escrow_id),
    });
  }
  const inventoryReservation = state.reserveInventory(funding.escrow_id, quote.lines);
  if (!inventoryReservation) {
    return reply(res, 409, err('inventory.insufficient', 'quoted inventory is no longer available'));
  }
  const orderId = newId('ord');
  state.createEscrow(funding.escrow_id, {
    state: 'pending',
    intent_id: intentId,
    quote_id: quoteId,
    amount: quote.total,
    settler: quote.settler,
    seq: 0,
    order_id: orderId,
    inventory_reservation_id: inventoryReservation.reservation_id,
  });
  appendSignedEscrowEvent(funding.escrow_id, {
    type: 'icp.escrow.event',
    v: 'icp-1.0',
    escrow_id: funding.escrow_id,
    intent_id: intentId,
    seq: 0,
    from_state: 'none',
    to_state: 'pending',
    trigger: { kind: 'quote-accepted', quote_id: quoteId },
    iat: new Date().toISOString(),
  });

  return reply(res, 200, {
    funding,
    order: { order_id: orderId, status: 'authorized', quote_id: quoteId },
    inventory_reservation: inventoryReservation,
  });
}

async function handleFulfill(req, res, _url, [escrowId]) {
  const body = await readJson(req);
  if (!body) return reply(res, 400, errBadJson());
  const e = state.getEscrow(escrowId);
  if (!e) return reply(res, 404, err('format.unknown_escrow', `escrow ${escrowId} not found`));
  if (e.state !== 'funded' && e.state !== 'pending') {
    return reply(res, 409, err('escrow.wrong_state', `cannot fulfill from state ${e.state}`));
  }

  // Demo: pretend funding is confirmed at time-of-fulfill (so that this all
  // works without a real chain).
  if (e.state === 'pending') {
    state.updateEscrow(escrowId, { state: 'funded' });
    appendSignedEscrowEvent(escrowId, makeEvent(escrowId, e, 'pending', 'funded', { kind: 'rail-confirmed-mock' }));
  }

  state.updateEscrow(escrowId, { state: 'fulfilled' });
  appendSignedEscrowEvent(escrowId, makeEvent(escrowId, e, 'funded', 'fulfilled', {
    kind: 'fulfillment-evidence-accepted',
    evidence_id: body.evidence_id ?? newId('icp_ful'),
  }));

  // Demo: time-lock skipped — auto-release immediately so the demo shows the receipt.
  state.updateEscrow(escrowId, { state: 'released' });
  appendSignedEscrowEvent(escrowId, makeEvent(escrowId, e, 'fulfilled', 'released', { kind: 'demo-auto-release' }));

  // Settlement receipt (co-signed merchant + this handler-as-settler stub).
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
  const sigHex = signEd25519(canonical, merchantKp.privateKey);
  receipt.merchant_signature = { alg: 'ed25519', kid: merchantAid, sig: sigHex };
  receipt.settler_signature = receipt.merchant_signature; // stub: same key for both
  state.recordSettlement(receipt);

  // ICPIP-0005: fan out settlement.released to every subscribed webhook.
  // Fire-and-forget — the synchronous response shouldn't wait for HTTP
  // round-trips to external receivers. In production, retries land via a
  // durable job queue; the reference impl is single-attempt for now.
  publishToSubscribers(
    channelStore,
    'settlement.released',
    {
      settlement_id: receipt.settlement_id,
      escrow_id: escrowId,
      intent_id: e.intent_id,
      amount: e.amount,
      final_state: 'released',
      settled_at: receipt.settled_at,
    },
    { signingKey: merchantKp.privateKey, sourceAid: merchantAid },
  ).catch((err) => {
    console.error(`publishToSubscribers failed: ${err?.message ?? err}`);
  });

  return reply(res, 200, { receipt });
}

async function handleCosignSettlement(req, res) {
  const body = await readJson(req);
  const receipt = body?.receipt;
  if (!receipt || receipt.type !== 'icp.settlement.receipt') {
    return reply(res, 400, err('format.missing_field', 'receipt is required'));
  }
  if (!ALLOWED_SETTLERS.has(receipt.settler)) {
    return reply(res, 400, err('policy.settler.not_allowed', `settler ${receipt.settler} not in allowlist`));
  }
  const trustedKeyHex = TRUSTED_SETTLER_KEYS.get(receipt.settler);
  if (!trustedKeyHex) {
    return reply(res, 503, err('settler.key_unavailable', `no operator-owned key for ${receipt.settler}`));
  }
  if (!receipt.settler_signature?.sig) {
    return reply(res, 400, err('format.missing_field', 'settler_signature is required'));
  }
  const { merchant_signature: _merchant, settler_signature: _settler, ...unsigned } = receipt;
  let settlerKey;
  try {
    settlerKey = Buffer.from(trustedKeyHex, 'hex');
    if (settlerKey.length !== 32) throw new Error('key must contain 32 bytes');
  } catch (error) {
    return reply(res, 503, err('settler.key_invalid', error.message));
  }
  const canonical = canonicalJson(unsigned);
  if (!verifyEd25519(canonical, receipt.settler_signature.sig, settlerKey)) {
    return reply(res, 401, err('settlement.settler_signature_invalid', 'settler receipt signature failed'));
  }
  const intentRecord = state.getIntent(receipt.intent_id);
  const quoteRecord = state.getQuoteByIntent(receipt.intent_id);
  if (!intentRecord || !quoteRecord) {
    return reply(res, 404, err('format.unknown_intent', `intent ${receipt.intent_id} is not known`));
  }
  if (receipt.amount?.amount !== quoteRecord.quote.total?.amount
      || receipt.amount?.currency !== quoteRecord.quote.total?.currency) {
    return reply(res, 409, err('settlement.amount_mismatch', 'receipt amount does not match the signed quote'));
  }
  if (receipt.final_state !== 'released' && receipt.final_state !== 'refunded') {
    return reply(res, 409, err('settlement.not_final', `cannot co-sign ${receipt.final_state}`));
  }
  const coSigned = {
    ...unsigned,
    settler_signature: receipt.settler_signature,
    merchant_signature: {
      alg: 'ed25519',
      kid: merchantAid,
      sig: signEd25519(canonical, merchantKp.privateKey),
    },
  };
  state.recordSettlement(coSigned);
  return reply(res, 200, { receipt: coSigned });
}

async function handleDispute(req, res, _url, [escrowId]) {
  const body = await readJson(req);
  if (!body) return reply(res, 400, errBadJson());
  const e = state.getEscrow(escrowId);
  if (!e) return reply(res, 404, err('format.unknown_escrow', `escrow ${escrowId} not found`));
  if (e.state !== 'funded' && e.state !== 'fulfilled') {
    return reply(res, 409, err('escrow.wrong_state', `cannot dispute from state ${e.state}`));
  }
  const priorState = e.state;
  state.updateEscrow(escrowId, { state: 'disputed' });
  const disputeId = newId('icp_disp');
  const openedAt = new Date().toISOString();
  const reason = body.reason ?? 'unspecified';
  appendSignedEscrowEvent(escrowId, makeEvent(escrowId, e, priorState, 'disputed', {
    kind: 'dispute-opened',
    dispute_id: disputeId,
    reason,
  }));

  // ICPIP-0005: fan out dispute.opened to every subscribed webhook.
  // Same fire-and-forget pattern as fulfill — receivers dedupe by
  // envelope event_id; sequence is monotonic per channel.
  publishToSubscribers(
    channelStore,
    'dispute.opened',
    {
      dispute_id: disputeId,
      escrow_id: escrowId,
      intent_id: e.intent_id,
      reason,
      amount: e.amount,
      opened_at: openedAt,
      prior_state: priorState,
    },
    { signingKey: merchantKp.privateKey, sourceAid: merchantAid },
  ).catch((err) => {
    console.error(`publishToSubscribers(dispute.opened) failed: ${err?.message ?? err}`);
  });

  return reply(res, 200, { state: 'disputed', dispute_id: disputeId });
}

function handleObserve(req, res, _url, [escrowId]) {
  const e = state.getEscrow(escrowId);
  if (!e) return reply(res, 404, err('format.unknown_escrow', `escrow ${escrowId} not found`));

  res.writeHead(200, {
    'Content-Type': 'text/event-stream',
    'Cache-Control': 'no-cache',
    Connection: 'keep-alive',
  });
  // Replay existing events.
  for (const ev of state.getEscrowEvents(escrowId)) {
    res.write(`event: escrow.event\ndata: ${JSON.stringify(ev)}\n\n`);
  }
  // Subscribe to future events.
  const sub = { write: (data) => res.write(data) };
  state.addObserver(escrowId, sub);
  req.on('close', () => state.removeObserver(escrowId, sub));
}

function handleGetSettlement(req, res, _url, [settlementId]) {
  const s = state.getSettlement(settlementId);
  if (!s) return reply(res, 404, err('format.unknown_settlement', `settlement ${settlementId} not found`));
  reply(res, 200, s);
}

function handleGetChannel(req, res, _url, [channelId]) {
  const ch = channelStore.get(channelId);
  if (!ch) return reply(res, 404, err('channel.not_found', `channel ${channelId} not registered`));
  if (Date.parse(ch.expires_at) < Date.now()) {
    return reply(res, 410, err('channel.expired', `channel ${channelId} expired at ${ch.expires_at}`));
  }
  reply(res, 200, ch);
}

// ICPIP-0005 §5 — Recovery API. Agents that observed a sequence gap in
// the live stream call this to backfill missed events. `since` is the
// last sequence number the agent successfully observed; the handler
// returns every retained event with `sequence > since`.
function handleGetChannelEvents(req, res, url, [channelId]) {
  const ch = channelStore.get(channelId);
  if (!ch) return reply(res, 404, err('channel.not_found', `channel ${channelId} not registered`));
  if (Date.parse(ch.expires_at) < Date.now()) {
    return reply(res, 410, err('channel.expired', `channel ${channelId} expired at ${ch.expires_at}`));
  }
  const sinceParam = url.searchParams.get('since');
  const since = sinceParam == null ? 0 : Number(sinceParam);
  if (!Number.isFinite(since) || since < 0) {
    return reply(res, 400, err('format.bad_query_param', `since must be a non-negative integer, got ${sinceParam}`));
  }
  const events = fetchChannelEvents(channelId, since);
  if (events === null) {
    return reply(res, 409, err(
      'channel.sequence_gap',
      `since=${since} is before retained window; channel must be re-registered`,
    ));
  }
  reply(res, 200, { channel_id: channelId, since, events });
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function readJson(req) {
  const chunks = [];
  for await (const c of req) chunks.push(c);
  try {
    return JSON.parse(Buffer.concat(chunks).toString('utf8'));
  } catch (_) {
    return null;
  }
}

function reply(res, code, obj) {
  res.writeHead(code, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify(obj));
}

function err(code, message) {
  return { type: 'icp.error', code, message };
}
function errBadJson() {
  return err('format.bad_json', 'request body is not valid JSON');
}

function makeEvent(escrowId, e, fromState, toState, trigger) {
  return {
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
}

function appendSignedEscrowEvent(escrowId, event) {
  const canonical = canonicalJson(event);
  const sigHex = signEd25519(canonical, merchantKp.privateKey);
  event.settler_signature = { alg: 'ed25519', kid: merchantAid, sig: sigHex };
  state.appendEscrowEvent(escrowId, event);
}

// Allow the test to stop the server cleanly.
export async function stop() {
  server.close();
  await once(server, 'close');
}
export { server, merchantKp, merchantAid, merchantPubRaw, replayGuard };

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

import { canonicalJson, verifyEd25519, signEd25519, newId, pubkeyForAid, publicKeyToRaw } from './codec.mjs';
import * as state from './state.mjs';
import {
  stubQuote,
  stubFundingInstructions,
  stubSubscriptionAuthorize,
  stubReturnAuthorize,
  stubInventoryQuery,
} from './backend-stub.mjs';

const PORT = Number(process.env.PORT ?? 8787);

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
  ['GET',  /^\/icp\/v1\/escrows\/([^/]+)\/events$/,            handleObserve],
  ['GET',  /^\/icp\/v1\/settlements\/([^/]+)$/,                handleGetSettlement],
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
      verbs: ['purchase.create', 'subscription.create', 'purchase.return', 'inventory.query'],
      transports: ['http'],
      pqc_hybrid: false,
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
    'purchase.return',
    'inventory.query',
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

  // 4. Signature verification
  let edPubRaw;
  try {
    edPubRaw = pubkeyForAid(intent.buyer, body._pubkey_hex);
  } catch (e) {
    return reply(res, 400, err('auth.aid_resolution_failed', e.message));
  }
  const canonical = canonicalJson(intent);
  if (!verifyEd25519(canonical, signature.sig, edPubRaw)) {
    return reply(res, 401, err('signature.invalid', 'Ed25519 verification failed'));
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
  state.createEscrow(funding.escrow_id, {
    state: 'pending',
    intent_id: intentId,
    quote_id: quoteId,
    amount: quote.total,
    settler: quote.settler,
    seq: 0,
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

  return reply(res, 200, { funding });
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

  return reply(res, 200, { receipt });
}

async function handleDispute(req, res, _url, [escrowId]) {
  const body = await readJson(req);
  if (!body) return reply(res, 400, errBadJson());
  const e = state.getEscrow(escrowId);
  if (!e) return reply(res, 404, err('format.unknown_escrow', `escrow ${escrowId} not found`));
  if (e.state !== 'funded' && e.state !== 'fulfilled') {
    return reply(res, 409, err('escrow.wrong_state', `cannot dispute from state ${e.state}`));
  }
  state.updateEscrow(escrowId, { state: 'disputed' });
  appendSignedEscrowEvent(escrowId, makeEvent(escrowId, e, e.state, 'disputed', {
    kind: 'dispute-opened',
    reason: body.reason ?? 'unspecified',
  }));
  return reply(res, 200, { state: 'disputed' });
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
export { server, merchantKp, merchantAid, merchantPubRaw };

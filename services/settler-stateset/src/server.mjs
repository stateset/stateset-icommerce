// settler-stateset — reference Settler daemon for ICP-1.0
//
// Implements the Settler side of the protocol per icp-spec/SETTLERS.md:
//
//   - Discovery doc at /.well-known/icp-settler
//   - Signed EscrowEvent emission on lifecycle transitions
//   - SettlementReceipt issuance (co-signed) at terminal states
//   - Proof-of-reserves attestation endpoint
//   - WebSocket-equivalent SSE stream of EscrowEvents
//
// Two modes (selected by env):
//   - default ("mock"): events injected via POST /admin/escrow/event
//   - "chain": [not implemented in this version — runtime hooks ready]
//
// Run:
//   PORT=8788 node src/server.mjs                       # mock mode (default)
//   PORT=8788 SETTLER_CHAIN_RPC=https://... node src/server.mjs  # chain mode (future)
//
// Zero dependencies. Stock Node 20+.

import { createServer } from 'node:http';
import { once } from 'node:events';
import { createPrivateKey, createPublicKey, generateKeyPairSync, randomBytes } from 'node:crypto';

import { canonicalJson, signEd25519, publicKeyToRaw, newId } from '../../../icp-handler/src/codec.mjs';
import * as state from './state.mjs';

const PORT = Number(process.env.PORT ?? 8788);
const SETTLER_ID = process.env.SETTLER_ID ?? 'settler:stateset.usdc.base-sepolia';
const MODE = process.env.SETTLER_CHAIN_RPC ? 'chain' : 'mock';

// ---------------------------------------------------------------------------
// Settler identity. Production reads from a KMS-backed key. The demo
// generates a fresh Ed25519 key per process and exposes the public part
// in the discovery doc.
// ---------------------------------------------------------------------------

const settlerKp = generateKeyPairSync('ed25519');
const settlerPubRaw = publicKeyToRaw(settlerKp.publicKey);
const settlerKid = `settler-stateset-${process.pid}`;

// ---------------------------------------------------------------------------
// Discovery document (per SETTLERS.md §S.1)
// ---------------------------------------------------------------------------

function discoveryDoc(addr) {
  const portInUse = addr?.port ?? PORT;
  const base = `http://127.0.0.1:${portInUse}`;
  return {
    settler_id: SETTLER_ID,
    operator: {
      name: 'StateSet, Inc. (bootstrap testnet operator)',
      lei: '_PENDING_LEI_REGISTRATION_',
      jurisdiction: 'US',
    },
    signing_keys: [
      { alg: 'ed25519', kid: settlerKid, pub_hex: settlerPubRaw.toString('hex') },
    ],
    endpoints: {
      fund: `${base}/admin/escrow/event`, // mock-mode only
      observe: `${base}/icp/v1/escrows/{escrow_id}/events`,
      release: `${base}/admin/escrow/event`, // mock-mode
      refund: `${base}/admin/escrow/event`,
      dispute: `${base}/admin/escrow/event`,
      receipts: `${base}/icp/v1/settlements/{settlement_id}`,
      proof_of_reserves: `${base}/icp/v1/settlers/${encodeURIComponent(SETTLER_ID)}/proof-of-reserves`,
    },
    limits: {
      min_intent: { amount: '0.01', currency: 'USDC' },
      max_intent: { amount: '100000.00', currency: 'USDC' }, // bootstrap testnet cap
      max_pending_per_aid: { amount: '500000.00', currency: 'USDC' },
    },
    finality: {
      rail: 'base-sepolia',
      blocks_to_finality: 18,
      expected_seconds_to_finality: 30,
    },
    proof_of_reserves: {
      method: 'merkle-attestation',
      endpoint: `${base}/icp/v1/settlers/${encodeURIComponent(SETTLER_ID)}/proof-of-reserves`,
    },
    operating_mode: MODE,
    version: 'icp-1.0',
  };
}

// ---------------------------------------------------------------------------
// HTTP routes
// ---------------------------------------------------------------------------

const routes = [
  ['GET',  '/healthz',                                       handleHealthz],
  ['GET',  '/.well-known/icp-settler',                       handleDiscovery],
  ['POST', '/admin/escrow/event',                            handleAdminEvent],
  ['GET',  /^\/icp\/v1\/escrows\/([^/]+)\/events$/,         handleObserve],
  ['GET',  /^\/icp\/v1\/escrows\/([^/]+)$/,                 handleGetEscrow],
  ['GET',  /^\/icp\/v1\/settlements\/([^/]+)$/,             handleGetSettlement],
  ['GET',  /^\/icp\/v1\/settlers\/([^/]+)\/proof-of-reserves$/, handlePor],
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
  process.stderr.write(`settler-stateset listening on http://127.0.0.1:${addr.port}\n`);
  process.stderr.write(`  settler_id:   ${SETTLER_ID}\n`);
  process.stderr.write(`  settler_kid:  ${settlerKid}\n`);
  process.stderr.write(`  settler_pub:  ${settlerPubRaw.toString('hex')}\n`);
  process.stderr.write(`  mode:         ${MODE}\n`);
});

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

function handleHealthz(req, res) {
  reply(res, 200, { ok: true, ...state.snapshot() });
}

function handleDiscovery(req, res) {
  reply(res, 200, discoveryDoc(server.address()));
}

async function handleAdminEvent(req, res) {
  if (MODE !== 'mock') {
    return reply(res, 403, err('settler.unavailable', 'admin event injection disabled in chain mode'));
  }
  const body = await readJson(req);
  if (!body) return reply(res, 400, err('format.bad_json', 'invalid JSON'));
  const { escrow_id, intent_id, kind, init, evidence_id, reason, rail_event, payout_amount, payout_currency } = body;
  if (!escrow_id || !kind) {
    return reply(res, 400, err('format.missing_field', 'expected { escrow_id, kind, ... }'));
  }

  // State-machine dispatch. The settler is the authority on every transition.
  let event;
  switch (kind) {
    case 'fund': {
      // First-time funding event. Create the escrow and emit pending → funded.
      if (state.knownEscrow(escrow_id)) {
        return reply(res, 409, err('escrow.already_funded', `escrow ${escrow_id} exists`));
      }
      if (!init?.amount) {
        return reply(res, 400, err('format.missing_field', 'fund event requires init.amount'));
      }
      // intent_id is OPTIONAL in chain-mode (the chain doesn't carry it; the
      // merchant Backend resolves intent_id via quote_hash post-hoc). If not
      // provided, we record null and the merchant patches it later via a
      // separate API. Mock-mode callers SHOULD still provide it.
      const resolvedIntentId = init.intent_id ?? intent_id ?? null;
      state.createOrGetEscrow(escrow_id, {
        state: 'funded',
        intent_id: resolvedIntentId,
        amount: init.amount,
        settler: SETTLER_ID,
      });
      event = makeEvent(escrow_id, resolvedIntentId, 'none', 'funded', {
        kind: 'rail-funded',
        rail_event: rail_event ?? null,
      });
      break;
    }
    case 'fulfill': {
      const e = state.getEscrow(escrow_id);
      if (!e) return reply(res, 404, err('format.unknown_escrow', escrow_id));
      if (e.state !== 'funded') {
        return reply(res, 409, err('escrow.wrong_state', `cannot fulfill from ${e.state}`));
      }
      state.updateEscrow(escrow_id, { state: 'fulfilled' });
      event = makeEvent(escrow_id, e.intent_id, 'funded', 'fulfilled', {
        kind: 'fulfillment-evidence-accepted',
        evidence_id: evidence_id ?? newId('icp_ful'),
      });
      break;
    }
    case 'release': {
      const e = state.getEscrow(escrow_id);
      if (!e) return reply(res, 404, err('format.unknown_escrow', escrow_id));
      if (e.state !== 'fulfilled' && e.state !== 'disputed') {
        return reply(res, 409, err('escrow.wrong_state', `cannot release from ${e.state}`));
      }
      state.updateEscrow(escrow_id, { state: 'released' });
      event = makeEvent(escrow_id, e.intent_id, e.state === 'disputed' ? 'disputed' : 'fulfilled', 'released', {
        kind: 'rail-released',
        rail_event: rail_event ?? null,
      });
      // Terminal state → emit + sign SettlementReceipt.
      const receipt = makeReceipt(escrow_id, e, 'released', {
        amount: { amount: payout_amount ?? e.amount.amount, currency: payout_currency ?? e.amount.currency },
        rail_txid: rail_event?.tx_hash ?? '0x' + 'cafe'.repeat(16),
      });
      state.recordSettlement(receipt);
      break;
    }
    case 'refund': {
      const e = state.getEscrow(escrow_id);
      if (!e) return reply(res, 404, err('format.unknown_escrow', escrow_id));
      if (e.state !== 'funded' && e.state !== 'disputed') {
        return reply(res, 409, err('escrow.wrong_state', `cannot refund from ${e.state}`));
      }
      state.updateEscrow(escrow_id, { state: 'refunded' });
      event = makeEvent(escrow_id, e.intent_id, e.state, 'refunded', {
        kind: 'rail-refunded',
        reason: reason ?? 'merchant-cancel',
        rail_event: rail_event ?? null,
      });
      const receipt = makeReceipt(escrow_id, e, 'refunded', {
        amount: e.amount,
        rail_txid: rail_event?.tx_hash ?? '0xbeefbeef'.repeat(8),
      });
      state.recordSettlement(receipt);
      break;
    }
    case 'dispute': {
      const e = state.getEscrow(escrow_id);
      if (!e) return reply(res, 404, err('format.unknown_escrow', escrow_id));
      if (e.state !== 'funded' && e.state !== 'fulfilled') {
        return reply(res, 409, err('escrow.wrong_state', `cannot dispute from ${e.state}`));
      }
      state.updateEscrow(escrow_id, { state: 'disputed' });
      event = makeEvent(escrow_id, e.intent_id, e.state, 'disputed', {
        kind: 'dispute-opened',
        reason: reason ?? 'unspecified',
      });
      break;
    }
    default:
      return reply(res, 400, err('format.unknown_verb', `unknown kind ${kind}`));
  }

  state.appendEvent(escrow_id, event);
  return reply(res, 200, { event });
}

function handleObserve(req, res, _url, [escrowId]) {
  if (!state.knownEscrow(escrowId)) {
    return reply(res, 404, err('format.unknown_escrow', escrowId));
  }
  res.writeHead(200, {
    'Content-Type': 'text/event-stream',
    'Cache-Control': 'no-cache',
    Connection: 'keep-alive',
  });
  for (const ev of state.getEvents(escrowId)) {
    res.write(`event: escrow.event\ndata: ${JSON.stringify(ev)}\n\n`);
  }
  const sub = { write: (data) => res.write(data) };
  state.addObserver(escrowId, sub);
  req.on('close', () => state.removeObserver(escrowId, sub));
}

function handleGetEscrow(req, res, _url, [escrowId]) {
  const e = state.getEscrow(escrowId);
  if (!e) return reply(res, 404, err('format.unknown_escrow', escrowId));
  reply(res, 200, { escrow_id: escrowId, ...e, events: state.getEvents(escrowId) });
}

function handleGetSettlement(req, res, _url, [settlementId]) {
  const s = state.getSettlement(settlementId);
  if (!s) return reply(res, 404, err('format.unknown_settlement', settlementId));
  reply(res, 200, s);
}

function handlePor(req, res) {
  // Mock proof-of-reserves attestation. Production POR is signed and derived
  // from on-chain contract balance vs. open escrow Merkle tree.
  const snap = state.snapshot();
  const por = {
    settler_id: SETTLER_ID,
    as_of_unix: Math.floor(Date.now() / 1000),
    open_escrow_count: snap.open_escrows,
    open_escrow_total: snap.open_escrow_total_units,
    currency: 'USDC',
    contract_balance_attested: snap.open_escrow_total_units, // mock: balance = held
    delta_buffer: '0.00',
    method: 'merkle-attestation',
    merkle_root: '0x' + 'aa'.repeat(32), // mock fixed root
  };
  const canonical = canonicalJson(por);
  por.signature = { alg: 'ed25519', kid: settlerKid, sig: signEd25519(canonical, settlerKp.privateKey) };
  reply(res, 200, por);
}

// ---------------------------------------------------------------------------
// EscrowEvent and SettlementReceipt construction (per SETTLERS.md §S.2, §S.3)
// ---------------------------------------------------------------------------

function makeEvent(escrowId, intentId, fromState, toState, trigger) {
  const e = state.getEscrow(escrowId);
  if (!e) throw new Error(`internal: escrow ${escrowId} missing`);
  const event = {
    type: 'icp.escrow.event',
    v: 'icp-1.0',
    escrow_id: escrowId,
    intent_id: intentId,
    seq: ++e.seq,
    from_state: fromState,
    to_state: toState,
    trigger,
    iat: new Date().toISOString(),
  };
  const canonical = canonicalJson(event);
  event.settler_signature = {
    alg: 'ed25519',
    kid: settlerKid,
    sig: signEd25519(canonical, settlerKp.privateKey),
  };
  return event;
}

function makeReceipt(escrowId, escrowRecord, finalState, opts) {
  const receipt = {
    type: 'icp.settlement.receipt',
    v: 'icp-1.0',
    settlement_id: newId('icp_set'),
    escrow_id: escrowId,
    intent_id: escrowRecord.intent_id,
    final_state: finalState,
    amount: opts.amount,
    rail: 'base-sepolia',
    rail_txid: opts.rail_txid,
    rail_block_number: opts.rail_block_number ?? null,
    rail_finalized_at: new Date().toISOString(),
    released_to: opts.released_to ?? '<rail-recipient-address>',
    settled_at: new Date().toISOString(),
  };
  const canonical = canonicalJson(receipt);
  const settlerSigHex = signEd25519(canonical, settlerKp.privateKey);
  receipt.settler_signature = { alg: 'ed25519', kid: settlerKid, sig: settlerSigHex };
  // In production the merchant co-signs out-of-band. For the demo we attach a
  // placeholder so the receipt shape is correct; tests verify that production
  // handlers would reject single-sig receipts as INVALID.
  receipt.merchant_signature = null;
  return receipt;
}

// ---------------------------------------------------------------------------
// Util
// ---------------------------------------------------------------------------

async function readJson(req) {
  const chunks = [];
  for await (const c of req) chunks.push(c);
  try { return JSON.parse(Buffer.concat(chunks).toString('utf8')); } catch (_) { return null; }
}
function reply(res, code, obj) {
  res.writeHead(code, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify(obj));
}
function err(code, message) { return { type: 'icp.error', code, message }; }

export async function stop() {
  server.close();
  await once(server, 'close');
}

export { server, settlerKp, settlerKid, settlerPubRaw, SETTLER_ID, MODE };

// The Intent signer must BE the party the Intent acts as — in every trust
// mode.
//
// `purchase.create` was held to this from the start (`auth.buyer_mismatch`),
// but `payout.request` — the one verb that moves money OUT — was not, and it
// is inverted-direction: the acting party is `seller`, not `buyer`. So a
// caller could sign a payout naming somebody else's AID as the seller and
// draw against that seller's held balance. Trust mode was no defence:
// `demo` skips the delegation check entirely, and that is exactly where the
// hole was reachable. Demo trust relaxes who DELEGATED an Agent; it never
// licensed acting as one.
//
// This suite runs the handler PERMISSIVE (no ICP_TRUST_MODE, in-memory
// state), which is the posture the hole lived in. The enforcing counterpart
// is in client-binding.test.mjs.
//
// Run: PORT=0 node --test test/acting-party.test.mjs

import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { generateKeyPairSync, randomBytes } from 'node:crypto';

import {
  canonicalJson,
  deriveAidFromPubkeys,
  newId,
  newNonceHex,
  publicKeyToRaw,
  signEd25519,
} from '../src/codec.mjs';

function agent() {
  const ed = generateKeyPairSync('ed25519');
  const x = generateKeyPairSync('x25519');
  const edPub = publicKeyToRaw(ed.publicKey);
  const xPub = publicKeyToRaw(x.publicKey);
  return { key: ed.privateKey, edPub, xPub, aid: deriveAidFromPubkeys(edPub, xPub) };
}

// The seller whose balance is at stake, and a stranger who wants it.
const seller = agent();
const stranger = agent();

process.env.PORT ??= '0';
process.env.ICP_MERCHANT_AID = 'aid:v1:zActingPartyTestPlatform';
delete process.env.ICP_TRUST_MODE;
delete process.env.ICP_PRINCIPAL_KEYS_JSON;
delete process.env.ICP_AGENT_KEYS_JSON;
const { server } = await import('../src/server.mjs');

let baseUrl;
before(async () => {
  if (!server.listening) await new Promise((resolve) => server.once('listening', resolve));
  baseUrl = `http://127.0.0.1:${server.address().port}`;
});
after(() => server.close());

test('the handler under test is permissive — the posture the hole lived in', async () => {
  const caps = await (await fetch(`${baseUrl}/icp/v1/.well-known/icp`)).json();
  assert.notEqual(caps.trust_mode, 'enforce');
});

function payoutIntent(sellerAid) {
  const now = new Date();
  return {
    v: 'icp-1.0',
    verb: 'payout.request',
    intent_id: newId('icp_int'),
    seller: sellerAid,
    platform: 'aid:v1:zActingPartyTestPlatform',
    settler: 'settler:stateset.usdc.base-sepolia',
    amount: { amount: '100.00', currency: 'USDC' },
    destination: { type: 'wallet', wallet_address: '0x2222222222222222222222222222222222222222' },
    // Deliberately no principal_binding: a permissive handler asks for none,
    // so nothing but this check stands between the stranger and the funds.
    expiry: new Date(now.getTime() + 300_000).toISOString(),
    nonce: newNonceHex(),
    iat: now.toISOString(),
    exp: new Date(now.getTime() + 300_000).toISOString(),
  };
}

// `kid` defaults to the signer's own AID; pass `kid: undefined` to omit it
// (or `x: null` to withhold the X25519 half, which is what lets a caller with
// no AID slip past §4.2 re-derivation).
async function submit(intent, signer, options = {}) {
  // `Object.hasOwn`, not a destructuring default: `{ kid = signer.aid }` would
  // quietly restore the AID for the very cases that omit it on purpose.
  const kid = Object.hasOwn(options, 'kid') ? options.kid : signer.aid;
  const x = Object.hasOwn(options, 'x') ? options.x : signer.xPub;
  const signature = { alg: 'ed25519', sig: signEd25519(canonicalJson(intent), signer.key) };
  if (kid !== undefined) signature.kid = kid;
  const response = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      intent,
      signature,
      _pubkey_hex: signer.edPub.toString('hex'),
      ...(x ? { _x_pubkey_hex: x.toString('hex') } : {}),
    }),
  });
  return { status: response.status, json: await response.json() };
}

test('a stranger cannot request a payout in the seller\'s name', async () => {
  // The stranger's own keys and AID — §4.2 binding and the Ed25519 signature
  // both pass. Only the acting-party check stands in the way.
  const { status, json } = await submit(payoutIntent(seller.aid), stranger);
  assert.equal(status, 401, JSON.stringify(json));
  assert.equal(json.code, 'auth.acting_party_mismatch');
});

test('a payout signed by its own seller still succeeds', async () => {
  const { status, json } = await submit(payoutIntent(seller.aid), seller);
  assert.equal(status, 200, JSON.stringify(json));
  assert.equal(json.authorization.type, 'payout.authorization');
  assert.equal(json.authorization.seller, seller.aid);
});

test('a payout that names no seller at all is refused, not defaulted', async () => {
  const intent = payoutIntent(seller.aid);
  delete intent.seller;
  const { status, json } = await submit(intent, stranger);
  assert.equal(status, 400, JSON.stringify(json));
  assert.equal(json.code, 'format.missing_field');
  assert.match(json.message, /seller/);
});

// Two ABSENT fields are not a match. The acting-party comparison alone was
// satisfied by `undefined === undefined`, and nothing downstream caught it:
// with no `_x_pubkey_hex` there is no AID to re-derive, a permissive handler
// requires no registration and no delegation, and the payout stub happily
// opened a fresh balance keyed on `undefined`. The probe below returned a
// merchant-signed authorization for thousands of dollars to a caller who
// named no identity whatsoever.
test('an Intent naming NO signer and NO acting party is refused', async () => {
  const intent = payoutIntent(seller.aid);
  delete intent.seller;
  const { status, json } = await submit(intent, stranger, { kid: undefined, x: null });
  assert.notEqual(status, 200);
  assert.equal(status, 400, JSON.stringify(json));
  assert.equal(json.code, 'format.missing_field');
  assert.match(json.message, /signature\.kid/);
  assert.equal(json.authorization, undefined);
});

test('an Intent naming an acting party but no signer is refused', async () => {
  const { status, json } = await submit(payoutIntent(seller.aid), stranger, {
    kid: undefined,
    x: null,
  });
  assert.equal(status, 400, JSON.stringify(json));
  assert.equal(json.code, 'format.missing_field');
  assert.match(json.message, /signature\.kid/);
});

test('an empty-string signer or acting party is not a signer either', async () => {
  const blankKid = await submit(payoutIntent(seller.aid), stranger, { kid: '', x: null });
  assert.equal(blankKid.status, 400);
  assert.equal(blankKid.json.code, 'format.missing_field');
  const blankSeller = payoutIntent(seller.aid);
  blankSeller.seller = '';
  const { status, json } = await submit(blankSeller, stranger);
  assert.equal(status, 400, JSON.stringify(json));
  assert.equal(json.code, 'format.missing_field');
  assert.match(json.message, /seller/);
});

test('the buyer-side check keeps its own code, so consumers do not move', async () => {
  const now = new Date();
  const intent = {
    v: 'icp-1.0',
    verb: 'purchase.create',
    intent_id: newId('icp_int'),
    buyer: seller.aid, // somebody else's AID
    merchant: 'aid:v1:zActingPartyTestPlatform',
    settler: 'settler:stateset.usdc.base-sepolia',
    items: [{ sku: 'WIDGET-001', quantity: 1, unit_price: { amount: '10.00', currency: 'USDC' } }],
    max_total: { amount: '20.00', currency: 'USDC' },
    expiry: new Date(now.getTime() + 300_000).toISOString(),
    nonce: newNonceHex(),
    iat: now.toISOString(),
    exp: new Date(now.getTime() + 300_000).toISOString(),
  };
  const { status, json } = await submit(intent, stranger);
  assert.equal(status, 401);
  assert.equal(json.code, 'auth.buyer_mismatch');
});

test('a non-payout verb is bound to its buyer too', async () => {
  const now = new Date();
  const intent = {
    v: 'icp-1.0',
    verb: 'inventory.query',
    intent_id: newId('icp_int'),
    buyer: seller.aid,
    merchant: 'aid:v1:zActingPartyTestPlatform',
    settler: 'settler:stateset.usdc.base-sepolia',
    expiry: new Date(now.getTime() + 300_000).toISOString(),
    nonce: randomBytes(16).toString('hex'),
    iat: now.toISOString(),
    exp: new Date(now.getTime() + 300_000).toISOString(),
  };
  const { status, json } = await submit(intent, stranger);
  assert.equal(status, 401);
  assert.equal(json.code, 'auth.acting_party_mismatch');
});

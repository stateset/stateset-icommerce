// Unit tests for AID→pubkey binding (ICP-1.0-DRAFT §4.2) in codec.mjs, plus
// HTTP-level integration: a forged pubkey must NOT verify as an arbitrary AID,
// and a replayed nonce must be rejected with `replay.nonce_seen`.
//
// Run: PORT=0 node --test test/aid-binding.test.mjs

import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { generateKeyPairSync, createHash } from 'node:crypto';

import {
  canonicalJson,
  signEd25519,
  publicKeyToRaw,
  newId,
  newNonceHex,
  base58btcEncode,
  deriveAidFromPubkeys,
  resolveAidPubkey,
  AidBindingError,
} from '../src/codec.mjs';
import { server } from '../src/server.mjs';

// ---------------------------------------------------------------------------
// Test identities
// ---------------------------------------------------------------------------
const buyerKp = generateKeyPairSync('ed25519');
const buyerXkp = generateKeyPairSync('x25519');
const buyerEdPubRaw = publicKeyToRaw(buyerKp.publicKey);
const buyerXPubRaw = publicKeyToRaw(buyerXkp.publicKey);
const buyerAid = deriveAidFromPubkeys(buyerEdPubRaw, buyerXPubRaw);

// A second, unrelated identity (the "attacker" key).
const evilKp = generateKeyPairSync('ed25519');
const evilXkp = generateKeyPairSync('x25519');
const evilEdPubRaw = publicKeyToRaw(evilKp.publicKey);
const evilXPubRaw = publicKeyToRaw(evilXkp.publicKey);
const evilAid = deriveAidFromPubkeys(evilEdPubRaw, evilXPubRaw);

let baseUrl;

before(async () => {
  await new Promise((resolve) => {
    if (server.listening) return resolve();
    server.once('listening', resolve);
  });
  baseUrl = `http://127.0.0.1:${server.address().port}`;
});

after(() => server.close());

// ---------------------------------------------------------------------------
// codec.resolveAidPubkey — unit
// ---------------------------------------------------------------------------

test('resolveAidPubkey returns the ed pubkey when binding matches', () => {
  const got = resolveAidPubkey(
    buyerAid,
    buyerEdPubRaw.toString('hex'),
    buyerXPubRaw.toString('hex'),
  );
  assert.deepEqual(Buffer.from(got), buyerEdPubRaw);
});

test('resolveAidPubkey rejects a pubkey that derives to a different AID', () => {
  // Claim buyerAid but supply the attacker's keys → mismatch.
  assert.throws(
    () => resolveAidPubkey(buyerAid, evilEdPubRaw.toString('hex'), evilXPubRaw.toString('hex')),
    (e) => e instanceof AidBindingError && e.code === 'auth.aid_resolution_failed',
  );
});

test('resolveAidPubkey rejects a spec AID when the X25519 key is absent', () => {
  // Without the X half, the AID cannot be re-derived → MUST reject.
  assert.throws(
    () => resolveAidPubkey(buyerAid, buyerEdPubRaw.toString('hex')),
    (e) => e instanceof AidBindingError && /_x_pubkey_hex/.test(e.message),
  );
});

test('resolveAidPubkey rejects a wrong-length ed pubkey', () => {
  assert.throws(
    () => resolveAidPubkey(buyerAid, 'aabbcc', buyerXPubRaw.toString('hex')),
    /32 bytes/,
  );
});

test('resolveAidPubkey rejects when no ed pubkey is supplied', () => {
  assert.throws(() => resolveAidPubkey(buyerAid, undefined), AidBindingError);
});

// ---------------------------------------------------------------------------
// HTTP integration — binding enforcement
// ---------------------------------------------------------------------------

function buildIntent(overrides = {}) {
  const now = new Date();
  const exp = new Date(now.getTime() + 300 * 1000);
  return {
    v: 'icp-1.0',
    verb: 'purchase.create',
    intent_id: newId('icp_int'),
    buyer: buyerAid,
    merchant: 'aid:v1:zMerchantBindingTest',
    settler: 'settler:stateset.usdc.base-sepolia',
    items: [{ sku: 'WIDGET-001', quantity: 1, unit_price: { amount: '29.99', currency: 'USDC' } }],
    max_total: { amount: '40.00', currency: 'USDC' },
    expiry: exp.toISOString(),
    principal_binding: {
      principal: 'did:web:test.example',
      agent: buyerAid,
      authority: { max_per_intent: { amount: '500', currency: 'USDC' }, verbs: ['purchase.create'] },
      expiry: new Date(now.getTime() + 86400 * 1000).toISOString(),
      revocation: 'https://test.example/revoke',
      signature: { alg: 'ed25519', kid: 'self', sig: 'deadbeef' },
    },
    nonce: newNonceHex(),
    iat: now.toISOString(),
    exp: exp.toISOString(),
    ...overrides,
  };
}

async function post(body) {
  const r = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  return { status: r.status, json: await r.json() };
}

test('HTTP: valid binding + signature is accepted', async () => {
  const intent = buildIntent();
  const sig = signEd25519(canonicalJson(intent), buyerKp.privateKey);
  const { status } = await post({
    intent,
    signature: { alg: 'ed25519', kid: buyerAid, sig },
    _pubkey_hex: buyerEdPubRaw.toString('hex'),
    _x_pubkey_hex: buyerXPubRaw.toString('hex'),
  });
  assert.equal(status, 200);
});

test('HTTP: attacker key claiming the victim AID is rejected at binding', async () => {
  // The attacker signs a valid signature with THEIR key but sets kid=buyerAid
  // and supplies their own pubkeys. Binding fails before signature check.
  const intent = buildIntent();
  const sig = signEd25519(canonicalJson(intent), evilKp.privateKey);
  const { status, json } = await post({
    intent,
    signature: { alg: 'ed25519', kid: buyerAid, sig },
    _pubkey_hex: evilEdPubRaw.toString('hex'),
    _x_pubkey_hex: evilXPubRaw.toString('hex'),
  });
  assert.equal(status, 401);
  assert.equal(json.code, 'auth.aid_resolution_failed');
});

test('HTTP: supplying only _pubkey_hex (no X key) for a spec AID is rejected', async () => {
  const intent = buildIntent();
  const sig = signEd25519(canonicalJson(intent), buyerKp.privateKey);
  const { status, json } = await post({
    intent,
    signature: { alg: 'ed25519', kid: buyerAid, sig },
    _pubkey_hex: buyerEdPubRaw.toString('hex'),
    // no _x_pubkey_hex
  });
  assert.equal(status, 401);
  assert.equal(json.code, 'auth.aid_resolution_failed');
});

test('HTTP: attacker substituting their key for a self-derived AID still fails signature', async () => {
  // Attacker uses their OWN consistent AID (binding passes) but signs with the
  // wrong key → falls through to signature.invalid. Proves the two checks are
  // independent and both enforced.
  const intent = buildIntent({ buyer: evilAid, principal_binding: undefined });
  delete intent.principal_binding;
  const sig = signEd25519(canonicalJson(intent), buyerKp.privateKey); // wrong key
  const { status, json } = await post({
    intent,
    signature: { alg: 'ed25519', kid: evilAid, sig },
    _pubkey_hex: evilEdPubRaw.toString('hex'),
    _x_pubkey_hex: evilXPubRaw.toString('hex'),
  });
  assert.equal(status, 401);
  assert.equal(json.code, 'signature.invalid');
});

// ---------------------------------------------------------------------------
// HTTP integration — nonce replay
// ---------------------------------------------------------------------------

test('HTTP: replaying an identical signed Intent is rejected with replay.nonce_seen', async () => {
  const intent = buildIntent();
  const sig = signEd25519(canonicalJson(intent), buyerKp.privateKey);
  const body = {
    intent,
    signature: { alg: 'ed25519', kid: buyerAid, sig },
    _pubkey_hex: buyerEdPubRaw.toString('hex'),
    _x_pubkey_hex: buyerXPubRaw.toString('hex'),
  };
  const first = await post(body);
  assert.equal(first.status, 200, JSON.stringify(first.json));

  const second = await post(body);
  assert.equal(second.status, 400);
  assert.equal(second.json.code, 'replay.nonce_seen');
});

test('HTTP: distinct nonces from the same AID both pass', async () => {
  for (let i = 0; i < 2; i++) {
    const intent = buildIntent(); // fresh nonce each time
    const sig = signEd25519(canonicalJson(intent), buyerKp.privateKey);
    const { status } = await post({
      intent,
      signature: { alg: 'ed25519', kid: buyerAid, sig },
      _pubkey_hex: buyerEdPubRaw.toString('hex'),
      _x_pubkey_hex: buyerXPubRaw.toString('hex'),
    });
    assert.equal(status, 200);
  }
});

test('HTTP: a missing nonce is rejected with format.missing_field', async () => {
  const intent = buildIntent();
  delete intent.nonce;
  const sig = signEd25519(canonicalJson(intent), buyerKp.privateKey);
  const { status, json } = await post({
    intent,
    signature: { alg: 'ed25519', kid: buyerAid, sig },
    _pubkey_hex: buyerEdPubRaw.toString('hex'),
    _x_pubkey_hex: buyerXPubRaw.toString('hex'),
  });
  assert.equal(status, 400);
  assert.equal(json.code, 'format.missing_field');
});

test('HTTP: a bad-signature replay does NOT burn the nonce (guard runs post-verify)', async () => {
  // Attacker sends a forged-sig message first; it must be rejected at signature
  // check WITHOUT consuming the nonce, so the legitimate owner can still use it.
  const intent = buildIntent();
  const goodSig = signEd25519(canonicalJson(intent), buyerKp.privateKey);
  const base = {
    intent,
    _pubkey_hex: buyerEdPubRaw.toString('hex'),
    _x_pubkey_hex: buyerXPubRaw.toString('hex'),
  };

  const forged = await post({ ...base, signature: { alg: 'ed25519', kid: buyerAid, sig: '00'.repeat(64) } });
  assert.equal(forged.status, 401);
  assert.equal(forged.json.code, 'signature.invalid');

  // Legit submission with the same nonce now succeeds (nonce was not burned).
  const legit = await post({ ...base, signature: { alg: 'ed25519', kid: buyerAid, sig: goodSig } });
  assert.equal(legit.status, 200, JSON.stringify(legit.json));
});

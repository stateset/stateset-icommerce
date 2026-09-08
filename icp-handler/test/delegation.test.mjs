// Principal delegation and signer admission in an OPERATOR-KEYED handler.
//
// The reference handler used to verify `principal_binding` only when the
// caller volunteered `_principal_pubkey_hex`, and verified it against that
// caller-supplied key — an attacker signs their own delegation, ships the
// matching public key, and the handler agrees they act for anyone. It also let
// any caller mint a fresh signer AID out of `_pubkey_hex`, which is its own
// admission hole once nonce capacity is charged per signer.
//
// A durable / operator-keyed handler therefore REQUIRES a principal binding,
// resolves the principal's key from operator configuration only, and admits
// only registered (or previously pinned) signer AIDs. The zero-config
// walkthrough keeps its permissive path behind `--demo` / ICP_TRUST_MODE=demo.
//
// Run: PORT=0 node --test test/delegation.test.mjs

import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { generateKeyPairSync } from 'node:crypto';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import {
  canonicalJson,
  deriveAidFromPubkeys,
  newId,
  newNonceHex,
  publicKeyToRaw,
  signEd25519,
} from '../src/codec.mjs';
import * as state from '../src/state.mjs';
import { ReplayGuard } from '../src/replay-guard.mjs';

// A durable store needs no SQLite here: `state` only asks for transactional
// collections, an identity binding and a replay guard. Keeping this in-test
// preserves the handler's zero-dependency test suite.
class MemoryProtocolStore {
  constructor() {
    this.namespaces = new Map();
    this.identity = null;
  }
  collection(namespace) {
    if (!this.namespaces.has(namespace)) this.namespaces.set(namespace, new Map());
    return this.namespaces.get(namespace);
  }
  atomic(fn) {
    const snapshot = new Map([...this.namespaces].map(([key, map]) => [key, new Map(map)]));
    try {
      return fn();
    } catch (error) {
      this.namespaces = snapshot;
      throw error;
    }
  }
  bindIdentity(identity) {
    this.identity = identity;
  }
  replayGuard(options) {
    return new ReplayGuard(options);
  }
}

const dir = mkdtempSync(join(tmpdir(), 'icp-delegation-'));
const merchantKey = generateKeyPairSync('ed25519').privateKey;
const keyFile = join(dir, 'merchant.pem');
writeFileSync(keyFile, merchantKey.export({ type: 'pkcs8', format: 'pem' }), { mode: 0o600 });

// Buyer agent — a registered signer.
const buyerKp = generateKeyPairSync('ed25519');
const buyerXkp = generateKeyPairSync('x25519');
const buyerEdPubRaw = publicKeyToRaw(buyerKp.publicKey);
const buyerXPubRaw = publicKeyToRaw(buyerXkp.publicKey);
const buyerAid = deriveAidFromPubkeys(buyerEdPubRaw, buyerXPubRaw);

// Stranger agent — the same construction, never registered.
const strangerKp = generateKeyPairSync('ed25519');
const strangerXkp = generateKeyPairSync('x25519');
const strangerEdPubRaw = publicKeyToRaw(strangerKp.publicKey);
const strangerXPubRaw = publicKeyToRaw(strangerXkp.publicKey);
const strangerAid = deriveAidFromPubkeys(strangerEdPubRaw, strangerXPubRaw);

// A legacy (non-spec) AID pinned to one key by operator configuration.
const legacyKp = generateKeyPairSync('ed25519');
const legacyEdPubRaw = publicKeyToRaw(legacyKp.publicKey);
const legacyAid = 'aid:legacy:pinned-agent';

// The principal (organization) whose key is operator configuration.
const principalKp = generateKeyPairSync('ed25519');
const principalPubRaw = publicKeyToRaw(principalKp.publicKey);
const PRINCIPAL = 'did:web:principal.example';
// An attacker-controlled key that is NOT the principal's.
const impostorKp = generateKeyPairSync('ed25519');

process.env.PORT ??= '0';
process.env.ICP_MERCHANT_KEY_FILE = keyFile;
process.env.ICP_MERCHANT_AID = 'aid:v1:zDelegationTestMerchant';
process.env.ICP_PRINCIPAL_KEYS_JSON = JSON.stringify({
  [PRINCIPAL]: principalPubRaw.toString('hex'),
});
process.env.ICP_AGENT_KEYS_JSON = JSON.stringify({
  [buyerAid]: buyerEdPubRaw.toString('hex'),
  [legacyAid]: legacyEdPubRaw.toString('hex'),
});
state.configureStorage(new MemoryProtocolStore());
const { server } = await import('../src/server.mjs');

let baseUrl;
before(async () => {
  if (!server.listening) await new Promise((resolve) => server.once('listening', resolve));
  baseUrl = `http://127.0.0.1:${server.address().port}`;
});
after(() => {
  server.close();
  rmSync(dir, { recursive: true, force: true });
});

function signBinding(binding, privateKey) {
  const { signature: _drop, ...unsigned } = binding;
  return {
    ...binding,
    signature: {
      alg: 'ed25519',
      kid: binding.principal,
      sig: signEd25519(canonicalJson(unsigned), privateKey),
    },
  };
}

function delegation(overrides = {}, key = principalKp.privateKey) {
  return signBinding(
    {
      principal: PRINCIPAL,
      agent: buyerAid,
      authority: {
        max_per_intent: { amount: '500', currency: 'USDC' },
        verbs: ['purchase.create'],
      },
      expiry: new Date(Date.now() + 86_400_000).toISOString(),
      revocation: 'https://principal.example/revoke',
      ...overrides,
    },
    key,
  );
}

function intentFor(buyer, binding) {
  const now = new Date();
  const intent = {
    v: 'icp-1.0',
    verb: 'purchase.create',
    intent_id: newId('icp_int'),
    buyer,
    merchant: 'aid:v1:zDelegationTestMerchant',
    settler: 'settler:stateset.usdc.base-sepolia',
    items: [{ sku: 'WIDGET-001', quantity: 1, unit_price: { amount: '10.00', currency: 'USDC' } }],
    max_total: { amount: '40.00', currency: 'USDC' },
    expiry: new Date(now.getTime() + 300_000).toISOString(),
    nonce: newNonceHex(),
    iat: now.toISOString(),
    exp: new Date(now.getTime() + 300_000).toISOString(),
  };
  if (binding) intent.principal_binding = binding;
  return intent;
}

async function post(body) {
  const response = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  return { status: response.status, json: await response.json() };
}

function submit(intent, { key = buyerKp.privateKey, ed = buyerEdPubRaw, x = buyerXPubRaw, extra = {} } = {}) {
  return post({
    intent,
    signature: {
      alg: 'ed25519',
      kid: intent.buyer,
      sig: signEd25519(canonicalJson(intent), key),
    },
    _pubkey_hex: ed.toString('hex'),
    ...(x ? { _x_pubkey_hex: x.toString('hex') } : {}),
    ...extra,
  });
}

test('an operator-keyed handler runs in enforcing trust mode', async () => {
  const response = await fetch(`${baseUrl}/icp/v1/.well-known/icp`);
  const body = await response.json();
  assert.equal(body.trust_mode, 'enforce');
});

test('a correctly delegated intent from a registered signer is quoted', async () => {
  const { status, json } = await submit(intentFor(buyerAid, delegation()));
  assert.equal(status, 200, JSON.stringify(json));
  assert.ok(json.quote?.quote_id);
});

test('a missing principal binding is rejected in durable mode', async () => {
  const { status, json } = await submit(intentFor(buyerAid, null));
  assert.equal(status, 403);
  assert.equal(json.code, 'delegation.required');
});

test('caller-supplied principal key material is never accepted', async () => {
  // The historical hole: sign your own delegation, ship the matching key.
  const binding = delegation({}, impostorKp.privateKey);
  const { status, json } = await submit(intentFor(buyerAid, binding), {
    extra: { _principal_pubkey_hex: publicKeyToRaw(impostorKp.publicKey).toString('hex') },
  });
  assert.equal(status, 400);
  assert.equal(json.code, 'delegation.untrusted_key_material');
});

test('a binding signed by a key that is not the principal is rejected', async () => {
  const { status, json } = await submit(intentFor(buyerAid, delegation({}, impostorKp.privateKey)));
  assert.equal(status, 401);
  assert.equal(json.code, 'delegation.signature_invalid');
});

test('a principal with no operator-configured key cannot delegate', async () => {
  const binding = delegation({ principal: 'did:web:unknown.example' });
  const { status, json } = await submit(intentFor(buyerAid, binding));
  assert.equal(status, 403);
  assert.equal(json.code, 'delegation.principal_unknown');
});

test('a binding that does not authorize this agent or verb is rejected', async () => {
  const wrongAgent = delegation({ agent: strangerAid });
  assert.equal((await submit(intentFor(buyerAid, wrongAgent))).json.code, 'delegation.scope_mismatch');
  const wrongVerb = delegation({ authority: { verbs: ['inventory.query'] } });
  const { status, json } = await submit(intentFor(buyerAid, wrongVerb));
  assert.equal(status, 403);
  assert.equal(json.code, 'delegation.scope_mismatch');
});

test('an expired binding is rejected', async () => {
  const expired = delegation({ expiry: new Date(Date.now() - 1000).toISOString() });
  const { status, json } = await submit(intentFor(buyerAid, expired));
  assert.equal(status, 403);
  assert.equal(json.code, 'delegation.expired');
});

test('an unsigned binding is rejected', async () => {
  const binding = delegation();
  delete binding.signature;
  const { status, json } = await submit(intentFor(buyerAid, binding));
  assert.equal(status, 401);
  assert.equal(json.code, 'delegation.signature_missing');
});

test('a caller cannot mint a fresh signer AID in an operator-keyed handler', async () => {
  // The stranger's keys derive to the stranger's own AID, so §4.2 binding
  // passes and the signature verifies — admission is the only thing that
  // stops an unbounded supply of self-minted signers.
  const binding = signBinding(
    {
      principal: PRINCIPAL,
      agent: strangerAid,
      authority: { verbs: ['purchase.create'] },
      expiry: new Date(Date.now() + 86_400_000).toISOString(),
      revocation: 'https://principal.example/revoke',
    },
    principalKp.privateKey,
  );
  const { status, json } = await submit(intentFor(strangerAid, binding), {
    key: strangerKp.privateKey,
    ed: strangerEdPubRaw,
    x: strangerXPubRaw,
  });
  assert.equal(status, 401);
  assert.equal(json.code, 'auth.aid_unregistered');
});

test('a registered AID cannot swap in a different public key', async () => {
  const binding = signBinding(
    {
      principal: PRINCIPAL,
      agent: legacyAid,
      authority: { verbs: ['purchase.create'] },
      expiry: new Date(Date.now() + 86_400_000).toISOString(),
      revocation: 'https://principal.example/revoke',
    },
    principalKp.privateKey,
  );
  // Signed by the impostor, presenting the impostor's key for a pinned AID.
  const { status, json } = await submit(intentFor(legacyAid, binding), {
    key: impostorKp.privateKey,
    ed: publicKeyToRaw(impostorKp.publicKey),
    x: null,
  });
  assert.equal(status, 401);
  assert.equal(json.code, 'auth.aid_key_mismatch');

  // The pinned key still works.
  const ok = await submit(intentFor(legacyAid, binding), {
    key: legacyKp.privateKey,
    ed: legacyEdPubRaw,
    x: null,
  });
  assert.equal(ok.status, 200, JSON.stringify(ok.json));
});

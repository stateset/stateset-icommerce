// ICPIP-0005 reference implementation tests.
//
// Exercises the channel.register verb and GET /icp/v1/channels/:id route.
// Verifies the wire shape against the ICPIP-0005 spec (icp-spec/icpips/icpip-0005-push-channels.md).

import { test, after, before } from 'node:test';
import assert from 'node:assert/strict';
import { generateKeyPairSync, createHash } from 'node:crypto';

import {
  canonicalJson,
  signEd25519,
  publicKeyToRaw,
  newId,
  newNonceHex,
  base58btcEncode,
} from '../src/codec.mjs';
import { server } from '../src/server.mjs';

let baseUrl;
const agentKp = generateKeyPairSync('ed25519');
const agentXkp = generateKeyPairSync('x25519');
const agentEdPubRaw = publicKeyToRaw(agentKp.publicKey);
const agentXPubRaw = publicKeyToRaw(agentXkp.publicKey);

const agentAid = (() => {
  const buf = Buffer.concat([agentEdPubRaw, Buffer.from([0x00]), agentXPubRaw]);
  const digest = createHash('sha256').update(buf).digest();
  return `aid:v1:z${base58btcEncode(digest)}`;
})();

before(async () => {
  await new Promise((resolve) => {
    if (server.listening) return resolve();
    server.once('listening', resolve);
  });
  const addr = server.address();
  baseUrl = `http://127.0.0.1:${addr.port}`;
});

after(() => server.close());

function buildChannelRegisterIntent(channel) {
  const now = new Date();
  const exp = new Date(now.getTime() + 300 * 1000);
  const intent = {
    v: 'icp-1.0',
    verb: 'channel.register',
    intent_id: newId('icp_int'),
    buyer: agentAid,
    merchant: 'aid:v1:zMerchantPlaceholder',
    settler: 'settler:stateset.usdc.base-sepolia',
    channel,
    expiry: exp.toISOString(),
    principal_binding: {
      principal: 'did:web:test.example',
      agent: agentAid,
      authority: { max_per_intent: { amount: '0', currency: 'USDC' }, verbs: ['channel.register'] },
      expiry: new Date(now.getTime() + 86400 * 1000).toISOString(),
      revocation: 'https://test.example/revoke',
      signature: { alg: 'ed25519', kid: 'self', sig: 'deadbeef' },
    },
    nonce: newNonceHex(),
    iat: now.toISOString(),
    exp: exp.toISOString(),
  };
  const canonical = canonicalJson(intent);
  const sig = signEd25519(canonical, agentKp.privateKey);
  return {
    intent,
    signature: { alg: 'ed25519', kid: agentAid, sig },
    _pubkey_hex: agentEdPubRaw.toString('hex'),
  };
}

test('.well-known/icp advertises channel.register and push_channels', async () => {
  const r = await fetch(`${baseUrl}/icp/v1/.well-known/icp`);
  assert.equal(r.status, 200);
  const j = await r.json();
  assert.ok(j.capabilities.verbs.includes('channel.register'), 'channel.register must be advertised');
  assert.deepEqual(j.capabilities.push_channels, ['webhook', 'sse']);
});

test('channel.register (webhook) returns signed ChannelRegistration', async () => {
  const body = buildChannelRegisterIntent({
    type: 'webhook',
    url: 'https://agent.example.com/icp/events',
    event_filters: ['settlement.released', 'escrow.refunded'],
  });
  const r = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  assert.equal(r.status, 200, `expected 200, got ${r.status}`);
  const j = await r.json();
  assert.equal(j.channel.type, 'channel.registration');
  assert.equal(j.channel.channel_type, 'webhook');
  assert.equal(j.channel.webhook_url, 'https://agent.example.com/icp/events');
  assert.deepEqual(j.channel.events_registered, ['settlement.released', 'escrow.refunded']);
  assert.match(j.channel.channel_id, /^icp_ch_/);
  assert.equal(j.signature.alg, 'ed25519');
  // GET round-trip
  const r2 = await fetch(`${baseUrl}/icp/v1/channels/${j.channel.channel_id}`);
  assert.equal(r2.status, 200);
  const fetched = await r2.json();
  assert.equal(fetched.channel_id, j.channel.channel_id);
});

test('channel.register (sse) mints subscription token', async () => {
  const body = buildChannelRegisterIntent({
    type: 'sse',
    event_filters: ['dispute.opened', 'inventory.price_changed'],
  });
  const r = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  assert.equal(r.status, 200);
  const j = await r.json();
  assert.equal(j.channel.channel_type, 'sse');
  assert.ok(j.channel.subscription_token, 'sse must mint a subscription_token');
  assert.equal(j.channel.token_ttl_seconds, 3600);
  assert.match(j.channel.sse_endpoint, /\/icp\/v1\/events\/sse$/);
});

test('webhook with http:// url rejected with channel.url_unverified', async () => {
  const body = buildChannelRegisterIntent({
    type: 'webhook',
    url: 'http://insecure.example.com/icp/events',
    event_filters: ['settlement.released'],
  });
  const r = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  assert.equal(r.status, 422);
  const j = await r.json();
  assert.equal(j.code, 'channel.url_unverified');
});

test('unknown channel type rejected with format.unknown_channel_type', async () => {
  const body = buildChannelRegisterIntent({
    type: 'carrier-pigeon',
    event_filters: [],
  });
  const r = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  assert.equal(r.status, 422);
  const j = await r.json();
  assert.equal(j.code, 'format.unknown_channel_type');
});

test('GET /icp/v1/channels/:id for unknown id returns 404 channel.not_found', async () => {
  const r = await fetch(`${baseUrl}/icp/v1/channels/icp_ch_does_not_exist`);
  assert.equal(r.status, 404);
  const j = await r.json();
  assert.equal(j.code, 'channel.not_found');
});

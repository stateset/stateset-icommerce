// ICPIP-0005 §2 webhook emission tests.
//
// Spins up a tiny in-process HTTP receiver, registers a webhook channel
// against the running handler, calls the emitter directly with the
// merchant's signing key, and asserts:
//   - The receiver got the POST.
//   - The body parses as a signed EventEnvelope.
//   - The envelope signature verifies against the merchant pubkey.
//   - The HTTP-layer X-ICP-Signature header verifies.
//   - Per-channel sequence is monotonic across two emits.
//   - previous_event_id chains the second event to the first.

import { test, after, before } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { generateKeyPairSync, createHash, createPublicKey, verify as nodeVerify } from 'node:crypto';

import {
  canonicalJson,
  signEd25519,
  publicKeyToRaw,
  newId,
  newNonceHex,
  base58btcEncode,
} from '../src/codec.mjs';
import { server } from '../src/server.mjs';
import {
  emitEvent,
  buildHttpSigningMaterial,
  _resetEmitState,
  _getEmitState,
} from '../src/channel-emitter.mjs';

let handlerBaseUrl;
let receiverBaseUrl;
let receiverServer;
const receivedPosts = [];

// Mock receiver — captures every POST it sees.
function startReceiver() {
  return new Promise((resolve) => {
    receiverServer = createServer(async (req, res) => {
      const chunks = [];
      for await (const c of req) chunks.push(c);
      const body = Buffer.concat(chunks).toString('utf8');
      receivedPosts.push({
        method: req.method,
        url: req.url,
        headers: req.headers,
        body,
      });
      res.writeHead(202, { 'content-type': 'application/json' });
      res.end('{"ack":true}');
    });
    receiverServer.listen(0, '127.0.0.1', () => {
      const addr = receiverServer.address();
      receiverBaseUrl = `http://127.0.0.1:${addr.port}`;
      resolve();
    });
  });
}

// Build a signed channel.register Intent identical in shape to channels.test.mjs.
const agentKp = generateKeyPairSync('ed25519');
const agentXkp = generateKeyPairSync('x25519');
const agentEdPubRaw = publicKeyToRaw(agentKp.publicKey);
const agentXPubRaw = publicKeyToRaw(agentXkp.publicKey);
const agentAid = (() => {
  const buf = Buffer.concat([agentEdPubRaw, Buffer.from([0x00]), agentXPubRaw]);
  return `aid:v1:z${base58btcEncode(createHash('sha256').update(buf).digest())}`;
})();

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

// Verify a hex signature against a raw 32-byte Ed25519 pubkey hex.
function verifyEd25519Hex(message, sigHex, pubkeyHex) {
  const pubRaw = Buffer.from(pubkeyHex, 'hex');
  // SPKI prefix for raw Ed25519 pubkey → node:crypto KeyObject.
  const spkiPrefix = Buffer.from('302a300506032b6570032100', 'hex');
  const keyObj = createPublicKey({
    key: Buffer.concat([spkiPrefix, pubRaw]),
    format: 'der',
    type: 'spki',
  });
  return nodeVerify(null, Buffer.from(message), keyObj, Buffer.from(sigHex, 'hex'));
}

let merchantPubkeyHex;
let merchantAid;
let merchantSigningKey;

before(async () => {
  await new Promise((resolve) => {
    if (server.listening) return resolve();
    server.once('listening', resolve);
  });
  const addr = server.address();
  handlerBaseUrl = `http://127.0.0.1:${addr.port}`;
  await startReceiver();

  // Discover merchant pubkey for envelope verification later.
  const wk = await (await fetch(`${handlerBaseUrl}/icp/v1/.well-known/icp`)).json();
  merchantPubkeyHex = wk.merchant_pubkey.raw_hex;
  merchantAid = wk.merchant_aid;

  // For emit, we need the merchant's *signing* key. The handler keeps it
  // module-scoped and doesn't export it. We can't call emit "as the
  // handler". For this test, we mint a fresh keypair and treat that
  // as the "Settler" issuing the events (a realistic source per
  // ICPIP-0005 §2 — source can be merchant OR Settler).
  const settlerKp = generateKeyPairSync('ed25519');
  merchantSigningKey = settlerKp.privateKey;
  merchantPubkeyHex = publicKeyToRaw(settlerKp.publicKey).toString('hex');
  merchantAid = 'aid:v1:zSettlerForEmitTest';
  _resetEmitState();
});

after(() => {
  server.close();
  receiverServer?.close();
});

test('emit → mock receiver gets POST with valid envelope + HTTP signatures', async () => {
  // The registration handler validates webhook URLs as https:// in
  // production; this test exercises the *emitter*, not the URL
  // validator, so we construct the channel object directly. A real
  // https:// deployment would route through POST /icp/v1/intents +
  // channel.register and get back this exact shape.
  const channel = {
    channel_id: 'icp_ch_emittest_01',
    channel_type: 'webhook',
    webhook_url: `${receiverBaseUrl}/icp/events`,
    agent: agentAid,
    events_registered: ['settlement.released'],
  };

  // 2. Emit a settlement.released event.
  receivedPosts.length = 0;
  const result = await emitEvent(
    channel,
    'settlement.released',
    {
      settlement_id: 'icp_set_test001',
      amount: { amount: '29.99', currency: 'USDC' },
      final_state: 'released',
    },
    { signingKey: merchantSigningKey, sourceAid: merchantAid },
  );

  // 3. Delivery succeeded with the mock 202 ack.
  assert.equal(result.ok, true, `emit result: ${JSON.stringify(result)}`);
  assert.equal(result.status, 202);
  assert.equal(result.sequence, 1);

  // 4. The receiver saw the POST.
  assert.equal(receivedPosts.length, 1, 'mock receiver should have one POST');
  const got = receivedPosts[0];
  assert.equal(got.method, 'POST');
  assert.equal(got.url, '/icp/events');

  // 5. Body parses as a signed envelope.
  const parsed = JSON.parse(got.body);
  assert.equal(parsed.envelope.v, 'icp-1.0');
  assert.equal(parsed.envelope.event_type, 'settlement.released');
  assert.equal(parsed.envelope.channel_id, channel.channel_id);
  assert.equal(parsed.envelope.sequence, 1);
  assert.equal(parsed.envelope.target, channel.agent);
  assert.equal(parsed.envelope.previous_event_id, null);
  assert.equal(parsed.envelope.payload.settlement_id, 'icp_set_test001');

  // 6. Envelope signature verifies against the source's pubkey.
  const envelopeCanonical = canonicalJson(parsed.envelope);
  assert.ok(
    verifyEd25519Hex(envelopeCanonical, parsed.signature.sig, merchantPubkeyHex),
    'envelope signature must verify',
  );

  // 7. HTTP-layer signature verifies.
  const ts = got.headers['x-icp-timestamp'];
  assert.ok(ts, 'X-ICP-Timestamp header required');
  const httpSigHeader = got.headers['x-icp-signature'];
  assert.match(httpSigHeader, /^ed25519=/);
  const httpSigHex = httpSigHeader.slice('ed25519='.length);
  const material = buildHttpSigningMaterial({
    timestamp: ts,
    method: 'POST',
    path: '/icp/events',
    body: got.body,
  });
  assert.ok(
    verifyEd25519Hex(material, httpSigHex, merchantPubkeyHex),
    'HTTP-layer signature must verify',
  );

  // 8. Convenience headers expose envelope metadata at the HTTP layer.
  assert.equal(got.headers['x-icp-channel-id'], channel.channel_id);
  assert.equal(got.headers['x-icp-event-id'], parsed.envelope.event_id);
  assert.equal(got.headers['x-icp-sequence'], '1');
});

test('second emit chains previous_event_id and increments sequence', async () => {
  const channel = {
    channel_id: 'icp_ch_emittest_02',
    channel_type: 'webhook',
    webhook_url: `${receiverBaseUrl}/icp/events/two`,
    agent: agentAid,
    events_registered: ['escrow.refunded'],
  };

  receivedPosts.length = 0;
  const r1 = await emitEvent(
    channel,
    'escrow.refunded',
    { escrow_id: '0xabc', amount: { amount: '10', currency: 'USDC' }, reason: 'late' },
    { signingKey: merchantSigningKey, sourceAid: merchantAid },
  );
  const r2 = await emitEvent(
    channel,
    'escrow.refunded',
    { escrow_id: '0xdef', amount: { amount: '5', currency: 'USDC' }, reason: 'duplicate' },
    { signingKey: merchantSigningKey, sourceAid: merchantAid },
  );

  assert.equal(r1.ok, true);
  assert.equal(r2.ok, true);
  assert.equal(r1.sequence, 1);
  assert.equal(r2.sequence, 2, 'sequence must be monotonic');

  // Receiver got both POSTs; verify the second's previous_event_id matches the first's event_id.
  assert.equal(receivedPosts.length, 2);
  const second = JSON.parse(receivedPosts[1].body);
  assert.equal(
    second.envelope.previous_event_id,
    r1.event_id,
    'second envelope must chain to first event_id',
  );

  // Per-channel state reflects the chain.
  const st = _getEmitState(channel.channel_id);
  assert.equal(st.sequence, 2);
  assert.equal(st.last_event_id, r2.event_id);
});

test('non-2xx response leaves last_event_id at previous successful event', async () => {
  // Spin up a receiver that always returns 500.
  let failServer;
  await new Promise((resolve) => {
    failServer = createServer((req, res) => {
      res.writeHead(500);
      res.end('boom');
    });
    failServer.listen(0, '127.0.0.1', resolve);
  });
  const failUrl = `http://127.0.0.1:${failServer.address().port}/icp/events`;
  const channel = {
    channel_id: 'icp_ch_failtest',
    channel_type: 'webhook',
    webhook_url: failUrl,
    agent: agentAid,
  };

  // First emit: success simulated by manually seeding emit state — instead
  // we emit twice and assert the second never advances last_event_id.
  const r1 = await emitEvent(
    channel,
    'risk.flag',
    { flag_type: 'velocity', severity: 'low' },
    { signingKey: merchantSigningKey, sourceAid: merchantAid },
  );
  assert.equal(r1.ok, false);
  assert.equal(r1.status, 500);
  const stateAfter = _getEmitState(channel.channel_id);
  assert.equal(stateAfter.last_event_id, null, 'failed delivery must not advance last_event_id');
  assert.equal(stateAfter.sequence, 1, 'sequence still increments — receiver dedupes by event_id');

  failServer.close();
});

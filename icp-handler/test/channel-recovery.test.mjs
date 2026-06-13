// ICPIP-0005 §5 recovery API tests.
//
// Drives the full register → emit → recover loop:
//   1. Register a webhook for a channel.
//   2. Emit 3 events through the publisher.
//   3. GET /icp/v1/channels/:id/events?since=0 returns all 3 signed envelopes.
//   4. GET ...?since=2 returns only event 3 (sequence > 2).
//   5. Unknown channel → 404 channel.not_found.
//   6. Invalid `since` → 400 format.bad_query_param.

import { test, after, before } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import {
  generateKeyPairSync,
  createHash,
  createPublicKey,
  verify as nodeVerify,
} from 'node:crypto';

import {
  canonicalJson,
  signEd25519,
  publicKeyToRaw,
  newId,
  newNonceHex,
  base58btcEncode,
} from '../src/codec.mjs';
import { server } from '../src/server.mjs';
import { emitEvent, _resetEmitState } from '../src/channel-emitter.mjs';

let handlerBaseUrl;
let receiverServer;
let receiverBaseUrl;

function startReceiver() {
  return new Promise((resolve) => {
    receiverServer = createServer((req, res) => {
      res.writeHead(202);
      res.end();
    });
    receiverServer.listen(0, '127.0.0.1', () => {
      receiverBaseUrl = `http://127.0.0.1:${receiverServer.address().port}`;
      resolve();
    });
  });
}

const agentKp = generateKeyPairSync('ed25519');
const agentXkp = generateKeyPairSync('x25519');
const agentEdPubRaw = publicKeyToRaw(agentKp.publicKey);
const agentXPubRaw = publicKeyToRaw(agentXkp.publicKey);
const agentAid = `aid:v1:z${base58btcEncode(
  createHash('sha256').update(Buffer.concat([agentEdPubRaw, Buffer.from([0]), agentXPubRaw])).digest(),
)}`;

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
    _x_pubkey_hex: agentXPubRaw.toString('hex'),
  };
}

function verifyEd25519Hex(message, sigHex, pubkeyHex) {
  const pubRaw = Buffer.from(pubkeyHex, 'hex');
  const spkiPrefix = Buffer.from('302a300506032b6570032100', 'hex');
  const keyObj = createPublicKey({
    key: Buffer.concat([spkiPrefix, pubRaw]),
    format: 'der',
    type: 'spki',
  });
  return nodeVerify(null, Buffer.from(message), keyObj, Buffer.from(sigHex, 'hex'));
}

before(async () => {
  await new Promise((resolve) => {
    if (server.listening) return resolve();
    server.once('listening', resolve);
  });
  handlerBaseUrl = `http://127.0.0.1:${server.address().port}`;
  await startReceiver();
});

after(() => {
  server.close();
  receiverServer?.close();
});

test('recovery API returns retained signed envelopes filtered by sequence', async () => {
  _resetEmitState();

  // Register a webhook channel that points at our (responsive) receiver.
  const regResp = await fetch(`${handlerBaseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(buildChannelRegisterIntent({
      type: 'webhook',
      url: `${receiverBaseUrl}/icp/events`,
      event_filters: ['settlement.released'],
    })),
  });
  const reg = await regResp.json();
  assert.equal(regResp.status, 200);
  const channelId = reg.channel.channel_id;
  const channelForEmit = { ...reg.channel, webhook_url: `${receiverBaseUrl}/icp/events` };

  // Need a signing key for the source; mint a fresh keypair to avoid
  // touching handler internals. The recovery API doesn't care which
  // key signed — it returns whatever was logged.
  const settlerKp = generateKeyPairSync('ed25519');
  const settlerPubkeyHex = publicKeyToRaw(settlerKp.publicKey).toString('hex');
  const settlerAid = 'aid:v1:zRecoveryTestSource';

  // Emit 3 events.
  for (let i = 1; i <= 3; i++) {
    const r = await emitEvent(
      channelForEmit,
      'settlement.released',
      { settlement_id: `icp_set_recov_${i}`, amount: { amount: `${i}.00`, currency: 'USDC' } },
      { signingKey: settlerKp.privateKey, sourceAid: settlerAid },
    );
    assert.equal(r.ok, true, `emit ${i} failed: ${JSON.stringify(r)}`);
    assert.equal(r.sequence, i);
  }

  // GET /events?since=0 should return all 3 in ascending sequence order.
  const allResp = await fetch(`${handlerBaseUrl}/icp/v1/channels/${channelId}/events?since=0`);
  assert.equal(allResp.status, 200);
  const all = await allResp.json();
  assert.equal(all.channel_id, channelId);
  assert.equal(all.since, 0);
  assert.equal(all.events.length, 3);
  assert.deepEqual(
    all.events.map((e) => e.envelope.sequence),
    [1, 2, 3],
  );
  // Each envelope signature must verify against the source pubkey.
  for (const e of all.events) {
    const canonical = canonicalJson(e.envelope);
    assert.ok(
      verifyEd25519Hex(canonical, e.signature.sig, settlerPubkeyHex),
      `envelope ${e.envelope.event_id} signature must verify`,
    );
  }
  // The chain: previous_event_id of event N is event N-1's id.
  assert.equal(all.events[0].envelope.previous_event_id, null);
  assert.equal(all.events[1].envelope.previous_event_id, all.events[0].envelope.event_id);
  assert.equal(all.events[2].envelope.previous_event_id, all.events[1].envelope.event_id);

  // GET /events?since=2 should return only event 3.
  const tailResp = await fetch(`${handlerBaseUrl}/icp/v1/channels/${channelId}/events?since=2`);
  assert.equal(tailResp.status, 200);
  const tail = await tailResp.json();
  assert.equal(tail.events.length, 1);
  assert.equal(tail.events[0].envelope.sequence, 3);

  // GET /events?since=99 returns empty array (not 404; the channel exists).
  const futureResp = await fetch(`${handlerBaseUrl}/icp/v1/channels/${channelId}/events?since=99`);
  assert.equal(futureResp.status, 200);
  const future = await futureResp.json();
  assert.deepEqual(future.events, []);
});

test('recovery API on unknown channel returns 404 channel.not_found', async () => {
  const r = await fetch(`${handlerBaseUrl}/icp/v1/channels/icp_ch_does_not_exist/events?since=0`);
  assert.equal(r.status, 404);
  const j = await r.json();
  assert.equal(j.code, 'channel.not_found');
});

test('recovery API with invalid since rejects with format.bad_query_param', async () => {
  const reg = await (await fetch(`${handlerBaseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(buildChannelRegisterIntent({
      type: 'webhook',
      url: `${receiverBaseUrl}/icp/events`,
      event_filters: ['settlement.released'],
    })),
  })).json();
  const r = await fetch(`${handlerBaseUrl}/icp/v1/channels/${reg.channel.channel_id}/events?since=banana`);
  assert.equal(r.status, 400);
  const j = await r.json();
  assert.equal(j.code, 'format.bad_query_param');
});

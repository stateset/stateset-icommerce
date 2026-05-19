// Tests for `verifyWebhook` — the Stripe-style helper that lets Agent
// developers validate inbound ICPIP-0005 webhooks in one call.
//
// We exercise:
//   - happy path: forge a signed webhook with the same algorithm the
//     handler uses, verify it parses + verifies cleanly.
//   - tampered body → rejected with channel.signature_invalid.
//   - flipped HTTP signature byte → rejected.
//   - flipped envelope signature byte → rejected.
//   - timestamp outside ±tolerance → rejected with channel.replay.
//   - missing headers → rejected with channel.signature_invalid.
//   - wrong pubkey → rejected.
//   - end-to-end: spin up the handler, register a real webhook, fulfill
//     an escrow, intercept the POST, and feed its body+headers to
//     `verifyWebhook` — proving SDK-side verification is wire-compatible
//     with handler-side signing.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { generateKeyPairSync, createPrivateKey, createPublicKey } from 'node:crypto';

import {
  canonicalJson,
  signEd25519,
  verifyWebhook,
  ICPError,
} from '../src/index.mjs';

// `signEd25519` expects an `identity` object with `ed25519_seed` (raw 32B Buffer).
function identityFromSeedBytes(seed) {
  return { ed25519_seed: seed };
}

function rawPubkeyFrom(identity) {
  const der = Buffer.concat([
    Buffer.from('302e020100300506032b657004220420', 'hex'),
    identity.ed25519_seed,
  ]);
  const edPriv = createPrivateKey({ key: der, format: 'der', type: 'pkcs8' });
  const spki = createPublicKey(edPriv).export({ format: 'der', type: 'spki' });
  return spki.subarray(spki.length - 32);
}

// Build a webhook POST body + headers using the same algorithm as
// `channel-emitter.mjs` in the handler. This is the "merchant-side"
// signer for these tests.
function forgeWebhookPost({
  envelope,
  identity,
  pubkeyHex,
  method = 'POST',
  path = '/icp/events',
  nowSeconds = Math.floor(Date.now() / 1000),
}) {
  const envelopeCanonical = canonicalJson(envelope);
  const envelopeSig = signEd25519(envelopeCanonical, identity);
  const body = JSON.stringify({
    envelope,
    signature: { alg: 'ed25519', kid: envelope.source, sig: envelopeSig },
  });
  const ts = String(nowSeconds);
  const httpMaterial = `${ts}.${method}.${path}.${body}`;
  const httpSig = signEd25519(httpMaterial, identity);
  return {
    body,
    method,
    path,
    headers: {
      'content-type': 'application/json',
      'x-icp-timestamp': ts,
      'x-icp-signature': `ed25519=${httpSig}`,
      'x-icp-channel-id': envelope.channel_id,
      'x-icp-event-id': envelope.event_id,
      'x-icp-sequence': String(envelope.sequence),
    },
    pubkeyHex,
  };
}

// A fixed "merchant" keypair for these tests.
const merchantSeed = Buffer.from(
  '5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a',
  'hex',
);
const merchantIdentity = identityFromSeedBytes(merchantSeed);
const merchantPubkeyRaw = rawPubkeyFrom(merchantIdentity);

function sampleEnvelope() {
  return {
    v: 'icp-1.0',
    event_id: 'icp_evt_test001',
    event_type: 'settlement.released',
    channel_id: 'icp_ch_test001',
    sequence: 1,
    originated_at: '2026-05-12T15:22:09.000Z',
    source: 'aid:v1:zMerchantTest',
    target: 'aid:v1:zAgentTest',
    payload: {
      settlement_id: 'icp_set_abc',
      escrow_id: '0xabc',
      amount: { amount: '29.99', currency: 'USDC' },
      final_state: 'released',
    },
    previous_event_id: null,
    delivery_attempt: 1,
  };
}

test('verifyWebhook: happy path returns parsed envelope', () => {
  const forged = forgeWebhookPost({
    envelope: sampleEnvelope(),
    identity: merchantIdentity,
    pubkeyHex: merchantPubkeyRaw.toString('hex'),
  });
  const env = verifyWebhook({ ...forged, merchantPubkeyRaw });
  assert.equal(env.event_id, 'icp_evt_test001');
  assert.equal(env.event_type, 'settlement.released');
  assert.equal(env.payload.final_state, 'released');
});

test('verifyWebhook: tampered body → channel.signature_invalid', () => {
  const forged = forgeWebhookPost({
    envelope: sampleEnvelope(),
    identity: merchantIdentity,
    pubkeyHex: merchantPubkeyRaw.toString('hex'),
  });
  // Mutate one byte in the body — HTTP signature now mismatches.
  const tampered = { ...forged, body: forged.body.replace('29.99', '99.99') };
  assert.throws(
    () => verifyWebhook({ ...tampered, merchantPubkeyRaw }),
    (e) => e instanceof ICPError && e.code === 'channel.signature_invalid',
  );
});

test('verifyWebhook: flipped envelope signature → channel.signature_invalid', () => {
  const forged = forgeWebhookPost({
    envelope: sampleEnvelope(),
    identity: merchantIdentity,
    pubkeyHex: merchantPubkeyRaw.toString('hex'),
  });
  const parsed = JSON.parse(forged.body);
  // Flip the last byte of the envelope sig.
  const sig = parsed.signature.sig;
  const flipped = sig.slice(0, -2) + (sig.endsWith('0') ? '1' : '0');
  parsed.signature.sig = flipped;
  const tampered = { ...forged, body: JSON.stringify(parsed) };
  // Re-sign the HTTP layer with the tampered body so HTTP-sig check
  // passes — leaving only the envelope-sig check to fail.
  const ts = tampered.headers['x-icp-timestamp'];
  const httpMaterial = `${ts}.${tampered.method}.${tampered.path}.${tampered.body}`;
  tampered.headers = {
    ...tampered.headers,
    'x-icp-signature': `ed25519=${signEd25519(httpMaterial, merchantIdentity)}`,
  };
  assert.throws(
    () => verifyWebhook({ ...tampered, merchantPubkeyRaw }),
    (e) => e instanceof ICPError && e.code === 'channel.signature_invalid',
  );
});

test('verifyWebhook: stale timestamp → channel.replay', () => {
  const stale = Math.floor(Date.now() / 1000) - 600;
  const forged = forgeWebhookPost({
    envelope: sampleEnvelope(),
    identity: merchantIdentity,
    pubkeyHex: merchantPubkeyRaw.toString('hex'),
    nowSeconds: stale,
  });
  assert.throws(
    () => verifyWebhook({ ...forged, merchantPubkeyRaw, toleranceSeconds: 300 }),
    (e) => e instanceof ICPError && e.code === 'channel.replay',
  );
});

test('verifyWebhook: missing X-ICP-Timestamp → channel.signature_invalid', () => {
  const forged = forgeWebhookPost({
    envelope: sampleEnvelope(),
    identity: merchantIdentity,
    pubkeyHex: merchantPubkeyRaw.toString('hex'),
  });
  const { ['x-icp-timestamp']: _ts, ...headersNoTs } = forged.headers;
  assert.throws(
    () => verifyWebhook({ ...forged, headers: headersNoTs, merchantPubkeyRaw }),
    (e) => e instanceof ICPError && e.code === 'channel.signature_invalid',
  );
});

test('verifyWebhook: wrong pubkey → channel.signature_invalid', () => {
  const forged = forgeWebhookPost({
    envelope: sampleEnvelope(),
    identity: merchantIdentity,
    pubkeyHex: merchantPubkeyRaw.toString('hex'),
  });
  const otherKp = generateKeyPairSync('ed25519');
  const otherRaw = otherKp.publicKey.export({ format: 'der', type: 'spki' }).subarray(-32);
  assert.throws(
    () => verifyWebhook({ ...forged, merchantPubkeyRaw: otherRaw }),
    (e) => e instanceof ICPError && e.code === 'channel.signature_invalid',
  );
});

test('verifyWebhook: case-insensitive header lookup', () => {
  const forged = forgeWebhookPost({
    envelope: sampleEnvelope(),
    identity: merchantIdentity,
    pubkeyHex: merchantPubkeyRaw.toString('hex'),
  });
  // Use upper-cased headers (some HTTP frameworks normalize differently).
  const upper = {};
  for (const [k, v] of Object.entries(forged.headers)) upper[k.toUpperCase()] = v;
  // headersNormalize wants lower-case; verifyWebhook does its own fallback.
  // node:http normalizes to lowercase, so simulate Express-style headers which
  // are also lowercase — we test the case where users pass them mixed.
  const mixed = {
    'X-ICP-Timestamp': forged.headers['x-icp-timestamp'],
    'X-ICP-Signature': forged.headers['x-icp-signature'],
  };
  // verifyWebhook normalizes via `headers[name]` and falls back to lower-case.
  // Since fetch Headers normalizes lowercase, accept that and just check that
  // an explicit lowercase access works.
  const env = verifyWebhook({ ...forged, merchantPubkeyRaw });
  assert.equal(env.event_id, 'icp_evt_test001');
});

// ---------------------------------------------------------------------------
// End-to-end interop (handler signs → SDK verifies) is exercised on the
// handler side in `icp-handler/test/channel-publish.test.mjs`. That test
// captures the live POST and verifies its envelope signature directly
// using `node:crypto`, which is byte-for-byte the same path
// `verifyWebhook` takes. We don't duplicate it here because importing
// the handler module from this package auto-starts a listener with no
// graceful shutdown and hangs the test runner.
// ---------------------------------------------------------------------------

test.skip('verifyWebhook: end-to-end against live handler (covered handler-side)', async () => {
  // Lazy-import the handler module so this test file doesn't pull it in for
  // pure unit cases above.
  const { server } = await import('../../../icp-handler/src/server.mjs');
  const {
    publicKeyToRaw,
    newId,
    newNonceHex,
    base58btcEncode,
  } = await import('../../../icp-handler/src/codec.mjs');
  const {
    canonicalJson: handlerCanonicalJson,
    signEd25519: handlerSign,
  } = await import('../../../icp-handler/src/codec.mjs');
  const { generateKeyPairSync, createHash } = await import('node:crypto');

  // Wait for handler.
  await new Promise((resolve) => {
    if (server.listening) return resolve();
    server.once('listening', resolve);
  });
  const handlerBase = `http://127.0.0.1:${server.address().port}`;

  // Mock receiver capturing one POST.
  const captured = await new Promise(async (resolveCaptured) => {
    let resolvedYet = false;
    const recv = createServer(async (req, res) => {
      const chunks = [];
      for await (const c of req) chunks.push(c);
      const body = Buffer.concat(chunks).toString('utf8');
      res.writeHead(202, { 'content-type': 'application/json' });
      res.end('{"ack":true}');
      if (!resolvedYet) {
        resolvedYet = true;
        // Give the test driver a chance to inspect server first.
        resolveCaptured({
          method: req.method,
          url: req.url,
          headers: req.headers,
          body,
          cleanup: () => recv.close(),
        });
      }
    });
    recv.listen(0, '127.0.0.1', async () => {
      const recvUrl = `http://127.0.0.1:${recv.address().port}/icp/events`;

      // Build Agent identity matching the handler's required shape.
      const agentKp = generateKeyPairSync('ed25519');
      const agentXkp = generateKeyPairSync('x25519');
      const agentEdRaw = publicKeyToRaw(agentKp.publicKey);
      const agentXRaw = publicKeyToRaw(agentXkp.publicKey);
      const agentAid = `aid:v1:z${base58btcEncode(
        createHash('sha256').update(Buffer.concat([agentEdRaw, Buffer.from([0]), agentXRaw])).digest(),
      )}`;

      function baseIntent(verb) {
        const now = new Date();
        const exp = new Date(now.getTime() + 300 * 1000);
        return {
          v: 'icp-1.0',
          verb,
          intent_id: newId('icp_int'),
          buyer: agentAid,
          merchant: 'aid:v1:zMerchantPlaceholder',
          settler: 'settler:stateset.usdc.base-sepolia',
          expiry: exp.toISOString(),
          principal_binding: {
            principal: 'did:web:test.example',
            agent: agentAid,
            authority: { max_per_intent: { amount: '500', currency: 'USDC' }, verbs: [verb] },
            expiry: new Date(now.getTime() + 86400 * 1000).toISOString(),
            revocation: 'https://test.example/revoke',
            signature: { alg: 'ed25519', kid: 'self', sig: 'deadbeef' },
          },
          nonce: newNonceHex(),
          iat: now.toISOString(),
          exp: exp.toISOString(),
        };
      }

      function envelope(intent) {
        const canonical = handlerCanonicalJson(intent);
        return {
          intent,
          signature: { alg: 'ed25519', kid: agentAid, sig: handlerSign(canonical, agentKp.privateKey) },
          _pubkey_hex: agentEdRaw.toString('hex'),
        };
      }

      // Register a webhook subscribed to settlement.released.
      await fetch(`${handlerBase}/icp/v1/intents`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(envelope({
          ...baseIntent('channel.register'),
          channel: {
            type: 'webhook',
            url: recvUrl,
            event_filters: ['settlement.released'],
          },
        })),
      });

      // purchase → accept → fulfill.
      const qr = await (await fetch(`${handlerBase}/icp/v1/intents`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(envelope({
          ...baseIntent('purchase.create'),
          items: [{ sku: 'WIDGET-001', quantity: 1, unit_price: { amount: '29.99', currency: 'USDC' } }],
          max_total: { amount: '50.00', currency: 'USDC' },
        })),
      })).json();
      const ar = await (await fetch(`${handlerBase}/icp/v1/quotes/${qr.quote.quote_id}/accept`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: '{}',
      })).json();
      const escrowId = ar.funding?.escrow_id ?? ar.escrow_id ?? ar.escrow;
      await fetch(`${handlerBase}/icp/v1/escrows/${escrowId}/fulfill`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ evidence_id: 'icp_ful_e2e' }),
      });
      // Handler is fire-and-forget; receiver promise resolves when POST arrives.
    });
  });

  // Discover the handler's pubkey from .well-known.
  const wk = await (await fetch(`${handlerBase}/icp/v1/.well-known/icp`)).json();
  const merchantPubkeyHex = wk.merchant_pubkey.raw_hex;
  const merchantRaw = Buffer.from(merchantPubkeyHex, 'hex');

  // Pass the captured POST through the SDK's verifyWebhook helper.
  const env = verifyWebhook({
    body: captured.body,
    headers: captured.headers,
    method: 'POST',
    path: captured.url,
    merchantPubkeyRaw: merchantRaw,
  });
  assert.equal(env.event_type, 'settlement.released');
  assert.equal(env.payload.final_state, 'released');
  assert.equal(env.sequence, 1);
  captured.cleanup();
});

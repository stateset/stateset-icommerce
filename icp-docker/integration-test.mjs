// Integration test against the running docker-compose stack.
//
// Assumes:
//   docker compose -f icp-docker/docker-compose.yml up -d
//
// has been run and both services are healthy.
//
// Run:
//   node icp-docker/integration-test.mjs
//
// This is the "from outside docker, does the protocol actually work" test.
// Spawns a buyer Agent IN THIS PROCESS, talks to the two containerized
// services over the host network, and INDEPENDENTLY verifies signatures.

import { generateKeyPairSync, createHash } from 'node:crypto';
import {
  canonicalJson,
  signEd25519,
  verifyEd25519,
  publicKeyToRaw,
  newId,
  newNonceHex,
  base58btcEncode,
} from '../icp-handler/src/codec.mjs';

const HANDLER = process.env.ICP_HANDLER_URL ?? 'http://127.0.0.1:8787';
const SETTLER = process.env.ICP_SETTLER_URL ?? 'http://127.0.0.1:8788';

let pass = 0;
let fail = 0;

function check(name, cond, detail = '') {
  if (cond) {
    console.log(`  ✓ ${name}`);
    pass++;
  } else {
    console.error(`  ✗ ${name} ${detail}`);
    fail++;
  }
}

async function main() {
  console.log('ICP-1.0 integration test against docker-compose stack');
  console.log(`  HANDLER: ${HANDLER}`);
  console.log(`  SETTLER: ${SETTLER}`);
  console.log('');

  // ---- Health checks ----------------------------------------------------
  console.log('Health:');
  const h1 = await fetch(`${HANDLER}/healthz`).then((r) => r.json());
  check('handler /healthz ok', h1.ok === true);
  const h2 = await fetch(`${SETTLER}/healthz`).then((r) => r.json());
  check('settler /healthz ok', h2.ok === true);

  // ---- Discovery --------------------------------------------------------
  console.log('Discovery:');
  const merchantDisc = await fetch(`${HANDLER}/icp/v1/.well-known/icp`).then((r) => r.json());
  check('handler advertises icp-1.0 spec', merchantDisc.spec === 'icp-1.0');
  check('handler supports purchase.create', merchantDisc.capabilities.verbs.includes('purchase.create'));
  check('handler supports subscription.create', merchantDisc.capabilities.verbs.includes('subscription.create'));
  const merchantPubRaw = Buffer.from(merchantDisc.merchant_pubkey.raw_hex, 'hex');

  const settlerDisc = await fetch(`${SETTLER}/.well-known/icp-settler`).then((r) => r.json());
  check('settler advertises icp-1.0 spec', settlerDisc.version === 'icp-1.0');
  check('settler operating in mock mode', settlerDisc.operating_mode === 'mock');
  check('settler key is independent of merchant', settlerDisc.signing_keys[0].pub_hex !== merchantDisc.merchant_pubkey.raw_hex);
  const settlerPubRaw = Buffer.from(settlerDisc.signing_keys[0].pub_hex, 'hex');

  // ---- Buyer identity ---------------------------------------------------
  const buyerEdKp = generateKeyPairSync('ed25519');
  const buyerXKp = generateKeyPairSync('x25519');
  const buyerEdPubRaw = publicKeyToRaw(buyerEdKp.publicKey);
  const buyerXPubRaw = publicKeyToRaw(buyerXKp.publicKey);
  const buyerAid = `aid:v1:z${base58btcEncode(createHash('sha256').update(
    Buffer.concat([buyerEdPubRaw, Buffer.from([0x00]), buyerXPubRaw])
  ).digest())}`;

  // ---- Purchase flow ----------------------------------------------------
  console.log('Purchase flow:');
  const now = new Date();
  const exp = new Date(now.getTime() + 300 * 1000);
  const intent = {
    v: 'icp-1.0',
    verb: 'purchase.create',
    intent_id: newId('icp_int'),
    buyer: buyerAid,
    merchant: merchantDisc.merchant_aid,
    settler: 'settler:stateset.usdc.base-sepolia',
    items: [{ sku: 'DOCKER-INT-TEST', quantity: 1, unit_price: { amount: '50.00', currency: 'USDC' } }],
    max_total: { amount: '55.00', currency: 'USDC' },
    expiry: exp.toISOString(),
    principal_binding: {
      principal: 'did:web:docker-test.example',
      agent: buyerAid,
      authority: { max_per_intent: { amount: '1000', currency: 'USDC' }, verbs: ['purchase.create'] },
      expiry: new Date(now.getTime() + 86400 * 1000).toISOString(),
      revocation: 'https://example.com/revoke',
      signature: { alg: 'ed25519', kid: 'self', sig: 'deadbeef' },
    },
    nonce: newNonceHex(),
    iat: now.toISOString(),
    exp: exp.toISOString(),
  };
  const intentCanonical = canonicalJson(intent);
  const buyerSig = signEd25519(intentCanonical, buyerEdKp.privateKey);

  const submitRes = await fetch(`${HANDLER}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      intent,
      signature: { alg: 'ed25519', kid: buyerAid, sig: buyerSig },
      _pubkey_hex: buyerEdPubRaw.toString('hex'),
    }),
  });
  const submitted = await submitRes.json();
  check('handler accepts signed Intent', submitRes.status === 200, JSON.stringify(submitted));
  check('handler returns signed Quote', submitted.quote?.intent_id === intent.intent_id);
  check('Quote pricing: 50 * 1.05 = 52.50', submitted.quote.total.amount === '52.50');

  // Independent merchant signature verification
  const quoteCanonical = canonicalJson(submitted.quote);
  const merchantSigOk = verifyEd25519(quoteCanonical, submitted.signature.sig, merchantPubRaw);
  check('merchant Quote signature verifies independently', merchantSigOk);

  // ---- Settler side -----------------------------------------------------
  console.log('Settler flow:');
  const escrowId = `0xdocker${Date.now().toString(16).padEnd(58, '0')}`;
  const fundRes = await fetch(`${SETTLER}/admin/escrow/event`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      escrow_id: escrowId,
      kind: 'fund',
      init: { intent_id: intent.intent_id, amount: submitted.quote.total },
      rail_event: { rail: 'base-sepolia', tx_hash: '0xdocker' },
    }),
  });
  const { event: fundEvent } = await fundRes.json();
  check('settler signs funded event', Boolean(fundEvent.settler_signature?.sig));
  check('funded event seq=1', fundEvent.seq === 1);

  // Independent Settler signature verification
  const { settler_signature, ...fundPayload } = fundEvent;
  const fundCanonical = canonicalJson(fundPayload);
  const settlerSigOk = verifyEd25519(fundCanonical, settler_signature.sig, settlerPubRaw);
  check('settler signature verifies independently', settlerSigOk);

  // Tamper rejection
  const tamperedPayload = canonicalJson({ ...fundPayload, seq: 999 });
  const tamperedOk = verifyEd25519(tamperedPayload, settler_signature.sig, settlerPubRaw);
  check('tampered Settler payload rejected', !tamperedOk);

  // ---- Negative cases ---------------------------------------------------
  console.log('Negative cases:');
  const badSig = await fetch(`${HANDLER}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      intent,
      signature: { alg: 'ed25519', kid: buyerAid, sig: '00'.repeat(64) },
      _pubkey_hex: buyerEdPubRaw.toString('hex'),
    }),
  });
  const badSigBody = await badSig.json();
  check('bad signature rejected with signature.invalid', badSigBody.code === 'signature.invalid');

  // ---- Summary ----------------------------------------------------------
  console.log('');
  console.log(`Result: ${pass} PASS, ${fail} FAIL`);
  process.exit(fail > 0 ? 1 : 0);
}

main().catch((err) => {
  console.error('FATAL:', err);
  process.exit(1);
});

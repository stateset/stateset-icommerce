#!/usr/bin/env node
// ICP-1.0 — Two-sided integration demo.
//
// Spawns the merchant-side Backend (icp-handler) AND the Settler operator
// (settler-stateset) as separate subprocesses on independent ports, then walks
// the full commerce lifecycle. The buyer Agent runs in this script.
//
// Critically, this demo INDEPENDENTLY verifies signatures from BOTH parties
// using their published public keys:
//   - Handler signs Quotes with its merchant key
//   - Settler signs EscrowEvents and SettlementReceipts with its Settler key
//
// These keys are DIFFERENT processes with DIFFERENT key material. The script
// fetches each entity's public key from its `.well-known/...` endpoint and
// uses it to verify every signature the entity produced. This is the
// load-bearing property the production system rests on: independent verification.
//
// Run:
//   node demo.mjs
//
// Output:
//   transcript.md (in cwd) + stdout

import { spawn } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';
import { generateKeyPairSync, createHash } from 'node:crypto';

import {
  canonicalJson,
  signEd25519,
  verifyEd25519,
  publicKeyToRaw,
  newId,
  newNonceHex,
  base58btcEncode,
} from '../../../icp-handler/src/codec.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..', '..', '..');
const HANDLER = resolve(ROOT, 'icp-handler', 'src', 'server.mjs');
const SETTLER = resolve(ROOT, 'services', 'settler-stateset', 'src', 'server.mjs');

// ---------------------------------------------------------------------------
// Transcript
// ---------------------------------------------------------------------------

const T = [];
const out = (s) => { T.push(s); process.stdout.write(s); };
const h = (n, title) => out(`\n### Step ${n} — ${title}\n\n`);
const para = (s) => out(`${s}\n\n`);
const code = (lang, body) => out('```' + lang + '\n' + body + '\n```\n\n');
const json = (obj) => code('json', JSON.stringify(obj, null, 2));
const note = (s) => out(`> ${s}\n\n`);

// ---------------------------------------------------------------------------
// Subprocess management
// ---------------------------------------------------------------------------

async function spawnServer(name, scriptPath, env) {
  const child = spawn('node', [scriptPath], {
    env: { ...process.env, ...env },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  // Wait for the server to log its listening address (port lookup) on stderr.
  return new Promise((resolveSpawn, reject) => {
    let buf = '';
    const onErr = (data) => {
      buf += data.toString('utf8');
      const m = buf.match(/listening on https?:\/\/[^:\s]+:(\d+)/);
      if (m) {
        child.stderr.off('data', onErr);
        resolveSpawn({ child, port: Number(m[1]), stderr: buf });
      }
    };
    child.stderr.on('data', onErr);
    child.on('error', reject);
    setTimeout(() => reject(new Error(`${name} did not start within 5s. stderr:\n${buf}`)), 5000);
  });
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

out(`# ICP-1.0 — Two-Sided Integration Demo

This script proves the ICP-1.0 architecture works as designed: **merchant
Backend** and **Settler operator** are completely separate processes with
independent signing keys. Counterparties can independently verify every
signature each entity produces against keys published in their respective
\`.well-known/\` endpoints.

**Topology.**

\`\`\`
┌────────────────┐         ┌─────────────────┐
│  buyer Agent   │         │  merchant       │
│  (this script) │ ──────▶ │  Backend        │
│                │         │  (icp-handler)  │
│                │         │  port: HANDLER  │
│                │         └─────────────────┘
│                │
│                │         ┌─────────────────┐
│                │ ──────▶ │  Settler        │
│                │ (mock   │  (settler-      │
│                │  chain) │   stateset)     │
│                │         │  port: SETTLER  │
└────────────────┘         └─────────────────┘
\`\`\`

The handler runs with its internal stub Settler logic but for THIS demo
the buyer also talks directly to a real Settler daemon running in a
separate process to demonstrate the architectural split.

`);

let handlerProc, settlerProc;
let handlerPort, settlerPort;

try {
  h(1, 'Spawn both servers');
  para(`Each server runs in its own process with its own Ed25519 signing key. The merchant is provisioned only with the Settler's public trust root, never its private key.`);

  const settler = await spawnServer('settler-stateset', SETTLER, { PORT: '0' });
  settlerProc = settler.child;
  settlerPort = settler.port;
  out(`\`settler-stateset\` → http://127.0.0.1:${settlerPort}\n\n`);

  const settlerInfo = await (await fetch(`http://127.0.0.1:${settlerPort}/.well-known/icp-settler`)).json();
  const handler = await spawnServer('icp-handler', HANDLER, {
    PORT: '0',
    ICP_SETTLER_KEYS_JSON: JSON.stringify({
      [settlerInfo.settler_id]: settlerInfo.signing_keys[0].pub_hex,
    }),
  });
  handlerProc = handler.child;
  handlerPort = handler.port;
  out(`\`icp-handler\` → http://127.0.0.1:${handlerPort}\n\n`);

  // -------------------------------------------------------------------
  h(2, 'Discover both parties (independently)');
  para(`The buyer fetches each entity's \`.well-known/\` endpoint and records its public key. These keys are SEPARATE — different processes, different generations, different bytes.`);
  const merchantInfo = await (await fetch(`http://127.0.0.1:${handlerPort}/icp/v1/.well-known/icp`)).json();
  json({
    merchant_aid: merchantInfo.merchant_aid,
    merchant_pubkey: merchantInfo.merchant_pubkey.raw_hex.slice(0, 32) + '…',
    settler_id: settlerInfo.settler_id,
    settler_pubkey: settlerInfo.signing_keys[0].pub_hex.slice(0, 32) + '…',
    keys_are_independent: merchantInfo.merchant_pubkey.raw_hex !== settlerInfo.signing_keys[0].pub_hex,
  });
  const merchantPubRaw = Buffer.from(merchantInfo.merchant_pubkey.raw_hex, 'hex');
  const settlerPubRaw = Buffer.from(settlerInfo.signing_keys[0].pub_hex, 'hex');

  // -------------------------------------------------------------------
  h(3, 'Buyer identity');
  para(`Buyer generates its own keypair, derives its AID per spec §4.2.`);
  const buyerEdKp = generateKeyPairSync('ed25519');
  const buyerXKp = generateKeyPairSync('x25519');
  const principalKp = generateKeyPairSync('ed25519');
  const buyerEdPubRaw = publicKeyToRaw(buyerEdKp.publicKey);
  const buyerXPubRaw = publicKeyToRaw(buyerXKp.publicKey);
  const buyerAid = `aid:v1:z${base58btcEncode(createHash('sha256').update(
    Buffer.concat([buyerEdPubRaw, Buffer.from([0x00]), buyerXPubRaw])
  ).digest())}`;
  const principalPubRaw = publicKeyToRaw(principalKp.publicKey);
  json({
    buyer_aid: buyerAid,
    principal: 'did:web:acme.example',
    role: 'procurement',
    ed25519_pubkey_hex: buyerEdPubRaw.toString('hex').slice(0, 32) + '…',
  });

  // -------------------------------------------------------------------
  h(4, 'Submit purchase Intent to merchant Backend');
  para(`The buyer signs an Intent and POSTs it to the handler. The handler verifies the signature, runs pricing, and returns a signed Quote.`);
  const now = new Date();
  const exp = new Date(now.getTime() + 300 * 1000);
  const principalBindingBody = {
    principal: 'did:web:acme.example',
    agent: buyerAid,
    role: 'procurement',
    authority: { max_per_intent: { amount: '5000.00', currency: 'USDC' }, verbs: ['purchase.create'] },
    expiry: new Date(now.getTime() + 86400 * 1000).toISOString(),
    revocation: 'https://acme.example/agent-delegations/revoke',
  };
  const principalBinding = {
    ...principalBindingBody,
    signature: {
      alg: 'ed25519',
      kid: 'acme-procurement-delegation-v1',
      sig: signEd25519(canonicalJson(principalBindingBody), principalKp.privateKey),
    },
  };
  const intent = {
    v: 'icp-1.0',
    verb: 'purchase.create',
    intent_id: newId('icp_int'),
    buyer: buyerAid,
    merchant: merchantInfo.merchant_aid,
    settler: 'settler:stateset.usdc.base-sepolia',
    items: [{ sku: 'SKU-100', quantity: 50, unit_price: { amount: '88.57', currency: 'USDC' } }],
    max_total: { amount: '5000.00', currency: 'USDC' },
    expiry: exp.toISOString(),
    principal_binding: principalBinding,
    nonce: newNonceHex(),
    iat: now.toISOString(),
    exp: exp.toISOString(),
  };
  const intentCanonical = canonicalJson(intent);
  const buyerSig = signEd25519(intentCanonical, buyerEdKp.privateKey);

  const submitRes = await fetch(`http://127.0.0.1:${handlerPort}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      intent,
      signature: { alg: 'ed25519', kid: buyerAid, sig: buyerSig },
      _pubkey_hex: buyerEdPubRaw.toString('hex'),
      _x_pubkey_hex: buyerXPubRaw.toString('hex'),
      _principal_pubkey_hex: principalPubRaw.toString('hex'),
    }),
  });
  const submitted = await submitRes.json();
  if (submitRes.status !== 200) throw new Error(`submit failed: ${JSON.stringify(submitted)}`);
  json({
    requested: '50 × SKU-100',
    authority_ceiling: intent.max_total,
    quote_id: submitted.quote.quote_id,
    total: submitted.quote.total,
    merchant_signature_alg: submitted.signature.alg,
    merchant_signature_kid: submitted.signature.kid,
  });

  // -------------------------------------------------------------------
  h(5, 'Independently verify merchant Quote signature');
  para(`The buyer canonicalizes the Quote and verifies the merchant's signature using the public key from \`.well-known/icp\` — NOT trusting the handler's word about what it signed.`);
  const quoteCanonical = canonicalJson(submitted.quote);
  const merchantSigOk = verifyEd25519(quoteCanonical, submitted.signature.sig, merchantPubRaw);
  out(`Merchant Quote signature verified independently: **${merchantSigOk ? 'PASS ✓' : 'FAIL ✗'}**\n\n`);
  // Tamper test
  const tamperedQuote = canonicalJson({
    ...submitted.quote,
    total: { ...submitted.quote.total, amount: '46.50' },
  });
  const tamperedOk = verifyEd25519(tamperedQuote, submitted.signature.sig, merchantPubRaw);
  out(`Tampered Quote rejected by signature check: **${!tamperedOk ? 'PASS ✓' : 'FAIL ✗ (security bug!)'}**\n\n`);
  if (tamperedOk) throw new Error('tampered merchant quote passed signature verification');

  // -------------------------------------------------------------------
  h(6, 'Accept quote, reserve inventory, create order, and fund');
  const acceptRes = await fetch(
    `http://127.0.0.1:${handlerPort}/icp/v1/quotes/${submitted.quote.quote_id}/accept`,
    { method: 'POST', headers: { 'content-type': 'application/json' }, body: '{}' },
  );
  const accepted = await acceptRes.json();
  if (acceptRes.status !== 200) throw new Error(`accept failed: ${JSON.stringify(accepted)}`);
  const escrowId = accepted.funding.escrow_id;
  json({
    order: accepted.order,
    inventory_reservation: accepted.inventory_reservation,
    funding: {
      escrow_id: escrowId,
      rail: accepted.funding.chain,
      amount: accepted.funding.args.amount,
    },
  });
  para(`In production, the buyer's wallet would broadcast a \`fund\` transaction against the \`ICPEscrow.sol\` contract; the Settler daemon would observe it on Base Sepolia and sign the corresponding ICP EscrowEvent. For the demo we POST a mock \`fund\` event directly to the Settler — simulating what the chain watcher would do.`);
  const fundRes = await fetch(`http://127.0.0.1:${settlerPort}/admin/escrow/event`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      escrow_id: escrowId,
      kind: 'fund',
      init: { intent_id: intent.intent_id, amount: submitted.quote.total },
      rail_event: { rail: 'base-sepolia', block_number: 18342901, tx_hash: '0xdeadbeef' },
    }),
  });
  const { event: fundEvent } = await fundRes.json();
  json({
    seq: fundEvent.seq,
    from_state: fundEvent.from_state,
    to_state: fundEvent.to_state,
    trigger: fundEvent.trigger,
    settler_signature_kid: fundEvent.settler_signature.kid,
  });

  // -------------------------------------------------------------------
  h(7, 'Independently verify Settler EscrowEvent signature');
  para(`The buyer can verify the Settler's signature against the public key from the Settler's discovery document — again, no trust required.`);
  const { settler_signature, ...fundPayload } = fundEvent;
  const fundCanonical = canonicalJson(fundPayload);
  const settlerSigOk = verifyEd25519(fundCanonical, settler_signature.sig, settlerPubRaw);
  out(`Settler EscrowEvent signature verified independently: **${settlerSigOk ? 'PASS ✓' : 'FAIL ✗'}**\n\n`);
  const tamperedEvent = canonicalJson({ ...fundPayload, seq: 999 });
  const tamperedEventOk = verifyEd25519(tamperedEvent, settler_signature.sig, settlerPubRaw);
  out(`Tampered EscrowEvent rejected: **${!tamperedEventOk ? 'PASS ✓' : 'FAIL ✗ (security bug!)'}**\n\n`);

  // -------------------------------------------------------------------
  h(8, 'Walk the lifecycle');
  para(`Buyer drives the rest of the state machine via Settler mock injections (fulfill → release). At release the Settler signs a SettlementReceipt; the merchant verifies its configured Settler key and the signed Quote amount before adding the second signature.`);
  const ff = await fetch(`http://127.0.0.1:${settlerPort}/admin/escrow/event`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ escrow_id: escrowId, kind: 'fulfill', evidence_id: 'icp_ful_TWO_SIDED' }),
  });
  const { event: fulfillEvent } = await ff.json();

  const rr = await fetch(`http://127.0.0.1:${settlerPort}/admin/escrow/event`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      escrow_id: escrowId,
      kind: 'release',
      rail_event: { rail: 'base-sepolia', block_number: 18342919, tx_hash: '0x82' + 'ab'.repeat(31) },
    }),
  });
  const { event: releaseEvent, receipt: settlerReceipt } = await rr.json();

  const events = [fundEvent, fulfillEvent, releaseEvent];
  code('text', events.map((e) =>
    `  seq=${e.seq}  ${e.from_state} → ${e.to_state}  (${e.trigger.kind})  kid=${e.settler_signature.kid}`
  ).join('\n'));

  // Merchant verifies the configured Settler trust root and exact quoted
  // amount before adding its independent signature.
  const refusedCosign = await fetch(`http://127.0.0.1:${handlerPort}/icp/v1/settlements/cosign`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      receipt: { ...settlerReceipt, amount: { ...settlerReceipt.amount, amount: '46.50' } },
    }),
  });
  if (refusedCosign.status !== 401) {
    throw new Error(`merchant accepted a tampered Settler receipt (${refusedCosign.status})`);
  }
  const cosignRes = await fetch(`http://127.0.0.1:${handlerPort}/icp/v1/settlements/cosign`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ receipt: settlerReceipt }),
  });
  const cosigned = await cosignRes.json();
  if (cosignRes.status !== 200) throw new Error(`co-sign failed: ${JSON.stringify(cosigned)}`);
  const settlementReceipt = cosigned.receipt;

  // -------------------------------------------------------------------
  h(9, 'Two-sided settlement audit');
  para(`A regulator or auditor presented with the SettlementReceipt + the discovery documents from both entities can independently verify:`);
  const { merchant_signature, settler_signature: receiptSettlerSignature, ...receiptBody } = settlementReceipt;
  const receiptCanonical = canonicalJson(receiptBody);
  const receiptMerchantOk = verifyEd25519(receiptCanonical, merchant_signature.sig, merchantPubRaw);
  const receiptSettlerOk = verifyEd25519(receiptCanonical, receiptSettlerSignature.sig, settlerPubRaw);
  const tamperedReceiptOk = verifyEd25519(
    canonicalJson({ ...receiptBody, amount: { ...receiptBody.amount, amount: '46.50' } }),
    merchant_signature.sig,
    merchantPubRaw,
  );
  out(`- ${receiptMerchantOk ? '✓' : '✗'} Merchant SettlementReceipt signature independently verified\n`);
  out(`- ${receiptSettlerOk ? '✓' : '✗'} Settler SettlementReceipt signature independently verified\n`);
  out(`- ✓ Merchant refused to co-sign a tampered Settler receipt\n`);
  out(`- ${!tamperedReceiptOk ? '✓' : '✗'} Tampered amount rejected\n`);
  json({
    agent: buyerAid,
    principal: principalBinding.principal,
    intent: 'Purchase 50 × SKU-100 under 5000.00 USDC',
    decision: 'APPROVED',
    amount: settlementReceipt.amount,
    rail: settlementReceipt.rail,
    transaction: settlementReceipt.rail_txid,
    merchant_signature: merchant_signature.kid,
    settler_signature: receiptSettlerSignature.kid,
  });
  if (!receiptMerchantOk || !receiptSettlerOk || tamperedReceiptOk) {
    throw new Error('economic receipt verification failed');
  }
  para(`No need to trust the handler. No need to trust the Settler. **Only trust the public keys.** This is the load-bearing property the entire two-sided architecture rests on, and this demo just verified it from outside both servers.`);

  // -------------------------------------------------------------------
  h(10, 'What just happened');
  out(`Across 9 protocol steps the demo:

- Ran **2 servers** (handler + Settler) as separate processes on separate ports with separate signing keys
- Generated a fresh buyer Agent with its own keypair
- Submitted a delegated request for **50 × SKU-100 under 5,000 USDC** → received a signed Quote
- **Independently verified** the handler's merchant signature against \`merchant_pubkey\` published in \`.well-known/icp\` (and confirmed tampering is detected)
- Accepted the quote, reserved 50 units in the reference merchant state, and created an authorized order
- Injected mock chain events into the Settler daemon (simulating Base Sepolia \`ICPEscrow.sol\` event observation)
- Received signed EscrowEvents from the Settler with monotonic \`seq\` and Settler signature
- **Independently verified** the Settler's signature against \`signing_keys[0].pub_hex\` published in \`.well-known/icp-settler\` (and confirmed tampering is detected)
- Walked the full lifecycle: fund → fulfill → release
- Required the merchant to verify the configured Settler key and quoted amount before co-signing
- Independently verified both signatures on the same canonical Economic Receipt and rejected tampering

**Two separate processes. Two separate keys. Independent verifiability of every signature.** That's the trust model production ICP rests on. This demo just executed it end-to-end with cryptographic proof.

---

**Reproduce:**

\`\`\`sh
cd icp-spec/examples/03-two-sided-flow
node demo.mjs
\`\`\`

Runs in under 5 seconds. Zero installs.

`);

  writeFileSync(resolve(__dirname, 'transcript.md'), T.join(''));
  process.stderr.write(`\n✓ Transcript written to transcript.md (${T.join('').length} bytes)\n`);
} finally {
  if (handlerProc) handlerProc.kill();
  if (settlerProc) settlerProc.kill();
}

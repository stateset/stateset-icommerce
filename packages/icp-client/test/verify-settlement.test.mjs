// Tests for `verifySettlementReceipt` — Stripe-style co-signed receipt
// verifier. The receipt is signed by BOTH the merchant AND the Settler
// over the canonical bytes of the receipt body minus the two signature
// fields; both signatures must verify for the receipt to be considered
// final.
//
// Mirrors the handler's signing path in `src/server.mjs::handleFulfill`:
//   canonical = canonicalJson(receipt without signatures)
//   merchant_signature = sign(canonical, merchantKey)
//   settler_signature  = sign(canonical, settlerKey)
//   receipt.{merchant_signature, settler_signature} = those sigs

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createPrivateKey, createPublicKey } from 'node:crypto';

import {
  canonicalJson,
  signEd25519,
  verifySettlementReceipt,
  ICPError,
} from '../src/index.mjs';

function identityFromSeed(seed) {
  return { ed25519_seed: seed };
}

function pubkeyRaw(seed) {
  const der = Buffer.concat([
    Buffer.from('302e020100300506032b657004220420', 'hex'),
    seed,
  ]);
  const priv = createPrivateKey({ key: der, format: 'der', type: 'pkcs8' });
  const spki = createPublicKey(priv).export({ format: 'der', type: 'spki' });
  return spki.subarray(spki.length - 32);
}

const MERCHANT_SEED = Buffer.from('aa'.repeat(32), 'hex');
const SETTLER_SEED = Buffer.from('bb'.repeat(32), 'hex');
const merchantPub = pubkeyRaw(MERCHANT_SEED);
const settlerPub = pubkeyRaw(SETTLER_SEED);

function buildSignedReceipt(overrides = {}) {
  const unsigned = {
    type: 'icp.settlement.receipt',
    v: 'icp-1.0',
    settlement_id: 'icp_set_TEST',
    escrow_id: '0xabcdef',
    intent_id: 'icp_int_TEST',
    final_state: 'released',
    amount: { amount: '29.99', currency: 'USDC' },
    rail: 'demo-mock',
    rail_txid: '0xcafe',
    settled_at: '2026-05-12T18:00:00.000Z',
    released_to: '0xMerchantPayout',
    ...overrides,
  };
  const canonical = canonicalJson(unsigned);
  const merchantSig = signEd25519(canonical, identityFromSeed(MERCHANT_SEED));
  const settlerSig = signEd25519(canonical, identityFromSeed(SETTLER_SEED));
  return {
    ...unsigned,
    merchant_signature: { alg: 'ed25519', kid: 'aid:v1:zMerchant', sig: merchantSig },
    settler_signature: { alg: 'ed25519', kid: 'aid:v1:zSettler', sig: settlerSig },
  };
}

test('verifySettlementReceipt: happy path returns the receipt unchanged', () => {
  const receipt = buildSignedReceipt();
  const out = verifySettlementReceipt({
    receipt,
    merchantPubkeyRaw: merchantPub,
    settlerPubkeyRaw: settlerPub,
  });
  assert.equal(out, receipt);
  assert.equal(out.final_state, 'released');
});

test('verifySettlementReceipt: tampered amount → signature.invalid (merchant)', () => {
  const receipt = buildSignedReceipt();
  receipt.amount = { amount: '999.99', currency: 'USDC' };  // mutate post-sign
  assert.throws(
    () => verifySettlementReceipt({
      receipt,
      merchantPubkeyRaw: merchantPub,
      settlerPubkeyRaw: settlerPub,
    }),
    (e) => e instanceof ICPError && e.code === 'signature.invalid',
  );
});

test('verifySettlementReceipt: wrong settler pubkey → settlement.settler_signature_invalid', () => {
  const receipt = buildSignedReceipt();
  // Use a different keypair as the "Settler" expected pubkey — the
  // settler_signature was signed with SETTLER_SEED, so verification
  // against an unrelated key fails.
  const otherPub = pubkeyRaw(Buffer.from('cc'.repeat(32), 'hex'));
  assert.throws(
    () => verifySettlementReceipt({
      receipt,
      merchantPubkeyRaw: merchantPub,
      settlerPubkeyRaw: otherPub,
    }),
    (e) => e instanceof ICPError && e.code === 'settlement.settler_signature_invalid',
  );
});

test('verifySettlementReceipt: missing merchant_signature → format.missing_field', () => {
  const receipt = buildSignedReceipt();
  delete receipt.merchant_signature;
  assert.throws(
    () => verifySettlementReceipt({
      receipt,
      merchantPubkeyRaw: merchantPub,
      settlerPubkeyRaw: settlerPub,
    }),
    (e) => e instanceof ICPError && e.code === 'format.missing_field',
  );
});

test('verifySettlementReceipt: missing settler_signature → format.missing_field', () => {
  const receipt = buildSignedReceipt();
  delete receipt.settler_signature;
  assert.throws(
    () => verifySettlementReceipt({
      receipt,
      merchantPubkeyRaw: merchantPub,
      settlerPubkeyRaw: settlerPub,
    }),
    (e) => e instanceof ICPError && e.code === 'format.missing_field',
  );
});

test('verifySettlementReceipt: requireSettler=false skips the settler check', () => {
  const receipt = buildSignedReceipt();
  delete receipt.settler_signature;
  // Should NOT throw despite missing settler_signature.
  const out = verifySettlementReceipt({
    receipt,
    merchantPubkeyRaw: merchantPub,
    settlerPubkeyRaw: Buffer.alloc(32),  // ignored
    requireSettler: false,
  });
  assert.equal(out, receipt);
});

test('verifySettlementReceipt: both signatures cover the same canonical bytes', () => {
  // Build a receipt by hand to assert the merchant + settler signatures
  // are both over canonicalJson(unsigned).
  const receipt = buildSignedReceipt();
  const { merchant_signature, settler_signature, ...unsigned } = receipt;
  const canonical = canonicalJson(unsigned);
  // Re-sign with the known seeds and confirm equality.
  const expectedMerchant = signEd25519(canonical, identityFromSeed(MERCHANT_SEED));
  const expectedSettler = signEd25519(canonical, identityFromSeed(SETTLER_SEED));
  assert.equal(merchant_signature.sig, expectedMerchant);
  assert.equal(settler_signature.sig, expectedSettler);
});

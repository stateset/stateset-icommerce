const assert = require('assert');
const { test } = require('node:test');

const {
  Commerce,
  vesHybridGenerateSigningKeypair,
  vesHybridSignEventHash,
  vesStrictGenerateSigningKeypair,
  vesStrictSignEventHash,
  vesX402ComputeSigningHash,
} = require('../index.js');

function toHex(buffer) {
  return `0x${Buffer.from(buffer).toString('hex')}`;
}

function signingHashInputFromIntent(intent) {
  return {
    payerAddress: intent.payerAddress,
    payeeAddress: intent.payeeAddress,
    amount: intent.amount,
    asset: intent.asset,
    network: intent.network,
    chainId: intent.chainId,
    validUntil: intent.validUntil,
    nonce: intent.nonce,
    resourceUri: intent.resourceUri,
    resourceMethod: intent.resourceMethod,
  };
}

test('X402: signIntent accepts PQC-strict ML-DSA-65 signatures', async () => {
  const commerce = new Commerce(':memory:');
  const intent = await commerce.x402.createIntent({
    payerAddress: '0x1234567890abcdef1234567890abcdef12345678',
    payeeAddress: '0xabcdef1234567890abcdef1234567890abcdef12',
    amount: 1_000_000,
    asset: 'usdc',
    network: 'set_chain',
    signatureScheme: 'ml_dsa65',
    nonce: 7,
    resourceUri: '/strict',
    resourceMethod: 'POST',
  });

  const hash = vesX402ComputeSigningHash(signingHashInputFromIntent(intent));
  const keypair = vesStrictGenerateSigningKeypair();
  const signature = vesStrictSignEventHash(hash, keypair.mlDsa65Seed);

  const signed = await commerce.x402.signIntent(intent.id, {
    intentId: intent.id,
    signatureScheme: 'ml_dsa65',
    signature: '',
    publicKey: '',
    signatureBundle: { mlDsa65Signature: signature },
    publicKeyBundle: { mlDsa65PublicKey: keypair.mlDsa65PublicKey },
  });

  assert.strictEqual(signed.status, 'signed');
  assert.strictEqual(signed.payerSignatureScheme, 'ml_dsa65');
  assert.ok(!signed.payerSignature);
  assert.ok(!signed.payerPublicKey);
  assert.deepStrictEqual(
    Buffer.from(signed.payerSignatureBundle.mlDsa65Signature),
    Buffer.from(signature),
  );
  assert.deepStrictEqual(
    Buffer.from(signed.payerPublicKeyBundle.mlDsa65PublicKey),
    Buffer.from(keypair.mlDsa65PublicKey),
  );
});

test('X402: signIntent accepts hybrid Ed25519 + ML-DSA-65 signatures', async () => {
  const commerce = new Commerce(':memory:');
  const intent = await commerce.x402.createIntent({
    payerAddress: '0x1234567890abcdef1234567890abcdef12345678',
    payeeAddress: '0xabcdef1234567890abcdef1234567890abcdef12',
    amount: 2_000_000,
    asset: 'usdc',
    network: 'set_chain',
    signatureScheme: 'ed25519_ml_dsa65',
    nonce: 9,
    resourceUri: '/hybrid',
    resourceMethod: 'POST',
  });

  const hash = vesX402ComputeSigningHash(signingHashInputFromIntent(intent));
  const keypair = vesHybridGenerateSigningKeypair();
  const signature = vesHybridSignEventHash(hash, keypair.ed25519PrivateKey, keypair.mlDsa65Seed);

  const signed = await commerce.x402.signIntent(intent.id, {
    intentId: intent.id,
    signatureScheme: 'ed25519_ml_dsa65',
    signature: toHex(signature.ed25519Signature),
    publicKey: toHex(keypair.ed25519PublicKey),
    signatureBundle: { mlDsa65Signature: signature.mlDsa65Signature },
    publicKeyBundle: { mlDsa65PublicKey: keypair.mlDsa65PublicKey },
  });

  assert.strictEqual(signed.status, 'signed');
  assert.strictEqual(signed.payerSignatureScheme, 'ed25519_ml_dsa65');
  assert.strictEqual(signed.payerSignature, toHex(signature.ed25519Signature));
  assert.strictEqual(signed.payerPublicKey, toHex(keypair.ed25519PublicKey));
  assert.deepStrictEqual(
    Buffer.from(signed.payerSignatureBundle.mlDsa65Signature),
    Buffer.from(signature.mlDsa65Signature),
  );
  assert.deepStrictEqual(
    Buffer.from(signed.payerPublicKeyBundle.mlDsa65PublicKey),
    Buffer.from(keypair.mlDsa65PublicKey),
  );
});

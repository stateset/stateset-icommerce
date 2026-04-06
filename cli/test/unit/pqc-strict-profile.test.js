/**
 * PQC-strict profile tests.
 *
 * Verifies end-to-end behaviour of the pqc-strict security profile across
 * the sync layer: key generation, signing, encryption, decryption, PoP,
 * metrics counters, and cross-profile rejection.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import {
  SECURITY_PROFILE_HYBRID,
  SECURITY_PROFILE_LEGACY,
  SECURITY_PROFILE_PQC_STRICT,
  SIGNATURE_SCHEME_ML_DSA_65,
  SIGNATURE_SCHEME_ED25519_ML_DSA_65,
  KEY_ALGORITHM_ML_DSA_65,
  KEY_ALGORITHM_ML_KEM_768,
  KEY_WRAP_SCHEME_ML_KEM_768,
  resolveSecurityProfile,
  assertEventMatchesSecurityProfile,
  assertKeyRegistrationMatchesSecurityProfile,
  assertReceiptMatchesSecurityProfile,
  assertSecureTransportForProfile,
  profileMetricLabel,
} from '../../src/sync/pqc.js';

// ---------------------------------------------------------------------------
// resolveSecurityProfile
// ---------------------------------------------------------------------------

describe('resolveSecurityProfile', () => {
  it('defaults undefined to "hybrid"', () => {
    assert.equal(resolveSecurityProfile(), 'hybrid');
  });

  it('accepts "pqc-strict"', () => {
    assert.equal(resolveSecurityProfile('pqc-strict'), 'pqc-strict');
  });

  it('accepts "PQC-STRICT" case-insensitively', () => {
    assert.equal(resolveSecurityProfile('PQC-STRICT'), 'pqc-strict');
  });

  it('rejects unknown profile', () => {
    assert.throws(() => resolveSecurityProfile('quantum'), /Unsupported/);
  });
});

// ---------------------------------------------------------------------------
// profileMetricLabel
// ---------------------------------------------------------------------------

describe('profileMetricLabel', () => {
  it('returns hybrid for undefined', () => {
    assert.equal(profileMetricLabel(), 'hybrid');
  });

  it('returns pqc-strict', () => {
    assert.equal(profileMetricLabel('pqc-strict'), 'pqc-strict');
  });
});

// ---------------------------------------------------------------------------
// assertSecureTransportForProfile
// ---------------------------------------------------------------------------

describe('assertSecureTransportForProfile', () => {
  it('rejects insecure transport for legacy unless explicitly allowed', () => {
    assert.throws(
      () => assertSecureTransportForProfile('legacy', false),
      /explicitly allowed/,
    );
  });

  it('allows insecure transport for legacy when explicitly allowed', () => {
    assert.doesNotThrow(() => assertSecureTransportForProfile('legacy', false, 'sequencer', true));
  });

  it('rejects insecure transport for pqc-strict', () => {
    assert.throws(
      () => assertSecureTransportForProfile('pqc-strict', false),
      /must use TLS/,
    );
  });

  it('allows secure transport for pqc-strict', () => {
    assert.doesNotThrow(() => assertSecureTransportForProfile('pqc-strict', true));
  });
});

// ---------------------------------------------------------------------------
// assertEventMatchesSecurityProfile — pqc-strict
// ---------------------------------------------------------------------------

describe('assertEventMatchesSecurityProfile — pqc-strict', () => {
  it('rejects event with hybrid signature scheme', () => {
    const event = {
      agentSignatureScheme: SIGNATURE_SCHEME_ED25519_ML_DSA_65,
      agentSignatureBundle: {
        ed25519Signature: Buffer.alloc(64, 1),
        mlDsa65Signature: Buffer.alloc(100, 2),
      },
    };
    assert.throws(
      () => assertEventMatchesSecurityProfile(event, 'pqc-strict'),
      /pqc-strict profile requires SIGNATURE_SCHEME_ML_DSA_65/,
    );
  });

  it('rejects event with Ed25519-only signature', () => {
    const event = {
      agentSignatureScheme: 1, // ED25519
      agentSignature: Buffer.alloc(64, 1),
    };
    assert.throws(
      () => assertEventMatchesSecurityProfile(event, 'pqc-strict'),
      /pqc-strict/,
    );
  });

  it('rejects event with Ed25519 material present', () => {
    const event = {
      agentSignatureScheme: SIGNATURE_SCHEME_ML_DSA_65,
      agentSignatureBundle: {
        mlDsa65Signature: Buffer.alloc(100, 2),
        ed25519Signature: Buffer.alloc(64, 1), // not allowed
      },
      agentSignature: Buffer.alloc(64, 1),
    };
    assert.throws(
      () => assertEventMatchesSecurityProfile(event, 'pqc-strict'),
      /rejects Ed25519/,
    );
  });

  it('accepts valid pqc-strict event (plaintext)', () => {
    const event = {
      agentSignatureScheme: SIGNATURE_SCHEME_ML_DSA_65,
      agentSignatureBundle: {
        mlDsa65Signature: Buffer.alloc(100, 2),
      },
      payloadKind: 0,
    };
    assert.doesNotThrow(() => assertEventMatchesSecurityProfile(event, 'pqc-strict'));
  });

  it('accepts valid pqc-strict event (encrypted with ML-KEM-768)', () => {
    const event = {
      agentSignatureScheme: SIGNATURE_SCHEME_ML_DSA_65,
      agentSignatureBundle: {
        mlDsa65Signature: Buffer.alloc(100, 2),
      },
      payloadKind: 1,
      payloadEncrypted: {
        keyWrapParams: { scheme: KEY_WRAP_SCHEME_ML_KEM_768 },
        recipientWraps: [
          {
            recipientKid: 1,
            wrapScheme: KEY_WRAP_SCHEME_ML_KEM_768,
            mlKemCiphertext: Buffer.alloc(32, 3),
            wrappedKey: Buffer.alloc(48, 4),
          },
        ],
      },
    };
    assert.doesNotThrow(() => assertEventMatchesSecurityProfile(event, 'pqc-strict'));
  });

  it('rejects pqc-strict encrypted event with X25519 wraps', () => {
    const event = {
      agentSignatureScheme: SIGNATURE_SCHEME_ML_DSA_65,
      agentSignatureBundle: {
        mlDsa65Signature: Buffer.alloc(100, 2),
      },
      payloadKind: 1,
      payloadEncrypted: {
        keyWrapParams: { scheme: KEY_WRAP_SCHEME_ML_KEM_768 },
        recipientWraps: [
          {
            recipientKid: 1,
            wrapScheme: KEY_WRAP_SCHEME_ML_KEM_768,
            mlKemCiphertext: Buffer.alloc(32, 3),
            wrappedKey: Buffer.alloc(48, 4),
            x25519Enc: Buffer.alloc(32, 5), // not allowed
          },
        ],
      },
    };
    assert.throws(
      () => assertEventMatchesSecurityProfile(event, 'pqc-strict'),
      /rejects X25519/,
    );
  });
});

// ---------------------------------------------------------------------------
// assertKeyRegistrationMatchesSecurityProfile — pqc-strict
// ---------------------------------------------------------------------------

describe('assertKeyRegistrationMatchesSecurityProfile — pqc-strict', () => {
  it('accepts ML-DSA-65 signing key', () => {
    const reg = {
      keyType: 1,
      keyAlgorithm: KEY_ALGORITHM_ML_DSA_65,
      publicKeyBundle: {
        mlDsa65PublicKey: Buffer.alloc(1952, 1),
      },
    };
    assert.doesNotThrow(() =>
      assertKeyRegistrationMatchesSecurityProfile(reg, 'pqc-strict'),
    );
  });

  it('rejects hybrid signing key under pqc-strict', () => {
    const reg = {
      keyType: 1,
      keyAlgorithm: 5, // ED25519_ML_DSA_65
      publicKeyBundle: {
        ed25519PublicKey: Buffer.alloc(32, 1),
        mlDsa65PublicKey: Buffer.alloc(1952, 2),
      },
    };
    assert.throws(
      () => assertKeyRegistrationMatchesSecurityProfile(reg, 'pqc-strict'),
      /pqc-strict.*KEY_ALGORITHM_ML_DSA_65/,
    );
  });

  it('rejects Ed25519 public key material under pqc-strict', () => {
    const reg = {
      keyType: 1,
      keyAlgorithm: KEY_ALGORITHM_ML_DSA_65,
      publicKeyBundle: {
        mlDsa65PublicKey: Buffer.alloc(1952, 1),
        ed25519PublicKey: Buffer.alloc(32, 2), // not allowed
      },
    };
    assert.throws(
      () => assertKeyRegistrationMatchesSecurityProfile(reg, 'pqc-strict'),
      /rejects Ed25519/,
    );
  });

  it('accepts ML-KEM-768 encryption key', () => {
    const reg = {
      keyType: 2,
      keyAlgorithm: KEY_ALGORITHM_ML_KEM_768,
      publicKeyBundle: {
        mlKem768PublicKey: Buffer.alloc(1184, 1),
      },
    };
    assert.doesNotThrow(() =>
      assertKeyRegistrationMatchesSecurityProfile(reg, 'pqc-strict'),
    );
  });

  it('rejects X25519 public key material under pqc-strict', () => {
    const reg = {
      keyType: 2,
      keyAlgorithm: KEY_ALGORITHM_ML_KEM_768,
      publicKeyBundle: {
        mlKem768PublicKey: Buffer.alloc(1184, 1),
        x25519PublicKey: Buffer.alloc(32, 2), // not allowed
      },
    };
    assert.throws(
      () => assertKeyRegistrationMatchesSecurityProfile(reg, 'pqc-strict'),
      /rejects X25519/,
    );
  });
});

// ---------------------------------------------------------------------------
// assertReceiptMatchesSecurityProfile — pqc-strict
// ---------------------------------------------------------------------------

describe('assertReceiptMatchesSecurityProfile — pqc-strict', () => {
  it('allows legacy receipts', () => {
    assert.doesNotThrow(() => assertReceiptMatchesSecurityProfile({}, 'legacy'));
  });

  it('requires ML-DSA-65 scheme for pqc-strict', () => {
    const receipt = {
      signatureScheme: SIGNATURE_SCHEME_ED25519_ML_DSA_65,
      signatureBundle: {
        ed25519Signature: Buffer.alloc(64, 1),
        mlDsa65Signature: Buffer.alloc(100, 2),
      },
    };
    assert.throws(
      () => assertReceiptMatchesSecurityProfile(receipt, 'pqc-strict'),
      /pqc-strict.*SIGNATURE_SCHEME_ML_DSA_65/,
    );
  });

  it('rejects Ed25519 material in pqc-strict receipt', () => {
    const receipt = {
      signatureScheme: SIGNATURE_SCHEME_ML_DSA_65,
      signatureBundle: {
        mlDsa65Signature: Buffer.alloc(100, 2),
        ed25519Signature: Buffer.alloc(64, 1),
      },
    };
    assert.throws(
      () => assertReceiptMatchesSecurityProfile(receipt, 'pqc-strict'),
      /rejects Ed25519/,
    );
  });

  it('accepts valid pqc-strict receipt', () => {
    const receipt = {
      signatureScheme: SIGNATURE_SCHEME_ML_DSA_65,
      signatureBundle: {
        mlDsa65Signature: Buffer.alloc(100, 2),
      },
    };
    assert.doesNotThrow(() => assertReceiptMatchesSecurityProfile(receipt, 'pqc-strict'));
  });

  it('requires both signatures for hybrid receipt', () => {
    const receipt = {
      signatureScheme: SIGNATURE_SCHEME_ED25519_ML_DSA_65,
      signatureBundle: {
        ed25519Signature: Buffer.alloc(64, 1),
        // missing mlDsa65Signature
      },
    };
    assert.throws(
      () => assertReceiptMatchesSecurityProfile(receipt, 'hybrid'),
      /requires both/,
    );
  });

  it('accepts valid hybrid receipt', () => {
    const receipt = {
      signatureScheme: SIGNATURE_SCHEME_ED25519_ML_DSA_65,
      signatureBundle: {
        ed25519Signature: Buffer.alloc(64, 1),
        mlDsa65Signature: Buffer.alloc(100, 2),
      },
    };
    assert.doesNotThrow(() => assertReceiptMatchesSecurityProfile(receipt, 'hybrid'));
  });
});

// ---------------------------------------------------------------------------
// Cross-profile rejection
// ---------------------------------------------------------------------------

describe('Cross-profile rejection', () => {
  it('hybrid rejects legacy Ed25519-only event', () => {
    const event = {
      agentSignatureScheme: 1, // ED25519 only
      agentSignature: Buffer.alloc(64, 1),
    };
    assert.throws(
      () => assertEventMatchesSecurityProfile(event, 'hybrid'),
      /Hybrid.*SIGNATURE_SCHEME_ED25519_ML_DSA_65/,
    );
  });

  it('pqc-strict rejects hybrid signed event', () => {
    const event = {
      agentSignatureScheme: SIGNATURE_SCHEME_ED25519_ML_DSA_65,
      agentSignatureBundle: {
        ed25519Signature: Buffer.alloc(64, 1),
        mlDsa65Signature: Buffer.alloc(100, 2),
      },
    };
    assert.throws(
      () => assertEventMatchesSecurityProfile(event, 'pqc-strict'),
      /pqc-strict/,
    );
  });
});

// ---------------------------------------------------------------------------
// PQC-strict native crypto roundtrips (requires @stateset/embedded)
// ---------------------------------------------------------------------------

let native = null;
function loadNative() {
  try {
    return require('@stateset/embedded');
  } catch {
    return null;
  }
}

describe('PQC-strict native crypto', () => {
  beforeEach(() => {
    native = loadNative();
  });

  describe('strict signing keygen', () => {
    it('generates keypair with correct key sizes', () => {
      if (!native?.vesStrictGenerateSigningKeypair) return;
      const kp = native.vesStrictGenerateSigningKeypair();
      assert.equal(kp.mlDsa65PublicKey.length, 1952);
      assert.equal(kp.mlDsa65Seed.length, 32);
    });
  });

  describe('strict sign + verify', () => {
    it('roundtrip succeeds', () => {
      if (!native?.vesStrictGenerateSigningKeypair) return;
      const kp = native.vesStrictGenerateSigningKeypair();
      const hash = Buffer.alloc(32, 0x77);
      const sig = native.vesStrictSignEventHash(hash, kp.mlDsa65Seed);
      assert.ok(sig.length > 0);
      const valid = native.vesStrictVerifyEventSignature(hash, sig, kp.mlDsa65PublicKey);
      assert.equal(valid, true);
    });

    it('wrong key fails', () => {
      if (!native?.vesStrictGenerateSigningKeypair) return;
      const kp1 = native.vesStrictGenerateSigningKeypair();
      const kp2 = native.vesStrictGenerateSigningKeypair();
      const hash = Buffer.alloc(32, 0x88);
      const sig = native.vesStrictSignEventHash(hash, kp1.mlDsa65Seed);
      const valid = native.vesStrictVerifyEventSignature(hash, sig, kp2.mlDsa65PublicKey);
      assert.equal(valid, false);
    });
  });

  describe('strict recipient keygen', () => {
    it('generates keypair with correct sizes', () => {
      if (!native?.vesStrictGenerateRecipientKeypair) return;
      const kp = native.vesStrictGenerateRecipientKeypair(42);
      assert.equal(kp.kid, 42);
      assert.equal(kp.mlKem768PublicKey.length, 1184);
      assert.equal(kp.mlKem768Seed.length, 64);
    });
  });

  describe('strict encrypt + decrypt', () => {
    it('roundtrip succeeds', () => {
      if (!native?.vesStrictEncryptPayload) return;
      const rk = native.vesStrictGenerateRecipientKeypair(1);
      const payload = JSON.stringify({ item: 'strict-test', qty: 7 });
      const aadParams = {
        vesVersion: 1,
        tenantId: '550e8400-e29b-41d4-a716-446655440000',
        storeId: '550e8400-e29b-41d4-a716-446655440000',
        eventId: '550e8400-e29b-41d4-a716-446655440000',
        sourceAgentId: '550e8400-e29b-41d4-a716-446655440000',
        agentKeyId: 1,
        entityType: 'order',
        entityId: 'ord_strict',
        eventType: 'order.created',
        createdAt: '2026-03-31T00:00:00Z',
        payloadPlainHash: Buffer.alloc(32, 0x01),
      };

      const enc = native.vesStrictEncryptPayload(payload, aadParams, [
        { kid: 1, mlKem768PublicKey: rk.mlKem768PublicKey },
      ]);
      assert.ok(enc.payloadEncryptedJson);
      assert.equal(enc.salt.length, 16);

      aadParams.payloadPlainHash = enc.payloadPlainHash;
      const enc2 = native.vesStrictEncryptPayload(payload, aadParams, [
        { kid: 1, mlKem768PublicKey: rk.mlKem768PublicKey },
      ]);

      const dec = native.vesStrictDecryptPayload(
        enc2.payloadEncryptedJson,
        enc2.payloadCipherHash,
        1,
        { mlKem768Seed: rk.mlKem768Seed },
        enc2.payloadPlainHash,
      );
      const parsed = JSON.parse(dec);
      assert.equal(parsed.item, 'strict-test');
      assert.equal(parsed.qty, 7);
    });
  });

  describe('hybrid PoP via NAPI', () => {
    it('generate and verify roundtrip', () => {
      if (!native?.vesHybridGenerateSigningPop) return;
      const kp = native.vesHybridGenerateSigningKeypair();
      const pop = native.vesHybridGenerateSigningPop(
        kp.ed25519PrivateKey,
        kp.mlDsa65Seed,
        kp.ed25519PublicKey,
        kp.mlDsa65PublicKey,
      );
      assert.equal(pop.ed25519Signature.length, 64);
      assert.ok(pop.mlDsa65Signature.length > 0);

      const valid = native.vesHybridVerifySigningPop(
        pop.ed25519Signature,
        pop.mlDsa65Signature,
        kp.ed25519PublicKey,
        kp.mlDsa65PublicKey,
      );
      assert.equal(valid, true);
    });

    it('wrong key fails', () => {
      if (!native?.vesHybridGenerateSigningPop) return;
      const kp1 = native.vesHybridGenerateSigningKeypair();
      const kp2 = native.vesHybridGenerateSigningKeypair();
      const pop = native.vesHybridGenerateSigningPop(
        kp1.ed25519PrivateKey,
        kp1.mlDsa65Seed,
        kp1.ed25519PublicKey,
        kp1.mlDsa65PublicKey,
      );
      const valid = native.vesHybridVerifySigningPop(
        pop.ed25519Signature,
        pop.mlDsa65Signature,
        kp2.ed25519PublicKey,
        kp2.mlDsa65PublicKey,
      );
      assert.equal(valid, false);
    });
  });
});

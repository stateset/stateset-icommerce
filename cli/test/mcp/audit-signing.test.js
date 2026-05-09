// Unit tests for cli/src/mcp/audit-signing.js
//
// Covers `signAuditArtifact`:
//  - Signed path: HMAC-SHA256 with the supplied key + keyId
//  - Unsigned path: deterministic SHA-256 marker, signed=false
//  - payloadHash always present + matches sha256(stableStringify(payload))
//  - Same input → same output (deterministic across invocations)
//  - Different keys produce different signatures for the same payload
//  - Default opts: empty signingKey + 'stateset-default' keyId

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { createHmac, createHash } from 'node:crypto';

import { signAuditArtifact } from '../../src/mcp/audit-signing.js';
import { sha256, stableStringify } from '../../src/mcp/replay-sanitizer.js';

describe('signAuditArtifact', () => {
  describe('signed path', () => {
    it('produces an HMAC-SHA256 signature when signingKey is non-empty', () => {
      const payload = { tool: 'create_order', orderId: 'ord_1' };
      const sig = signAuditArtifact(payload, {
        signingKey: 'super-secret-key',
        keyId: 'k-prod-1',
      });

      assert.equal(sig.algorithm, 'hmac-sha256');
      assert.equal(sig.signed, true);
      assert.equal(sig.keyId, 'k-prod-1');

      // Cross-verify the HMAC matches what we compute independently.
      const canonical = stableStringify(payload);
      const expected = createHmac('sha256', 'super-secret-key')
        .update(canonical)
        .digest('hex');
      assert.equal(sig.signature, expected);
    });

    it('uses default keyId "stateset-default" when not supplied', () => {
      const sig = signAuditArtifact({ x: 1 }, { signingKey: 'k' });
      assert.equal(sig.keyId, 'stateset-default');
    });

    it('different signing keys → different signatures for the same payload', () => {
      const payload = { tool: 'ship_order' };
      const a = signAuditArtifact(payload, { signingKey: 'key-a' });
      const b = signAuditArtifact(payload, { signingKey: 'key-b' });
      assert.notEqual(a.signature, b.signature);
      // payloadHash is purely a function of the canonical payload, so it
      // matches across keys (the hash is not key-dependent).
      assert.equal(a.payloadHash, b.payloadHash);
    });
  });

  describe('unsigned path', () => {
    it('falls back to deterministic SHA-256 when signingKey is empty', () => {
      const payload = { x: 1 };
      const sig = signAuditArtifact(payload, { signingKey: '' });

      assert.equal(sig.algorithm, 'sha256');
      assert.equal(sig.signed, false);
      assert.equal(sig.keyId, 'unsigned-deterministic');

      // The unsigned signature is `sha256("unsigned:" + payloadHash)`.
      const expected = sha256(`unsigned:${sig.payloadHash}`);
      assert.equal(sig.signature, expected);
    });

    it('falls back to unsigned when opts is omitted entirely', () => {
      const sig = signAuditArtifact({ x: 1 });
      assert.equal(sig.signed, false);
      assert.equal(sig.algorithm, 'sha256');
    });

    it('falls back to unsigned when signingKey is undefined', () => {
      const sig = signAuditArtifact({ x: 1 }, { keyId: 'ignored' });
      assert.equal(sig.signed, false);
      // Note: keyId from opts is intentionally NOT used on the unsigned
      // path — it's always reported as 'unsigned-deterministic' so
      // auditors can't be fooled by a fake keyId on an unsigned artifact.
      assert.equal(sig.keyId, 'unsigned-deterministic');
    });

    it('ignores keyId on the unsigned path even when explicitly passed', () => {
      const sig = signAuditArtifact(
        { x: 1 },
        { signingKey: '', keyId: 'fake-prod-key' },
      );
      assert.equal(sig.keyId, 'unsigned-deterministic');
    });
  });

  describe('payloadHash invariants', () => {
    it('is sha256 of stableStringify(payload), regardless of signing path', () => {
      const payload = { b: 2, a: 1 }; // unsorted
      const expected = sha256(stableStringify(payload));

      const signed = signAuditArtifact(payload, { signingKey: 'k' });
      const unsigned = signAuditArtifact(payload);
      assert.equal(signed.payloadHash, expected);
      assert.equal(unsigned.payloadHash, expected);
    });

    it('produces the same payloadHash for canonically-equivalent inputs', () => {
      // stableStringify sorts keys, so insertion order shouldn't matter.
      const a = signAuditArtifact({ b: 2, a: 1 });
      const b = signAuditArtifact({ a: 1, b: 2 });
      assert.equal(a.payloadHash, b.payloadHash);
    });

    it('returns a 64-char hex string for payloadHash', () => {
      const sig = signAuditArtifact({ x: 1 });
      assert.match(sig.payloadHash, /^[0-9a-f]{64}$/);
    });
  });

  describe('determinism', () => {
    it('same input + same key → same output across calls', () => {
      const payload = { tool: 'create_order' };
      const a = signAuditArtifact(payload, { signingKey: 'k', keyId: 'k1' });
      const b = signAuditArtifact(payload, { signingKey: 'k', keyId: 'k1' });
      assert.deepEqual(a, b);
    });

    it('unsigned path is deterministic too (no random component)', () => {
      const payload = { tool: 'create_order' };
      const a = signAuditArtifact(payload);
      const b = signAuditArtifact(payload);
      assert.deepEqual(a, b);
    });

    it('handles edge inputs (null, empty object, primitive)', () => {
      // Smoke: should not throw on any JSON-serializable input.
      assert.doesNotThrow(() => signAuditArtifact(null));
      assert.doesNotThrow(() => signAuditArtifact({}));
      assert.doesNotThrow(() => signAuditArtifact('a string'));
      assert.doesNotThrow(() => signAuditArtifact(42));
      assert.doesNotThrow(() => signAuditArtifact([1, 2, 3]));
    });
  });

  describe('cross-binding sanity', () => {
    it('payloadHash matches a manually-computed sha256(JCS-like serialization)', () => {
      const payload = { b: 'two', a: 'one' };
      const canonical = stableStringify(payload);
      const expected = createHash('sha256').update(canonical).digest('hex');

      const sig = signAuditArtifact(payload);
      assert.equal(sig.payloadHash, expected);
    });
  });
});

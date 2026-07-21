/**
 * Tests for the Verifiable Commerce Proof Generator
 *
 * Covers: Merkle proof generation/verification, inclusion proofs,
 * receipt bundles, batch summaries, compliance packages, and event
 * verification — using the real VES crypto module.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';

import * as cryptoMod from '../../src/sync/crypto.js';
import { createProofGenerator } from '../../src/sync/proof-generator.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Build a minimal test event.
 * The `eventSigningHash` is a deterministic SHA-256 so tests are reproducible.
 */
function makeEvent(index, payload = { idx: index }) {
  const id = `evt-${index}`;
  const payloadHash = cryptoMod.computePayloadPlainHash(payload).toString('hex');
  const eventSigningHash = crypto.createHash('sha256').update(`event-${index}`).digest();
  return {
    id,
    index,
    payload,
    payloadHash,
    eventSigningHash,
    timestamp: new Date(1700000000000 + index * 1000).toISOString(),
  };
}

function makeEvents(n) {
  return Array.from({ length: n }, (_, i) => makeEvent(i));
}

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

describe('proof-generator', () => {
  /** @type {ReturnType<typeof createProofGenerator>} */
  let pg;

  beforeEach(() => {
    pg = createProofGenerator(cryptoMod);
  });

  // =========================================================================
  // 1. Merkle proof generation (15 tests)
  // =========================================================================
  describe('buildMerkleProof', () => {
    it('builds proof for a single leaf (empty proof)', () => {
      const leaf = crypto.randomBytes(32);
      const { proof, root } = pg.buildMerkleProof([leaf], 0);
      assert.equal(proof.length, 0);
      assert.ok(root.equals(leaf));
    });

    it('builds proof for 2 leaves — index 0', () => {
      const leaves = [crypto.randomBytes(32), crypto.randomBytes(32)];
      const { proof, root } = pg.buildMerkleProof(leaves, 0);
      assert.equal(proof.length, 1);
      assert.equal(proof[0].position, 'right');
      const expected = cryptoMod.computeNodeHash(leaves[0], leaves[1]);
      assert.ok(root.equals(expected));
    });

    it('builds proof for 2 leaves — index 1', () => {
      const leaves = [crypto.randomBytes(32), crypto.randomBytes(32)];
      const { proof, root } = pg.buildMerkleProof(leaves, 1);
      assert.equal(proof.length, 1);
      assert.equal(proof[0].position, 'left');
    });

    it('builds proof for 4 leaves', () => {
      const leaves = Array.from({ length: 4 }, () => crypto.randomBytes(32));
      const { proof } = pg.buildMerkleProof(leaves, 2);
      // log2(4) = 2 levels
      assert.equal(proof.length, 2);
    });

    it('builds proof for 8 leaves', () => {
      const leaves = Array.from({ length: 8 }, () => crypto.randomBytes(32));
      const { proof } = pg.buildMerkleProof(leaves, 5);
      // log2(8) = 3 levels
      assert.equal(proof.length, 3);
    });

    it('pads 3 leaves to 4 (next power of 2)', () => {
      const leaves = Array.from({ length: 3 }, () => crypto.randomBytes(32));
      const { proof } = pg.buildMerkleProof(leaves, 0);
      assert.equal(proof.length, 2); // log2(4) = 2
    });

    it('pads 5 leaves to 8', () => {
      const leaves = Array.from({ length: 5 }, () => crypto.randomBytes(32));
      const { proof } = pg.buildMerkleProof(leaves, 3);
      assert.equal(proof.length, 3); // log2(8) = 3
    });

    it('pads 7 leaves to 8', () => {
      const leaves = Array.from({ length: 7 }, () => crypto.randomBytes(32));
      const { proof } = pg.buildMerkleProof(leaves, 6);
      assert.equal(proof.length, 3);
    });

    it('computes correct root for 4 leaves', () => {
      const leaves = Array.from({ length: 4 }, () => crypto.randomBytes(32));
      const { root: r0 } = pg.buildMerkleProof(leaves, 0);
      const { root: r1 } = pg.buildMerkleProof(leaves, 1);
      const { root: r2 } = pg.buildMerkleProof(leaves, 2);
      const { root: r3 } = pg.buildMerkleProof(leaves, 3);
      // All target indices must yield the same root
      assert.ok(r0.equals(r1));
      assert.ok(r1.equals(r2));
      assert.ok(r2.equals(r3));
    });

    it('proof path differs for different target indices', () => {
      const leaves = Array.from({ length: 4 }, () => crypto.randomBytes(32));
      const { proof: p0 } = pg.buildMerkleProof(leaves, 0);
      const { proof: p3 } = pg.buildMerkleProof(leaves, 3);
      // The first sibling hash should differ
      assert.notEqual(p0[0].hash, p3[0].hash);
    });

    it('proof for first element has all "right" at leaf level', () => {
      const leaves = Array.from({ length: 4 }, () => crypto.randomBytes(32));
      const { proof } = pg.buildMerkleProof(leaves, 0);
      assert.equal(proof[0].position, 'right');
    });

    it('proof for last element in power-of-2 set has "left" at leaf level', () => {
      const leaves = Array.from({ length: 4 }, () => crypto.randomBytes(32));
      const { proof } = pg.buildMerkleProof(leaves, 3);
      assert.equal(proof[0].position, 'left');
    });

    it('throws for empty leaves', () => {
      assert.throws(() => pg.buildMerkleProof([], 0), /empty/i);
    });

    it('throws for targetIndex out of range (too large)', () => {
      const leaves = [crypto.randomBytes(32)];
      assert.throws(() => pg.buildMerkleProof(leaves, 1), /out of range/i);
    });

    it('throws for negative targetIndex', () => {
      const leaves = [crypto.randomBytes(32)];
      assert.throws(() => pg.buildMerkleProof(leaves, -1), /out of range/i);
    });
  });

  // =========================================================================
  // 2. Merkle proof verification (15 tests)
  // =========================================================================
  describe('verifyMerkleProof', () => {
    it('returns true for a valid proof (2 leaves)', () => {
      const leaves = [crypto.randomBytes(32), crypto.randomBytes(32)];
      const { proof, root } = pg.buildMerkleProof(leaves, 0);
      assert.ok(pg.verifyMerkleProof(leaves[0], proof, root));
    });

    it('returns true for a valid proof (4 leaves, index 2)', () => {
      const leaves = Array.from({ length: 4 }, () => crypto.randomBytes(32));
      const { proof, root } = pg.buildMerkleProof(leaves, 2);
      assert.ok(pg.verifyMerkleProof(leaves[2], proof, root));
    });

    it('returns true for a valid proof (8 leaves, index 7)', () => {
      const leaves = Array.from({ length: 8 }, () => crypto.randomBytes(32));
      const { proof, root } = pg.buildMerkleProof(leaves, 7);
      assert.ok(pg.verifyMerkleProof(leaves[7], proof, root));
    });

    it('returns false for tampered leaf hash', () => {
      const leaves = [crypto.randomBytes(32), crypto.randomBytes(32)];
      const { proof, root } = pg.buildMerkleProof(leaves, 0);
      const tampered = crypto.randomBytes(32);
      assert.equal(pg.verifyMerkleProof(tampered, proof, root), false);
    });

    it('returns false for tampered sibling hash', () => {
      const leaves = Array.from({ length: 4 }, () => crypto.randomBytes(32));
      const { proof, root } = pg.buildMerkleProof(leaves, 1);
      const tamperedProof = proof.map((s) => ({ ...s }));
      tamperedProof[0].hash = crypto.randomBytes(32).toString('hex');
      assert.equal(pg.verifyMerkleProof(leaves[1], tamperedProof, root), false);
    });

    it('returns false for wrong root', () => {
      const leaves = [crypto.randomBytes(32), crypto.randomBytes(32)];
      const { proof } = pg.buildMerkleProof(leaves, 0);
      const wrongRoot = crypto.randomBytes(32);
      assert.equal(pg.verifyMerkleProof(leaves[0], proof, wrongRoot), false);
    });

    it('returns true for single leaf (empty proof)', () => {
      const leaf = crypto.randomBytes(32);
      const { proof, root } = pg.buildMerkleProof([leaf], 0);
      assert.ok(pg.verifyMerkleProof(leaf, proof, root));
    });

    it('works with hex-string inputs', () => {
      const leaves = [crypto.randomBytes(32), crypto.randomBytes(32)];
      const { proof, root } = pg.buildMerkleProof(leaves, 0);
      assert.ok(pg.verifyMerkleProof(leaves[0].toString('hex'), proof, root.toString('hex')));
    });

    it('round-trip for 3 leaves (non-power-of-2)', () => {
      const leaves = Array.from({ length: 3 }, () => crypto.randomBytes(32));
      for (let i = 0; i < 3; i++) {
        const { proof, root } = pg.buildMerkleProof(leaves, i);
        assert.ok(pg.verifyMerkleProof(leaves[i], proof, root));
      }
    });

    it('round-trip for 5 leaves', () => {
      const leaves = Array.from({ length: 5 }, () => crypto.randomBytes(32));
      for (let i = 0; i < 5; i++) {
        const { proof, root } = pg.buildMerkleProof(leaves, i);
        assert.ok(pg.verifyMerkleProof(leaves[i], proof, root));
      }
    });

    it('round-trip for 7 leaves', () => {
      const leaves = Array.from({ length: 7 }, () => crypto.randomBytes(32));
      for (let i = 0; i < 7; i++) {
        const { proof, root } = pg.buildMerkleProof(leaves, i);
        assert.ok(pg.verifyMerkleProof(leaves[i], proof, root));
      }
    });

    it('round-trip for 16 leaves', () => {
      const leaves = Array.from({ length: 16 }, () => crypto.randomBytes(32));
      for (let i = 0; i < 16; i++) {
        const { proof, root } = pg.buildMerkleProof(leaves, i);
        assert.ok(pg.verifyMerkleProof(leaves[i], proof, root));
      }
    });

    it('proof for different trees with same leaves is deterministic', () => {
      const leaves = Array.from({ length: 4 }, () => crypto.randomBytes(32));
      const r1 = pg.buildMerkleProof(leaves, 0);
      const r2 = pg.buildMerkleProof(leaves, 0);
      assert.ok(r1.root.equals(r2.root));
      assert.deepEqual(r1.proof, r2.proof);
    });

    it('returns false when proof position is flipped', () => {
      const leaves = Array.from({ length: 4 }, () => crypto.randomBytes(32));
      const { proof, root } = pg.buildMerkleProof(leaves, 0);
      const flipped = proof.map((s) => ({
        ...s,
        position: s.position === 'left' ? 'right' : 'left',
      }));
      assert.equal(pg.verifyMerkleProof(leaves[0], flipped, root), false);
    });

    it('returns false when proof steps are reordered', () => {
      const leaves = Array.from({ length: 8 }, () => crypto.randomBytes(32));
      const { proof, root } = pg.buildMerkleProof(leaves, 3);
      if (proof.length >= 2) {
        const reordered = [proof[1], proof[0], ...proof.slice(2)];
        assert.equal(pg.verifyMerkleProof(leaves[3], reordered, root), false);
      }
    });
  });

  // =========================================================================
  // 3. Inclusion proof (10 tests)
  // =========================================================================
  describe('generateInclusionProof', () => {
    it('generates a valid inclusion proof', () => {
      const events = makeEvents(4);
      const proof = pg.generateInclusionProof('evt-2', events);
      assert.equal(proof.eventId, 'evt-2');
      assert.ok(proof.leaf);
      assert.ok(Array.isArray(proof.proof));
      assert.ok(proof.root);
    });

    it('proof verifies correctly via verifyInclusionProof', () => {
      const events = makeEvents(4);
      const generated = pg.generateInclusionProof('evt-1', events);
      const result = pg.verifyInclusionProof({
        leafHash: generated.leaf,
        proof: generated.proof,
        expectedRoot: generated.root,
      });
      assert.ok(result.valid);
    });

    it('throws when event not found', () => {
      const events = makeEvents(3);
      assert.throws(() => pg.generateInclusionProof('nonexistent', events), /not found/i);
    });

    it('round-trip generate then verify', () => {
      const events = makeEvents(8);
      for (const evt of events) {
        const gen = pg.generateInclusionProof(evt.id, events);
        const v = pg.verifyInclusionProof({
          leafHash: gen.leaf,
          proof: gen.proof,
          expectedRoot: gen.root,
        });
        assert.ok(v.valid, `Failed for ${evt.id}`);
      }
    });

    it('includes batchId from metadata', () => {
      const events = makeEvents(2);
      const proof = pg.generateInclusionProof('evt-0', events, {
        batchId: 'batch-42',
      });
      assert.equal(proof.batchId, 'batch-42');
    });

    it('includes anchorTxHash from metadata', () => {
      const events = makeEvents(2);
      const proof = pg.generateInclusionProof('evt-0', events, {
        anchorTxHash: '0xdeadbeef',
      });
      assert.equal(proof.anchorTxHash, '0xdeadbeef');
    });

    it('omits batchId when not provided', () => {
      const events = makeEvents(2);
      const proof = pg.generateInclusionProof('evt-0', events);
      assert.equal(proof.batchId, undefined);
    });

    it('all events in a batch share the same root', () => {
      const events = makeEvents(5);
      const roots = events.map((e) => pg.generateInclusionProof(e.id, events).root);
      for (let i = 1; i < roots.length; i++) {
        assert.equal(roots[i], roots[0]);
      }
    });

    it('proof for first event is valid', () => {
      const events = makeEvents(6);
      const gen = pg.generateInclusionProof('evt-0', events);
      const v = pg.verifyInclusionProof({
        leafHash: gen.leaf,
        proof: gen.proof,
        expectedRoot: gen.root,
      });
      assert.ok(v.valid);
    });

    it('proof for last event is valid', () => {
      const events = makeEvents(6);
      const gen = pg.generateInclusionProof('evt-5', events);
      const v = pg.verifyInclusionProof({
        leafHash: gen.leaf,
        proof: gen.proof,
        expectedRoot: gen.root,
      });
      assert.ok(v.valid);
    });
  });

  // =========================================================================
  // 4. Receipt bundle (10 tests)
  // =========================================================================
  describe('generateReceiptBundle / verifyReceiptBundle', () => {
    it('generates a complete receipt bundle', () => {
      const events = makeEvents(3);
      const bundle = pg.generateReceiptBundle(events[1], events);
      assert.equal(bundle.event.id, 'evt-1');
      assert.ok(bundle.leafHash);
      assert.ok(Array.isArray(bundle.inclusionProof));
      assert.ok(bundle.merkleRoot);
      assert.ok(bundle.timestamp);
    });

    it('bundle contains all expected fields', () => {
      const events = makeEvents(2);
      const bundle = pg.generateReceiptBundle(events[0], events, {
        batchId: 'b1',
        anchorTxHash: '0xabc',
      });
      assert.equal(bundle.batchId, 'b1');
      assert.equal(bundle.anchorTxHash, '0xabc');
      assert.ok(bundle.event.payload);
      assert.ok(bundle.event.payloadHash);
      assert.ok(bundle.event.eventSigningHash);
      assert.ok(bundle.event.timestamp);
    });

    it('verify valid bundle passes (inclusion check)', () => {
      const events = makeEvents(4);
      const bundle = pg.generateReceiptBundle(events[2], events);
      const result = pg.verifyReceiptBundle(bundle);
      assert.ok(result.valid);
      assert.ok(result.checks.length >= 1);
      const inclusionCheck = result.checks.find((c) => c.check === 'inclusion_proof');
      assert.ok(inclusionCheck.passed);
    });

    it('verify bundle checks payload hash when present', () => {
      const events = makeEvents(3);
      const bundle = pg.generateReceiptBundle(events[0], events);
      const result = pg.verifyReceiptBundle(bundle);
      const payloadCheck = result.checks.find((c) => c.check === 'payload_hash');
      assert.ok(payloadCheck);
      assert.ok(payloadCheck.passed);
    });

    it('verify tampered bundle fails', () => {
      const events = makeEvents(3);
      const bundle = pg.generateReceiptBundle(events[0], events);
      // Tamper with the leafHash
      bundle.leafHash = crypto.randomBytes(32).toString('hex');
      const result = pg.verifyReceiptBundle(bundle);
      assert.equal(result.valid, false);
    });

    it('verify bundle with tampered payload fails payload_hash check', () => {
      const events = makeEvents(3);
      const bundle = pg.generateReceiptBundle(events[0], events);
      bundle.event.payload = { tampered: true };
      const result = pg.verifyReceiptBundle(bundle);
      const payloadCheck = result.checks.find((c) => c.check === 'payload_hash');
      assert.ok(payloadCheck);
      assert.equal(payloadCheck.passed, false);
    });

    it('bundle with batch metadata includes batchId', () => {
      const events = makeEvents(2);
      const bundle = pg.generateReceiptBundle(events[0], events, {
        batchId: 'my-batch',
      });
      assert.equal(bundle.batchId, 'my-batch');
    });

    it('bundle without batch metadata has null batchId', () => {
      const events = makeEvents(2);
      const bundle = pg.generateReceiptBundle(events[0], events);
      assert.equal(bundle.batchId, null);
    });

    it('bundle without signature sets signature to null', () => {
      const events = makeEvents(2);
      const bundle = pg.generateReceiptBundle(events[0], events);
      assert.equal(bundle.event.signature, null);
    });

    it('verify bundle with tampered merkleRoot fails', () => {
      const events = makeEvents(4);
      const bundle = pg.generateReceiptBundle(events[1], events);
      bundle.merkleRoot = crypto.randomBytes(32).toString('hex');
      const result = pg.verifyReceiptBundle(bundle);
      assert.equal(result.valid, false);
    });
  });

  // =========================================================================
  // 5. Batch summary (10 tests)
  // =========================================================================
  describe('generateBatchSummary', () => {
    it('computes correct root', () => {
      const events = makeEvents(4);
      const summary = pg.generateBatchSummary('batch-1', events);
      assert.ok(summary.root);
      assert.equal(typeof summary.root, 'string');
      assert.equal(summary.root.length, 64); // 32 bytes hex
    });

    it('reports correct event count', () => {
      const events = makeEvents(7);
      const summary = pg.generateBatchSummary('batch-2', events);
      assert.equal(summary.eventCount, 7);
    });

    it('extracts time range', () => {
      const events = makeEvents(5);
      const summary = pg.generateBatchSummary('batch-3', events);
      assert.ok(summary.timeRange.start);
      assert.ok(summary.timeRange.end);
      assert.ok(new Date(summary.timeRange.start) <= new Date(summary.timeRange.end));
    });

    it('handles empty batch', () => {
      const summary = pg.generateBatchSummary('empty', []);
      assert.equal(summary.eventCount, 0);
      assert.equal(summary.timeRange.start, null);
      assert.equal(summary.timeRange.end, null);
      assert.equal(summary.root, cryptoMod.ZERO_HASH.toString('hex'));
    });

    it('includes anchorTxHash when provided', () => {
      const events = makeEvents(2);
      const summary = pg.generateBatchSummary('batch-4', events, {
        txHash: '0x123abc',
      });
      assert.equal(summary.anchorTxHash, '0x123abc');
    });

    it('anchorTxHash is null when not provided', () => {
      const events = makeEvents(2);
      const summary = pg.generateBatchSummary('batch-5', events);
      assert.equal(summary.anchorTxHash, null);
    });

    it('batchId matches input', () => {
      const events = makeEvents(3);
      const summary = pg.generateBatchSummary('my-batch-id', events);
      assert.equal(summary.batchId, 'my-batch-id');
    });

    it('root is deterministic for same events', () => {
      const events = makeEvents(4);
      const s1 = pg.generateBatchSummary('b', events);
      const s2 = pg.generateBatchSummary('b', events);
      assert.equal(s1.root, s2.root);
    });

    it('root differs for different events', () => {
      const events1 = makeEvents(3);
      const events2 = [makeEvent(10), makeEvent(11), makeEvent(12)];
      const s1 = pg.generateBatchSummary('b', events1);
      const s2 = pg.generateBatchSummary('b', events2);
      assert.notEqual(s1.root, s2.root);
    });

    it('single-event batch has correct count and root', () => {
      const events = makeEvents(1);
      const summary = pg.generateBatchSummary('single', events);
      assert.equal(summary.eventCount, 1);
      assert.ok(summary.root);
    });
  });

  // =========================================================================
  // 6. Compliance package (10 tests)
  // =========================================================================
  describe('generateCompliancePackage', () => {
    it('generates receipts for all events', () => {
      const events = makeEvents(5);
      const pkg = pg.generateCompliancePackage(events);
      assert.equal(pkg.receipts.length, 5);
    });

    it('all receipts are individually verifiable', () => {
      const events = makeEvents(4);
      const pkg = pg.generateCompliancePackage(events);
      for (const receipt of pkg.receipts) {
        const result = pg.verifyReceiptBundle(receipt);
        assert.ok(result.valid, `Receipt for ${receipt.event.id} failed`);
      }
    });

    it('includes summary', () => {
      const events = makeEvents(3);
      const pkg = pg.generateCompliancePackage(events);
      assert.ok(pkg.summary);
      assert.equal(pkg.summary.eventCount, 3);
    });

    it('summary root matches receipt roots', () => {
      const events = makeEvents(4);
      const pkg = pg.generateCompliancePackage(events);
      for (const receipt of pkg.receipts) {
        assert.equal(receipt.merkleRoot, pkg.summary.root);
      }
    });

    it('includes generatedAt timestamp', () => {
      const events = makeEvents(2);
      const pkg = pg.generateCompliancePackage(events);
      assert.ok(pkg.generatedAt);
      const dt = new Date(pkg.generatedAt);
      assert.ok(!isNaN(dt.getTime()));
    });

    it('uses provided batchId', () => {
      const events = makeEvents(2);
      const pkg = pg.generateCompliancePackage(events, {
        batchId: 'compliance-batch',
      });
      assert.equal(pkg.summary.batchId, 'compliance-batch');
      for (const receipt of pkg.receipts) {
        assert.equal(receipt.batchId, 'compliance-batch');
      }
    });

    it('auto-generates batchId when not provided', () => {
      const events = makeEvents(2);
      const pkg = pg.generateCompliancePackage(events);
      assert.ok(pkg.summary.batchId);
      // Should look like a UUID
      assert.match(
        pkg.summary.batchId,
        /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/,
      );
    });

    it('includes anchorTxHash in receipts', () => {
      const events = makeEvents(3);
      const pkg = pg.generateCompliancePackage(events, {
        anchorTxHash: '0xchain123',
      });
      for (const receipt of pkg.receipts) {
        assert.equal(receipt.anchorTxHash, '0xchain123');
      }
    });

    it('handles single-event batch', () => {
      const events = makeEvents(1);
      const pkg = pg.generateCompliancePackage(events);
      assert.equal(pkg.receipts.length, 1);
      assert.equal(pkg.summary.eventCount, 1);
    });

    it('receipt event ids match input event ids', () => {
      const events = makeEvents(5);
      const pkg = pg.generateCompliancePackage(events);
      const receiptIds = pkg.receipts.map((r) => r.event.id).sort();
      const eventIds = events.map((e) => e.id).sort();
      assert.deepEqual(receiptIds, eventIds);
    });
  });

  // =========================================================================
  // 7. Event verification (10 tests)
  // =========================================================================
  describe('verifyEvent', () => {
    it('valid event with 32-byte hash passes', () => {
      const event = makeEvent(0);
      const result = pg.verifyEvent(event);
      assert.ok(result.valid);
      assert.ok(result.hashValid);
      assert.equal(result.signatureValid, null); // no sig provided
    });

    it('event with short hash fails hashValid', () => {
      const event = makeEvent(0);
      event.eventSigningHash = Buffer.alloc(16); // too short
      const result = pg.verifyEvent(event);
      assert.equal(result.hashValid, false);
      assert.equal(result.valid, false);
    });

    it('event with hex-string hash is accepted', () => {
      const event = makeEvent(0);
      event.eventSigningHash = crypto.randomBytes(32).toString('hex');
      const result = pg.verifyEvent(event);
      assert.ok(result.hashValid);
    });

    it('missing signature returns signatureValid=null', () => {
      const event = makeEvent(0);
      const result = pg.verifyEvent(event, crypto.randomBytes(32));
      assert.equal(result.signatureValid, null);
    });

    it('invalid signature returns signatureValid=false', () => {
      const event = makeEvent(0);
      event.signature = crypto.randomBytes(64).toString('hex');
      const pubKey = crypto.randomBytes(32);
      const result = pg.verifyEvent(event, pubKey);
      assert.equal(result.signatureValid, false);
      assert.equal(result.valid, false);
    });

    it('hash validity is independent of signature', () => {
      const event = makeEvent(0);
      event.signature = crypto.randomBytes(64).toString('hex');
      const result = pg.verifyEvent(event, crypto.randomBytes(32));
      assert.ok(result.hashValid); // hash is still valid
    });

    it('event with valid Ed25519 signature passes', () => {
      // Generate a real Ed25519 key pair
      const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
      const privRaw = privateKey.export({ type: 'pkcs8', format: 'der' }).subarray(-32);
      const pubRaw = publicKey.export({ type: 'spki', format: 'der' }).subarray(-32);

      const event = makeEvent(0);
      // Sign the eventSigningHash
      const sig = cryptoMod.signEventHash(event.eventSigningHash, privRaw);
      event.signature = sig.toString('hex');

      const result = pg.verifyEvent(event, pubRaw);
      assert.ok(result.signatureValid);
      assert.ok(result.valid);
    });

    it('event with wrong public key fails signature', () => {
      const { privateKey } = crypto.generateKeyPairSync('ed25519');
      const { publicKey: wrongPub } = crypto.generateKeyPairSync('ed25519');
      const privRaw = privateKey.export({ type: 'pkcs8', format: 'der' }).subarray(-32);
      const wrongPubRaw = wrongPub.export({ type: 'spki', format: 'der' }).subarray(-32);

      const event = makeEvent(0);
      const sig = cryptoMod.signEventHash(event.eventSigningHash, privRaw);
      event.signature = sig.toString('hex');

      const result = pg.verifyEvent(event, wrongPubRaw);
      assert.equal(result.signatureValid, false);
    });

    it('passes when no publicKey given even with signature', () => {
      const event = makeEvent(0);
      event.signature = crypto.randomBytes(64).toString('hex');
      // No publicKey => signature check skipped
      const result = pg.verifyEvent(event);
      assert.equal(result.signatureValid, null);
      assert.ok(result.valid);
    });

    it('works with Buffer publicKey', () => {
      const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
      const privRaw = privateKey.export({ type: 'pkcs8', format: 'der' }).subarray(-32);
      const pubBuf = publicKey.export({ type: 'spki', format: 'der' }).subarray(-32);

      const event = makeEvent(0);
      event.signature = cryptoMod.signEventHash(event.eventSigningHash, privRaw).toString('hex');

      const result = pg.verifyEvent(event, pubBuf);
      assert.ok(result.signatureValid);
    });

    it(
      'verifies a hybrid signature bundle when given a hybrid public-key bundle',
      { skip: !cryptoMod.hasNativeHybridPqcVerificationSupport() },
      () => {
        const hybrid = cryptoMod.generateHybridSigningKeypair();
        const event = makeEvent(1);
        const signatureBundle = cryptoMod.signEventHashHybrid(event.eventSigningHash, {
          ed25519PrivateKey: hybrid.ed25519PrivateKey,
          mlDsa65Seed: hybrid.mlDsa65Seed,
        });

        event.signature = signatureBundle.ed25519Signature.toString('hex');
        event.signatureBundle = {
          ed25519Signature: signatureBundle.ed25519Signature.toString('hex'),
          mlDsa65Signature: signatureBundle.mlDsa65Signature.toString('hex'),
        };

        const result = pg.verifyEvent(event, {
          ed25519PublicKey: hybrid.ed25519PublicKey.toString('hex'),
          mlDsa65PublicKey: hybrid.mlDsa65PublicKey.toString('hex'),
        });
        assert.ok(result.signatureValid);
        assert.ok(result.valid);
      },
    );
  });

  // =========================================================================
  // 8. verifyInclusionProof edge cases (5 extra)
  // =========================================================================
  describe('verifyInclusionProof (standalone)', () => {
    it('passes eventId through to result', () => {
      const events = makeEvents(3);
      const gen = pg.generateInclusionProof('evt-1', events);
      const result = pg.verifyInclusionProof({
        leafHash: gen.leaf,
        proof: gen.proof,
        expectedRoot: gen.root,
        eventId: 'evt-1',
      });
      assert.equal(result.eventId, 'evt-1');
    });

    it('root is echoed back', () => {
      const events = makeEvents(2);
      const gen = pg.generateInclusionProof('evt-0', events);
      const result = pg.verifyInclusionProof({
        leafHash: gen.leaf,
        proof: gen.proof,
        expectedRoot: gen.root,
      });
      assert.equal(result.root, gen.root);
    });

    it('invalid proof returns valid=false', () => {
      const result = pg.verifyInclusionProof({
        leafHash: crypto.randomBytes(32).toString('hex'),
        proof: [{ position: 'right', hash: crypto.randomBytes(32).toString('hex') }],
        expectedRoot: crypto.randomBytes(32).toString('hex'),
      });
      assert.equal(result.valid, false);
    });

    it('empty proof with matching leaf and root returns true', () => {
      const leaf = crypto.randomBytes(32);
      const result = pg.verifyInclusionProof({
        leafHash: leaf.toString('hex'),
        proof: [],
        expectedRoot: leaf.toString('hex'),
      });
      assert.ok(result.valid);
    });

    it('empty proof with mismatched leaf and root returns false', () => {
      const result = pg.verifyInclusionProof({
        leafHash: crypto.randomBytes(32).toString('hex'),
        proof: [],
        expectedRoot: crypto.randomBytes(32).toString('hex'),
      });
      assert.equal(result.valid, false);
    });
  });
});

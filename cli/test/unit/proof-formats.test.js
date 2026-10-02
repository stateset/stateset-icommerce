/**
 * Versioned proof formats.
 *
 * The proof generator claimed VES v1.0 compliance while hashing leaves in the
 * NODE domain and padding with the zero hash, so its proofs could not be
 * verified by the SDK's own spec verifier: one package shipped two Merkle
 * proof systems that could not check each other. Roots it produced may
 * already be anchored on-chain, so the fix is a FORMAT, not a rewrite:
 *
 *   ves-v1     spec leaves, pad leaves and node hashing -- new proofs
 *   legacy-v0  the original construction, kept so anchored roots verify
 *
 * The first test below is the one that matters: a proof from the generator,
 * verified by `SequencerClient.verifyInclusion`, the spec verifier.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { createProofGenerator } from '../../src/sync/proof-generator.js';
import { SequencerClient } from '../../src/sync/client.js';
import * as cryptoMod from '../../src/sync/crypto.js';

const {
  computeEventSigningHash,
  computePayloadPlainHash,
  computePadLeaf,
  hexToBuffer,
} = cryptoMod;

const pg = createProofGenerator(cryptoMod);
const { PROOF_FORMATS } = pg;

const TENANT = '550e8400-e29b-41d4-a716-446655440002';
const STORE = '550e8400-e29b-41d4-a716-446655440003';

function envelope(i) {
  return {
    eventId: `550e8400-e29b-41d4-a716-${String(446655440100 + i).padStart(12, '0')}`,
    commandId: null,
    tenantId: TENANT,
    storeId: STORE,
    entityType: 'order',
    entityId: `ORD-${i}`,
    eventType: 'created',
    vesVersion: 1,
    payloadKind: 0,
    payloadPlainHash: computePayloadPlainHash({ order: i }).toString('hex'),
    payloadCipherHash: '0'.repeat(64),
    agentKeyId: 1,
    agentSignature: String(i % 10).repeat(128),
    baseVersion: null,
    createdAt: '2026-09-22T00:00:00Z',
    sourceAgent: '550e8400-e29b-41d4-a716-446655440001',
    sequenceNumber: i,
  };
}

/** The signing hash the spec verifier recomputes from the envelope. */
function signingHash(env) {
  return computeEventSigningHash({
    vesVersion: env.vesVersion,
    tenantId: env.tenantId,
    storeId: env.storeId,
    eventId: env.eventId,
    commandId: env.commandId,
    sourceAgentId: env.sourceAgent,
    agentKeyId: env.agentKeyId,
    entityType: env.entityType,
    entityId: env.entityId,
    eventType: env.eventType,
    payloadKind: env.payloadKind,
    baseVersion: env.baseVersion,
    createdAt: env.createdAt,
    payloadPlainHash: hexToBuffer(env.payloadPlainHash),
    payloadCipherHash: hexToBuffer(env.payloadCipherHash),
  });
}

/** An event as the generator takes it, carrying every field the spec leaf binds. */
function specEvent(env) {
  return {
    id: env.eventId,
    tenantId: env.tenantId,
    storeId: env.storeId,
    sequenceNumber: env.sequenceNumber,
    eventSigningHash: signingHash(env),
    agentSignature: env.agentSignature,
  };
}

function client() {
  return new SequencerClient({
    sequencerUrl: 'https://seq.example.com',
    securityProfile: 'legacy',
    sequencer: { url: 'https://seq.example.com' },
    tenantId: TENANT,
    storeId: STORE,
    getCredentials: () => ({ apiKey: null, jwt: null }),
    retryPolicy: { maxRetries: 0, baseDelay: 1, maxDelay: 1 },
  });
}

describe('proof formats', () => {
  it('a ves-v1 proof verifies through the SDK spec verifier', () => {
    // Five events: not a power of two, so padding is exercised too.
    const envelopes = [0, 1, 2, 3, 4].map(envelope);
    const events = envelopes.map(specEvent);

    for (const [i, env] of envelopes.entries()) {
      const proof = pg.generateInclusionProof(env.eventId, events);
      assert.equal(proof.format, PROOF_FORMATS.VES_V1);
      assert.equal(proof.leafIndex, i);
      assert.equal(
        client().verifyInclusion(env, proof, proof.root),
        true,
        `event ${i}: the generator's proof must verify under the spec verifier`,
      );
    }
  });

  it('a legacy-v0 proof does NOT verify through the spec verifier (the original bug)', () => {
    const envelopes = [0, 1, 2].map(envelope);
    const events = envelopes.map(specEvent);
    const legacy = pg.generateInclusionProof(envelopes[1].eventId, events, {}, {
      format: PROOF_FORMATS.LEGACY_V0,
    });
    assert.equal(client().verifyInclusion(envelopes[1], legacy, legacy.root), false);
  });

  it('the ves-v1 tree matches the Rust merkle_root vectors', () => {
    // bindings/test-vectors/v1.json is ground truth for tree construction:
    // stateset-crypto's compute_merkle_root produced every expected root.
    const here = path.dirname(fileURLToPath(import.meta.url));
    const corpus = JSON.parse(
      fs.readFileSync(path.join(here, '..', '..', '..', 'bindings', 'test-vectors', 'v1.json'), 'utf8'),
    );
    const vectors = corpus.categories.merkle_root;
    assert.ok(vectors.length > 0, 'the corpus has merkle_root vectors');
    for (const v of vectors) {
      if (v.leaves_hex.length === 0) {
        assert.equal(computePadLeaf().toString('hex'), v.expected_hex, `${v.id}: empty root`);
        continue;
      }
      const leaves = v.leaves_hex.map((h) => Buffer.from(h, 'hex'));
      const { root } = pg.buildMerkleProof(leaves, 0, computePadLeaf());
      assert.equal(root.toString('hex'), v.expected_hex, `${v.id}: ves-v1 root disagrees with Rust`);
    }
  });

  it('a proof with no format field is read as legacy-v0 and still verifies', () => {
    // This is what keeps roots already anchored on-chain valid.
    const events = [0, 1, 2].map((i) => ({ id: `e${i}`, eventSigningHash: Buffer.alloc(32, i + 1) }));
    const proof = pg.generateInclusionProof('e1', events);
    assert.equal(proof.format, PROOF_FORMATS.LEGACY_V0);
    const verdict = pg.verifyInclusionProof({
      leafHash: proof.leaf,
      proof: proof.proof,
      expectedRoot: proof.root,
    });
    assert.equal(verdict.valid, true);
    assert.equal(verdict.format, PROOF_FORMATS.LEGACY_V0);
  });

  it('minimal events fall back to legacy-v0 and say why', () => {
    const events = [{ id: 'e0', eventSigningHash: Buffer.alloc(32, 1) }];
    const proof = pg.generateInclusionProof('e0', events);
    assert.equal(proof.format, PROOF_FORMATS.LEGACY_V0);
    assert.match(proof.warning, /legacy-v0/);
    assert.match(proof.warning, /sequenceNumber/);
  });

  it('asking for ves-v1 without the fields it binds fails loudly', () => {
    const events = [{ id: 'e0', eventSigningHash: Buffer.alloc(32, 1) }];
    assert.throws(
      () => pg.generateInclusionProof('e0', events, {}, { format: PROOF_FORMATS.VES_V1 }),
      /missing .*tenantId/,
    );
  });

  it('an unknown format is refused on generation and on verification', () => {
    const events = [{ id: 'e0', eventSigningHash: Buffer.alloc(32, 1) }];
    assert.throws(() => pg.generateInclusionProof('e0', events, {}, { format: 'v9' }), /Unknown/);
    const verdict = pg.verifyInclusionProof({
      format: 'v9',
      leafHash: '00'.repeat(32),
      proof: [],
      expectedRoot: '00'.repeat(32),
    });
    assert.equal(verdict.valid, false);
  });

  it('a ves-v1 receipt recomputes its leaf, and a swapped leaf fails', () => {
    const envelopes = [0, 1, 2, 3].map(envelope);
    const events = envelopes.map(specEvent);
    const bundle = pg.generateReceiptBundle(events[2], events);
    assert.equal(bundle.format, PROOF_FORMATS.VES_V1);

    const good = pg.verifyReceiptBundle(bundle);
    assert.equal(good.valid, true);
    assert.ok(good.checks.some((c) => c.check === 'leaf_hash' && c.passed));

    // Claim the proof is for a different event: the inclusion walk alone
    // would still pass, because the leaf and siblings are unchanged. The
    // recomputed leaf is what catches it.
    const forged = { ...bundle, event: { ...bundle.event, sequenceNumber: 99 } };
    const verdict = pg.verifyReceiptBundle(forged);
    assert.equal(verdict.valid, false);
    assert.ok(verdict.checks.some((c) => c.check === 'leaf_hash' && !c.passed));
  });

  it('an explicit ves-v1 empty batch has the spec empty root', () => {
    const summary = pg.generateBatchSummary('b0', [], {}, { format: PROOF_FORMATS.VES_V1 });
    assert.equal(summary.root, computePadLeaf().toString('hex'));
  });
});

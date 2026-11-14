/**
 * Canonical path shape in the JavaScript Merkle inclusion verifiers.
 *
 * The Rust verifier (crates/stateset-sync/src/attestation.rs) refuses a proof
 * whose leaf index is outside the tree or whose path is not exactly one
 * sibling per level of a tree of `total_leaves` leaves. These tests hold the
 * JavaScript verifiers -- the proof generator's, the SDK spec verifier
 * `SequencerClient.verifyInclusion`, and the inline walks behind the
 * `sync_verify_inclusion` tool and the `sync verify-inclusion` command -- to
 * the same rule, and check that a verification result says what was and was
 * not checked.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import nodeCrypto from 'node:crypto';

import { createProofGenerator } from '../../src/sync/proof-generator.js';
import { SequencerClient } from '../../src/sync/client.js';
import * as cryptoMod from '../../src/sync/crypto.js';
import { syncTools } from '../../src/tools/sync.js';
import { execute as executeSyncCommand } from '../../src/commands/sync.js';

const {
  computeEventSigningHash,
  computeLeafHash,
  computeNodeHash,
  computePayloadPlainHash,
  computePadLeaf,
  hexToBuffer,
  signEventHash,
} = cryptoMod;

const pg = createProofGenerator(cryptoMod);
const { PROOF_FORMATS } = pg;

const TENANT = '550e8400-e29b-41d4-a716-446655440002';
const STORE = '550e8400-e29b-41d4-a716-446655440003';
const TREE_SIZES = [1, 2, 3, 5, 8];

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

/** An event carrying every field the ves-v1 leaf binds. */
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

/** An event with only what a legacy-v0 leaf needs. */
function legacyEvent(i) {
  return { id: `e${i}`, eventSigningHash: Buffer.alloc(32, i + 1) };
}

/** The ves-v1 leaf the spec verifier recomputes for an envelope. */
function specLeaf(env, sequenceNumber = env.sequenceNumber) {
  return computeLeafHash({
    tenantId: env.tenantId,
    storeId: env.storeId,
    sequenceNumber,
    eventSigningHash: signingHash(env),
    agentSignature: hexToBuffer(env.agentSignature),
  });
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

const hex = (buf) => buf.toString('hex');

// ---------------------------------------------------------------------------
// proof-generator.js
// ---------------------------------------------------------------------------

describe('proof generator: canonical path shape', () => {
  it('generated proofs and bundles carry the tree size', () => {
    const events = [0, 1, 2].map(legacyEvent);
    assert.equal(pg.generateInclusionProof('e1', events).leafCount, 3);
    assert.equal(pg.generateReceiptBundle(events[1], events).leafCount, 3);
  });

  for (const size of TREE_SIZES) {
    it(`accepts every legitimate proof of a ${size}-leaf tree, in both formats`, () => {
      const envelopes = Array.from({ length: size }, (_, i) => envelope(i));
      const batches = {
        [PROOF_FORMATS.VES_V1]: envelopes.map(specEvent),
        [PROOF_FORMATS.LEGACY_V0]: envelopes.map((_, i) => legacyEvent(i)),
      };
      for (const [format, events] of Object.entries(batches)) {
        for (const [i, evt] of events.entries()) {
          const proof = pg.generateInclusionProof(evt.id, events);
          assert.equal(proof.format, format);
          const verdict = pg.verifyInclusionProof({
            format: proof.format,
            leafHash: proof.leaf,
            proof: proof.proof,
            expectedRoot: proof.root,
            leafCount: proof.leafCount,
            leafIndex: proof.leafIndex,
          });
          assert.equal(verdict.valid, true, `${format} ${size} leaves, index ${i}`);
          assert.equal(verdict.pathBound, true);

          const bundle = pg.generateReceiptBundle(evt, events);
          const receipt = pg.verifyReceiptBundle(bundle);
          assert.equal(receipt.valid, true, `${format} bundle, ${size} leaves, index ${i}`);
          assert.equal(receipt.pathBound, true);
          // A caller that knows the batch size independently gets the same answer.
          assert.equal(pg.verifyReceiptBundle(bundle, undefined, { leafCount: size }).valid, true);
        }
      }
    });
  }

  it('rejects a path longer than the tree size allows', () => {
    const [a, b, extra] = [1, 2, 3].map((n) => Buffer.alloc(32, n));
    // A consistent three-level walk, presented as a proof in a 2-leaf tree.
    const root = computeNodeHash(computeNodeHash(a, b), extra);
    const proof = [
      { position: 'right', hash: hex(b) },
      { position: 'right', hash: hex(extra) },
    ];
    assert.equal(pg.verifyMerkleProof(a, proof, root, { leafCount: 2 }), false);
    const verdict = pg.verifyInclusionProof({
      leafHash: hex(a),
      proof,
      expectedRoot: hex(root),
      leafCount: 2,
    });
    assert.equal(verdict.valid, false);
    // The same walk is a legitimate proof in a tree that deep.
    assert.equal(pg.verifyMerkleProof(a, proof, root, { leafCount: 4 }), true);
  });

  it('rejects a path shorter than the tree size requires', () => {
    const [a, b] = [1, 2].map((n) => Buffer.alloc(32, n));
    const root = computeNodeHash(a, b);
    const proof = [{ position: 'right', hash: hex(b) }];
    assert.equal(pg.verifyMerkleProof(a, proof, root, { leafCount: 5 }), false);
  });

  it('rejects an empty path with leaf == root unless the tree has exactly one leaf', () => {
    const leaf = Buffer.alloc(32, 7);
    for (const leafCount of [2, 3, 8]) {
      const verdict = pg.verifyInclusionProof({
        leafHash: hex(leaf),
        proof: [],
        expectedRoot: hex(leaf),
        leafCount,
      });
      assert.equal(verdict.valid, false, `${leafCount} leaves`);
    }
    const single = pg.verifyInclusionProof({
      leafHash: hex(leaf),
      proof: [],
      expectedRoot: hex(leaf),
      leafCount: 1,
    });
    assert.equal(single.valid, true);
    assert.equal(single.pathBound, true);
  });

  it('rejects a leaf index at or beyond the tree size', () => {
    // Three real leaves padded to four: slot 3 holds padding, not an event.
    const leaves = [1, 2, 3].map((n) => Buffer.alloc(32, n));
    const pad = computePadLeaf();
    const { root } = pg.buildMerkleProof(leaves, 0, pad);
    const padded = pg.buildMerkleProof([...leaves, pad], 3, pad);
    assert.ok(padded.root.equals(root), 'the padding slot hashes to the same root');
    assert.equal(pg.verifyMerkleProof(pad, padded.proof, root, { leafCount: 3 }), false);
    assert.equal(
      pg.verifyMerkleProof(pad, padded.proof, root, { leafCount: 3, leafIndex: 3 }),
      false,
    );
  });

  it('rejects a path whose positions disagree with the stated leaf index', () => {
    const leaves = [1, 2, 3, 4].map((n) => Buffer.alloc(32, n));
    const { proof, root } = pg.buildMerkleProof(leaves, 2);
    assert.equal(
      pg.verifyMerkleProof(leaves[2], proof, root, { leafCount: 4, leafIndex: 2 }),
      true,
    );
    assert.equal(
      pg.verifyMerkleProof(leaves[2], proof, root, { leafCount: 4, leafIndex: 1 }),
      false,
    );
  });

  it('rejects a step whose position is neither left nor right', () => {
    const [a, b] = [1, 2].map((n) => Buffer.alloc(32, n));
    const root = computeNodeHash(b, a);
    assert.equal(pg.verifyMerkleProof(a, [{ position: 'left', hash: hex(b) }], root), true);
    assert.equal(pg.verifyMerkleProof(a, [{ position: 'up', hash: hex(b) }], root), false);
    assert.equal(pg.verifyMerkleProof(a, [{ hash: hex(b) }], root), false);
  });

  it('rejects a path deeper than any tree can be, even with no tree size', () => {
    let node = Buffer.alloc(32, 1);
    const leaf = node;
    const proof = [];
    for (let i = 0; i < 33; i++) {
      const sibling = Buffer.alloc(32, i + 2);
      proof.push({ position: 'right', hash: hex(sibling) });
      node = computeNodeHash(node, sibling);
    }
    assert.equal(pg.verifyMerkleProof(leaf, proof, node), false);
    assert.equal(pg.verifyMerkleProof(leaf, 'not-a-path', node), false);
  });

  it('a proof with no tree size still verifies, and says the path was not bound', () => {
    // Bundles written before `leafCount` existed carry no tree size. Whether
    // to refuse them is the owner's call; until then the result is honest.
    const events = [0, 1, 2].map(legacyEvent);
    const proof = pg.generateInclusionProof('e1', events);
    const verdict = pg.verifyInclusionProof({
      leafHash: proof.leaf,
      proof: proof.proof,
      expectedRoot: proof.root,
    });
    assert.equal(verdict.valid, true);
    assert.equal(verdict.pathBound, false);

    const bundle = pg.generateReceiptBundle(events[1], events);
    delete bundle.leafCount;
    const receipt = pg.verifyReceiptBundle(bundle);
    assert.equal(receipt.valid, true);
    assert.equal(receipt.pathBound, false);
  });

  it('an independently known tree size overrides what the bundle claims', () => {
    const events = [0, 1, 2, 3].map(legacyEvent);
    const honest = pg.generateReceiptBundle(events[2], events);
    assert.equal(pg.verifyReceiptBundle(honest, undefined, { leafCount: 4 }).valid, true);
    assert.equal(pg.verifyReceiptBundle(honest, undefined, { leafCount: 8 }).valid, false);

    // A forged bundle: the root itself offered as the leaf of a one-leaf tree.
    const forged = {
      ...honest,
      leafHash: honest.merkleRoot,
      inclusionProof: [],
      leafIndex: 0,
      leafCount: 1,
    };
    assert.equal(pg.verifyReceiptBundle(forged).valid, true, 'self-consistent on its own word');
    const verdict = pg.verifyReceiptBundle(forged, undefined, { leafCount: 4 });
    assert.equal(verdict.valid, false);
    assert.ok(verdict.checks.some((c) => c.check === 'inclusion_proof' && !c.passed));
  });
});

describe('proof generator: the result says what was checked', () => {
  function keypair() {
    const { publicKey, privateKey } = nodeCrypto.generateKeyPairSync('ed25519');
    return {
      priv: privateKey.export({ type: 'pkcs8', format: 'der' }).subarray(-32),
      pub: publicKey.export({ type: 'spki', format: 'der' }).subarray(-32),
    };
  }

  it('ves-v1 bundle: the leaf is bound to the event', () => {
    const events = [0, 1, 2, 3].map(envelope).map(specEvent);
    const bundle = pg.generateReceiptBundle(events[1], events);
    const verdict = pg.verifyReceiptBundle(bundle);
    assert.equal(verdict.valid, true);
    assert.equal(verdict.format, PROOF_FORMATS.VES_V1);
    assert.equal(verdict.leafBound, true);
    assert.equal(verdict.signatureChecked, false);
  });

  it('ves-v1 bundle whose leaf does not match its event is not leaf-bound', () => {
    const events = [0, 1, 2, 3].map(envelope).map(specEvent);
    const bundle = pg.generateReceiptBundle(events[1], events);
    const forged = { ...bundle, event: { ...bundle.event, sequenceNumber: 99 } };
    const verdict = pg.verifyReceiptBundle(forged);
    assert.equal(verdict.valid, false);
    assert.equal(verdict.leafBound, false);
  });

  it('legacy-v0 bundle: valid, but the leaf is NOT bound to the event', () => {
    const events = [0, 1, 2].map(legacyEvent);
    const bundle = pg.generateReceiptBundle(events[1], events);
    assert.equal(bundle.format, PROOF_FORMATS.LEGACY_V0);
    const verdict = pg.verifyReceiptBundle(bundle);
    assert.equal(verdict.valid, true);
    assert.equal(verdict.format, PROOF_FORMATS.LEGACY_V0);
    assert.equal(verdict.leafBound, false);
    assert.equal(verdict.signatureChecked, false);
  });

  it('a bundle with no format is read as legacy-v0 and reported as such', () => {
    const events = [0, 1, 2].map(legacyEvent);
    const bundle = pg.generateReceiptBundle(events[1], events);
    delete bundle.format;
    const verdict = pg.verifyReceiptBundle(bundle);
    assert.equal(verdict.valid, true);
    assert.equal(verdict.format, PROOF_FORMATS.LEGACY_V0);
    assert.equal(verdict.leafBound, false);
  });

  it('signatureChecked is true only when a public key lets the signature be checked', () => {
    const { priv, pub } = keypair();
    const events = [0, 1].map(legacyEvent);
    events[0].signature = hex(signEventHash(events[0].eventSigningHash, priv));
    const bundle = pg.generateReceiptBundle(events[0], events);

    const unkeyed = pg.verifyReceiptBundle(bundle);
    assert.equal(unkeyed.valid, true);
    assert.equal(unkeyed.signatureChecked, false);

    const keyed = pg.verifyReceiptBundle(bundle, pub);
    assert.equal(keyed.valid, true);
    assert.equal(keyed.signatureChecked, true);

    const wrongKey = pg.verifyReceiptBundle(bundle, keypair().pub);
    assert.equal(wrongKey.valid, false);
    assert.equal(wrongKey.signatureChecked, true);
  });

  it('a public key with no signature on the bundle checks nothing', () => {
    const events = [0, 1].map(legacyEvent);
    const bundle = pg.generateReceiptBundle(events[0], events);
    const verdict = pg.verifyReceiptBundle(bundle, keypair().pub);
    assert.equal(verdict.valid, true);
    assert.equal(verdict.signatureChecked, false);
  });

  it('verifyInclusionProof takes its leaf on trust and checks no signature', () => {
    const events = [0, 1, 2].map(envelope).map(specEvent);
    const proof = pg.generateInclusionProof(events[0].id, events);
    const verdict = pg.verifyInclusionProof({
      format: proof.format,
      leafHash: proof.leaf,
      proof: proof.proof,
      expectedRoot: proof.root,
      leafCount: proof.leafCount,
    });
    assert.equal(verdict.valid, true);
    assert.equal(verdict.leafBound, false);
    assert.equal(verdict.signatureChecked, false);

    const unknown = pg.verifyInclusionProof({
      format: 'v9',
      leafHash: proof.leaf,
      proof: proof.proof,
      expectedRoot: proof.root,
    });
    assert.equal(unknown.valid, false);
    assert.equal(unknown.leafBound, false);
    assert.equal(unknown.signatureChecked, false);
    assert.equal(unknown.pathBound, false);
  });
});

// ---------------------------------------------------------------------------
// client.js -- SequencerClient.verifyInclusion
// ---------------------------------------------------------------------------

describe('SequencerClient.verifyInclusion: canonical path shape', () => {
  for (const size of TREE_SIZES) {
    it(`accepts every legitimate proof of a ${size}-leaf tree`, () => {
      const envelopes = Array.from({ length: size }, (_, i) => envelope(i));
      const events = envelopes.map(specEvent);
      for (const [i, env] of envelopes.entries()) {
        const proof = pg.generateInclusionProof(env.eventId, events);
        assert.equal(proof.leafCount, size);
        assert.equal(
          client().verifyInclusion(env, proof, proof.root),
          true,
          `${size} leaves, index ${i}`,
        );
      }
    });
  }

  it('rejects a path longer than the tree size allows', () => {
    const env = envelope(0);
    const [sibling, extra] = [0xaa, 0xbb].map((n) => Buffer.alloc(32, n));
    const root = hex(computeNodeHash(computeNodeHash(specLeaf(env), sibling), extra));
    const proofHashes = [hex(sibling), hex(extra)];
    assert.equal(
      client().verifyInclusion(env, { leafIndex: 0, proofHashes, leafCount: 2 }, root),
      false,
    );
    assert.equal(
      client().verifyInclusion(env, { leafIndex: 0, proofHashes, leafCount: 4 }, root),
      true,
    );
  });

  it('rejects an empty path with leaf == root in a tree of more than one leaf', () => {
    const env = envelope(0);
    const root = hex(specLeaf(env));
    for (const leafCount of [2, 3, 8]) {
      assert.equal(
        client().verifyInclusion(env, { leafIndex: 0, proofHashes: [], leafCount }, root),
        false,
        `${leafCount} leaves`,
      );
    }
    assert.equal(
      client().verifyInclusion(env, { leafIndex: 0, proofHashes: [], leafCount: 1 }, root),
      true,
    );
  });

  it('rejects a leaf index at or beyond the tree size', () => {
    const env = envelope(5);
    const sibling = Buffer.alloc(32, 0xcc);
    const root = hex(computeNodeHash(specLeaf(env), sibling));
    const proofHashes = [hex(sibling)];
    // Index 4 walks exactly like index 0 for one level, but is not in a 2-leaf tree.
    for (const leafIndex of [2, 4, -2, 0.5, 2 ** 32]) {
      assert.equal(
        client().verifyInclusion(env, { leafIndex, proofHashes, leafCount: 2 }, root),
        false,
        `leafIndex ${leafIndex}`,
      );
    }
    assert.equal(
      client().verifyInclusion(env, { leafIndex: 0, proofHashes, leafCount: 2 }, root),
      true,
    );
  });

  it('rejects a proof with no usable tree size', () => {
    const env = envelope(0);
    const root = hex(specLeaf(env));
    for (const leafCount of [undefined, 0, -1, 1.5, '1', 2 ** 32]) {
      assert.equal(
        client().verifyInclusion(env, { leafIndex: 0, proofHashes: [], leafCount }, root),
        false,
        `leafCount ${String(leafCount)}`,
      );
    }
    assert.equal(client().verifyInclusion(env, { leafIndex: 0, leafCount: 1 }, root), false);
  });

  it('takes the tree size from the caller when given, and refuses a proof that disagrees', () => {
    const env = envelope(0);
    const root = hex(specLeaf(env));
    const bare = { leafIndex: 0, proofHashes: [] };
    assert.equal(client().verifyInclusion(env, bare, root, { leafCount: 1 }), true);
    assert.equal(client().verifyInclusion(env, bare, root, { leafCount: 4 }), false);
    assert.equal(
      client().verifyInclusion(env, { ...bare, leafCount: 1 }, root, { leafCount: 4 }),
      false,
    );
  });

  it('does not walk index bits past 31 back onto low bits', () => {
    // `leafIndex >> i` wraps at i = 32, so a 33-sibling path used to re-read
    // bit 0 at the top level. Build the root the wrapped walk would accept.
    const env = envelope(1);
    let node = specLeaf(env);
    const proofHashes = [];
    for (let i = 0; i < 33; i++) {
      const sibling = Buffer.alloc(32, i + 1);
      proofHashes.push(hex(sibling));
      const bit = i === 0 || i === 32 ? 1 : 0;
      node = bit ? computeNodeHash(sibling, node) : computeNodeHash(node, sibling);
    }
    assert.equal(
      client().verifyInclusion(env, { leafIndex: 1, proofHashes, leafCount: 2 }, hex(node)),
      false,
    );
  });

  it("does not take the sequence number from the proof's own leaf index", () => {
    // Sequence 3 sits at index 3 of its batch; the envelope does not say so.
    const { sequenceNumber, ...env } = envelope(3);
    const sibling = Buffer.alloc(32, 0xdd);
    const proof = { leafIndex: 3, proofHashes: [hex(sibling), hex(sibling)], leafCount: 4 };
    const root = hex(
      computeNodeHash(sibling, computeNodeHash(sibling, specLeaf(env, sequenceNumber))),
    );
    assert.equal(client().verifyInclusion(env, proof, root), false);
    // The caller's own knowledge of the sequence number is what binds it.
    assert.equal(client().verifyInclusion(env, proof, root, { sequenceNumber: 3 }), true);
    assert.equal(client().verifyInclusion(env, proof, root, { sequenceNumber: 4 }), false);
  });
});

// ---------------------------------------------------------------------------
// tools/sync.js `sync_verify_inclusion` and commands/sync.js `verify-inclusion`
// ---------------------------------------------------------------------------

/**
 * The leaf these two inline verifiers compute: SHA-256(payloadPlainHash ||
 * agentSignature). It is not the VES v1.0 leaf; these tests pin the path
 * shape only and deliberately leave the leaf as it is.
 */
function inlineLeaf(payloadPlainHash, agentSignature) {
  return nodeCrypto.createHash('sha256').update(payloadPlainHash).update(agentSignature).digest();
}

function inlineEnvelope(seed) {
  const payloadPlainHash = nodeCrypto.createHash('sha256').update(`payload-${seed}`).digest();
  const agentSignature = Buffer.concat([payloadPlainHash, payloadPlainHash]);
  return {
    envelope: {
      eventId: `evt-${seed}`,
      payloadPlainHash: hex(payloadPlainHash),
      agentSignature: hex(agentSignature),
    },
    leaf: inlineLeaf(payloadPlainHash, agentSignature),
  };
}

/** A padded tree of `size` leaves with the envelope's leaf at `index`. */
function inlineTree(leaf, size, index) {
  const leaves = Array.from({ length: size }, (_, i) =>
    i === index ? leaf : Buffer.alloc(32, i + 1),
  );
  const { proof, root } = pg.buildMerkleProof(leaves, index, computePadLeaf());
  return { proofHashes: proof.map((step) => step.hash), root: hex(root) };
}

const INLINE_VERIFIERS = {
  'sync_verify_inclusion tool': async (envelope, proof, expectedRoot) => {
    const tool = syncTools.find((t) => t.name === 'sync_verify_inclusion');
    return tool.handler({ params: { envelope, proof, expectedRoot } });
  },
  'sync verify-inclusion command': async (envelope, proof, expectedRoot) =>
    executeSyncCommand(
      'verify-inclusion',
      [JSON.stringify(envelope), JSON.stringify(proof), expectedRoot],
      { jsonOutput: true, output: () => {} },
    ),
};

for (const [name, verify] of Object.entries(INLINE_VERIFIERS)) {
  describe(`${name}: canonical path shape`, () => {
    for (const size of TREE_SIZES) {
      it(`accepts every legitimate proof of a ${size}-leaf tree, with and without a tree size`, async () => {
        for (let index = 0; index < size; index++) {
          const { envelope, leaf } = inlineEnvelope(`${size}-${index}`);
          const { proofHashes, root } = inlineTree(leaf, size, index);

          const bound = await verify(
            envelope,
            { leafIndex: index, proofHashes, leafCount: size },
            root,
          );
          assert.equal(bound.valid, true, `${size} leaves, index ${index}`);
          assert.equal(bound.pathBound, true);

          // A proof with no tree size verifies as before, and says so.
          const unbound = await verify(envelope, { leafIndex: index, proofHashes }, root);
          assert.equal(unbound.valid, true, `${size} leaves, index ${index}, no tree size`);
          assert.equal(unbound.pathBound, false);
        }
      });
    }

    it('rejects a path longer than the tree size allows', async () => {
      const { envelope, leaf } = inlineEnvelope('long');
      const [sibling, extra] = [0xaa, 0xbb].map((n) => Buffer.alloc(32, n));
      const root = hex(computeNodeHash(computeNodeHash(leaf, sibling), extra));
      const proofHashes = [hex(sibling), hex(extra)];
      const verdict = await verify(envelope, { leafIndex: 0, proofHashes, leafCount: 2 }, root);
      assert.equal(verdict.valid, false);
      // The same walk is a legitimate proof in a tree that deep.
      const deeper = await verify(envelope, { leafIndex: 0, proofHashes, leafCount: 4 }, root);
      assert.equal(deeper.valid, true);
    });

    it('rejects a path deeper than any tree can be, even with no tree size', async () => {
      const { envelope, leaf } = inlineEnvelope('deep');
      let node = leaf;
      const proofHashes = [];
      for (let i = 0; i < 33; i++) {
        const sibling = Buffer.alloc(32, i + 1);
        proofHashes.push(hex(sibling));
        node = computeNodeHash(node, sibling);
      }
      const verdict = await verify(envelope, { leafIndex: 0, proofHashes }, hex(node));
      assert.equal(verdict.valid, false);
    });

    it('rejects an empty path with leaf == root unless the tree has exactly one leaf', async () => {
      const { envelope, leaf } = inlineEnvelope('empty');
      const root = hex(leaf);
      for (const leafCount of [2, 3, 8]) {
        const verdict = await verify(envelope, { leafIndex: 0, proofHashes: [], leafCount }, root);
        assert.equal(verdict.valid, false, `${leafCount} leaves`);
      }
      const single = await verify(envelope, { leafIndex: 0, proofHashes: [], leafCount: 1 }, root);
      assert.equal(single.valid, true);
      assert.equal(single.pathBound, true);
    });

    it('rejects a leaf index outside the tree', async () => {
      const { envelope, leaf } = inlineEnvelope('index');
      // Three leaves padded to four: slot 3 is padding, not an event.
      const padded = inlineTree(leaf, 4, 3);
      const paddingSlot = await verify(
        envelope,
        { leafIndex: 3, proofHashes: padded.proofHashes, leafCount: 3 },
        padded.root,
      );
      assert.equal(paddingSlot.valid, false);

      // Index 2 (or 4) walks exactly like index 0 for one level.
      const sibling = Buffer.alloc(32, 0xcc);
      const root = hex(computeNodeHash(leaf, sibling));
      const proofHashes = [hex(sibling)];
      const inside = await verify(envelope, { leafIndex: 0, proofHashes, leafCount: 2 }, root);
      assert.equal(inside.valid, true);
      const outside = await verify(envelope, { leafIndex: 2, proofHashes, leafCount: 2 }, root);
      assert.equal(outside.valid, false);
      // With no tree size, the path still bounds the index: one sibling, two slots.
      const unaddressable = await verify(envelope, { leafIndex: 4, proofHashes }, root);
      assert.equal(unaddressable.valid, false);
      for (const leafIndex of [-2, 0.5, 2 ** 53]) {
        const verdict = await verify(envelope, { leafIndex, proofHashes, leafCount: 2 }, root);
        assert.equal(verdict.valid, false, `leafIndex ${leafIndex}`);
      }
    });

    it('rejects an unusable tree size', async () => {
      const { envelope, leaf } = inlineEnvelope('size');
      const root = hex(leaf);
      for (const leafCount of [0, -1, 1.5, 2 ** 32]) {
        const verdict = await verify(envelope, { leafIndex: 0, proofHashes: [], leafCount }, root);
        assert.equal(verdict.valid, false, `leafCount ${leafCount}`);
      }
    });
  });
}

describe('sync verify-inclusion command: human-readable output', () => {
  it('keeps its text line and carries pathBound on the structured result', async () => {
    const { envelope, leaf } = inlineEnvelope('text');
    const out = await executeSyncCommand(
      'verify-inclusion',
      [JSON.stringify(envelope), JSON.stringify({ leafIndex: 0, proofHashes: [] }), hex(leaf)],
      { jsonOutput: false, output: () => {} },
    );
    assert.equal(out.formatted, 'Inclusion proof: valid');
    assert.equal(out.result.valid, true);
    assert.equal(out.result.pathBound, false);
  });
});

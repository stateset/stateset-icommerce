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
 *
 * A tree size binds a path only when the caller supplies it. A size the
 * proof (or bundle) states about itself is chosen by whoever made the proof:
 * it is applied as a sanity bound, but `pathBound` stays false and
 * `leafCountSource` says 'proof' (or 'bundle').
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import nodeCrypto from 'node:crypto';

import { createProofGenerator } from '../../src/sync/proof-generator.js';
import { SequencerClient } from '../../src/sync/client.js';
import * as cryptoMod from '../../src/sync/crypto.js';
import { syncTools } from '../../src/tools/sync.js';
import { execute as executeSyncCommand } from '../../src/commands/sync.js';
import {
  decodeHash32,
  merklePathDepth,
  parseTreeInteger,
  resolveLeafCount,
} from '../../src/sync/merkle-shape.js';

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
          // The proof's own leafCount bounds the path but does not bind it.
          assert.equal(verdict.pathBound, false);
          assert.equal(verdict.leafCountSource, 'proof');
          const callerBound = pg.verifyInclusionProof(
            {
              format: proof.format,
              leafHash: proof.leaf,
              proof: proof.proof,
              expectedRoot: proof.root,
            },
            { leafCount: size },
          );
          assert.equal(callerBound.valid, true);
          assert.equal(callerBound.pathBound, true);
          assert.equal(callerBound.leafCountSource, 'caller');

          const bundle = pg.generateReceiptBundle(evt, events);
          const receipt = pg.verifyReceiptBundle(bundle);
          assert.equal(receipt.valid, true, `${format} bundle, ${size} leaves, index ${i}`);
          assert.equal(receipt.pathBound, false);
          assert.equal(receipt.leafCountSource, 'bundle');
          // A caller that knows the batch size independently binds the path.
          const bound = pg.verifyReceiptBundle(bundle, undefined, { leafCount: size });
          assert.equal(bound.valid, true);
          assert.equal(bound.pathBound, true);
          assert.equal(bound.leafCountSource, 'caller');
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
    const bare = { leafHash: hex(leaf), proof: [], expectedRoot: hex(leaf) };
    for (const leafCount of [2, 3, 8]) {
      assert.equal(pg.verifyInclusionProof({ ...bare, leafCount }).valid, false, `${leafCount}`);
      assert.equal(
        pg.verifyInclusionProof(bare, { leafCount }).valid,
        false,
        `caller ${leafCount}`,
      );
    }
    // A proof that calls itself a one-leaf tree is self-consistent, and says
    // only that: the size came from the proof, so the path is not bound.
    const selfStated = pg.verifyInclusionProof({ ...bare, leafCount: 1 });
    assert.equal(selfStated.valid, true);
    assert.equal(selfStated.pathBound, false);
    assert.equal(selfStated.leafCountSource, 'proof');
    const single = pg.verifyInclusionProof(bare, { leafCount: 1 });
    assert.equal(single.valid, true);
    assert.equal(single.pathBound, true);
    assert.equal(single.leafCountSource, 'caller');
    // A caller who knows the tree is bigger refuses the self-stated size.
    const disagreeing = pg.verifyInclusionProof({ ...bare, leafCount: 1 }, { leafCount: 4 });
    assert.equal(disagreeing.valid, false);
    assert.match(disagreeing.reason, /disagrees/);
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

  it('rejects a leaf or root that is not exactly a 32-byte hash', () => {
    const [a, b] = [1, 2].map((n) => Buffer.alloc(32, n));
    const root = computeNodeHash(b, a);
    const proof = [{ position: 'left', hash: hex(b) }];
    assert.equal(pg.verifyMerkleProof(hex(a), proof, hex(root)), true);
    assert.equal(pg.verifyMerkleProof(`0x${hex(a)}`, proof, `0x${hex(root)}`), true);
    // Buffer.from(hex) stops at the first non-hex pair; these used to verify.
    assert.equal(pg.verifyMerkleProof(`${hex(a)}zz`, proof, hex(root)), false);
    assert.equal(pg.verifyMerkleProof(hex(a), proof, `${hex(root)}zz`), false);
    assert.equal(pg.verifyMerkleProof(hex(a).slice(0, 62), proof, hex(root)), false);
    assert.equal(pg.verifyMerkleProof(Buffer.alloc(31, 1), proof, root), false);
    assert.equal(pg.verifyMerkleProof(null, proof, root), false);
    assert.equal(pg.verifyMerkleProof(a, proof, 'not-hex'), false);
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
    assert.equal(verdict.leafCountSource, null);

    const bundle = pg.generateReceiptBundle(events[1], events);
    delete bundle.leafCount;
    const receipt = pg.verifyReceiptBundle(bundle);
    assert.equal(receipt.valid, true);
    assert.equal(receipt.pathBound, false);
    assert.equal(receipt.leafCountSource, null);
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
    const onItsWord = pg.verifyReceiptBundle(forged);
    assert.equal(onItsWord.valid, true, 'self-consistent on its own word');
    // ...which the result does not pass off as a bound path.
    assert.equal(onItsWord.pathBound, false);
    assert.equal(onItsWord.leafCountSource, 'bundle');
    const verdict = pg.verifyReceiptBundle(forged, undefined, { leafCount: 4 });
    assert.equal(verdict.valid, false);
    assert.ok(verdict.checks.some((c) => c.check === 'inclusion_proof' && !c.passed));
  });

  it('a bundle size written as a canonical decimal string agrees with the same number', () => {
    const events = [0, 1, 2].map(legacyEvent);
    const bundle = {
      ...pg.generateReceiptBundle(events[1], events),
      leafCount: '3',
      leafIndex: '1',
    };
    assert.equal(pg.verifyReceiptBundle(bundle).valid, true);
    assert.equal(pg.verifyReceiptBundle(bundle, undefined, { leafCount: 3 }).valid, true);
    const noncanonical = { ...bundle, leafCount: '03' };
    const verdict = pg.verifyReceiptBundle(noncanonical, undefined, { leafCount: 3 });
    assert.equal(verdict.valid, false);
    const detail = verdict.checks.find((c) => c.check === 'inclusion_proof').detail;
    assert.match(detail, /not a usable tree size/);
  });

  it('rejects a sibling that is not exactly 32 bytes of hex instead of truncating it', () => {
    const [a, b] = [1, 2].map((n) => Buffer.alloc(32, n));
    const root = computeNodeHash(a, b);
    const step = (hash) => [{ position: 'right', hash }];
    assert.equal(pg.verifyMerkleProof(a, step(hex(b)), root, { leafCount: 2 }), true);
    assert.equal(pg.verifyMerkleProof(a, step(`0x${hex(b)}`), root, { leafCount: 2 }), true);
    // Buffer.from(s, 'hex') stops at the first non-hex character, so each of
    // these used to hash as `b` (or a prefix of it) and could verify.
    for (const hash of [
      `${hex(b)}zz`,
      `${hex(b)}0`,
      hex(b).slice(0, 63),
      `${hex(b)}00`,
      'zz',
      '',
    ]) {
      assert.equal(pg.verifyMerkleProof(a, step(hash), root), false, JSON.stringify(hash));
    }
    assert.equal(pg.verifyMerkleProof(a, step(undefined), root), false);
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
    const unusable = [undefined, null, 0, -1, 1.5, NaN, 1n, '01', ' 1', '1.0', '0x1', 2 ** 32];
    for (const leafCount of unusable) {
      assert.equal(
        client().verifyInclusion(env, { leafIndex: 0, proofHashes: [], leafCount }, root),
        false,
        `proof leafCount ${String(leafCount)}`,
      );
      if (leafCount === undefined || leafCount === null) continue;
      assert.equal(
        client().verifyInclusion(env, { leafIndex: 0, proofHashes: [] }, root, { leafCount }),
        false,
        `caller leafCount ${String(leafCount)}`,
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

  it('reports pathBound only for a caller-supplied tree size', () => {
    const env = envelope(0);
    const root = hex(specLeaf(env));
    const bare = { leafIndex: 0, proofHashes: [] };
    const c = client();
    assert.deepEqual(c.verifyInclusionDetailed(env, bare, root, { leafCount: 1 }), {
      valid: true,
      pathBound: true,
      leafCountSource: 'caller',
    });
    assert.deepEqual(c.verifyInclusionDetailed(env, { ...bare, leafCount: 1 }, root), {
      valid: true,
      pathBound: false,
      leafCountSource: 'proof',
    });
    assert.deepEqual(
      c.verifyInclusionDetailed(env, { ...bare, leafCount: 1 }, root, { leafCount: 1 }),
      { valid: true, pathBound: true, leafCountSource: 'caller' },
    );
    const disagreeing = c.verifyInclusionDetailed(env, { ...bare, leafCount: 1 }, root, {
      leafCount: 2,
    });
    assert.equal(disagreeing.valid, false);
    assert.match(disagreeing.reason, /disagrees/);
    const none = c.verifyInclusionDetailed(env, bare, root);
    assert.equal(none.valid, false);
    assert.equal(none.leafCountSource, null);
  });

  it('accepts canonical decimal strings for leafIndex and leafCount, as gRPC loads uint64', () => {
    const env = envelope(1);
    const sibling = Buffer.alloc(32, 0xee);
    const root = hex(computeNodeHash(sibling, specLeaf(env)));
    // The gRPC client loads longs as strings and proof_hashes as bytes.
    const grpcStyle = { leafIndex: '1', leafCount: '2', proofHashes: [sibling] };
    assert.equal(client().verifyInclusion(env, grpcStyle, root), true);
    assert.equal(client().verifyInclusion(env, grpcStyle, root, { leafCount: '2' }), true);
    assert.equal(client().verifyInclusion(env, grpcStyle, root, { leafCount: 2 }), true);
    for (const leafIndex of ['01', ' 1', '1 ', '+1', '1.0', '0x1', '1e0', '', 1n]) {
      assert.equal(
        client().verifyInclusion(env, { ...grpcStyle, leafIndex }, root),
        false,
        `leafIndex ${JSON.stringify(String(leafIndex))}`,
      );
    }
    assert.equal(client().verifyInclusion(env, { ...grpcStyle, leafCount: '02' }, root), false);
  });

  it('rejects a sibling that is not exactly 32 bytes of hex instead of truncating it', () => {
    const env = envelope(0);
    const sibling = Buffer.alloc(32, 0xab);
    const root = hex(computeNodeHash(specLeaf(env), sibling));
    const proof = (hash) => ({ leafIndex: 0, leafCount: 2, proofHashes: [hash] });
    assert.equal(client().verifyInclusion(env, proof(hex(sibling)), root), true);
    assert.equal(client().verifyInclusion(env, proof(`0x${hex(sibling)}`), root), true);
    assert.equal(client().verifyInclusion(env, proof(sibling), root), true);
    // Each of these used to be cut down to `sibling` (or a prefix) by
    // Buffer.from(s, 'hex'); the first two would then have verified.
    for (const hash of [
      `${hex(sibling)}zz`,
      `${hex(sibling)}0`,
      `${hex(sibling)}00`,
      hex(sibling).slice(0, 62),
      Buffer.alloc(31, 0xab),
      42,
      null,
    ]) {
      assert.equal(
        client().verifyInclusion(env, proof(hash), root),
        false,
        `sibling ${String(hash).slice(0, 20)}`,
      );
    }
  });

  it('returns invalid instead of throwing on a null proof, options or envelope', () => {
    const env = envelope(0);
    const root = hex(specLeaf(env));
    const proof = { leafIndex: 0, proofHashes: [], leafCount: 1 };
    const c = client();
    assert.equal(c.verifyInclusion(env, null, root, { leafCount: 1 }), false);
    assert.equal(c.verifyInclusion(env, undefined, root), false);
    assert.equal(c.verifyInclusion(env, 'not-a-proof', root, { leafCount: 1 }), false);
    // null options are no options: the proof's own size still applies.
    assert.equal(c.verifyInclusion(env, proof, root, null), true);
    assert.equal(c.verifyInclusion(null, proof, root, { leafCount: 1 }), false);
    assert.deepEqual(c.verifyInclusionDetailed(env, null, root, null), {
      valid: false,
      pathBound: false,
      leafCountSource: null,
      reason: 'no usable leaf count',
    });
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
 * agentSignature). It is not the VES v1.0 leaf. It has no domain prefix, so
 * the verifiers fix the lengths of its inputs (32-byte payload hash, 64- or
 * 3309-byte signature) to keep its preimage from ever spelling a node's.
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

/**
 * Each inline verifier, called the way its users call it. `leafCount` is the
 * caller-supplied tree size: the tool's top-level `leafCount` input, the
 * command's `--leaf-count` option.
 */
const INLINE_VERIFIERS = {
  'sync_verify_inclusion tool': async (envelope, proof, expectedRoot, leafCount) => {
    const tool = syncTools.find((t) => t.name === 'sync_verify_inclusion');
    return tool.handler({ params: { envelope, proof, expectedRoot, leafCount } });
  },
  'sync verify-inclusion command': async (envelope, proof, expectedRoot, leafCount) =>
    executeSyncCommand(
      'verify-inclusion',
      [
        JSON.stringify(envelope),
        JSON.stringify(proof),
        expectedRoot,
        ...(leafCount === undefined ? [] : ['--leaf-count', String(leafCount)]),
      ],
      { jsonOutput: true, output: () => {} },
    ),
};

for (const [name, verify] of Object.entries(INLINE_VERIFIERS)) {
  describe(`${name}: canonical path shape`, () => {
    for (const size of TREE_SIZES) {
      it(`accepts every legitimate proof of a ${size}-leaf tree, however the size is known`, async () => {
        for (let index = 0; index < size; index++) {
          const { envelope, leaf } = inlineEnvelope(`${size}-${index}`);
          const { proofHashes, root } = inlineTree(leaf, size, index);
          const proof = { leafIndex: index, proofHashes };
          const label = `${size} leaves, index ${index}`;

          // The caller's own count binds the path.
          const bound = await verify(envelope, proof, root, size);
          assert.equal(bound.valid, true, label);
          assert.equal(bound.pathBound, true);
          assert.equal(bound.leafCountSource, 'caller');

          // The proof's own count bounds the path, but does not bind it.
          const selfStated = await verify(envelope, { ...proof, leafCount: size }, root);
          assert.equal(selfStated.valid, true, `${label}, proof leafCount`);
          assert.equal(selfStated.pathBound, false);
          assert.equal(selfStated.leafCountSource, 'proof');

          // Both, agreeing: the caller's is the one that counts.
          const both = await verify(envelope, { ...proof, leafCount: size }, root, size);
          assert.equal(both.valid, true, `${label}, both`);
          assert.equal(both.pathBound, true);
          assert.equal(both.leafCountSource, 'caller');

          // Neither: verifies as before, and says so.
          const unbound = await verify(envelope, proof, root);
          assert.equal(unbound.valid, true, `${label}, no tree size`);
          assert.equal(unbound.pathBound, false);
          assert.equal(unbound.leafCountSource, null);
        }
      });
    }

    it('rejects a proof whose leafCount disagrees with the caller-supplied one', async () => {
      const { envelope, leaf } = inlineEnvelope('disagree');
      const { proofHashes, root } = inlineTree(leaf, 4, 1);
      const proof = { leafIndex: 1, proofHashes, leafCount: 4 };
      assert.equal((await verify(envelope, proof, root, 4)).valid, true);
      for (const callerCount of [3, 5]) {
        const verdict = await verify(envelope, proof, root, callerCount);
        assert.equal(verdict.valid, false, `caller says ${callerCount}`);
        assert.equal(verdict.pathBound, true);
        assert.equal(verdict.leafCountSource, 'caller');
        assert.match(verdict.reason, /disagrees/);
      }
    });

    it('accepts 0x-prefixed and upper-case hex, and a 3309-byte ML-DSA-65 signature', async () => {
      const payloadPlainHash = Buffer.alloc(32, 0x5a);
      for (const signatureLength of [64, 3309]) {
        const agentSignature = Buffer.alloc(signatureLength, 0xc3);
        const leaf = inlineLeaf(payloadPlainHash, agentSignature);
        const sibling = Buffer.alloc(32, 0x11);
        const root = hex(computeNodeHash(leaf, sibling));
        const spellings = [
          { p: hex(payloadPlainHash), s: hex(agentSignature), sib: hex(sibling) },
          {
            p: `0x${hex(payloadPlainHash)}`,
            s: `0x${hex(agentSignature)}`,
            sib: `0x${hex(sibling)}`,
          },
          {
            p: hex(payloadPlainHash).toUpperCase(),
            s: hex(agentSignature).toUpperCase(),
            sib: hex(sibling).toUpperCase(),
          },
        ];
        for (const { p, s, sib } of spellings) {
          const verdict = await verify(
            { eventId: 'evt-spelling', payloadPlainHash: p, agentSignature: s },
            { leafIndex: 0, proofHashes: [sib] },
            root,
            2,
          );
          assert.equal(verdict.valid, true, `${signatureLength}-byte signature, ${p.slice(0, 4)}`);
        }
      }
    });

    it('rejects leaf material of any other length, even when the walk itself matches', async () => {
      // Each case hashes to a leaf that the root really is built on, so only
      // the length rule can refuse it.
      const cases = [
        ['31-byte payload hash', Buffer.alloc(31, 1), Buffer.alloc(64, 2)],
        ['33-byte payload hash', Buffer.alloc(33, 1), Buffer.alloc(64, 2)],
        ['11-byte payload hash (a domain tag)', Buffer.from('VES_NODE_V1'), Buffer.alloc(64, 2)],
        ['32-byte signature (the REST placeholder)', Buffer.alloc(32, 1), Buffer.alloc(32, 0)],
        ['43-byte signature', Buffer.alloc(32, 1), Buffer.alloc(43, 2)],
        ['63-byte signature', Buffer.alloc(32, 1), Buffer.alloc(63, 2)],
        ['65-byte signature', Buffer.alloc(32, 1), Buffer.alloc(65, 2)],
        ['3308-byte signature', Buffer.alloc(32, 1), Buffer.alloc(3308, 2)],
      ];
      for (const [label, payloadPlainHash, agentSignature] of cases) {
        const root = hex(inlineLeaf(payloadPlainHash, agentSignature));
        const verdict = await verify(
          {
            eventId: 'evt-length',
            payloadPlainHash: hex(payloadPlainHash),
            agentSignature: hex(agentSignature),
          },
          { leafIndex: 0, proofHashes: [] },
          root,
          1,
        );
        assert.equal(verdict.valid, false, label);
        assert.match(verdict.reason, /payloadPlainHash|agentSignature/, label);
      }
      // Malformed hex is refused rather than cut short.
      const { envelope, leaf } = inlineEnvelope('malformed');
      for (const field of ['payloadPlainHash', 'agentSignature']) {
        for (const spoil of [(v) => `${v}zz`, (v) => `${v}0`, (v) => v.slice(1), () => '']) {
          const verdict = await verify(
            { ...envelope, [field]: spoil(envelope[field]) },
            { leafIndex: 0, proofHashes: [] },
            hex(leaf),
            1,
          );
          assert.equal(verdict.valid, false, `${field} ${spoil.toString()}`);
        }
      }
    });

    it('rejects a sibling that is not exactly 32 bytes of hex instead of truncating it', async () => {
      const { envelope, leaf } = inlineEnvelope('sibling');
      const sibling = Buffer.alloc(32, 0x42);
      const root = hex(computeNodeHash(leaf, sibling));
      const verdictFor = (hash) => verify(envelope, { leafIndex: 0, proofHashes: [hash] }, root, 2);
      assert.equal((await verdictFor(hex(sibling))).valid, true);
      for (const hash of [`${hex(sibling)}zz`, `${hex(sibling)}0`, hex(sibling).slice(0, 62), '']) {
        const verdict = await verdictFor(hash);
        assert.equal(verdict.valid, false, JSON.stringify(hash));
        assert.match(verdict.reason, /32-byte/);
      }
    });

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
        const caller = await verify(envelope, { leafIndex: 0, proofHashes: [] }, root, leafCount);
        assert.equal(caller.valid, false, `caller: ${leafCount} leaves`);
      }
      const selfStated = await verify(
        envelope,
        { leafIndex: 0, proofHashes: [], leafCount: 1 },
        root,
      );
      assert.equal(selfStated.valid, true);
      assert.equal(selfStated.pathBound, false);
      assert.equal(selfStated.leafCountSource, 'proof');
      const single = await verify(envelope, { leafIndex: 0, proofHashes: [] }, root, 1);
      assert.equal(single.valid, true);
      assert.equal(single.pathBound, true);
      assert.equal(single.leafCountSource, 'caller');
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
      for (const leafCount of [0, -1, 1.5, 2 ** 32, '01', ' 1', 'one']) {
        const verdict = await verify(envelope, { leafIndex: 0, proofHashes: [], leafCount }, root);
        assert.equal(verdict.valid, false, `leafCount ${leafCount}`);
      }
    });
  });
}

describe('sync_verify_inclusion tool: an unusable caller-supplied tree size', () => {
  it('is invalid, not trusted', async () => {
    const { envelope, leaf } = inlineEnvelope('caller-size');
    const verify = INLINE_VERIFIERS['sync_verify_inclusion tool'];
    for (const leafCount of [0, -1, 1.5, 2 ** 32, '01']) {
      const verdict = await verify(
        envelope,
        { leafIndex: 0, proofHashes: [] },
        hex(leaf),
        leafCount,
      );
      assert.equal(verdict.valid, false, `leafCount ${leafCount}`);
      assert.equal(verdict.leafCountSource, 'caller');
    }
  });

  it('declares the caller-supplied leafCount as its own input', () => {
    const tool = syncTools.find((t) => t.name === 'sync_verify_inclusion');
    assert.ok(tool.inputSchema.leafCount, 'top-level leafCount input');
    assert.equal(tool.inputSchema.leafCount.safeParse(undefined).success, true);
    assert.equal(tool.inputSchema.leafCount.safeParse(4).success, true);
    assert.equal(tool.inputSchema.leafCount.safeParse(0).success, false);
  });
});

describe('sync verify-inclusion command: --leaf-count', () => {
  const { envelope, leaf } = inlineEnvelope('flag');
  const sibling = Buffer.alloc(32, 0x77);
  const root = hex(computeNodeHash(leaf, sibling));
  const proofJson = JSON.stringify({ leafIndex: 0, proofHashes: [hex(sibling)] });
  const run = (args) =>
    executeSyncCommand('verify-inclusion', args, { jsonOutput: true, output: () => {} });

  it('takes --leaf-count <n>, --leaf-count=<n>, before or after the positionals', async () => {
    const env = JSON.stringify(envelope);
    for (const args of [
      [env, proofJson, root, '--leaf-count', '2'],
      [env, proofJson, root, '--leaf-count=2'],
      ['--leaf-count', '2', env, proofJson, root],
    ]) {
      const verdict = await run(args);
      assert.equal(verdict.valid, true, args.join(' ').slice(-20));
      assert.equal(verdict.pathBound, true);
      assert.equal(verdict.leafCountSource, 'caller');
    }
    const wrongSize = await run([env, proofJson, root, '--leaf-count', '4']);
    assert.equal(wrongSize.valid, false);
  });

  it('refuses a missing, repeated or unusable --leaf-count with a usage error', async () => {
    const env = JSON.stringify(envelope);
    await assert.rejects(run([env, proofJson, root, '--leaf-count']), /Usage/);
    await assert.rejects(
      run([env, proofJson, root, '--leaf-count', '2', '--leaf-count=2']),
      /Usage/,
    );
    for (const value of ['0', '-1', '1.5', '02', 'abc', '', '4294967296']) {
      await assert.rejects(
        run([env, proofJson, root, `--leaf-count=${value}`]),
        /--leaf-count must be/,
        `--leaf-count=${value}`,
      );
    }
  });
});

describe('interior-node forgery against the inline verifiers (review finding #1)', () => {
  // A real 4-leaf tree.
  const L = [1, 2, 3, 4].map((n) => Buffer.alloc(32, n));
  const n01 = computeNodeHash(L[0], L[1]);
  const n23 = computeNodeHash(L[2], L[3]);
  const root = hex(computeNodeHash(n01, n23));
  const NODE_DOMAIN = Buffer.from('VES_NODE_V1');

  for (const [name, verify] of Object.entries(INLINE_VERIFIERS)) {
    it(`${name}: refuses the root itself passed off as an envelope's leaf`, async () => {
      // SHA-256(payloadPlainHash || agentSignature) with these fields is
      // computeNodeHash(n01, n23), the root, for an eventId of the forger's choosing.
      const agentSignature = Buffer.concat([n01, n23]);
      assert.equal(hex(inlineLeaf(NODE_DOMAIN, agentSignature)), root, 'the forgery is real');
      const forged = {
        eventId: 'forged-1',
        payloadPlainHash: hex(NODE_DOMAIN),
        agentSignature: hex(agentSignature),
      };
      // The exact scenario from the review: before the fix this returned
      // { valid: true, pathBound: true }.
      const reviewed = await verify(forged, { leafIndex: 0, proofHashes: [], leafCount: 1 }, root);
      assert.equal(reviewed.valid, false);
      assert.equal(reviewed.pathBound, false);
      assert.equal(reviewed.leafCountSource, 'proof');
      assert.match(reviewed.reason, /payloadPlainHash/);
      assert.equal((await verify(forged, { leafIndex: 0, proofHashes: [] }, root)).valid, false);
      // And the true batch size from the caller refuses it on shape alone.
      const trueSize = await verify(
        forged,
        { leafIndex: 0, proofHashes: [], leafCount: 1 },
        root,
        4,
      );
      assert.equal(trueSize.valid, false);
    });

    it(`${name}: refuses an interior node passed off as a leaf one level down`, async () => {
      const agentSignature = Buffer.concat([L[0], L[1]]);
      assert.ok(inlineLeaf(NODE_DOMAIN, agentSignature).equals(n01), 'the forgery is real');
      const forged = {
        eventId: 'forged-2',
        payloadPlainHash: hex(NODE_DOMAIN),
        agentSignature: hex(agentSignature),
      };
      const proof = { leafIndex: 0, proofHashes: [hex(n23)], leafCount: 2 };
      const verdict = await verify(forged, proof, root);
      assert.equal(verdict.valid, false);
      assert.equal(verdict.pathBound, false);
    });

    it(`${name}: refuses the forgery re-cut around a 32-byte payload hash`, async () => {
      // NODE_DOMAIN || L0 || L1 split as 32 + 43 bytes: the payload hash has
      // the right length, so only the signature length rule stops it.
      const preimage = Buffer.concat([NODE_DOMAIN, L[0], L[1]]);
      const payloadPlainHash = preimage.subarray(0, 32);
      const agentSignature = preimage.subarray(32);
      assert.equal(agentSignature.length, 43);
      assert.ok(inlineLeaf(payloadPlainHash, agentSignature).equals(n01), 'the forgery is real');
      const verdict = await verify(
        {
          eventId: 'forged-3',
          payloadPlainHash: hex(payloadPlainHash),
          agentSignature: hex(agentSignature),
        },
        { leafIndex: 0, proofHashes: [hex(n23)] },
        root,
        2,
      );
      assert.equal(verdict.valid, false);
      assert.match(verdict.reason, /agentSignature/);
    });
  }
});

describe('merkle-shape: the one parser for leaf indexes and counts', () => {
  it('accepts nonnegative safe integers and their canonical decimal spelling', () => {
    for (const [value, expected] of [
      [0, 0],
      [-0, 0],
      [5, 5],
      ['0', 0],
      ['5', 5],
      ['4294967295', 4294967295],
      [String(Number.MAX_SAFE_INTEGER), Number.MAX_SAFE_INTEGER],
      [Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER],
    ]) {
      assert.equal(parseTreeInteger(value), expected, JSON.stringify(String(value)));
    }
    assert.ok(Object.is(parseTreeInteger(-0), 0));
  });

  it('refuses every other value', () => {
    const refused = [
      '05',
      '00',
      ' 5',
      '5 ',
      '+5',
      '-5',
      '-0',
      '5.0',
      '5.',
      '0x5',
      '1e3',
      '',
      '٥', // a non-ASCII digit
      '9007199254740992',
      '9007199254740993',
      '1'.repeat(400),
      -1,
      1.5,
      NaN,
      Infinity,
      -Infinity,
      2 ** 53,
      5n,
      null,
      undefined,
      true,
      {},
      [5],
    ];
    for (const value of refused) {
      assert.equal(
        parseTreeInteger(value),
        null,
        typeof value === 'string' ? JSON.stringify(value) : String(value),
      );
    }
  });

  it('merklePathDepth reads its argument with the same parser', () => {
    assert.equal(merklePathDepth('4'), 2);
    assert.equal(merklePathDepth('5'), 3);
    assert.equal(merklePathDepth('04'), null);
    assert.equal(merklePathDepth(4n), null);
    assert.equal(merklePathDepth('4294967296'), null);
  });

  it('decodeHash32 takes exactly 32 bytes', () => {
    const bytes = Buffer.alloc(32, 0x9c);
    assert.ok(decodeHash32(hex(bytes)).equals(bytes));
    assert.ok(decodeHash32(`0x${hex(bytes)}`).equals(bytes));
    assert.ok(decodeHash32(hex(bytes).toUpperCase()).equals(bytes));
    assert.ok(decodeHash32(new Uint8Array(bytes)).equals(bytes));
    for (const bad of [
      `${hex(bytes)}0`,
      hex(bytes).slice(2),
      `0X${hex(bytes)}`,
      `${hex(bytes)}zz`,
      Buffer.alloc(33),
      7,
      null,
    ]) {
      assert.equal(decodeHash32(bad), null);
    }
  });

  it('resolveLeafCount reports where the size came from, and binds only the caller', () => {
    assert.deepEqual(resolveLeafCount({}), {
      ok: true,
      leafCount: null,
      depth: null,
      leafCountSource: null,
      pathBound: false,
    });
    assert.deepEqual(resolveLeafCount({ statedLeafCount: 5 }), {
      ok: true,
      leafCount: 5,
      depth: 3,
      leafCountSource: 'proof',
      pathBound: false,
    });
    assert.equal(
      resolveLeafCount({ statedLeafCount: '5', statedSource: 'bundle' }).leafCountSource,
      'bundle',
    );
    assert.deepEqual(resolveLeafCount({ callerLeafCount: '5', statedLeafCount: 5 }), {
      ok: true,
      leafCount: 5,
      depth: 3,
      leafCountSource: 'caller',
      pathBound: true,
    });
    assert.equal(resolveLeafCount({ callerLeafCount: 4, statedLeafCount: 5 }).ok, false);
    assert.equal(resolveLeafCount({ callerLeafCount: 5, statedLeafCount: 'x' }).ok, false);
    assert.equal(resolveLeafCount({ callerLeafCount: 0 }).ok, false);
  });
});

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

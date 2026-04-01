/**
 * Tests for VES Receipt Verification Tools
 *
 * Tests sync_verify_receipt, sync_verify_inclusion, and sync_inspect_commitment
 * tool definitions and handlers.
 */

import { describe, it, before } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'crypto';

// Import the tools
import { syncTools } from '../../src/tools/sync.js';
import {
  computeEventSigningHash,
  generateHybridSigningKeypair,
  hasNativeHybridPqcVerificationSupport,
  signEventHash,
  signEventHashHybrid,
  verifyEventSignature,
  hexToBuffer,
  bufferToHex,
  computeNodeHash,
  ZERO_HASH,
} from '../../src/sync/crypto.js';
import { SIGNATURE_SCHEME_ED25519_ML_DSA_65 } from '../../src/sync/pqc.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function findTool(name) {
  return syncTools.find((t) => t.name === name);
}

/**
 * Generate a test Ed25519 keypair and return raw 32-byte buffers.
 */
function generateEd25519TestKeypair() {
  const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
  const pubDer = publicKey.export({ type: 'spki', format: 'der' });
  const privDer = privateKey.export({ type: 'pkcs8', format: 'der' });
  return {
    publicKey: pubDer.subarray(-32),
    privateKey: privDer.subarray(-32),
  };
}

/**
 * Build a valid VES envelope signed with the given keypair.
 */
function buildSignedEnvelope(keyPair) {
  const tenantId = '00000000-0000-4000-8000-000000000001';
  const storeId = '00000000-0000-4000-8000-000000000002';
  const eventId = '00000000-0000-4000-8000-000000000003';
  const sourceAgent = '00000000-0000-4000-8000-000000000004';

  const payloadPlainHash = crypto.createHash('sha256').update('test-payload').digest();
  const payloadCipherHash = ZERO_HASH;

  const signingHash = computeEventSigningHash({
    vesVersion: 1,
    tenantId,
    storeId,
    eventId,
    sourceAgentId: sourceAgent,
    agentKeyId: 1,
    entityType: 'order',
    entityId: 'ORD-001',
    eventType: 'order.created',
    createdAt: '2026-03-01T00:00:00Z',
    payloadKind: 0,
    payloadPlainHash,
    payloadCipherHash,
  });

  const signature = signEventHash(signingHash, keyPair.privateKey);

  return {
    eventId,
    tenantId,
    storeId,
    sourceAgent,
    agentKeyId: 1,
    entityType: 'order',
    entityId: 'ORD-001',
    eventType: 'order.created',
    createdAt: '2026-03-01T00:00:00Z',
    payloadPlainHash: bufferToHex(payloadPlainHash),
    payloadCipherHash: bufferToHex(payloadCipherHash),
    agentSignature: bufferToHex(signature),
    vesVersion: 1,
  };
}

function buildHybridSignedEnvelope(keyPair) {
  const envelope = buildSignedEnvelope({
    publicKey: keyPair.ed25519PublicKey,
    privateKey: keyPair.ed25519PrivateKey,
  });
  const signingHash = computeEventSigningHash({
    vesVersion: envelope.vesVersion,
    tenantId: envelope.tenantId,
    storeId: envelope.storeId,
    eventId: envelope.eventId,
    sourceAgentId: envelope.sourceAgent,
    agentKeyId: envelope.agentKeyId,
    entityType: envelope.entityType,
    entityId: envelope.entityId,
    eventType: envelope.eventType,
    createdAt: envelope.createdAt,
    payloadKind: 0,
    payloadPlainHash: hexToBuffer(envelope.payloadPlainHash),
    payloadCipherHash: hexToBuffer(envelope.payloadCipherHash),
  });

  const signatureBundle = signEventHashHybrid(signingHash, {
    ed25519PrivateKey: keyPair.ed25519PrivateKey,
    mlDsa65Seed: keyPair.mlDsa65Seed,
  });

  return {
    ...envelope,
    agentSignatureScheme: SIGNATURE_SCHEME_ED25519_ML_DSA_65,
    agentSignature: bufferToHex(signatureBundle.ed25519Signature),
    agentSignatureBundle: {
      ed25519Signature: bufferToHex(signatureBundle.ed25519Signature),
      mlDsa65Signature: bufferToHex(signatureBundle.mlDsa65Signature),
    },
  };
}

// ===========================================================================
// Tool Definition Tests
// ===========================================================================

describe('VES Receipt Verification Tools — Definitions', () => {
  it('sync_verify_receipt tool exists', () => {
    const tool = findTool('sync_verify_receipt');
    assert.ok(tool, 'sync_verify_receipt should exist in syncTools');
  });

  it('sync_verify_receipt has permission read', () => {
    const tool = findTool('sync_verify_receipt');
    assert.equal(tool.permission, 'read');
  });

  it('sync_verify_receipt has correct schema fields', () => {
    const tool = findTool('sync_verify_receipt');
    assert.ok(tool.inputSchema.envelope, 'should have envelope schema');
    assert.ok(tool.inputSchema.publicKeyHex, 'should have publicKeyHex schema');
  });

  it('sync_verify_inclusion tool exists', () => {
    const tool = findTool('sync_verify_inclusion');
    assert.ok(tool, 'sync_verify_inclusion should exist in syncTools');
  });

  it('sync_verify_inclusion has permission read', () => {
    const tool = findTool('sync_verify_inclusion');
    assert.equal(tool.permission, 'read');
  });

  it('sync_verify_inclusion has correct schema fields', () => {
    const tool = findTool('sync_verify_inclusion');
    assert.ok(tool.inputSchema.envelope, 'should have envelope schema');
    assert.ok(tool.inputSchema.proof, 'should have proof schema');
    assert.ok(tool.inputSchema.expectedRoot, 'should have expectedRoot schema');
  });

  it('sync_inspect_commitment tool exists', () => {
    const tool = findTool('sync_inspect_commitment');
    assert.ok(tool, 'sync_inspect_commitment should exist in syncTools');
  });

  it('sync_inspect_commitment has permission read', () => {
    const tool = findTool('sync_inspect_commitment');
    assert.equal(tool.permission, 'read');
  });

  it('sync_inspect_commitment has correct schema fields', () => {
    const tool = findTool('sync_inspect_commitment');
    assert.ok(tool.inputSchema.batchId, 'should have batchId schema');
  });

  it('sync_verify_receipt has correct description', () => {
    const tool = findTool('sync_verify_receipt');
    assert.ok(tool.description.includes('Ed25519'));
    assert.ok(tool.description.includes('signature'));
  });

  it('sync_verify_inclusion has correct description', () => {
    const tool = findTool('sync_verify_inclusion');
    assert.ok(tool.description.includes('Merkle'));
  });

  it('sync_inspect_commitment has correct description', () => {
    const tool = findTool('sync_inspect_commitment');
    assert.ok(tool.description.includes('commitment'));
  });
});

// ===========================================================================
// sync_verify_receipt Handler Tests
// ===========================================================================

describe('sync_verify_receipt — Handler', () => {
  let keyPair;
  let envelope;

  before(() => {
    keyPair = generateEd25519TestKeypair();
    envelope = buildSignedEnvelope(keyPair);
  });

  it('returns valid=true for a correctly signed envelope', async () => {
    const tool = findTool('sync_verify_receipt');
    const result = await tool.handler({
      params: {
        envelope,
        publicKeyHex: bufferToHex(keyPair.publicKey),
      },
    });
    assert.equal(result.valid, true);
    assert.equal(result.eventId, envelope.eventId);
    assert.equal(result.sourceAgent, envelope.sourceAgent);
    assert.equal(result.entityType, 'order');
    assert.equal(result.entityId, 'ORD-001');
  });

  it('returns valid=false for a wrong public key', async () => {
    const wrongKeyPair = generateEd25519TestKeypair();
    const tool = findTool('sync_verify_receipt');
    const result = await tool.handler({
      params: {
        envelope,
        publicKeyHex: bufferToHex(wrongKeyPair.publicKey),
      },
    });
    assert.equal(result.valid, false);
    assert.equal(result.eventId, envelope.eventId);
  });

  it('returns valid=false for a tampered envelope', async () => {
    const tampered = { ...envelope, entityId: 'ORD-FAKE' };
    const tool = findTool('sync_verify_receipt');
    const result = await tool.handler({
      params: {
        envelope: tampered,
        publicKeyHex: bufferToHex(keyPair.publicKey),
      },
    });
    assert.equal(result.valid, false);
  });

  it('returns valid=false for a tampered agentSignature', async () => {
    const fakeSignature = bufferToHex(crypto.randomBytes(64));
    const tampered = { ...envelope, agentSignature: fakeSignature };
    const tool = findTool('sync_verify_receipt');
    const result = await tool.handler({
      params: {
        envelope: tampered,
        publicKeyHex: bufferToHex(keyPair.publicKey),
      },
    });
    assert.equal(result.valid, false);
  });

  it('handles 0x-prefixed public key hex', async () => {
    const tool = findTool('sync_verify_receipt');
    const pubHex = '0x' + keyPair.publicKey.toString('hex');
    const result = await tool.handler({
      params: {
        envelope,
        publicKeyHex: pubHex,
      },
    });
    assert.equal(result.valid, true);
  });

  it('returns all expected fields', async () => {
    const tool = findTool('sync_verify_receipt');
    const result = await tool.handler({
      params: { envelope, publicKeyHex: bufferToHex(keyPair.publicKey) },
    });
    assert.ok('valid' in result);
    assert.ok('eventId' in result);
    assert.ok('sourceAgent' in result);
    assert.ok('entityType' in result);
    assert.ok('entityId' in result);
  });

  it(
    'returns valid=true for a correctly signed hybrid envelope',
    { skip: !hasNativeHybridPqcVerificationSupport() },
    async () => {
      const hybridKeyPair = generateHybridSigningKeypair();
      const hybridEnvelope = buildHybridSignedEnvelope(hybridKeyPair);
      const tool = findTool('sync_verify_receipt');
      const result = await tool.handler({
        params: {
          envelope: hybridEnvelope,
          publicKeyBundle: {
            ed25519PublicKey: bufferToHex(hybridKeyPair.ed25519PublicKey),
            mlDsa65PublicKey: bufferToHex(hybridKeyPair.mlDsa65PublicKey),
          },
        },
      });
      assert.equal(result.valid, true);
    },
  );

  it(
    'returns valid=false when the ML-DSA component of a hybrid envelope is tampered',
    { skip: !hasNativeHybridPqcVerificationSupport() },
    async () => {
      const hybridKeyPair = generateHybridSigningKeypair();
      const hybridEnvelope = buildHybridSignedEnvelope(hybridKeyPair);
      const tamperedEnvelope = {
        ...hybridEnvelope,
        agentSignatureBundle: {
          ...hybridEnvelope.agentSignatureBundle,
          mlDsa65Signature: bufferToHex(crypto.randomBytes(32)),
        },
      };

      const tool = findTool('sync_verify_receipt');
      const result = await tool.handler({
        params: {
          envelope: tamperedEnvelope,
          publicKeyBundle: {
            ed25519PublicKey: bufferToHex(hybridKeyPair.ed25519PublicKey),
            mlDsa65PublicKey: bufferToHex(hybridKeyPair.mlDsa65PublicKey),
          },
        },
      });
      assert.equal(result.valid, false);
    },
  );
});

// ===========================================================================
// sync_verify_inclusion Handler Tests
// ===========================================================================

describe('sync_verify_inclusion — Handler', () => {
  it('verifies a valid 2-leaf Merkle inclusion proof (leaf 0)', async () => {
    // Build a simple 2-leaf tree
    const leaf0 = crypto.createHash('sha256').update('leaf-0-data').digest();
    const leaf1 = crypto.createHash('sha256').update('leaf-1-data').digest();
    const root = computeNodeHash(leaf0, leaf1);

    // Create an envelope whose payloadPlainHash || agentSignature hashes to leaf0
    // We cheat: construct payloadPlainHash and agentSignature so that
    // H(payloadPlainHash || agentSignature) = leaf0
    // Instead, let's build leaf0 from known payloadPlainHash + agentSignature
    const payloadPlainHash = crypto.randomBytes(32);
    const agentSignature = crypto.randomBytes(64);
    const computedLeaf = crypto.createHash('sha256')
      .update(payloadPlainHash)
      .update(agentSignature)
      .digest();
    const otherLeaf = crypto.createHash('sha256').update('other-leaf').digest();
    const computedRoot = computeNodeHash(computedLeaf, otherLeaf);

    const tool = findTool('sync_verify_inclusion');
    const result = await tool.handler({
      params: {
        envelope: {
          eventId: 'evt-001',
          payloadPlainHash: bufferToHex(payloadPlainHash),
          agentSignature: bufferToHex(agentSignature),
        },
        proof: { leafIndex: 0, proofHashes: [bufferToHex(otherLeaf)] },
        expectedRoot: bufferToHex(computedRoot),
      },
    });
    assert.equal(result.valid, true);
    assert.equal(result.eventId, 'evt-001');
  });

  it('verifies a valid 2-leaf Merkle inclusion proof (leaf 1)', async () => {
    const payloadPlainHash = crypto.randomBytes(32);
    const agentSignature = crypto.randomBytes(64);
    const computedLeaf = crypto.createHash('sha256')
      .update(payloadPlainHash)
      .update(agentSignature)
      .digest();
    const otherLeaf = crypto.createHash('sha256').update('other-leaf-2').digest();
    // leaf 1 is on the right side
    const computedRoot = computeNodeHash(otherLeaf, computedLeaf);

    const tool = findTool('sync_verify_inclusion');
    const result = await tool.handler({
      params: {
        envelope: {
          eventId: 'evt-002',
          payloadPlainHash: bufferToHex(payloadPlainHash),
          agentSignature: bufferToHex(agentSignature),
        },
        proof: { leafIndex: 1, proofHashes: [bufferToHex(otherLeaf)] },
        expectedRoot: bufferToHex(computedRoot),
      },
    });
    assert.equal(result.valid, true);
  });

  it('rejects an invalid Merkle proof (wrong root)', async () => {
    const payloadPlainHash = crypto.randomBytes(32);
    const agentSignature = crypto.randomBytes(64);
    const otherLeaf = crypto.randomBytes(32);
    const fakeRoot = crypto.randomBytes(32);

    const tool = findTool('sync_verify_inclusion');
    const result = await tool.handler({
      params: {
        envelope: {
          eventId: 'evt-bad',
          payloadPlainHash: bufferToHex(payloadPlainHash),
          agentSignature: bufferToHex(agentSignature),
        },
        proof: { leafIndex: 0, proofHashes: [bufferToHex(otherLeaf)] },
        expectedRoot: bufferToHex(fakeRoot),
      },
    });
    assert.equal(result.valid, false);
  });

  it('rejects an invalid Merkle proof (wrong sibling)', async () => {
    const payloadPlainHash = crypto.randomBytes(32);
    const agentSignature = crypto.randomBytes(64);
    const computedLeaf = crypto.createHash('sha256')
      .update(payloadPlainHash)
      .update(agentSignature)
      .digest();
    const correctSibling = crypto.randomBytes(32);
    const wrongSibling = crypto.randomBytes(32);
    const correctRoot = computeNodeHash(computedLeaf, correctSibling);

    const tool = findTool('sync_verify_inclusion');
    const result = await tool.handler({
      params: {
        envelope: {
          eventId: 'evt-wrong-sibling',
          payloadPlainHash: bufferToHex(payloadPlainHash),
          agentSignature: bufferToHex(agentSignature),
        },
        proof: { leafIndex: 0, proofHashes: [bufferToHex(wrongSibling)] },
        expectedRoot: bufferToHex(correctRoot),
      },
    });
    assert.equal(result.valid, false);
  });

  it('verifies a 4-leaf tree inclusion proof', async () => {
    // Build 4 leaves
    const leaves = Array.from({ length: 4 }, (_, i) =>
      crypto.createHash('sha256').update(`leaf-${i}`).digest()
    );
    // Level 1
    const node01 = computeNodeHash(leaves[0], leaves[1]);
    const node23 = computeNodeHash(leaves[2], leaves[3]);
    // Root
    const root = computeNodeHash(node01, node23);

    // Prove leaf 2 (index=2): sibling is leaf[3], then sibling is node01
    const payloadPlainHash = crypto.randomBytes(32);
    const agentSignature = crypto.randomBytes(64);

    // We need the leaf hash to match leaves[2], so we construct it properly
    // Actually let's just use leaves[2] as the data directly
    // The handler computes leafHash = H(payloadPlainHash || agentSignature)
    // So we need to find payloadPlainHash and agentSignature such that
    // H(payloadPlainHash || agentSignature) = leaves[2]
    // That's not feasible with a hash function, so let's build the tree around the handler's computation.

    const ph = crypto.randomBytes(32);
    const sig = crypto.randomBytes(64);
    const leaf2 = crypto.createHash('sha256').update(ph).update(sig).digest();
    const leaf3 = crypto.createHash('sha256').update('leaf-3-4tree').digest();
    const leaf0 = crypto.createHash('sha256').update('leaf-0-4tree').digest();
    const leaf1 = crypto.createHash('sha256').update('leaf-1-4tree').digest();

    const n01 = computeNodeHash(leaf0, leaf1);
    const n23 = computeNodeHash(leaf2, leaf3);
    const r = computeNodeHash(n01, n23);

    const tool = findTool('sync_verify_inclusion');
    const result = await tool.handler({
      params: {
        envelope: {
          eventId: 'evt-4leaf',
          payloadPlainHash: bufferToHex(ph),
          agentSignature: bufferToHex(sig),
        },
        proof: {
          leafIndex: 2,
          proofHashes: [bufferToHex(leaf3), bufferToHex(n01)],
        },
        expectedRoot: bufferToHex(r),
      },
    });
    assert.equal(result.valid, true);
  });

  it('returns expectedRoot in result', async () => {
    const payloadPlainHash = crypto.randomBytes(32);
    const agentSignature = crypto.randomBytes(64);
    const fakeRoot = bufferToHex(crypto.randomBytes(32));
    const tool = findTool('sync_verify_inclusion');
    const result = await tool.handler({
      params: {
        envelope: {
          eventId: 'evt-test',
          payloadPlainHash: bufferToHex(payloadPlainHash),
          agentSignature: bufferToHex(agentSignature),
        },
        proof: { leafIndex: 0, proofHashes: [] },
        expectedRoot: fakeRoot,
      },
    });
    assert.equal(result.expectedRoot, fakeRoot);
  });
});

// ===========================================================================
// sync_inspect_commitment Handler Tests
// ===========================================================================

describe('sync_inspect_commitment — Handler', () => {
  it('returns "not configured" when sync is not configured', async () => {
    // Run from a temp dir where .stateset/sync.json does not exist
    const origCwd = process.cwd();
    const tmpDir = crypto.randomUUID();
    const os = await import('os');
    const fs = await import('fs/promises');
    const tempPath = await fs.mkdtemp(os.default.tmpdir() + '/ves-test-');
    process.chdir(tempPath);
    try {
      const tool = findTool('sync_inspect_commitment');
      const result = await tool.handler({ params: { batchId: 'batch-001' } });
      assert.equal(result.success, false);
      assert.ok(result.error.includes('not configured') || result.error.includes('Sync not configured'));
    } finally {
      process.chdir(origCwd);
      await fs.rm(tempPath, { recursive: true, force: true });
    }
  });

  it('has handler that is a function', () => {
    const tool = findTool('sync_inspect_commitment');
    assert.equal(typeof tool.handler, 'function');
  });
});

// ===========================================================================
// Integration: all VES tools exist in syncTools
// ===========================================================================

describe('VES Tools — All registered', () => {
  it('syncTools contains sync_verify_receipt', () => {
    assert.ok(findTool('sync_verify_receipt'));
  });

  it('syncTools contains sync_verify_inclusion', () => {
    assert.ok(findTool('sync_verify_inclusion'));
  });

  it('syncTools contains sync_inspect_commitment', () => {
    assert.ok(findTool('sync_inspect_commitment'));
  });

  it('all VES tools have handlers', () => {
    for (const name of ['sync_verify_receipt', 'sync_verify_inclusion', 'sync_inspect_commitment']) {
      const tool = findTool(name);
      assert.equal(typeof tool.handler, 'function', `${name} handler should be a function`);
    }
  });
});

/**
 * Verifiable Commerce Proof Generator
 *
 * Generates and verifies Merkle inclusion proofs, receipt bundles, and
 * compliance packages for VES (Verifiable Event Sequencing) commerce events.
 *
 * Uses the VES v1.0 domain-separated hashing primitives from crypto.js to
 * build standards-compliant proofs that can be independently verified against
 * on-chain anchor transactions.
 *
 * @module sync/proof-generator
 */

import { randomUUID } from 'node:crypto';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Return the smallest power of 2 >= n
 * @param {number} n
 * @returns {number}
 */
function nextPow2(n) {
  if (n <= 1) return 1;
  let p = 1;
  while (p < n) p <<= 1;
  return p;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/**
 * Create a proof generator that operates on VES events.
 *
 * @param {typeof import('./crypto.js')} cryptoModule — the VES crypto primitives
 * @returns {object} proof generator instance
 */
export function createProofGenerator(cryptoModule) {
  const crypto = cryptoModule;

  function normalizeHybridPublicKeyBundle(publicKey) {
    if (
      !publicKey ||
      Buffer.isBuffer(publicKey) ||
      publicKey instanceof Uint8Array ||
      typeof publicKey === 'string'
    ) {
      return null;
    }

    return {
      ed25519PublicKey:
        publicKey.ed25519PublicKey ?? publicKey.ed25519_public_key ?? publicKey.publicKey ?? null,
      mlDsa65PublicKey: publicKey.mlDsa65PublicKey ?? publicKey.ml_dsa_65_public_key ?? null,
    };
  }

  function verifyEventSignatureMaterial(eventSigningHash, signature, signatureBundle, publicKey) {
    const publicKeyBundle = normalizeHybridPublicKeyBundle(publicKey);
    const toBuffer = (value) => {
      if (Buffer.isBuffer(value)) {
        return value;
      }
      if (typeof value === 'string' && typeof crypto.hexToBuffer === 'function') {
        return crypto.hexToBuffer(value);
      }
      return Buffer.from(value, 'hex');
    };

    if (
      signatureBundle &&
      publicKeyBundle?.ed25519PublicKey &&
      publicKeyBundle?.mlDsa65PublicKey &&
      typeof crypto.verifyEventSignatureHybrid === 'function'
    ) {
      try {
        return crypto.verifyEventSignatureHybrid(
          eventSigningHash,
          signatureBundle,
          publicKeyBundle,
        );
      } catch {
        return false;
      }
    }

    if (!signature || !publicKey) {
      return null;
    }

    const sigBuf = toBuffer(signature);
    const pubMaterial = publicKeyBundle?.ed25519PublicKey ?? publicKey;
    const pubBuf = toBuffer(pubMaterial);

    try {
      return crypto.verifyEventSignature(eventSigningHash, sigBuf, pubBuf);
    } catch {
      return false;
    }
  }

  // -------------------------------------------------------------------------
  // Internal: Merkle proof construction
  // -------------------------------------------------------------------------

  /**
   * Build a Merkle proof (sibling path) for `targetIndex` within `leaves`.
   *
   * Leaves are padded to the next power of 2 with ZERO_HASH, then a
   * bottom-up tree is built using `computeNodeHash`. The sibling at each
   * level is recorded along with its position relative to the target node.
   *
   * @param {Buffer[]} leaves — array of 32-byte leaf hashes
   * @param {number} targetIndex — index of the leaf to prove
   * @returns {{ proof: Array<{position: 'left'|'right', hash: string}>, root: Buffer }}
   */
  function buildMerkleProof(leaves, targetIndex) {
    if (leaves.length === 0) {
      throw new Error('Cannot build proof for empty leaf set');
    }
    if (targetIndex < 0 || targetIndex >= leaves.length) {
      throw new Error(`Target index ${targetIndex} out of range [0, ${leaves.length - 1}]`);
    }

    const padded = nextPow2(leaves.length);
    const paddedLeaves = leaves.map((l) => (Buffer.isBuffer(l) ? l : Buffer.from(l, 'hex')));
    while (paddedLeaves.length < padded) {
      paddedLeaves.push(crypto.ZERO_HASH);
    }

    // Single-leaf tree: proof is empty, root = the leaf itself
    if (paddedLeaves.length === 1) {
      return { proof: [], root: Buffer.from(paddedLeaves[0]) };
    }

    let level = paddedLeaves.map((l) => Buffer.from(l));
    const proof = [];
    let idx = targetIndex;

    while (level.length > 1) {
      const nextLevel = [];
      for (let i = 0; i < level.length; i += 2) {
        const left = level[i];
        const right = level[i + 1] || crypto.ZERO_HASH;
        nextLevel.push(crypto.computeNodeHash(left, right));
      }

      // Record sibling
      const siblingIdx = idx % 2 === 0 ? idx + 1 : idx - 1;
      if (siblingIdx >= 0 && siblingIdx < level.length) {
        proof.push({
          position: idx % 2 === 0 ? 'right' : 'left',
          hash: level[siblingIdx].toString('hex'),
        });
      }

      idx = Math.floor(idx / 2);
      level = nextLevel;
    }

    return { proof, root: level[0] };
  }

  // -------------------------------------------------------------------------
  // Internal: Merkle proof verification
  // -------------------------------------------------------------------------

  /**
   * Verify a Merkle inclusion proof.
   *
   * Starting from `leafHash`, the proof's sibling hashes are folded in
   * (respecting left/right positions) until a root is computed and compared
   * against `expectedRoot`.
   *
   * @param {Buffer|string} leafHash — the leaf to verify
   * @param {Array<{position: 'left'|'right', hash: string}>} proof — sibling path
   * @param {Buffer|string} expectedRoot — expected Merkle root
   * @returns {boolean}
   */
  function verifyMerkleProof(leafHash, proof, expectedRoot) {
    let current = Buffer.isBuffer(leafHash) ? leafHash : Buffer.from(leafHash, 'hex');

    for (const step of proof) {
      const sibling = Buffer.from(step.hash, 'hex');
      if (step.position === 'right') {
        current = crypto.computeNodeHash(current, sibling);
      } else {
        current = crypto.computeNodeHash(sibling, current);
      }
    }

    const root = Buffer.isBuffer(expectedRoot) ? expectedRoot : Buffer.from(expectedRoot, 'hex');
    return current.equals(root);
  }

  // -------------------------------------------------------------------------
  // Public API
  // -------------------------------------------------------------------------

  /**
   * Generate a Merkle inclusion proof for a specific event within a batch.
   *
   * @param {string} eventId — the target event's id
   * @param {Array<object>} events — full batch of events (each must have `.id` and `.eventSigningHash`)
   * @param {object} [batchMeta] — optional batch metadata (`{ batchId, anchorTxHash }`)
   * @returns {{ eventId: string, leaf: string, proof: Array, root: string, batchId?: string, anchorTxHash?: string }}
   */
  function generateInclusionProof(eventId, events, batchMeta = {}) {
    const targetIndex = events.findIndex((e) => e.id === eventId);
    if (targetIndex === -1) {
      throw new Error(`Event '${eventId}' not found in batch`);
    }

    // Compute leaf hashes for every event in the batch
    const leafHashes = events.map((evt, i) => {
      const signingHash = Buffer.isBuffer(evt.eventSigningHash)
        ? evt.eventSigningHash
        : Buffer.from(evt.eventSigningHash, 'hex');
      return crypto.computeNodeHash(Buffer.from(`leaf-${i}`), signingHash);
    });

    const { proof, root } = buildMerkleProof(leafHashes, targetIndex);

    return {
      eventId,
      leaf: leafHashes[targetIndex].toString('hex'),
      proof,
      root: root.toString('hex'),
      ...(batchMeta.batchId && { batchId: batchMeta.batchId }),
      ...(batchMeta.anchorTxHash && { anchorTxHash: batchMeta.anchorTxHash }),
    };
  }

  /**
   * Verify a previously generated inclusion proof.
   *
   * @param {{ leafHash: string, proof: Array, expectedRoot: string }} proofData
   * @returns {{ valid: boolean, eventId?: string, root: string }}
   */
  function verifyInclusionProof(proofData) {
    const { leafHash, proof, expectedRoot, eventId } = proofData;
    const valid = verifyMerkleProof(leafHash, proof, expectedRoot);
    return {
      valid,
      ...(eventId && { eventId }),
      root: expectedRoot,
    };
  }

  /**
   * Generate a full receipt bundle for an event.
   *
   * Combines the event data, its leaf hash, inclusion proof, Merkle root,
   * and optional on-chain anchor metadata into a single verifiable object.
   *
   * @param {object} event — the event object (must have `.id`, `.eventSigningHash`, `.timestamp`)
   * @param {Array<object>} batchEvents — all events in the batch
   * @param {object} [batchMeta] — optional `{ batchId, anchorTxHash }`
   * @returns {object} receipt bundle
   */
  function generateReceiptBundle(event, batchEvents, batchMeta = {}) {
    const inclusionProof = generateInclusionProof(event.id, batchEvents, batchMeta);

    return {
      event: {
        id: event.id,
        payload: event.payload,
        payloadHash: event.payloadHash,
        eventSigningHash: Buffer.isBuffer(event.eventSigningHash)
          ? event.eventSigningHash.toString('hex')
          : event.eventSigningHash,
        signature: event.signature || null,
        timestamp: event.timestamp,
      },
      leafHash: inclusionProof.leaf,
      inclusionProof: inclusionProof.proof,
      merkleRoot: inclusionProof.root,
      batchId: batchMeta.batchId || null,
      anchorTxHash: batchMeta.anchorTxHash || null,
      timestamp: new Date().toISOString(),
    };
  }

  /**
   * Verify a receipt bundle.
   *
   * Performs up to three checks:
   * 1. Leaf hash recomputation matches the bundle's leafHash
   * 2. Merkle inclusion proof is valid against the root
   * 3. Event signature is valid (when signature + publicKey present)
   *
   * @param {object} bundle — a receipt bundle from `generateReceiptBundle`
   * @param {Buffer|string} [publicKey] — optional Ed25519 public key for signature verification
   * @returns {{ valid: boolean, checks: Array<{ check: string, passed: boolean, detail?: string }> }}
   */
  function verifyReceiptBundle(bundle, publicKey) {
    const checks = [];

    // Check 1: inclusion proof
    const inclusionValid = verifyMerkleProof(
      bundle.leafHash,
      bundle.inclusionProof,
      bundle.merkleRoot,
    );
    checks.push({
      check: 'inclusion_proof',
      passed: inclusionValid,
      detail: inclusionValid
        ? 'Leaf is included in the Merkle root'
        : 'Inclusion proof verification failed',
    });

    // Check 2: payload hash consistency (if payloadHash present)
    if (bundle.event.payloadHash && bundle.event.payload !== undefined) {
      const recomputed = crypto.computePayloadPlainHash(bundle.event.payload).toString('hex');
      const hashMatch = recomputed === bundle.event.payloadHash;
      checks.push({
        check: 'payload_hash',
        passed: hashMatch,
        detail: hashMatch ? 'Payload hash matches recomputed value' : 'Payload hash mismatch',
      });
    }

    // Check 3: signature verification (legacy or hybrid)
    if ((bundle.event.signature || bundle.event.signatureBundle) && publicKey) {
      const hashBuf = Buffer.isBuffer(bundle.event.eventSigningHash)
        ? bundle.event.eventSigningHash
        : Buffer.from(bundle.event.eventSigningHash, 'hex');

      const sigValid = verifyEventSignatureMaterial(
        hashBuf,
        bundle.event.signature,
        bundle.event.signatureBundle || null,
        publicKey,
      );
      checks.push({
        check: 'signature',
        passed: Boolean(sigValid),
        detail: sigValid ? 'Event signature is valid' : 'Signature verification failed',
      });
    }

    const valid = checks.every((c) => c.passed);
    return { valid, checks };
  }

  /**
   * Generate a batch summary including the Merkle root, event count, and
   * time range.
   *
   * @param {string} batchId
   * @param {Array<object>} events — events in the batch
   * @param {object} [anchorTx] — optional `{ txHash }`
   * @returns {{ batchId: string, root: string, eventCount: number, timeRange: { start: string, end: string }, anchorTxHash?: string }}
   */
  function generateBatchSummary(batchId, events, anchorTx = {}) {
    if (events.length === 0) {
      return {
        batchId,
        root: crypto.ZERO_HASH.toString('hex'),
        eventCount: 0,
        timeRange: { start: null, end: null },
        anchorTxHash: anchorTx.txHash || null,
      };
    }

    // Compute leaf hashes
    const leafHashes = events.map((evt, i) => {
      const signingHash = Buffer.isBuffer(evt.eventSigningHash)
        ? evt.eventSigningHash
        : Buffer.from(evt.eventSigningHash, 'hex');
      return crypto.computeNodeHash(Buffer.from(`leaf-${i}`), signingHash);
    });

    const { root } = buildMerkleProof(leafHashes, 0);

    // Extract time range
    const timestamps = events
      .filter((e) => e.timestamp)
      .map((e) => new Date(e.timestamp).getTime())
      .sort((a, b) => a - b);

    return {
      batchId,
      root: root.toString('hex'),
      eventCount: events.length,
      timeRange: {
        start: timestamps.length > 0 ? new Date(timestamps[0]).toISOString() : null,
        end:
          timestamps.length > 0 ? new Date(timestamps[timestamps.length - 1]).toISOString() : null,
      },
      anchorTxHash: anchorTx.txHash || null,
    };
  }

  /**
   * Generate a compliance package — a complete, self-contained bundle of
   * receipt proofs for every event in a batch. Suitable for regulatory
   * export or third-party audit.
   *
   * @param {Array<object>} events — all events to include
   * @param {object} [batchMeta] — optional `{ batchId, anchorTxHash }`
   * @returns {{ receipts: Array, summary: object, generatedAt: string }}
   */
  function generateCompliancePackage(events, batchMeta = {}) {
    const effectiveBatchId = batchMeta.batchId || randomUUID();

    const receipts = events.map((evt) =>
      generateReceiptBundle(evt, events, {
        batchId: effectiveBatchId,
        anchorTxHash: batchMeta.anchorTxHash,
      }),
    );

    const summary = generateBatchSummary(effectiveBatchId, events, {
      txHash: batchMeta.anchorTxHash,
    });

    return {
      receipts,
      summary,
      generatedAt: new Date().toISOString(),
    };
  }

  /**
   * Verify an individual event's signature and hash integrity.
   *
   * @param {object} event — event with `.eventSigningHash` and optionally `.signature`
   * @param {Buffer|string|Object} [publicKey] — Ed25519 key or hybrid public-key bundle
   * @returns {{ valid: boolean, signatureValid: boolean|null, hashValid: boolean }}
   */
  function verifyEvent(event, publicKey) {
    // Hash validity: check that eventSigningHash is a plausible 32-byte hash
    const hashBuf = Buffer.isBuffer(event.eventSigningHash)
      ? event.eventSigningHash
      : typeof crypto.hexToBuffer === 'function'
        ? crypto.hexToBuffer(event.eventSigningHash)
        : Buffer.from(event.eventSigningHash, 'hex');
    const hashValid = hashBuf.length === 32;

    // Signature verification
    let signatureValid = null;
    if ((event.signature || event.signatureBundle) && publicKey) {
      signatureValid = verifyEventSignatureMaterial(
        hashBuf,
        event.signature,
        event.signatureBundle || null,
        publicKey,
      );
    }

    const valid = hashValid && (signatureValid === null || signatureValid);
    return { valid, signatureValid, hashValid };
  }

  // -------------------------------------------------------------------------
  // Return public interface
  // -------------------------------------------------------------------------

  return {
    generateInclusionProof,
    verifyInclusionProof,
    generateReceiptBundle,
    verifyReceiptBundle,
    generateBatchSummary,
    generateCompliancePackage,
    verifyEvent,
    // Expose building blocks for advanced use / testing
    buildMerkleProof,
    verifyMerkleProof,
  };
}

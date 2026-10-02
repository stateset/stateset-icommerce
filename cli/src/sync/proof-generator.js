/**
 * Verifiable Commerce Proof Generator
 *
 * Generates and verifies Merkle inclusion proofs, receipt bundles, and
 * compliance packages for VES (Verifiable Event Sequencing) commerce events.
 *
 * Proofs carry a `format`. `ves-v1` is VES v1.0 section 10 exactly -- spec
 * leaves, pad leaves and node hashing, the same tree stateset-crypto builds --
 * and verifies through the SDK's spec verifier. `legacy-v0` is this module's
 * original construction, which claimed VES v1.0 compliance while hashing
 * leaves in the NODE domain and padding with the zero hash; it is kept only
 * so roots already anchored on-chain stay verifiable. See PROOF_FORMATS.
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

  // Proof formats.
  //
  // `ves-v1` follows VES v1.0 section 10: leaves come from computeLeafHash
  // (domain VES_LEAF_V1, binding tenant, store, sequence number, signing hash
  // and agent signature), padding from computePadLeaf, and nodes from
  // computeNodeHash. That is the tree stateset-crypto's compute_merkle_root
  // builds, so its root matches the `merkle_root` vectors in
  // bindings/test-vectors/v1.json, and its proofs verify through the SDK's
  // spec verifier (client.js verifyInclusion).
  //
  // `legacy-v0` is this module's original construction: leaves hashed in the
  // NODE domain over a positional label and the signing hash, padded with the
  // zero hash. Its header claimed VES v1.0 compliance it did not have. It is
  // kept ONLY so roots already anchored on-chain stay verifiable; a proof with
  // no `format` field is read as legacy-v0.
  const PROOF_FORMATS = Object.freeze({ LEGACY_V0: 'legacy-v0', VES_V1: 'ves-v1' });
  const SPEC_LEAF_FIELDS = Object.freeze([
    'tenantId',
    'storeId',
    'sequenceNumber',
    'eventSigningHash',
    'agentSignature',
  ]);

  function toBuffer(value) {
    return Buffer.isBuffer(value) ? value : Buffer.from(value, 'hex');
  }

  function missingSpecFields(evt) {
    return SPEC_LEAF_FIELDS.filter((field) => evt[field] === undefined || evt[field] === null);
  }

  /**
   * Choose the proof format for a batch. An explicit `ves-v1` request fails
   * loudly when any event lacks a field the spec leaf binds. With no request,
   * a batch that carries every spec field gets `ves-v1`; one that does not
   * falls back to `legacy-v0`, and the caller is told why.
   */
  function resolveFormat(events, requested) {
    const gaps = events
      .map((evt) => ({ id: evt.id, missing: missingSpecFields(evt) }))
      .filter((entry) => entry.missing.length > 0);

    if (requested === PROOF_FORMATS.VES_V1) {
      if (gaps.length > 0) {
        const first = gaps[0];
        throw new Error(
          `ves-v1 proofs bind ${SPEC_LEAF_FIELDS.join(', ')}; event '${first.id}' is missing ` +
            `${first.missing.join(', ')}`,
        );
      }
      return { format: PROOF_FORMATS.VES_V1, warning: null };
    }
    if (requested === PROOF_FORMATS.LEGACY_V0) {
      return { format: PROOF_FORMATS.LEGACY_V0, warning: null };
    }
    if (requested !== undefined && requested !== null) {
      throw new Error(
        `Unknown proof format '${requested}'; expected one of ${Object.values(PROOF_FORMATS).join(', ')}`,
      );
    }
    // Auto-detection needs positive evidence. An empty batch carries none, so
    // it keeps the output it has always had rather than switching format by
    // vacuous truth; ask for ves-v1 explicitly to get the spec's empty root.
    if (events.length > 0 && gaps.length === 0) {
      return { format: PROOF_FORMATS.VES_V1, warning: null };
    }
    if (events.length === 0) {
      return { format: PROOF_FORMATS.LEGACY_V0, warning: null };
    }
    return {
      format: PROOF_FORMATS.LEGACY_V0,
      warning:
        `Generated a legacy-v0 proof: ${gaps.length} event(s) lack fields the VES v1.0 leaf ` +
        `binds (${SPEC_LEAF_FIELDS.join(', ')}). Supply them for a spec proof that the SDK ` +
        'verifier accepts.',
    };
  }

  function leafFor(evt, index, format) {
    if (format === PROOF_FORMATS.VES_V1) {
      return crypto.computeLeafHash({
        tenantId: evt.tenantId,
        storeId: evt.storeId,
        sequenceNumber: evt.sequenceNumber,
        eventSigningHash: toBuffer(evt.eventSigningHash),
        agentSignature: toBuffer(evt.agentSignature),
      });
    }
    return crypto.computeNodeHash(Buffer.from(`leaf-${index}`), toBuffer(evt.eventSigningHash));
  }

  function padFor(format) {
    return format === PROOF_FORMATS.VES_V1 ? crypto.computePadLeaf() : crypto.ZERO_HASH;
  }

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
  function buildMerkleProof(leaves, targetIndex, pad = crypto.ZERO_HASH) {
    if (leaves.length === 0) {
      throw new Error('Cannot build proof for empty leaf set');
    }
    if (targetIndex < 0 || targetIndex >= leaves.length) {
      throw new Error(`Target index ${targetIndex} out of range [0, ${leaves.length - 1}]`);
    }

    const padded = nextPow2(leaves.length);
    const paddedLeaves = leaves.map((l) => (Buffer.isBuffer(l) ? l : Buffer.from(l, 'hex')));
    while (paddedLeaves.length < padded) {
      paddedLeaves.push(pad);
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
        const right = level[i + 1] || pad;
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
  function generateInclusionProof(eventId, events, batchMeta = {}, options = {}) {
    const targetIndex = events.findIndex((e) => e.id === eventId);
    if (targetIndex === -1) {
      throw new Error(`Event '${eventId}' not found in batch`);
    }

    const { format, warning } = resolveFormat(events, options.format);
    const leafHashes = events.map((evt, i) => leafFor(evt, i, format));
    const { proof, root } = buildMerkleProof(leafHashes, targetIndex, padFor(format));

    return {
      format,
      eventId,
      leaf: leafHashes[targetIndex].toString('hex'),
      // The spec verifier walks `proofHashes` by the bits of `leafIndex`;
      // `proof` keeps the positional form this module has always returned.
      leafIndex: targetIndex,
      proofHashes: proof.map((step) => step.hash),
      proof,
      root: root.toString('hex'),
      ...(warning && { warning }),
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
    // No `format` means a proof generated before formats existed: legacy-v0.
    const format = proofData.format ?? PROOF_FORMATS.LEGACY_V0;
    if (!Object.values(PROOF_FORMATS).includes(format)) {
      return {
        valid: false,
        format,
        reason: `Unknown proof format '${format}'`,
        ...(eventId && { eventId }),
        root: expectedRoot,
      };
    }
    // Node hashing is identical in both formats, so the walk is the same; the
    // formats differ only in how the leaf and the padding were built.
    const valid = verifyMerkleProof(leafHash, proof, expectedRoot);
    return {
      valid,
      format,
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
  function generateReceiptBundle(event, batchEvents, batchMeta = {}, options = {}) {
    const inclusionProof = generateInclusionProof(event.id, batchEvents, batchMeta, options);
    const isSpec = inclusionProof.format === PROOF_FORMATS.VES_V1;

    return {
      format: inclusionProof.format,
      ...(inclusionProof.warning && { warning: inclusionProof.warning }),
      leafIndex: inclusionProof.leafIndex,
      event: {
        // A ves-v1 leaf binds these, so a verifier can recompute it from the
        // event instead of trusting the bundle's leafHash.
        ...(isSpec && {
          tenantId: event.tenantId,
          storeId: event.storeId,
          sequenceNumber: event.sequenceNumber,
          agentSignature: Buffer.isBuffer(event.agentSignature)
            ? event.agentSignature.toString('hex')
            : event.agentSignature,
        }),
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
   * Performs up to four checks:
   * 0. For ves-v1 bundles, the leaf hash recomputed from the event matches
   *    the bundle's leafHash (legacy-v0 leaves bind a batch position the
   *    bundle does not carry, so they cannot be recomputed)
   * 1. Merkle inclusion proof is valid against the root
   * 2. Payload hash is consistent (when payload + payloadHash present)
   * 3. Event signature is valid (when signature + publicKey present)
   *
   * @param {object} bundle — a receipt bundle from `generateReceiptBundle`
   * @param {Buffer|string} [publicKey] — optional Ed25519 public key for signature verification
   * @returns {{ valid: boolean, checks: Array<{ check: string, passed: boolean, detail?: string }> }}
   */
  function verifyReceiptBundle(bundle, publicKey) {
    const checks = [];
    const format = bundle.format ?? PROOF_FORMATS.LEGACY_V0;

    // Check 0: the leaf really is this event's leaf. A ves-v1 leaf binds the
    // tenant, store, sequence number, signing hash and agent signature, all
    // carried on the bundle's event, so it can be recomputed rather than
    // trusted. A legacy-v0 leaf binds a batch position the bundle does not
    // record, so it cannot be; that is one reason legacy-v0 is not generated
    // by choice.
    if (format === PROOF_FORMATS.VES_V1) {
      const missing = missingSpecFields(bundle.event);
      if (missing.length > 0) {
        checks.push({
          check: 'leaf_hash',
          passed: false,
          detail: `ves-v1 bundle event is missing ${missing.join(', ')}`,
        });
      } else {
        const recomputed = leafFor(bundle.event, bundle.leafIndex, format).toString('hex');
        const leafMatch = recomputed === bundle.leafHash;
        checks.push({
          check: 'leaf_hash',
          passed: leafMatch,
          detail: leafMatch
            ? 'Leaf hash recomputed from the event matches'
            : 'Leaf hash does not match the event it claims to prove',
        });
      }
    }

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
  function generateBatchSummary(batchId, events, anchorTx = {}, options = {}) {
    const { format, warning } = resolveFormat(events, options.format);
    if (events.length === 0) {
      // The spec root of an empty batch is the pad leaf, as in
      // stateset-crypto's compute_merkle_root.
      return {
        format,
        batchId,
        root: padFor(format).toString('hex'),
        eventCount: 0,
        timeRange: { start: null, end: null },
        anchorTxHash: anchorTx.txHash || null,
      };
    }

    const leafHashes = events.map((evt, i) => leafFor(evt, i, format));
    const { root } = buildMerkleProof(leafHashes, 0, padFor(format));

    // Extract time range
    const timestamps = events
      .filter((e) => e.timestamp)
      .map((e) => new Date(e.timestamp).getTime())
      .sort((a, b) => a - b);

    return {
      format,
      ...(warning && { warning }),
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
    PROOF_FORMATS,
  };
}

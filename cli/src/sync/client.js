/**
 * Sequencer REST Client (VES v1.0)
 *
 * Client for communicating with the stateset-sequencer service.
 * Supports VES v1.0 signed events and encrypted payloads.
 */

import {
  computeEventSigningHash,
  verifyEventSignature,
  verifyEventSignatureHybrid,
  verifyEventSignatureStrict,
  computeLeafHash,
  computeNodeHash,
  hexToBuffer,
} from './crypto.js';
import {
  KEY_WRAP_SCHEME_X25519_HKDF_SHA256,
  SIGNATURE_SCHEME_ED25519_ML_DSA_65,
  SIGNATURE_SCHEME_ML_DSA_65,
  assertEventMatchesSecurityProfile,
  assertKeyRegistrationMatchesSecurityProfile,
  assertReceiptMatchesSecurityProfile,
  assertSecureTransportForProfile,
  isSecureSequencerProtocol,
  resolveSecurityProfile,
} from './pqc.js';

/**
 * @typedef {Object} EventEnvelope
 * @property {string} eventId - UUID
 * @property {string} [commandId] - Idempotency key
 * @property {string} tenantId - Tenant UUID
 * @property {string} storeId - Store UUID
 * @property {string} entityType - Entity type
 * @property {string} entityId - Entity ID
 * @property {string} eventType - Event type
 * @property {Object} payload - Event payload (plaintext)
 * @property {number} vesVersion - VES protocol version (1)
 * @property {number} payloadKind - 0=plaintext, 1=encrypted
 * @property {Object} [payloadEncrypted] - Encrypted payload (VES-ENC-1)
 * @property {string} payloadPlainHash - SHA-256 of plaintext (hex)
 * @property {string} payloadCipherHash - SHA-256 of ciphertext or zero hash (hex)
 * @property {number} agentKeyId - Key ID used for signing
 * @property {string} agentSignature - Ed25519 signature (hex)
 * @property {number} [agentSignatureScheme] - PQ or hybrid signature scheme identifier
 * @property {Object} [agentSignatureBundle] - PQ or hybrid signature bundle
 * @property {number} [baseVersion] - OCC version
 * @property {string} createdAt - ISO timestamp
 * @property {string} sourceAgent - Agent UUID
 */

/**
 * @typedef {Object} SequencedEvent
 * @property {EventEnvelope} envelope
 * @property {number} sequenceNumber
 * @property {string} sequencedAt
 * @property {string} [receiptHash] - Sequencer receipt hash
 */

/**
 * @typedef {Object} IngestReceipt
 * @property {string} batchId
 * @property {number} eventsAccepted
 * @property {number} eventsRejected
 * @property {number} [sequenceStart]
 * @property {number} [sequenceEnd]
 * @property {number} headSequence
 * @property {Array<{eventId: string, reason: string}>} rejections
 */

/**
 * @typedef {Object} SyncState
 * @property {string} tenantId
 * @property {string} storeId
 * @property {number} headSequence
 * @property {string} [stateRoot]
 * @property {string} [lastCommitmentId]
 */

/**
 * @typedef {Object} BatchCommitment
 * @property {string} batchId
 * @property {string} merkleRoot
 * @property {number} startSequence
 * @property {number} endSequence
 * @property {number} eventCount
 * @property {string} committedAt
 */

/**
 * @typedef {Object} InclusionProof
 * @property {string} merkleRoot
 * @property {number} leafIndex
 * @property {string[]} proofHashes
 * @property {number} leafCount
 */

/**
 * REST client for the sequencer (fallback when gRPC not available)
 */

const ALLOWED_SEQUENCER_PROTOCOLS = new Set(['grpc:', 'grpcs:', 'http:', 'https:']);

function parseSequencerUrl(url) {
  if (typeof url !== 'string' || !url.trim()) {
    throw new Error('Sequencer URL must be a non-empty string');
  }

  const parsed = new URL(url.trim());
  if (!ALLOWED_SEQUENCER_PROTOCOLS.has(parsed.protocol)) {
    throw new Error(`Unsupported sequencer protocol: ${parsed.protocol}`);
  }
  if (!parsed.hostname) {
    throw new Error('Sequencer URL must include a host');
  }
  return parsed;
}

function toSnakeCaseSignatureBundle(bundle) {
  if (!bundle) {
    return null;
  }

  return {
    ed25519_signature: bundle.ed25519Signature ?? bundle.ed25519_signature ?? null,
    ml_dsa_65_signature: bundle.mlDsa65Signature ?? bundle.ml_dsa_65_signature ?? null,
  };
}

function fromSnakeCaseSignatureBundle(bundle) {
  if (!bundle) {
    return null;
  }

  return {
    ed25519Signature: bundle.ed25519_signature ?? bundle.ed25519Signature ?? null,
    mlDsa65Signature: bundle.ml_dsa_65_signature ?? bundle.mlDsa65Signature ?? null,
  };
}

function toSnakeCasePublicKeyBundle(bundle) {
  if (!bundle) {
    return null;
  }

  return {
    ed25519_public_key: bundle.ed25519PublicKey ?? bundle.ed25519_public_key ?? null,
    ml_dsa_65_public_key: bundle.mlDsa65PublicKey ?? bundle.ml_dsa_65_public_key ?? null,
    x25519_public_key: bundle.x25519PublicKey ?? bundle.x25519_public_key ?? null,
    ml_kem_768_public_key: bundle.mlKem768PublicKey ?? bundle.ml_kem_768_public_key ?? null,
  };
}

function fromSnakeCasePublicKeyBundle(bundle) {
  if (!bundle) {
    return null;
  }

  return {
    ed25519PublicKey: bundle.ed25519_public_key ?? bundle.ed25519PublicKey ?? null,
    mlDsa65PublicKey: bundle.ml_dsa_65_public_key ?? bundle.mlDsa65PublicKey ?? null,
    x25519PublicKey: bundle.x25519_public_key ?? bundle.x25519PublicKey ?? null,
    mlKem768PublicKey: bundle.ml_kem_768_public_key ?? bundle.mlKem768PublicKey ?? null,
  };
}

function normalizeVerificationPublicKeyBundle(publicKey) {
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

function toSnakeCaseProofOfPossessionBundle(bundle) {
  if (!bundle) {
    return null;
  }

  return {
    ed25519_pop: bundle.ed25519Pop ?? bundle.ed25519_pop ?? null,
    ml_dsa_65_pop: bundle.mlDsa65Pop ?? bundle.ml_dsa_65_pop ?? null,
  };
}

function toSnakeCaseKeyWrapParams(params) {
  if (!params) {
    return null;
  }

  return {
    scheme: Number(params.scheme ?? params.wrapScheme ?? params.wrap_scheme ?? 0),
    kdf: params.kdf ?? null,
    aead: params.aead ?? null,
  };
}

function fromSnakeCaseKeyWrapParams(params) {
  if (!params) {
    return null;
  }

  return {
    scheme: Number(params.scheme ?? params.wrapScheme ?? params.wrap_scheme ?? 0),
    kdf: params.kdf ?? null,
    aead: params.aead ?? null,
  };
}

function toSnakeCaseRecipientWrap(wrap) {
  if (!wrap) {
    return null;
  }

  const wrappedKey =
    wrap.wrappedKey ?? wrap.wrapped_key ?? wrap.wrapped_key_b64u ?? wrap.ct_b64u ?? null;

  return {
    recipient_kid: Number(wrap.recipientKid ?? wrap.recipient_kid ?? 0),
    wrap_scheme: Number(wrap.wrapScheme ?? wrap.wrap_scheme ?? 0),
    x25519_enc_b64u: wrap.x25519Enc ?? wrap.x25519_enc ?? wrap.x25519_enc_b64u ?? null,
    ml_kem_ciphertext_b64u:
      wrap.mlKemCiphertext ??
      wrap.ml_kem_ciphertext ??
      wrap.ml_kem_ciphertext_b64u ??
      wrap.mlkem_ct_b64u ??
      null,
    wrap_nonce_b64u: wrap.wrapNonce ?? wrap.wrap_nonce ?? wrap.wrap_nonce_b64u ?? null,
    wrapped_key_b64u: wrappedKey,
  };
}

function fromSnakeCaseRecipientWrap(wrap) {
  if (!wrap) {
    return null;
  }

  const wrappedKey =
    wrap.wrapped_key_b64u ?? wrap.wrappedKey ?? wrap.wrapped_key ?? wrap.ct_b64u ?? null;

  return {
    recipientKid: Number(wrap.recipient_kid ?? wrap.recipientKid ?? 0),
    wrapScheme: Number(wrap.wrap_scheme ?? wrap.wrapScheme ?? 0),
    x25519Enc: wrap.x25519_enc_b64u ?? wrap.x25519Enc ?? wrap.x25519_enc ?? null,
    mlKemCiphertext:
      wrap.ml_kem_ciphertext_b64u ??
      wrap.mlKemCiphertext ??
      wrap.ml_kem_ciphertext ??
      wrap.mlkem_ct_b64u ??
      null,
    wrapNonce: wrap.wrap_nonce_b64u ?? wrap.wrapNonce ?? wrap.wrap_nonce ?? null,
    wrappedKey,
  };
}

function toSnakeCaseRecipientWraps(recipientWraps) {
  if (!Array.isArray(recipientWraps)) {
    return null;
  }

  return recipientWraps.map((wrap) => toSnakeCaseRecipientWrap(wrap)).filter(Boolean);
}

function fromSnakeCaseRecipientWraps(recipientWraps) {
  if (!Array.isArray(recipientWraps)) {
    return null;
  }

  return recipientWraps.map((wrap) => fromSnakeCaseRecipientWrap(wrap)).filter(Boolean);
}

function deriveKeyWrapParams(payloadEncrypted) {
  if (!Array.isArray(payloadEncrypted?.recipients) || payloadEncrypted.recipients.length === 0) {
    return null;
  }

  return {
    scheme: KEY_WRAP_SCHEME_X25519_HKDF_SHA256,
    kdf: payloadEncrypted.hpke?.kdf ?? 'HKDF-SHA256',
    aead: payloadEncrypted.hpke?.aead ?? payloadEncrypted.aead ?? 'AES-256-GCM',
  };
}

function deriveRecipientWrapsFromLegacyRecipients(payloadEncrypted) {
  if (!Array.isArray(payloadEncrypted?.recipients)) {
    return null;
  }

  return payloadEncrypted.recipients.map((recipient) => ({
    recipient_kid: Number(recipient.recipient_kid ?? recipient.recipientKid ?? 0),
    wrap_scheme: KEY_WRAP_SCHEME_X25519_HKDF_SHA256,
    x25519_enc_b64u: recipient.enc_b64u ?? recipient.encB64u ?? null,
    ml_kem_ciphertext_b64u: null,
    wrap_nonce_b64u: null,
    wrapped_key_b64u:
      recipient.wrapped_key_b64u ?? recipient.wrappedKeyB64u ?? recipient.ct_b64u ?? null,
  }));
}

function normalizePayloadEncryptedForWire(payloadEncrypted) {
  if (!payloadEncrypted) {
    return null;
  }

  const normalized = { ...payloadEncrypted };
  const keyWrapParams =
    toSnakeCaseKeyWrapParams(payloadEncrypted.keyWrapParams ?? payloadEncrypted.key_wrap_params) ??
    deriveKeyWrapParams(payloadEncrypted);
  const recipientWraps =
    toSnakeCaseRecipientWraps(
      payloadEncrypted.recipientWraps ?? payloadEncrypted.recipient_wraps,
    ) ?? deriveRecipientWrapsFromLegacyRecipients(payloadEncrypted);

  if (keyWrapParams) {
    normalized.key_wrap_params = keyWrapParams;
  }
  if (recipientWraps) {
    normalized.recipient_wraps = recipientWraps;
  }

  return normalized;
}

function normalizePayloadEncryptedFromWire(payloadEncrypted) {
  if (!payloadEncrypted) {
    return null;
  }

  const normalized = { ...payloadEncrypted };
  if (payloadEncrypted.key_wrap_params) {
    normalized.keyWrapParams = fromSnakeCaseKeyWrapParams(payloadEncrypted.key_wrap_params);
  }
  if (payloadEncrypted.recipient_wraps) {
    normalized.recipientWraps = fromSnakeCaseRecipientWraps(payloadEncrypted.recipient_wraps);
  }
  return normalized;
}

export class SequencerClient {
  /**
   * @param {import('./config.js').SyncConfig} config
   */
  constructor(config) {
    this.config = config;
    this._connected = false;
    this.securityProfile = resolveSecurityProfile(
      config.securityProfile ?? config.sync?.securityProfile,
    );

    // Parse URL to determine REST endpoint
    const url = parseSequencerUrl(config.sequencerUrl);
    assertSecureTransportForProfile(
      this.securityProfile,
      isSecureSequencerProtocol(url.protocol),
      `Sequencer URL ${config.sequencerUrl}`,
      config.allowInsecureTransport === true || config.sequencer?.insecure === true,
    );
    if (url.protocol === 'grpc:' || url.protocol === 'grpcs:') {
      // Convert gRPC URL to REST
      const restProtocol = url.protocol === 'grpcs:' ? 'https:' : 'http:';
      this.baseUrl = `${restProtocol}//${url.host}`;
    } else {
      this.baseUrl = config.sequencerUrl.replace(/\/$/, '');
    }
  }

  /**
   * Get authentication headers
   * @returns {Object}
   */
  _getHeaders() {
    const headers = {
      'Content-Type': 'application/json',
    };

    const creds = this.config.getCredentials();
    if (creds.apiKey) {
      // API keys with ss_ prefix go in x-api-key header;
      // otherwise fall back to Authorization Bearer
      if (creds.apiKey.startsWith('ss_')) {
        headers['x-api-key'] = creds.apiKey;
      } else {
        headers['Authorization'] = `Bearer ${creds.apiKey}`;
      }
    } else if (creds.jwt) {
      headers['Authorization'] = `Bearer ${creds.jwt}`;
    }

    return headers;
  }

  /**
   * Make an HTTP request
   * @param {string} method
   * @param {string} path
   * @param {Object} [body]
   * @returns {Promise<Object>}
   */
  async _request(method, path, body) {
    const url = `${this.baseUrl}${path}`;
    const options = {
      method,
      headers: this._getHeaders(),
    };

    if (body) {
      options.body = JSON.stringify(body);
    }

    const response = await fetch(url, options);

    if (!response.ok) {
      const text = await response.text();
      throw new Error(`Sequencer request failed: ${response.status} ${text}`);
    }

    return response.json();
  }

  /**
   * Connect to the sequencer (verify connectivity)
   * @returns {Promise<void>}
   */
  async connect() {
    try {
      await this._request('GET', '/health');
      this._connected = true;
    } catch (error) {
      this._connected = false;
      throw new Error(`Failed to connect to sequencer: ${error.message}`);
    }
  }

  /**
   * Disconnect from the sequencer
   * @returns {Promise<void>}
   */
  async disconnect() {
    this._connected = false;
  }

  /**
   * Check if connected
   * @returns {boolean}
   */
  isConnected() {
    return this._connected;
  }

  /**
   * Push a batch of VES v1.0 events to the sequencer
   * @param {Object} batch
   * @param {string} batch.agentId
   * @param {Array<EventEnvelope>} batch.events
   * @returns {Promise<IngestReceipt>}
   */
  async push(batch) {
    for (const event of batch.events) {
      assertEventMatchesSecurityProfile(event, this.securityProfile);
    }

    // Build VES v1.0 request
    // Top-level uses camelCase (VesIngestRequest), event envelope uses snake_case (VesEventEnvelope)
    const payload = {
      agentId: batch.agentId,
      events: batch.events.map((e) => ({
        // Core event fields (snake_case for VesEventEnvelope)
        event_id: e.eventId,
        command_id: e.commandId || null,
        tenant_id: e.tenantId,
        store_id: e.storeId,
        entity_type: e.entityType,
        entity_id: e.entityId,
        event_type: e.eventType,
        // VES v1.0 payload fields
        ves_version: e.vesVersion || 1,
        payload: e.payloadKind === 0 ? e.payload : null,
        payload_kind: e.payloadKind || 0,
        payload_encrypted: normalizePayloadEncryptedForWire(e.payloadEncrypted),
        payload_plain_hash: e.payloadPlainHash,
        payload_cipher_hash: e.payloadCipherHash,
        // VES v1.0 signature fields
        agent_key_id: e.agentKeyId,
        agent_signature: e.agentSignature,
        agent_signature_scheme: e.agentSignatureScheme || 0,
        agent_signature_bundle: toSnakeCaseSignatureBundle(e.agentSignatureBundle),
        source_agent_id: e.sourceAgent,
        // Metadata
        base_version: e.baseVersion || null,
        created_at: e.createdAt,
      })),
    };

    // Use VES v1.0 endpoint with signature verification
    const response = await this._request('POST', '/api/v1/ves/events/ingest', payload);

    return {
      batchId: response.batchId,
      eventsAccepted: response.eventsAccepted,
      eventsRejected: response.eventsRejected || 0,
      sequenceStart: response.sequenceStart,
      sequenceEnd: response.sequenceEnd,
      headSequence: response.headSequence,
      rejections: response.rejections || [],
      receipts: response.receipts || [],
    };
  }

  /**
   * Push with automatic retry
   * @param {Object} batch
   * @param {number} [maxRetries]
   * @returns {Promise<IngestReceipt>}
   */
  async pushWithRetry(batch, maxRetries = 3) {
    const { retryPolicy } = this.config;
    const max = maxRetries || retryPolicy.maxRetries;
    let lastError;

    for (let attempt = 0; attempt <= max; attempt++) {
      try {
        return await this.push(batch);
      } catch (error) {
        lastError = error;

        if (attempt < max) {
          const delay = Math.min(
            retryPolicy.baseDelay * Math.pow(2, attempt),
            retryPolicy.maxDelay,
          );
          await new Promise((resolve) => setTimeout(resolve, delay));
        }
      }
    }

    throw lastError;
  }

  /**
   * Pull VES v1.0 events from the sequencer
   * @param {number} fromSequence - Start sequence number
   * @param {number} [limit=100] - Max events to pull
   * @returns {Promise<{events: SequencedEvent[], nextSequence: number, hasMore: boolean, headSequence: number}>}
   */
  async pull(fromSequence, limit = 100) {
    const params = new URLSearchParams({
      tenant_id: this.config.tenantId,
      store_id: this.config.storeId,
      from: fromSequence.toString(),
      limit: limit.toString(),
    });

    const response = await this._request('GET', `/api/v1/events?${params}`);

    const events = (response.events || []).map((e) => ({
      envelope: {
        // Core event fields
        eventId: e.envelope.event_id,
        commandId: e.envelope.command_id,
        tenantId: e.envelope.tenant_id,
        storeId: e.envelope.store_id,
        entityType: e.envelope.entity_type,
        entityId: e.envelope.entity_id,
        eventType: e.envelope.event_type,
        // VES v1.0 payload fields
        payload: e.envelope.payload,
        vesVersion: e.envelope.ves_version || 1,
        payloadKind: e.envelope.payload_kind || 0,
        payloadEncrypted: normalizePayloadEncryptedFromWire(e.envelope.payload_encrypted),
        // Backwards compat: use payload_hash if payload_plain_hash not present
        payloadPlainHash: e.envelope.payload_plain_hash || e.envelope.payload_hash,
        payloadCipherHash:
          e.envelope.payload_cipher_hash ||
          '0000000000000000000000000000000000000000000000000000000000000000',
        // VES v1.0 signature fields (may be null for legacy events)
        agentKeyId: e.envelope.agent_key_id || 0,
        agentSignature:
          e.envelope.agent_signature ||
          '0000000000000000000000000000000000000000000000000000000000000000',
        agentSignatureScheme: e.envelope.agent_signature_scheme || 0,
        agentSignatureBundle: fromSnakeCaseSignatureBundle(e.envelope.agent_signature_bundle),
        // Metadata
        baseVersion: e.envelope.base_version,
        createdAt: e.envelope.created_at,
        sourceAgent: e.envelope.source_agent,
      },
      sequenceNumber: e.envelope.sequence_number,
      sequencedAt: e.sequenced_at,
      receiptHash: e.receipt_hash,
      receiptSignatureScheme: e.receipt_signature_scheme || 0,
      receiptSignatureBundle: fromSnakeCaseSignatureBundle(e.receipt_signature_bundle),
    }));

    const maxSeq =
      events.length > 0 ? Math.max(...events.map((e) => e.sequenceNumber)) : fromSequence;

    return {
      events,
      nextSequence: maxSeq + 1,
      hasMore: events.length === limit,
      headSequence: response.head_sequence || maxSeq,
    };
  }

  /**
   * Pull events as async iterator
   * @param {number} fromSequence
   * @returns {AsyncIterable<SequencedEvent>}
   */
  async *pullStream(fromSequence) {
    let cursor = fromSequence;
    let hasMore = true;

    while (hasMore) {
      const result = await this.pull(cursor, 100);

      for (const event of result.events) {
        yield event;
      }

      cursor = result.nextSequence;
      hasMore = result.hasMore;
    }
  }

  /**
   * Get the current head sequence
   * @returns {Promise<SyncState>}
   */
  async getHead() {
    const params = new URLSearchParams({
      tenant_id: this.config.tenantId,
      store_id: this.config.storeId,
    });

    const response = await this._request('GET', `/api/v1/head?${params}`);

    return {
      tenantId: this.config.tenantId,
      storeId: this.config.storeId,
      headSequence: response.head_sequence || 0,
      stateRoot: response.state_root,
      lastCommitmentId: response.latest_commitment?.batch_id,
    };
  }

  /**
   * Get a batch commitment
   * @param {string} batchId
   * @returns {Promise<BatchCommitment|null>}
   */
  async getCommitment(batchId) {
    try {
      const response = await this._request('GET', `/api/v1/commitments/${batchId}`);

      return {
        batchId: response.batch_id,
        merkleRoot: response.merkle_root,
        startSequence: response.start_sequence,
        endSequence: response.end_sequence,
        eventCount: response.event_count,
        committedAt: response.committed_at,
      };
    } catch (error) {
      if (error.message.includes('404')) {
        return null;
      }
      throw error;
    }
  }

  /**
   * Get entity event history (VES v1.0)
   * @param {string} entityType
   * @param {string} entityId
   * @returns {Promise<SequencedEvent[]>}
   */
  async getEntityHistory(entityType, entityId) {
    const params = new URLSearchParams({
      tenant_id: this.config.tenantId,
      store_id: this.config.storeId,
    });

    const response = await this._request(
      'GET',
      `/api/v1/entities/${entityType}/${entityId}?${params}`,
    );

    return (response.events || []).map((e) => ({
      envelope: {
        // Core event fields
        eventId: e.event_id,
        commandId: e.command_id,
        tenantId: e.tenant_id,
        storeId: e.store_id,
        entityType: e.entity_type,
        entityId: e.entity_id,
        eventType: e.event_type,
        // VES v1.0 payload fields
        payload: e.payload,
        vesVersion: e.ves_version || 1,
        payloadKind: e.payload_kind || 0,
        payloadEncrypted: normalizePayloadEncryptedFromWire(e.payload_encrypted),
        payloadPlainHash: e.payload_plain_hash,
        payloadCipherHash: e.payload_cipher_hash,
        // VES v1.0 signature fields
        agentKeyId: e.agent_key_id,
        agentSignature: e.agent_signature,
        // Metadata
        baseVersion: e.base_version,
        createdAt: e.created_at,
        sourceAgent: e.source_agent,
      },
      sequenceNumber: e.sequence_number,
      sequencedAt: e.sequenced_at,
      receiptHash: e.receipt_hash,
    }));
  }

  /**
   * Verify VES v1.0 inclusion proof (client-side verification)
   * Uses domain-separated hashing per VES v1.0 Section 11
   * @param {EventEnvelope} envelope - The event envelope
   * @param {InclusionProof} proof
   * @param {string} expectedRoot - Expected merkle root (hex)
   * @returns {boolean}
   */
  verifyInclusion(envelope, proof, expectedRoot) {
    // Reconstruct the event signing hash (same computation as verifyEventSignature).
    // payloadKind MUST be bound here: it is part of the canonical event signing
    // preimage (computeEventSigningHash hashes u32BE(payloadKind)). Omitting it
    // silently defaults to 0 (plaintext), letting an inclusion proof built for an
    // encrypted envelope (payloadKind=1) verify as plaintext and vice versa.
    const eventSigningHash = computeEventSigningHash({
      vesVersion: envelope.vesVersion || 1,
      tenantId: envelope.tenantId,
      storeId: envelope.storeId,
      eventId: envelope.eventId,
      commandId: envelope.commandId || null,
      sourceAgentId: envelope.sourceAgent,
      agentKeyId: envelope.agentKeyId,
      entityType: envelope.entityType,
      entityId: envelope.entityId,
      eventType: envelope.eventType,
      payloadKind: envelope.payloadKind || 0,
      baseVersion: envelope.baseVersion || null,
      createdAt: envelope.createdAt,
      payloadPlainHash: hexToBuffer(envelope.payloadPlainHash),
      payloadCipherHash: hexToBuffer(envelope.payloadCipherHash),
    });

    // Compute VES v1.0 leaf hash with the correct parameter shape
    let hash = computeLeafHash({
      tenantId: envelope.tenantId,
      storeId: envelope.storeId,
      sequenceNumber: envelope.sequenceNumber ?? proof.leafIndex,
      eventSigningHash,
      agentSignature: hexToBuffer(envelope.agentSignature),
    });

    // Walk up the proof using VES v1.0 node hashing
    for (let i = 0; i < proof.proofHashes.length; i++) {
      const sibling = Buffer.from(proof.proofHashes[i], 'hex');
      const isLeftSibling = (proof.leafIndex >> i) & 1;

      if (isLeftSibling) {
        hash = computeNodeHash(sibling, hash);
      } else {
        hash = computeNodeHash(hash, sibling);
      }
    }

    return hash.toString('hex') === expectedRoot;
  }

  /**
   * Verify event signature (VES v1.0)
   * @param {EventEnvelope} envelope - The event envelope
   * @param {Buffer|string|Object} publicKey - Agent Ed25519 key or hybrid public-key bundle
   * @returns {boolean}
   */
  verifyEventSignature(envelope, publicKey) {
    // Reconstruct the signing hash. payloadKind is part of the canonical signing
    // preimage and MUST be bound so an encrypted envelope cannot verify under the
    // plaintext signing hash (or vice versa).
    const eventSigningHash = computeEventSigningHash({
      vesVersion: envelope.vesVersion || 1,
      tenantId: envelope.tenantId,
      storeId: envelope.storeId,
      eventId: envelope.eventId,
      commandId: envelope.commandId || null,
      sourceAgentId: envelope.sourceAgent,
      agentKeyId: envelope.agentKeyId,
      entityType: envelope.entityType,
      entityId: envelope.entityId,
      eventType: envelope.eventType,
      payloadKind: envelope.payloadKind || 0,
      baseVersion: envelope.baseVersion || null,
      createdAt: envelope.createdAt,
      payloadPlainHash: hexToBuffer(envelope.payloadPlainHash),
      payloadCipherHash: hexToBuffer(envelope.payloadCipherHash),
    });

    const publicKeyBundle = normalizeVerificationPublicKeyBundle(publicKey);
    const signatureBundle = envelope.agentSignatureBundle || null;

    if (
      Number(envelope.agentSignatureScheme || 0) === SIGNATURE_SCHEME_ED25519_ML_DSA_65 &&
      signatureBundle &&
      publicKeyBundle?.ed25519PublicKey &&
      publicKeyBundle?.mlDsa65PublicKey
    ) {
      try {
        return verifyEventSignatureHybrid(eventSigningHash, signatureBundle, publicKeyBundle);
      } catch (error) {
        console.debug(
          '[sync-client] Hybrid signature verification failed:',
          error?.message || error,
        );
      }
    }

    // Fall back to verifying the classical Ed25519 component when a full
    // hybrid bundle is unavailable to the caller.
    const signatureHex = envelope.agentSignature || signatureBundle?.ed25519Signature || null;
    if (!signatureHex) {
      return false;
    }
    const ed25519PublicKey = publicKeyBundle?.ed25519PublicKey ?? publicKey;
    if (!ed25519PublicKey) {
      return false;
    }
    const signature = hexToBuffer(signatureHex);
    return verifyEventSignature(
      eventSigningHash,
      signature,
      typeof ed25519PublicKey === 'string' ? hexToBuffer(ed25519PublicKey) : ed25519PublicKey,
    );
  }

  /**
   * Verify a receipt signature against a known sequencer public key.
   *
   * Supports legacy (Ed25519), hybrid (Ed25519 + ML-DSA-65), and
   * PQC-strict (ML-DSA-65) receipt signatures per VES-RECEIPT-2.
   *
   * @param {Object} receipt - Receipt or sequenced event with receipt fields.
   * @param {Buffer|string} receipt.receiptHash - 32-byte receipt hash.
   * @param {number} [receipt.receiptSignatureScheme] - Signature scheme identifier.
   * @param {Object} [receipt.receiptSignatureBundle] - Signature bundle.
   * @param {Buffer|string|Object} sequencerPublicKey - Sequencer public key or bundle.
   * @returns {boolean} True if the receipt signature is valid.
   */
  verifyReceiptSignature(receipt, sequencerPublicKey) {
    const receiptHash =
      typeof receipt.receiptHash === 'string'
        ? hexToBuffer(receipt.receiptHash)
        : receipt.receiptHash;
    if (!receiptHash || receiptHash.length !== 32) {
      return false;
    }

    const scheme = Number(receipt.receiptSignatureScheme || 0);
    const bundle = receipt.receiptSignatureBundle || null;

    // Validate receipt matches the active security profile (informational)
    try {
      assertReceiptMatchesSecurityProfile(
        { signatureScheme: scheme, signatureBundle: bundle },
        this.securityProfile,
      );
    } catch {
      // Profile mismatch — still attempt verification but log
      // istanbul ignore next
      if (typeof console !== 'undefined' && console.debug) {
        console.debug('[sync-client] Receipt profile mismatch for current security profile');
      }
    }

    const publicKeyBundle = normalizeVerificationPublicKeyBundle(sequencerPublicKey);

    // PQC-strict: ML-DSA-65 only
    if (
      scheme === SIGNATURE_SCHEME_ML_DSA_65 &&
      bundle?.mlDsa65Signature &&
      publicKeyBundle?.mlDsa65PublicKey
    ) {
      try {
        return verifyEventSignatureStrict(
          receiptHash,
          typeof bundle.mlDsa65Signature === 'string'
            ? hexToBuffer(bundle.mlDsa65Signature)
            : bundle.mlDsa65Signature,
          publicKeyBundle,
        );
      } catch {
        return false;
      }
    }

    // Hybrid: Ed25519 + ML-DSA-65
    if (
      scheme === SIGNATURE_SCHEME_ED25519_ML_DSA_65 &&
      bundle &&
      publicKeyBundle?.ed25519PublicKey &&
      publicKeyBundle?.mlDsa65PublicKey
    ) {
      try {
        return verifyEventSignatureHybrid(receiptHash, bundle, publicKeyBundle);
      } catch {
        return false;
      }
    }

    // Legacy: Ed25519
    const sigHex = bundle?.ed25519Signature || receipt.receiptSignature;
    const ed25519Pk = publicKeyBundle?.ed25519PublicKey ?? sequencerPublicKey;
    if (!sigHex || !ed25519Pk) {
      return false;
    }
    return verifyEventSignature(
      receiptHash,
      typeof sigHex === 'string' ? hexToBuffer(sigHex) : sigHex,
      typeof ed25519Pk === 'string' ? hexToBuffer(ed25519Pk) : ed25519Pk,
    );
  }

  /**
   * Register agent public key with the sequencer
   * @param {Object} keyRegistration
   * @param {string} keyRegistration.agentId - Agent UUID
   * @param {number} keyRegistration.keyId - Key ID
   * @param {string} keyRegistration.publicKey - Ed25519 public key (hex)
   * @param {number} [keyRegistration.keyType] - Signing or encryption key type
   * @param {number} [keyRegistration.keyAlgorithm] - Concrete key algorithm or hybrid bundle
   * @param {Object} [keyRegistration.publicKeyBundle] - PQ or hybrid key bundle
   * @param {string} [keyRegistration.validFrom] - Validity start (ISO)
   * @param {string} [keyRegistration.validTo] - Validity end (ISO)
   * @returns {Promise<{success: boolean}>}
   */
  async registerAgentKey(keyRegistration) {
    assertKeyRegistrationMatchesSecurityProfile(keyRegistration, this.securityProfile);

    const payload = {
      tenant_id: this.config.tenantId,
      agent_id: keyRegistration.agentId,
      key_id: keyRegistration.keyId,
      key_type: keyRegistration.keyType,
      key_algorithm: keyRegistration.keyAlgorithm,
      public_key: keyRegistration.publicKey,
      public_key_bundle: toSnakeCasePublicKeyBundle(keyRegistration.publicKeyBundle),
      valid_from: keyRegistration.validFrom,
      valid_to: keyRegistration.validTo,
      proof_of_possession: keyRegistration.proofOfPossession,
      proof_of_possession_bundle: toSnakeCaseProofOfPossessionBundle(
        keyRegistration.proofOfPossessionBundle,
      ),
    };

    const response = await this._request('POST', '/api/v1/agents/keys', payload);
    return { success: response.success !== false };
  }

  /**
   * Get agent's registered public keys
   * @param {string} agentId - Agent UUID
   * @returns {Promise<Array<{keyId: number, publicKey: string, status: string, createdAt: string}>>}
   */
  async getAgentKeys(agentId) {
    const params = new URLSearchParams({
      tenant_id: this.config.tenantId,
      agent_id: agentId,
    });

    const response = await this._request('GET', `/api/v1/agents/keys?${params}`);

    return (response.keys || []).map((k) => ({
      keyId: k.key_id,
      keyType: k.key_type,
      keyAlgorithm: k.key_algorithm,
      publicKey: k.public_key,
      publicKeyBundle: fromSnakeCasePublicKeyBundle(k.public_key_bundle),
      status: k.status,
      createdAt: k.created_at,
      validFrom: k.valid_from,
      validTo: k.valid_to,
    }));
  }
}

/**
 * Create a sequencer client
 * @param {import('./config.js').SyncConfig} config
 * @returns {SequencerClient}
 */
export function createSequencerClient(config) {
  return new SequencerClient(config);
}

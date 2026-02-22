/**
 * Sequencer REST Client (VES v1.0)
 *
 * Client for communicating with the stateset-sequencer service.
 * Supports VES v1.0 signed events and encrypted payloads.
 */

import {
  computeEventSigningHash,
  verifyEventSignature,
  computeLeafHash,
  computeNodeHash,
  hexToBuffer,
} from './crypto.js';

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

export class SequencerClient {
  /**
   * @param {import('./config.js').SyncConfig} config
   */
  constructor(config) {
    this.config = config;
    this._connected = false;

    // Parse URL to determine REST endpoint
    const url = parseSequencerUrl(config.sequencerUrl);
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
      headers['Authorization'] = `Bearer ${creds.apiKey}`;
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
        payload_encrypted: e.payloadEncrypted || null,
        payload_plain_hash: e.payloadPlainHash,
        payload_cipher_hash: e.payloadCipherHash,
        // VES v1.0 signature fields
        agent_key_id: e.agentKeyId,
        agent_signature: e.agentSignature,
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
        payloadEncrypted: e.envelope.payload_encrypted,
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
        // Metadata
        baseVersion: e.envelope.base_version,
        createdAt: e.envelope.created_at,
        sourceAgent: e.envelope.source_agent,
      },
      sequenceNumber: e.envelope.sequence_number,
      sequencedAt: e.sequenced_at,
      receiptHash: e.receipt_hash,
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
        payloadEncrypted: e.payload_encrypted,
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
    // Compute VES v1.0 leaf hash
    let hash = computeLeafHash({
      eventId: envelope.eventId,
      payloadPlainHash: hexToBuffer(envelope.payloadPlainHash),
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
   * @param {Buffer} publicKey - Agent's Ed25519 public key (32 bytes)
   * @returns {boolean}
   */
  verifyEventSignature(envelope, publicKey) {
    // Reconstruct the signing hash
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
      baseVersion: envelope.baseVersion || null,
      createdAt: envelope.createdAt,
      payloadPlainHash: hexToBuffer(envelope.payloadPlainHash),
      payloadCipherHash: hexToBuffer(envelope.payloadCipherHash),
    });

    // Verify the signature
    const signature = hexToBuffer(envelope.agentSignature);
    return verifyEventSignature(eventSigningHash, signature, publicKey);
  }

  /**
   * Register agent public key with the sequencer
   * @param {Object} keyRegistration
   * @param {string} keyRegistration.agentId - Agent UUID
   * @param {number} keyRegistration.keyId - Key ID
   * @param {string} keyRegistration.publicKey - Ed25519 public key (hex)
   * @param {string} [keyRegistration.validFrom] - Validity start (ISO)
   * @param {string} [keyRegistration.validTo] - Validity end (ISO)
   * @returns {Promise<{success: boolean}>}
   */
  async registerAgentKey(keyRegistration) {
    const payload = {
      tenant_id: this.config.tenantId,
      agent_id: keyRegistration.agentId,
      key_id: keyRegistration.keyId,
      public_key: keyRegistration.publicKey,
      valid_from: keyRegistration.validFrom,
      valid_to: keyRegistration.validTo,
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
      publicKey: k.public_key,
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

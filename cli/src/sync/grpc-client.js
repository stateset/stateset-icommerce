/**
 * gRPC Client for StateSet Sequencer v2
 *
 * Provides real-time bidirectional sync with the VES sequencer using gRPC.
 * Supports streaming events, push/pull operations, and automatic reconnection.
 *
 * Requires:
 *   npm install @grpc/grpc-js @grpc/proto-loader
 *
 * Usage:
 *   const client = new GrpcSequencerClient({
 *     url: 'sequencer.stateset.io:8081',
 *     tenantId: 'tenant-uuid',
 *     storeId: 'store-uuid',
 *     agentId: 'agent-uuid',
 *     apiKey: 'ss_xxxxxxxx...',
 *   });
 *   await client.connect();
 *   await client.pushEvents(events);
 *   client.onEvent((event) => console.log(event));
 */

import { EventEmitter } from 'events';
import path from 'path';
import { fileURLToPath } from 'url';
import {
  computeLegacyPayloadHash,
  hexToBuffer,
  verifyEventSignature,
  verifyEventSignatureHybrid,
  verifyEventSignatureStrict,
} from './crypto.js';
import {
  KEY_WRAP_SCHEME_X25519_HKDF_SHA256,
  SIGNATURE_SCHEME_ED25519_ML_DSA_65,
  SIGNATURE_SCHEME_ML_DSA_65,
  assertEventMatchesSecurityProfile,
  assertKeyRegistrationMatchesSecurityProfile,
  assertReceiptMatchesSecurityProfile,
  assertSecureTransportForProfile,
  resolveSecurityProfile,
} from './pqc.js';

// Dynamic imports for gRPC (optional dependency)
let grpc = null;
let protoLoader = null;

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PROTO_PATH = path.join(__dirname, 'proto', 'sequencer_v2.proto');

// =============================================================================
// GRPC CLIENT CONFIGURATION
// =============================================================================

/**
 * @typedef {Object} GrpcClientConfig
 * @property {string} url - Sequencer URL (host:port)
 * @property {string} tenantId - Tenant UUID
 * @property {string} storeId - Store UUID
 * @property {string} agentId - Agent UUID
 * @property {boolean} [tls=true] - Use TLS
 * @property {string} [certPath] - Custom CA certificate path
 * @property {string} [apiKey] - API key for authentication
 * @property {string} [jwtToken] - JWT token for authentication
 * @property {boolean} [allowInsecureTransport=false] - Allow insecure legacy transport explicitly
 * @property {Object} [retryPolicy] - Retry configuration
 * @property {number} [retryPolicy.maxRetries=5]
 * @property {number} [retryPolicy.baseDelayMs=1000]
 * @property {number} [retryPolicy.maxDelayMs=30000]
 * @property {number} [keepaliveMs=30000] - Keepalive interval
 */

const DEFAULT_CONFIG = {
  tls: true,
  securityProfile: 'hybrid',
  retryPolicy: {
    maxRetries: 5,
    baseDelayMs: 1000,
    maxDelayMs: 30000,
  },
  keepaliveMs: 30000,
  batchSize: 100,
  streamBufferSize: 1000,
};

function toProtoSignatureBundle(bundle) {
  if (!bundle) {
    return null;
  }

  return {
    ed25519_signature: bundle.ed25519Signature ?? bundle.ed25519_signature ?? null,
    ml_dsa_65_signature: bundle.mlDsa65Signature ?? bundle.ml_dsa_65_signature ?? null,
  };
}

function fromProtoSignatureBundle(bundle) {
  if (!bundle) {
    return null;
  }

  return {
    ed25519Signature: bundle.ed25519_signature ?? bundle.ed25519Signature ?? null,
    mlDsa65Signature: bundle.ml_dsa_65_signature ?? bundle.mlDsa65Signature ?? null,
  };
}

function toProtoPublicKeyBundle(bundle) {
  if (!bundle) {
    return null;
  }

  return {
    ed25519_public_key: bundle.ed25519PublicKey ?? bundle.ed25519_public_key ?? null,
    ml_dsa_65_public_key: bundle.mlDsa65PublicKey ?? bundle.ml_dsa_65_public_key ?? null,
    x25519_public_key: bundle.x25519PublicKey ?? bundle.x25519_public_key ?? null,
    ml_kem_768_public_key:
      bundle.mlKem768PublicKey ?? bundle.ml_kem_768_public_key ?? null,
  };
}

function fromProtoPublicKeyBundle(bundle) {
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

function toProtoProofOfPossessionBundle(bundle) {
  if (!bundle) {
    return null;
  }

  return {
    ed25519_pop: bundle.ed25519Pop ?? bundle.ed25519_pop ?? null,
    ml_dsa_65_pop: bundle.mlDsa65Pop ?? bundle.ml_dsa_65_pop ?? null,
  };
}

function fromHexOrBuffer(value) {
  if (value === null || value === undefined) {
    return Buffer.alloc(0);
  }
  if (Buffer.isBuffer(value)) {
    return value;
  }
  if (value instanceof Uint8Array) {
    return Buffer.from(value);
  }
  if (typeof value === 'string') {
    return hexToBuffer(value);
  }
  return Buffer.from(value);
}

function fromBase64UrlOrBuffer(value) {
  if (value === null || value === undefined) {
    return Buffer.alloc(0);
  }
  if (Buffer.isBuffer(value)) {
    return value;
  }
  if (value instanceof Uint8Array) {
    return Buffer.from(value);
  }
  if (typeof value === 'string') {
    return Buffer.from(value, 'base64url');
  }
  return Buffer.from(value);
}

function toBase64Url(value) {
  if (!value) {
    return '';
  }
  return Buffer.from(value).toString('base64url');
}

function toProtoKeyWrapParams(params) {
  if (!params) {
    return null;
  }

  return {
    scheme: Number(params.scheme ?? params.wrapScheme ?? params.wrap_scheme ?? 0),
    kdf: params.kdf ?? '',
    aead: params.aead ?? '',
  };
}

function fromProtoKeyWrapParams(params) {
  if (!params) {
    return null;
  }

  return {
    scheme: Number(params.scheme ?? params.wrapScheme ?? params.wrap_scheme ?? 0),
    kdf: params.kdf ?? null,
    aead: params.aead ?? null,
  };
}

function toProtoRecipientWrap(wrap) {
  if (!wrap) {
    return null;
  }

  return {
    recipient_kid: Number(wrap.recipientKid ?? wrap.recipient_kid ?? 0),
    wrap_scheme: Number(wrap.wrapScheme ?? wrap.wrap_scheme ?? 0),
    x25519_enc: fromBase64UrlOrBuffer(
      wrap.x25519Enc ?? wrap.x25519_enc ?? wrap.x25519_enc_b64u ?? null,
    ),
    ml_kem_ciphertext: fromBase64UrlOrBuffer(
      wrap.mlKemCiphertext ??
        wrap.ml_kem_ciphertext ??
        wrap.ml_kem_ciphertext_b64u ??
        wrap.mlkem_ct_b64u ??
        null,
    ),
    wrap_nonce: fromBase64UrlOrBuffer(
      wrap.wrapNonce ?? wrap.wrap_nonce ?? wrap.wrap_nonce_b64u ?? null,
    ),
    wrapped_key: fromBase64UrlOrBuffer(
      wrap.wrappedKey ??
        wrap.wrapped_key ??
        wrap.wrapped_key_b64u ??
        wrap.ct_b64u ??
        null,
    ),
  };
}

function fromProtoRecipientWrap(wrap) {
  if (!wrap) {
    return null;
  }

  return {
    recipientKid: Number(wrap.recipient_kid ?? wrap.recipientKid ?? 0),
    wrapScheme: Number(wrap.wrap_scheme ?? wrap.wrapScheme ?? 0),
    x25519Enc: wrap.x25519_enc?.length ? toBase64Url(wrap.x25519_enc) : null,
    mlKemCiphertext:
      wrap.ml_kem_ciphertext?.length ? toBase64Url(wrap.ml_kem_ciphertext) : null,
    wrapNonce: wrap.wrap_nonce?.length ? toBase64Url(wrap.wrap_nonce) : null,
    wrappedKey: wrap.wrapped_key?.length ? toBase64Url(wrap.wrapped_key) : null,
  };
}

function deriveProtoKeyWrapParams(payloadEncrypted) {
  if (!Array.isArray(payloadEncrypted?.recipients) || payloadEncrypted.recipients.length === 0) {
    return null;
  }

  return {
    scheme: KEY_WRAP_SCHEME_X25519_HKDF_SHA256,
    kdf: payloadEncrypted.hpke?.kdf ?? 'HKDF-SHA256',
    aead: payloadEncrypted.hpke?.aead ?? payloadEncrypted.aead ?? 'AES-256-GCM',
  };
}

function deriveProtoRecipientWraps(payloadEncrypted) {
  if (!Array.isArray(payloadEncrypted?.recipients)) {
    return null;
  }

  return payloadEncrypted.recipients.map((recipient) => ({
    recipient_kid: Number(recipient.recipient_kid ?? recipient.recipientKid ?? 0),
    wrap_scheme: KEY_WRAP_SCHEME_X25519_HKDF_SHA256,
    x25519_enc: fromBase64UrlOrBuffer(recipient.enc_b64u ?? recipient.encB64u ?? null),
    ml_kem_ciphertext: Buffer.alloc(0),
    wrap_nonce: Buffer.alloc(0),
    wrapped_key: fromBase64UrlOrBuffer(
      recipient.wrapped_key_b64u ?? recipient.wrappedKeyB64u ?? recipient.ct_b64u ?? null,
    ),
  }));
}

function toProtoEncryptedPayload(payloadEncrypted) {
  if (!payloadEncrypted) {
    return null;
  }

  const keyWrapParams =
    toProtoKeyWrapParams(payloadEncrypted.keyWrapParams ?? payloadEncrypted.key_wrap_params) ??
    deriveProtoKeyWrapParams(payloadEncrypted);
  const recipientWraps =
    (Array.isArray(payloadEncrypted.recipientWraps ?? payloadEncrypted.recipient_wraps)
      ? (payloadEncrypted.recipientWraps ?? payloadEncrypted.recipient_wraps)
          .map((wrap) => toProtoRecipientWrap(wrap))
          .filter(Boolean)
      : null) ?? deriveProtoRecipientWraps(payloadEncrypted);

  return {
    enc_version: payloadEncrypted.enc_version ?? payloadEncrypted.encVersion ?? 1,
    aead: payloadEncrypted.aead ?? 'AES-256-GCM',
    nonce: fromBase64UrlOrBuffer(payloadEncrypted.nonce_b64u ?? payloadEncrypted.nonce),
    ciphertext: fromBase64UrlOrBuffer(
      payloadEncrypted.ciphertext_b64u ?? payloadEncrypted.ciphertext,
    ),
    tag: fromBase64UrlOrBuffer(payloadEncrypted.tag_b64u ?? payloadEncrypted.tag),
    hpke: payloadEncrypted.hpke || null,
    recipients: Array.isArray(payloadEncrypted.recipients)
      ? payloadEncrypted.recipients.map((recipient) => ({
          recipient_kid: Number(recipient.recipient_kid ?? recipient.recipientKid ?? 0),
          ephemeral_public_key: fromBase64UrlOrBuffer(
            recipient.enc_b64u ?? recipient.encB64u ?? recipient.ephemeral_public_key ?? null,
          ),
          wrapped_dek: fromBase64UrlOrBuffer(
            recipient.wrapped_key_b64u ??
              recipient.wrappedKeyB64u ??
              recipient.ct_b64u ??
              recipient.wrapped_dek ??
              null,
          ),
        }))
      : [],
    key_wrap_params: keyWrapParams,
    recipient_wraps: recipientWraps || [],
  };
}

function fromProtoEncryptedPayload(payloadEncrypted) {
  if (!payloadEncrypted) {
    return null;
  }

  const normalized = {
    enc_version: payloadEncrypted.enc_version ?? 1,
    aead: payloadEncrypted.aead ?? 'AES-256-GCM',
    nonce_b64u: toBase64Url(payloadEncrypted.nonce),
    ciphertext_b64u: toBase64Url(payloadEncrypted.ciphertext),
    tag_b64u: toBase64Url(payloadEncrypted.tag),
    hpke: payloadEncrypted.hpke || null,
    recipients: Array.isArray(payloadEncrypted.recipients)
      ? payloadEncrypted.recipients.map((recipient) => ({
          recipient_kid: Number(recipient.recipient_kid ?? recipient.recipientKid ?? 0),
          enc_b64u: toBase64Url(recipient.ephemeral_public_key),
          ct_b64u: toBase64Url(recipient.wrapped_dek),
        }))
      : [],
  };

  if (payloadEncrypted.key_wrap_params) {
    normalized.keyWrapParams = fromProtoKeyWrapParams(payloadEncrypted.key_wrap_params);
    normalized.key_wrap_params = payloadEncrypted.key_wrap_params;
  }

  if (Array.isArray(payloadEncrypted.recipient_wraps) && payloadEncrypted.recipient_wraps.length > 0) {
    normalized.recipientWraps = payloadEncrypted.recipient_wraps
      .map((wrap) => fromProtoRecipientWrap(wrap))
      .filter(Boolean);
  }

  return normalized;
}

function toProtoPayloadKind(payloadKind) {
  return Number(payloadKind ?? 0) === 1 ? 2 : 1;
}

function fromProtoPayloadKind(payloadKind) {
  return Number(payloadKind ?? 0) === 2 ? 1 : 0;
}

// =============================================================================
// GRPC SEQUENCER CLIENT
// =============================================================================

/**
 * gRPC client for StateSet VES Sequencer v2.
 * Extends EventEmitter to provide event-based notifications.
 *
 * Events emitted:
 * - 'connected' - Connection established
 * - 'disconnected' - Connection lost
 * - 'event' - New event received from stream
 * - 'sync-state' - Sync state update
 * - 'error' - Error occurred
 * - 'push-ack' - Push acknowledged
 */
export class GrpcSequencerClient extends EventEmitter {
  /**
   * @param {GrpcClientConfig} config
   */
  constructor(config) {
    super();
    this.config = {
      ...DEFAULT_CONFIG,
      ...config,
      securityProfile: resolveSecurityProfile(
        config?.securityProfile ?? DEFAULT_CONFIG.securityProfile,
      ),
    };
    this.connected = false;
    this.client = null;
    this.keyClient = null;
    this.syncStream = null;
    this.eventStream = null;
    this.lastSequence = 0;
    this.pendingAcks = new Map();
    this.reconnectAttempt = 0;
    this.heartbeatInterval = null;
    this._grpcLoaded = false;
    this._intentionalDisconnect = false; // Track intentional disconnect to prevent auto-reconnect

    // Validate required config
    if (!this.config.url) throw new Error('url is required');
    if (!this.config.tenantId) throw new Error('tenantId is required');
    if (!this.config.storeId) throw new Error('storeId is required');
    if (!this.config.agentId) throw new Error('agentId is required');
    assertSecureTransportForProfile(
      this.config.securityProfile,
      this.config.tls === true,
      `gRPC sequencer transport ${this.config.url}`,
      this.config.allowInsecureTransport === true || this.config.insecure === true,
    );
  }

  // ===========================================================================
  // INITIALIZATION
  // ===========================================================================

  /**
   * Load gRPC dependencies (lazy loaded to make them optional)
   */
  async _loadGrpc() {
    if (this._grpcLoaded) return;

    try {
      grpc = await import('@grpc/grpc-js');
      protoLoader = await import('@grpc/proto-loader');
      this._grpcLoaded = true;
    } catch (err) {
      console.debug('[grpc-client] gRPC import failed:', err.message || err);
      throw new Error(
        'gRPC dependencies not installed. Run: npm install @grpc/grpc-js @grpc/proto-loader',
      );
    }
  }

  /**
   * Load proto definitions
   */
  async _loadProto() {
    const packageDefinition = await protoLoader.load(PROTO_PATH, {
      keepCase: true,
      longs: String,
      enums: String,
      defaults: true,
      oneofs: true,
    });
    return grpc.loadPackageDefinition(packageDefinition);
  }

  /**
   * Create metadata with authentication
   */
  _createMetadata() {
    const metadata = new grpc.Metadata();

    if (this.config.apiKey) {
      metadata.set('authorization', this.config.apiKey);
    } else if (this.config.jwtToken) {
      metadata.set('authorization', `Bearer ${this.config.jwtToken}`);
    }

    return metadata;
  }

  // ===========================================================================
  // CONNECTION MANAGEMENT
  // ===========================================================================

  /**
   * Connect to the sequencer.
   * @returns {Promise<void>}
   */
  async connect() {
    await this._loadGrpc();

    const proto = await this._loadProto();
    const Sequencer = proto.stateset.sequencer.v2.Sequencer;
    const KeyManagement = proto.stateset.sequencer.v2.KeyManagement;

    // Create channel credentials
    const credentials = this.config.tls
      ? grpc.credentials.createSsl()
      : grpc.credentials.createInsecure();

    // Create clients
    this.client = new Sequencer(this.config.url, credentials);
    this.keyClient = new KeyManagement(this.config.url, credentials);

    // Wait for connection
    await new Promise((resolve, reject) => {
      const deadline = new Date();
      deadline.setSeconds(deadline.getSeconds() + 10);

      this.client.waitForReady(deadline, (err) => {
        if (err) {
          reject(new Error(`Failed to connect to ${this.config.url}: ${err.message}`));
        } else {
          resolve();
        }
      });
    });

    this.connected = true;
    this.reconnectAttempt = 0;
    this.emit('connected');

    // Start heartbeat
    this._startHeartbeat();
  }

  /**
   * Disconnect from the sequencer.
   */
  disconnect() {
    this._intentionalDisconnect = true; // Mark as intentional to prevent auto-reconnect
    this._stopHeartbeat();

    if (this.syncStream) {
      this.syncStream.end();
      this.syncStream = null;
    }

    if (this.eventStream) {
      this.eventStream.cancel();
      this.eventStream = null;
    }

    if (this.client) {
      grpc.closeClient(this.client);
      this.client = null;
    }

    if (this.keyClient) {
      grpc.closeClient(this.keyClient);
      this.keyClient = null;
    }

    this.connected = false;
    this.emit('disconnected');
  }

  /**
   * Reconnect with exponential backoff.
   */
  async _reconnect() {
    // Don't reconnect if disconnect was intentional
    if (this._intentionalDisconnect) {
      return;
    }

    // Use defaults if retryPolicy is not configured
    const maxRetries = this.config.retryPolicy?.maxRetries ?? 5;
    const baseDelayMs = this.config.retryPolicy?.baseDelayMs ?? 1000;
    const maxDelayMs = this.config.retryPolicy?.maxDelayMs ?? 30000;

    if (this.reconnectAttempt >= maxRetries) {
      this.emit('error', new Error('Max reconnection attempts reached'));
      return;
    }

    const delay = Math.min(baseDelayMs * Math.pow(2, this.reconnectAttempt), maxDelayMs);

    this.reconnectAttempt++;
    this.emit('reconnecting', { attempt: this.reconnectAttempt, delayMs: delay });

    await new Promise((resolve) => setTimeout(resolve, delay));

    try {
      await this.connect();
    } catch (err) {
      this.emit('error', err);
      await this._reconnect();
    }
  }

  _startHeartbeat() {
    this._stopHeartbeat();
    this.heartbeatInterval = setInterval(() => {
      if (this.syncStream) {
        this.syncStream.write({
          heartbeat: {
            timestamp: { seconds: Math.floor(Date.now() / 1000), nanos: 0 },
            last_seen_sequence: this.lastSequence,
          },
        });
      }
    }, this.config.keepaliveMs);
    if (this.heartbeatInterval.unref) this.heartbeatInterval.unref();
  }

  _stopHeartbeat() {
    if (this.heartbeatInterval) {
      clearInterval(this.heartbeatInterval);
      this.heartbeatInterval = null;
    }
  }

  // ===========================================================================
  // PUSH OPERATIONS
  // ===========================================================================

  /**
   * Push events to the sequencer.
   * @param {Array} events - Events to push
   * @returns {Promise<Object>} Push response
   */
  async pushEvents(events) {
    if (!this.connected || !this.client) {
      throw new Error('Not connected');
    }

    for (const event of events) {
      assertEventMatchesSecurityProfile(event, this.config.securityProfile);
    }

    const request = {
      agent_id: this.config.agentId,
      tenant_id: this.config.tenantId,
      store_id: this.config.storeId,
      events: events.map((e) => this._toProtoEvent(e)),
      request_id: crypto.randomUUID(),
    };

    return new Promise((resolve, reject) => {
      this.client.push(request, this._createMetadata(), (err, response) => {
        if (err) {
          reject(err);
        } else {
          // Update last sequence
          if (response.sequence_end > this.lastSequence) {
            this.lastSequence = Number(response.sequence_end);
          }
          this.emit('push-ack', response);
          resolve(response);
        }
      });
    });
  }

  /**
   * Convert local event to proto format
   *
   * Note: The gRPC sequencer uses legacy EventEnvelope which expects
   * canonical_json_hash (no domain prefix), NOT VES payload_plain_hash.
   */
  _toProtoEvent(event) {
    const payload = event.payload || {};
    const localPayloadKind = Number(event.payloadKind ?? event.payload_kind ?? 0);
    const payloadKind = toProtoPayloadKind(localPayloadKind);

    // Compute legacy hash for gRPC compatibility (canonical JSON, no domain prefix)
    // The gRPC server validates using canonical_json_hash, not VES payload_plain_hash
    const payloadHash =
      event.payloadPlainHash !== undefined
        ? fromHexOrBuffer(event.payloadPlainHash)
        : computeLegacyPayloadHash(payload);

    const payloadCipherHash =
      event.payloadCipherHash !== undefined
        ? fromHexOrBuffer(event.payloadCipherHash)
        : Buffer.alloc(32);
    const agentSignature =
      event.agentSignature !== undefined
        ? fromHexOrBuffer(event.agentSignature)
        : event.signature !== undefined
          ? fromHexOrBuffer(event.signature)
          : Buffer.alloc(64);

    return {
      event_id: event.eventId || event.event_id || crypto.randomUUID(),
      command_id: event.commandId || event.command_id || '',
      tenant_id: this.config.tenantId,
      store_id: this.config.storeId,
      entity_type: event.entityType || event.entity_type,
      entity_id: event.entityId || event.entity_id,
      event_type: event.eventType || event.event_type,
      source_agent: this.config.agentId,
      ves_version: 1,
      payload_kind: payloadKind,
      payload: localPayloadKind === 1 ? Buffer.alloc(0) : Buffer.from(JSON.stringify(payload)),
      payload_encrypted: toProtoEncryptedPayload(event.payloadEncrypted),
      payload_plain_hash: payloadHash,
      payload_cipher_hash: payloadCipherHash,
      agent_key_id: event.agentKeyId || 0,
      agent_signature: agentSignature,
      agent_signature_scheme: event.agentSignatureScheme || 0,
      agent_signature_bundle: toProtoSignatureBundle(event.agentSignatureBundle),
      base_version: event.baseVersion || event.base_version || 0,
      created_at: {
        seconds: Math.floor(
          (event.createdAt instanceof Date
            ? event.createdAt.getTime()
            : event.createdAt || Date.now()) / 1000,
        ),
        nanos: 0,
      },
    };
  }

  // ===========================================================================
  // PULL OPERATIONS
  // ===========================================================================

  /**
   * Pull events from the sequencer.
   * @param {Object} options - Pull options
   * @param {number} [options.fromSequence=0] - Starting sequence
   * @param {number} [options.limit=100] - Max events to return
   * @param {string} [options.entityTypeFilter] - Filter by entity type
   * @param {string} [options.entityIdFilter] - Filter by entity ID
   * @returns {Promise<Object>} Pull response with events
   */
  async pullEvents(options = {}) {
    if (!this.connected || !this.client) {
      throw new Error('Not connected');
    }

    const request = {
      tenant_id: this.config.tenantId,
      store_id: this.config.storeId,
      from_sequence: options.fromSequence || this.lastSequence,
      limit: options.limit || 100,
      entity_type_filter: options.entityTypeFilter || '',
      entity_id_filter: options.entityIdFilter || '',
      event_type_filter: options.eventTypeFilter || [],
      agent_filter: options.agentFilter || [],
    };

    return new Promise((resolve, reject) => {
      this.client.pullEvents(request, this._createMetadata(), (err, response) => {
        if (err) {
          reject(err);
        } else {
          // Update last sequence
          const events = response.events.map((e) => this._fromProtoEvent(e));
          if (events.length > 0) {
            this.lastSequence = Math.max(
              this.lastSequence,
              ...events.map((e) => Number(e.sequenceNumber)),
            );
          }
          resolve({
            events,
            nextSequence: Number(response.next_sequence),
            hasMore: response.has_more,
            headSequence: Number(response.head_sequence),
          });
        }
      });
    });
  }

  /**
   * Convert proto event to local format
   */
  _fromProtoEvent(protoEvent) {
    const envelope = protoEvent.envelope || {};
    return {
      eventId: envelope.event_id,
      commandId: envelope.command_id || null,
      tenantId: envelope.tenant_id,
      storeId: envelope.store_id,
      entityType: envelope.entity_type,
      entityId: envelope.entity_id,
      eventType: envelope.event_type,
      sourceAgent: envelope.source_agent,
      vesVersion: envelope.ves_version,
      payload:
        fromProtoPayloadKind(envelope.payload_kind) === 1 || !envelope.payload?.length
          ? {}
          : JSON.parse(Buffer.from(envelope.payload).toString()),
      payloadKind: fromProtoPayloadKind(envelope.payload_kind),
      payloadEncrypted: fromProtoEncryptedPayload(envelope.payload_encrypted),
      payloadHash: envelope.payload_plain_hash,
      payloadPlainHash: envelope.payload_plain_hash,
      payloadCipherHash: envelope.payload_cipher_hash,
      agentKeyId: envelope.agent_key_id,
      agentSignature: envelope.agent_signature,
      agentSignatureScheme: envelope.agent_signature_scheme || 0,
      agentSignatureBundle: fromProtoSignatureBundle(envelope.agent_signature_bundle),
      baseVersion: envelope.base_version || null,
      createdAt: envelope.created_at ? new Date(Number(envelope.created_at.seconds) * 1000) : null,
      sequenceNumber: Number(protoEvent.sequence_number),
      sequencedAt: protoEvent.sequenced_at
        ? new Date(Number(protoEvent.sequenced_at.seconds) * 1000)
        : null,
      receiptHash: protoEvent.receipt_hash || null,
      receiptSignatureScheme: protoEvent.receipt_signature_scheme || 0,
      receiptSignatureBundle: fromProtoSignatureBundle(protoEvent.receipt_signature_bundle),
    };
  }

  // ===========================================================================
  // SYNC STATE
  // ===========================================================================

  /**
   * Get current sync state.
   * @returns {Promise<Object>} Sync state
   */
  async getSyncState() {
    if (!this.connected || !this.client) {
      throw new Error('Not connected');
    }

    const request = {
      tenant_id: this.config.tenantId,
      store_id: this.config.storeId,
    };

    return new Promise((resolve, reject) => {
      this.client.getSyncState(request, this._createMetadata(), (err, response) => {
        if (err) {
          reject(err);
        } else {
          resolve({
            tenantId: response.tenant_id,
            storeId: response.store_id,
            headSequence: Number(response.head_sequence),
            stateRoot: response.state_root,
            latestCommitment: response.latest_commitment,
            timestamp: response.timestamp
              ? new Date(Number(response.timestamp.seconds) * 1000)
              : null,
          });
        }
      });
    });
  }

  /**
   * Get health status.
   * @returns {Promise<Object>} Health response
   */
  async getHealth() {
    if (!this.connected || !this.client) {
      throw new Error('Not connected');
    }

    return new Promise((resolve, reject) => {
      this.client.getHealth({}, this._createMetadata(), (err, response) => {
        if (err) {
          reject(err);
        } else {
          resolve({
            healthy: response.healthy,
            version: response.version,
            timestamp: response.timestamp
              ? new Date(Number(response.timestamp.seconds) * 1000)
              : null,
          });
        }
      });
    });
  }

  // ===========================================================================
  // STREAMING
  // ===========================================================================

  /**
   * Start streaming events.
   * @param {Object} options - Stream options
   * @param {number} [options.fromSequence=0] - Starting sequence
   * @param {boolean} [options.includeHistory=true] - Include historical events
   * @param {string[]} [options.entityTypeFilter] - Filter by entity types
   * @param {string[]} [options.eventTypeFilter] - Filter by event types
   * @returns {Object} Stream object
   */
  startEventStream(options = {}) {
    if (!this.connected || !this.client) {
      throw new Error('Not connected');
    }

    const request = {
      tenant_id: this.config.tenantId,
      store_id: this.config.storeId,
      from_sequence: options.fromSequence || this.lastSequence,
      include_history: options.includeHistory !== false,
      entity_type_filter: options.entityTypeFilter || [],
      event_type_filter: options.eventTypeFilter || [],
      agent_filter: options.agentFilter || [],
      heartbeat_interval_ms: this.config.keepaliveMs,
    };

    this.eventStream = this.client.streamEvents(request, this._createMetadata());

    this.eventStream.on('data', (event) => {
      const parsed = this._fromProtoEvent(event);
      this.lastSequence = Math.max(this.lastSequence, parsed.sequenceNumber);
      this.emit('event', parsed);
    });

    this.eventStream.on('error', (err) => {
      this.emit('error', err);
      if (err.code !== grpc.status.CANCELLED) {
        this._reconnect();
      }
    });

    this.eventStream.on('end', () => {
      this.emit('stream-ended');
    });

    return this.eventStream;
  }

  /**
   * Start bidirectional sync stream.
   * @returns {Object} Duplex stream object
   */
  startSyncStream() {
    if (!this.connected || !this.client) {
      throw new Error('Not connected');
    }

    this.syncStream = this.client.syncStream(this._createMetadata());

    this.syncStream.on('data', (message) => {
      // Handle push acknowledgment (proto field: push_ack)
      const pushAck =
        message.pushAck || message.push_ack || message.pushResponse || message.push_response;
      if (pushAck) {
        this.emit('push-ack', pushAck);
      } else if (message.pullResponse || message.pull_response) {
        const pullResp = message.pullResponse || message.pull_response;
        const events = pullResp.events.map((e) => this._fromProtoEvent(e));
        events.forEach((e) => this.emit('event', e));
      } else if (message.event) {
        const parsed = this._fromProtoEvent(message.event);
        this.lastSequence = Math.max(this.lastSequence, parsed.sequenceNumber);
        this.emit('event', parsed);
      } else if (message.syncState || message.sync_state) {
        this.emit('sync-state', message.syncState || message.sync_state);
      } else if (message.serverHeartbeat || message.server_heartbeat) {
        // Heartbeat acknowledged
      }
    });

    this.syncStream.on('error', (err) => {
      this.emit('error', err);
      if (err.code !== grpc.status.CANCELLED) {
        this._reconnect();
      }
    });

    this.syncStream.on('end', () => {
      this.emit('stream-ended');
    });

    this._startHeartbeat();

    return this.syncStream;
  }

  /**
   * Push events via sync stream.
   * @param {Array} events - Events to push
   */
  pushEventsViaStream(events) {
    if (!this.syncStream) {
      throw new Error('Sync stream not started');
    }

    this.syncStream.write({
      push: {
        agent_id: this.config.agentId,
        tenant_id: this.config.tenantId,
        store_id: this.config.storeId,
        events: events.map((e) => this._toProtoEvent(e)),
        request_id: crypto.randomUUID(),
      },
    });
  }

  /**
   * Pull events via sync stream.
   * @param {Object} options - Pull options
   */
  pullEventsViaStream(options = {}) {
    if (!this.syncStream) {
      throw new Error('Sync stream not started');
    }

    this.syncStream.write({
      pull: {
        tenant_id: this.config.tenantId,
        store_id: this.config.storeId,
        from_sequence: options.fromSequence || this.lastSequence,
        limit: options.limit || 100,
        entity_type_filter: options.entityTypeFilter || '',
        entity_id_filter: options.entityIdFilter || '',
        event_type_filter: options.eventTypeFilter || [],
        agent_filter: options.agentFilter || [],
      },
    });
  }

  /**
   * Acknowledge received events.
   * @param {number[]} sequenceNumbers - Sequence numbers to ack
   */
  ackEvents(sequenceNumbers) {
    if (!this.syncStream) {
      throw new Error('Sync stream not started');
    }

    this.syncStream.write({
      ack: {
        sequence_numbers: sequenceNumbers,
        agent_head_sequence: Math.max(...sequenceNumbers),
      },
    });
  }

  // ===========================================================================
  // ENTITY SUBSCRIPTIONS
  // ===========================================================================

  /**
   * Subscribe to updates for a specific entity.
   * @param {string} entityType - Entity type
   * @param {string} entityId - Entity ID
   * @param {boolean} [includeHistory=true] - Include historical events
   * @returns {Object} Stream object
   */
  subscribeEntity(entityType, entityId, includeHistory = true) {
    if (!this.connected || !this.client) {
      throw new Error('Not connected');
    }

    const request = {
      tenant_id: this.config.tenantId,
      store_id: this.config.storeId,
      entity_type: entityType,
      entity_id: entityId,
      include_history: includeHistory,
    };

    const stream = this.client.subscribeEntity(request, this._createMetadata());

    stream.on('data', (event) => {
      const parsed = this._fromProtoEvent(event);
      this.emit('entity-event', parsed);
    });

    stream.on('error', (err) => {
      this.emit('error', err);
    });

    return stream;
  }

  // ===========================================================================
  // PROOFS & COMMITMENTS
  // ===========================================================================

  /**
   * Get inclusion proof for an event.
   * @param {Object} options - Query options
   * @param {string} [options.eventId] - Event ID
   * @param {number} [options.sequenceNumber] - Sequence number
   * @returns {Promise<Object>} Inclusion proof
   */
  async getInclusionProof(options) {
    if (!this.connected || !this.client) {
      throw new Error('Not connected');
    }

    const request = {
      tenant_id: this.config.tenantId,
      store_id: this.config.storeId,
      expected_root: options.expectedRoot || Buffer.alloc(0),
    };

    if (options.eventId) {
      request.event_id = options.eventId;
    } else if (options.sequenceNumber !== undefined) {
      request.sequence_number = options.sequenceNumber;
    } else {
      throw new Error('Either eventId or sequenceNumber required');
    }

    return new Promise((resolve, reject) => {
      this.client.getInclusionProof(request, this._createMetadata(), (err, response) => {
        if (err) {
          reject(err);
        } else {
          resolve({
            included: response.included,
            proof: response.proof,
            event: response.event ? this._fromProtoEvent(response.event) : null,
          });
        }
      });
    });
  }

  /**
   * Get batch commitment.
   * @param {Object} options - Query options
   * @param {string} [options.batchId] - Batch ID
   * @param {number} [options.sequenceNumber] - Sequence number
   * @returns {Promise<Object>} Batch commitment
   */
  async getCommitment(options) {
    if (!this.connected || !this.client) {
      throw new Error('Not connected');
    }

    const request = {};

    if (options.batchId) {
      request.batch_id = options.batchId;
    } else if (options.sequenceNumber !== undefined) {
      request.sequence_number = options.sequenceNumber;
    } else {
      throw new Error('Either batchId or sequenceNumber required');
    }

    return new Promise((resolve, reject) => {
      this.client.getCommitment(request, this._createMetadata(), (err, response) => {
        if (err) {
          reject(err);
        } else {
          resolve({
            batchId: response.batch_id,
            merkleRoot: response.merkle_root,
            startSequence: Number(response.start_sequence),
            endSequence: Number(response.end_sequence),
            eventCount: response.event_count,
            committedAt: response.committed_at
              ? new Date(Number(response.committed_at.seconds) * 1000)
              : null,
            previousRoot: response.previous_root,
          });
        }
      });
    });
  }

  /**
   * Get entity event history.
   * @param {string} entityType - Entity type
   * @param {string} entityId - Entity ID
   * @param {Object} [options] - Query options
   * @returns {Promise<Object>} Entity history
   */
  async getEntityHistory(entityType, entityId, options = {}) {
    if (!this.connected || !this.client) {
      throw new Error('Not connected');
    }

    const request = {
      tenant_id: this.config.tenantId,
      store_id: this.config.storeId,
      entity_type: entityType,
      entity_id: entityId,
      from_version: options.fromVersion || 0,
      to_version: options.toVersion || 0,
      limit: options.limit || 100,
    };

    return new Promise((resolve, reject) => {
      this.client.getEntityHistory(request, this._createMetadata(), (err, response) => {
        if (err) {
          reject(err);
        } else {
          resolve({
            events: response.events.map((e) => this._fromProtoEvent(e)),
            currentVersion: Number(response.current_version),
          });
        }
      });
    });
  }

  // ===========================================================================
  // KEY MANAGEMENT
  // ===========================================================================

  /**
   * Register an agent key.
   * @param {Object} keyInfo - Key information
   * @returns {Promise<Object>} Registration response
   */
  async registerAgentKey(keyInfo) {
    if (!this.connected || !this.keyClient) {
      throw new Error('Not connected');
    }

    assertKeyRegistrationMatchesSecurityProfile(keyInfo, this.config.securityProfile);

    const request = {
      tenant_id: this.config.tenantId,
      agent_id: keyInfo.agentId || this.config.agentId,
      key_id: keyInfo.keyId,
      key_type: keyInfo.keyType, // 1 = SIGNING, 2 = ENCRYPTION
      key_algorithm: keyInfo.keyAlgorithm || 0,
      public_key: keyInfo.publicKey,
      public_key_bundle: toProtoPublicKeyBundle(keyInfo.publicKeyBundle),
      proof_of_possession: keyInfo.proofOfPossession || Buffer.alloc(0),
      proof_of_possession_bundle: toProtoProofOfPossessionBundle(
        keyInfo.proofOfPossessionBundle,
      ),
    };

    if (keyInfo.validFrom) {
      request.valid_from = {
        seconds: Math.floor(keyInfo.validFrom.getTime() / 1000),
        nanos: 0,
      };
    }

    if (keyInfo.validTo) {
      request.valid_to = {
        seconds: Math.floor(keyInfo.validTo.getTime() / 1000),
        nanos: 0,
      };
    }

    return new Promise((resolve, reject) => {
      this.keyClient.registerAgentKey(request, this._createMetadata(), (err, response) => {
        if (err) {
          reject(err);
        } else {
          resolve({
            success: response.success,
            message: response.message,
            registeredAt: response.registered_at
              ? new Date(Number(response.registered_at.seconds) * 1000)
              : null,
          });
        }
      });
    });
  }

  /**
   * Get agent keys.
   * @param {string} [agentId] - Agent ID (defaults to current agent)
   * @param {Object} [options] - Query options
   * @returns {Promise<Object>} Agent keys
   */
  async getAgentKeys(agentId, options = {}) {
    if (!this.connected || !this.keyClient) {
      throw new Error('Not connected');
    }

    const request = {
      tenant_id: this.config.tenantId,
      agent_id: agentId || this.config.agentId,
      key_type_filter: options.keyType || 0, // 0 = all
      include_revoked: options.includeRevoked || false,
    };

    return new Promise((resolve, reject) => {
      this.keyClient.getAgentKeys(request, this._createMetadata(), (err, response) => {
        if (err) {
          reject(err);
        } else {
          resolve({
            keys: response.keys.map((k) => ({
              keyId: k.key_id,
              keyType: k.key_type,
              keyAlgorithm: k.key_algorithm,
              publicKey: k.public_key,
              publicKeyBundle: fromProtoPublicKeyBundle(k.public_key_bundle),
              status: k.status,
              createdAt: k.created_at ? new Date(Number(k.created_at.seconds) * 1000) : null,
              validFrom: k.valid_from ? new Date(Number(k.valid_from.seconds) * 1000) : null,
              validTo: k.valid_to ? new Date(Number(k.valid_to.seconds) * 1000) : null,
              revokedAt: k.revoked_at ? new Date(Number(k.revoked_at.seconds) * 1000) : null,
            })),
          });
        }
      });
    });
  }

  /**
   * Revoke an agent key.
   * @param {number} keyId - Key ID to revoke
   * @param {string} reason - Revocation reason
   * @param {Buffer} [authSignature] - Authorization signature
   * @returns {Promise<Object>} Revocation response
   */
  async revokeAgentKey(keyId, reason, authSignature) {
    if (!this.connected || !this.keyClient) {
      throw new Error('Not connected');
    }

    const request = {
      tenant_id: this.config.tenantId,
      agent_id: this.config.agentId,
      key_id: keyId,
      reason: reason,
      authorization_signature: authSignature || Buffer.alloc(0),
    };

    return new Promise((resolve, reject) => {
      this.keyClient.revokeAgentKey(request, this._createMetadata(), (err, response) => {
        if (err) {
          reject(err);
        } else {
          resolve({
            success: response.success,
            revokedAt: response.revoked_at
              ? new Date(Number(response.revoked_at.seconds) * 1000)
              : null,
          });
        }
      });
    });
  }
  /**
   * Verify a receipt signature against a known sequencer public key.
   *
   * Supports legacy (Ed25519), hybrid (Ed25519 + ML-DSA-65), and
   * PQC-strict (ML-DSA-65) receipt signatures per VES-RECEIPT-2.
   *
   * @param {Object} receipt - Receipt or sequenced event with receipt fields.
   * @param {Buffer|string|Object} sequencerPublicKey - Sequencer public key or bundle.
   * @returns {boolean}
   */
  verifyReceiptSignature(receipt, sequencerPublicKey) {
    const receiptHash = typeof receipt.receiptHash === 'string'
      ? hexToBuffer(receipt.receiptHash)
      : receipt.receiptHash;
    if (!receiptHash || receiptHash.length !== 32) {
      return false;
    }

    const scheme = Number(receipt.receiptSignatureScheme || 0);
    const bundle = receipt.receiptSignatureBundle || null;

    try {
      assertReceiptMatchesSecurityProfile(
        { signatureScheme: scheme, signatureBundle: bundle },
        this.config.securityProfile,
      );
    } catch {
      // Profile mismatch — still attempt verification
    }

    const pk = typeof sequencerPublicKey === 'object' && !Buffer.isBuffer(sequencerPublicKey)
      ? sequencerPublicKey
      : { ed25519PublicKey: sequencerPublicKey };

    if (scheme === SIGNATURE_SCHEME_ML_DSA_65 && bundle?.mlDsa65Signature && pk?.mlDsa65PublicKey) {
      try {
        const sig = typeof bundle.mlDsa65Signature === 'string'
          ? hexToBuffer(bundle.mlDsa65Signature) : bundle.mlDsa65Signature;
        return verifyEventSignatureStrict(receiptHash, sig, pk);
      } catch {
        return false;
      }
    }

    if (scheme === SIGNATURE_SCHEME_ED25519_ML_DSA_65 && bundle && pk?.ed25519PublicKey && pk?.mlDsa65PublicKey) {
      try {
        return verifyEventSignatureHybrid(receiptHash, bundle, pk);
      } catch {
        return false;
      }
    }

    const sigHex = bundle?.ed25519Signature;
    const edPk = pk?.ed25519PublicKey ?? sequencerPublicKey;
    if (!sigHex || !edPk) return false;
    return verifyEventSignature(
      receiptHash,
      typeof sigHex === 'string' ? hexToBuffer(sigHex) : sigHex,
      typeof edPk === 'string' ? hexToBuffer(edPk) : edPk,
    );
  }
}

// =============================================================================
// EXPORTS
// =============================================================================

export default GrpcSequencerClient;

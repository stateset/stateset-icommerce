/**
 * SQLite Outbox for Local Event Capture (VES v1.0)
 *
 * Implements the outbox pattern for CLI agents:
 * 1. Apply mutations locally in a single atomic transaction
 * 2. Append events to the outbox with VES v1.0 signing
 * 3. Later push events to the remote sequencer
 *
 * VES v1.0 Features:
 * - Ed25519 agent signatures
 * - VES-ENC-1 encrypted payloads (optional)
 * - Dual-hash binding (payload_plain_hash + payload_cipher_hash)
 */

import crypto from 'crypto';
import {
  computePayloadPlainHash,
  computeEventSigningHash,
  signEventHash,
  signEventHashHybrid,
  signEventHashStrict,
  encryptPayload,
  encryptPayloadHybrid,
  encryptPayloadStrict,
  bufferToHex,
  hexToBuffer,
  ZERO_HASH,
} from './crypto.js';
import { getKeyManager } from './keys.js';
import {
  SECURITY_PROFILE_HYBRID,
  SECURITY_PROFILE_LEGACY,
  SECURITY_PROFILE_PQC_STRICT,
  SIGNATURE_SCHEME_ED25519_ML_DSA_65,
  SIGNATURE_SCHEME_ML_DSA_65,
  KEY_WRAP_SCHEME_ML_KEM_768,
  resolveSecurityProfile,
  profileMetricLabel,
} from './pqc.js';
import { getRotationPolicyManager } from './rotation-policy.js';

/**
 * @typedef {Object} OutboxEvent
 * @property {number} localSeq - Local sequence number
 * @property {string} eventId - UUID of the event
 * @property {string|null} commandId - Idempotency key
 * @property {string} tenantId - Tenant UUID
 * @property {string} storeId - Store UUID
 * @property {string} entityType - Entity type (order, customer, etc.)
 * @property {string} entityId - Entity identifier
 * @property {string} eventType - Event type (order.created, etc.)
 * @property {Object} payload - Event payload (plaintext)
 * @property {number} vesVersion - VES protocol version (1)
 * @property {number} payloadKind - 0=plaintext, 1=encrypted
 * @property {Object|null} payloadEncrypted - Encrypted payload structure (VES-ENC-1)
 * @property {string} payloadPlainHash - SHA-256 of plaintext payload (hex)
 * @property {string} payloadCipherHash - SHA-256 of ciphertext or zero hash (hex)
 * @property {number} agentKeyId - Key ID used to sign
 * @property {string} agentSignature - Ed25519 signature (hex)
 * @property {number} [agentSignatureScheme] - PQ or hybrid signature scheme
 * @property {Object|null} [agentSignatureBundle] - Structured signature bundle
 * @property {number|null} baseVersion - Optimistic concurrency version
 * @property {string} sourceAgent - Agent UUID that created the event
 * @property {Date} createdAt - When event was created
 * @property {'pending'|'synced'|'failed'|'rejected'} syncStatus - Sync state
 * @property {number|null} remoteSequence - Sequence from sequencer
 * @property {Date|null} syncedAt - When synced
 * @property {string|null} rejectionReason - Why rejected
 * @property {number} retryCount - Number of retry attempts
 * @property {string|null} lastError - Last error message
 */

/**
 * @typedef {Object} OutboxStats
 * @property {number} total - Total events
 * @property {number} pending - Pending sync
 * @property {number} synced - Successfully synced
 * @property {number} failed - Failed attempts
 * @property {number} rejected - Rejected by sequencer
 * @property {Date|null} oldestPending - Oldest pending event
 * @property {Date|null} lastSynced - Most recent sync
 */

/**
 * @typedef {Object} SyncState
 * @property {string} agentId - This agent's UUID
 * @property {string|null} tenantId - Tenant UUID
 * @property {string|null} storeId - Store UUID
 * @property {number} lastPushedSequence - Last sequence pushed
 * @property {number} lastPulledSequence - Last sequence pulled
 * @property {number} headSequence - Known remote head
 * @property {Date} lastSyncAt - Last sync timestamp
 */

const OUTBOX_SCHEMA = `
-- VES v1.0 Outbox table for pending events
CREATE TABLE IF NOT EXISTS _ves_outbox (
    local_seq INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT UNIQUE NOT NULL,
    command_id TEXT,
    tenant_id TEXT NOT NULL,
    store_id TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    event_type TEXT NOT NULL,

    -- VES v1.0 payload fields
    ves_version INTEGER NOT NULL DEFAULT 1,
    payload TEXT NOT NULL,
    payload_kind INTEGER NOT NULL DEFAULT 0,
    payload_encrypted TEXT,
    payload_plain_hash TEXT NOT NULL,
    payload_cipher_hash TEXT NOT NULL,

    -- VES v1.0 signature fields
    agent_key_id INTEGER NOT NULL,
    agent_signature TEXT NOT NULL,
    agent_signature_scheme INTEGER NOT NULL DEFAULT 0,
    agent_signature_bundle TEXT,

    -- Metadata
    base_version INTEGER,
    source_agent TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),

    -- Sync tracking
    sync_status TEXT DEFAULT 'pending',
    remote_sequence INTEGER,
    synced_at TEXT,
    rejection_reason TEXT,
    retry_count INTEGER DEFAULT 0,
    last_error TEXT,

    CHECK(json_valid(payload)),
    CHECK(sync_status IN ('pending', 'synced', 'failed', 'rejected')),
    CHECK(payload_kind IN (0, 1)),
    CHECK(ves_version >= 1),
    CHECK((payload_kind = 0 AND payload_encrypted IS NULL) OR (payload_kind = 1 AND payload_encrypted IS NOT NULL))
);

-- Index for finding pending events
CREATE INDEX IF NOT EXISTS idx_ves_outbox_pending
    ON _ves_outbox (sync_status) WHERE sync_status = 'pending';

-- Index for entity history
CREATE INDEX IF NOT EXISTS idx_ves_outbox_entity
    ON _ves_outbox (tenant_id, store_id, entity_type, entity_id);

-- Index for command deduplication
CREATE INDEX IF NOT EXISTS idx_ves_outbox_command
    ON _ves_outbox (command_id) WHERE command_id IS NOT NULL;

-- VES Sync State table
CREATE TABLE IF NOT EXISTS _ves_sync_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- VES Entity Versions for optimistic concurrency
CREATE TABLE IF NOT EXISTS _ves_entity_versions (
    tenant_id TEXT NOT NULL,
    store_id TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (tenant_id, store_id, entity_type, entity_id)
);

-- VES v1.0 Pulled Events cache (events from remote)
CREATE TABLE IF NOT EXISTS _ves_pulled_events (
    sequence_number INTEGER PRIMARY KEY,
    event_id TEXT UNIQUE NOT NULL,
    command_id TEXT,
    tenant_id TEXT NOT NULL,
    store_id TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    event_type TEXT NOT NULL,

    -- VES v1.0 payload fields
    ves_version INTEGER NOT NULL DEFAULT 1,
    payload TEXT NOT NULL,
    payload_kind INTEGER NOT NULL DEFAULT 0,
    payload_encrypted TEXT,
    payload_plain_hash TEXT NOT NULL,
    payload_cipher_hash TEXT NOT NULL,

    -- VES v1.0 signature fields
    agent_key_id INTEGER NOT NULL,
    agent_signature TEXT NOT NULL,
    agent_signature_scheme INTEGER NOT NULL DEFAULT 0,
    agent_signature_bundle TEXT,

    -- Metadata
    base_version INTEGER,
    created_at TEXT NOT NULL,
    sequenced_at TEXT NOT NULL,
    pulled_at TEXT NOT NULL DEFAULT (datetime('now')),
    source_agent TEXT NOT NULL,

    CHECK(json_valid(payload)),
    CHECK(payload_kind IN (0, 1))
);

-- Initialize default sync state
INSERT OR IGNORE INTO _ves_sync_state (key, value, updated_at) VALUES
    ('last_pushed_sequence', '0', datetime('now')),
    ('last_pulled_sequence', '0', datetime('now')),
    ('head_sequence', '0', datetime('now'));
`;

/**
 * SQLite Outbox for local event capture and sync tracking (VES v1.0)
 */
export class Outbox {
  /**
   * @param {import('better-sqlite3').Database} db - SQLite database instance
   * @param {Object} [options] - Configuration options
   * @param {string} [options.configDir='.stateset'] - Config directory for keys
   * @param {import('./keys.js').AgentKeyManager} [options.keyManager] - Key manager instance
   */
  constructor(db, options = {}) {
    this.db = db;
    this.configDir = options.configDir || '.stateset';
    this.securityProfile = resolveSecurityProfile(
      options.securityProfile ?? SECURITY_PROFILE_LEGACY,
    );
    this.keyManager =
      options.keyManager ||
      getKeyManager(this.configDir, { securityProfile: this.securityProfile });
    this._initialized = false;
    /** @type {{ legacy: number, hybrid: number, 'pqc-strict': number }} */
    this._signatureProfileCounts = { legacy: 0, hybrid: 0, 'pqc-strict': 0 };
    /** @type {{ legacy: number, hybrid: number, 'pqc-strict': number }} */
    this._encryptionProfileCounts = { legacy: 0, hybrid: 0, 'pqc-strict': 0 };
  }

  /**
   * Return a snapshot of PQC profile usage counters.
   * @returns {{ signatures: { legacy: number, hybrid: number, 'pqc-strict': number }, encryptions: { legacy: number, hybrid: number, 'pqc-strict': number } }}
   */
  get pqcMetrics() {
    return {
      signatures: { ...this._signatureProfileCounts },
      encryptions: { ...this._encryptionProfileCounts },
    };
  }

  /**
   * Record key usage and check if rotation is due (non-blocking).
   * @private
   * @param {string} agentId
   * @param {'signing'|'encryption'} keyType
   * @param {number} keyId
   * @param {Object} keyData
   */
  async _recordKeyUsageAndCheckRotation(agentId, keyType, keyId, keyData) {
    try {
      const pm = getRotationPolicyManager(this.configDir);
      await pm.recordUsage(agentId, keyType, keyId);
      const { shouldRotate, reason } = await pm.shouldRotate(agentId, keyId, keyType, keyData);
      if (shouldRotate) {
        this._lastRotationWarning = { agentId, keyType, keyId, reason, at: new Date().toISOString() };
      }
    } catch {
      // Rotation tracking is best-effort; don't block event signing
    }
  }

  /**
   * Return the last rotation warning, if any.
   * @returns {{ agentId: string, keyType: string, keyId: number, reason: string, at: string } | null}
   */
  get rotationWarning() {
    return this._lastRotationWarning ?? null;
  }

  _ensureColumn(tableName, columnName, columnDefinition) {
    const columns = this.db.prepare(`PRAGMA table_info(${tableName})`).all();
    if (columns.some((column) => column.name === columnName)) {
      return;
    }
    this.db.exec(`ALTER TABLE ${tableName} ADD COLUMN ${columnDefinition}`);
  }

  /**
   * Initialize the outbox schema
   */
  initialize() {
    if (this._initialized) return;

    // Execute schema statements one by one
    const statements = OUTBOX_SCHEMA.split(';')
      .map((s) => s.trim())
      .filter((s) => s.length > 0);

    for (const stmt of statements) {
      this.db.exec(stmt);
    }

    this._ensureColumn(
      '_ves_outbox',
      'agent_signature_scheme',
      'agent_signature_scheme INTEGER NOT NULL DEFAULT 0',
    );
    this._ensureColumn(
      '_ves_outbox',
      'agent_signature_bundle',
      'agent_signature_bundle TEXT',
    );
    this._ensureColumn(
      '_ves_pulled_events',
      'agent_signature_scheme',
      'agent_signature_scheme INTEGER NOT NULL DEFAULT 0',
    );
    this._ensureColumn(
      '_ves_pulled_events',
      'agent_signature_bundle',
      'agent_signature_bundle TEXT',
    );

    this._initialized = true;
  }

  /**
   * Compute VES v1.0 payload plain hash
   * @param {Object} payload
   * @returns {Buffer} 32-byte hash
   */
  computePayloadPlainHashBuffer(payload) {
    return computePayloadPlainHash(payload);
  }

  /**
   * Compute SHA-256 hash of payload (legacy, returns hex)
   * @param {Object} payload
   * @returns {string} Hex-encoded hash
   * @deprecated Use computePayloadPlainHashBuffer for VES v1.0
   */
  computePayloadHash(payload) {
    return bufferToHex(computePayloadPlainHash(payload));
  }

  /**
   * Generate a new UUID v4
   * @returns {string}
   */
  generateEventId() {
    return crypto.randomUUID();
  }

  /**
   * Normalize a hybrid recipient key bundle from CLI/runtime input.
   * @private
   * @param {Object | null | undefined} bundle
   * @returns {{x25519PublicKey: Buffer, mlKem768PublicKey: Buffer} | null}
   */
  _normalizeRecipientPublicKeyBundle(bundle) {
    if (!bundle || typeof bundle !== 'object') {
      return null;
    }

    const x25519PublicKey = bundle.x25519PublicKey ?? bundle.x25519_public_key ?? null;
    const mlKem768PublicKey = bundle.mlKem768PublicKey ?? bundle.ml_kem_768_public_key ?? null;
    if (!x25519PublicKey || !mlKem768PublicKey) {
      return null;
    }

    const toBinary = (value) => {
      if (Buffer.isBuffer(value)) {
        return value;
      }
      if (value instanceof Uint8Array) {
        return Buffer.from(value);
      }
      if (typeof value === 'string' && value.startsWith('0x')) {
        return hexToBuffer(value);
      }
      return Buffer.from(value);
    };

    return {
      x25519PublicKey: toBinary(x25519PublicKey),
      mlKem768PublicKey: toBinary(mlKem768PublicKey),
    };
  }

  /**
   * Encrypt a payload for the active security profile.
   * @private
   * @param {Object} payload
   * @param {Object} aadParams
   * @param {Object} signingKey
   * @param {Object} options
   * @returns {{
   *   payloadKind: number,
   *   payloadEncrypted: Object | null,
   *   payloadPlainHash: Buffer,
   *   payloadCipherHash: Buffer,
   * }}
   */
  _encryptPayloadForProfile(payload, aadParams, signingKey, options) {
    let payloadKind = 0;
    let payloadEncrypted = null;
    let payloadPlainHash = aadParams.payloadPlainHash;
    let payloadCipherHash = ZERO_HASH;

    if (!options.encrypt) {
      return { payloadKind, payloadEncrypted, payloadPlainHash, payloadCipherHash };
    }

    const encLabel = profileMetricLabel(this.securityProfile);
    this._encryptionProfileCounts[encLabel] = (this._encryptionProfileCounts[encLabel] ?? 0) + 1;

    const recipientKid = Number(options.recipientKeyId ?? signingKey.keyId);

    if (this.securityProfile === SECURITY_PROFILE_HYBRID) {
      const recipientBundle =
        this._normalizeRecipientPublicKeyBundle(options.recipientPublicKeyBundle) ||
        this._normalizeRecipientPublicKeyBundle(options.recipientPublicKey);
      if (!recipientBundle) {
        throw new Error(
          'Hybrid payload encryption requires recipientPublicKeyBundle with x25519PublicKey and mlKem768PublicKey',
        );
      }

      const encrypted = encryptPayloadHybrid(payload, aadParams, [
        {
          kid: recipientKid,
          x25519PublicKey: recipientBundle.x25519PublicKey,
          mlKem768PublicKey: recipientBundle.mlKem768PublicKey,
        },
      ]);

      payloadKind = 1;
      payloadEncrypted = encrypted.payloadEncrypted;
      payloadPlainHash = encrypted.payloadPlainHash;
      payloadCipherHash = encrypted.payloadCipherHash;
      return { payloadKind, payloadEncrypted, payloadPlainHash, payloadCipherHash };
    }

    if (this.securityProfile === SECURITY_PROFILE_PQC_STRICT) {
      const recipientBundle =
        this._normalizeRecipientPublicKeyBundle(options.recipientPublicKeyBundle) ||
        this._normalizeRecipientPublicKeyBundle(options.recipientPublicKey);
      if (!recipientBundle?.mlKem768PublicKey) {
        throw new Error(
          'pqc-strict payload encryption requires recipientPublicKeyBundle with mlKem768PublicKey',
        );
      }

      const encrypted = encryptPayloadStrict(payload, aadParams, [
        {
          kid: recipientKid,
          mlKem768PublicKey: recipientBundle.mlKem768PublicKey,
        },
      ]);

      payloadKind = 1;
      payloadEncrypted = encrypted.payloadEncrypted;
      payloadPlainHash = encrypted.payloadPlainHash;
      payloadCipherHash = encrypted.payloadCipherHash;
      return { payloadKind, payloadEncrypted, payloadPlainHash, payloadCipherHash };
    }

    if (!options.recipientPublicKey) {
      throw new Error('Payload encryption requires recipientPublicKey');
    }

    const recipientPublicKey =
      typeof options.recipientPublicKey === 'string' &&
      options.recipientPublicKey.startsWith('0x')
        ? hexToBuffer(options.recipientPublicKey)
        : options.recipientPublicKey;

    const encrypted = encryptPayload(payload, aadParams, [
      { kid: recipientKid, publicKey: recipientPublicKey },
    ]);

    payloadKind = 1;
    payloadEncrypted = encrypted.payloadEncrypted;
    payloadPlainHash = encrypted.payloadPlainHash;
    payloadCipherHash = encrypted.payloadCipherHash;
    return { payloadKind, payloadEncrypted, payloadPlainHash, payloadCipherHash };
  }

  /**
   * Sign an event hash for the active security profile.
   * @private
   * @param {Buffer} eventSigningHash
   * @param {Object} signingKey
   * @returns {{agentSignature: Buffer, agentSignatureScheme: number, agentSignatureBundle: Object | null}}
   */
  _signEventForProfile(eventSigningHash, signingKey) {
    const label = profileMetricLabel(this.securityProfile);
    this._signatureProfileCounts[label] = (this._signatureProfileCounts[label] ?? 0) + 1;

    if (this.securityProfile === SECURITY_PROFILE_HYBRID) {
      if (!signingKey.privateKeyBundle) {
        throw new Error('Hybrid signing requires a privateKeyBundle');
      }

      const bundle = signEventHashHybrid(eventSigningHash, signingKey.privateKeyBundle);
      return {
        agentSignature: bundle.ed25519Signature,
        agentSignatureScheme: SIGNATURE_SCHEME_ED25519_ML_DSA_65,
        agentSignatureBundle: {
          ed25519Signature: bufferToHex(bundle.ed25519Signature),
          mlDsa65Signature: bufferToHex(bundle.mlDsa65Signature),
        },
      };
    }

    if (this.securityProfile === SECURITY_PROFILE_PQC_STRICT) {
      if (!signingKey.privateKeyBundle?.mlDsa65Seed) {
        throw new Error('pqc-strict signing requires a privateKeyBundle with mlDsa65Seed');
      }

      const mlDsa65Signature = signEventHashStrict(eventSigningHash, signingKey.privateKeyBundle);
      return {
        agentSignature: mlDsa65Signature,
        agentSignatureScheme: SIGNATURE_SCHEME_ML_DSA_65,
        agentSignatureBundle: {
          mlDsa65Signature: bufferToHex(mlDsa65Signature),
        },
      };
    }

    return {
      agentSignature: signEventHash(eventSigningHash, signingKey.privateKey),
      agentSignatureScheme: 0,
      agentSignatureBundle: null,
    };
  }

  /**
   * Append an event to the outbox with VES v1.0 signing
   * @param {Object} event - Event to append
   * @param {string} event.tenantId
   * @param {string} event.storeId
   * @param {string} event.entityType
   * @param {string} event.entityId
   * @param {string} event.eventType
   * @param {Object} event.payload
   * @param {string} event.sourceAgent
   * @param {string} [event.eventId] - Optional, generated if not provided
   * @param {string} [event.commandId] - Optional idempotency key
   * @param {number} [event.baseVersion] - Optional version for OCC
   * @param {Object} [options] - VES v1.0 options
   * @param {boolean} [options.encrypt=false] - Whether to encrypt payload
   * @param {Buffer} [options.recipientPublicKey] - X25519 public key for encryption
   * @param {Object} [options.recipientPublicKeyBundle] - Hybrid recipient public key bundle
   * @param {number} [options.recipientKeyId] - Recipient key identifier
   * @param {number} [options.vesVersion=1] - VES protocol version
   * @returns {Promise<number>} Local sequence number
   */
  async append(event, options = {}) {
    this.initialize();

    const vesVersion = options.vesVersion || 1;
    const eventId = event.eventId || this.generateEventId();
    const createdAt = new Date().toISOString();

    // Get signing key for agent
    const signingKey = await this.keyManager.getCurrentSigningKey(event.sourceAgent);
    if (!signingKey) {
      throw new Error(
        `No signing key found for agent ${event.sourceAgent}. Generate keys first with 'stateset-sync keys:generate'`,
      );
    }

    // Compute plaintext hash
    const initialPayloadPlainHash = computePayloadPlainHash(event.payload);
    const payloadJson = JSON.stringify(event.payload);

    // Handle encryption if requested
    if (options.encrypt) {
      if (!(await this.keyManager.getCurrentEncryptionKey(event.sourceAgent))) {
        throw new Error(`No encryption key found for agent ${event.sourceAgent}`);
      }
    }

    const { payloadKind, payloadEncrypted, payloadPlainHash, payloadCipherHash } =
      this._encryptPayloadForProfile(
        event.payload,
        {
          vesVersion,
          eventId,
          tenantId: event.tenantId,
          storeId: event.storeId,
          entityType: event.entityType,
          entityId: event.entityId,
          eventType: event.eventType,
          sourceAgentId: event.sourceAgent,
          agentKeyId: signingKey.keyId,
          createdAt,
          payloadPlainHash: initialPayloadPlainHash,
        },
        signingKey,
        options,
      );

    // Compute event signing hash per VES v1.0
    const eventSigningHash = computeEventSigningHash({
      vesVersion,
      tenantId: event.tenantId,
      storeId: event.storeId,
      eventId,
      sourceAgentId: event.sourceAgent,
      agentKeyId: signingKey.keyId,
      entityType: event.entityType,
      entityId: event.entityId,
      eventType: event.eventType,
      createdAt,
      payloadKind,
      payloadPlainHash,
      payloadCipherHash,
    });

    // Debug: log signing hash
    if (process.env.VES_DEBUG) {
      console.debug('[VES] Event signing:', {
        eventId,
        tenantId: event.tenantId,
        storeId: event.storeId,
        sourceAgentId: event.sourceAgent,
        agentKeyId: signingKey.keyId,
        entityType: event.entityType,
        entityId: event.entityId,
        eventType: event.eventType,
        createdAt,
        payloadKind,
        payloadPlainHash: bufferToHex(payloadPlainHash),
        payloadCipherHash: bufferToHex(payloadCipherHash),
        signingHash: bufferToHex(eventSigningHash),
      });
    }

    const { agentSignature, agentSignatureScheme, agentSignatureBundle } =
      this._signEventForProfile(eventSigningHash, signingKey);

    // Record key usage for rotation tracking (non-blocking)
    this._recordKeyUsageAndCheckRotation(
      event.sourceAgent, 'signing', signingKey.keyId, signingKey,
    );

    const stmt = this.db.prepare(`
      INSERT INTO _ves_outbox (
        event_id, command_id, tenant_id, store_id,
        entity_type, entity_id, event_type,
        ves_version, payload, payload_kind, payload_encrypted,
        payload_plain_hash, payload_cipher_hash,
        agent_key_id, agent_signature, agent_signature_scheme, agent_signature_bundle,
        base_version, source_agent, created_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `);

    const result = stmt.run(
      eventId,
      event.commandId || null,
      event.tenantId,
      event.storeId,
      event.entityType,
      event.entityId,
      event.eventType,
      vesVersion,
      payloadJson,
      payloadKind,
      payloadEncrypted ? JSON.stringify(payloadEncrypted) : null,
      bufferToHex(payloadPlainHash),
      bufferToHex(payloadCipherHash),
      signingKey.keyId,
      bufferToHex(agentSignature),
      agentSignatureScheme,
      agentSignatureBundle ? JSON.stringify(agentSignatureBundle) : null,
      event.baseVersion || null,
      event.sourceAgent,
      createdAt,
    );

    return result.lastInsertRowid;
  }

  /**
   * Append multiple events atomically with VES v1.0 signing
   * @param {Array<Object>} events
   * @param {Object} [options] - VES v1.0 options (applied to all events)
   * @param {boolean} [options.encrypt=false] - Whether to encrypt payloads
   * @param {Buffer} [options.recipientPublicKey] - X25519 public key for encryption
   * @param {Object} [options.recipientPublicKeyBundle] - Hybrid recipient public key bundle
   * @param {number} [options.recipientKeyId] - Recipient key identifier
   * @param {number} [options.vesVersion=1] - VES protocol version
   * @returns {Promise<Array<number>>} Local sequence numbers
   */
  async appendBatch(events, options = {}) {
    this.initialize();

    const vesVersion = options.vesVersion || 1;
    const createdAt = new Date().toISOString();

    // Pre-fetch signing keys for all unique agents
    const agentIds = [...new Set(events.map((e) => e.sourceAgent))];
    const signingKeys = new Map();

    for (const agentId of agentIds) {
      const signingKey = await this.keyManager.getCurrentSigningKey(agentId);
      if (!signingKey) {
        throw new Error(`No signing key found for agent ${agentId}`);
      }
      signingKeys.set(agentId, signingKey);

      if (options.encrypt) {
        if (!(await this.keyManager.getCurrentEncryptionKey(agentId))) {
          throw new Error(`No encryption key found for agent ${agentId}`);
        }
      }
    }

    const stmt = this.db.prepare(`
      INSERT INTO _ves_outbox (
        event_id, command_id, tenant_id, store_id,
        entity_type, entity_id, event_type,
        ves_version, payload, payload_kind, payload_encrypted,
        payload_plain_hash, payload_cipher_hash,
        agent_key_id, agent_signature, agent_signature_scheme, agent_signature_bundle,
        base_version, source_agent, created_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `);

    const results = [];

    const transaction = this.db.transaction(() => {
      for (const event of events) {
        const eventId = event.eventId || this.generateEventId();
        const signingKey = signingKeys.get(event.sourceAgent);

        // Compute plaintext hash
        const initialPayloadPlainHash = computePayloadPlainHash(event.payload);
        const payloadJson = JSON.stringify(event.payload);

        const { payloadKind, payloadEncrypted, payloadPlainHash, payloadCipherHash } =
          this._encryptPayloadForProfile(
            event.payload,
            {
              vesVersion,
              eventId,
              tenantId: event.tenantId,
              storeId: event.storeId,
              entityType: event.entityType,
              entityId: event.entityId,
              eventType: event.eventType,
              sourceAgentId: event.sourceAgent,
              agentKeyId: signingKey.keyId,
              createdAt,
              payloadPlainHash: initialPayloadPlainHash,
            },
            signingKey,
            options,
          );

        // Compute event signing hash
        const eventSigningHash = computeEventSigningHash({
          vesVersion,
          tenantId: event.tenantId,
          storeId: event.storeId,
          eventId,
          sourceAgentId: event.sourceAgent,
          agentKeyId: signingKey.keyId,
          entityType: event.entityType,
          entityId: event.entityId,
          eventType: event.eventType,
          createdAt,
          payloadKind,
          payloadPlainHash,
          payloadCipherHash,
        });

        const { agentSignature, agentSignatureScheme, agentSignatureBundle } =
          this._signEventForProfile(eventSigningHash, signingKey);

        const result = stmt.run(
          eventId,
          event.commandId || null,
          event.tenantId,
          event.storeId,
          event.entityType,
          event.entityId,
          event.eventType,
          vesVersion,
          payloadJson,
          payloadKind,
          payloadEncrypted ? JSON.stringify(payloadEncrypted) : null,
          bufferToHex(payloadPlainHash),
          bufferToHex(payloadCipherHash),
          signingKey.keyId,
          bufferToHex(agentSignature),
          agentSignatureScheme,
          agentSignatureBundle ? JSON.stringify(agentSignatureBundle) : null,
          event.baseVersion || null,
          event.sourceAgent,
          createdAt,
        );

        results.push(result.lastInsertRowid);
      }
    });

    transaction();
    return results;
  }

  /**
   * Get pending events (not yet synced)
   * @param {number} [limit=100] - Maximum events to return
   * @returns {Array<OutboxEvent>}
   */
  getPending(limit = 100) {
    this.initialize();

    const stmt = this.db.prepare(`
      SELECT * FROM _ves_outbox
      WHERE sync_status = 'pending'
      ORDER BY local_seq ASC
      LIMIT ?
    `);

    const rows = stmt.all(limit);
    return rows.map(this._rowToEvent);
  }

  /**
   * Get event by event ID
   * @param {string} eventId
   * @returns {OutboxEvent|null}
   */
  getByEventId(eventId) {
    this.initialize();

    const stmt = this.db.prepare(`
      SELECT * FROM _ves_outbox WHERE event_id = ?
    `);

    const row = stmt.get(eventId);
    return row ? this._rowToEvent(row, 'outbox') : null;
  }

  /**
   * Get events by entity
   * @param {string} entityType
   * @param {string} entityId
   * @returns {Array<OutboxEvent>}
   */
  getByEntityId(entityType, entityId) {
    this.initialize();

    const stmt = this.db.prepare(`
      SELECT * FROM _ves_outbox
      WHERE entity_type = ? AND entity_id = ?
      ORDER BY local_seq ASC
    `);

    const rows = stmt.all(entityType, entityId);
    return rows.map((row) => this._rowToEvent(row, 'outbox'));
  }

  /**
   * Get pulled events from remote storage.
   * @param {number} [limit=1000] - Maximum events to return
   * @returns {Array<OutboxEvent>}
   */
  getPulledEvents(limit = 1000) {
    this.initialize();

    const stmt = this.db.prepare(`
      SELECT * FROM _ves_pulled_events
      ORDER BY sequence_number DESC
      LIMIT ?
    `);

    const rows = stmt.all(limit);
    return rows.map((row) => this._rowToEvent(row, 'pulled'));
  }

  /**
   * Get pulled events for a specific entity from remote storage.
   * @param {string} entityType
   * @param {string} entityId
   * @param {number} [limit=1000]
   * @returns {Array<OutboxEvent>}
   */
  getPulledEventsByEntity(entityType, entityId, limit = 1000) {
    this.initialize();

    const stmt = this.db.prepare(`
      SELECT * FROM _ves_pulled_events
      WHERE entity_type = ? AND entity_id = ?
      ORDER BY sequence_number ASC
      LIMIT ?
    `);

    const rows = stmt.all(entityType, entityId, limit);
    return rows.map((row) => this._rowToEvent(row, 'pulled'));
  }

  /**
   * Get a pulled event by event ID.
   * @param {string} eventId
   * @returns {OutboxEvent|null}
   */
  getPulledEventByEventId(eventId) {
    this.initialize();

    const stmt = this.db.prepare(`
      SELECT * FROM _ves_pulled_events WHERE event_id = ?
    `);

    const row = stmt.get(eventId);
    return row ? this._rowToEvent(row, 'pulled') : null;
  }

  /**
   * Get a pulled event by sequence number.
   * @param {number} sequenceNumber
   * @returns {OutboxEvent|null}
   */
  getPulledEventBySequence(sequenceNumber) {
    this.initialize();

    const stmt = this.db.prepare(`
      SELECT * FROM _ves_pulled_events WHERE sequence_number = ?
    `);

    const row = stmt.get(sequenceNumber);
    return row ? this._rowToEvent(row, 'pulled') : null;
  }

  /**
   * Find a stored sync event in the outbox or pulled-event store.
   * @param {Object} params
   * @param {string} [params.eventId]
   * @param {number} [params.sequenceNumber]
   * @param {'auto'|'outbox'|'pulled'} [params.source='auto']
   * @returns {OutboxEvent|null}
   */
  findStoredEvent({ eventId, sequenceNumber, source = 'auto' }) {
    this.initialize();

    if (sequenceNumber !== undefined && sequenceNumber !== null) {
      return this.getPulledEventBySequence(sequenceNumber);
    }

    if (!eventId) {
      return null;
    }

    if (source === 'outbox') {
      return this.getByEventId(eventId);
    }

    if (source === 'pulled') {
      return this.getPulledEventByEventId(eventId);
    }

    return this.getByEventId(eventId) || this.getPulledEventByEventId(eventId);
  }

  /**
   * Mark events as synced
   * @param {Array<{localSeq: number, remoteSeq: number}>} acks
   */
  markSynced(acks) {
    this.initialize();

    const stmt = this.db.prepare(`
      UPDATE _ves_outbox
      SET sync_status = 'synced',
          remote_sequence = ?,
          synced_at = datetime('now')
      WHERE local_seq = ?
    `);

    const transaction = this.db.transaction(() => {
      for (const { localSeq, remoteSeq } of acks) {
        stmt.run(remoteSeq, localSeq);
      }
    });

    transaction();
  }

  /**
   * Mark event as failed (retriable)
   * @param {number} localSeq
   * @param {string} error
   */
  markFailed(localSeq, error) {
    this.initialize();

    const stmt = this.db.prepare(`
      UPDATE _ves_outbox
      SET sync_status = 'failed',
          retry_count = retry_count + 1,
          last_error = ?
      WHERE local_seq = ?
    `);

    stmt.run(error, localSeq);
  }

  /**
   * Mark event as rejected (not retriable)
   * @param {number} localSeq
   * @param {string} reason
   */
  markRejected(localSeq, reason) {
    this.initialize();

    const stmt = this.db.prepare(`
      UPDATE _ves_outbox
      SET sync_status = 'rejected',
          rejection_reason = ?
      WHERE local_seq = ?
    `);

    stmt.run(reason, localSeq);
  }

  /**
   * Reset failed events to pending for retry
   * @returns {number} Number of events reset
   */
  retryFailed() {
    this.initialize();

    const stmt = this.db.prepare(`
      UPDATE _ves_outbox
      SET sync_status = 'pending'
      WHERE sync_status = 'failed'
    `);

    const result = stmt.run();
    return result.changes;
  }

  /**
   * Get outbox statistics
   * @returns {OutboxStats}
   */
  getStats() {
    this.initialize();

    const counts = this.db
      .prepare(
        `
      SELECT
        COUNT(*) as total,
        SUM(CASE WHEN sync_status = 'pending' THEN 1 ELSE 0 END) as pending,
        SUM(CASE WHEN sync_status = 'synced' THEN 1 ELSE 0 END) as synced,
        SUM(CASE WHEN sync_status = 'failed' THEN 1 ELSE 0 END) as failed,
        SUM(CASE WHEN sync_status = 'rejected' THEN 1 ELSE 0 END) as rejected
      FROM _ves_outbox
    `,
      )
      .get();

    const oldestPending = this.db
      .prepare(
        `
      SELECT created_at FROM _ves_outbox
      WHERE sync_status = 'pending'
      ORDER BY local_seq ASC
      LIMIT 1
    `,
      )
      .get();

    const lastSynced = this.db
      .prepare(
        `
      SELECT synced_at FROM _ves_outbox
      WHERE sync_status = 'synced'
      ORDER BY synced_at DESC
      LIMIT 1
    `,
      )
      .get();

    return {
      total: counts.total,
      pending: counts.pending,
      synced: counts.synced,
      failed: counts.failed,
      rejected: counts.rejected,
      oldestPending: oldestPending ? new Date(oldestPending.created_at) : null,
      lastSynced: lastSynced ? new Date(lastSynced.synced_at) : null,
    };
  }

  /**
   * Get count of pending events
   * @returns {number}
   */
  getPendingCount() {
    this.initialize();

    const result = this.db
      .prepare(
        `
      SELECT COUNT(*) as count FROM _ves_outbox WHERE sync_status = 'pending'
    `,
      )
      .get();

    return result.count;
  }

  /**
   * Prune old synced events
   * @param {number} olderThanDays
   * @returns {number} Number of events pruned
   */
  pruneOldEvents(olderThanDays) {
    this.initialize();

    const stmt = this.db.prepare(`
      DELETE FROM _ves_outbox
      WHERE sync_status = 'synced'
        AND synced_at < datetime('now', '-' || ? || ' days')
    `);

    const result = stmt.run(olderThanDays);
    return result.changes;
  }

  /**
   * Get sync state
   * @returns {SyncState}
   */
  getSyncState() {
    this.initialize();

    const getState = (key) => {
      const row = this.db.prepare('SELECT value FROM _ves_sync_state WHERE key = ?').get(key);
      return row ? row.value : null;
    };

    return {
      agentId: getState('agent_id') || crypto.randomUUID(),
      tenantId: getState('tenant_id'),
      storeId: getState('store_id'),
      lastPushedSequence: parseInt(getState('last_pushed_sequence') || '0', 10),
      lastPulledSequence: parseInt(getState('last_pulled_sequence') || '0', 10),
      headSequence: parseInt(getState('head_sequence') || '0', 10),
      lastSyncAt: getState('last_sync_at') ? new Date(getState('last_sync_at')) : new Date(),
    };
  }

  /**
   * Update sync state
   * @param {Partial<SyncState>} state
   */
  updateSyncState(state) {
    this.initialize();

    const stmt = this.db.prepare(`
      INSERT INTO _ves_sync_state (key, value, updated_at)
      VALUES (?, ?, datetime('now'))
      ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
    `);

    const transaction = this.db.transaction(() => {
      if (state.agentId !== undefined) {
        stmt.run('agent_id', state.agentId);
      }
      if (state.tenantId !== undefined) {
        stmt.run('tenant_id', state.tenantId);
      }
      if (state.storeId !== undefined) {
        stmt.run('store_id', state.storeId);
      }
      if (state.lastPushedSequence !== undefined) {
        stmt.run('last_pushed_sequence', state.lastPushedSequence.toString());
      }
      if (state.lastPulledSequence !== undefined) {
        stmt.run('last_pulled_sequence', state.lastPulledSequence.toString());
      }
      if (state.headSequence !== undefined) {
        stmt.run('head_sequence', state.headSequence.toString());
      }
      if (state.lastSyncAt !== undefined) {
        stmt.run('last_sync_at', state.lastSyncAt.toISOString());
      }
    });

    transaction();
  }

  /**
   * Store a pulled event from remote (VES v1.0)
   * @param {Object} event - Sequenced event from remote
   */
  storePulledEvent(event) {
    this.initialize();

    const stmt = this.db.prepare(`
      INSERT OR REPLACE INTO _ves_pulled_events (
        sequence_number, event_id, command_id,
        tenant_id, store_id,
        entity_type, entity_id, event_type,
        ves_version, payload, payload_kind, payload_encrypted,
        payload_plain_hash, payload_cipher_hash,
        agent_key_id, agent_signature, agent_signature_scheme, agent_signature_bundle,
        base_version, created_at, sequenced_at, source_agent
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `);

    stmt.run(
      event.sequenceNumber,
      event.eventId,
      event.commandId || null,
      event.tenantId,
      event.storeId,
      event.entityType,
      event.entityId,
      event.eventType,
      event.vesVersion || 1,
      JSON.stringify(event.payload),
      event.payloadKind || 0,
      event.payloadEncrypted ? JSON.stringify(event.payloadEncrypted) : null,
      event.payloadPlainHash,
      event.payloadCipherHash,
      event.agentKeyId,
      event.agentSignature,
      event.agentSignatureScheme || 0,
      event.agentSignatureBundle ? JSON.stringify(event.agentSignatureBundle) : null,
      event.baseVersion || null,
      event.createdAt,
      event.sequencedAt,
      event.sourceAgent,
    );
  }

  /**
   * Store multiple pulled events (VES v1.0)
   * @param {Array<Object>} events
   */
  storePulledEvents(events) {
    this.initialize();

    const stmt = this.db.prepare(`
      INSERT OR REPLACE INTO _ves_pulled_events (
        sequence_number, event_id, command_id,
        tenant_id, store_id,
        entity_type, entity_id, event_type,
        ves_version, payload, payload_kind, payload_encrypted,
        payload_plain_hash, payload_cipher_hash,
        agent_key_id, agent_signature, agent_signature_scheme, agent_signature_bundle,
        base_version, created_at, sequenced_at, source_agent
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `);

    const transaction = this.db.transaction(() => {
      for (const event of events) {
        stmt.run(
          event.sequenceNumber,
          event.eventId,
          event.commandId || null,
          event.tenantId,
          event.storeId,
          event.entityType,
          event.entityId,
          event.eventType,
          event.vesVersion || 1,
          JSON.stringify(event.payload),
          event.payloadKind || 0,
          event.payloadEncrypted ? JSON.stringify(event.payloadEncrypted) : null,
          event.payloadPlainHash,
          event.payloadCipherHash,
          event.agentKeyId,
          event.agentSignature,
          event.agentSignatureScheme || 0,
          event.agentSignatureBundle ? JSON.stringify(event.agentSignatureBundle) : null,
          event.baseVersion || null,
          event.createdAt,
          event.sequencedAt,
          event.sourceAgent,
        );
      }
    });

    transaction();
  }

  /**
   * Get entity version for optimistic concurrency
   * @param {string} tenantId
   * @param {string} storeId
   * @param {string} entityType
   * @param {string} entityId
   * @returns {number|null}
   */
  getEntityVersion(tenantId, storeId, entityType, entityId) {
    this.initialize();

    const row = this.db
      .prepare(
        `
      SELECT version FROM _ves_entity_versions
      WHERE tenant_id = ? AND store_id = ? AND entity_type = ? AND entity_id = ?
    `,
      )
      .get(tenantId, storeId, entityType, entityId);

    return row ? row.version : null;
  }

  /**
   * Update entity version
   * @param {string} tenantId
   * @param {string} storeId
   * @param {string} entityType
   * @param {string} entityId
   * @param {number} newVersion
   */
  updateEntityVersion(tenantId, storeId, entityType, entityId, newVersion) {
    this.initialize();

    const stmt = this.db.prepare(`
      INSERT INTO _ves_entity_versions (tenant_id, store_id, entity_type, entity_id, version, updated_at)
      VALUES (?, ?, ?, ?, ?, datetime('now'))
      ON CONFLICT(tenant_id, store_id, entity_type, entity_id)
      DO UPDATE SET version = excluded.version, updated_at = datetime('now')
    `);

    stmt.run(tenantId, storeId, entityType, entityId, newVersion);
  }

  /**
   * Convert database row to OutboxEvent
   * @private
   */
  _rowToEvent(row, source = 'outbox') {
    return {
      source,
      localSeq: row.local_seq,
      sequenceNumber: row.sequence_number ?? null,
      eventId: row.event_id,
      commandId: row.command_id,
      tenantId: row.tenant_id,
      storeId: row.store_id,
      entityType: row.entity_type,
      entityId: row.entity_id,
      eventType: row.event_type,
      payload: JSON.parse(row.payload),
      // VES v1.0 fields
      vesVersion: row.ves_version,
      payloadKind: row.payload_kind,
      payloadEncrypted: row.payload_encrypted ? JSON.parse(row.payload_encrypted) : null,
      payloadPlainHash: row.payload_plain_hash,
      payloadCipherHash: row.payload_cipher_hash,
      agentKeyId: row.agent_key_id,
      agentSignature: row.agent_signature,
      agentSignatureScheme: row.agent_signature_scheme ?? 0,
      agentSignatureBundle: row.agent_signature_bundle
        ? JSON.parse(row.agent_signature_bundle)
        : null,
      // Metadata
      baseVersion: row.base_version,
      sourceAgent: row.source_agent,
      createdAt: row.created_at ? new Date(row.created_at) : null,
      sequencedAt: row.sequenced_at ? new Date(row.sequenced_at) : null,
      // Sync tracking
      syncStatus: row.sync_status ?? null,
      remoteSequence: row.remote_sequence,
      syncedAt: row.synced_at ? new Date(row.synced_at) : null,
      rejectionReason: row.rejection_reason ?? null,
      retryCount: row.retry_count ?? 0,
      lastError: row.last_error ?? null,
    };
  }
}

/**
 * Create an Outbox instance with VES v1.0 support
 * @param {import('better-sqlite3').Database} db
 * @param {Object} [options] - Configuration options
 * @param {string} [options.configDir='.stateset'] - Config directory for keys
 * @param {import('./keys.js').AgentKeyManager} [options.keyManager] - Key manager instance
 * @param {'legacy' | 'hybrid' | 'pqc-strict'} [options.securityProfile='legacy'] - PQ migration profile
 * @returns {Outbox}
 */
export function createOutbox(db, options = {}) {
  const outbox = new Outbox(db, options);
  outbox.initialize();
  return outbox;
}

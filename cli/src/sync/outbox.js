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
  computePayloadCipherHash,
  computeEventSigningHash,
  signEventHash,
  encryptPayload,
  bufferToHex,
  ZERO_HASH,
} from './crypto.js';
import { getKeyManager } from './keys.js';

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
    this.keyManager = options.keyManager || getKeyManager(this.configDir);
    this._initialized = false;
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
    const payloadPlainHash = computePayloadPlainHash(event.payload);
    const payloadJson = JSON.stringify(event.payload);

    // Handle encryption if requested
    let payloadKind = 0;
    let payloadEncrypted = null;
    let payloadCipherHash = ZERO_HASH;

    if (options.encrypt && options.recipientPublicKey) {
      // Get encryption key for ECDH
      const encryptionKey = await this.keyManager.getCurrentEncryptionKey(event.sourceAgent);
      if (!encryptionKey) {
        throw new Error(`No encryption key found for agent ${event.sourceAgent}`);
      }

      // Encrypt payload per VES-ENC-1
      const encrypted = encryptPayload(
        event.payload,
        encryptionKey.privateKey,
        options.recipientPublicKey,
        {
          eventId,
          tenantId: event.tenantId,
          storeId: event.storeId,
          entityType: event.entityType,
          entityId: event.entityId,
          eventType: event.eventType,
        },
      );

      payloadKind = 1;
      payloadEncrypted = encrypted;
      payloadCipherHash = computePayloadCipherHash(Buffer.from(encrypted.ciphertext, 'base64'));
    }

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

    // Sign the event
    const agentSignature = signEventHash(eventSigningHash, signingKey.privateKey);

    const stmt = this.db.prepare(`
      INSERT INTO _ves_outbox (
        event_id, command_id, tenant_id, store_id,
        entity_type, entity_id, event_type,
        ves_version, payload, payload_kind, payload_encrypted,
        payload_plain_hash, payload_cipher_hash,
        agent_key_id, agent_signature,
        base_version, source_agent, created_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
    const encryptionKeys = new Map();

    for (const agentId of agentIds) {
      const signingKey = await this.keyManager.getCurrentSigningKey(agentId);
      if (!signingKey) {
        throw new Error(`No signing key found for agent ${agentId}`);
      }
      signingKeys.set(agentId, signingKey);

      if (options.encrypt && options.recipientPublicKey) {
        const encryptionKey = await this.keyManager.getCurrentEncryptionKey(agentId);
        if (!encryptionKey) {
          throw new Error(`No encryption key found for agent ${agentId}`);
        }
        encryptionKeys.set(agentId, encryptionKey);
      }
    }

    const stmt = this.db.prepare(`
      INSERT INTO _ves_outbox (
        event_id, command_id, tenant_id, store_id,
        entity_type, entity_id, event_type,
        ves_version, payload, payload_kind, payload_encrypted,
        payload_plain_hash, payload_cipher_hash,
        agent_key_id, agent_signature,
        base_version, source_agent, created_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `);

    const results = [];

    const transaction = this.db.transaction(() => {
      for (const event of events) {
        const eventId = event.eventId || this.generateEventId();
        const signingKey = signingKeys.get(event.sourceAgent);

        // Compute plaintext hash
        const payloadPlainHash = computePayloadPlainHash(event.payload);
        const payloadJson = JSON.stringify(event.payload);

        // Handle encryption
        let payloadKind = 0;
        let payloadEncrypted = null;
        let payloadCipherHash = ZERO_HASH;

        if (options.encrypt && options.recipientPublicKey) {
          const encryptionKey = encryptionKeys.get(event.sourceAgent);
          const encrypted = encryptPayload(
            event.payload,
            encryptionKey.privateKey,
            options.recipientPublicKey,
            {
              eventId,
              tenantId: event.tenantId,
              storeId: event.storeId,
              entityType: event.entityType,
              entityId: event.entityId,
              eventType: event.eventType,
            },
          );

          payloadKind = 1;
          payloadEncrypted = encrypted;
          payloadCipherHash = computePayloadCipherHash(Buffer.from(encrypted.ciphertext, 'base64'));
        }

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

        // Sign the event
        const agentSignature = signEventHash(eventSigningHash, signingKey.privateKey);

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
    return row ? this._rowToEvent(row) : null;
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
    return rows.map(this._rowToEvent);
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
        agent_key_id, agent_signature,
        base_version, created_at, sequenced_at, source_agent
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
        agent_key_id, agent_signature,
        base_version, created_at, sequenced_at, source_agent
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
  _rowToEvent(row) {
    return {
      localSeq: row.local_seq,
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
      // Metadata
      baseVersion: row.base_version,
      sourceAgent: row.source_agent,
      createdAt: new Date(row.created_at),
      // Sync tracking
      syncStatus: row.sync_status,
      remoteSequence: row.remote_sequence,
      syncedAt: row.synced_at ? new Date(row.synced_at) : null,
      rejectionReason: row.rejection_reason,
      retryCount: row.retry_count,
      lastError: row.last_error,
    };
  }
}

/**
 * Create an Outbox instance with VES v1.0 support
 * @param {import('better-sqlite3').Database} db
 * @param {Object} [options] - Configuration options
 * @param {string} [options.configDir='.stateset'] - Config directory for keys
 * @param {import('./keys.js').AgentKeyManager} [options.keyManager] - Key manager instance
 * @returns {Outbox}
 */
export function createOutbox(db, options = {}) {
  const outbox = new Outbox(db, options);
  outbox.initialize();
  return outbox;
}

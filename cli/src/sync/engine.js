/**
 * Sync Engine
 *
 * Orchestrates synchronization between local SQLite and remote sequencer.
 * Supports both REST and gRPC transports, with real-time streaming via gRPC.
 */

import { EventEmitter } from 'events';
import { createOutbox } from './outbox.js';
import { createUnifiedClient } from './unified-client.js';
import { SyncConfig, loadSyncConfig } from './config.js';
import { createConflictResolver } from './conflict.js';
import {
  computePayloadAad,
  decryptPayload,
  decryptPayloadHybrid,
  decryptPayloadStrict,
  hexToBuffer,
} from './crypto.js';
import {
  KEY_WRAP_SCHEME_ML_KEM_768,
  KEY_WRAP_SCHEME_X25519_HKDF_SHA256,
  KEY_WRAP_SCHEME_X25519_ML_KEM_768,
  getPayloadWrapScheme,
} from './pqc.js';

function collectRecipientKeyIds(payloadEncrypted) {
  const keyIds = new Set();

  for (const recipient of payloadEncrypted?.recipients || []) {
    const recipientKid = Number(recipient?.recipient_kid ?? recipient?.recipientKid ?? 0);
    if (Number.isInteger(recipientKid) && recipientKid > 0) {
      keyIds.add(recipientKid);
    }
  }

  for (const wrap of payloadEncrypted?.recipientWraps ?? payloadEncrypted?.recipient_wraps ?? []) {
    const recipientKid = Number(wrap?.recipientKid ?? wrap?.recipient_kid ?? 0);
    if (Number.isInteger(recipientKid) && recipientKid > 0) {
      keyIds.add(recipientKid);
    }
  }

  return Array.from(keyIds).sort((left, right) => left - right);
}

/**
 * @typedef {Object} PushResult
 * @property {boolean} success
 * @property {number} pushed - Events pushed
 * @property {number} rejected - Events rejected
 * @property {Object} [receipt] - Ingest receipt
 * @property {string} [error] - Error message
 */

/**
 * @typedef {Object} PullResult
 * @property {boolean} success
 * @property {number} pulled - Events pulled
 * @property {number} applied - Events applied locally
 * @property {number} conflicts - Conflicts detected
 * @property {number[]} [sequenceNumbers] - Pulled sequence numbers stored locally
 * @property {string} [error] - Error message
 */

/**
 * @typedef {Object} SyncStatus
 * @property {boolean} connected - Connection status
 * @property {'grpc'|'rest'|null} transport - Transport type
 * @property {boolean} streaming - Whether streaming is active
 * @property {number} localHead - Local head sequence
 * @property {number} remoteHead - Remote head sequence
 * @property {number} pending - Pending events to push
 * @property {number} lag - Events behind remote
 * @property {Date} [lastPush] - Last push timestamp
 * @property {Date} [lastPull] - Last pull timestamp
 * @property {number} conflicts - Unresolved conflicts
 * @property {number} bufferedEvents - Events in stream buffer
 */

/**
 * Sync Engine for orchestrating local-remote sync
 */
export class SyncEngine extends EventEmitter {
  /**
   * @param {Object} options
   * @param {import('better-sqlite3').Database} options.db - SQLite database
   * @param {SyncConfig} options.config - Sync configuration
   * @param {boolean} [options.preferGrpc=true] - Prefer gRPC when available
   * @param {boolean} [options.enableStreaming=true] - Enable real-time streaming
   * @param {string} [options.configDir] - Config directory for keys
   * @param {import('./keys.js').AgentKeyManager} [options.keyManager] - Key manager instance
   */
  constructor(options) {
    super();
    this.db = options.db;
    this.config = options.config;
    this.outbox = createOutbox(options.db, {
      configDir: options.configDir,
      keyManager: options.keyManager,
      securityProfile:
        options.config?.securityProfile ?? options.config?.sync?.securityProfile,
    });
    this.client = createUnifiedClient(options.config, {
      preferGrpc: options.preferGrpc !== false,
      enableStreaming: options.enableStreaming !== false,
    });
    this.resolver = createConflictResolver(this.outbox, {
      defaultStrategy: options.defaultStrategy || 'remote-wins',
    });
    this._backgroundInterval = null;
    this._initialized = false;
    this._streamingEnabled = false;
    this._eventBuffer = [];
    this._eventBufferSize = 1000;
  }

  /**
   * Initialize the sync engine
   * @returns {Promise<void>}
   */
  async initialize() {
    if (this._initialized) return;

    // Initialize outbox schema
    this.outbox.initialize();

    // Set up identity in sync state if not set
    const state = this.outbox.getSyncState();
    if (!state.agentId || !state.tenantId || !state.storeId) {
      this.outbox.updateSyncState({
        agentId: this.config.agentId,
        tenantId: this.config.tenantId,
        storeId: this.config.storeId,
      });
    }

    // Set up event handlers for the unified client
    this._setupClientHandlers();

    // Connect to sequencer
    try {
      await this.client.connect();
      this.emit('connected', { transport: this.client.transport });
    } catch (error) {
      this.emit('error', error);
      // Don't throw - allow offline operation
    }

    this._initialized = true;
  }

  /**
   * Set up event handlers for the unified client
   * @private
   */
  _setupClientHandlers() {
    // Forward client events
    this.client.on('connected', () => {
      this.emit('transport:connected', { transport: this.client.transport });
    });

    this.client.on('disconnected', () => {
      this._streamingEnabled = false;
      this.emit('transport:disconnected');
    });

    this.client.on('error', (err) => {
      this.emit('error', err);
    });

    // Handle streaming events
    this.client.on('event', (event) => {
      this._handleStreamedEvent(event);
    });

    this.client.on('push-ack', (ack) => {
      this.emit('push:ack', ack);
    });

    this.client.on('sync-state', (state) => {
      this.emit('sync:state', state);
    });
  }

  /**
   * Handle an event received from the stream
   * @private
   */
  _handleStreamedEvent(event) {
    // Buffer the event
    this._eventBuffer.push(event);

    // Trim buffer if too large
    if (this._eventBuffer.length > this._eventBufferSize) {
      this._eventBuffer.shift();
    }

    // Emit the event for real-time consumers
    this.emit('event', event);

    // Store the event locally
    try {
      // Convert payloadHash buffer to hex string if needed
      const payloadHashHex = event.payloadHash
        ? Buffer.isBuffer(event.payloadHash)
          ? event.payloadHash.toString('hex')
          : event.payloadHash
        : '0'.repeat(64);

      this.outbox.storePulledEvents([
        {
          sequenceNumber: event.sequenceNumber,
          eventId: event.eventId,
          commandId: event.commandId,
          tenantId: event.tenantId,
          storeId: event.storeId,
          entityType: event.entityType,
          entityId: event.entityId,
          eventType: event.eventType,
          payload: event.payload,
          vesVersion: event.vesVersion || 1,
          payloadKind: event.payloadKind || 0,
          payloadEncrypted: event.payloadEncrypted || null,
          payloadPlainHash: payloadHashHex,
          payloadCipherHash: '0'.repeat(64), // Zero hash for plaintext
          agentKeyId: event.agentKeyId || 0,
          agentSignature: event.agentSignature || '',
          agentSignatureScheme: event.agentSignatureScheme || 0,
          agentSignatureBundle: event.agentSignatureBundle || null,
          baseVersion: event.baseVersion,
          createdAt:
            event.createdAt instanceof Date ? event.createdAt.toISOString() : event.createdAt,
          sequencedAt:
            event.sequencedAt instanceof Date ? event.sequencedAt.toISOString() : event.sequencedAt,
          sourceAgent: event.sourceAgent,
        },
      ]);

      // Update sync state
      this.outbox.updateSyncState({
        lastPulledSequence: event.sequenceNumber,
        lastSyncAt: new Date(),
      });
    } catch (err) {
      this.emit('error', err);
    }
  }

  /**
   * Shutdown the sync engine
   * @returns {Promise<void>}
   */
  async shutdown() {
    this.stopBackgroundSync();
    await this.client.disconnect();
    this._initialized = false;
  }

  /**
   * Push pending events to sequencer
   * @param {Object} [options]
   * @param {number} [options.batchSize] - Max events per batch
   * @param {boolean} [options.dryRun] - Don't actually push
   * @returns {Promise<PushResult>}
   */
  async push(options = {}) {
    const batchSize = options.batchSize || this.config.batchSize;

    try {
      // Get pending events
      const pending = this.outbox.getPending(batchSize);

      if (pending.length === 0) {
        return {
          success: true,
          pushed: 0,
          rejected: 0,
        };
      }

      if (options.dryRun) {
        return {
          success: true,
          pushed: pending.length,
          rejected: 0,
        };
      }

      // Convert to event envelopes for push (VES v1.0)
      const events = pending.map((e) => ({
        eventId: e.eventId,
        commandId: e.commandId,
        tenantId: e.tenantId,
        storeId: e.storeId,
        entityType: e.entityType,
        entityId: e.entityId,
        eventType: e.eventType,
        // VES v1.0 payload fields
        payload: e.payload,
        vesVersion: e.vesVersion || 1,
        payloadKind: e.payloadKind || 0,
        payloadEncrypted: e.payloadEncrypted,
        payloadPlainHash: e.payloadPlainHash,
        payloadCipherHash: e.payloadCipherHash,
        // VES v1.0 signature fields
        agentKeyId: e.agentKeyId,
        agentSignature: e.agentSignature,
        agentSignatureScheme: e.agentSignatureScheme || 0,
        agentSignatureBundle: e.agentSignatureBundle || null,
        // Metadata
        baseVersion: e.baseVersion,
        createdAt: e.createdAt.toISOString(),
        sourceAgent: e.sourceAgent,
      }));

      // Push to sequencer
      const receipt = await this.client.pushWithRetry({
        agentId: this.config.agentId,
        events,
      });

      // Mark events as synced
      if (receipt.eventsAccepted > 0) {
        const acks = [];
        let seq = receipt.sequenceStart;

        for (const event of pending) {
          // Check if this event was rejected
          const rejected = receipt.rejections?.find((r) => r.eventId === event.eventId);

          if (rejected) {
            this.outbox.markRejected(event.localSeq, rejected.reason);
          } else {
            acks.push({ localSeq: event.localSeq, remoteSeq: seq });
            seq++;
          }
        }

        if (acks.length > 0) {
          this.outbox.markSynced(acks);
        }
      }

      // Update sync state
      this.outbox.updateSyncState({
        lastPushedSequence: receipt.headSequence,
        headSequence: receipt.headSequence,
        lastSyncAt: new Date(),
      });

      this.emit('push', {
        pushed: receipt.eventsAccepted,
        rejected: receipt.eventsRejected,
        receipt,
      });

      return {
        success: true,
        pushed: receipt.eventsAccepted,
        rejected: receipt.eventsRejected,
        receipt,
      };
    } catch (error) {
      this.emit('error', error);
      return {
        success: false,
        pushed: 0,
        rejected: 0,
        error: error.message,
      };
    }
  }

  /**
   * Pull events from sequencer
   * @param {Object} [options]
   * @param {number} [options.fromSequence] - Start sequence
   * @param {number} [options.limit] - Max events to pull
   * @param {boolean} [options.dryRun] - Don't apply locally
   * @returns {Promise<PullResult>}
   */
  async pull(options = {}) {
    try {
      const state = this.outbox.getSyncState();
      const fromSequence = options.fromSequence ?? state.lastPulledSequence;
      const limit = options.limit || 1000;

      // Pull events
      const result = await this.client.pull(fromSequence, limit);

      if (result.events.length === 0) {
        return {
          success: true,
          pulled: 0,
          applied: 0,
          conflicts: 0,
          sequenceNumbers: options.includeEvents ? [] : undefined,
        };
      }

      if (options.dryRun) {
        return {
          success: true,
          pulled: result.events.length,
          applied: 0,
          conflicts: 0,
        };
      }

      // Store pulled events (VES v1.0 format)
      const eventsToStore = result.events.map((e) => ({
        sequenceNumber: e.sequenceNumber,
        eventId: e.envelope.eventId,
        commandId: e.envelope.commandId,
        tenantId: e.envelope.tenantId,
        storeId: e.envelope.storeId,
        entityType: e.envelope.entityType,
        entityId: e.envelope.entityId,
        eventType: e.envelope.eventType,
        payload: e.envelope.payload,
        // VES v1.0 fields
        vesVersion: e.envelope.vesVersion || 1,
        payloadKind: e.envelope.payloadKind || 0,
        payloadEncrypted: e.envelope.payloadEncrypted,
        payloadPlainHash: e.envelope.payloadPlainHash,
        payloadCipherHash: e.envelope.payloadCipherHash,
        agentKeyId: e.envelope.agentKeyId,
        agentSignature: e.envelope.agentSignature,
        agentSignatureScheme: e.envelope.agentSignatureScheme || 0,
        agentSignatureBundle: e.envelope.agentSignatureBundle || null,
        baseVersion: e.envelope.baseVersion,
        createdAt: e.envelope.createdAt,
        sequencedAt: e.sequencedAt,
        sourceAgent: e.envelope.sourceAgent,
      }));

      // Verify receipt signatures when sequencer public key is configured
      const sequencerPublicKey =
        this.config?.sequencerPublicKey ?? this.config?.sync?.sequencerPublicKey;
      if (sequencerPublicKey && this.client?.verifyReceiptSignature) {
        let verified = 0;
        let skipped = 0;
        for (const event of result.events) {
          if (event.receiptHash && event.receiptSignatureBundle) {
            try {
              const valid = this.client.verifyReceiptSignature(event, sequencerPublicKey);
              if (valid) {
                verified++;
              } else {
                this.emit('receipt-verification-failed', {
                  sequenceNumber: event.sequenceNumber,
                  eventId: event.envelope?.eventId,
                });
              }
            } catch {
              skipped++;
            }
          } else {
            skipped++;
          }
        }
        if (verified > 0 || skipped < result.events.length) {
          this.emit('receipt-verification', { verified, skipped, total: result.events.length });
        }
      }

      this.outbox.storePulledEvents(eventsToStore);

      // Update sync state
      this.outbox.updateSyncState({
        lastPulledSequence: result.nextSequence,
        headSequence: result.headSequence,
        lastSyncAt: new Date(),
      });

      this.emit('pull', {
        pulled: result.events.length,
        applied: result.events.length,
        conflicts: 0,
      });

      return {
        success: true,
        pulled: result.events.length,
        applied: result.events.length,
        conflicts: 0,
        sequenceNumbers: options.includeEvents
          ? eventsToStore.map((event) => event.sequenceNumber)
          : undefined,
      };
    } catch (error) {
      this.emit('error', error);
      return {
        success: false,
        pulled: 0,
        applied: 0,
        conflicts: 0,
        error: error.message,
      };
    }
  }

  /**
   * Perform a full sync (push then pull)
   * @returns {Promise<{push: PushResult, pull: PullResult}>}
   */
  async fullSync() {
    const pushResult = await this.push();
    const pullResult = await this.pull();

    return {
      push: pushResult,
      pull: pullResult,
    };
  }

  /**
   * Get current sync status
   * @returns {Promise<SyncStatus>}
   */
  async getStatus() {
    const state = this.outbox.getSyncState();
    const stats = this.outbox.getStats();

    let remoteHead = state.headSequence;
    let connected = false;

    try {
      if (this.client.isConnected()) {
        const remoteState = await this.client.getHead();
        remoteHead = remoteState.headSequence;
        connected = true;
      }
    } catch (err) {
      console.debug('[sync-engine] Remote head fetch failed:', err.message || err);
    }

    return {
      connected,
      transport: this.getTransport(),
      streaming: this._streamingEnabled,
      localHead: state.lastPulledSequence,
      remoteHead,
      pending: stats.pending,
      lag: remoteHead - state.lastPulledSequence,
      lastPush: stats.lastSynced,
      lastPull: state.lastSyncAt,
      conflicts: this.resolver.getConflictCount(),
      bufferedEvents: this._eventBuffer.length,
    };
  }

  /**
   * Get health status
   * @returns {Promise<{healthy: boolean, details: Object}>}
   */
  async getHealth() {
    const status = await this.getStatus();

    const healthy = status.connected && status.lag < 100 && status.pending < 1000;

    return {
      healthy,
      details: {
        connected: status.connected,
        lag: status.lag,
        pending: status.pending,
      },
    };
  }

  /**
   * Detect conflicts between local pending events and pulled remote events
   * @returns {Promise<Array<import('./conflict.js').ConflictInfo>>}
   */
  async detectConflicts() {
    const pending = this.outbox.getPending(1000);
    const pulledEvents = this._getPulledEvents();
    return this.resolver.detectConflicts(pending, pulledEvents);
  }

  /**
   * Get pulled events from the database
   * @private
   */
  _getPulledEvents() {
    return this.outbox.getPulledEvents(1000);
  }

  /**
   * Get pulled events from local storage.
   * @param {number} [limit=1000]
   * @returns {Array<import('./outbox.js').OutboxEvent>}
   */
  getPulledEvents(limit = 1000) {
    return this.outbox.getPulledEvents(limit);
  }

  /**
   * Get pulled events for a specific entity from local storage.
   * @param {string} entityType
   * @param {string} entityId
   * @param {number} [limit=1000]
   * @returns {Array<import('./outbox.js').OutboxEvent>}
   */
  getPulledEventsForEntity(entityType, entityId, limit = 1000) {
    return this.outbox.getPulledEventsByEntity(entityType, entityId, limit);
  }

  /**
   * Get a stored sync event from the outbox or pulled-event store.
   * @param {Object} params
   * @param {string} [params.eventId]
   * @param {number} [params.sequenceNumber]
   * @param {'auto'|'outbox'|'pulled'} [params.source='auto']
   * @returns {import('./outbox.js').OutboxEvent|null}
   */
  getStoredEvent(params) {
    return this.outbox.findStoredEvent(params);
  }

  async _resolveRecipientDecryptionKey(payloadEncrypted, requestedKeyId) {
    const candidateKeyIds = collectRecipientKeyIds(payloadEncrypted);

    if (requestedKeyId !== undefined) {
      if (candidateKeyIds.length > 0 && !candidateKeyIds.includes(requestedKeyId)) {
        throw new Error(
          `Encryption key ${requestedKeyId} is not listed as a recipient for this event`,
        );
      }
      const key = await this.outbox.keyManager.getEncryptionKey(this.config.agentId, requestedKeyId);
      if (!key) {
        throw new Error(
          `Encryption key ${requestedKeyId} not found for agent ${this.config.agentId}`,
        );
      }
      return { key, keyId: key.keyId, candidateKeyIds };
    }

    const currentKey = await this.outbox.keyManager.getCurrentEncryptionKey(this.config.agentId);
    if (
      currentKey &&
      (candidateKeyIds.length === 0 || candidateKeyIds.includes(currentKey.keyId))
    ) {
      return { key: currentKey, keyId: currentKey.keyId, candidateKeyIds };
    }

    if (candidateKeyIds.length === 1) {
      const key = await this.outbox.keyManager.getEncryptionKey(
        this.config.agentId,
        candidateKeyIds[0],
      );
      if (!key) {
        throw new Error(
          `Encryption key ${candidateKeyIds[0]} not found for agent ${this.config.agentId}`,
        );
      }
      return { key, keyId: key.keyId, candidateKeyIds };
    }

    if (candidateKeyIds.length > 1) {
      throw new Error(
        `Multiple recipient key IDs found (${candidateKeyIds.join(', ')}); specify keyId explicitly`,
      );
    }

    throw new Error(`No local encryption key available for agent ${this.config.agentId}`);
  }

  /**
   * Decrypt a stored encrypted sync event using local key material.
   * @param {Object} params
   * @param {string} [params.eventId]
   * @param {number} [params.sequenceNumber]
   * @param {'auto'|'outbox'|'pulled'} [params.source='auto']
   * @param {number} [params.keyId]
   * @returns {Promise<Object>}
   */
  async decryptStoredEvent({ eventId, sequenceNumber, source = 'auto', keyId } = {}) {
    const event = this.getStoredEvent({ eventId, sequenceNumber, source });
    if (!event) {
      throw new Error('Stored sync event not found');
    }

    if (Number(event.payloadKind ?? 0) !== 1 || !event.payloadEncrypted) {
      throw new Error('Event is not encrypted');
    }

    const payloadPlainHash = hexToBuffer(event.payloadPlainHash);
    const payloadAad = computePayloadAad({
      vesVersion: event.vesVersion,
      tenantId: event.tenantId,
      storeId: event.storeId,
      eventId: event.eventId,
      sourceAgentId: event.sourceAgent,
      agentKeyId: event.agentKeyId,
      entityType: event.entityType,
      entityId: event.entityId,
      eventType: event.eventType,
      createdAt:
        event.createdAt instanceof Date ? event.createdAt.toISOString() : event.createdAt,
      payloadPlainHash,
    });

    const wrapScheme = getPayloadWrapScheme(event.payloadEncrypted);
    const { key: encryptionKey, keyId: resolvedKeyId, candidateKeyIds } =
      await this._resolveRecipientDecryptionKey(event.payloadEncrypted, keyId);

    let payload = null;
    let encryptionProfile = 'legacy';

    if (wrapScheme === KEY_WRAP_SCHEME_X25519_ML_KEM_768) {
      if (
        !encryptionKey.privateKeyBundle?.x25519PrivateKey ||
        !encryptionKey.privateKeyBundle?.mlKem768Seed
      ) {
        throw new Error(
          `Encryption key ${resolvedKeyId} does not include hybrid private key material`,
        );
      }

      payload = decryptPayloadHybrid(
        event.payloadEncrypted,
        payloadAad,
        resolvedKeyId,
        encryptionKey.privateKeyBundle,
        payloadPlainHash,
      );
      encryptionProfile = 'hybrid';
    } else if (wrapScheme === KEY_WRAP_SCHEME_X25519_HKDF_SHA256 || wrapScheme === 0) {
      payload = decryptPayload(
        event.payloadEncrypted,
        payloadAad,
        resolvedKeyId,
        encryptionKey.privateKey,
        payloadPlainHash,
      );
    } else if (wrapScheme === KEY_WRAP_SCHEME_ML_KEM_768) {
      if (!encryptionKey.privateKeyBundle?.mlKem768Seed) {
        throw new Error(
          `Encryption key ${resolvedKeyId} does not include ML-KEM-768 private key material`,
        );
      }

      payload = decryptPayloadStrict(
        event.payloadEncrypted,
        payloadAad,
        resolvedKeyId,
        encryptionKey.privateKeyBundle,
        payloadPlainHash,
      );
      encryptionProfile = 'pqc-strict';
    } else {
      throw new Error(`Unsupported payload wrap scheme: ${wrapScheme}`);
    }

    return {
      source: event.source,
      eventId: event.eventId,
      sequenceNumber: event.sequenceNumber,
      localSeq: event.localSeq,
      entityType: event.entityType,
      entityId: event.entityId,
      eventType: event.eventType,
      encryptionProfile,
      wrapScheme,
      recipientKeyId: resolvedKeyId,
      recipientKeyCandidates: candidateKeyIds,
      payload,
    };
  }

  /**
   * Check for conflicts
   * @returns {Promise<boolean>}
   */
  async hasConflicts() {
    // First check for any stored unresolved conflicts
    const storedCount = this.resolver.getConflictCount();
    if (storedCount > 0) {
      return true;
    }

    // Then detect any new conflicts
    const newConflicts = await this.detectConflicts();
    return newConflicts.length > 0;
  }

  /**
   * Get conflict details
   * @returns {Promise<Array<import('./conflict.js').ConflictInfo>>}
   */
  async getConflicts() {
    // First detect any new conflicts
    await this.detectConflicts();
    // Return all unresolved conflicts
    return this.resolver.getUnresolvedConflicts();
  }

  /**
   * Resolve a specific conflict
   * @param {string} conflictId - Conflict ID
   * @param {import('./conflict.js').ResolutionStrategy} [strategy] - Resolution strategy
   * @returns {Promise<import('./conflict.js').Resolution>}
   */
  async resolveConflict(conflictId, strategy) {
    const result = await this.resolver.resolve(conflictId, strategy);
    if (result.success) {
      this.emit('conflictResolved', result);
    }
    return result;
  }

  /**
   * Rebase local state after conflict
   * @param {Object} [options]
   * @param {boolean} [options.force] - Force remote wins (deprecated, use strategy)
   * @param {import('./conflict.js').ResolutionStrategy} [options.strategy='remote-wins'] - Resolution strategy
   * @returns {Promise<{success: boolean, rebased: number, failed: number, errors: Array}>}
   */
  async rebase(options = {}) {
    // Detect any new conflicts first
    await this.detectConflicts();

    const conflicts = this.resolver.getUnresolvedConflicts();

    if (conflicts.length === 0) {
      return {
        success: true,
        rebased: 0,
        failed: 0,
        errors: [],
      };
    }

    // Determine strategy - force flag maps to remote-wins for backward compat
    const strategy = options.strategy || (options.force ? 'remote-wins' : 'remote-wins');

    let rebased = 0;
    let failed = 0;
    const errors = [];

    for (const conflict of conflicts) {
      const result = await this.resolver.resolve(conflict, strategy);
      if (result.success) {
        rebased++;
        this.emit('conflictResolved', result);
      } else {
        failed++;
        errors.push({ conflictId: conflict.id, error: result.error });
      }
    }

    this.emit('rebase', { rebased, failed, strategy });

    return {
      success: failed === 0,
      rebased,
      failed,
      errors,
    };
  }

  /**
   * Skip a conflict without resolving
   * @param {string} conflictId
   * @param {string} [reason]
   */
  skipConflict(conflictId, reason) {
    this.resolver.skipConflict(conflictId, reason);
    this.emit('conflictSkipped', { conflictId, reason });
  }

  /**
   * Start background sync
   * @param {number} [intervalMs] - Sync interval
   */
  startBackgroundSync(intervalMs) {
    const interval = intervalMs || this.config.sync.syncIntervalMs;

    if (this._backgroundInterval) {
      this.stopBackgroundSync();
    }

    this._backgroundInterval = setInterval(async () => {
      try {
        await this.fullSync();
      } catch (error) {
        this.emit('error', error);
      }
    }, interval);
    if (this._backgroundInterval.unref) this._backgroundInterval.unref();

    this.emit('backgroundSyncStarted', { interval });
  }

  /**
   * Stop background sync
   */
  stopBackgroundSync() {
    if (this._backgroundInterval) {
      clearInterval(this._backgroundInterval);
      this._backgroundInterval = null;
      this.emit('backgroundSyncStopped');
    }
  }

  /**
   * Check if background sync is running
   * @returns {boolean}
   */
  isBackgroundSyncRunning() {
    return this._backgroundInterval !== null;
  }

  // ===========================================================================
  // STREAMING SYNC (gRPC only)
  // ===========================================================================

  /**
   * Check if streaming is supported (requires gRPC)
   * @returns {boolean}
   */
  supportsStreaming() {
    return this.client?.supportsStreaming || false;
  }

  /**
   * Check if streaming is active
   * @returns {boolean}
   */
  isStreaming() {
    return this._streamingEnabled;
  }

  /**
   * Start real-time streaming sync.
   * Only available with gRPC transport.
   * @param {Object} [options]
   * @param {string[]} [options.entityTypeFilter] - Filter by entity types
   * @param {string[]} [options.eventTypeFilter] - Filter by event types
   * @param {boolean} [options.bidirectional=false] - Use bidirectional sync stream
   * @returns {boolean} Whether streaming was started
   */
  startStreamingSync(options = {}) {
    if (!this.supportsStreaming()) {
      this.emit('error', new Error('Streaming requires gRPC transport'));
      return false;
    }

    if (this._streamingEnabled) {
      return true;
    }

    const state = this.outbox.getSyncState();

    if (options.bidirectional) {
      // Use bidirectional sync stream for push/pull
      const started = this.client.startSyncStream();
      if (started) {
        this._streamingEnabled = true;
        this.emit('streamingStarted', { mode: 'bidirectional' });

        // Pull initial events
        this.client.pullViaStream({ fromSequence: state.lastPulledSequence || 0 });
      }
      return started;
    } else {
      // Use server-side streaming for pull only
      const started = this.client.startStreaming({
        fromSequence: state.lastPulledSequence || 0,
        entityTypeFilter: options.entityTypeFilter,
        eventTypeFilter: options.eventTypeFilter,
      });
      if (started) {
        this._streamingEnabled = true;
        this.emit('streamingStarted', { mode: 'server' });
      }
      return started;
    }
  }

  /**
   * Stop streaming sync
   */
  stopStreamingSync() {
    if (!this._streamingEnabled) return;

    this.client.stopStreaming();
    this._streamingEnabled = false;
    this.emit('streamingStopped');
  }

  /**
   * Push events via the bidirectional stream.
   * Requires startStreamingSync({ bidirectional: true }) to be called first.
   * @param {Array} events - Events to push
   * @returns {boolean}
   */
  pushViaStream(events) {
    if (!this._streamingEnabled || !this.client.isStreaming()) {
      return false;
    }

    // Convert to stream format
    const streamEvents = events.map((e) => ({
      eventId: e.eventId,
      commandId: e.commandId,
      entityType: e.entityType,
      entityId: e.entityId,
      eventType: e.eventType,
      payload: e.payload,
      baseVersion: e.baseVersion,
      createdAt: e.createdAt,
    }));

    return this.client.pushViaStream(streamEvents);
  }

  /**
   * Subscribe to a specific entity's events.
   * Only available with gRPC transport.
   * @param {string} entityType
   * @param {string} entityId
   * @param {Function} callback - Called for each event
   * @returns {Object|null} Subscription handle or null if not supported
   */
  subscribeEntity(entityType, entityId, callback) {
    if (!this.supportsStreaming()) {
      return null;
    }

    return this.client.subscribeEntity(entityType, entityId, callback);
  }

  /**
   * Get recent events from the buffer
   * @param {number} [limit=100]
   * @returns {Array}
   */
  getRecentEvents(limit = 100) {
    const start = Math.max(0, this._eventBuffer.length - limit);
    return this._eventBuffer.slice(start);
  }

  /**
   * Register an event listener for streamed events
   * @param {Function} callback
   */
  onEvent(callback) {
    this.on('event', callback);
  }

  /**
   * Remove event listener
   * @param {Function} callback
   */
  offEvent(callback) {
    this.off('event', callback);
  }

  // ===========================================================================
  // TRANSPORT INFO
  // ===========================================================================

  /**
   * Get current transport type
   * @returns {'grpc' | 'rest' | null}
   */
  getTransport() {
    return this.client?.transport || null;
  }

  /**
   * Get transport capabilities
   * @returns {Object}
   */
  getCapabilities() {
    return {
      transport: this.getTransport(),
      streaming: this.supportsStreaming(),
      bidirectionalSync: this.supportsStreaming(),
      entitySubscription: this.supportsStreaming(),
      batchPush: true,
      inclusionProofs: true,
    };
  }
}

/**
 * Create a sync engine
 * @param {Object} options
 * @param {import('better-sqlite3').Database} options.db - SQLite database
 * @param {SyncConfig} [options.config] - Sync configuration
 * @param {string} [options.cwd] - Working directory for config
 * @param {boolean} [options.preferGrpc=true] - Prefer gRPC when available
 * @param {boolean} [options.enableStreaming=true] - Enable real-time streaming
 * @param {string} [options.configDir] - Config directory for keys
 * @param {import('./keys.js').AgentKeyManager} [options.keyManager] - Key manager instance
 * @returns {SyncEngine}
 */
export function createSyncEngine(options) {
  const config = options.config || loadSyncConfig(options.cwd);

  if (!config) {
    throw new Error('Sync not configured. Run "stateset-sync init" first.');
  }

  return new SyncEngine({
    db: options.db,
    config: config instanceof SyncConfig ? config : new SyncConfig(config),
    preferGrpc: options.preferGrpc,
    enableStreaming: options.enableStreaming,
    configDir: options.configDir,
    keyManager: options.keyManager,
  });
}

/**
 * Check if gRPC is available for streaming
 * @returns {Promise<boolean>}
 */
export async function checkGrpcAvailable() {
  const { checkGrpcAvailability } = await import('./unified-client.js');
  return checkGrpcAvailability();
}

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
import { createPeerKeyDirectory } from './key-directory.js';
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
 * @property {number} pulled - Events returned by the sequencer
 * @property {number} verified - Events whose author signature verified
 * @property {number} quarantined - Events actually written to the quarantine table
 * @property {number} stored - Events actually written to the local pulled-event store
 * @property {number|null} conflicts - Conflicts outstanding between local pending
 *   events and locally stored remote events. `null` means NOT COMPUTED on this
 *   branch — an empty pull, a dry run, or a failed pull. It is deliberately not
 *   0 there: 0 is a claim that there are no conflicts, and these branches make
 *   no such claim. Computing it costs a read and JSON.parse of up to 1,000
 *   pending plus 1,000 pulled rows, which is not worth paying on every idle
 *   poll of the background sync loop.
 * @property {number[]} [sequenceNumbers] - Sequence numbers of the events stored locally
 * @property {string} [error] - Error message
 *
 * There is deliberately no `applied` field: pulled events are stored and made
 * available for reads, but nothing is applied to local entity state yet.
 *
 * `stored <= verified` and `quarantined <= pulled - verified`. The local schema
 * can refuse a record whose attacker-controlled envelope violates a NOT NULL
 * column; those events are named individually in `receive-store-failed` /
 * `receive-quarantine-failed` events rather than silently inflating a count.
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
      securityProfile: options.config?.securityProfile ?? options.config?.sync?.securityProfile,
    });
    this.client = createUnifiedClient(options.config, {
      preferGrpc: options.preferGrpc !== false,
      enableStreaming: options.enableStreaming !== false,
    });
    this.resolver = createConflictResolver(this.outbox, {
      defaultStrategy: options.defaultStrategy || 'remote-wins',
    });
    this.keyDirectory = createPeerKeyDirectory(this.outbox, this.client, this.config);
    this._backgroundInterval = null;
    this._initialized = false;
    this._streamingEnabled = false;
    this._streamStoreRefusalWarned = false;
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
   * Handle an event received from the stream.
   *
   * The stream is a NOTIFICATION channel only: it never writes to
   * `_ves_pulled_events` and never advances `lastPulledSequence`. It used to do
   * both, with no key resolution and no signature check, which made a hostile
   * or compromised sequencer stream a second, unverified writer into the exact
   * table peer verification exists to protect.
   *
   * It fails closed rather than verifying inline because the streamed record
   * cannot be verified as it stands: the gRPC event is flat (no `envelope`),
   * carries no `payloadCipherHash` (this code fabricated a zero hash), and
   * defaults `agentSignature` to the empty string — so the signing preimage
   * cannot be reconstructed. Verifying it would reject legitimate events;
   * skipping verification would reopen the hole.
   *
   * There is no "the next pull() picks these up" mitigation, and an earlier
   * version of this comment claiming one was wrong. Streaming exists only on
   * gRPC, and the gRPC receive path stores nothing at all: `pull()` refuses to
   * run on that transport (see `pull()`), the gRPC envelope mapping cannot
   * satisfy `verifyEventSignature`, and the gRPC key directory is never
   * cryptographically attested. A gRPC deployment that needs the receive path
   * must move to an https:// sequencer URL. Repairing the gRPC envelope
   * mapping is separate, tracked work.
   *
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

    const error = new Error(
      'Refusing to store a streamed event: streamed events carry no verifiable signature, ' +
        'so they are not written to local state. Streaming is gRPC-only and the gRPC receive ' +
        'path is unsupported — point the agent at an https:// sequencer URL and use pull().',
    );
    this.emit('stream-store-refused', {
      reason: 'stream_unverified',
      sequenceNumber: event.sequenceNumber,
      eventId: event.eventId,
      error,
    });

    // Say it once per streaming session so an operator sees it, without
    // emitting a line per event on a busy stream.
    if (!this._streamStoreRefusalWarned) {
      this._streamStoreRefusalWarned = true;
      console.warn(`[sync-engine] ${error.message}`);
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
   * Why this transport cannot be used to receive, or null if it can.
   *
   * The gRPC receive path stores nothing, and says so rather than quietly
   * pulling zero events forever. It is not a repairable-in-passing gap:
   * streamed events are refused (they carry no verifiable signature), the gRPC
   * `pull()` mapping throws on `envelope.eventId`, and the gRPC key directory
   * carries no `algorithm` and no directory signature, so it can neither be
   * cached nor trusted. Mapping those fields without the attestation guard in
   * `PeerKeyDirectory.refresh()` would make unattested keys the verification
   * anchor, which is worse than failing. Fixing the gRPC receive path is
   * separate, tracked work; until then REST is the supported receive path.
   *
   * Push over gRPC is unaffected.
   *
   * @private
   * @returns {string|null}
   */
  _unsupportedReceiveTransport() {
    if (this.getTransport() !== 'grpc') return null;
    return (
      'The gRPC receive path is unsupported: streamed events carry no verifiable signature, ' +
      'the gRPC envelope mapping omits fields the signing hash binds, and the gRPC key ' +
      'directory is never cryptographically attested. Use an https:// sequencer URL to pull. ' +
      'Push over gRPC is unaffected.'
    );
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
    const unsupported = this._unsupportedReceiveTransport();
    if (unsupported) {
      console.error(`[sync-engine] ${unsupported}`);
      return {
        success: false,
        pulled: 0,
        verified: 0,
        quarantined: 0,
        stored: 0,
        conflicts: null,
        error: unsupported,
      };
    }

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
          verified: 0,
          quarantined: 0,
          stored: 0,
          // Not computed on this branch: null, never a made-up 0. See PullResult.
          conflicts: null,
          sequenceNumbers: options.includeEvents ? [] : undefined,
        };
      }

      if (options.dryRun) {
        return {
          success: true,
          pulled: result.events.length,
          verified: 0,
          quarantined: 0,
          stored: 0,
          conflicts: null,
        };
      }

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

      // Verify every event against its author's signing key before it can be
      // read. Self-authored echoes are verified too: it costs nothing and keeps
      // a continuous check on our own signing path.
      const verifiedRecords = [];
      const quarantinedRecords = [];

      for (const event of result.events) {
        const envelope = event.envelope;
        const record = {
          sequenceNumber: event.sequenceNumber,
          eventId: envelope.eventId,
          commandId: envelope.commandId,
          tenantId: envelope.tenantId,
          storeId: envelope.storeId,
          entityType: envelope.entityType,
          entityId: envelope.entityId,
          eventType: envelope.eventType,
          payload: envelope.payload,
          // VES v1.0 fields
          vesVersion: envelope.vesVersion || 1,
          payloadKind: envelope.payloadKind || 0,
          payloadEncrypted: envelope.payloadEncrypted,
          payloadPlainHash: envelope.payloadPlainHash,
          payloadCipherHash: envelope.payloadCipherHash,
          agentKeyId: envelope.agentKeyId,
          agentSignature: envelope.agentSignature,
          agentSignatureScheme: envelope.agentSignatureScheme || 0,
          agentSignatureBundle: envelope.agentSignatureBundle || null,
          baseVersion: envelope.baseVersion,
          createdAt: envelope.createdAt,
          sequencedAt: event.sequencedAt,
          sourceAgent: envelope.sourceAgent,
        };

        // A throw out of key resolution would escape to pull()'s catch before
        // the cursor is updated, so it is caught here and fails closed.
        let resolution;
        try {
          resolution = await this.keyDirectory.resolve(
            envelope.sourceAgent,
            envelope.agentKeyId,
            envelope.createdAt,
          );
        } catch (error) {
          resolution = { error: 'key_unresolved', detail: error?.message };
        }

        if (resolution.error) {
          quarantinedRecords.push({
            record,
            reason: resolution.error,
            detail: resolution.detail,
          });
          continue;
        }

        // NOTE (deliberate, tracked): the receive path does NOT call
        // `assertEventMatchesSecurityProfile`, which the push path does call
        // (client.js `push`, grpc-client.js `pushEvents`). An event declaring
        // `agentSignatureScheme: 0` is therefore accepted by an agent
        // configured `hybrid` or `pqc-strict`, and `verifyEventSignature`
        // below falls back to the classical Ed25519 component. Enforcing the
        // profile here would quarantine every legacy peer on upgrade, so the
        // asymmetry stands until peers have migrated. No forgery becomes
        // possible in the meantime: the attacker still needs the peer's
        // Ed25519 private key. See docs/src/guides/sync.md.

        // Anything thrown here (malformed hex, a bad key bundle) is a failure
        // to verify, not a reason to accept the event.
        let valid = false;
        try {
          valid = this.client.verifyEventSignature(
            envelope,
            resolution.publicKeyBundle ?? resolution.publicKey,
          );
        } catch {
          valid = false;
        }

        if (valid) {
          verifiedRecords.push(record);
        } else {
          quarantinedRecords.push({ record, reason: 'signature_invalid' });
        }
      }

      const storedRecords = this._persistVerified(verifiedRecords);

      // A verified event the local schema refuses is dropped permanently: it
      // is not quarantined (it verified) and the cursor moves past it. The
      // per-record `receive-store-failed` events name the offenders, but they
      // need a listener; this does not.
      if (storedRecords.length !== verifiedRecords.length) {
        const dropped = verifiedRecords.length - storedRecords.length;
        this.emit('receive-store-dropped', {
          dropped,
          verified: verifiedRecords.length,
          stored: storedRecords.length,
        });
        console.warn(
          `[sync-engine] ${dropped} verified event(s) could not be stored and were DROPPED, ` +
            'not quarantined; the pull cursor has moved past them. ' +
            'Listen for "receive-store-failed" for the individual event ids.',
        );
      }

      // storeQuarantinedEvents takes one reason per call, so group first.
      // Re-quarantining an event_id overwrites the previous reason on purpose:
      // the stored reason is the current diagnosis, and the newest one is the
      // one an operator must act on (a key_unresolved that later becomes
      // signature_invalid is a forgery, not a directory outage).
      let quarantinedCount = 0;
      const reasons = new Set(quarantinedRecords.map((entry) => entry.reason));
      for (const reason of reasons) {
        const forReason = quarantinedRecords.filter((entry) => entry.reason === reason);
        const detail = forReason.find((entry) => entry.detail)?.detail;
        quarantinedCount += this._persistQuarantined(
          forReason.map((entry) => entry.record),
          reason,
        );
        this.emit('receive-verification-failed', { reason, count: forReason.length, detail });

        // A local misconfiguration and an untrustworthy directory are not
        // routine quarantine traffic: the first halts the entire receive path
        // on a stock deployment, the second is a live attack signal. Neither
        // may depend on someone having attached an event listener.
        if (reason === 'sequencer_key_not_configured' || reason === 'directory_untrusted') {
          console.warn(
            `[sync-engine] ${forReason.length} pulled event(s) quarantined as ${reason}: ${detail}`,
          );
        }
      }

      // The cursor advances regardless of what quarantined: one bad event from
      // one peer must not wedge this agent's sync forever.
      this.outbox.updateSyncState({
        lastPulledSequence: result.nextSequence,
        headSequence: result.headSequence,
        lastSyncAt: new Date(),
      });

      const conflicts = (await this.detectConflicts()).length;
      const summary = {
        pulled: result.events.length,
        verified: verifiedRecords.length,
        quarantined: quarantinedCount,
        stored: storedRecords.length,
        conflicts,
      };
      this.emit('pull', summary);

      return {
        success: true,
        ...summary,
        sequenceNumbers: options.includeEvents
          ? storedRecords.map((event) => event.sequenceNumber)
          : undefined,
      };
    } catch (error) {
      this.emit('error', error);
      return {
        success: false,
        pulled: 0,
        verified: 0,
        quarantined: 0,
        stored: 0,
        conflicts: null,
        error: error.message,
      };
    }
  }

  /**
   * Write verified records to the pulled-event store and clear any stale
   * quarantine row for the same event.
   *
   * Every envelope field here is attacker-controlled, and the pulled-event
   * table has NOT NULL columns, so a peer can sign an envelope the local schema
   * refuses. `storePulledEvents` runs one transaction, so a single such record
   * would otherwise roll back the whole batch AND throw past `updateSyncState`,
   * wedging the cursor on that sequence forever — the precise failure the
   * "one bad event must not wedge sync" property forbids. So: try the batch,
   * and on failure fall back to per-record writes so only the offender is lost.
   *
   * @private
   * @param {Array<Object>} records
   * @returns {Array<Object>} the records actually written
   */
  _persistVerified(records) {
    if (!records.length) return [];

    let stored;
    try {
      this.outbox.storePulledEvents(records);
      stored = records;
    } catch {
      stored = [];
      for (const record of records) {
        try {
          this.outbox.storePulledEvents([record]);
          stored.push(record);
        } catch (error) {
          this.emit('receive-store-failed', {
            eventId: record.eventId,
            sequenceNumber: record.sequenceNumber,
            error: error.message,
          });
        }
      }
    }

    // An event that now verifies supersedes any earlier quarantine row for the
    // same event_id — otherwise a re-pull leaves it readable AND quarantined,
    // and `sync doctor` reports a failure that has already been resolved.
    for (const record of stored) {
      try {
        this.outbox.deleteQuarantinedEvent(record.eventId);
      } catch (error) {
        this.emit('receive-store-failed', {
          eventId: record.eventId,
          sequenceNumber: record.sequenceNumber,
          error: error.message,
        });
      }
    }

    return stored;
  }

  /**
   * Write failed records to the quarantine table under one reason, with the
   * same per-record isolation and the same reason: the quarantine table has the
   * same NOT NULL columns, so a malformed envelope must not take the batch, or
   * the cursor, down with it.
   *
   * @private
   * @param {Array<Object>} records
   * @param {string} reason
   * @returns {number} how many records were actually written
   */
  _persistQuarantined(records, reason) {
    if (!records.length) return 0;

    try {
      this.outbox.storeQuarantinedEvents(records, reason);
      return records.length;
    } catch {
      let written = 0;
      for (const record of records) {
        try {
          this.outbox.storeQuarantinedEvents([record], reason);
          written += 1;
        } catch (error) {
          this.emit('receive-quarantine-failed', {
            eventId: record.eventId,
            sequenceNumber: record.sequenceNumber,
            reason,
            error: error.message,
          });
        }
      }
      return written;
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
      const key = await this.outbox.keyManager.getEncryptionKey(
        this.config.agentId,
        requestedKeyId,
      );
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
      createdAt: event.createdAt instanceof Date ? event.createdAt.toISOString() : event.createdAt,
      payloadPlainHash,
    });

    const wrapScheme = getPayloadWrapScheme(event.payloadEncrypted);
    const {
      key: encryptionKey,
      keyId: resolvedKeyId,
      candidateKeyIds,
    } = await this._resolveRecipientDecryptionKey(event.payloadEncrypted, keyId);

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
   * Get recent events from the in-memory stream buffer.
   *
   * These are UNVERIFIED notifications straight off the wire — no signature has
   * been checked. They are not store state; read store state through
   * `getPulledEvents()`, which only ever returns verified events.
   *
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

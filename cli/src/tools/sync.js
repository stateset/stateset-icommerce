/**
 * Sync Tools Module
 *
 * Tools for syncing local database with remote sequencer.
 * Requires sync module imports (config, outbox, engine, client).
 */

import { z } from 'zod';
import { loadSyncConfig, SyncConfig, isSyncConfigured } from '../sync/config.js';
import { createOutbox } from '../sync/outbox.js';
import { createSyncEngine } from '../sync/engine.js';
import { createSequencerClient } from '../sync/client.js';
import { getPayloadWrapScheme } from '../sync/pqc.js';

function getDefaultSecurityProfile() {
  if (!isSyncConfigured()) {
    return undefined;
  }

  try {
    const rawConfig = loadSyncConfig();
    const config = new SyncConfig(rawConfig);
    return config.securityProfile;
  } catch {
    return undefined;
  }
}

function getAgentKeyManagerOptions(securityProfile) {
  const resolvedSecurityProfile = securityProfile || getDefaultSecurityProfile();
  return resolvedSecurityProfile ? { securityProfile: resolvedSecurityProfile } : {};
}

function createConfiguredOutbox(db) {
  return createOutbox(db, getAgentKeyManagerOptions());
}

function formatPulledEvent(event, { includePayloads = false } = {}) {
  const encrypted = Number(event.payloadKind ?? 0) === 1 && !!event.payloadEncrypted;

  const formatted = {
    source: event.source ?? 'pulled',
    sequenceNumber: event.sequenceNumber,
    eventId: event.eventId,
    entityType: event.entityType,
    entityId: event.entityId,
    eventType: event.eventType,
    sourceAgent: event.sourceAgent,
    createdAt: event.createdAt,
    sequencedAt: event.sequencedAt,
    payloadKind: Number(event.payloadKind ?? 0),
    encrypted,
    wrapScheme: encrypted ? getPayloadWrapScheme(event.payloadEncrypted) : null,
  };

  if (includePayloads && !encrypted) {
    formatted.payload = event.payload;
  }

  return formatted;
}

async function collectFormattedPulledEvents(
  engine,
  events,
  { includePayloads = false, decryptPayloads = false, keyId } = {},
) {
  const includePlaintextPayloads = includePayloads || decryptPayloads;
  const formattedEvents = [];

  for (const event of events) {
    const formatted = formatPulledEvent(event, {
      includePayloads: includePlaintextPayloads,
    });

    if (formatted.encrypted && decryptPayloads) {
      try {
        const decrypted = await engine.decryptStoredEvent({
          sequenceNumber: event.sequenceNumber,
          source: 'pulled',
          keyId,
        });

        formatted.payload = decrypted.payload;
        formatted.encryptionProfile = decrypted.encryptionProfile;
        formatted.wrapScheme = decrypted.wrapScheme;
        formatted.recipientKeyId = decrypted.recipientKeyId;
        formatted.recipientKeyCandidates = decrypted.recipientKeyCandidates;
      } catch (error) {
        formatted.decryptionError = error.message;
      }
    }

    formattedEvents.push(formatted);
  }

  return {
    includePayloads: includePlaintextPayloads,
    events: formattedEvents,
  };
}

export const syncTools = [
  {
    name: 'sync_status',
    description:
      'Get the current sync status between local database and remote sequencer. Shows pending events, sync lag, and connection status.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      if (!isSyncConfigured())
        return {
          configured: false,
          message: 'Sync not configured. Run "stateset-sync init" to set up sync.',
          hint: 'stateset-sync init --sequencer-url <url> --tenant-id <uuid> --store-id <uuid>',
        };
      const rawConfig = loadSyncConfig();
      const config = new SyncConfig(rawConfig);
      const outbox = createConfiguredOutbox(commerce.db);
      const stats = outbox.getStats();
      const syncState = outbox.getSyncState();
      let remoteHead = syncState.headSequence;
      let connected = false;
      let connectionError = null;
      try {
        const client = createSequencerClient(config);
        await client.connect();
        const remoteState = await client.getHead();
        remoteHead = remoteState.headSequence;
        connected = true;
      } catch (error) {
        connectionError = error.message;
      }
      const lag = remoteHead - syncState.lastPulledSequence;
      return {
        configured: true,
        connected,
        connectionError,
        sequencer: config.sequencerUrl,
        identity: { tenantId: config.tenantId, storeId: config.storeId, agentId: config.agentId },
        localState: {
          lastPushedSequence: syncState.lastPushedSequence,
          lastPulledSequence: syncState.lastPulledSequence,
          lastSyncAt: syncState.lastSyncAt,
        },
        remoteHead,
        lag,
        outbox: {
          total: stats.total,
          pending: stats.pending,
          synced: stats.synced,
          failed: stats.failed,
          rejected: stats.rejected,
          oldestPending: stats.oldestPending,
          lastSynced: stats.lastSynced,
        },
        health: lag > 100 ? 'degraded' : connected ? 'healthy' : 'offline',
      };
    },
  },
  {
    name: 'sync_push',
    description:
      'Push pending local events to the remote sequencer. Requires --apply flag for actual push.',
    inputSchema: {
      batchSize: z
        .number()
        .int()
        .positive()
        .max(1000)
        .optional()
        .describe('Maximum events to push in one batch (default: 100)'),
      dryRun: z.boolean().optional().describe('Show what would be pushed without actually pushing'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { batchSize = 100, dryRun = false } = params;
      if (!isSyncConfigured())
        return {
          success: false,
          error: 'Sync not configured',
          hint: 'Run "stateset-sync init" to set up sync.',
        };
      if (!dryRun && !allowApply) {
        const outbox = createConfiguredOutbox(commerce.db);
        const pending = outbox.getPending(batchSize);
        return {
          success: false,
          error: 'Push operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable push, or use dryRun: true to preview.',
          wouldPush: pending.length,
          pendingEvents: pending.map((e) => ({
            eventId: e.eventId,
            eventType: e.eventType,
            entityType: e.entityType,
            entityId: e.entityId,
            createdAt: e.createdAt,
          })),
        };
      }
      const rawConfig = loadSyncConfig();
      const config = new SyncConfig(rawConfig);
      const engine = createSyncEngine({ db: commerce.db, config });
      await engine.initialize();
      const result = await engine.push({ batchSize, dryRun });
      await engine.shutdown();
      if (dryRun)
        return {
          dryRun: true,
          wouldPush: result.pushed,
          message: `Would push ${result.pushed} events to sequencer`,
        };
      return {
        success: result.success,
        pushed: result.pushed,
        rejected: result.rejected,
        receipt: result.receipt
          ? {
              batchId: result.receipt.batchId,
              sequenceStart: result.receipt.sequenceStart,
              sequenceEnd: result.receipt.sequenceEnd,
            }
          : null,
        error: result.error,
      };
    },
  },
  {
    name: 'sync_pull',
    description: 'Pull events from the remote sequencer and store them locally.',
    inputSchema: {
      fromSequence: z
        .number()
        .int()
        .min(0)
        .optional()
        .describe('Start pulling from this sequence number'),
      limit: z
        .number()
        .int()
        .positive()
        .max(10000)
        .optional()
        .describe('Maximum events to pull (default: 1000)'),
      includeEvents: z
        .boolean()
        .optional()
        .describe('Include the stored pulled events in the response'),
      includePayloads: z
        .boolean()
        .optional()
        .describe('When including events, include plaintext payloads for non-encrypted events'),
      decryptPayloads: z
        .boolean()
        .optional()
        .describe('When including events, attempt to decrypt encrypted pulled events'),
      keyId: z
        .number()
        .int()
        .positive()
        .optional()
        .describe('Optional local encryption key ID override for decrypting pulled events'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const {
        fromSequence,
        limit = 1000,
        includeEvents: requestedIncludeEvents = false,
        includePayloads = false,
        decryptPayloads = false,
        keyId,
      } = params;
      if (!isSyncConfigured())
        return {
          success: false,
          error: 'Sync not configured',
          hint: 'Run "stateset-sync init" to set up sync.',
        };
      const rawConfig = loadSyncConfig();
      const config = new SyncConfig(rawConfig);
      const engine = createSyncEngine({ db: commerce.db, config });
      await engine.initialize();
      const includeEvents = requestedIncludeEvents || includePayloads || decryptPayloads;
      const result = await engine.pull({ fromSequence, limit, includeEvents });
      await engine.shutdown();

      let events;
      let resolvedIncludePayloads = includePayloads || decryptPayloads;

      if (includeEvents && result.success) {
        const storedEvents = (result.sequenceNumbers || [])
          .map((sequenceNumber) => engine.getStoredEvent({ sequenceNumber, source: 'pulled' }))
          .filter(Boolean);
        const formatted = await collectFormattedPulledEvents(engine, storedEvents, {
          includePayloads,
          decryptPayloads,
          keyId,
        });
        events = formatted.events;
        resolvedIncludePayloads = formatted.includePayloads;
      }

      return {
        success: result.success,
        pulled: result.pulled,
        applied: result.applied,
        conflicts: result.conflicts,
        error: result.error,
        includeEvents,
        includePayloads: includeEvents ? resolvedIncludePayloads : false,
        decryptPayloads,
        events,
      };
    },
  },
  {
    name: 'sync_outbox',
    description:
      'List events in the local outbox. Shows pending, synced, failed, and rejected events.',
    inputSchema: {
      status: z
        .enum(['pending', 'synced', 'failed', 'rejected', 'all'])
        .optional()
        .describe('Filter by status (default: all)'),
      limit: z
        .number()
        .int()
        .positive()
        .max(500)
        .optional()
        .describe('Maximum events to return (default: 20)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { status = 'all', limit = 20 } = params;
      if (!isSyncConfigured())
        return {
          success: false,
          error: 'Sync not configured',
          hint: 'Run "stateset-sync init" to set up sync.',
        };
      const outbox = createConfiguredOutbox(commerce.db);
      outbox.initialize();
      const stmt =
        status === 'all'
          ? commerce.db.prepare('SELECT * FROM _ves_outbox ORDER BY local_seq DESC LIMIT ?')
          : commerce.db.prepare(
              'SELECT * FROM _ves_outbox WHERE sync_status = ? ORDER BY local_seq DESC LIMIT ?',
            );
      const rows = status === 'all' ? stmt.all(limit) : stmt.all(status, limit);
      const events = rows.map((row) => ({
        localSeq: row.local_seq,
        eventId: row.event_id,
        eventType: row.event_type,
        entityType: row.entity_type,
        entityId: row.entity_id,
        syncStatus: row.sync_status,
        remoteSequence: row.remote_sequence,
        createdAt: row.created_at,
        syncedAt: row.synced_at,
        rejectionReason: row.rejection_reason,
        retryCount: row.retry_count,
      }));
      return { count: events.length, filter: status, events };
    },
  },
  {
    name: 'sync_pulled_events',
    description:
      'List events already pulled from the sequencer and stored locally. Can optionally include plaintext payloads or decrypt encrypted payloads with local keys.',
    inputSchema: {
      limit: z
        .number()
        .int()
        .positive()
        .max(500)
        .optional()
        .describe('Maximum pulled events to return (default: 20)'),
      includePayloads: z
        .boolean()
        .optional()
        .describe('Include plaintext payloads for locally stored non-encrypted events'),
      decryptPayloads: z
        .boolean()
        .optional()
        .describe('Attempt to decrypt encrypted pulled events with local recipient keys'),
      keyId: z
        .number()
        .int()
        .positive()
        .optional()
        .describe('Optional local encryption key ID override for decrypting encrypted events'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { limit = 20, includePayloads = false, decryptPayloads = false, keyId } = params;
      if (!isSyncConfigured())
        return {
          success: false,
          error: 'Sync not configured',
          hint: 'Run "stateset-sync init" to set up sync.',
        };

      try {
        const rawConfig = loadSyncConfig();
        const config = new SyncConfig(rawConfig);
        const engine = createSyncEngine({ db: commerce.db, config });
        const events = engine.getPulledEvents(limit);
        const formatted = await collectFormattedPulledEvents(engine, events, {
          includePayloads,
          decryptPayloads,
          keyId,
        });

        return {
          success: true,
          source: 'pulled',
          count: formatted.events.length,
          limit,
          includePayloads: formatted.includePayloads,
          decryptPayloads,
          events: formatted.events,
        };
      } catch (error) {
        return {
          success: false,
          error: error.message,
          source: 'pulled',
        };
      }
    },
  },
  {
    name: 'sync_decrypt_event',
    description:
      'Decrypt an encrypted sync event from the local outbox or pulled-event store using the local recipient key. Supports legacy X25519 and hybrid X25519 + ML-KEM-768 wraps.',
    inputSchema: {
      eventId: z.string().min(1).optional().describe('Event ID to inspect and decrypt'),
      sequenceNumber: z
        .number()
        .int()
        .positive()
        .optional()
        .describe('Pulled-event sequence number to inspect and decrypt'),
      source: z
        .enum(['auto', 'outbox', 'pulled'])
        .optional()
        .describe('Where to look for the event (default: auto)'),
      keyId: z
        .number()
        .int()
        .positive()
        .optional()
        .describe('Optional local encryption key ID override for decryption'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { eventId, sequenceNumber, source = 'auto', keyId } = params;
      if (!isSyncConfigured())
        return {
          success: false,
          error: 'Sync not configured',
          hint: 'Run "stateset-sync init" to set up sync.',
        };
      if (!eventId && sequenceNumber === undefined) {
        return {
          success: false,
          error: 'eventId or sequenceNumber is required',
        };
      }
      if (sequenceNumber !== undefined && source === 'outbox') {
        return {
          success: false,
          error: 'sequenceNumber can only be used with pulled or auto source',
        };
      }

      try {
        const rawConfig = loadSyncConfig();
        const config = new SyncConfig(rawConfig);
        const engine = createSyncEngine({ db: commerce.db, config });
        const decrypted = await engine.decryptStoredEvent({
          eventId,
          sequenceNumber,
          source,
          keyId,
        });

        return {
          success: true,
          ...decrypted,
        };
      } catch (error) {
        return {
          success: false,
          error: error.message,
          eventId: eventId ?? null,
          sequenceNumber: sequenceNumber ?? null,
          source,
        };
      }
    },
  },
  {
    name: 'sync_retry_failed',
    description:
      'Reset failed events to pending status so they can be retried. Requires --apply flag.',
    inputSchema: {},
    permission: 'write',
    handler: async ({ commerce, params: _params, allowApply }) => {
      if (!isSyncConfigured())
        return {
          success: false,
          error: 'Sync not configured',
          hint: 'Run "stateset-sync init" to set up sync.',
        };
      if (!allowApply) {
        const outbox = createConfiguredOutbox(commerce.db);
        const stats = outbox.getStats();
        return {
          success: false,
          error: 'Retry operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable retry.',
          failedCount: stats.failed,
        };
      }
      const outbox = createConfiguredOutbox(commerce.db);
      const retriedCount = outbox.retryFailed();
      return {
        success: true,
        retriedCount,
        message: `Reset ${retriedCount} failed events to pending`,
      };
    },
  },
  {
    name: 'sync_entity_history',
    description:
      'Get the event history for a specific entity from the remote sequencer or the local pulled-event store.',
    inputSchema: {
      entityType: z
        .string()
        .min(1)
        .describe('Entity type (order, customer, product, inventory, return, cart)'),
      entityId: z.string().min(1).describe('Entity ID'),
      source: z
        .enum(['remote', 'local'])
        .optional()
        .describe('Read history from the remote sequencer or from locally stored pulled events'),
      limit: z
        .number()
        .int()
        .positive()
        .max(1000)
        .optional()
        .describe('Maximum local events to return (default: 100)'),
      includePayloads: z
        .boolean()
        .optional()
        .describe('When reading local history, include plaintext payloads for non-encrypted events'),
      decryptPayloads: z
        .boolean()
        .optional()
        .describe('When reading local history, attempt to decrypt encrypted pulled events'),
      keyId: z
        .number()
        .int()
        .positive()
        .optional()
        .describe('Optional local encryption key ID override for decrypting local history events'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const {
        entityType,
        entityId,
        source = 'remote',
        limit = 100,
        includePayloads = false,
        decryptPayloads = false,
        keyId,
      } = params;
      if (!isSyncConfigured())
        return {
          success: false,
          error: 'Sync not configured',
          hint: 'Run "stateset-sync init" to set up sync.',
        };
      const rawConfig = loadSyncConfig();
      const config = new SyncConfig(rawConfig);

      if (source === 'local') {
        try {
          const engine = createSyncEngine({ db: commerce.db, config });
          const events = engine.getPulledEventsForEntity(entityType, entityId, limit);
          const formatted = await collectFormattedPulledEvents(engine, events, {
            includePayloads,
            decryptPayloads,
            keyId,
          });

          return {
            entityType,
            entityId,
            source: 'local',
            limit,
            includePayloads: formatted.includePayloads,
            decryptPayloads,
            eventCount: formatted.events.length,
            events: formatted.events,
          };
        } catch (error) {
          return {
            success: false,
            entityType,
            entityId,
            source: 'local',
            error: error.message,
          };
        }
      }

      const client = createSequencerClient(config);
      await client.connect();
      const events = await client.getEntityHistory(entityType, entityId);
      return {
        entityType,
        entityId,
        source: 'remote',
        eventCount: events.length,
        events: events.map((e) => ({
          sequenceNumber: e.sequenceNumber,
          eventId: e.envelope.eventId,
          eventType: e.envelope.eventType,
          createdAt: e.envelope.createdAt,
          sequencedAt: e.sequencedAt,
          sourceAgent: e.envelope.sourceAgent,
        })),
      };
    },
  },
  {
    name: 'sync_full',
    description:
      'Perform a full sync: push pending events then pull new events. Requires --apply flag for push.',
    inputSchema: {
      pushBatchSize: z
        .number()
        .int()
        .positive()
        .max(1000)
        .optional()
        .describe('Maximum events to push (default: 100)'),
      pullLimit: z
        .number()
        .int()
        .positive()
        .max(10000)
        .optional()
        .describe('Maximum events to pull (default: 1000)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { pushBatchSize = 100, pullLimit = 1000 } = params;
      if (!isSyncConfigured())
        return {
          success: false,
          error: 'Sync not configured',
          hint: 'Run "stateset-sync init" to set up sync.',
        };
      const rawConfig = loadSyncConfig();
      const config = new SyncConfig(rawConfig);
      const engine = createSyncEngine({ db: commerce.db, config });
      await engine.initialize();
      let pushResult = { success: true, pushed: 0, rejected: 0 };
      if (allowApply) {
        pushResult = await engine.push({ batchSize: pushBatchSize });
      } else {
        const outbox = createConfiguredOutbox(commerce.db);
        pushResult = {
          success: false,
          pushed: 0,
          rejected: 0,
          skipped: true,
          pendingCount: outbox.getPendingCount(),
          message: 'Push skipped: --apply flag not set',
        };
      }
      const pullResult = await engine.pull({ limit: pullLimit });
      await engine.shutdown();
      return { push: pushResult, pull: pullResult };
    },
  },
  {
    name: 'sync_conflicts',
    description:
      'List unresolved sync conflicts. Conflicts occur when local and remote events modify the same entity concurrently.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      if (!isSyncConfigured())
        return {
          success: false,
          error: 'Sync not configured',
          hint: 'Run "stateset-sync init" to set up sync.',
        };
      const rawConfig = loadSyncConfig();
      const config = new SyncConfig(rawConfig);
      const engine = createSyncEngine({ db: commerce.db, config });
      await engine.initialize();
      const conflicts = await engine.getConflicts();
      await engine.shutdown();
      return {
        count: conflicts.length,
        conflicts: conflicts.map((c) => ({
          id: c.id,
          type: c.type,
          entityType: c.entityType,
          entityId: c.entityId,
          description: c.description,
          suggestedStrategy: c.suggestedStrategy,
          detectedAt: c.detectedAt,
          localEvent: c.localEvent
            ? {
                localSeq: c.localEvent.localSeq,
                eventType: c.localEvent.eventType,
                createdAt: c.localEvent.createdAt,
              }
            : null,
        })),
      };
    },
  },
  {
    name: 'sync_resolve',
    description:
      'Resolve a specific sync conflict using a resolution strategy. Requires --apply flag.',
    inputSchema: {
      conflictId: z.string().min(1).describe('The conflict ID to resolve'),
      strategy: z
        .enum(['remote-wins', 'local-wins', 'merge'])
        .optional()
        .describe('Resolution strategy (default: uses suggested strategy)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { conflictId, strategy } = params;
      if (!isSyncConfigured())
        return {
          success: false,
          error: 'Sync not configured',
          hint: 'Run "stateset-sync init" to set up sync.',
        };
      if (!allowApply)
        return {
          success: false,
          error: 'Resolve operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable conflict resolution.',
          conflictId,
          wouldUseStrategy: strategy || 'suggested',
        };
      const rawConfig = loadSyncConfig();
      const config = new SyncConfig(rawConfig);
      const engine = createSyncEngine({ db: commerce.db, config });
      await engine.initialize();
      const result = await engine.resolveConflict(conflictId, strategy);
      await engine.shutdown();
      return {
        success: result.success,
        conflictId: result.conflictId,
        strategy: result.strategy,
        result: result.result,
        error: result.error,
      };
    },
  },
  {
    name: 'sync_rebase',
    description: 'Resolve all sync conflicts using a resolution strategy. Requires --apply flag.',
    inputSchema: {
      strategy: z
        .enum(['remote-wins', 'local-wins', 'merge'])
        .optional()
        .describe('Resolution strategy for all conflicts (default: remote-wins)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { strategy = 'remote-wins' } = params;
      if (!isSyncConfigured())
        return {
          success: false,
          error: 'Sync not configured',
          hint: 'Run "stateset-sync init" to set up sync.',
        };
      const rawConfig = loadSyncConfig();
      const config = new SyncConfig(rawConfig);
      const engine = createSyncEngine({ db: commerce.db, config });
      await engine.initialize();
      const conflicts = await engine.getConflicts();
      if (!allowApply) {
        await engine.shutdown();
        return {
          success: false,
          error: 'Rebase operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable rebase.',
          wouldResolve: conflicts.length,
          conflicts: conflicts.map((c) => ({
            id: c.id,
            entityType: c.entityType,
            entityId: c.entityId,
            type: c.type,
          })),
          strategy,
        };
      }
      const result = await engine.rebase({ strategy });
      await engine.shutdown();
      return {
        success: result.success,
        resolved: result.rebased,
        failed: result.failed,
        strategy,
        errors: result.errors,
      };
    },
  },

  // ===========================================================================
  // VES Receipt Verification Tools
  // ===========================================================================

  {
    name: 'sync_verify_receipt',
    description:
      'Verify the signature on a VES event receipt. Supports legacy Ed25519 receipts and hybrid Ed25519 + ML-DSA-65 bundles.',
    inputSchema: {
      envelope: z
        .object({
          eventId: z.string().min(1),
          tenantId: z.string().min(1),
          storeId: z.string().min(1),
          sourceAgent: z.string().min(1),
          agentKeyId: z.number().int(),
          entityType: z.string().min(1),
          entityId: z.string().min(1),
          eventType: z.string().min(1),
          createdAt: z.string().min(1),
          payloadPlainHash: z.string().min(1),
          payloadCipherHash: z.string().min(1),
          agentSignature: z.string().min(1).optional(),
          agentSignatureScheme: z.number().int().optional(),
          agentSignatureBundle: z
            .object({
              ed25519Signature: z.string().optional(),
              mlDsa65Signature: z.string().optional(),
            })
            .optional(),
          vesVersion: z.number().int().positive(),
        })
        .describe('VES event envelope with signature fields'),
      publicKeyHex: z.string().min(1).optional().describe("Agent's Ed25519 public key, hex encoded"),
      publicKeyBundle: z
        .object({
          ed25519PublicKey: z.string().optional(),
          mlDsa65PublicKey: z.string().optional(),
        })
        .optional()
        .describe('Optional hybrid public-key bundle, hex encoded'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const {
        computeEventSigningHash,
        verifyEventSignature,
        verifyEventSignatureHybrid,
        hexToBuffer,
      } = await import('../sync/crypto.js');
      const { envelope, publicKeyHex, publicKeyBundle } = params;

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

      let valid = false;
      if (
        publicKeyBundle?.ed25519PublicKey &&
        publicKeyBundle?.mlDsa65PublicKey &&
        envelope.agentSignatureBundle?.ed25519Signature &&
        envelope.agentSignatureBundle?.mlDsa65Signature
      ) {
        try {
          valid = verifyEventSignatureHybrid(signingHash, envelope.agentSignatureBundle, publicKeyBundle);
        } catch {
          valid = false;
        }
      } else if (envelope.agentSignature && publicKeyHex) {
        valid = verifyEventSignature(
          signingHash,
          hexToBuffer(envelope.agentSignature),
          hexToBuffer(publicKeyHex),
        );
      }

      return {
        valid,
        eventId: envelope.eventId,
        sourceAgent: envelope.sourceAgent,
        entityType: envelope.entityType,
        entityId: envelope.entityId,
      };
    },
  },
  {
    name: 'sync_verify_inclusion',
    description:
      'Verify a Merkle inclusion proof for a VES event. Proves the event is included in a committed batch.',
    inputSchema: {
      envelope: z
        .object({
          eventId: z.string().min(1),
          payloadPlainHash: z.string().min(1),
          agentSignature: z.string().min(1),
        })
        .describe('Partial VES event envelope (eventId, payloadPlainHash, agentSignature)'),
      proof: z
        .object({
          leafIndex: z.number().int().min(0),
          proofHashes: z.array(z.string()),
        })
        .describe('Merkle inclusion proof with leaf index and sibling hashes'),
      expectedRoot: z.string().min(1).describe('Hex-encoded Merkle root from the commitment'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const { computeNodeHash, hexToBuffer } = await import('../sync/crypto.js');
      const { envelope, proof, expectedRoot } = params;

      // The leaf hash is H(payloadPlainHash || agentSignature) — simplified for verification
      const crypto = await import('crypto');
      const leafHash = crypto
        .createHash('sha256')
        .update(hexToBuffer(envelope.payloadPlainHash))
        .update(hexToBuffer(envelope.agentSignature))
        .digest();

      // Walk up the proof tree
      let currentHash = leafHash;
      let index = proof.leafIndex;
      for (const siblingHex of proof.proofHashes) {
        const sibling = hexToBuffer(siblingHex);
        if (index % 2 === 0) {
          currentHash = computeNodeHash(currentHash, sibling);
        } else {
          currentHash = computeNodeHash(sibling, currentHash);
        }
        index = Math.floor(index / 2);
      }

      const expectedRootBuf = hexToBuffer(expectedRoot);
      const valid = currentHash.equals(expectedRootBuf);

      return {
        valid,
        eventId: envelope.eventId,
        expectedRoot,
      };
    },
  },
  {
    name: 'sync_inspect_commitment',
    description:
      'Inspect a VES batch commitment from the sequencer. Shows the Merkle root, sequence range, and event count.',
    inputSchema: {
      batchId: z.string().min(1).describe('Batch commitment ID'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const { batchId } = params;
      if (!isSyncConfigured())
        return {
          success: false,
          error: 'Sync not configured',
          hint: 'Run "stateset-sync init" to set up sync.',
        };
      try {
        const rawConfig = loadSyncConfig();
        const config = new SyncConfig(rawConfig);
        const client = createSequencerClient(config);
        const commitment = await client.getCommitment(batchId);
        if (!commitment) {
          return { success: false, error: `Commitment '${batchId}' not found` };
        }
        return {
          success: true,
          batchId: commitment.batchId,
          merkleRoot: commitment.merkleRoot,
          startSequence: commitment.startSequence,
          endSequence: commitment.endSequence,
          eventCount: commitment.eventCount,
          committedAt: commitment.committedAt,
        };
      } catch (error) {
        return { success: false, error: error.message };
      }
    },
  },

  // ===========================================================================
  // Agent Key Management Tools
  // ===========================================================================

  {
    name: 'agent_key_generate',
    description:
      'Generate a new Ed25519 signing or X25519 encryption key pair for an agent. Requires --apply flag.',
    inputSchema: {
      agentId: z.string().min(1).describe('Agent identifier'),
      keyType: z.enum(['signing', 'encryption']).describe('Key type to generate'),
      securityProfile: z
        .enum(['legacy', 'hybrid', 'pqc-strict'])
        .optional()
        .describe('Optional PQ migration profile override'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      const { agentId, keyType, securityProfile } = params;
      if (!allowApply) {
        return {
          success: false,
          error: 'Key generation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable key generation.',
        };
      }
      try {
        const { AgentKeyManager } = await import('../sync/keys.js');
        const { bufferToHex } = await import('../sync/crypto.js');
        const keyManager = new AgentKeyManager(
          '.stateset',
          getAgentKeyManagerOptions(securityProfile),
        );
        const keyPair =
          keyType === 'signing'
            ? await keyManager.generateSigningKey(agentId)
            : await keyManager.generateEncryptionKey(agentId);
        return {
          success: true,
          agentId,
          keyType,
          keyId: keyPair.keyId,
          keyAlgorithm: keyPair.keyAlgorithm,
          securityProfile: keyPair.securityProfile,
          publicKeyHex: bufferToHex(keyPair.publicKey),
          createdAt: keyPair.createdAt,
        };
      } catch (error) {
        return { success: false, error: error.message };
      }
    },
  },
  {
    name: 'agent_key_list',
    description:
      'List signing and/or encryption keys for an agent. Returns only public metadata — never exposes private keys.',
    inputSchema: {
      agentId: z.string().min(1).describe('Agent identifier'),
      keyType: z.enum(['signing', 'encryption']).optional().describe('Filter by key type'),
      securityProfile: z
        .enum(['legacy', 'hybrid', 'pqc-strict'])
        .optional()
        .describe('Optional PQ migration profile override'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const { agentId, keyType, securityProfile } = params;
      try {
        const { AgentKeyManager } = await import('../sync/keys.js');
        const { bufferToHex } = await import('../sync/crypto.js');
        const keyManager = new AgentKeyManager(
          '.stateset',
          getAgentKeyManagerOptions(securityProfile),
        );
        const results = [];
        const types = keyType ? [keyType] : ['signing', 'encryption'];
        for (const kt of types) {
          const keys =
            kt === 'signing'
              ? await keyManager.listSigningKeys(agentId)
              : await keyManager.listEncryptionKeys(agentId);
          for (const k of keys) {
            results.push({
              keyId: k.keyId,
              keyType: kt,
              keyAlgorithm: k.keyAlgorithm,
              securityProfile: k.securityProfile,
              publicKeyHex: bufferToHex(k.publicKey),
              createdAt: k.createdAt,
              revokedAt: k.revokedAt || null,
            });
          }
        }
        return { success: true, agentId, count: results.length, keys: results };
      } catch (error) {
        return { success: false, error: error.message };
      }
    },
  },
  {
    name: 'agent_key_info',
    description:
      'Get detailed info for a specific agent key. Returns metadata only — no private key.',
    inputSchema: {
      agentId: z.string().min(1).describe('Agent identifier'),
      keyType: z.enum(['signing', 'encryption']).describe('Key type'),
      keyId: z.number().int().positive().describe('Key ID'),
      securityProfile: z
        .enum(['legacy', 'hybrid', 'pqc-strict'])
        .optional()
        .describe('Optional PQ migration profile override'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const { agentId, keyType, keyId, securityProfile } = params;
      try {
        const { AgentKeyManager } = await import('../sync/keys.js');
        const { bufferToHex } = await import('../sync/crypto.js');
        const keyManager = new AgentKeyManager(
          '.stateset',
          getAgentKeyManagerOptions(securityProfile),
        );
        const key =
          keyType === 'signing'
            ? await keyManager.getSigningKey(agentId, keyId)
            : await keyManager.getEncryptionKey(agentId, keyId);
        if (!key) {
          return { success: false, error: `Key ${keyId} not found for agent '${agentId}'` };
        }
        return {
          success: true,
          agentId,
          keyType,
          keyId: key.keyId,
          keyAlgorithm: key.keyAlgorithm,
          securityProfile: key.securityProfile,
          publicKeyHex: bufferToHex(key.publicKey),
          createdAt: key.createdAt,
          revokedAt: key.revokedAt || null,
        };
      } catch (error) {
        return { success: false, error: error.message };
      }
    },
  },
  {
    name: 'agent_key_rotate',
    description:
      'Rotate an agent key: generate a new key and revoke the current one. Requires --apply flag.',
    inputSchema: {
      agentId: z.string().min(1).describe('Agent identifier'),
      keyType: z.enum(['signing', 'encryption']).describe('Key type to rotate'),
      securityProfile: z
        .enum(['legacy', 'hybrid', 'pqc-strict'])
        .optional()
        .describe('Optional PQ migration profile override'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      const { agentId, keyType, securityProfile } = params;
      if (!allowApply) {
        return {
          success: false,
          error: 'Key rotation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable key rotation.',
        };
      }
      try {
        const { AgentKeyManager } = await import('../sync/keys.js');
        const { bufferToHex } = await import('../sync/crypto.js');
        const keyManager = new AgentKeyManager(
          '.stateset',
          getAgentKeyManagerOptions(securityProfile),
        );

        // Get the current key before rotation
        const currentKey =
          keyType === 'signing'
            ? await keyManager.getCurrentSigningKey(agentId)
            : await keyManager.getCurrentEncryptionKey(agentId);
        if (!currentKey) {
          return {
            success: false,
            error: `No current ${keyType} key found for agent '${agentId}'`,
          };
        }

        // Generate new key
        const newKey =
          keyType === 'signing'
            ? await keyManager.generateSigningKey(agentId)
            : await keyManager.generateEncryptionKey(agentId);

        // Revoke old key
        if (keyType === 'signing') {
          await keyManager.revokeSigningKey(agentId, currentKey.keyId);
        } else {
          await keyManager.revokeEncryptionKey(agentId, currentKey.keyId);
        }

        return {
          success: true,
          agentId,
          keyType,
          oldKeyId: currentKey.keyId,
          newKeyId: newKey.keyId,
          keyAlgorithm: newKey.keyAlgorithm,
          securityProfile: newKey.securityProfile,
          newPublicKeyHex: bufferToHex(newKey.publicKey),
          createdAt: newKey.createdAt,
        };
      } catch (error) {
        return { success: false, error: error.message };
      }
    },
  },
  {
    name: 'agent_key_export',
    description:
      'Export an agent public key for sequencer registration. Returns public key only — never the private key.',
    inputSchema: {
      agentId: z.string().min(1).describe('Agent identifier'),
      keyType: z
        .enum(['signing', 'encryption'])
        .optional()
        .default('signing')
        .describe('Key type (default: signing)'),
      keyId: z
        .number()
        .int()
        .positive()
        .optional()
        .describe('Specific key ID (default: current active key)'),
      securityProfile: z
        .enum(['legacy', 'hybrid', 'pqc-strict'])
        .optional()
        .describe('Optional PQ migration profile override'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const { agentId, keyType, keyId, securityProfile } = params;
      try {
        const { AgentKeyManager } = await import('../sync/keys.js');
        const keyManager = new AgentKeyManager(
          '.stateset',
          getAgentKeyManagerOptions(securityProfile),
        );
        const exported =
          keyType === 'signing'
            ? await keyManager.exportSigningPublicKey(agentId, keyId || null)
            : await keyManager.exportEncryptionPublicKey(agentId, keyId || null);
        return {
          success: true,
          agentId,
          keyType,
          keyId: exported.keyId,
          keyAlgorithm: exported.keyAlgorithm,
          securityProfile: exported.securityProfile,
          publicKeyHex: exported.publicKey,
          publicKeyBundle: exported.publicKeyBundle || null,
          createdAt: exported.createdAt,
        };
      } catch (error) {
        return { success: false, error: error.message };
      }
    },
  },
];

export default syncTools;

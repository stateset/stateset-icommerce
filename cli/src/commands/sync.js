/**
 * Sync Commands Module
 */

import { loadSyncConfig, SyncConfig, isSyncConfigured } from '../sync/config.js';
import { createOutbox } from '../sync/outbox.js';
import { createSyncEngine } from '../sync/engine.js';
import { createSequencerClient } from '../sync/client.js';
import { getPayloadWrapScheme } from '../sync/pqc.js';

function parseJsonArg(value, label) {
  try {
    return JSON.parse(value);
  } catch (error) {
    throw new Error(`Invalid ${label} JSON: ${error.message}`);
  }
}

function parseOptionalInt(value, usage) {
  if (value === undefined) return undefined;
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed < 0) throw new Error(usage);
  return parsed;
}

function getDefaultSecurityProfile() {
  if (!isSyncConfigured()) return undefined;
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

function ensureConfigured() {
  if (!isSyncConfigured()) {
    throw new Error('Sync not configured');
  }
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

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'status': {
      ensureConfigured();
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
      const result = {
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
        lag: remoteHead - syncState.lastPulledSequence,
        outbox: stats,
      };
      return jsonOutput
        ? result
        : {
            result,
            formatted: `Sync status: ${connected ? 'healthy' : 'offline'} (lag ${result.lag})`,
          };
    }

    case 'push': {
      ensureConfigured();
      const [batchSizeRaw, dryRunRaw] = args;
      const batchSize =
        parseOptionalInt(batchSizeRaw, 'Usage: sync push [batchSize] [dryRun]') || 100;
      const dryRun = ['true', '1', 'yes', 'y'].includes(String(dryRunRaw || '').toLowerCase());
      const rawConfig = loadSyncConfig();
      const config = new SyncConfig(rawConfig);
      const engine = createSyncEngine({ db: commerce.db, config });
      await engine.initialize();
      const result = await engine.push({ batchSize, dryRun });
      await engine.shutdown();
      return jsonOutput
        ? result
        : { result, formatted: `${dryRun ? 'Would push' : 'Pushed'} ${result.pushed} sync events` };
    }

    case 'pull': {
      ensureConfigured();
      const [
        fromSequenceRaw,
        limitRaw,
        includeEventsRaw,
        includePayloadsRaw,
        decryptPayloadsRaw,
        keyIdRaw,
      ] = args;
      const fromSequence = parseOptionalInt(
        fromSequenceRaw,
        'Usage: sync pull [fromSequence] [limit] [includeEvents] [includePayloads] [decryptPayloads] [keyId]',
      );
      const limit =
        parseOptionalInt(
          limitRaw,
          'Usage: sync pull [fromSequence] [limit] [includeEvents] [includePayloads] [decryptPayloads] [keyId]',
        ) || 1000;
      const includeEvents = ['true', '1', 'yes', 'y'].includes(
        String(includeEventsRaw || '').toLowerCase(),
      );
      const includePayloads = ['true', '1', 'yes', 'y'].includes(
        String(includePayloadsRaw || '').toLowerCase(),
      );
      const decryptPayloads = ['true', '1', 'yes', 'y'].includes(
        String(decryptPayloadsRaw || '').toLowerCase(),
      );
      const keyId = parseOptionalInt(keyIdRaw, 'keyId must be a positive integer');
      const rawConfig = loadSyncConfig();
      const config = new SyncConfig(rawConfig);
      const engine = createSyncEngine({ db: commerce.db, config });
      await engine.initialize();
      const shouldIncludeEvents = includeEvents || includePayloads || decryptPayloads;
      const result = await engine.pull({ fromSequence, limit, includeEvents: shouldIncludeEvents });
      let events;
      if (shouldIncludeEvents && result.success) {
        const storedEvents = (result.sequenceNumbers || [])
          .map((sequenceNumber) => engine.getStoredEvent({ sequenceNumber, source: 'pulled' }))
          .filter(Boolean);
        const formatted = await collectFormattedPulledEvents(engine, storedEvents, {
          includePayloads,
          decryptPayloads,
          keyId,
        });
        events = formatted.events;
      }
      await engine.shutdown();
      return jsonOutput
        ? { ...result, events }
        : { result, events, formatted: `Pulled ${result.pulled} events` };
    }

    case 'outbox': {
      ensureConfigured();
      const [status = 'all', limitRaw] = args;
      const limit = parseOptionalInt(limitRaw, 'Usage: sync outbox [status] [limit]') || 20;
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
      }));
      return formatSyncRows(events, { output, jsonOutput, empty: 'No outbox events found.' });
    }

    case 'pulled': {
      ensureConfigured();
      const [limitRaw, includePayloadsRaw, decryptPayloadsRaw, keyIdRaw] = args;
      const limit =
        parseOptionalInt(
          limitRaw,
          'Usage: sync pulled [limit] [includePayloads] [decryptPayloads] [keyId]',
        ) || 20;
      const includePayloads = ['true', '1', 'yes', 'y'].includes(
        String(includePayloadsRaw || '').toLowerCase(),
      );
      const decryptPayloads = ['true', '1', 'yes', 'y'].includes(
        String(decryptPayloadsRaw || '').toLowerCase(),
      );
      const keyId = parseOptionalInt(keyIdRaw, 'keyId must be a positive integer');
      const rawConfig = loadSyncConfig();
      const config = new SyncConfig(rawConfig);
      const engine = createSyncEngine({ db: commerce.db, config });
      const events = engine.getPulledEvents(limit);
      const formatted = await collectFormattedPulledEvents(engine, events, {
        includePayloads,
        decryptPayloads,
        keyId,
      });
      return formatSyncRows(formatted.events, {
        output,
        jsonOutput,
        empty: 'No pulled events found.',
      });
    }

    case 'decrypt': {
      ensureConfigured();
      const [eventId, sequenceNumberRaw, source = 'auto', keyIdRaw] = args;
      if (!eventId && sequenceNumberRaw === undefined) {
        throw new Error('Usage: sync decrypt [eventId] [sequenceNumber] [source] [keyId]');
      }
      const rawConfig = loadSyncConfig();
      const config = new SyncConfig(rawConfig);
      const engine = createSyncEngine({ db: commerce.db, config });
      const result = await engine.decryptStoredEvent({
        eventId: eventId || undefined,
        sequenceNumber: parseOptionalInt(
          sequenceNumberRaw,
          'sequenceNumber must be a positive integer',
        ),
        source,
        keyId: parseOptionalInt(keyIdRaw, 'keyId must be a positive integer'),
      });
      return jsonOutput
        ? result
        : {
            result,
            formatted: `Decrypted sync event ${result.eventId || eventId || sequenceNumberRaw}`,
          };
    }

    case 'retry': {
      ensureConfigured();
      const outbox = createConfiguredOutbox(commerce.db);
      const retriedCount = outbox.retryFailed();
      return { retriedCount, formatted: `Reset ${retriedCount} failed events to pending` };
    }

    case 'history': {
      ensureConfigured();
      const [
        entityType,
        entityId,
        source = 'remote',
        limitRaw,
        includePayloadsRaw,
        decryptPayloadsRaw,
        keyIdRaw,
      ] = args;
      if (!entityType || !entityId) {
        throw new Error(
          'Usage: sync history <entityType> <entityId> [source] [limit] [includePayloads] [decryptPayloads] [keyId]',
        );
      }
      const rawConfig = loadSyncConfig();
      const config = new SyncConfig(rawConfig);
      if (source === 'local') {
        const engine = createSyncEngine({ db: commerce.db, config });
        const events = engine.getPulledEventsForEntity(
          entityType,
          entityId,
          parseOptionalInt(limitRaw, 'limit must be a positive integer') || 100,
        );
        const formatted = await collectFormattedPulledEvents(engine, events, {
          includePayloads: ['true', '1', 'yes', 'y'].includes(
            String(includePayloadsRaw || '').toLowerCase(),
          ),
          decryptPayloads: ['true', '1', 'yes', 'y'].includes(
            String(decryptPayloadsRaw || '').toLowerCase(),
          ),
          keyId: parseOptionalInt(keyIdRaw, 'keyId must be a positive integer'),
        });
        return formatSyncRows(formatted.events, {
          output,
          jsonOutput,
          empty: 'No sync history found.',
        });
      }
      const client = createSequencerClient(config);
      await client.connect();
      const events = await client.getEntityHistory(entityType, entityId);
      return formatSyncRows(
        events.map((event) => ({
          sequenceNumber: event.sequenceNumber,
          eventId: event.envelope.eventId,
          eventType: event.envelope.eventType,
          createdAt: event.envelope.createdAt,
          sourceAgent: event.envelope.sourceAgent,
        })),
        { output, jsonOutput, empty: 'No sync history found.' },
      );
    }

    case 'full': {
      ensureConfigured();
      const [pushBatchSizeRaw, pullLimitRaw] = args;
      const pushBatchSize =
        parseOptionalInt(pushBatchSizeRaw, 'Usage: sync full [pushBatchSize] [pullLimit]') || 100;
      const pullLimit =
        parseOptionalInt(pullLimitRaw, 'Usage: sync full [pushBatchSize] [pullLimit]') || 1000;
      const rawConfig = loadSyncConfig();
      const config = new SyncConfig(rawConfig);
      const engine = createSyncEngine({ db: commerce.db, config });
      await engine.initialize();
      const push = await engine.push({ batchSize: pushBatchSize });
      const pull = await engine.pull({ limit: pullLimit });
      await engine.shutdown();
      return jsonOutput
        ? { push, pull }
        : {
            push,
            pull,
            formatted: `Full sync complete: pushed ${push.pushed}, pulled ${pull.pulled}`,
          };
    }

    case 'conflicts': {
      ensureConfigured();
      const rawConfig = loadSyncConfig();
      const config = new SyncConfig(rawConfig);
      const engine = createSyncEngine({ db: commerce.db, config });
      await engine.initialize();
      const conflicts = await engine.getConflicts();
      await engine.shutdown();
      return formatSyncRows(conflicts, { output, jsonOutput, empty: 'No sync conflicts found.' });
    }

    case 'resolve': {
      ensureConfigured();
      const [conflictId, strategy] = args;
      if (!conflictId) throw new Error('Usage: sync resolve <conflictId> [strategy]');
      const rawConfig = loadSyncConfig();
      const config = new SyncConfig(rawConfig);
      const engine = createSyncEngine({ db: commerce.db, config });
      await engine.initialize();
      const result = await engine.resolveConflict(conflictId, strategy || undefined);
      await engine.shutdown();
      return jsonOutput ? result : { result, formatted: `Resolved sync conflict ${conflictId}` };
    }

    case 'rebase': {
      ensureConfigured();
      const strategy = args[0] || 'remote-wins';
      const rawConfig = loadSyncConfig();
      const config = new SyncConfig(rawConfig);
      const engine = createSyncEngine({ db: commerce.db, config });
      await engine.initialize();
      const result = await engine.rebase({ strategy });
      await engine.shutdown();
      return jsonOutput
        ? result
        : { result, formatted: `Rebased sync conflicts with strategy ${strategy}` };
    }

    case 'verify-receipt': {
      const [envelopeJson, publicKeyHex, publicKeyBundleJson] = args;
      if (!envelopeJson) {
        throw new Error(
          'Usage: sync verify-receipt <envelopeJson> [publicKeyHex] [publicKeyBundleJson]',
        );
      }
      const {
        computeEventSigningHash,
        verifyEventSignature,
        verifyEventSignatureHybrid,
        hexToBuffer,
      } = await import('../sync/crypto.js');
      const envelope = parseJsonArg(envelopeJson, 'envelope');
      const publicKeyBundle = publicKeyBundleJson
        ? parseJsonArg(publicKeyBundleJson, 'publicKeyBundle')
        : undefined;
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
          valid = verifyEventSignatureHybrid(
            signingHash,
            envelope.agentSignatureBundle,
            publicKeyBundle,
          );
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
      const result = {
        valid,
        eventId: envelope.eventId,
        sourceAgent: envelope.sourceAgent,
        entityType: envelope.entityType,
        entityId: envelope.entityId,
      };
      return jsonOutput
        ? result
        : { result, formatted: `Receipt verification: ${valid ? 'valid' : 'invalid'}` };
    }

    case 'verify-inclusion': {
      const [envelopeJson, proofJson, expectedRoot] = args;
      if (!envelopeJson || !proofJson || !expectedRoot) {
        throw new Error('Usage: sync verify-inclusion <envelopeJson> <proofJson> <expectedRoot>');
      }
      const { computeNodeHash, hexToBuffer } = await import('../sync/crypto.js');
      const crypto = await import('node:crypto');
      const envelope = parseJsonArg(envelopeJson, 'envelope');
      const proof = parseJsonArg(proofJson, 'proof');
      const leafHash = crypto
        .createHash('sha256')
        .update(hexToBuffer(envelope.payloadPlainHash))
        .update(hexToBuffer(envelope.agentSignature))
        .digest();
      let currentHash = leafHash;
      let index = proof.leafIndex;
      for (const siblingHex of proof.proofHashes) {
        const sibling = hexToBuffer(siblingHex);
        currentHash =
          index % 2 === 0
            ? computeNodeHash(currentHash, sibling)
            : computeNodeHash(sibling, currentHash);
        index = Math.floor(index / 2);
      }
      const valid = currentHash.equals(hexToBuffer(expectedRoot));
      const result = { valid, eventId: envelope.eventId, expectedRoot };
      return jsonOutput
        ? result
        : { result, formatted: `Inclusion proof: ${valid ? 'valid' : 'invalid'}` };
    }

    case 'commitment': {
      ensureConfigured();
      const batchId = args[0];
      if (!batchId) throw new Error('Usage: sync commitment <batchId>');
      const rawConfig = loadSyncConfig();
      const config = new SyncConfig(rawConfig);
      const client = createSequencerClient(config);
      const result = await client.getCommitment(batchId);
      if (!result) throw new Error(`Commitment not found: ${batchId}`);
      return jsonOutput
        ? result
        : { result, formatted: `Commitment ${batchId}: ${result.eventCount} events` };
    }

    case 'key-generate': {
      const [agentId, keyType, securityProfile] = args;
      if (!agentId || !keyType)
        throw new Error('Usage: sync key-generate <agentId> <keyType> [securityProfile]');
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
      const result = {
        agentId,
        keyType,
        keyId: keyPair.keyId,
        keyAlgorithm: keyPair.keyAlgorithm,
        securityProfile: keyPair.securityProfile,
        publicKeyHex: bufferToHex(keyPair.publicKey),
        createdAt: keyPair.createdAt,
      };
      return jsonOutput
        ? result
        : { result, formatted: `Generated ${keyType} key ${keyPair.keyId} for ${agentId}` };
    }

    case 'key-list': {
      const [agentId, keyType, securityProfile] = args;
      if (!agentId) throw new Error('Usage: sync key-list <agentId> [keyType] [securityProfile]');
      const { AgentKeyManager } = await import('../sync/keys.js');
      const { bufferToHex } = await import('../sync/crypto.js');
      const keyManager = new AgentKeyManager(
        '.stateset',
        getAgentKeyManagerOptions(securityProfile),
      );
      const results = [];
      const types = keyType ? [keyType] : ['signing', 'encryption'];
      for (const currentType of types) {
        const keys =
          currentType === 'signing'
            ? await keyManager.listSigningKeys(agentId)
            : await keyManager.listEncryptionKeys(agentId);
        for (const key of keys) {
          results.push({
            keyId: key.keyId,
            keyType: currentType,
            keyAlgorithm: key.keyAlgorithm,
            securityProfile: key.securityProfile,
            publicKeyHex: bufferToHex(key.publicKey),
            createdAt: key.createdAt,
            revokedAt: key.revokedAt || null,
          });
        }
      }
      return formatSyncRows(results, { output, jsonOutput, empty: 'No agent keys found.' });
    }

    case 'key-info': {
      const [agentId, keyType, keyIdRaw, securityProfile] = args;
      if (!agentId || !keyType || !keyIdRaw) {
        throw new Error('Usage: sync key-info <agentId> <keyType> <keyId> [securityProfile]');
      }
      const { AgentKeyManager } = await import('../sync/keys.js');
      const { bufferToHex } = await import('../sync/crypto.js');
      const keyManager = new AgentKeyManager(
        '.stateset',
        getAgentKeyManagerOptions(securityProfile),
      );
      const key =
        keyType === 'signing'
          ? await keyManager.getSigningKey(
              agentId,
              parseOptionalInt(keyIdRaw, 'keyId must be a positive integer'),
            )
          : await keyManager.getEncryptionKey(
              agentId,
              parseOptionalInt(keyIdRaw, 'keyId must be a positive integer'),
            );
      if (!key) throw new Error(`Key not found: ${agentId}/${keyType}/${keyIdRaw}`);
      const result = {
        agentId,
        keyType,
        keyId: key.keyId,
        keyAlgorithm: key.keyAlgorithm,
        securityProfile: key.securityProfile,
        publicKeyHex: bufferToHex(key.publicKey),
        createdAt: key.createdAt,
        revokedAt: key.revokedAt || null,
      };
      return jsonOutput ? result : { result, formatted: `Key ${key.keyId} for ${agentId}` };
    }

    case 'key-rotate': {
      const [agentId, keyType, securityProfile] = args;
      if (!agentId || !keyType)
        throw new Error('Usage: sync key-rotate <agentId> <keyType> [securityProfile]');
      const { AgentKeyManager } = await import('../sync/keys.js');
      const { bufferToHex } = await import('../sync/crypto.js');
      const keyManager = new AgentKeyManager(
        '.stateset',
        getAgentKeyManagerOptions(securityProfile),
      );
      const currentKey =
        keyType === 'signing'
          ? await keyManager.getCurrentSigningKey(agentId)
          : await keyManager.getCurrentEncryptionKey(agentId);
      if (!currentKey) throw new Error(`No current ${keyType} key found for ${agentId}`);
      const newKey =
        keyType === 'signing'
          ? await keyManager.generateSigningKey(agentId)
          : await keyManager.generateEncryptionKey(agentId);
      if (keyType === 'signing') {
        await keyManager.revokeSigningKey(agentId, currentKey.keyId);
      } else {
        await keyManager.revokeEncryptionKey(agentId, currentKey.keyId);
      }
      const result = {
        agentId,
        keyType,
        oldKeyId: currentKey.keyId,
        newKeyId: newKey.keyId,
        keyAlgorithm: newKey.keyAlgorithm,
        securityProfile: newKey.securityProfile,
        newPublicKeyHex: bufferToHex(newKey.publicKey),
        createdAt: newKey.createdAt,
      };
      return jsonOutput ? result : { result, formatted: `Rotated ${keyType} key for ${agentId}` };
    }

    case 'key-export': {
      const [agentId, keyType = 'signing', keyIdRaw, securityProfile] = args;
      if (!agentId)
        throw new Error('Usage: sync key-export <agentId> [keyType] [keyId] [securityProfile]');
      const { AgentKeyManager } = await import('../sync/keys.js');
      const keyManager = new AgentKeyManager(
        '.stateset',
        getAgentKeyManagerOptions(securityProfile),
      );
      const result =
        keyType === 'signing'
          ? await keyManager.exportSigningPublicKey(
              agentId,
              parseOptionalInt(keyIdRaw, 'keyId must be a positive integer') || null,
            )
          : await keyManager.exportEncryptionPublicKey(
              agentId,
              parseOptionalInt(keyIdRaw, 'keyId must be a positive integer') || null,
            );
      return jsonOutput
        ? result
        : { result, formatted: `Exported ${keyType} public key for ${agentId}` };
    }

    default:
      throw new Error(
        `Unknown action: sync ${action}\n\n` +
          'Available actions:\n' +
          '  status                                               Get sync status\n' +
          '  push [batchSize] [dryRun]                            Push pending events\n' +
          '  pull [fromSequence] [limit] [includeEvents] [includePayloads] [decryptPayloads] [keyId]  Pull events\n' +
          '  outbox [status] [limit]                              List outbox events\n' +
          '  pulled [limit] [includePayloads] [decryptPayloads] [keyId]  List pulled events\n' +
          '  decrypt [eventId] [sequenceNumber] [source] [keyId] Decrypt stored event\n' +
          '  retry                                                Retry failed events\n' +
          '  history <entityType> <entityId> [source] [limit] [includePayloads] [decryptPayloads] [keyId]  Get entity history\n' +
          '  full [pushBatchSize] [pullLimit]                     Run full sync\n' +
          '  conflicts                                            List sync conflicts\n' +
          '  resolve <conflictId> [strategy]                      Resolve conflict\n' +
          '  rebase [strategy]                                    Rebase all conflicts\n' +
          '  verify-receipt <envelopeJson> [publicKeyHex] [publicKeyBundleJson]  Verify receipt\n' +
          '  verify-inclusion <envelopeJson> <proofJson> <expectedRoot>  Verify inclusion proof\n' +
          '  commitment <batchId>                                 Inspect batch commitment\n' +
          '  key-generate <agentId> <keyType> [securityProfile]   Generate agent key\n' +
          '  key-list <agentId> [keyType] [securityProfile]       List agent keys\n' +
          '  key-info <agentId> <keyType> <keyId> [securityProfile]  Get agent key info\n' +
          '  key-rotate <agentId> <keyType> [securityProfile]     Rotate agent key\n' +
          '  key-export <agentId> [keyType] [keyId] [securityProfile]  Export public key',
      );
  }
}

function formatSyncRows(rows, { output, jsonOutput, empty }) {
  if (jsonOutput) return rows;
  if (!rows || rows.length === 0) return { formatted: empty };
  const first = rows[0];
  const columns = Object.keys(first)
    .slice(0, 6)
    .map((key) => ({ key, header: key }));
  const formatted = output.table(rows, columns);
  return { rows, formatted };
}

export const metadata = {
  name: 'sync',
  aliases: ['ves', 'sequencer'],
  description: 'Event sync, receipt verification, and key-management commands',
  actions: {
    status: { description: 'Get sync status', args: [] },
    push: { description: 'Push pending events', args: ['[batchSize]', '[dryRun]'] },
    pull: {
      description: 'Pull events',
      args: [
        '[fromSequence]',
        '[limit]',
        '[includeEvents]',
        '[includePayloads]',
        '[decryptPayloads]',
        '[keyId]',
      ],
    },
    outbox: { description: 'List outbox events', args: ['[status]', '[limit]'] },
    pulled: {
      description: 'List pulled events',
      args: ['[limit]', '[includePayloads]', '[decryptPayloads]', '[keyId]'],
    },
    decrypt: {
      description: 'Decrypt stored event',
      args: ['[eventId]', '[sequenceNumber]', '[source]', '[keyId]'],
    },
    retry: { description: 'Retry failed events', args: [] },
    history: {
      description: 'Get entity history',
      args: [
        '<entityType>',
        '<entityId>',
        '[source]',
        '[limit]',
        '[includePayloads]',
        '[decryptPayloads]',
        '[keyId]',
      ],
    },
    full: { description: 'Run full sync', args: ['[pushBatchSize]', '[pullLimit]'] },
    conflicts: { description: 'List sync conflicts', args: [] },
    resolve: { description: 'Resolve sync conflict', args: ['<conflictId>', '[strategy]'] },
    rebase: { description: 'Rebase all sync conflicts', args: ['[strategy]'] },
    'verify-receipt': {
      description: 'Verify event receipt',
      args: ['<envelopeJson>', '[publicKeyHex]', '[publicKeyBundleJson]'],
    },
    'verify-inclusion': {
      description: 'Verify inclusion proof',
      args: ['<envelopeJson>', '<proofJson>', '<expectedRoot>'],
    },
    commitment: { description: 'Inspect batch commitment', args: ['<batchId>'] },
    'key-generate': {
      description: 'Generate agent key',
      args: ['<agentId>', '<keyType>', '[securityProfile]'],
    },
    'key-list': {
      description: 'List agent keys',
      args: ['<agentId>', '[keyType]', '[securityProfile]'],
    },
    'key-info': {
      description: 'Get agent key info',
      args: ['<agentId>', '<keyType>', '<keyId>', '[securityProfile]'],
    },
    'key-rotate': {
      description: 'Rotate agent key',
      args: ['<agentId>', '<keyType>', '[securityProfile]'],
    },
    'key-export': {
      description: 'Export public key',
      args: ['<agentId>', '[keyType]', '[keyId]', '[securityProfile]'],
    },
  },
};

export default { execute, metadata };

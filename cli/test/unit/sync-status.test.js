import { afterEach, beforeEach, describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import Database from 'better-sqlite3';
import { createOutbox } from '../../src/sync/outbox.js';
import { readSyncStatus } from '../../src/sync/status.js';
import { execute } from '../../src/commands/sync.js';
import { syncTools } from '../../src/tools/sync.js';

describe('operator sync status', () => {
  let db;
  let outbox;
  let root;
  let cwd;
  beforeEach(() => {
    cwd = process.cwd();
    root = mkdtempSync(join(tmpdir(), 'stateset-sync-status-'));
    mkdirSync(join(root, '.stateset'));
    writeFileSync(
      join(root, '.stateset', 'sync.json'),
      JSON.stringify({
        sequencer: { url: 'https://sequencer.invalid' },
        identity: { tenantId: 'tenant', storeId: 'store', agentId: 'agent' },
        sync: { securityProfile: 'legacy' },
      }),
    );
    process.chdir(root);
    db = new Database(join(root, 'store.db'));
    outbox = createOutbox(db);
    outbox.initialize();
  });
  afterEach(() => {
    mock.restoreAll();
    db.close();
    process.chdir(cwd);
    rmSync(root, { recursive: true, force: true });
  });

  function headResponse(head) {
    mock.method(globalThis, 'fetch', async (url) => {
      assert.equal(new URL(url).hostname, 'sequencer.invalid');
      return new Response(
        JSON.stringify(url.endsWith('/health') ? { status: 'ok' } : { head_sequence: head }),
      );
    });
  }

  function outgoing(count, status) {
    const insert = db.prepare(`INSERT INTO _ves_outbox
      (event_id, tenant_id, store_id, entity_type, entity_id, event_type, payload,
       payload_plain_hash, payload_cipher_hash, agent_key_id, agent_signature, source_agent, sync_status)
      VALUES (?, 'tenant', 'store', 'order', 'order', 'order.created', '{}', 'hash', 'hash', 1, 'sig', 'agent', ?)`);
    db.transaction(() => {
      for (let i = 0; i < count; i++) insert.run(`${status}-${i}`, status);
    })();
  }

  it('reports numeric zero counts for a fresh database', () => {
    const status = readSyncStatus(outbox, { connected: true, remoteHead: 0 });
    assert.equal(status.health, 'healthy');
    assert.equal(status.lag, 0);
    assert.equal(status.localHead, 0);
    assert.equal(status.nextPullCursor, 0);
    for (const key of ['pending', 'synced', 'failed', 'rejected'])
      assert.equal(status.outbox[key], 0);
    assert.equal(status.receive.verifiedHead, null);
    assert.equal(status.localState.lastSyncAt, null);
    assert.equal(status.localState.lastPullAt, null);
  });

  it('keeps failed/rejected writes degraded even when no pending work remains', () => {
    outgoing(1, 'failed');
    outgoing(1, 'rejected');
    const status = readSyncStatus(outbox, { connected: true, remoteHead: 0 });
    assert.equal(status.pending, 0);
    assert.equal(status.health, 'degraded');
    assert.deepEqual(status.healthReasons, ['failed_outgoing_events', 'rejected_outgoing_events']);
  });

  it('uses consistent lag and pending backlog thresholds', () => {
    outgoing(999, 'pending');
    assert.equal(readSyncStatus(outbox, { connected: true, remoteHead: 99 }).health, 'healthy');
    db.prepare(
      "INSERT INTO _ves_outbox (event_id, tenant_id, store_id, entity_type, entity_id, event_type, payload, payload_plain_hash, payload_cipher_hash, agent_key_id, agent_signature, source_agent) VALUES ('last', 'tenant', 'store', 'order', 'order', 'order.created', '{}', 'hash', 'hash', 1, 'sig', 'agent')",
    ).run();
    const status = readSyncStatus(outbox, { connected: true, remoteHead: 100 });
    assert.deepEqual(status.healthReasons, ['pull_lag', 'pending_backlog']);
  });

  it('preserves complete failure counts after restart and exposes the same diagnostics through tools and commands', async () => {
    db.transaction(() => {
      for (let sequenceNumber = 1; sequenceNumber <= 1001; sequenceNumber++) {
        outbox.storeReceiveFailure(
          { sequenceNumber, payload: { private: 'secret-payload' } },
          'verified',
          'storage_failed',
          'sensitive-error',
        );
      }
    })();
    outbox.updateSyncState({ lastPulledSequence: 1002, headSequence: 1001 });
    db.close();
    db = new Database(join(root, 'store.db'));
    headResponse(1001);
    const tool = await syncTools
      .find((t) => t.name === 'sync_status')
      .handler({ commerce: { db } });
    const command = await execute('status', [], { db, jsonOutput: true });
    assert.deepEqual(tool, command);
    assert.equal(command.health, 'degraded');
    assert.equal(command.localHead, 1001);
    assert.equal(command.nextPullCursor, 1002);
    assert.equal(command.lag, 0);
    assert.deepEqual(command.receive.failures, { count: 1001, oldestSequence: 1 });
    assert.equal(command.receive.verified, 0);
    assert.ok(!JSON.stringify(command).includes('secret-payload'));
    assert.ok(!JSON.stringify(command).includes('sensitive-error'));
    const formatted = await execute('status', [], { db, jsonOutput: false });
    assert.match(formatted.formatted, /degraded.*retained_receive_failures/);
  });

  it('counts quarantine beyond the event-list page limit', () => {
    const insert = db.prepare(`INSERT INTO _ves_quarantined_events
      (event_id, sequence_number, tenant_id, store_id, entity_type, entity_id, event_type,
       payload, payload_plain_hash, payload_cipher_hash, agent_key_id, agent_signature,
       created_at, sequenced_at, source_agent, reason)
      VALUES (?, ?, 'tenant', 'store', 'order', 'order', 'order.created', '{}', 'hash', 'hash', 1, 'sig', 'time', 'time', 'agent', 'signature_invalid')`);
    db.transaction(() => {
      for (let i = 1; i <= 1001; i++) insert.run(`event-${i}`, i);
    })();
    const status = readSyncStatus(outbox, { connected: true, remoteHead: 1001 });
    assert.equal(status.receive.quarantined, 1001);
    assert.deepEqual(status.receive.quarantineReasons, [
      { reason: 'signature_invalid', count: 1001 },
    ]);
  });

  it('retains cached progress and diagnostics when the network is unavailable', async () => {
    outgoing(1, 'rejected');
    outbox.updateSyncState({ lastPulledSequence: 10, headSequence: 12 });
    mock.method(globalThis, 'fetch', async () => {
      throw new Error('test offline');
    });
    const result = await execute('status', [], { db, jsonOutput: true });
    assert.equal(result.health, 'offline');
    assert.equal(result.remoteHead, 12);
    assert.equal(result.lag, 3);
    assert.deepEqual(result.healthReasons, ['offline', 'rejected_outgoing_events']);
  });

  it('does not call a missing remote head a healthy empty store', async () => {
    headResponse(undefined);
    const result = await execute('status', [], { db, jsonOutput: true });
    assert.notEqual(result.health, 'healthy');
    assert.match(result.connectionError, /Invalid remote head/);
  });
});

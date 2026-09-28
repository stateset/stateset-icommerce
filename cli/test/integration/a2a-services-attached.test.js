// The A2A automation/platform tools guard on services attached to the commerce
// wrapper (`commerce._disputeResolver`, `_slaService`, ...). Only
// `initializeIntelligenceServices` attaches them, and it used to attach just
// the intelligence set — so dozens of advertised tools answered "not
// initialized" on every real server. These tests build the server the way the
// bins do (createStatesetMcpServer over a file database), await service
// initialization, and drive each service through its tool handler.

import { after, afterEach, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { createStatesetMcpServer } from '../../src/mcp-server.js';
import {
  createA2AServiceBinding,
  disposeA2AServices,
  initializeIntelligenceServices,
} from '../../src/mcp/a2a-service.js';
import { A2AStore } from '../../src/a2a/store.js';
import { A2A_SERVICE_REQUIREMENTS } from '../../src/a2a/service-requirements.js';

const ALWAYS_ATTACHED = [
  '_notificationService',
  '_escrowService',
  '_disputeResolver',
  '_slaService',
  '_marketplace',
  '_healthService',
  '_sagaOrchestrator',
  '_fanOutCoordinator',
  '_handshakeService',
  '_dataExportService',
];
const WALLET_ATTACHED = ['_billingExecutor', '_batchService'];
const WALLET = '0x00000000000000000000000000000000000000a1';

async function callTool(server, name, params = {}) {
  const response = await server.executeTool(name, params);
  assert.ok(response, `${name} returned nothing`);
  return response;
}

/** Assert the tool ran against a real service (no "not initialized" answer). */
async function callLive(server, name, params = {}) {
  const response = await callTool(server, name, params);
  assert.doesNotMatch(
    JSON.stringify(response.result ?? response.error ?? null),
    /not initialized/i,
    `${name} still answers "not initialized"`,
  );
  assert.equal(response.status, 'success', `${name}: ${JSON.stringify(response.error)}`);
  return response.result;
}

async function closeServer(server) {
  server?.dispose?.();
  try {
    await server?.close?.();
  } catch {
    // Never connected to a transport; dispose above already stopped timers.
  }
}

describe('initializeIntelligenceServices attaches the A2A automation services', () => {
  let dir;
  let store;
  let commerce;

  before(async () => {
    dir = await mkdtemp(join(tmpdir(), 'a2a-services-unit-'));
    store = new A2AStore({ dbPath: join(dir, 'a2a.db') });
  });

  afterEach(() => disposeA2AServices(commerce));

  after(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it('attaches every service it can build without a wallet, and none of the wallet ones', async () => {
    const binding = createA2AServiceBinding(store);
    commerce = { a2a: binding.a2a };
    await initializeIntelligenceServices({
      commerceWithA2A: commerce,
      a2aStore: store,
      setA2AServiceFactory: binding.setFactory,
      checkpointDir: join(dir, 'checkpoints'),
    });

    for (const key of [...ALWAYS_ATTACHED, '_checkpointService']) {
      assert.ok(commerce[key], `${key} should be attached`);
    }
    for (const key of [...WALLET_ATTACHED, '_sequencerClient', '_tickOptimizer']) {
      assert.equal(commerce[key], undefined, `${key} must not be attached without its config`);
    }
  });

  it('attaches the wallet-bound services when an agent wallet is configured', async () => {
    const binding = createA2AServiceBinding(store);
    commerce = { a2a: binding.a2a };
    const sequencerClient = {
      getCircuitStatus: () => ({ state: 'closed', failures: 0, queueDepth: 0 }),
    };
    await initializeIntelligenceServices({
      commerceWithA2A: commerce,
      a2aStore: store,
      setA2AServiceFactory: binding.setFactory,
      agentConfig: { walletAddress: WALLET, sequencerClient },
    });

    for (const key of [...ALWAYS_ATTACHED, ...WALLET_ATTACHED]) {
      assert.ok(commerce[key], `${key} should be attached`);
    }
    assert.equal(commerce._sequencerClient, sequencerClient);
    // No data dir was given, so no checkpoint service.
    assert.equal(commerce._checkpointService, undefined);
  });

  it('constructing the services starts no background loop', async () => {
    const binding = createA2AServiceBinding(store);
    commerce = { a2a: binding.a2a };
    await initializeIntelligenceServices({
      commerceWithA2A: commerce,
      a2aStore: store,
      setA2AServiceFactory: binding.setFactory,
      agentConfig: { walletAddress: WALLET },
    });
    assert.equal(commerce._billingExecutor.getMetrics().running, false);
    assert.equal(commerce._disputeResolver.getMetrics().running, false);
  });
});

describe('MCP server: A2A service-backed tools are live', () => {
  let dir;
  let server;

  before(async () => {
    dir = await mkdtemp(join(tmpdir(), 'a2a-services-mcp-'));
    const dbPath = join(dir, 'store.db');
    server = createStatesetMcpServer({ dbPath, allowApply: true });
    await server.servicesReady;
  });

  after(async () => {
    await closeServer(server);
    await rm(dir, { recursive: true, force: true });
  });

  it('dispute resolver: tick and metrics', async () => {
    const tick = await callLive(server, 'a2a_dispute_resolver_tick');
    assert.equal(typeof tick.transitions, 'number');
    const metrics = await callLive(server, 'a2a_dispute_resolver_metrics');
    assert.equal(metrics.totalTicks, 1);
    assert.equal(metrics.running, false);
  });

  it('SLA enforcement cycle', async () => {
    const result = await callLive(server, 'a2a_sla_enforce_all');
    assert.ok(result && typeof result === 'object');
  });

  it('marketplace maintenance and auto-award', async () => {
    const tick = await callLive(server, 'a2a_marketplace_maintenance');
    assert.equal(tick.awarded, 0);
    assert.ok(tick.timestamp);
    const award = await callLive(server, 'a2a_marketplace_auto_award');
    assert.equal(award.expired, 0);
  });

  it('notification retry', async () => {
    const result = await callLive(server, 'a2a_notification_retry_all');
    assert.ok(result && typeof result === 'object');
  });

  it('health check runs the real checks (database + subsystems)', async () => {
    const health = await callLive(server, 'a2a_health_check');
    assert.equal(health.status, 'healthy');
    assert.equal(health.checks.database.status, 'ok');
    assert.equal(health.checks.sequencer.status, 'not_configured');
    assert.ok(health.checks.disputeResolver, 'dispute resolver reported as a subsystem');
    assert.equal((await callLive(server, 'a2a_readiness')).status, 'ready');
  });

  it('escrow processing', async () => {
    const result = await callLive(server, 'a2a_escrow_process_all');
    assert.ok(result && typeof result === 'object');
  });

  it('saga status/list are live; execute refuses with the reason', async () => {
    assert.deepEqual(await callLive(server, 'a2a_saga_list'), []);
    const status = await callTool(server, 'a2a_saga_status', { sagaId: 'missing' });
    assert.doesNotMatch(JSON.stringify(status), /not initialized/i);
    const exec = await callTool(server, 'a2a_saga_execute', {
      sagaType: 'purchase',
      context: {},
    });
    assert.equal(exec.result.error, A2A_SERVICE_REQUIREMENTS.sagaExecute);
  });

  it('fan-out scatter and status (timers are cleared on dispose)', async () => {
    const coordinationId = await callLive(server, 'a2a_scatter', {
      targets: ['0xb0b'],
      taskType: 'status_check',
      payload: { ping: true },
      timeoutMs: 60_000,
    });
    assert.equal(typeof coordinationId, 'string');
    const status = await callLive(server, 'a2a_coordination_status', { coordinationId });
    assert.equal(status.status, 'pending');
  });

  it('handshake manifest and negotiation', async () => {
    const mine = await callLive(server, 'a2a_my_capabilities');
    assert.ok(Array.isArray(mine.supportedNetworks));
    const hs = await callLive(server, 'a2a_handshake', {
      targetCapabilities: { supportedNetworks: ['set_chain'], supportedAssets: ['USDC'] },
    });
    assert.equal(typeof hs.compatible, 'boolean');
  });

  it('data export, commerce report and data stats', async () => {
    const exported = await callLive(server, 'a2a_export_agent_data', { agentAddress: '0xa11ce' });
    assert.ok(exported && typeof exported === 'object');
    const report = await callLive(server, 'a2a_commerce_report', { agentAddress: '0xa11ce' });
    assert.ok(report && typeof report === 'object');
    const stats = await callLive(server, 'a2a_data_stats');
    assert.ok(stats && typeof stats === 'object');
  });

  it('checkpoints persist beside a file-backed database', async () => {
    await callLive(server, 'a2a_save_checkpoint', { data: { cursor: 7 } });
    const loaded = await callLive(server, 'a2a_load_checkpoint');
    assert.equal(loaded.cursor, 7);
    const list = await callLive(server, 'a2a_list_checkpoints');
    assert.ok(JSON.stringify(list).includes('default'));
  });

  it('wallet-bound and unconfigured tools name what is missing', async () => {
    for (const tool of ['a2a_billing_tick', 'a2a_billing_start', 'a2a_billing_metrics']) {
      const res = await callTool(server, tool);
      assert.equal(res.result.error, A2A_SERVICE_REQUIREMENTS.billingExecutor, tool);
    }
    const batch = await callTool(server, 'a2a_batch_request_quotes', {
      requests: [{ seller: '0xb0b', items: [{ sku: 'x' }] }],
    });
    assert.equal(batch.result.error, A2A_SERVICE_REQUIREMENTS.batchService);
    const tick = await callTool(server, 'a2a_tick_metrics');
    assert.equal(tick.result.error, A2A_SERVICE_REQUIREMENTS.tickOptimizer);
    const circuit = await callLive(server, 'x402_circuit_status');
    assert.equal(circuit.state, 'not_configured');
    assert.equal(circuit.note, A2A_SERVICE_REQUIREMENTS.sequencerClient);
  });
});

describe('MCP server with an agent wallet', () => {
  let dir;
  let server;
  let store;

  before(async () => {
    dir = await mkdtemp(join(tmpdir(), 'a2a-services-wallet-'));
    const dbPath = join(dir, 'store.db');
    server = createStatesetMcpServer({
      dbPath,
      allowApply: true,
      agentConfig: { agentId: 'agent-test', walletAddress: WALLET },
    });
    await server.servicesReady;
    store = new A2AStore({ dbPath });
  });

  after(async () => {
    await closeServer(server);
    await rm(dir, { recursive: true, force: true });
  });

  it('billing tick only bills subscriptions this wallet subscribes to', async () => {
    const past = new Date(Date.now() - 86_400_000).toISOString();
    const foreign = store.createSubscription({
      subscriber_address: '0x00000000000000000000000000000000000000b2',
      provider_address: '0x00000000000000000000000000000000000000c3',
      plan_name: 'someone else',
      amount: 1_000_000,
      amount_decimal: 1,
      current_period_end: past,
      next_billing_date: past,
    });

    const tick = await callLive(server, 'a2a_billing_tick');
    assert.equal(tick.billed, 0);
    assert.equal(tick.failed, 0);

    const after = store.getSubscription(foreign.id);
    assert.equal(after.status, 'active');
    assert.equal(after.billing_count ?? 0, 0);
    assert.equal(after.past_due_since ?? null, null);
  });

  it('billing start/stop drive a real loop, and dispose stops it', async () => {
    await callLive(server, 'a2a_billing_start');
    assert.equal((await callLive(server, 'a2a_billing_metrics')).running, true);
    await callLive(server, 'a2a_billing_stop');
    assert.equal((await callLive(server, 'a2a_billing_metrics')).running, false);

    await callLive(server, 'a2a_billing_start');
    await callLive(server, 'a2a_dispute_resolver_start');
    server.dispose();
    assert.equal((await callLive(server, 'a2a_billing_metrics')).running, false);
    assert.equal((await callLive(server, 'a2a_dispute_resolver_metrics')).running, false);
  });

  it('batch quote requests reach the wallet-bound service', async () => {
    const res = await callLive(server, 'a2a_batch_request_quotes', {
      requests: [{ seller: '0xb0b', items: [{ sku: 'x', quantity: 1 }] }],
    });
    assert.equal(res.sent, 1);
    assert.equal(res.failed, 0);
  });
});

describe('MCP server: webhook dead-letter queue tools', () => {
  let dir;
  let server;
  let store;

  before(async () => {
    dir = await mkdtemp(join(tmpdir(), 'a2a-services-dlq-'));
    const dbPath = join(dir, 'store.db');
    server = createStatesetMcpServer({ dbPath, allowApply: true });
    await server.servicesReady;
    store = new A2AStore({ dbPath });
  });

  after(async () => {
    await closeServer(server);
    await rm(dir, { recursive: true, force: true });
  });

  it('quarantine, list, count, replay and purge reach the store', async () => {
    const failed = store.createNotificationLog({
      recipient_address: '0xdead',
      endpoint_url: 'https://example.invalid/hook',
      event_type: 'payment.completed',
      payload: { ok: false },
      status: 'failed',
      attempts: 3,
      last_error: 'HTTP 500',
    });

    const quarantined = await callLive(server, 'a2a_quarantine_failed_webhooks', { limit: 10 });
    assert.equal(quarantined.quarantined, 1);

    const listed = await callLive(server, 'a2a_list_webhook_dlq', { recipientAddress: '0xdead' });
    assert.equal(listed.count, 1);
    assert.equal(listed.totalCount, 1);
    assert.equal(listed.entries[0].original_notification_id, failed.id);

    const counted = await callLive(server, 'a2a_dlq_count', { eventType: 'payment.completed' });
    assert.equal(counted.count, 1);

    const replayed = await callLive(server, 'a2a_replay_dlq_entry', {
      dlqId: listed.entries[0].id,
    });
    assert.equal(replayed.replayed, true);
    assert.equal(store.getNotificationLog(failed.id).status, 'pending');

    const purged = await callLive(server, 'a2a_purge_dlq', { olderThanDays: 1 });
    assert.equal(purged.purged, 0);
  });

  it('the DLQ write tools stay preview-only without --apply', async () => {
    const preview = createStatesetMcpServer({ dbPath: join(dir, 'store.db'), allowApply: false });
    try {
      await preview.servicesReady;
      const res = await preview.executeTool('a2a_purge_dlq', { olderThanDays: 1 });
      assert.notEqual(res.status, 'success');
      assert.doesNotMatch(JSON.stringify(res), /"purged"/);
    } finally {
      await closeServer(preview);
    }
  });
});

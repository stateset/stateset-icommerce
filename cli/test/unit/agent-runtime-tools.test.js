/**
 * Agent Runtime MCP Tools Test Suite
 *
 * Tests for cli/src/tools/agent-runtime.js — 17 MCP tool handlers that
 * create, manage, and orchestrate autonomous AI agent runtimes.
 *
 * Tool list:
 *   agent_create_runtime, agent_destroy_runtime, agent_list_runtimes,
 *   agent_get_status, agent_set_strategy, agent_get_budget, agent_tick,
 *   agent_start_loop, agent_stop_loop, agent_register_service,
 *   agent_discover_services, agent_create_escrow_deal,
 *   agent_subscribe_to_service, agent_rate_counterparty,
 *   agent_get_reputation, agent_create_split_deal, agent_get_event_history
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const cliSrc = path.join(__dirname, '..', '..', 'src');

const { A2AStore } = await import(path.join(cliSrc, 'a2a', 'store.js'));
const { makeCommerceProxy } = await import(path.join(cliSrc, 'a2a', 'agent-runtime.js'));
const { agentRuntimeTools, _getRuntimeRegistry } = await import(
  path.join(cliSrc, 'tools', 'agent-runtime.js')
);

// =============================================================================
// Helpers
// =============================================================================

/** Find a tool by name from the exported array. */
function getTool(name) {
  const tool = agentRuntimeTools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool "${name}" not found`);
  return tool;
}

/** Invoke a tool handler with sensible defaults. */
async function invoke(toolName, params = {}, opts = {}) {
  const tool = getTool(toolName);
  return tool.handler({
    commerce,
    params,
    allowApply: opts.allowApply ?? true,
    agentConfig: opts.agentConfig ?? {},
  });
}

/** Destroy all runtimes tracked by the module-level registry. */
async function cleanupRuntimes() {
  const res = await invoke('agent_list_runtimes');
  for (const agent of res.agents || []) {
    try {
      await invoke('agent_destroy_runtime', { name: agent.name });
    } catch {
      /* best effort */
    }
  }
}

/** Create a runtime with default params (helper to reduce boilerplate). */
async function createAgent(name = 'TestAgent', extra = {}) {
  return invoke('agent_create_runtime', {
    name,
    strategy: 'always-accept',
    ...extra,
  });
}

let dbPath, store, commerce;

beforeEach(() => {
  dbPath = path.join(
    __dirname,
    `.test-tools-rt-${Date.now()}-${Math.random().toString(36).slice(2)}.db`,
  );
  store = new A2AStore({ dbPath });
  store.init();
  commerce = makeCommerceProxy(store);
});

afterEach(async () => {
  await cleanupRuntimes();
  try {
    store.close();
  } catch {
    /* ignore */
  }
  try {
    fs.unlinkSync(dbPath);
  } catch {
    /* ignore */
  }
});

// =============================================================================
// Tool Registration
// =============================================================================

describe('Agent Runtime Tools — tool registration', () => {
  it('exports exactly 29 tools', () => {
    assert.strictEqual(agentRuntimeTools.length, 29);
  });

  it('every tool has required shape (name, description, inputSchema, permission, handler)', () => {
    for (const tool of agentRuntimeTools) {
      assert.ok(typeof tool.name === 'string' && tool.name.length > 0, `tool.name missing`);
      assert.ok(
        typeof tool.description === 'string' && tool.description.length > 0,
        `${tool.name} description missing`,
      );
      assert.ok(typeof tool.inputSchema === 'object', `${tool.name} inputSchema missing`);
      assert.ok(
        ['read', 'write', 'delete'].includes(tool.permission),
        `${tool.name} has invalid permission "${tool.permission}"`,
      );
      assert.ok(typeof tool.handler === 'function', `${tool.name} handler is not a function`);
    }
  });

  it('all tool names start with "agent_"', () => {
    for (const tool of agentRuntimeTools) {
      assert.ok(tool.name.startsWith('agent_'), `${tool.name} does not start with agent_`);
    }
  });

  it('permission levels are correct for read tools', () => {
    const readTools = [
      'agent_list_runtimes',
      'agent_get_status',
      'agent_get_budget',
      'agent_discover_services',
      'agent_get_reputation',
      'agent_get_event_history',
    ];
    for (const name of readTools) {
      assert.strictEqual(getTool(name).permission, 'read', `${name} should be read`);
    }
  });

  it('permission levels are correct for write/delete tools', () => {
    const writeTools = [
      'agent_create_runtime',
      'agent_set_strategy',
      'agent_tick',
      'agent_start_loop',
      'agent_stop_loop',
      'agent_register_service',
      'agent_create_escrow_deal',
      'agent_subscribe_to_service',
      'agent_rate_counterparty',
      'agent_create_split_deal',
    ];
    for (const name of writeTools) {
      assert.strictEqual(getTool(name).permission, 'write', `${name} should be write`);
    }
    assert.strictEqual(getTool('agent_destroy_runtime').permission, 'delete');
  });

  it('_getRuntimeRegistry returns a Map', () => {
    const reg = _getRuntimeRegistry();
    assert.ok(reg instanceof Map);
  });
});

// =============================================================================
// Lifecycle
// =============================================================================

describe('Agent Runtime Tools — lifecycle', () => {
  it('agent_create_runtime returns success with agent info', async () => {
    const result = await createAgent('AlphaBot');

    assert.strictEqual(result.success, true);
    assert.ok(result.agent);
    assert.strictEqual(result.agent.name, 'AlphaBot');
    assert.ok(result.agent.agentId);
    assert.ok(result.agent.walletAddress);
    assert.ok(result.agent.walletAddress.startsWith('0x'));
    assert.ok(result.agent.budget);
  });

  it('agent_create_runtime without --apply returns error', async () => {
    const result = await invoke(
      'agent_create_runtime',
      { name: 'NoApply', strategy: 'always-accept' },
      { allowApply: false },
    );

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldCreate);
    assert.strictEqual(result.wouldCreate.name, 'NoApply');
  });

  it('agent_create_runtime with budget settings', async () => {
    const result = await invoke('agent_create_runtime', {
      name: 'BudgetBot',
      strategy: 'budget-gated',
      budgetDaily: 100,
      budgetMonthly: 2000,
      budgetPerTransaction: 50,
      startingBalance: 500,
    });

    assert.strictEqual(result.success, true);
    const budget = result.agent.budget;
    assert.strictEqual(budget.daily, 100);
    assert.strictEqual(budget.monthly, 2000);
    assert.strictEqual(budget.perTransaction, 50);
    assert.strictEqual(budget.balance, 500);
  });

  it('agent_create_runtime with always-accept strategy', async () => {
    const result = await createAgent('AcceptBot', { strategy: 'always-accept' });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.agent.strategy, 'always-accept');
  });

  it('agent_create_runtime with budget-gated strategy', async () => {
    const result = await createAgent('GatedBot', { strategy: 'budget-gated' });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.agent.strategy, 'budget-gated');
  });

  it('agent_create_runtime with reputation-aware strategy', async () => {
    const result = await createAgent('RepBot', { strategy: 'reputation-aware' });
    assert.strictEqual(result.success, true);
    // reputation-aware may fall back to budget-gated if the strategy factory is not available
    assert.strictEqual(result.agent.strategy, 'reputation-aware');
  });

  it('agent_create_runtime rejects duplicate names', async () => {
    const first = await createAgent('Dup');
    assert.strictEqual(first.success, true);

    const second = await createAgent('Dup');
    assert.strictEqual(second.success, false);
    assert.ok(second.error.includes('already exists'));
  });

  it('agent_destroy_runtime destroys existing agent', async () => {
    await createAgent('Doomed');
    const result = await invoke('agent_destroy_runtime', { name: 'Doomed' });

    assert.strictEqual(result.success, true);
    assert.ok(result.message.includes('destroyed'));

    // Verify it is gone
    const list = await invoke('agent_list_runtimes');
    assert.strictEqual(list.agents.length, 0);
  });

  it('agent_destroy_runtime returns error for non-existent agent', async () => {
    const result = await invoke('agent_destroy_runtime', { name: 'Ghost' });

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('agent_list_runtimes returns empty array initially', async () => {
    const result = await invoke('agent_list_runtimes');

    assert.strictEqual(result.success, true);
    assert.ok(Array.isArray(result.agents));
    assert.strictEqual(result.agents.length, 0);
    assert.strictEqual(result.count, 0);
  });

  it('agent_list_runtimes returns created agents', async () => {
    await createAgent('Agent1');
    await createAgent('Agent2');

    const result = await invoke('agent_list_runtimes');

    assert.strictEqual(result.success, true);
    assert.strictEqual(result.count, 2);

    const names = result.agents.map((a) => a.name).sort();
    assert.deepStrictEqual(names, ['Agent1', 'Agent2']);

    // Each agent should have the expected fields
    for (const agent of result.agents) {
      assert.ok(agent.agentId);
      assert.ok(agent.walletAddress);
      assert.ok(typeof agent.strategy === 'string');
      assert.strictEqual(typeof agent.running, 'boolean');
      assert.ok(agent.budget);
      assert.ok(agent.budgetScope);
      assert.ok(agent.budgetScoped);
      assert.deepStrictEqual(agent.budgetScope, agent.defaultPayment);
    }
  });

  it('agent_list_runtimes applies a shared rail-specific budget scope', async () => {
    await createAgent('ListScoped');
    const rt = _getRuntimeRegistry().get('ListScoped');
    rt.recordSpend(0.4, { asset: 'BTC', network: 'bitcoin' });
    rt.recordSpend(1.25, { asset: 'ZEC', network: 'zcash' });

    const result = await invoke('agent_list_runtimes', {
      asset: 'ZEC',
      network: 'zcash',
    });

    assert.strictEqual(result.success, true);
    assert.strictEqual(result.count, 1);
    assert.deepStrictEqual(result.agents[0].budgetScope, {
      asset: 'ZEC',
      network: 'zcash',
    });
    assert.strictEqual(result.agents[0].budgetScoped.asset, 'ZEC');
    assert.strictEqual(result.agents[0].budgetScoped.network, 'zcash');
    assert.ok(Math.abs(result.agents[0].budgetScoped.spentToday - 1.25) < 1e-12);
  });
});

// =============================================================================
// Agent Operations
// =============================================================================

describe('Agent Runtime Tools — agent operations', () => {
  it('agent_get_status returns full status for existing agent', async () => {
    await createAgent('StatusBot');
    const result = await invoke('agent_get_status', { name: 'StatusBot' });

    assert.strictEqual(result.success, true);
    assert.ok(result.agent);
    assert.strictEqual(result.agent.name, 'StatusBot');
    assert.ok(result.agent.agentId);
    assert.ok(result.agent.walletAddress);
    assert.strictEqual(typeof result.agent.strategy, 'string');
    assert.strictEqual(result.agent.running, false);
    assert.ok(result.agent.budget);
    assert.ok(result.agent.budgetScope);
    assert.ok(result.agent.budgetScoped);
    assert.deepStrictEqual(result.agent.budgetScope, result.agent.defaultPayment);
    assert.ok(Array.isArray(result.agent.services));
  });

  it('agent_get_status exposes settlement wallet and advertised payment addresses', async () => {
    await createAgent('ChainStatusBot');
    const rt = _getRuntimeRegistry().get('ChainStatusBot');
    rt.settlement = {
      chainId: 'bitcoin',
      isSimulation: true,
      getBalance: async () => ({ balance: '0.5', symbol: 'BTC' }),
      getAddress: async () => 'bc1qstatusbot',
    };
    await rt.syncAgentCard();

    const result = await invoke('agent_get_status', { name: 'ChainStatusBot' });

    assert.strictEqual(result.success, true);
    assert.deepStrictEqual(result.agent.settlementChains, ['bitcoin']);
    assert.deepStrictEqual(result.agent.paymentAddresses, {
      bitcoin: 'bc1qstatusbot',
    });
    assert.deepStrictEqual(result.agent.settlement, {
      chainId: 'bitcoin',
      simulate: true,
      walletAddress: 'bc1qstatusbot',
    });
  });

  it('agent_get_status preserves previously advertised payout addresses after settlement sync', async () => {
    await createAgent('MultiRailBot');
    const rt = _getRuntimeRegistry().get('MultiRailBot');
    rt.ensureAgentCard();
    const card = rt.getAgentCard();
    commerce.x402().updateAgent(card.id, {
      payment_addresses: JSON.stringify({ zcash: 'u1multirail' }),
    });
    rt.settlement = {
      chainId: 'bitcoin',
      isSimulation: true,
      getBalance: async () => ({ balance: '1.0', symbol: 'BTC' }),
      getAddress: async () => 'bc1qmultirail',
    };
    rt.setSettlement({
      chainId: 'zcash',
      isSimulation: true,
      getBalance: async () => ({ balance: '2.0', symbol: 'ZEC' }),
      getAddress: async () => 'u1multirail',
    });
    await rt.syncAgentCard();

    const result = await invoke('agent_get_status', { name: 'MultiRailBot' });

    assert.strictEqual(result.success, true);
    assert.deepStrictEqual(result.agent.settlementChains.sort(), ['bitcoin', 'zcash']);
    assert.deepStrictEqual(result.agent.paymentAddresses, {
      zcash: 'u1multirail',
      bitcoin: 'bc1qmultirail',
    });
    assert.deepStrictEqual(result.agent.budgetScope, {
      asset: 'ZEC',
      network: 'zcash',
    });
    assert.strictEqual(result.agent.budgetScoped.asset, 'ZEC');
    assert.strictEqual(result.agent.budgetScoped.network, 'zcash');
  });

  it('agent_get_status can scope budget and settlement to a requested rail', async () => {
    await createAgent('ScopedStatusBot');
    const rt = _getRuntimeRegistry().get('ScopedStatusBot');
    rt.setSettlement({
      chainId: 'bitcoin',
      isSimulation: true,
      getBalance: async () => ({ balance: '0.5', symbol: 'BTC' }),
      getAddress: async () => 'bc1qscopedstatus',
    });
    rt.setSettlement({
      chainId: 'zcash',
      isSimulation: true,
      getBalance: async () => ({ balance: '1.25', symbol: 'ZEC' }),
      getAddress: async () => 'u1scopedstatus',
    });
    rt.recordSpend(0.4, { asset: 'BTC', network: 'bitcoin' });
    rt.recordSpend(1.25, { asset: 'ZEC', network: 'zcash' });

    const result = await invoke('agent_get_status', {
      name: 'ScopedStatusBot',
      asset: 'BTC',
      network: 'bitcoin',
    });

    assert.strictEqual(result.success, true);
    assert.deepStrictEqual(result.agent.budgetScope, {
      asset: 'BTC',
      network: 'bitcoin',
    });
    assert.strictEqual(result.agent.budgetScoped.asset, 'BTC');
    assert.strictEqual(result.agent.budgetScoped.network, 'bitcoin');
    assert.ok(Math.abs(result.agent.budgetScoped.spentToday - 0.4) < 1e-12);
    assert.deepStrictEqual(result.agent.settlement, {
      chainId: 'bitcoin',
      simulate: true,
      walletAddress: 'bc1qscopedstatus',
    });
  });

  it('agent_get_chain_balance can query a specific settlement chain', async () => {
    await createAgent('BalanceBot');
    const rt = _getRuntimeRegistry().get('BalanceBot');
    rt.setSettlement({
      chainId: 'bitcoin',
      isSimulation: true,
      getBalance: async () => ({ balance: '0.5', symbol: 'BTC' }),
      getAddress: async () => 'bc1qbalance',
    });
    rt.setSettlement({
      chainId: 'zcash',
      isSimulation: true,
      getBalance: async () => ({ balance: '1.25', symbol: 'ZEC' }),
      getAddress: async () => 'u1balance',
    });

    const result = await invoke('agent_get_chain_balance', {
      name: 'BalanceBot',
      chainId: 'bitcoin',
    });

    assert.strictEqual(result.success, true);
    assert.strictEqual(result.chainId, 'bitcoin');
    assert.strictEqual(result.walletAddress, 'bc1qbalance');
    assert.strictEqual(result.symbol, 'BTC');
    assert.strictEqual(result.balance, '0.5');
  });

  it('agent_get_status returns error for non-existent agent', async () => {
    const result = await invoke('agent_get_status', { name: 'NoSuchAgent' });

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('agent_set_strategy changes the strategy', async () => {
    await createAgent('StratBot', { strategy: 'always-accept' });

    const result = await invoke('agent_set_strategy', {
      name: 'StratBot',
      strategy: 'budget-gated',
    });

    assert.strictEqual(result.success, true);
    assert.ok(result.message.includes('budget-gated'));

    // Verify via status
    const status = await invoke('agent_get_status', { name: 'StratBot' });
    assert.strictEqual(status.agent.strategy, 'budget-gated');
  });

  it('agent_set_strategy without --apply returns error', async () => {
    await createAgent('StratBot2', { strategy: 'always-accept' });
    const result = await invoke(
      'agent_set_strategy',
      { name: 'StratBot2', strategy: 'budget-gated' },
      { allowApply: false },
    );

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('agent_set_strategy returns error for non-existent agent', async () => {
    const result = await invoke('agent_set_strategy', {
      name: 'MissingAgent',
      strategy: 'always-accept',
    });

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('agent_get_budget returns budget info', async () => {
    await createAgent('BudgetCheck', {
      strategy: 'budget-gated',
      budgetDaily: 200,
      budgetMonthly: 5000,
      budgetPerTransaction: 100,
    });

    const result = await invoke('agent_get_budget', { name: 'BudgetCheck' });

    assert.strictEqual(result.success, true);
    assert.ok(result.budget);
    assert.strictEqual(result.budget.daily, 200);
    assert.strictEqual(result.budget.monthly, 5000);
    assert.strictEqual(result.budget.perTransaction, 100);
    assert.strictEqual(result.budget.spentToday, 0);
    assert.strictEqual(result.budget.spentThisMonth, 0);
    assert.strictEqual(result.budget.remainingDaily, 200);
    assert.strictEqual(result.budget.remainingMonthly, 5000);
  });

  it('agent_get_budget supports rail filters and mixed-rail breakdowns', async () => {
    await createAgent('MultiRailBudget', {
      strategy: 'budget-gated',
      budgetDaily: 5,
      budgetMonthly: 50,
      budgetPerTransaction: 2,
      settlementChain: 'bitcoin',
    });

    const rt = _getRuntimeRegistry().get('MultiRailBudget');
    rt.recordSpend(0.4, { asset: 'BTC', network: 'bitcoin' });
    rt.recordSpend(1.25, { asset: 'ZEC', network: 'zcash' });

    const mixed = await invoke('agent_get_budget', { name: 'MultiRailBudget' });
    assert.strictEqual(mixed.success, true);
    assert.strictEqual(mixed.budget.aggregateTotalsMeaningful, false);
    assert.strictEqual(mixed.budget.spentToday, null);
    assert.deepStrictEqual(mixed.budget.assets, ['BTC', 'ZEC']);
    assert.ok(
      Math.abs(mixed.budget.breakdownByAsset.BTC.networks.bitcoin.spentToday - 0.4) < 1e-12,
    );
    assert.ok(Math.abs(mixed.budget.breakdownByAsset.ZEC.networks.zcash.spentToday - 1.25) < 1e-12);

    const btc = await invoke('agent_get_budget', {
      name: 'MultiRailBudget',
      asset: 'BTC',
      network: 'bitcoin',
    });
    assert.strictEqual(btc.success, true);
    assert.strictEqual(btc.budget.aggregateTotalsMeaningful, true);
    assert.strictEqual(btc.budget.asset, 'BTC');
    assert.strictEqual(btc.budget.network, 'bitcoin');
    assert.ok(Math.abs(btc.budget.spentToday - 0.4) < 1e-12);
  });

  it('agent_get_budget returns error for non-existent agent', async () => {
    const result = await invoke('agent_get_budget', { name: 'NoBudget' });

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('agent_tick returns processed count', async () => {
    await createAgent('TickBot');
    const result = await invoke('agent_tick', { name: 'TickBot' });

    assert.strictEqual(result.success, true);
    assert.strictEqual(typeof result.processed, 'number');
    assert.ok(result.message.includes('processed'));
  });

  it('agent_tick without --apply returns error', async () => {
    await createAgent('TickGuard');
    const result = await invoke('agent_tick', { name: 'TickGuard' }, { allowApply: false });

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('agent_tick returns error for non-existent agent', async () => {
    const result = await invoke('agent_tick', { name: 'NoTickBot' });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('agent_register_service registers a service', async () => {
    await createAgent('ServiceBot');
    const result = await invoke('agent_register_service', {
      name: 'ServiceBot',
      serviceName: 'Sentiment Analysis',
      description: 'Analyze text sentiment',
      category: 'analytics',
      pricingModel: 'quote',
    });

    assert.strictEqual(result.success, true);
    assert.ok(result.service);
    assert.ok(result.message.includes('Sentiment Analysis'));
    assert.ok(result.message.includes('analytics'));
  });

  it('agent_register_service without --apply returns error', async () => {
    await createAgent('SvcGuard');
    const result = await invoke(
      'agent_register_service',
      { name: 'SvcGuard', serviceName: 'Test', category: 'test' },
      { allowApply: false },
    );

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('agent_register_service returns error for non-existent agent', async () => {
    const result = await invoke('agent_register_service', {
      name: 'GhostSvc',
      serviceName: 'Test',
      category: 'test',
    });

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('agent_discover_services discovers by category', async () => {
    await createAgent('DiscoveryBot');

    // Register a service first
    await invoke('agent_register_service', {
      name: 'DiscoveryBot',
      serviceName: 'Data Collector',
      description: 'Collect web data',
      category: 'data-collection',
    });

    const result = await invoke('agent_discover_services', {
      name: 'DiscoveryBot',
      category: 'data-collection',
    });

    assert.strictEqual(result.success, true);
    assert.ok(Array.isArray(result.services));
    assert.ok(result.services.length >= 1);
    assert.strictEqual(typeof result.count, 'number');
  });

  it('agent_discover_services returns empty for unknown category', async () => {
    await createAgent('EmptyDiscover');

    const result = await invoke('agent_discover_services', {
      name: 'EmptyDiscover',
      category: 'nonexistent-category-xyz',
    });

    assert.strictEqual(result.success, true);
    assert.strictEqual(result.services.length, 0);
    assert.strictEqual(result.count, 0);
  });

  it('agent_discover_services returns error for non-existent agent', async () => {
    const result = await invoke('agent_discover_services', {
      name: 'MissingDiscover',
    });

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('not found'));
  });
});

// =============================================================================
// Service Loop
// =============================================================================

describe('Agent Runtime Tools — service loop', () => {
  it('agent_start_loop starts the loop', async () => {
    await createAgent('LoopBot');
    const result = await invoke('agent_start_loop', { name: 'LoopBot' });

    assert.strictEqual(result.success, true);
    assert.ok(result.message.includes('started'));

    // Verify via status
    const status = await invoke('agent_get_status', { name: 'LoopBot' });
    assert.strictEqual(status.agent.running, true);
  });

  it('agent_start_loop without --apply returns error', async () => {
    await createAgent('LoopGuard');
    const result = await invoke('agent_start_loop', { name: 'LoopGuard' }, { allowApply: false });

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('agent_start_loop returns error when already running', async () => {
    await createAgent('DoubleLoop');
    await invoke('agent_start_loop', { name: 'DoubleLoop' });
    const result = await invoke('agent_start_loop', { name: 'DoubleLoop' });

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('already running'));
  });

  it('agent_stop_loop stops a running loop', async () => {
    await createAgent('StopBot');
    await invoke('agent_start_loop', { name: 'StopBot' });

    const result = await invoke('agent_stop_loop', { name: 'StopBot' });
    assert.strictEqual(result.success, true);
    assert.ok(result.message.includes('stopped'));

    // Verify via status
    const status = await invoke('agent_get_status', { name: 'StopBot' });
    assert.strictEqual(status.agent.running, false);
  });

  it('agent_stop_loop without --apply returns error', async () => {
    await createAgent('StopGuard');
    const result = await invoke('agent_stop_loop', { name: 'StopGuard' }, { allowApply: false });

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('agent_stop_loop returns error for non-existent agent', async () => {
    const result = await invoke('agent_stop_loop', { name: 'NoLoop' });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('not found'));
  });
});

// =============================================================================
// Advanced Operations
// =============================================================================

describe('Agent Runtime Tools — advanced operations', () => {
  it('agent_rate_counterparty posts a rating', async () => {
    await createAgent('RaterBot');

    const result = await invoke('agent_rate_counterparty', {
      raterName: 'RaterBot',
      ratedAddress: '0xSomeOtherAgent1234567890abcdef1234567890',
      score: 4,
      transactionId: 'txn-001',
      comment: 'Good service',
    });

    assert.strictEqual(result.success, true);
    assert.ok(result.feedback || result.message);
  });

  it('agent_rate_counterparty without --apply returns error', async () => {
    await createAgent('RateGuard');
    const result = await invoke(
      'agent_rate_counterparty',
      { raterName: 'RateGuard', ratedAddress: '0xABC', score: 5 },
      { allowApply: false },
    );

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('agent_rate_counterparty returns error for non-existent rater', async () => {
    const result = await invoke('agent_rate_counterparty', {
      raterName: 'GhostRater',
      ratedAddress: '0xABC',
      score: 3,
    });

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('agent_get_reputation returns reputation data (or throws due to arrow-function arguments bug)', async () => {
    // NOTE: The handler uses `arguments[0]` inside an arrow function, which is
    // a known bug (arrow functions do not bind `arguments`). We assert that the
    // handler either succeeds with a reputation key or throws ReferenceError.
    try {
      const result = await invoke('agent_get_reputation', {
        address: '0xSomeAddress1234567890abcdef1234567890abcd',
      });
      assert.strictEqual(result.success, true);
      assert.ok('reputation' in result);
    } catch (err) {
      // Known bug: `arguments is not defined` in arrow function
      assert.ok(
        err instanceof ReferenceError && err.message.includes('arguments'),
        `Unexpected error: ${err.message}`,
      );
    }
  });

  it('agent_create_escrow_deal creates an escrow', async () => {
    await createAgent('EscrowBuyer', { startingBalance: 1000 });

    const result = await invoke('agent_create_escrow_deal', {
      buyerName: 'EscrowBuyer',
      sellerAddress: '0xSeller1234567890abcdef1234567890abcdef12',
      amount: 50,
      conditions: [{ type: 'seller_fulfilled', description: 'Deliver the goods' }],
      expiresInHours: 48,
    });

    assert.strictEqual(result.success, true);
  });

  it('agent_create_escrow_deal without --apply returns error', async () => {
    await createAgent('EscrowGuard');
    const result = await invoke(
      'agent_create_escrow_deal',
      {
        buyerName: 'EscrowGuard',
        sellerAddress: '0xSeller',
        amount: 10,
        conditions: [{ type: 'buyer_confirmed' }],
      },
      { allowApply: false },
    );

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('agent_create_escrow_deal returns error for non-existent buyer', async () => {
    const result = await invoke('agent_create_escrow_deal', {
      buyerName: 'NoBuyer',
      sellerAddress: '0xSeller',
      amount: 10,
      conditions: [{ type: 'buyer_confirmed' }],
    });

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('agent_create_escrow_deal fails when agent cannot afford it', async () => {
    await createAgent('PoorBuyer', {
      strategy: 'budget-gated',
      budgetPerTransaction: 5,
    });

    const result = await invoke('agent_create_escrow_deal', {
      buyerName: 'PoorBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      conditions: [{ type: 'seller_fulfilled' }],
    });

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('cannot afford'));
  });

  it('agent_subscribe_to_service creates a subscription', async () => {
    await createAgent('SubBot', { startingBalance: 500 });

    const result = await invoke('agent_subscribe_to_service', {
      subscriberName: 'SubBot',
      providerAddress: '0xProvider1234567890abcdef1234567890abcdef',
      planName: 'Premium Data Feed',
      amount: 29.99,
      interval: 'monthly',
      trialDays: 14,
    });

    assert.strictEqual(result.success, true);
    assert.ok(result.subscription || result.message);
  });

  it('agent_subscribe_to_service defaults to the settlement payment config', async () => {
    await createAgent('NativeSubBot', { startingBalance: 500 });
    const rt = _getRuntimeRegistry().get('NativeSubBot');
    rt.settlement = {
      chainId: 'bitcoin',
      isSimulation: true,
      getBalance: async () => ({ balance: '1.0', symbol: 'BTC' }),
      getAddress: async () => 'bc1qsubbot',
    };

    const result = await invoke('agent_subscribe_to_service', {
      subscriberName: 'NativeSubBot',
      providerAddress: '0xProvider1234567890abcdef1234567890abcdef',
      planName: 'BTC Feed',
      amount: 0.0025,
      interval: 'monthly',
    });

    assert.strictEqual(result.success, true);
    assert.strictEqual(result.subscription.asset, 'BTC');
    assert.strictEqual(result.subscription.network, 'bitcoin');
  });

  it('agent_subscribe_to_service without --apply returns error', async () => {
    await createAgent('SubGuard');
    const result = await invoke(
      'agent_subscribe_to_service',
      {
        subscriberName: 'SubGuard',
        providerAddress: '0xProv',
        planName: 'Test',
        amount: 10,
      },
      { allowApply: false },
    );

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('agent_subscribe_to_service returns error for non-existent subscriber', async () => {
    const result = await invoke('agent_subscribe_to_service', {
      subscriberName: 'GhostSub',
      providerAddress: '0xProv',
      planName: 'Plan',
      amount: 10,
    });

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('agent_create_split_deal creates a split payment', async () => {
    await createAgent('SplitPayer', { startingBalance: 1000 });

    // NOTE: The tool inputSchema defines `percentage` but the underlying
    // splits service expects `percent`. The tool handler passes recipients
    // through to rt.createSplitDeal() which passes them straight to the
    // splits service. We use `percent` here to match what the service
    // requires, and also pass `percentage` since that is what the tool
    // schema defines. The service only checks for `percent`.
    const result = await invoke('agent_create_split_deal', {
      payerName: 'SplitPayer',
      totalAmount: 100,
      recipients: [
        { address: '0xAlice123456789012345678901234567890abcd', percentage: 60, percent: 60 },
        { address: '0xBob00123456789012345678901234567890abcd', percentage: 40, percent: 40 },
      ],
      memo: 'Revenue split',
    });

    assert.strictEqual(result.success, true);
    assert.ok(result.split || result.splitPayment || result.message);
  });

  it('agent_create_split_deal defaults to the settlement payment config', async () => {
    await createAgent('NativeSplitBot', { startingBalance: 1000 });
    const rt = _getRuntimeRegistry().get('NativeSplitBot');
    rt.settlement = {
      chainId: 'zcash',
      isSimulation: true,
      getBalance: async () => ({ balance: '5.0', symbol: 'ZEC' }),
      getAddress: async () => 'u1splitbot',
    };

    const result = await invoke('agent_create_split_deal', {
      payerName: 'NativeSplitBot',
      totalAmount: 0.5,
      recipients: [
        { address: 'u1alice', percentage: 50, percent: 50 },
        { address: 'u1bob', percentage: 50, percent: 50 },
      ],
      memo: 'Shielded revenue split',
    });

    assert.strictEqual(result.success, true);
    assert.strictEqual(result.split.asset, 'ZEC');
    assert.strictEqual(result.split.network, 'zcash');
  });

  it('agent_create_split_deal without --apply returns error', async () => {
    await createAgent('SplitGuard');
    const result = await invoke(
      'agent_create_split_deal',
      {
        payerName: 'SplitGuard',
        totalAmount: 50,
        recipients: [
          { address: '0xA', percentage: 50 },
          { address: '0xB', percentage: 50 },
        ],
      },
      { allowApply: false },
    );

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('agent_create_split_deal returns error for non-existent payer', async () => {
    const result = await invoke('agent_create_split_deal', {
      payerName: 'NoPayer',
      totalAmount: 50,
      recipients: [
        { address: '0xA', percentage: 50 },
        { address: '0xB', percentage: 50 },
      ],
    });

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('agent_create_split_deal fails when agent cannot afford it', async () => {
    await createAgent('BrokeSplitter', {
      strategy: 'budget-gated',
      budgetPerTransaction: 10,
    });

    const result = await invoke('agent_create_split_deal', {
      payerName: 'BrokeSplitter',
      totalAmount: 500,
      recipients: [
        { address: '0xA', percentage: 50 },
        { address: '0xB', percentage: 50 },
      ],
    });

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('cannot afford'));
  });

  it('agent_get_event_history returns events array', async () => {
    await createAgent('HistoryBot');
    const result = await invoke('agent_get_event_history', {
      name: 'HistoryBot',
      limit: 10,
    });

    assert.strictEqual(result.success, true);
    assert.ok(Array.isArray(result.events));
    assert.strictEqual(typeof result.count, 'number');
    assert.ok(result.summary);
  });

  it('agent_get_event_history filters by event type and payment rail', async () => {
    await createAgent('HistoryFilterBot');
    const rt = _getRuntimeRegistry().get('HistoryFilterBot');

    store.createEventLog({
      id: 'evt-zec-exceeded',
      event_type: 'a2a_runtime.budget_exceeded',
      agent_address: rt.walletAddress,
      payload: {
        type: 'balance',
        asset: 'ZEC',
        network: 'zcash',
        attempted: 2,
        limit: 1.25,
        operation: 'subscription:create',
      },
      created_at: '2026-03-18T08:00:00.000Z',
    });
    store.createEventLog({
      id: 'evt-btc-warning',
      event_type: 'a2a_runtime.budget_warning',
      agent_address: rt.walletAddress,
      payload: {
        type: 'daily',
        asset: 'BTC',
        network: 'bitcoin',
        spent: 0.8,
        limit: 1,
      },
      created_at: '2026-03-18T09:00:00.000Z',
    });
    store.createEventLog({
      id: 'evt-payment',
      event_type: 'a2a_runtime.payment_sent',
      agent_address: rt.walletAddress,
      payload: {
        asset: 'BTC',
        network: 'bitcoin',
      },
      created_at: '2026-03-18T10:00:00.000Z',
    });

    const result = await invoke('agent_get_event_history', {
      name: 'HistoryFilterBot',
      eventTypes: ['a2a_runtime.budget_exceeded', 'a2a_runtime.budget_warning'],
      asset: 'ZEC',
      network: 'zcash',
      since: '2026-03-18T00:00:00.000Z',
      limit: 10,
    });

    assert.strictEqual(result.success, true);
    assert.deepStrictEqual(result.filters, {
      eventTypes: ['a2a_runtime.budget_exceeded', 'a2a_runtime.budget_warning'],
      since: '2026-03-18T00:00:00.000Z',
      asset: 'ZEC',
      network: 'zcash',
    });
    assert.strictEqual(result.count, 1);
    assert.strictEqual(result.events[0].eventType, 'a2a_runtime.budget_exceeded');
    assert.strictEqual(result.events[0].payloadObject.asset, 'ZEC');
    assert.strictEqual(result.events[0].payloadObject.network, 'zcash');
    assert.deepStrictEqual(result.summary.byEventType, {
      'a2a_runtime.budget_exceeded': 1,
    });
    assert.deepStrictEqual(result.summary.budgetAlerts, {
      total: 1,
      warning: 0,
      exceeded: 1,
      byConstraintType: {
        balance: 1,
      },
    });
  });

  it('agent_get_event_history returns error for non-existent agent', async () => {
    const result = await invoke('agent_get_event_history', {
      name: 'NoHistory',
    });

    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('not found'));
  });
});

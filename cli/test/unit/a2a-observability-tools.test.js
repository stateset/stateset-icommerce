import { afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { a2aObservabilityTools } from '../../src/tools/a2a-observability.js';
import { _getRuntimeRegistry } from '../../src/tools/agent-runtime.js';

function findTool(name) {
  const tool = a2aObservabilityTools.find((candidate) => candidate.name === name);
  assert.ok(tool, `Observability tool ${name} not found`);
  return tool;
}

afterEach(() => {
  _getRuntimeRegistry().clear();
});

describe('a2aObservabilityTools', () => {
  it('a2a_agent_dashboard includes runtime and filtered economics context', async () => {
    _getRuntimeRegistry().set('dashboard-runtime', {
      walletAddress: '0xAlice',
      isRunning() {
        return true;
      },
      getBudget(scope) {
        const base = { daily: 1, monthly: 5, perTransaction: 0.5, spentToday: 0.1 };
        if (scope && (scope.asset || scope.network)) {
          return { ...base, asset: scope.asset || null, network: scope.network || null };
        }
        return base;
      },
      getDefaultPaymentConfig() {
        return { asset: 'BTC', network: 'bitcoin' };
      },
      listSettlementChains() {
        return ['bitcoin', 'zcash'];
      },
      async getChainWalletAddress(chainId) {
        return chainId === 'bitcoin' ? 'bc1qdashwallet' : null;
      },
    });

    let summaryFilter = null;
    let marginFilter = null;
    let operationFilter = null;
    let trendArgs = null;
    let anomalyFilter = null;
    const eventLogCalls = [];

    const commerce = {
      a2a() {
        return {
          listEventLog(filter) {
            eventLogCalls.push(filter);
            if (filter.event_type === 'a2a_runtime.budget_warning') {
              return [
                {
                  id: 'warn-btc',
                  event_type: filter.event_type,
                  agent_address: '0xAlice',
                  payload: JSON.stringify({
                    type: 'daily',
                    asset: 'BTC',
                    network: 'bitcoin',
                    spent: 0.8,
                    limit: 1,
                  }),
                  created_at: '2026-03-18T10:00:00.000Z',
                },
                {
                  id: 'warn-zec',
                  event_type: filter.event_type,
                  agent_address: '0xAlice',
                  payload: JSON.stringify({
                    type: 'daily',
                    asset: 'ZEC',
                    network: 'zcash',
                    spent: 1.8,
                    limit: 2,
                  }),
                  created_at: '2026-03-18T09:00:00.000Z',
                },
              ];
            }

            return [
              {
                id: 'exceed-btc',
                event_type: filter.event_type,
                agent_address: '0xAlice',
                payload: JSON.stringify({
                  type: 'perTransaction',
                  asset: 'BTC',
                  network: 'bitcoin',
                  attempted: 1.2,
                  limit: 1,
                  remaining: 1,
                  operation: 'split:create',
                }),
                created_at: '2026-03-18T11:00:00.000Z',
              },
            ];
          },
        };
      },
      _introspectionService: {
        getAgentDashboard(agentAddress) {
          return {
            agentAddress,
            runtimeStatus: 'running',
            lastTickAt: '2026-03-18T12:00:00.000Z',
          };
        },
      },
      _costAnalytics: {
        getAgentSpendSummary(agentAddress, filter) {
          summaryFilter = { agentAddress, filter };
          return { totalSpent: 0.01, assets: ['BTC'], aggregateAsset: 'BTC' };
        },
        getMarginAnalysis(agentAddress, filter) {
          marginFilter = { agentAddress, filter };
          return { grossMargin: -0.01, assets: ['BTC'], aggregateAsset: 'BTC' };
        },
        getOperationBreakdown(agentAddress, filter) {
          operationFilter = { agentAddress, filter };
          return [{ operation: 'quote_payment', totalAmount: 0.01 }];
        },
        getDailySpendTrend(agentAddress, days, filter) {
          trendArgs = { agentAddress, days, filter };
          return [{ date: '2026-03-18', spent: 0.01, earned: 0, net: -0.01 }];
        },
        detectAnomalies(agentAddress, filter) {
          anomalyFilter = { agentAddress, filter };
          return { transactionAnomalies: [{ id: 'tx1' }], dailyAnomalies: [] };
        },
      },
    };

    const result = await findTool('a2a_agent_dashboard').handler({
      commerce,
      params: {
        agentAddress: '0xAlice',
        asset: 'BTC',
        network: 'bitcoin',
        trendDays: 14,
      },
    });

    assert.equal(result.runtime.paymentConfig.asset, 'BTC');
    assert.equal(result.runtime.paymentConfig.network, 'bitcoin');
    assert.equal(result.runtime.chainWalletAddress, 'bc1qdashwallet');
    assert.equal(result.runtime.budget.daily, 1);
    assert.deepEqual(result.runtime.budgetScope, { asset: 'BTC', network: 'bitcoin' });
    assert.equal(result.runtime.budgetScoped.asset, 'BTC');
    assert.equal(result.runtime.budgetScoped.network, 'bitcoin');
    assert.deepEqual(result.runtime.budgetEvents.counts, {
      warning: 1,
      exceeded: 1,
      total: 2,
    });
    assert.deepEqual(result.runtime.budgetEvents.byConstraintType, {
      daily: 1,
      perTransaction: 1,
    });
    assert.equal(result.runtime.budgetEvents.latestExceededAt, '2026-03-18T11:00:00.000Z');
    assert.equal(result.runtime.budgetEvents.recent[0].severity, 'exceeded');
    assert.equal(result.runtime.budgetEvents.recent[0].operation, 'split:create');
    assert.deepEqual(result.runtime.settlementChains, ['bitcoin', 'zcash']);

    assert.equal(result.economics.filter.asset, 'BTC');
    assert.equal(result.economics.filter.network, 'bitcoin');
    assert.equal(result.economics.trendDays, 14);
    assert.equal(result.economics.anomalyCounts.transaction, 1);
    assert.equal(result.economics.summary.aggregateAsset, 'BTC');
    assert.equal(result.economics.margin.aggregateAsset, 'BTC');
    assert.deepEqual(result.economics.operationBreakdown, [{ operation: 'quote_payment', totalAmount: 0.01 }]);
    assert.deepEqual(result.economics.dailyTrend, [{ date: '2026-03-18', spent: 0.01, earned: 0, net: -0.01 }]);

    assert.deepEqual(summaryFilter, {
      agentAddress: '0xAlice',
      filter: { asset: 'BTC', network: 'bitcoin' },
    });
    assert.deepEqual(marginFilter, summaryFilter);
    assert.deepEqual(operationFilter, summaryFilter);
    assert.deepEqual(anomalyFilter, summaryFilter);
    assert.deepEqual(trendArgs, {
      agentAddress: '0xAlice',
      days: 14,
      filter: { asset: 'BTC', network: 'bitcoin' },
    });
    assert.deepEqual(eventLogCalls, [
      {
        agent_address: '0xAlice',
        event_type: 'a2a_runtime.budget_warning',
        since: result.runtime.budgetEvents.since,
        limit: 250,
      },
      {
        agent_address: '0xAlice',
        event_type: 'a2a_runtime.budget_exceeded',
        since: result.runtime.budgetEvents.since,
        limit: 250,
      },
    ]);
  });

  it('a2a_agent_performance adds runtime and economics context when available', async () => {
    _getRuntimeRegistry().set('performance-runtime', {
      walletAddress: '0xPerf',
      isRunning() {
        return false;
      },
      getBudget(scope) {
        const base = { daily: 2, monthly: 10, perTransaction: 1, spentToday: 0.2 };
        if (scope && (scope.asset || scope.network)) {
          return { ...base, asset: scope.asset || null, network: scope.network || null };
        }
        return base;
      },
      getDefaultPaymentConfig() {
        return { asset: 'ZEC', network: 'zcash' };
      },
      listSettlementChains() {
        return ['zcash'];
      },
      async getChainWalletAddress() {
        return 'u1perfwallet';
      },
    });

    const commerce = {
      a2a() {
        return {
          listEventLog(filter) {
            if (filter.event_type === 'a2a_runtime.budget_warning') {
              return [];
            }
            return [
              {
                id: 'perf-exceed',
                event_type: filter.event_type,
                agent_address: '0xPerf',
                payload: JSON.stringify({
                  type: 'balance',
                  asset: 'ZEC',
                  network: 'zcash',
                  attempted: 2,
                  limit: 1.25,
                  remaining: 1.25,
                  operation: 'subscription:create',
                }),
                created_at: '2026-03-18T08:00:00.000Z',
              },
            ];
          },
        };
      },
      _introspectionService: {
        getPerformanceReport(agentAddress) {
          return {
            agentAddress,
            quoteAcceptRate: 0.5,
            avgResponseTimeMs: 25,
            settlementSuccessRate: 1,
            disputeRate: 0,
            uptimePercent: 100,
          };
        },
      },
      _costAnalytics: {
        getAgentSpendSummary() {
          return { totalSpent: 1.25, assets: ['ZEC'], aggregateAsset: 'ZEC' };
        },
        getMarginAnalysis() {
          return { grossMargin: -1.25, assets: ['ZEC'], aggregateAsset: 'ZEC' };
        },
        getOperationBreakdown() {
          return [{ operation: 'subscription_billing', totalAmount: 1.25 }];
        },
        getDailySpendTrend() {
          return [{ date: '2026-03-18', spent: 1.25, earned: 0, net: -1.25 }];
        },
        detectAnomalies() {
          return { transactionAnomalies: [], dailyAnomalies: [{ date: '2026-03-18' }] };
        },
      },
    };

    const result = await findTool('a2a_agent_performance').handler({
      commerce,
      params: {
        agentAddress: '0xPerf',
        asset: 'ZEC',
        network: 'zcash',
        trendDays: 30,
      },
    });

    assert.equal(result.runtime.paymentConfig.asset, 'ZEC');
    assert.equal(result.runtime.chainWalletAddress, 'u1perfwallet');
    assert.deepEqual(result.runtime.budgetScope, { asset: 'ZEC', network: 'zcash' });
    assert.equal(result.runtime.budgetScoped.asset, 'ZEC');
    assert.equal(result.runtime.budgetScoped.network, 'zcash');
    assert.deepEqual(result.runtime.budgetEvents.counts, {
      warning: 0,
      exceeded: 1,
      total: 1,
    });
    assert.deepEqual(result.runtime.budgetEvents.byConstraintType, {
      balance: 1,
    });
    assert.equal(result.runtime.budgetEvents.recent[0].operation, 'subscription:create');
    assert.equal(result.economics.filter.asset, 'ZEC');
    assert.equal(result.economics.filter.network, 'zcash');
    assert.equal(result.economics.trendDays, 30);
    assert.equal(result.economics.summary.aggregateAsset, 'ZEC');
    assert.equal(result.economics.margin.aggregateAsset, 'ZEC');
    assert.deepEqual(result.economics.anomalyCounts, { transaction: 0, daily: 1 });
    assert.deepEqual(result.economics.dailyTrend, [{ date: '2026-03-18', spent: 1.25, earned: 0, net: -1.25 }]);
  });

  it('a2a_agent_alerts merges budget and settlement alerts with rail filters', async () => {
    const commerce = {
      a2a() {
        return {
          listEventLog(filter) {
            if (filter.event_type === 'a2a_runtime.budget_exceeded') {
              return [
                {
                  id: 'budget-zec',
                  event_type: filter.event_type,
                  agent_address: '0xAlertBot',
                  payload: JSON.stringify({
                    type: 'balance',
                    asset: 'ZEC',
                    network: 'zcash',
                    attempted: 2,
                    limit: 1.25,
                    operation: 'subscription:create',
                  }),
                  created_at: '2026-03-18T09:00:00.000Z',
                },
              ];
            }

            if (filter.event_type === 'a2a_runtime.budget_warning') {
              return [
                {
                  id: 'budget-btc-warning',
                  event_type: filter.event_type,
                  agent_address: '0xAlertBot',
                  payload: JSON.stringify({
                    type: 'daily',
                    asset: 'BTC',
                    network: 'bitcoin',
                    spent: 0.8,
                    limit: 1,
                  }),
                  created_at: '2026-03-18T08:00:00.000Z',
                },
              ];
            }

            if (filter.event_type === 'a2a_runtime.settlement_failed') {
              return [
                {
                  id: 'settlement-btc-failed',
                  event_type: filter.event_type,
                  agent_address: '0xAlertBot',
                  payload: JSON.stringify({
                    chainId: 'bitcoin',
                    error: 'No settlement service configured for network bitcoin',
                    phase: 'selection',
                    referenceType: 'quote',
                    referenceId: 'quote-1',
                  }),
                  created_at: '2026-03-18T11:00:00.000Z',
                },
              ];
            }

            if (filter.event_type === 'a2a_runtime.settlement_insufficient_funds') {
              return [
                {
                  id: 'settlement-btc-funds',
                  event_type: filter.event_type,
                  agent_address: '0xAlertBot',
                  payload: JSON.stringify({
                    chainId: 'bitcoin',
                    symbol: 'BTC',
                    required: 0.5,
                    available: 0.1,
                    paymentId: 'pay-1',
                  }),
                  created_at: '2026-03-18T10:00:00.000Z',
                },
              ];
            }

            return [];
          },
        };
      },
    };

    const result = await findTool('a2a_agent_alerts').handler({
      commerce,
      params: {
        agentAddress: '0xAlertBot',
        categories: ['budget', 'settlement'],
        asset: 'BTC',
        network: 'bitcoin',
        since: '2026-03-18T00:00:00.000Z',
        limit: 10,
      },
    });

    assert.equal(result.count, 3);
    assert.deepEqual(result.filters, {
      agentAddress: '0xAlertBot',
      categories: ['budget', 'settlement'],
      asset: 'BTC',
      network: 'bitcoin',
      since: '2026-03-18T00:00:00.000Z',
      limit: 10,
    });
    assert.deepEqual(result.summary.byCategory, {
      settlement: 2,
      budget: 1,
    });
    assert.deepEqual(result.summary.bySeverity, {
      failed: 1,
      insufficient_funds: 1,
      warning: 1,
    });
    assert.deepEqual(result.summary.budgetAlerts, {
      total: 1,
      warning: 1,
      exceeded: 0,
      byConstraintType: {
        daily: 1,
      },
    });
    assert.deepEqual(result.summary.settlementAlerts, {
      total: 2,
      failed: 1,
      insufficientFunds: 1,
      byPhase: {
        selection: 1,
        funding: 1,
      },
    });
    assert.equal(result.alerts[0].eventType, 'a2a_runtime.settlement_failed');
    assert.equal(result.alerts[0].network, 'bitcoin');
    assert.equal(result.alerts[1].severity, 'insufficient_funds');
    assert.equal(result.alerts[2].severity, 'warning');
  });
});

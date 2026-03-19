import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { a2aAutomationTools } from '../../src/tools/a2a-automation.js';

function findTool(name) {
  const tool = a2aAutomationTools.find((candidate) => candidate.name === name);
  assert.ok(tool, `Tool ${name} not found`);
  return tool;
}

describe('a2aAutomationTools', () => {
  it('a2a_cost_summary forwards asset and network filters', async () => {
    let received = null;
    const commerce = {
      _costAnalytics: {
        getAgentSpendSummary(agentAddress, filter) {
          received = { agentAddress, filter };
          return { totalSpent: 0, totalEarned: 0, transactionCount: 0 };
        },
      },
    };

    const result = await findTool('a2a_cost_summary').handler({
      commerce,
      params: { agentAddress: '0xAlice', asset: 'BTC', network: 'bitcoin' },
    });

    assert.equal(result.totalSpent, 0);
    assert.deepEqual(received, {
      agentAddress: '0xAlice',
      filter: { asset: 'BTC', network: 'bitcoin' },
    });
  });

  it('a2a_cost_counterparty_breakdown forwards asset and network filters', async () => {
    let received = null;
    const commerce = {
      _costAnalytics: {
        getCounterpartyBreakdown(agentAddress, filter) {
          received = { agentAddress, filter };
          return [];
        },
      },
    };

    const result = await findTool('a2a_cost_counterparty_breakdown').handler({
      commerce,
      params: { agentAddress: '0xAlice', asset: 'ZEC', network: 'zcash' },
    });

    assert.deepEqual(result, []);
    assert.deepEqual(received, {
      agentAddress: '0xAlice',
      filter: { asset: 'ZEC', network: 'zcash' },
    });
  });

  it('a2a_cost_operation_breakdown forwards asset and network filters', async () => {
    let received = null;
    const commerce = {
      _costAnalytics: {
        getOperationBreakdown(agentAddress, filter) {
          received = { agentAddress, filter };
          return [];
        },
      },
    };

    const result = await findTool('a2a_cost_operation_breakdown').handler({
      commerce,
      params: { agentAddress: '0xAlice', asset: 'BTC', network: 'bitcoin' },
    });

    assert.deepEqual(result, []);
    assert.deepEqual(received, {
      agentAddress: '0xAlice',
      filter: { asset: 'BTC', network: 'bitcoin' },
    });
  });

  it('a2a_cost_daily_trend forwards days, asset, and network filters', async () => {
    let received = null;
    const commerce = {
      _costAnalytics: {
        getDailySpendTrend(agentAddress, days, filter) {
          received = { agentAddress, days, filter };
          return [];
        },
      },
    };

    const result = await findTool('a2a_cost_daily_trend').handler({
      commerce,
      params: { agentAddress: '0xAlice', days: 14, asset: 'ZEC', network: 'zcash' },
    });

    assert.deepEqual(result, []);
    assert.deepEqual(received, {
      agentAddress: '0xAlice',
      days: 14,
      filter: { asset: 'ZEC', network: 'zcash' },
    });
  });

  it('a2a_cost_margin_analysis forwards asset and network filters', async () => {
    let received = null;
    const commerce = {
      _costAnalytics: {
        getMarginAnalysis(agentAddress, filter) {
          received = { agentAddress, filter };
          return { grossMargin: 0, perCounterparty: [] };
        },
      },
    };

    const result = await findTool('a2a_cost_margin_analysis').handler({
      commerce,
      params: { agentAddress: '0xAlice', asset: 'BTC', network: 'bitcoin' },
    });

    assert.equal(result.grossMargin, 0);
    assert.deepEqual(received, {
      agentAddress: '0xAlice',
      filter: { asset: 'BTC', network: 'bitcoin' },
    });
  });

  it('a2a_cost_anomalies forwards asset and network filters', async () => {
    let received = null;
    const commerce = {
      _costAnalytics: {
        detectAnomalies(agentAddress, filter) {
          received = { agentAddress, filter };
          return { transactionAnomalies: [], dailyAnomalies: [] };
        },
      },
    };

    const result = await findTool('a2a_cost_anomalies').handler({
      commerce,
      params: { agentAddress: '0xAlice', asset: 'ZEC', network: 'zcash' },
    });

    assert.deepEqual(result, { transactionAnomalies: [], dailyAnomalies: [] });
    assert.deepEqual(received, {
      agentAddress: '0xAlice',
      filter: { asset: 'ZEC', network: 'zcash' },
    });
  });

  it('a2a_cost_budget_forecast forwards budget, lookback, asset, and network filters', async () => {
    let received = null;
    const commerce = {
      _costAnalytics: {
        getBudgetForecast(agentAddress, monthlyBudget, lookbackDays, filter) {
          received = { agentAddress, monthlyBudget, lookbackDays, filter };
          return { dailyAvgSpend: 0, spentThisMonth: 0, remainingBudget: monthlyBudget };
        },
      },
    };

    const result = await findTool('a2a_cost_budget_forecast').handler({
      commerce,
      params: {
        agentAddress: '0xAlice',
        monthlyBudget: 0.5,
        lookbackDays: 14,
        asset: 'BTC',
        network: 'bitcoin',
      },
    });

    assert.equal(result.remainingBudget, 0.5);
    assert.deepEqual(received, {
      agentAddress: '0xAlice',
      monthlyBudget: 0.5,
      lookbackDays: 14,
      filter: { asset: 'BTC', network: 'bitcoin' },
    });
  });

  it('a2a_cost_top_spenders forwards limit, asset, and network filters', async () => {
    let received = null;
    const commerce = {
      _costAnalytics: {
        getTopSpenders(limit, filter) {
          received = { limit, filter };
          return [];
        },
      },
    };

    const result = await findTool('a2a_cost_top_spenders').handler({
      commerce,
      params: { limit: 5, asset: 'BTC', network: 'bitcoin' },
    });

    assert.deepEqual(result, []);
    assert.deepEqual(received, {
      limit: 5,
      filter: { asset: 'BTC', network: 'bitcoin' },
    });
  });
});

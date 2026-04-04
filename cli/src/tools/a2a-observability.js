/**
 * A2A Observability & Protocol Tools Module
 *
 * MCP tools for distributed tracing, agent introspection, settlement finality,
 * protocol handshake, and operational visibility.
 */

import { z } from 'zod';
import { getDefaultAssetForNetwork } from '../a2a/assets.js';
import { adaptCommerceApis, resolveCommerceApi } from '../commerce.js';

function parseJsonObject(value) {
  if (!value) return null;
  if (typeof value === 'object') return value;
  if (typeof value !== 'string') return null;
  try {
    const parsed = JSON.parse(value);
    return parsed && typeof parsed === 'object' ? parsed : null;
  } catch {
    return null;
  }
}

async function ensureFinalityTracker(commerce) {
  if (!commerce._finalityTracker) {
    const { createFinalityTracker } = await import('../a2a/settlement-finality.js');
    commerce._finalityTracker = createFinalityTracker();
  }
  return commerce._finalityTracker;
}

async function getA2AContextForWallet(commerce, walletAddress) {
  if (!walletAddress) {
    return { runtime: null, a2a: null };
  }

  const { findRuntimeByWalletAddress } = await import('./agent-runtime.js');
  const runtime = findRuntimeByWalletAddress(walletAddress);
  if (runtime) {
    return {
      runtime,
      a2a: runtime.a2a,
    };
  }

  const { createA2AService } = await import('../a2a/index.js');
  return {
    runtime: null,
    a2a: createA2AService(commerce, { walletAddress }),
  };
}

function getA2AStore(commerce) {
  try {
    return resolveCommerceApi(adaptCommerceApis(commerce, ['a2a']), 'a2a');
  } catch {
    return null;
  }
}

function buildAssetNetworkFilter(params = {}) {
  const filter = {};
  if (params.asset) filter.asset = params.asset;
  if (params.network) filter.network = params.network;
  return filter;
}

function normalizeBudgetEventAsset(asset) {
  return asset ? String(asset).toUpperCase() : null;
}

function normalizeBudgetEventNetwork(network) {
  return network ? String(network).toLowerCase() : null;
}

const ALERT_EVENT_TYPES = {
  budget: ['a2a_runtime.budget_warning', 'a2a_runtime.budget_exceeded'],
  settlement: ['a2a_runtime.settlement_failed', 'a2a_runtime.settlement_insufficient_funds'],
};

const ALERT_EVENT_METADATA = {
  'a2a_runtime.budget_warning': { category: 'budget', severity: 'warning' },
  'a2a_runtime.budget_exceeded': { category: 'budget', severity: 'exceeded' },
  'a2a_runtime.settlement_failed': { category: 'settlement', severity: 'failed' },
  'a2a_runtime.settlement_insufficient_funds': {
    category: 'settlement',
    severity: 'insufficient_funds',
  },
};

function createEmptyAgentAlertFeed(filters = {}) {
  return {
    filters,
    count: 0,
    alerts: [],
    summary: {
      byCategory: {},
      bySeverity: {},
      byEventType: {},
      budgetAlerts: {
        total: 0,
        warning: 0,
        exceeded: 0,
        byConstraintType: {},
      },
      settlementAlerts: {
        total: 0,
        failed: 0,
        insufficientFunds: 0,
        byPhase: {},
      },
      latestAlertAt: null,
    },
  };
}

async function buildAgentAlertFeed(commerce, agentAddress, options = {}) {
  const store = getA2AStore(commerce);
  const categories =
    Array.isArray(options.categories) && options.categories.length > 0
      ? options.categories.filter((value) => value === 'budget' || value === 'settlement')
      : ['budget', 'settlement'];
  const filters = {
    agentAddress: agentAddress || null,
    categories,
    asset: options.asset || null,
    network: options.network || null,
    since: options.since || null,
    limit: options.limit || 25,
  };

  if (!store || typeof store.listEventLog !== 'function' || !agentAddress) {
    return createEmptyAgentAlertFeed(filters);
  }

  const requestedEventTypes = categories.flatMap((category) => ALERT_EVENT_TYPES[category] || []);
  const overfetchMultiplier = options.asset || options.network ? 5 : 1;
  const fetchLimit = Math.max(filters.limit, 1) * overfetchMultiplier;
  const rows = (
    await Promise.all(
      requestedEventTypes.map((eventType) =>
        store.listEventLog({
          agent_address: agentAddress,
          event_type: eventType,
          since: options.since,
          limit: fetchLimit,
        }),
      ),
    )
  ).flat();

  const assetFilter = normalizeBudgetEventAsset(options.asset);
  const networkFilter = normalizeBudgetEventNetwork(options.network);
  const alerts = [...new Map((rows || []).filter(Boolean).map((row) => [row.id, row])).values()]
    .map((event) => {
      const payload = parseJsonObject(event.payload) || {};
      const metadata = ALERT_EVENT_METADATA[event.event_type] || {
        category: 'unknown',
        severity: 'unknown',
      };
      const network = payload.network || payload.chainId || null;
      return {
        id: event.id || null,
        eventType: event.event_type || null,
        category: metadata.category,
        severity: metadata.severity,
        createdAt: event.created_at || null,
        asset: payload.asset || payload.symbol || getDefaultAssetForNetwork(network) || null,
        network,
        type: payload.type || null,
        operation: payload.operation || null,
        error: payload.error || null,
        phase: payload.phase || null,
        required: payload.required ?? null,
        available: payload.available ?? null,
        limit: payload.limit ?? null,
        attempted: payload.attempted ?? null,
        remaining: payload.remaining ?? null,
        paymentId: payload.paymentId || null,
        referenceType: payload.referenceType || null,
        referenceId: payload.referenceId || null,
        quoteId: payload.quoteId || null,
        subscriptionId: payload.subscriptionId || null,
        splitId: payload.splitId || null,
        chainId: payload.chainId || null,
        payload,
      };
    })
    .filter((alert) => {
      if (assetFilter && normalizeBudgetEventAsset(alert.asset) !== assetFilter) {
        return false;
      }
      if (networkFilter && normalizeBudgetEventNetwork(alert.network) !== networkFilter) {
        return false;
      }
      return true;
    })
    .sort((left, right) => {
      const leftTime = left.createdAt ? new Date(left.createdAt).getTime() : 0;
      const rightTime = right.createdAt ? new Date(right.createdAt).getTime() : 0;
      return rightTime - leftTime;
    })
    .slice(0, filters.limit);

  const summary = {
    byCategory: {},
    bySeverity: {},
    byEventType: {},
    budgetAlerts: {
      total: 0,
      warning: 0,
      exceeded: 0,
      byConstraintType: {},
    },
    settlementAlerts: {
      total: 0,
      failed: 0,
      insufficientFunds: 0,
      byPhase: {},
    },
    latestAlertAt: alerts[0]?.createdAt || null,
  };

  for (const alert of alerts) {
    summary.byCategory[alert.category] = (summary.byCategory[alert.category] || 0) + 1;
    summary.bySeverity[alert.severity] = (summary.bySeverity[alert.severity] || 0) + 1;
    summary.byEventType[alert.eventType] = (summary.byEventType[alert.eventType] || 0) + 1;

    if (alert.category === 'budget') {
      summary.budgetAlerts.total++;
      if (alert.severity === 'warning') summary.budgetAlerts.warning++;
      if (alert.severity === 'exceeded') summary.budgetAlerts.exceeded++;
      const constraintType = alert.type || 'unknown';
      summary.budgetAlerts.byConstraintType[constraintType] =
        (summary.budgetAlerts.byConstraintType[constraintType] || 0) + 1;
      continue;
    }

    if (alert.category === 'settlement') {
      summary.settlementAlerts.total++;
      if (alert.severity === 'failed') summary.settlementAlerts.failed++;
      if (alert.severity === 'insufficient_funds') summary.settlementAlerts.insufficientFunds++;
      const phase =
        alert.phase || (alert.severity === 'insufficient_funds' ? 'funding' : 'unknown');
      summary.settlementAlerts.byPhase[phase] = (summary.settlementAlerts.byPhase[phase] || 0) + 1;
    }
  }

  return {
    filters,
    count: alerts.length,
    alerts,
    summary,
  };
}

async function buildBudgetEventSnapshot(commerce, agentAddress, options = {}) {
  const store = getA2AStore(commerce);
  if (!store || typeof store.listEventLog !== 'function' || !agentAddress) {
    return null;
  }

  const filter = buildAssetNetworkFilter(options);
  const assetFilter = normalizeBudgetEventAsset(filter.asset);
  const networkFilter = normalizeBudgetEventNetwork(filter.network);
  const lookbackDays =
    Number.isInteger(options.trendDays) && options.trendDays > 0 ? options.trendDays : 7;
  const since = new Date(Date.now() - lookbackDays * 24 * 60 * 60 * 1000).toISOString();
  const limitPerType = 250;

  const [warningRows, exceededRows] = await Promise.all([
    store.listEventLog({
      agent_address: agentAddress,
      event_type: 'a2a_runtime.budget_warning',
      since,
      limit: limitPerType,
    }),
    store.listEventLog({
      agent_address: agentAddress,
      event_type: 'a2a_runtime.budget_exceeded',
      since,
      limit: limitPerType,
    }),
  ]);

  const recent = [...(warningRows || []), ...(exceededRows || [])]
    .map((event) => {
      const payload = parseJsonObject(event?.payload) || {};
      return {
        id: event?.id || null,
        eventType: event?.event_type || null,
        severity: event?.event_type === 'a2a_runtime.budget_exceeded' ? 'exceeded' : 'warning',
        createdAt: event?.created_at || null,
        type: payload.type || null,
        asset: payload.asset || null,
        network: payload.network || null,
        spent: payload.spent ?? null,
        attempted: payload.attempted ?? null,
        limit: payload.limit ?? null,
        remaining: payload.remaining ?? null,
        operation: payload.operation || null,
        referenceType: payload.referenceType || null,
        referenceId: payload.referenceId || null,
      };
    })
    .filter((event) => {
      if (assetFilter && normalizeBudgetEventAsset(event.asset) !== assetFilter) {
        return false;
      }
      if (networkFilter && normalizeBudgetEventNetwork(event.network) !== networkFilter) {
        return false;
      }
      return true;
    })
    .sort((left, right) => {
      const leftTime = left.createdAt ? new Date(left.createdAt).getTime() : 0;
      const rightTime = right.createdAt ? new Date(right.createdAt).getTime() : 0;
      return rightTime - leftTime;
    });

  const counts = {
    warning: 0,
    exceeded: 0,
    total: recent.length,
  };
  const byConstraintType = {};

  for (const event of recent) {
    counts[event.severity]++;
    const constraintType = event.type || 'unknown';
    byConstraintType[constraintType] = (byConstraintType[constraintType] || 0) + 1;
  }

  return {
    filter: {
      asset: filter.asset || null,
      network: filter.network || null,
    },
    lookbackDays,
    since,
    counts,
    byConstraintType,
    latestExceededAt: recent.find((event) => event.severity === 'exceeded')?.createdAt || null,
    recent: recent.slice(0, 10),
  };
}

async function getRuntimeObservabilitySnapshot(commerce, agentAddress, options = {}) {
  const { findRuntimeByWalletAddress } = await import('./agent-runtime.js');
  const runtime = findRuntimeByWalletAddress(agentAddress);
  if (!runtime) {
    return null;
  }

  const requestedBudgetScope = buildAssetNetworkFilter(options);
  const paymentConfig =
    typeof runtime.getDefaultPaymentConfig === 'function'
      ? runtime.getDefaultPaymentConfig()
      : null;
  const budgetScope =
    requestedBudgetScope.asset || requestedBudgetScope.network
      ? {
          asset: requestedBudgetScope.asset || null,
          network: requestedBudgetScope.network || null,
        }
      : paymentConfig
        ? {
            asset: paymentConfig.asset || null,
            network: paymentConfig.network || null,
          }
        : null;
  const settlementChains =
    typeof runtime.listSettlementChains === 'function' ? runtime.listSettlementChains() : [];
  let chainWalletAddress = null;

  const walletNetwork = budgetScope?.network || paymentConfig?.network || null;
  if (walletNetwork && typeof runtime.getChainWalletAddress === 'function') {
    try {
      chainWalletAddress = await runtime.getChainWalletAddress(walletNetwork);
    } catch {
      chainWalletAddress = null;
    }
  }

  const budget = typeof runtime.getBudget === 'function' ? runtime.getBudget() : null;
  const budgetScoped =
    typeof runtime.getBudget === 'function' &&
    budgetScope &&
    (budgetScope.asset || budgetScope.network)
      ? runtime.getBudget(budgetScope)
      : null;
  const budgetEvents = await buildBudgetEventSnapshot(commerce, agentAddress, options);

  return {
    isRunning: typeof runtime.isRunning === 'function' ? runtime.isRunning() : null,
    budget,
    budgetScope,
    budgetScoped,
    budgetEvents,
    paymentConfig,
    settlementChains,
    chainWalletAddress,
  };
}

function buildEconomicsSnapshot(commerce, agentAddress, options = {}) {
  if (!commerce?._costAnalytics) {
    return null;
  }

  const filter = buildAssetNetworkFilter(options);
  const trendDays =
    Number.isInteger(options.trendDays) && options.trendDays > 0 ? options.trendDays : 7;
  const summary = commerce._costAnalytics.getAgentSpendSummary(agentAddress, filter);
  const margin = commerce._costAnalytics.getMarginAnalysis(agentAddress, filter);
  const operationBreakdown = commerce._costAnalytics.getOperationBreakdown(agentAddress, filter);
  const dailyTrend = commerce._costAnalytics.getDailySpendTrend(agentAddress, trendDays, filter);
  const anomalies = commerce._costAnalytics.detectAnomalies(agentAddress, filter);

  return {
    filter: {
      asset: filter.asset || null,
      network: filter.network || null,
    },
    trendDays,
    summary,
    margin,
    operationBreakdown,
    dailyTrend,
    anomalies,
    anomalyCounts: {
      transaction: anomalies.transactionAnomalies.length,
      daily: anomalies.dailyAnomalies.length,
    },
  };
}

function syncPaymentIntoFinalityTracker(tracker, payment) {
  if (!tracker || !payment?.id || !payment.tx_hash || !payment.network) {
    return null;
  }

  const metadata = parseJsonObject(payment.metadata);
  if (metadata?.simulated === true) {
    return null;
  }

  try {
    tracker.getSettlementStatus(payment.id);
  } catch {
    try {
      tracker.trackSettlement(
        payment.id,
        payment.tx_hash,
        payment.network,
        payment.block_number || 0,
      );
    } catch {
      // Ignore duplicate tracking attempts.
    }
  }

  if (payment.status === 'failed') {
    try {
      return tracker.markFailed(payment.id, metadata?.settlement_error || 'payment_failed');
    } catch {
      try {
        return tracker.getSettlementStatus(payment.id);
      } catch {
        return null;
      }
    }
  }

  const confirmations =
    metadata?.confirmations !== undefined && metadata?.confirmations !== null
      ? Number(metadata.confirmations)
      : 0;
  const latestBlock =
    payment.block_number && confirmations > 0
      ? payment.block_number + confirmations - 1
      : payment.block_number || 0;

  try {
    tracker.updateConfirmations(payment.id, confirmations, latestBlock);
  } catch {
    // Best-effort only.
  }

  try {
    return tracker.getSettlementStatus(payment.id);
  } catch {
    return null;
  }
}

function isTrackedPayment(payment) {
  if (!payment?.id || !payment.tx_hash || !payment.network) {
    return false;
  }

  const metadata = parseJsonObject(payment.metadata);
  return metadata?.simulated !== true;
}

function getStoredPaymentConfirmations(payment) {
  const metadata = parseJsonObject(payment?.metadata);
  const confirmations = Number(metadata?.confirmations ?? 0);
  return Number.isFinite(confirmations) && confirmations > 0 ? confirmations : 0;
}

async function deriveStoredPaymentFinality(payment) {
  const { getFinalityRequirement } = await import('../a2a/settlement-finality.js');
  const requiredConfirmations = getFinalityRequirement(payment.network);
  const confirmations = getStoredPaymentConfirmations(payment);

  if (payment.status === 'failed') {
    return {
      state: 'failed',
      isFinal: false,
      confirmations,
      requiredConfirmations,
      progress: requiredConfirmations > 0 ? Math.min(confirmations / requiredConfirmations, 1) : 1,
    };
  }

  if (payment.status === 'completed') {
    return {
      state: 'final',
      isFinal: true,
      confirmations: Math.max(confirmations, requiredConfirmations),
      requiredConfirmations,
      progress: 1,
    };
  }

  if (confirmations >= requiredConfirmations) {
    return {
      state: 'final',
      isFinal: true,
      confirmations,
      requiredConfirmations,
      progress: 1,
    };
  }

  if (confirmations > 0) {
    return {
      state: 'confirming',
      isFinal: false,
      confirmations,
      requiredConfirmations,
      progress: requiredConfirmations > 0 ? Math.min(confirmations / requiredConfirmations, 1) : 1,
    };
  }

  return {
    state: payment.tx_hash ? 'unconfirmed' : 'broadcast',
    isFinal: false,
    confirmations,
    requiredConfirmations,
    progress: 0,
  };
}

async function computeFinalityMetricsFromPayments(commerce, payments, options = {}) {
  const trackedPayments = payments.filter(isTrackedPayment);
  const durations = [];
  let totalFinal = 0;
  let totalFailed = 0;
  let pendingCount = 0;
  const includeTrackerReorgs = options.includeTrackerReorgs === true;
  const trackerMetrics = includeTrackerReorgs
    ? (await ensureFinalityTracker(commerce)).getMetrics()
    : null;

  if (trackedPayments.length === 0 && trackerMetrics) {
    return {
      ...trackerMetrics,
      historyHydrated: false,
      reorgCountSource: 'in_memory_tracker',
    };
  }

  for (const payment of trackedPayments) {
    const finality = await deriveStoredPaymentFinality(payment);
    if (finality.state === 'failed') {
      totalFailed++;
      continue;
    }

    if (finality.isFinal) {
      totalFinal++;
      if (payment.created_at && payment.completed_at) {
        const durationMs =
          new Date(payment.completed_at).getTime() - new Date(payment.created_at).getTime();
        if (Number.isFinite(durationMs) && durationMs >= 0) {
          durations.push(durationMs);
        }
      }
      continue;
    }

    pendingCount++;
  }

  const avgConfirmationTimeMs =
    durations.length > 0
      ? Math.round(durations.reduce((sum, d) => sum + d, 0) / durations.length)
      : 0;
  const finalityRate = trackedPayments.length > 0 ? totalFinal / trackedPayments.length : 0;

  return {
    totalTracked: trackedPayments.length,
    totalFinal,
    totalReorgs: trackerMetrics?.totalReorgs || 0,
    totalFailed,
    avgConfirmationTimeMs,
    finalityRate,
    pendingCount,
    historyHydrated: true,
    reorgCountSource: includeTrackerReorgs ? 'in_memory_tracker' : 'not_persisted',
  };
}

async function hydrateFinalityTrackerFromPayments(commerce, options = {}) {
  const { agentAddress = null, includeCompleted = false, limit = 100, network = null } = options;
  const store = getA2AStore(commerce);
  const payments =
    store && typeof store.listPayments === 'function'
      ? (await store.listPayments({ limit, network })) || []
      : [];
  const filteredPayments = [];

  for (const payment of payments) {
    if (network && payment.network !== network) {
      continue;
    }

    if (agentAddress) {
      const matchesAgent =
        payment.sender_address === agentAddress || payment.recipient_address === agentAddress;
      if (!matchesAgent) {
        continue;
      }
    }

    const shouldInclude =
      payment.status === 'submitted' ||
      payment.status === 'failed' ||
      (includeCompleted && payment.status === 'completed');

    if (!shouldInclude) {
      continue;
    }

    filteredPayments.push(payment);
  }

  const tracker = await ensureFinalityTracker(commerce);
  for (const payment of filteredPayments) {
    syncPaymentIntoFinalityTracker(tracker, payment);
  }

  return { tracker, payments: filteredPayments };
}

async function refreshSettlementForAgent(commerce, payment, agentAddress) {
  if (!payment || !agentAddress) {
    return null;
  }

  const involved =
    payment.sender_address === agentAddress || payment.recipient_address === agentAddress;
  if (!involved) {
    return null;
  }

  const { runtime, a2a } = await getA2AContextForWallet(commerce, agentAddress);
  if (!a2a || typeof a2a.refreshPayment !== 'function') {
    return null;
  }

  const result = await a2a.refreshPayment(payment.id);
  return {
    ...result,
    viaRuntime: Boolean(runtime),
  };
}

export const a2aObservabilityTools = [
  // ==========================================================================
  // Distributed Tracing
  // ==========================================================================
  {
    name: 'a2a_get_trace',
    description:
      'Retrieve all spans for a distributed trace ID. Shows the full journey of a transaction across agents.',
    inputSchema: {
      traceId: z.string().min(1).describe('Trace ID (32-char hex)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._tracingService) {
        return { success: false, error: 'Tracing service not initialized' };
      }
      return commerce._tracingService.getTrace(params.traceId);
    },
  },
  {
    name: 'a2a_tracing_metrics',
    description: 'Get tracing metrics: p50/p95/p99 latency, error rate, throughput, span count.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      if (!commerce._tracingService) {
        return { success: false, error: 'Tracing service not initialized' };
      }
      return commerce._tracingService.getMetrics();
    },
  },
  {
    name: 'a2a_recent_spans',
    description: 'Get the most recent trace spans for debugging.',
    inputSchema: {
      limit: z.number().optional().default(20).describe('Max spans to return'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._tracingService) {
        return { success: false, error: 'Tracing service not initialized' };
      }
      return commerce._tracingService.getRecentSpans(params.limit);
    },
  },
  {
    name: 'a2a_export_traces',
    description: 'Export all buffered spans in OpenTelemetry-compatible OTLP JSON format.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      if (!commerce._tracingService) {
        return { success: false, error: 'Tracing service not initialized' };
      }
      return commerce._tracingService.exportOTLP();
    },
  },

  // ==========================================================================
  // Agent Introspection
  // ==========================================================================
  {
    name: 'a2a_agent_dashboard',
    description:
      'Get a full operational dashboard for an agent: runtime status, budget, recent budget alerts, tick metrics, and rail-aware economics.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
      asset: z
        .string()
        .optional()
        .describe('Optional asset filter for economics, for example BTC or ZEC'),
      network: z
        .string()
        .optional()
        .describe('Optional network filter for economics, for example bitcoin or zcash'),
      trendDays: z
        .number()
        .int()
        .positive()
        .optional()
        .default(7)
        .describe('Lookback window for economics daily trend'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._introspectionService) {
        return { success: false, error: 'Introspection service not initialized' };
      }
      const dashboard = commerce._introspectionService.getAgentDashboard(params.agentAddress);
      const runtime = await getRuntimeObservabilitySnapshot(commerce, params.agentAddress, params);
      const economics = buildEconomicsSnapshot(commerce, params.agentAddress, {
        asset: params.asset,
        network: params.network,
        trendDays: params.trendDays,
      });

      if (runtime) {
        dashboard.runtime = runtime;
      }
      if (economics) {
        dashboard.economics = economics;
      }

      return dashboard;
    },
  },
  {
    name: 'a2a_agent_decisions',
    description: 'Get recent strategy decisions for an agent: what was accepted/rejected and why.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
      limit: z.number().optional().default(20).describe('Max decisions'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._introspectionService) {
        return { success: false, error: 'Introspection service not initialized' };
      }
      return commerce._introspectionService.getDecisionHistory(params.agentAddress, params.limit);
    },
  },
  {
    name: 'a2a_agent_performance',
    description:
      'Get performance report with optional rail-aware economics context: quote accept rate, response time, settlement success rate, dispute rate, filtered payment metrics, and recent budget alert activity.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
      asset: z
        .string()
        .optional()
        .describe('Optional asset filter for economics, for example BTC or ZEC'),
      network: z
        .string()
        .optional()
        .describe('Optional network filter for economics, for example bitcoin or zcash'),
      trendDays: z
        .number()
        .int()
        .positive()
        .optional()
        .default(7)
        .describe('Lookback window for economics daily trend'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._introspectionService) {
        return { success: false, error: 'Introspection service not initialized' };
      }
      const report = commerce._introspectionService.getPerformanceReport(params.agentAddress);
      const runtime = await getRuntimeObservabilitySnapshot(commerce, params.agentAddress, params);
      const economics = buildEconomicsSnapshot(commerce, params.agentAddress, {
        asset: params.asset,
        network: params.network,
        trendDays: params.trendDays,
      });

      if (runtime) {
        report.runtime = runtime;
      }
      if (economics) {
        report.economics = {
          filter: economics.filter,
          trendDays: economics.trendDays,
          summary: economics.summary,
          margin: economics.margin,
          anomalyCounts: economics.anomalyCounts,
          dailyTrend: economics.dailyTrend,
        };
      }

      return report;
    },
  },
  {
    name: 'a2a_agent_tick_metrics',
    description:
      'Get tick loop metrics: avg duration, ticks/min, quotes evaluated, payments executed, errors.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._introspectionService) {
        return { success: false, error: 'Introspection service not initialized' };
      }
      return commerce._introspectionService.getTickMetrics(params.agentAddress);
    },
  },
  {
    name: 'a2a_agent_lifecycle',
    description:
      'Get agent lifecycle history: start/stop/pause/resume events with timestamps and reasons.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._introspectionService) {
        return { success: false, error: 'Introspection service not initialized' };
      }
      return commerce._introspectionService.getLifecycleHistory(params.agentAddress);
    },
  },
  {
    name: 'a2a_agent_alerts',
    description:
      'List recent budget and settlement alerts for an agent, with optional category, time window, and payment-rail filters.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
      categories: z
        .array(z.enum(['budget', 'settlement']))
        .optional()
        .describe('Optional alert categories to include; defaults to budget and settlement'),
      asset: z
        .string()
        .optional()
        .describe(
          'Optional asset filter applied against alert payload metadata, for example BTC or ZEC',
        ),
      network: z
        .string()
        .optional()
        .describe(
          'Optional network filter applied against alert payload metadata, for example bitcoin or zcash',
        ),
      since: z
        .string()
        .optional()
        .describe('Optional ISO timestamp; only alerts after this time are returned'),
      limit: z
        .number()
        .int()
        .min(1)
        .max(100)
        .optional()
        .default(25)
        .describe('Max alerts to return'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      return buildAgentAlertFeed(commerce, params.agentAddress, params);
    },
  },

  // ==========================================================================
  // Settlement Finality
  // ==========================================================================
  {
    name: 'a2a_settlement_status',
    description:
      'Get settlement finality status: broadcast → unconfirmed → confirming → final. Shows confirmation count vs chain requirement.',
    inputSchema: {
      intentId: z.string().min(1).describe('Payment intent ID'),
      agentAddress: z
        .string()
        .optional()
        .describe(
          'Agent wallet address to use when refreshing the underlying payment from live chain state',
        ),
      refreshOnChain: z
        .boolean()
        .optional()
        .describe(
          'Refresh the underlying payment from live chain state before reading tracker status',
        ),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const tracker = await ensureFinalityTracker(commerce);
      const store = getA2AStore(commerce);
      let payment =
        store && typeof store.getPayment === 'function'
          ? await store.getPayment(params.intentId)
          : null;

      if (params.refreshOnChain) {
        if (!params.agentAddress) {
          return { success: false, error: 'agentAddress is required when refreshOnChain is true.' };
        }
        if (!payment) {
          return { success: false, error: 'Settlement not found.' };
        }

        const refresh = await refreshSettlementForAgent(commerce, payment, params.agentAddress);
        if (!refresh) {
          return {
            success: false,
            error: 'Settlement is not accessible for the provided agentAddress.',
          };
        }

        payment =
          store && typeof store.getPayment === 'function'
            ? await store.getPayment(params.intentId)
            : payment;
        const status =
          refresh.finality || (payment ? syncPaymentIntoFinalityTracker(tracker, payment) : null);

        return status
          ? {
              ...status,
              refreshed: Boolean(refresh.refreshed),
              payment: refresh.payment || null,
              onChain: refresh.onChain || null,
              viaRuntime: Boolean(refresh.viaRuntime),
            }
          : { success: false, error: 'Settlement not found.' };
      }

      try {
        return tracker.getSettlementStatus(params.intentId);
      } catch (_err) {
        void _err;
      }

      if (!payment) {
        return { success: false, error: 'Settlement not found.' };
      }

      const status = syncPaymentIntoFinalityTracker(tracker, payment);
      return status || { success: false, error: 'Settlement not found.' };
    },
  },
  {
    name: 'a2a_settlement_pending',
    description: 'List all settlements not yet final — awaiting blockchain confirmations.',
    inputSchema: {
      agentAddress: z
        .string()
        .optional()
        .describe(
          'Optional agent wallet filter to only include settlements sent or received by that agent',
        ),
      network: z
        .string()
        .optional()
        .describe('Optional settlement network filter, for example bitcoin or zcash'),
      includeCompleted: z
        .boolean()
        .optional()
        .describe(
          'Include already-completed payments when reconstructing tracker state from stored payments',
        ),
      refreshOnChain: z
        .boolean()
        .optional()
        .describe(
          'Refresh pending settlements from live chain state before returning tracker results (requires agentAddress)',
        ),
      limit: z
        .number()
        .int()
        .min(1)
        .max(500)
        .optional()
        .describe('Max payments to scan from storage when hydrating pending settlements'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (params.refreshOnChain && !params.agentAddress) {
        return { success: false, error: 'agentAddress is required when refreshOnChain is true.' };
      }

      if (params.refreshOnChain && params.agentAddress) {
        const { payments } = await hydrateFinalityTrackerFromPayments(commerce, {
          agentAddress: params.agentAddress,
          includeCompleted: false,
          network: params.network || null,
          limit: params.limit || 100,
        });
        for (const payment of payments) {
          if (payment.status !== 'submitted') continue;
          await refreshSettlementForAgent(commerce, payment, params.agentAddress);
        }
      }

      const { tracker, payments } = await hydrateFinalityTrackerFromPayments(commerce, {
        agentAddress: params.agentAddress || null,
        includeCompleted: params.includeCompleted || false,
        network: params.network || null,
        limit: params.limit || 100,
      });

      const pending = tracker.listPending();
      if (!params.agentAddress && !params.network) {
        return pending;
      }

      const allowedIntentIds = new Set(payments.map((payment) => payment.id));
      return pending.filter((entry) => allowedIntentIds.has(entry.intentId));
    },
  },
  {
    name: 'a2a_settlement_finality_metrics',
    description: 'Get settlement metrics: avg confirmation time, finality rate, reorg count.',
    inputSchema: {
      agentAddress: z
        .string()
        .optional()
        .describe(
          'Optional agent wallet filter to scope metrics to settlements sent or received by that agent',
        ),
      network: z
        .string()
        .optional()
        .describe('Optional settlement network filter, for example bitcoin or zcash'),
      refreshOnChain: z
        .boolean()
        .optional()
        .describe(
          'Refresh pending settlements from live chain state before computing metrics (requires agentAddress)',
        ),
      limit: z
        .number()
        .int()
        .min(1)
        .max(500)
        .optional()
        .describe('Max payments to scan from storage when hydrating finality metrics'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (params.refreshOnChain && !params.agentAddress) {
        return { success: false, error: 'agentAddress is required when refreshOnChain is true.' };
      }

      const limit = params.limit || 100;
      const network = params.network || null;

      if (params.refreshOnChain) {
        const { payments } = await hydrateFinalityTrackerFromPayments(commerce, {
          agentAddress: params.agentAddress,
          includeCompleted: false,
          network,
          limit,
        });

        for (const payment of payments) {
          if (payment.status !== 'submitted') continue;
          await refreshSettlementForAgent(commerce, payment, params.agentAddress);
        }
      }

      const { payments } = await hydrateFinalityTrackerFromPayments(commerce, {
        agentAddress: params.agentAddress || null,
        includeCompleted: true,
        network,
        limit,
      });

      const metrics = await computeFinalityMetricsFromPayments(commerce, payments, {
        includeTrackerReorgs: !params.agentAddress && !network,
      });

      return {
        ...metrics,
        refreshed: Boolean(params.refreshOnChain),
        filters: {
          agentAddress: params.agentAddress || null,
          network,
          limit,
        },
      };
    },
  },

  // ==========================================================================
  // Protocol Handshake
  // ==========================================================================
  {
    name: 'a2a_handshake',
    description:
      'Initiate capability handshake with another agent. Returns compatibility report: shared networks/assets, feature mismatches, recommended network/asset.',
    inputSchema: {
      targetCapabilities: z
        .object({
          protocolVersion: z.string().optional(),
          supportedNetworks: z.array(z.string()).optional(),
          supportedAssets: z.array(z.string()).optional(),
          features: z.record(z.boolean()).optional(),
          maxTransactionAmount: z.number().optional(),
        })
        .describe('Target agent capabilities (from their agent card or handshake response)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._handshakeService) {
        return { success: false, error: 'Handshake service not initialized' };
      }
      return commerce._handshakeService.initiateHandshake(params.targetCapabilities);
    },
  },
  {
    name: 'a2a_my_capabilities',
    description: "Get this agent's capability manifest for protocol handshake.",
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      if (!commerce._handshakeService) {
        return { success: false, error: 'Handshake service not initialized' };
      }
      return commerce._handshakeService.getMyCapabilities();
    },
  },
];

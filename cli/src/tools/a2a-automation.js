/**
 * A2A Automation Tools Module
 *
 * MCP tools for billing executor, dispute resolver, SLA enforcement,
 * marketplace auto-award, notification DLQ management, health checks,
 * and rate limit inspection.
 */

import { z } from 'zod';

export const a2aAutomationTools = [
  // ==========================================================================
  // Billing Executor
  // ==========================================================================
  {
    name: 'a2a_billing_tick',
    description:
      'Run one billing cycle: process due subscriptions, execute payments, handle past-due, activate trials.',
    inputSchema: {},
    permission: 'write',
    handler: async ({ commerce }) => {
      if (!commerce._billingExecutor) {
        return { success: false, error: 'Billing executor not initialized' };
      }
      return commerce._billingExecutor.tick();
    },
  },
  {
    name: 'a2a_billing_start',
    description: 'Start the automated billing executor loop.',
    inputSchema: {},
    permission: 'admin',
    handler: async ({ commerce }) => {
      if (!commerce._billingExecutor) {
        return { success: false, error: 'Billing executor not initialized' };
      }
      commerce._billingExecutor.start();
      return { success: true, message: 'Billing executor started' };
    },
  },
  {
    name: 'a2a_billing_stop',
    description: 'Stop the automated billing executor loop.',
    inputSchema: {},
    permission: 'admin',
    handler: async ({ commerce }) => {
      if (!commerce._billingExecutor) {
        return { success: false, error: 'Billing executor not initialized' };
      }
      commerce._billingExecutor.stop();
      return { success: true, message: 'Billing executor stopped' };
    },
  },
  {
    name: 'a2a_billing_metrics',
    description: 'Get billing executor metrics: total billed, failed, cancelled, etc.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      if (!commerce._billingExecutor) {
        return { success: false, error: 'Billing executor not initialized' };
      }
      return commerce._billingExecutor.getMetrics();
    },
  },

  // ==========================================================================
  // Dispute Resolver
  // ==========================================================================
  {
    name: 'a2a_dispute_resolver_tick',
    description:
      'Run one dispute resolution cycle: auto-transition deadlines, apply rule-based arbitration.',
    inputSchema: {},
    permission: 'write',
    handler: async ({ commerce }) => {
      if (!commerce._disputeResolver) {
        return { success: false, error: 'Dispute resolver not initialized' };
      }
      return commerce._disputeResolver.tick();
    },
  },
  {
    name: 'a2a_dispute_resolver_start',
    description: 'Start the automated dispute resolver loop.',
    inputSchema: {},
    permission: 'admin',
    handler: async ({ commerce }) => {
      if (!commerce._disputeResolver) {
        return { success: false, error: 'Dispute resolver not initialized' };
      }
      commerce._disputeResolver.start();
      return { success: true, message: 'Dispute resolver started' };
    },
  },
  {
    name: 'a2a_dispute_resolver_metrics',
    description: 'Get dispute resolver metrics: transitions, resolutions, escalations.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      if (!commerce._disputeResolver) {
        return { success: false, error: 'Dispute resolver not initialized' };
      }
      return commerce._disputeResolver.getMetrics();
    },
  },

  // ==========================================================================
  // SLA Enforcement
  // ==========================================================================
  {
    name: 'a2a_sla_enforce',
    description:
      'Enforce SLA penalties for a service: detect breaches and apply credits/suspensions/refunds.',
    inputSchema: {
      serviceId: z.string().min(1).describe('Service ID to enforce SLAs for'),
    },
    permission: 'write',
    handler: async ({ commerce, params }) => {
      if (!commerce._slaService) {
        return { success: false, error: 'SLA service not initialized' };
      }
      return commerce._slaService.enforcePenalties(params.serviceId, commerce.a2a?.());
    },
  },
  {
    name: 'a2a_sla_enforce_all',
    description: 'Run a full SLA enforcement cycle across all services.',
    inputSchema: {},
    permission: 'write',
    handler: async ({ commerce }) => {
      if (!commerce._slaService) {
        return { success: false, error: 'SLA service not initialized' };
      }
      return commerce._slaService.enforceAll(commerce.a2a?.());
    },
  },

  // ==========================================================================
  // Marketplace Auto-Award
  // ==========================================================================
  {
    name: 'a2a_marketplace_auto_award',
    description:
      'Auto-award expired RFQs to the highest-scored response. Expires RFQs with no responses.',
    inputSchema: {},
    permission: 'write',
    handler: async ({ commerce }) => {
      if (!commerce._marketplace) {
        return { success: false, error: 'Marketplace service not initialized' };
      }
      return commerce._marketplace.autoAwardExpiredRFQs();
    },
  },
  {
    name: 'a2a_marketplace_maintenance',
    description: 'Run a full marketplace maintenance tick: auto-award + expiry + cleanup.',
    inputSchema: {},
    permission: 'write',
    handler: async ({ commerce }) => {
      if (!commerce._marketplace) {
        return { success: false, error: 'Marketplace service not initialized' };
      }
      return commerce._marketplace.maintenanceTick();
    },
  },

  // ==========================================================================
  // Notification DLQ
  // ==========================================================================
  {
    name: 'a2a_list_failed_notifications',
    description:
      'List failed webhook notifications (dead-letter queue). Shows notifications that exceeded max retry attempts.',
    inputSchema: {
      limit: z.number().optional().default(50).describe('Max results'),
      recipientAddress: z.string().optional().describe('Filter by recipient'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const filter = { status: 'failed' };
      if (params.recipientAddress) filter.recipient_address = params.recipientAddress;
      if (params.limit) filter.limit = params.limit;
      const logs = await commerce._store.listNotificationLog(filter);
      return { count: logs.length, notifications: logs };
    },
  },
  {
    name: 'a2a_replay_notification',
    description: 'Manually retry a specific failed notification.',
    inputSchema: {
      notificationId: z.string().min(1).describe('Notification log ID to retry'),
    },
    permission: 'write',
    handler: async ({ commerce, params }) => {
      const notification = await commerce._store.getNotificationLog(params.notificationId);
      if (!notification) {
        return { success: false, error: 'Notification not found' };
      }
      if (notification.status === 'delivered') {
        return { success: false, error: 'Notification already delivered' };
      }

      // Reset status to pending and reduce attempts so retry picks it up
      await commerce._store.updateNotificationLog(params.notificationId, {
        status: 'pending',
        attempts: Math.max((notification.attempts || 0) - 1, 0),
      });

      // Trigger retry
      if (commerce._notificationService) {
        const result = await commerce._notificationService.retryPendingNotifications();
        return { success: true, retryResult: result };
      }

      return { success: true, message: 'Notification reset to pending for next retry cycle' };
    },
  },
  {
    name: 'a2a_notification_retry_all',
    description: 'Trigger retry of all pending webhook notifications.',
    inputSchema: {},
    permission: 'write',
    handler: async ({ commerce }) => {
      if (!commerce._notificationService) {
        return { success: false, error: 'Notification service not initialized' };
      }
      return commerce._notificationService.retryPendingNotifications();
    },
  },
  {
    name: 'a2a_webhook_dlq_status',
    description: 'Get dead-letter queue metrics: pending, failed, delivered counts.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const [pending, failed, delivered] = await Promise.all([
        commerce._store.listNotificationLog({ status: 'pending', limit: 1000 }),
        commerce._store.listNotificationLog({ status: 'failed', limit: 1000 }),
        commerce._store.listNotificationLog({ status: 'delivered', limit: 1 }),
      ]);
      return {
        pending: pending.length,
        failed: failed.length,
        deliveredSample: delivered.length > 0,
        timestamp: new Date().toISOString(),
      };
    },
  },

  // ==========================================================================
  // Health & Readiness
  // ==========================================================================
  {
    name: 'a2a_health_check',
    description: 'Run a full health check: database, sequencer, subsystems.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      if (!commerce._healthService) {
        return { status: 'healthy', note: 'Health service not initialized — basic check only' };
      }
      return commerce._healthService.check();
    },
  },
  {
    name: 'a2a_readiness',
    description: 'Check if the system is ready to accept traffic.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      if (!commerce._healthService) {
        return { status: 'ready', note: 'Health service not initialized' };
      }
      return commerce._healthService.ready();
    },
  },

  // ==========================================================================
  // x402 Circuit Breaker
  // ==========================================================================
  {
    name: 'x402_circuit_status',
    description:
      'Get x402 sequencer circuit breaker status: state (closed/open/half_open), failures, queue depth.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      if (!commerce._sequencerClient) {
        return { state: 'not_configured', failures: 0, queueDepth: 0 };
      }
      return commerce._sequencerClient.getCircuitStatus();
    },
  },

  // ==========================================================================
  // Rate Limit Inspection
  // ==========================================================================
  {
    name: 'a2a_rate_limit_metrics',
    description: 'Get MCP rate limiter metrics: active buckets, top agents by request count.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      if (!commerce._rateLimiter) {
        return { activeBuckets: 0, topAgents: [], note: 'Rate limiter not initialized' };
      }
      return commerce._rateLimiter.getMetrics();
    },
  },

  // ==========================================================================
  // Saga Orchestration
  // ==========================================================================
  {
    name: 'a2a_saga_execute',
    description:
      'Execute a multi-step transaction saga (e.g., purchase, subscription, RFQ). Automatically rolls back on failure.',
    inputSchema: {
      sagaType: z.enum(['purchase', 'subscription', 'rfq']).describe('Type of saga to execute'),
      context: z
        .record(z.any())
        .describe('Saga context: buyer/seller addresses, amount, items, etc.'),
      sagaId: z.string().optional().describe('Optional saga ID for idempotency'),
    },
    permission: 'write',
    handler: async ({ commerce, params }) => {
      if (!commerce._sagaOrchestrator) {
        return { success: false, error: 'Saga orchestrator not initialized' };
      }
      return commerce._sagaOrchestrator.execute(params.sagaType, params.context, params.sagaId);
    },
  },
  {
    name: 'a2a_saga_status',
    description: 'Get the status of a running or completed saga by ID.',
    inputSchema: {
      sagaId: z.string().min(1).describe('Saga ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._sagaOrchestrator) {
        return { success: false, error: 'Saga orchestrator not initialized' };
      }
      return commerce._sagaOrchestrator.getStatus(params.sagaId);
    },
  },
  {
    name: 'a2a_saga_list',
    description: 'List sagas with optional status filter.',
    inputSchema: {
      status: z
        .string()
        .optional()
        .describe('Filter by status: pending, running, completed, failed, compensated'),
      limit: z.number().optional().default(20).describe('Max results'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._sagaOrchestrator) {
        return { success: false, error: 'Saga orchestrator not initialized' };
      }
      return commerce._sagaOrchestrator.listSagas(params);
    },
  },
  {
    name: 'a2a_saga_cancel',
    description: 'Cancel a running saga and trigger compensation/rollback.',
    inputSchema: {
      sagaId: z.string().min(1).describe('Saga ID to cancel'),
    },
    permission: 'write',
    handler: async ({ commerce, params }) => {
      if (!commerce._sagaOrchestrator) {
        return { success: false, error: 'Saga orchestrator not initialized' };
      }
      return commerce._sagaOrchestrator.cancelSaga(params.sagaId);
    },
  },

  // ==========================================================================
  // Cost Analytics
  // ==========================================================================
  {
    name: 'a2a_cost_summary',
    description:
      'Get spend summary for an agent with optional asset/network filters and per-rail breakdowns.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
      asset: z.string().optional().describe('Optional asset filter, for example USDC, BTC, or ZEC'),
      network: z
        .string()
        .optional()
        .describe('Optional network filter, for example set_chain, bitcoin, or zcash'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._costAnalytics) {
        return { success: false, error: 'Cost analytics not initialized' };
      }
      return commerce._costAnalytics.getAgentSpendSummary(params.agentAddress, {
        asset: params.asset,
        network: params.network,
      });
    },
  },
  {
    name: 'a2a_cost_counterparty_breakdown',
    description:
      'Get per-counterparty spend/earn breakdown for an agent, with optional asset/network filters and per-rail details.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
      asset: z.string().optional().describe('Optional asset filter, for example USDC, BTC, or ZEC'),
      network: z
        .string()
        .optional()
        .describe('Optional network filter, for example set_chain, bitcoin, or zcash'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._costAnalytics) {
        return { success: false, error: 'Cost analytics not initialized' };
      }
      return commerce._costAnalytics.getCounterpartyBreakdown(params.agentAddress, {
        asset: params.asset,
        network: params.network,
      });
    },
  },
  {
    name: 'a2a_cost_operation_breakdown',
    description:
      'Get per-operation cost breakdown for an agent, with optional asset/network filters and per-rail details.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
      asset: z.string().optional().describe('Optional asset filter, for example USDC, BTC, or ZEC'),
      network: z
        .string()
        .optional()
        .describe('Optional network filter, for example set_chain, bitcoin, or zcash'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._costAnalytics) {
        return { success: false, error: 'Cost analytics not initialized' };
      }
      return commerce._costAnalytics.getOperationBreakdown(params.agentAddress, {
        asset: params.asset,
        network: params.network,
      });
    },
  },
  {
    name: 'a2a_cost_daily_trend',
    description:
      'Get daily spend and earnings trend for an agent, with optional asset/network filters and per-rail day breakdowns.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
      days: z.number().int().positive().optional().default(30).describe('Lookback window in days'),
      asset: z.string().optional().describe('Optional asset filter, for example USDC, BTC, or ZEC'),
      network: z
        .string()
        .optional()
        .describe('Optional network filter, for example set_chain, bitcoin, or zcash'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._costAnalytics) {
        return { success: false, error: 'Cost analytics not initialized' };
      }
      return commerce._costAnalytics.getDailySpendTrend(params.agentAddress, params.days, {
        asset: params.asset,
        network: params.network,
      });
    },
  },
  {
    name: 'a2a_cost_anomalies',
    description:
      'Detect per-rail spending anomalies, with optional asset/network filters to avoid mixed-unit comparisons.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
      asset: z.string().optional().describe('Optional asset filter, for example USDC, BTC, or ZEC'),
      network: z
        .string()
        .optional()
        .describe('Optional network filter, for example set_chain, bitcoin, or zcash'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._costAnalytics) {
        return { success: false, error: 'Cost analytics not initialized' };
      }
      return commerce._costAnalytics.detectAnomalies(params.agentAddress, {
        asset: params.asset,
        network: params.network,
      });
    },
  },
  {
    name: 'a2a_cost_margin_analysis',
    description:
      'Get margin analysis with optional asset/network filters and per-rail counterparty breakdowns.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
      asset: z.string().optional().describe('Optional asset filter, for example USDC, BTC, or ZEC'),
      network: z
        .string()
        .optional()
        .describe('Optional network filter, for example set_chain, bitcoin, or zcash'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._costAnalytics) {
        return { success: false, error: 'Cost analytics not initialized' };
      }
      return commerce._costAnalytics.getMarginAnalysis(params.agentAddress, {
        asset: params.asset,
        network: params.network,
      });
    },
  },
  {
    name: 'a2a_cost_budget_forecast',
    description:
      'Forecast when a budget in the selected asset units will be exhausted, with optional asset/network filters and per-rail spend breakdowns.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
      monthlyBudget: z.number().positive().describe('Monthly budget in the selected asset units'),
      lookbackDays: z
        .number()
        .int()
        .positive()
        .optional()
        .default(30)
        .describe('Optional lookback window for spend trend analysis'),
      asset: z.string().optional().describe('Optional asset filter, for example USDC, BTC, or ZEC'),
      network: z
        .string()
        .optional()
        .describe('Optional network filter, for example set_chain, bitcoin, or zcash'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._costAnalytics) {
        return { success: false, error: 'Cost analytics not initialized' };
      }
      return commerce._costAnalytics.getBudgetForecast(
        params.agentAddress,
        params.monthlyBudget,
        params.lookbackDays,
        {
          asset: params.asset,
          network: params.network,
        },
      );
    },
  },
  {
    name: 'a2a_cost_top_spenders',
    description: 'Get top-spending agents across the system, with optional asset/network filters.',
    inputSchema: {
      limit: z.number().optional().default(10).describe('Max results'),
      asset: z.string().optional().describe('Optional asset filter, for example USDC, BTC, or ZEC'),
      network: z
        .string()
        .optional()
        .describe('Optional network filter, for example set_chain, bitcoin, or zcash'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._costAnalytics) {
        return { success: false, error: 'Cost analytics not initialized' };
      }
      return commerce._costAnalytics.getTopSpenders(params.limit, {
        asset: params.asset,
        network: params.network,
      });
    },
  },

  // ==========================================================================
  // Escrow Processing
  // ==========================================================================
  {
    name: 'a2a_escrow_process_all',
    description:
      'Process all escrows: auto-release time-locked escrows where conditions are met, expire past-deadline escrows.',
    inputSchema: {},
    permission: 'write',
    handler: async ({ commerce }) => {
      if (!commerce._escrowService) {
        return { success: false, error: 'Escrow service not initialized' };
      }
      return commerce._escrowService.processEscrows();
    },
  },
];

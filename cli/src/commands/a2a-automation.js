/**
 * A2A Automation Commands Module
 */

import { a2aAutomationTools } from '../tools/a2a-automation.js';
import {
  createMetadata,
  createUnknownActionError,
  formatToolResult,
  invokeTool,
  parseJsonArg,
  parseOptionalInteger,
  parseOptionalNumber,
} from '../command-tooling.js';

const ACTIONS = {
  'billing-tick': {
    tool: 'a2a_billing_tick',
    description: 'Run one billing tick',
    args: [],
    parse: () => ({ params: {} }),
  },
  'billing-start': {
    tool: 'a2a_billing_start',
    description: 'Start billing loop',
    args: [],
    parse: () => ({ params: {} }),
  },
  'billing-stop': {
    tool: 'a2a_billing_stop',
    description: 'Stop billing loop',
    args: [],
    parse: () => ({ params: {} }),
  },
  'billing-metrics': {
    tool: 'a2a_billing_metrics',
    description: 'Get billing metrics',
    args: [],
    parse: () => ({ params: {} }),
  },
  'dispute-resolver-tick': {
    tool: 'a2a_dispute_resolver_tick',
    description: 'Run one dispute resolver tick',
    args: [],
    parse: () => ({ params: {} }),
  },
  'dispute-resolver-start': {
    tool: 'a2a_dispute_resolver_start',
    description: 'Start dispute resolver loop',
    args: [],
    parse: () => ({ params: {} }),
  },
  'dispute-resolver-metrics': {
    tool: 'a2a_dispute_resolver_metrics',
    description: 'Get dispute resolver metrics',
    args: [],
    parse: () => ({ params: {} }),
  },
  'sla-enforce': {
    tool: 'a2a_sla_enforce',
    description: 'Enforce SLA penalties for a service',
    args: ['<serviceId>'],
    parse: ([serviceId]) => {
      if (!serviceId) throw new Error('Usage: a2a-automation sla-enforce <serviceId>');
      return { params: { serviceId } };
    },
  },
  'sla-enforce-all': {
    tool: 'a2a_sla_enforce_all',
    description: 'Enforce SLAs for all services',
    args: [],
    parse: () => ({ params: {} }),
  },
  'marketplace-auto-award': {
    tool: 'a2a_marketplace_auto_award',
    description: 'Auto-award expired RFQs',
    args: [],
    parse: () => ({ params: {} }),
  },
  'marketplace-maintenance': {
    tool: 'a2a_marketplace_maintenance',
    description: 'Run marketplace maintenance',
    args: [],
    parse: () => ({ params: {} }),
  },
  'failed-notifications': {
    tool: 'a2a_list_failed_notifications',
    description: 'List failed notifications',
    args: ['[limit]', '[recipientAddress]'],
    parse: ([limitRaw, recipientAddress]) => ({
      params: {
        limit: parseOptionalInteger(
          limitRaw,
          'Usage: a2a-automation failed-notifications [limit] [recipientAddress]',
        ),
        recipientAddress: recipientAddress || undefined,
      },
    }),
  },
  'replay-notification': {
    tool: 'a2a_replay_notification',
    description: 'Replay a failed notification',
    args: ['<notificationId>'],
    parse: ([notificationId]) => {
      if (!notificationId)
        throw new Error('Usage: a2a-automation replay-notification <notificationId>');
      return { params: { notificationId } };
    },
  },
  'notification-retry-all': {
    tool: 'a2a_notification_retry_all',
    description: 'Retry all pending notifications',
    args: [],
    parse: () => ({ params: {} }),
  },
  'webhook-dlq-status': {
    tool: 'a2a_webhook_dlq_status',
    description: 'Get webhook DLQ status',
    args: [],
    parse: () => ({ params: {} }),
  },
  'health-check': {
    tool: 'a2a_health_check',
    description: 'Run a health check',
    args: [],
    parse: () => ({ params: {} }),
  },
  readiness: {
    tool: 'a2a_readiness',
    description: 'Check readiness',
    args: [],
    parse: () => ({ params: {} }),
  },
  'circuit-status': {
    tool: 'x402_circuit_status',
    description: 'Get x402 circuit breaker status',
    args: [],
    parse: () => ({ params: {} }),
  },
  'rate-limit-metrics': {
    tool: 'a2a_rate_limit_metrics',
    description: 'Get rate limiter metrics',
    args: [],
    parse: () => ({ params: {} }),
  },
  'saga-execute': {
    tool: 'a2a_saga_execute',
    description: 'Execute a saga',
    args: ['<sagaType>', '<contextJson>', '[sagaId]'],
    parse: ([sagaType, contextJson, sagaId]) => {
      if (!sagaType || !contextJson) {
        throw new Error('Usage: a2a-automation saga-execute <sagaType> <contextJson> [sagaId]');
      }
      return {
        params: {
          sagaType,
          context: parseJsonArg(contextJson, 'context'),
          sagaId: sagaId || undefined,
        },
      };
    },
  },
  'saga-status': {
    tool: 'a2a_saga_status',
    description: 'Get saga status',
    args: ['<sagaId>'],
    parse: ([sagaId]) => {
      if (!sagaId) throw new Error('Usage: a2a-automation saga-status <sagaId>');
      return { params: { sagaId } };
    },
  },
  'saga-list': {
    tool: 'a2a_saga_list',
    description: 'List sagas',
    args: ['[status]', '[limit]'],
    parse: ([status, limitRaw]) => ({
      params: {
        status: status || undefined,
        limit: parseOptionalInteger(limitRaw, 'Usage: a2a-automation saga-list [status] [limit]'),
      },
    }),
  },
  'saga-cancel': {
    tool: 'a2a_saga_cancel',
    description: 'Cancel a saga',
    args: ['<sagaId>'],
    parse: ([sagaId]) => {
      if (!sagaId) throw new Error('Usage: a2a-automation saga-cancel <sagaId>');
      return { params: { sagaId } };
    },
  },
  'cost-summary': {
    tool: 'a2a_cost_summary',
    description: 'Get cost summary',
    args: ['<agentAddress>', '[asset]', '[network]'],
    parse: ([agentAddress, asset, network]) => {
      if (!agentAddress)
        throw new Error('Usage: a2a-automation cost-summary <agentAddress> [asset] [network]');
      return { params: { agentAddress, asset: asset || undefined, network: network || undefined } };
    },
  },
  'cost-counterparty-breakdown': {
    tool: 'a2a_cost_counterparty_breakdown',
    description: 'Get cost breakdown by counterparty',
    args: ['<agentAddress>', '[asset]', '[network]'],
    parse: ([agentAddress, asset, network]) => {
      if (!agentAddress) {
        throw new Error(
          'Usage: a2a-automation cost-counterparty-breakdown <agentAddress> [asset] [network]',
        );
      }
      return { params: { agentAddress, asset: asset || undefined, network: network || undefined } };
    },
  },
  'cost-operation-breakdown': {
    tool: 'a2a_cost_operation_breakdown',
    description: 'Get cost breakdown by operation',
    args: ['<agentAddress>', '[asset]', '[network]'],
    parse: ([agentAddress, asset, network]) => {
      if (!agentAddress) {
        throw new Error(
          'Usage: a2a-automation cost-operation-breakdown <agentAddress> [asset] [network]',
        );
      }
      return { params: { agentAddress, asset: asset || undefined, network: network || undefined } };
    },
  },
  'cost-daily-trend': {
    tool: 'a2a_cost_daily_trend',
    description: 'Get daily cost trend',
    args: ['<agentAddress>', '[days]', '[asset]', '[network]'],
    parse: ([agentAddress, daysRaw, asset, network]) => {
      if (!agentAddress) {
        throw new Error(
          'Usage: a2a-automation cost-daily-trend <agentAddress> [days] [asset] [network]',
        );
      }
      return {
        params: {
          agentAddress,
          days: parseOptionalInteger(
            daysRaw,
            'Usage: a2a-automation cost-daily-trend <agentAddress> [days] [asset] [network]',
          ),
          asset: asset || undefined,
          network: network || undefined,
        },
      };
    },
  },
  'cost-anomalies': {
    tool: 'a2a_cost_anomalies',
    description: 'Detect spending anomalies',
    args: ['<agentAddress>', '[asset]', '[network]'],
    parse: ([agentAddress, asset, network]) => {
      if (!agentAddress)
        throw new Error('Usage: a2a-automation cost-anomalies <agentAddress> [asset] [network]');
      return { params: { agentAddress, asset: asset || undefined, network: network || undefined } };
    },
  },
  'cost-margin-analysis': {
    tool: 'a2a_cost_margin_analysis',
    description: 'Get margin analysis',
    args: ['<agentAddress>', '[asset]', '[network]'],
    parse: ([agentAddress, asset, network]) => {
      if (!agentAddress) {
        throw new Error(
          'Usage: a2a-automation cost-margin-analysis <agentAddress> [asset] [network]',
        );
      }
      return { params: { agentAddress, asset: asset || undefined, network: network || undefined } };
    },
  },
  'cost-budget-forecast': {
    tool: 'a2a_cost_budget_forecast',
    description: 'Forecast budget exhaustion',
    args: ['<agentAddress>', '<monthlyBudget>', '[lookbackDays]', '[asset]', '[network]'],
    parse: ([agentAddress, monthlyBudgetRaw, lookbackDaysRaw, asset, network]) => {
      if (!agentAddress || !monthlyBudgetRaw) {
        throw new Error(
          'Usage: a2a-automation cost-budget-forecast <agentAddress> <monthlyBudget> [lookbackDays] [asset] [network]',
        );
      }
      return {
        params: {
          agentAddress,
          monthlyBudget: parseOptionalNumber(
            monthlyBudgetRaw,
            'Usage: a2a-automation cost-budget-forecast <agentAddress> <monthlyBudget> [lookbackDays] [asset] [network]',
          ),
          lookbackDays: parseOptionalInteger(
            lookbackDaysRaw,
            'Usage: a2a-automation cost-budget-forecast <agentAddress> <monthlyBudget> [lookbackDays] [asset] [network]',
          ),
          asset: asset || undefined,
          network: network || undefined,
        },
      };
    },
  },
  'cost-top-spenders': {
    tool: 'a2a_cost_top_spenders',
    description: 'List top spenders',
    args: ['[limit]', '[asset]', '[network]'],
    parse: ([limitRaw, asset, network]) => ({
      params: {
        limit: parseOptionalInteger(
          limitRaw,
          'Usage: a2a-automation cost-top-spenders [limit] [asset] [network]',
        ),
        asset: asset || undefined,
        network: network || undefined,
      },
    }),
  },
  'escrow-process-all': {
    tool: 'a2a_escrow_process_all',
    description: 'Process all escrows',
    args: [],
    parse: () => ({ params: {} }),
  },
};

export const toolActionMap = Object.entries(ACTIONS).map(([action, config]) => ({
  action,
  tool: config.tool,
}));

export async function execute(action, args, context) {
  const config = ACTIONS[action];
  if (!config) throw createUnknownActionError('a2a-automation', ACTIONS, action);

  const { params = {}, agentAddress } = config.parse(args);
  const result = await invokeTool(a2aAutomationTools, config.tool, context, params, {
    agentAddress,
  });
  return formatToolResult(result, context, 'No automation data found.');
}

export const metadata = createMetadata(
  'a2a-automation',
  ['a2aa', 'ops'],
  'A2A automation, maintenance, saga, and cost analytics commands',
  ACTIONS,
);

export default { execute, metadata };

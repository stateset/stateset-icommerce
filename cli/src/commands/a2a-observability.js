/**
 * A2A Observability Commands Module
 */

import { a2aObservabilityTools } from '../tools/a2a-observability.js';
import {
  createMetadata,
  createUnknownActionError,
  formatToolResult,
  invokeTool,
  parseCsvArg,
  parseJsonArg,
  parseOptionalBoolean,
  parseOptionalInteger,
} from '../command-tooling.js';

const ACTIONS = {
  trace: {
    tool: 'a2a_get_trace',
    description: 'Get a distributed trace',
    args: ['<traceId>'],
    parse: ([traceId]) => {
      if (!traceId) throw new Error('Usage: a2a-observability trace <traceId>');
      return { params: { traceId } };
    },
  },
  'tracing-metrics': {
    tool: 'a2a_tracing_metrics',
    description: 'Get tracing metrics',
    args: [],
    parse: () => ({ params: {} }),
  },
  'recent-spans': {
    tool: 'a2a_recent_spans',
    description: 'Get recent spans',
    args: ['[limit]'],
    parse: ([limitRaw]) => ({
      params: {
        limit: parseOptionalInteger(limitRaw, 'Usage: a2a-observability recent-spans [limit]'),
      },
    }),
  },
  'export-traces': {
    tool: 'a2a_export_traces',
    description: 'Export traces',
    args: [],
    parse: () => ({ params: {} }),
  },
  dashboard: {
    tool: 'a2a_agent_dashboard',
    description: 'Get an agent dashboard',
    args: ['<agentAddress>', '[asset]', '[network]', '[trendDays]'],
    parse: ([agentAddress, asset, network, trendDaysRaw]) => {
      if (!agentAddress) {
        throw new Error(
          'Usage: a2a-observability dashboard <agentAddress> [asset] [network] [trendDays]',
        );
      }
      return {
        params: {
          agentAddress,
          asset: asset || undefined,
          network: network || undefined,
          trendDays: parseOptionalInteger(
            trendDaysRaw,
            'Usage: a2a-observability dashboard <agentAddress> [asset] [network] [trendDays]',
          ),
        },
      };
    },
  },
  decisions: {
    tool: 'a2a_agent_decisions',
    description: 'Get agent decision history',
    args: ['<agentAddress>', '[limit]'],
    parse: ([agentAddress, limitRaw]) => {
      if (!agentAddress)
        throw new Error('Usage: a2a-observability decisions <agentAddress> [limit]');
      return {
        params: {
          agentAddress,
          limit: parseOptionalInteger(
            limitRaw,
            'Usage: a2a-observability decisions <agentAddress> [limit]',
          ),
        },
      };
    },
  },
  performance: {
    tool: 'a2a_agent_performance',
    description: 'Get agent performance report',
    args: ['<agentAddress>', '[asset]', '[network]', '[trendDays]'],
    parse: ([agentAddress, asset, network, trendDaysRaw]) => {
      if (!agentAddress) {
        throw new Error(
          'Usage: a2a-observability performance <agentAddress> [asset] [network] [trendDays]',
        );
      }
      return {
        params: {
          agentAddress,
          asset: asset || undefined,
          network: network || undefined,
          trendDays: parseOptionalInteger(
            trendDaysRaw,
            'Usage: a2a-observability performance <agentAddress> [asset] [network] [trendDays]',
          ),
        },
      };
    },
  },
  'tick-metrics': {
    tool: 'a2a_agent_tick_metrics',
    description: 'Get agent tick metrics',
    args: ['<agentAddress>'],
    parse: ([agentAddress]) => {
      if (!agentAddress) throw new Error('Usage: a2a-observability tick-metrics <agentAddress>');
      return { params: { agentAddress } };
    },
  },
  lifecycle: {
    tool: 'a2a_agent_lifecycle',
    description: 'Get agent lifecycle history',
    args: ['<agentAddress>'],
    parse: ([agentAddress]) => {
      if (!agentAddress) throw new Error('Usage: a2a-observability lifecycle <agentAddress>');
      return { params: { agentAddress } };
    },
  },
  alerts: {
    tool: 'a2a_agent_alerts',
    description: 'Get agent alerts',
    args: ['<agentAddress>', '[categoriesCsv]', '[asset]', '[network]', '[since]', '[limit]'],
    parse: ([agentAddress, categoriesCsv, asset, network, since, limitRaw]) => {
      if (!agentAddress) {
        throw new Error(
          'Usage: a2a-observability alerts <agentAddress> [categoriesCsv] [asset] [network] [since] [limit]',
        );
      }
      return {
        params: {
          agentAddress,
          categories: parseCsvArg(categoriesCsv),
          asset: asset || undefined,
          network: network || undefined,
          since: since || undefined,
          limit: parseOptionalInteger(
            limitRaw,
            'Usage: a2a-observability alerts <agentAddress> [categoriesCsv] [asset] [network] [since] [limit]',
          ),
        },
      };
    },
  },
  'settlement-status': {
    tool: 'a2a_settlement_status',
    description: 'Get settlement finality status',
    args: ['<intentId>', '[agentAddress]', '[refreshOnChain]'],
    parse: ([intentId, agentAddress, refreshOnChainRaw]) => {
      if (!intentId) {
        throw new Error(
          'Usage: a2a-observability settlement-status <intentId> [agentAddress] [refreshOnChain]',
        );
      }
      return {
        params: {
          intentId,
          agentAddress: agentAddress || undefined,
          refreshOnChain: parseOptionalBoolean(
            refreshOnChainRaw,
            'Usage: a2a-observability settlement-status <intentId> [agentAddress] [refreshOnChain]',
          ),
        },
      };
    },
  },
  'settlement-pending': {
    tool: 'a2a_settlement_pending',
    description: 'List pending settlements',
    args: ['[agentAddress]', '[network]', '[includeCompleted]', '[refreshOnChain]', '[limit]'],
    parse: ([agentAddress, network, includeCompletedRaw, refreshOnChainRaw, limitRaw]) => ({
      params: {
        agentAddress: agentAddress || undefined,
        network: network || undefined,
        includeCompleted: parseOptionalBoolean(
          includeCompletedRaw,
          'Usage: a2a-observability settlement-pending [agentAddress] [network] [includeCompleted] [refreshOnChain] [limit]',
        ),
        refreshOnChain: parseOptionalBoolean(
          refreshOnChainRaw,
          'Usage: a2a-observability settlement-pending [agentAddress] [network] [includeCompleted] [refreshOnChain] [limit]',
        ),
        limit: parseOptionalInteger(
          limitRaw,
          'Usage: a2a-observability settlement-pending [agentAddress] [network] [includeCompleted] [refreshOnChain] [limit]',
        ),
      },
    }),
  },
  'settlement-finality-metrics': {
    tool: 'a2a_settlement_finality_metrics',
    description: 'Get settlement finality metrics',
    args: ['[agentAddress]', '[network]', '[refreshOnChain]', '[limit]'],
    parse: ([agentAddress, network, refreshOnChainRaw, limitRaw]) => ({
      params: {
        agentAddress: agentAddress || undefined,
        network: network || undefined,
        refreshOnChain: parseOptionalBoolean(
          refreshOnChainRaw,
          'Usage: a2a-observability settlement-finality-metrics [agentAddress] [network] [refreshOnChain] [limit]',
        ),
        limit: parseOptionalInteger(
          limitRaw,
          'Usage: a2a-observability settlement-finality-metrics [agentAddress] [network] [refreshOnChain] [limit]',
        ),
      },
    }),
  },
  handshake: {
    tool: 'a2a_handshake',
    description: 'Initiate a capability handshake',
    args: ['<targetCapabilitiesJson>'],
    parse: ([targetCapabilitiesJson]) => {
      if (!targetCapabilitiesJson)
        throw new Error('Usage: a2a-observability handshake <targetCapabilitiesJson>');
      return {
        params: { targetCapabilities: parseJsonArg(targetCapabilitiesJson, 'targetCapabilities') },
      };
    },
  },
  'my-capabilities': {
    tool: 'a2a_my_capabilities',
    description: 'Get local capabilities',
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
  if (!config) throw createUnknownActionError('a2a-observability', ACTIONS, action);

  const { params = {}, agentAddress } = config.parse(args);
  const result = await invokeTool(a2aObservabilityTools, config.tool, context, params, {
    agentAddress,
  });
  return formatToolResult(result, context, 'No observability data found.');
}

export const metadata = createMetadata(
  'a2a-observability',
  ['a2ao', 'obs'],
  'A2A tracing, introspection, alerting, and finality commands',
  ACTIONS,
);

export default { execute, metadata };

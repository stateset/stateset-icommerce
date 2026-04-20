/**
 * Agent Runtime Commands Module
 */

import { agentRuntimeTools } from '../tools/agent-runtime.js';
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
  'create-runtime': {
    tool: 'agent_create_runtime',
    description: 'Create an agent runtime',
    args: ['<payloadJson>'],
    parse: ([payloadJson]) => ({ params: parseJsonArg(payloadJson, 'payload') }),
  },
  'destroy-runtime': {
    tool: 'agent_destroy_runtime',
    description: 'Destroy an agent runtime',
    args: ['<name>'],
    parse: ([name]) => {
      if (!name) throw new Error('Usage: agent-runtime destroy-runtime <name>');
      return { params: { name } };
    },
  },
  runtimes: {
    tool: 'agent_list_runtimes',
    description: 'List agent runtimes',
    args: ['[asset]', '[network]'],
    parse: ([asset, network]) => ({
      params: { asset: asset || undefined, network: network || undefined },
    }),
  },
  status: {
    tool: 'agent_get_status',
    description: 'Get runtime status',
    args: ['<name>', '[asset]', '[network]'],
    parse: ([name, asset, network]) => {
      if (!name) throw new Error('Usage: agent-runtime status <name> [asset] [network]');
      return { params: { name, asset: asset || undefined, network: network || undefined } };
    },
  },
  'set-strategy': {
    tool: 'agent_set_strategy',
    description: 'Set runtime strategy',
    args: ['<name>', '<strategy>', '[optionsJson]'],
    parse: ([name, strategy, optionsJson]) => {
      if (!name || !strategy) {
        throw new Error('Usage: agent-runtime set-strategy <name> <strategy> [optionsJson]');
      }
      return {
        params: {
          name,
          strategy,
          options: optionsJson ? parseJsonArg(optionsJson, 'options') : undefined,
        },
      };
    },
  },
  budget: {
    tool: 'agent_get_budget',
    description: 'Get runtime budget',
    args: ['<name>', '[asset]', '[network]'],
    parse: ([name, asset, network]) => {
      if (!name) throw new Error('Usage: agent-runtime budget <name> [asset] [network]');
      return { params: { name, asset: asset || undefined, network: network || undefined } };
    },
  },
  tick: {
    tool: 'agent_tick',
    description: 'Run one runtime tick',
    args: ['<name>'],
    parse: ([name]) => {
      if (!name) throw new Error('Usage: agent-runtime tick <name>');
      return { params: { name } };
    },
  },
  'start-loop': {
    tool: 'agent_start_loop',
    description: 'Start runtime loop',
    args: ['<name>', '[intervalMs]'],
    parse: ([name, intervalMsRaw]) => {
      if (!name) throw new Error('Usage: agent-runtime start-loop <name> [intervalMs]');
      return {
        params: {
          name,
          intervalMs: parseOptionalInteger(
            intervalMsRaw,
            'Usage: agent-runtime start-loop <name> [intervalMs]',
          ),
        },
      };
    },
  },
  'stop-loop': {
    tool: 'agent_stop_loop',
    description: 'Stop runtime loop',
    args: ['<name>'],
    parse: ([name]) => {
      if (!name) throw new Error('Usage: agent-runtime stop-loop <name>');
      return { params: { name } };
    },
  },
  'register-service': {
    tool: 'agent_register_service',
    description: 'Register a marketplace service',
    args: ['<name>', '<serviceName>', '<category>', '[description]', '[pricingModel]'],
    parse: ([name, serviceName, category, description, pricingModel]) => {
      if (!name || !serviceName || !category) {
        throw new Error(
          'Usage: agent-runtime register-service <name> <serviceName> <category> [description] [pricingModel]',
        );
      }
      return {
        params: {
          name,
          serviceName,
          category,
          description: description || undefined,
          pricingModel: pricingModel || undefined,
        },
      };
    },
  },
  'discover-services': {
    tool: 'agent_discover_services',
    description: 'Discover marketplace services',
    args: ['<name>', '[category]'],
    parse: ([name, category]) => {
      if (!name) throw new Error('Usage: agent-runtime discover-services <name> [category]');
      return { params: { name, category: category || undefined } };
    },
  },
  'create-escrow-deal': {
    tool: 'agent_create_escrow_deal',
    description: 'Create an escrow-backed deal',
    args: ['<payloadJson>'],
    parse: ([payloadJson]) => ({ params: parseJsonArg(payloadJson, 'payload') }),
  },
  'subscribe-to-service': {
    tool: 'agent_subscribe_to_service',
    description: 'Subscribe to a service',
    args: ['<payloadJson>'],
    parse: ([payloadJson]) => ({ params: parseJsonArg(payloadJson, 'payload') }),
  },
  'rate-counterparty': {
    tool: 'agent_rate_counterparty',
    description: 'Rate a counterparty',
    args: ['<payloadJson>'],
    parse: ([payloadJson]) => ({ params: parseJsonArg(payloadJson, 'payload') }),
  },
  reputation: {
    tool: 'agent_get_reputation',
    description: 'Get agent reputation',
    args: ['<address>'],
    parse: ([address]) => {
      if (!address) throw new Error('Usage: agent-runtime reputation <address>');
      return { params: { address } };
    },
  },
  'create-split-deal': {
    tool: 'agent_create_split_deal',
    description: 'Create a split payment deal',
    args: ['<payloadJson>'],
    parse: ([payloadJson]) => ({ params: parseJsonArg(payloadJson, 'payload') }),
  },
  'event-history': {
    tool: 'agent_get_event_history',
    description: 'Get runtime event history',
    args: ['<name>', '[eventTypesCsv]', '[since]', '[asset]', '[network]', '[limit]'],
    parse: ([name, eventTypesCsv, since, asset, network, limitRaw]) => {
      if (!name) {
        throw new Error(
          'Usage: agent-runtime event-history <name> [eventTypesCsv] [since] [asset] [network] [limit]',
        );
      }
      return {
        params: {
          name,
          eventTypes: parseCsvArg(eventTypesCsv),
          since: since || undefined,
          asset: asset || undefined,
          network: network || undefined,
          limit: parseOptionalInteger(
            limitRaw,
            'Usage: agent-runtime event-history <name> [eventTypesCsv] [since] [asset] [network] [limit]',
          ),
        },
      };
    },
  },
  'enable-settlement': {
    tool: 'agent_enable_settlement',
    description: 'Enable runtime settlement',
    args: ['<name>', '[chainId]', '[simulate]', '[tokenSymbol]'],
    parse: ([name, chainId, simulateRaw, tokenSymbol]) => {
      if (!name) {
        throw new Error(
          'Usage: agent-runtime enable-settlement <name> [chainId] [simulate] [tokenSymbol]',
        );
      }
      return {
        params: {
          name,
          chainId: chainId || undefined,
          simulate: parseOptionalBoolean(
            simulateRaw,
            'Usage: agent-runtime enable-settlement <name> [chainId] [simulate] [tokenSymbol]',
          ),
          tokenSymbol: tokenSymbol || undefined,
        },
      };
    },
  },
  'chain-balance': {
    tool: 'agent_get_chain_balance',
    description: 'Get settlement chain balance',
    args: ['<name>', '[chainId]'],
    parse: ([name, chainId]) => {
      if (!name) throw new Error('Usage: agent-runtime chain-balance <name> [chainId]');
      return { params: { name, chainId: chainId || undefined } };
    },
  },
  'broadcast-rfq': {
    tool: 'agent_broadcast_rfq',
    description: 'Broadcast an RFQ',
    args: ['<payloadJson>'],
    parse: ([payloadJson]) => ({ params: parseJsonArg(payloadJson, 'payload') }),
  },
  'collect-rfq-responses': {
    tool: 'agent_collect_rfq_responses',
    description: 'Collect RFQ responses',
    args: ['<name>', '<rfqId>'],
    parse: ([name, rfqId]) => {
      if (!name || !rfqId) {
        throw new Error('Usage: agent-runtime collect-rfq-responses <name> <rfqId>');
      }
      return { params: { name, rfqId } };
    },
  },
  'award-rfq': {
    tool: 'agent_award_rfq',
    description: 'Award an RFQ',
    args: ['<name>', '<rfqId>', '[winnerId]'],
    parse: ([name, rfqId, winnerId]) => {
      if (!name || !rfqId)
        throw new Error('Usage: agent-runtime award-rfq <name> <rfqId> [winnerId]');
      return { params: { name, rfqId, winnerId: winnerId || undefined } };
    },
  },
  'marketplace-metrics': {
    tool: 'agent_get_marketplace_metrics',
    description: 'Get marketplace metrics',
    args: ['<serviceId>'],
    parse: ([serviceId]) => {
      if (!serviceId) throw new Error('Usage: agent-runtime marketplace-metrics <serviceId>');
      return { params: { serviceId } };
    },
  },
  'attach-sla': {
    tool: 'agent_attach_sla',
    description: 'Attach an SLA',
    args: ['<payloadJson>'],
    parse: ([payloadJson]) => ({ params: parseJsonArg(payloadJson, 'payload') }),
  },
  'check-sla-compliance': {
    tool: 'agent_check_sla_compliance',
    description: 'Check SLA compliance',
    args: ['<serviceId>'],
    parse: ([serviceId]) => {
      if (!serviceId) throw new Error('Usage: agent-runtime check-sla-compliance <serviceId>');
      return { params: { serviceId } };
    },
  },
  'create-workflow': {
    tool: 'agent_create_workflow',
    description: 'Create a workflow',
    args: ['<payloadJson>'],
    parse: ([payloadJson]) => ({ params: parseJsonArg(payloadJson, 'payload') }),
  },
  'execute-workflow': {
    tool: 'agent_execute_workflow',
    description: 'Execute a workflow',
    args: ['<name>', '<workflowId>', '[contextJson]'],
    parse: ([name, workflowId, contextJson]) => {
      if (!name || !workflowId) {
        throw new Error('Usage: agent-runtime execute-workflow <name> <workflowId> [contextJson]');
      }
      return {
        params: {
          name,
          workflowId,
          context: contextJson ? parseJsonArg(contextJson, 'context') : undefined,
        },
      };
    },
  },
  'workflow-status': {
    tool: 'agent_get_workflow_status',
    description: 'Get workflow status',
    args: ['<workflowId>'],
    parse: ([workflowId]) => {
      if (!workflowId) throw new Error('Usage: agent-runtime workflow-status <workflowId>');
      return { params: { workflowId } };
    },
  },
  'set-dynamic-pricing': {
    tool: 'agent_set_dynamic_pricing',
    description: 'Configure dynamic pricing',
    args: [
      '<name>',
      '[volumeBreaksJson]',
      '[reputationTiersJson]',
      '[peakHoursJson]',
      '[loyaltyTiersJson]',
    ],
    parse: ([name, volumeBreaksJson, reputationTiersJson, peakHoursJson, loyaltyTiersJson]) => {
      if (!name) {
        throw new Error(
          'Usage: agent-runtime set-dynamic-pricing <name> [volumeBreaksJson] [reputationTiersJson] [peakHoursJson] [loyaltyTiersJson]',
        );
      }
      return {
        params: {
          name,
          volumeBreaks: volumeBreaksJson
            ? parseJsonArg(volumeBreaksJson, 'volumeBreaks')
            : undefined,
          reputationTiers: reputationTiersJson
            ? parseJsonArg(reputationTiersJson, 'reputationTiers')
            : undefined,
          peakHours: peakHoursJson ? parseJsonArg(peakHoursJson, 'peakHours') : undefined,
          loyaltyTiers: loyaltyTiersJson
            ? parseJsonArg(loyaltyTiersJson, 'loyaltyTiers')
            : undefined,
        },
      };
    },
  },
};

export const toolActionMap = Object.entries(ACTIONS).map(([action, config]) => ({
  action,
  tool: config.tool,
}));

export async function execute(action, args, context) {
  const config = ACTIONS[action];
  if (!config) throw createUnknownActionError('agent-runtime', ACTIONS, action);

  const { params = {}, agentAddress } = config.parse(args);
  const result = await invokeTool(agentRuntimeTools, config.tool, context, params, {
    agentAddress,
  });
  return formatToolResult(result, context, 'No agent runtimes found.');
}

export const metadata = createMetadata(
  'agent-runtime',
  ['runtime', 'rt'],
  'Autonomous agent runtime orchestration commands',
  ACTIONS,
);

export default { execute, metadata };

/**
 * A2A Intelligence Commands Module
 */

import { a2aIntelligenceTools } from '../tools/a2a-intelligence.js';
import {
  createMetadata,
  createUnknownActionError,
  formatToolResult,
  invokeTool,
  parseCsvArg,
  parseJsonArg,
  parseOptionalInteger,
  parseOptionalNumber,
} from '../command-tooling.js';

const ACTIONS = {
  'schedule-action': {
    tool: 'a2a_schedule_action',
    description: 'Schedule a future action',
    args: [
      '<agentAddress>',
      '<actionType>',
      '<payloadJson>',
      '<executeAt>',
      '[repeatInterval]',
      '[maxExecutions]',
      '[description]',
    ],
    parse: ([
      agentAddress,
      actionType,
      payloadJson,
      executeAt,
      repeatIntervalRaw,
      maxExecutionsRaw,
      description,
    ]) => {
      if (!agentAddress || !actionType || !payloadJson || !executeAt) {
        throw new Error(
          'Usage: a2a-intelligence schedule-action <agentAddress> <actionType> <payloadJson> <executeAt> [repeatInterval] [maxExecutions] [description]',
        );
      }
      return {
        agentAddress,
        params: {
          actionType,
          payload: parseJsonArg(payloadJson, 'payload'),
          executeAt,
          repeatInterval: parseOptionalInteger(
            repeatIntervalRaw,
            'Usage: a2a-intelligence schedule-action <agentAddress> <actionType> <payloadJson> <executeAt> [repeatInterval] [maxExecutions] [description]',
          ),
          maxExecutions: parseOptionalInteger(
            maxExecutionsRaw,
            'Usage: a2a-intelligence schedule-action <agentAddress> <actionType> <payloadJson> <executeAt> [repeatInterval] [maxExecutions] [description]',
          ),
          description: description || undefined,
        },
      };
    },
  },
  'cancel-scheduled': {
    tool: 'a2a_cancel_scheduled',
    description: 'Cancel a scheduled action',
    args: ['<actionId>'],
    parse: ([actionId]) => {
      if (!actionId) throw new Error('Usage: a2a-intelligence cancel-scheduled <actionId>');
      return { params: { actionId } };
    },
  },
  scheduled: {
    tool: 'a2a_list_scheduled',
    description: 'List scheduled actions',
    args: ['<agentAddress>', '[status]', '[actionType]'],
    parse: ([agentAddress, status, actionType]) => {
      if (!agentAddress)
        throw new Error('Usage: a2a-intelligence scheduled <agentAddress> [status] [actionType]');
      return {
        agentAddress,
        params: {
          status: status || undefined,
          actionType: actionType || undefined,
        },
      };
    },
  },
  'scheduler-metrics': {
    tool: 'a2a_scheduler_metrics',
    description: 'Get scheduler metrics',
    args: [],
    parse: () => ({ params: {} }),
  },
  'remember-interaction': {
    tool: 'a2a_remember_interaction',
    description: 'Record a counterparty interaction',
    args: [
      '<agentAddress>',
      '<counterpartyAddress>',
      '<interactionType>',
      '<outcome>',
      '[amount]',
      '[responseTimeMs]',
      '[metadataJson]',
    ],
    parse: ([
      agentAddress,
      counterpartyAddress,
      interactionType,
      outcome,
      amountRaw,
      responseTimeMsRaw,
      metadataJson,
    ]) => {
      if (!agentAddress || !counterpartyAddress || !interactionType || !outcome) {
        throw new Error(
          'Usage: a2a-intelligence remember-interaction <agentAddress> <counterpartyAddress> <interactionType> <outcome> [amount] [responseTimeMs] [metadataJson]',
        );
      }
      return {
        agentAddress,
        params: {
          counterpartyAddress,
          interactionType,
          outcome,
          amount: parseOptionalNumber(
            amountRaw,
            'Usage: a2a-intelligence remember-interaction <agentAddress> <counterpartyAddress> <interactionType> <outcome> [amount] [responseTimeMs] [metadataJson]',
          ),
          responseTimeMs: parseOptionalInteger(
            responseTimeMsRaw,
            'Usage: a2a-intelligence remember-interaction <agentAddress> <counterpartyAddress> <interactionType> <outcome> [amount] [responseTimeMs] [metadataJson]',
          ),
          metadata: metadataJson ? parseJsonArg(metadataJson, 'metadata') : undefined,
        },
      };
    },
  },
  'counterparty-profile': {
    tool: 'a2a_counterparty_profile',
    description: 'Get a counterparty profile',
    args: ['<agentAddress>', '<counterpartyAddress>'],
    parse: ([agentAddress, counterpartyAddress]) => {
      if (!agentAddress || !counterpartyAddress) {
        throw new Error(
          'Usage: a2a-intelligence counterparty-profile <agentAddress> <counterpartyAddress>',
        );
      }
      return { agentAddress, params: { counterpartyAddress } };
    },
  },
  'should-transact': {
    tool: 'a2a_should_transact',
    description: 'Get a transact recommendation',
    args: ['<agentAddress>', '<counterpartyAddress>', '[actionType]'],
    parse: ([agentAddress, counterpartyAddress, actionType]) => {
      if (!agentAddress || !counterpartyAddress) {
        throw new Error(
          'Usage: a2a-intelligence should-transact <agentAddress> <counterpartyAddress> [actionType]',
        );
      }
      return { agentAddress, params: { counterpartyAddress, actionType: actionType || undefined } };
    },
  },
  'agent-insights': {
    tool: 'a2a_agent_insights',
    description: 'Get aggregate agent insights',
    args: ['<agentAddress>'],
    parse: ([agentAddress]) => {
      if (!agentAddress) throw new Error('Usage: a2a-intelligence agent-insights <agentAddress>');
      return { agentAddress, params: {} };
    },
  },
  'top-counterparties': {
    tool: 'a2a_top_counterparties',
    description: 'Get top counterparties',
    args: ['<agentAddress>', '[sortBy]', '[limit]'],
    parse: ([agentAddress, sortBy, limitRaw]) => {
      if (!agentAddress)
        throw new Error(
          'Usage: a2a-intelligence top-counterparties <agentAddress> [sortBy] [limit]',
        );
      return {
        agentAddress,
        params: {
          sortBy: sortBy || undefined,
          limit: parseOptionalInteger(
            limitRaw,
            'Usage: a2a-intelligence top-counterparties <agentAddress> [sortBy] [limit]',
          ),
        },
      };
    },
  },
  'add-rule': {
    tool: 'a2a_add_rule',
    description: 'Add a rules-engine rule',
    args: ['<agentAddress>', '<payloadJson>'],
    parse: ([agentAddress, payloadJson]) => {
      if (!agentAddress || !payloadJson)
        throw new Error('Usage: a2a-intelligence add-rule <agentAddress> <payloadJson>');
      return { agentAddress, params: parseJsonArg(payloadJson, 'payload') };
    },
  },
  'evaluate-rules': {
    tool: 'a2a_evaluate_rules',
    description: 'Evaluate rules against context',
    args: ['<contextJson>'],
    parse: ([contextJson]) => {
      if (!contextJson) throw new Error('Usage: a2a-intelligence evaluate-rules <contextJson>');
      return { params: { context: parseJsonArg(contextJson, 'context') } };
    },
  },
  'list-rules': {
    tool: 'a2a_list_rules',
    description: 'List rules',
    args: ['<agentAddress>', '[tagsCsv]', '[enabled]'],
    parse: ([agentAddress, tagsCsv, enabledRaw]) => {
      if (!agentAddress)
        throw new Error('Usage: a2a-intelligence list-rules <agentAddress> [tagsCsv] [enabled]');
      return {
        agentAddress,
        params: {
          tags: parseCsvArg(tagsCsv),
          enabled:
            enabledRaw === undefined
              ? undefined
              : ['true', '1', 'yes', 'y', 'on'].includes(String(enabledRaw).toLowerCase()),
        },
      };
    },
  },
  'rule-audit-log': {
    tool: 'a2a_rule_audit_log',
    description: 'Get rule audit log',
    args: ['[limit]'],
    parse: ([limitRaw]) => ({
      params: {
        limit: parseOptionalInteger(limitRaw, 'Usage: a2a-intelligence rule-audit-log [limit]'),
      },
    }),
  },
  scatter: {
    tool: 'a2a_scatter',
    description: 'Scatter a task to multiple agents',
    args: [
      '<agentAddress>',
      '<targetsJson>',
      '<taskType>',
      '<payloadJson>',
      '[timeoutMs]',
      '[joinStrategy]',
    ],
    parse: ([agentAddress, targetsJson, taskType, payloadJson, timeoutMsRaw, joinStrategy]) => {
      if (!agentAddress || !targetsJson || !taskType || !payloadJson) {
        throw new Error(
          'Usage: a2a-intelligence scatter <agentAddress> <targetsJson> <taskType> <payloadJson> [timeoutMs] [joinStrategy]',
        );
      }
      return {
        agentAddress,
        params: {
          targets: parseJsonArg(targetsJson, 'targets'),
          taskType,
          payload: parseJsonArg(payloadJson, 'payload'),
          timeoutMs: parseOptionalInteger(
            timeoutMsRaw,
            'Usage: a2a-intelligence scatter <agentAddress> <targetsJson> <taskType> <payloadJson> [timeoutMs] [joinStrategy]',
          ),
          joinStrategy: joinStrategy || undefined,
        },
      };
    },
  },
  'coordination-status': {
    tool: 'a2a_coordination_status',
    description: 'Get fan-out coordination status',
    args: ['<coordinationId>'],
    parse: ([coordinationId]) => {
      if (!coordinationId)
        throw new Error('Usage: a2a-intelligence coordination-status <coordinationId>');
      return { params: { coordinationId } };
    },
  },
  'submit-response': {
    tool: 'a2a_submit_response',
    description: 'Submit a fan-out response',
    args: ['<agentAddress>', '<coordinationId>', '<responseJson>'],
    parse: ([agentAddress, coordinationId, responseJson]) => {
      if (!agentAddress || !coordinationId || !responseJson) {
        throw new Error(
          'Usage: a2a-intelligence submit-response <agentAddress> <coordinationId> <responseJson>',
        );
      }
      return {
        agentAddress,
        params: {
          coordinationId,
          response: parseJsonArg(responseJson, 'response'),
        },
      };
    },
  },
  'join-results': {
    tool: 'a2a_join_results',
    description: 'Join fan-out results',
    args: ['<coordinationId>'],
    parse: ([coordinationId]) => {
      if (!coordinationId) throw new Error('Usage: a2a-intelligence join-results <coordinationId>');
      return { params: { coordinationId } };
    },
  },
};

export const toolActionMap = Object.entries(ACTIONS).map(([action, config]) => ({
  action,
  tool: config.tool,
}));

export async function execute(action, args, context) {
  const config = ACTIONS[action];
  if (!config) throw createUnknownActionError('a2a-intelligence', ACTIONS, action);

  const { params = {}, agentAddress } = config.parse(args);
  const result = await invokeTool(a2aIntelligenceTools, config.tool, context, params, {
    agentAddress,
  });
  return formatToolResult(result, context, 'No intelligence data found.');
}

export const metadata = createMetadata(
  'a2a-intelligence',
  ['a2ai', 'intel'],
  'A2A scheduling, memory, rules, and coordination commands',
  ACTIONS,
);

export default { execute, metadata };

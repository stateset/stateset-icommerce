/**
 * A2A Platform Commands Module
 */

import { a2aPlatformTools } from '../tools/a2a-platform.js';
import {
  createMetadata,
  createUnknownActionError,
  formatToolResult,
  invokeTool,
  parseJsonArg,
  parseOptionalBoolean,
  parseOptionalInteger,
  parseOptionalNumber,
} from '../command-tooling.js';

const ACTIONS = {
  'send-message': {
    tool: 'a2a_send_message',
    description: 'Send a direct agent message',
    args: ['<agentAddress>', '<to>', '<type>', '<payloadJson>', '[parentMessageId]'],
    parse: ([agentAddress, to, type, payloadJson, parentMessageId]) => {
      if (!agentAddress || !to || !type || !payloadJson) {
        throw new Error(
          'Usage: a2a-platform send-message <agentAddress> <to> <type> <payloadJson> [parentMessageId]',
        );
      }
      return {
        agentAddress,
        params: {
          to,
          type,
          payload: parseJsonArg(payloadJson, 'payload'),
          parentMessageId: parentMessageId || undefined,
        },
      };
    },
  },
  inbox: {
    tool: 'a2a_get_inbox',
    description: 'Get agent inbox',
    args: ['<agentAddress>', '[unreadOnly]', '[type]', '[limit]'],
    parse: ([agentAddress, unreadOnlyRaw, type, limitRaw]) => {
      if (!agentAddress)
        throw new Error('Usage: a2a-platform inbox <agentAddress> [unreadOnly] [type] [limit]');
      return {
        agentAddress,
        params: {
          unreadOnly: parseOptionalBoolean(
            unreadOnlyRaw,
            'Usage: a2a-platform inbox <agentAddress> [unreadOnly] [type] [limit]',
          ),
          type: type || undefined,
          limit: parseOptionalInteger(
            limitRaw,
            'Usage: a2a-platform inbox <agentAddress> [unreadOnly] [type] [limit]',
          ),
        },
      };
    },
  },
  'delegate-task': {
    tool: 'a2a_delegate_task',
    description: 'Delegate a task to another agent',
    args: ['<agentAddress>', '<to>', '<description>', '[deadline]', '[reward]', '[priority]'],
    parse: ([agentAddress, to, description, deadline, rewardRaw, priority]) => {
      if (!agentAddress || !to || !description) {
        throw new Error(
          'Usage: a2a-platform delegate-task <agentAddress> <to> <description> [deadline] [reward] [priority]',
        );
      }
      return {
        agentAddress,
        params: {
          to,
          description,
          deadline: deadline || undefined,
          reward: parseOptionalNumber(
            rewardRaw,
            'Usage: a2a-platform delegate-task <agentAddress> <to> <description> [deadline] [reward] [priority]',
          ),
          priority: priority || undefined,
        },
      };
    },
  },
  'respond-to-task': {
    tool: 'a2a_respond_to_task',
    description: 'Respond to a delegated task',
    args: ['<messageId>', '<status>', '[resultJson]'],
    parse: ([messageId, status, resultJson]) => {
      if (!messageId || !status) {
        throw new Error('Usage: a2a-platform respond-to-task <messageId> <status> [resultJson]');
      }
      return {
        params: {
          messageId,
          status,
          result: resultJson ? parseJsonArg(resultJson, 'result') : undefined,
        },
      };
    },
  },
  thread: {
    tool: 'a2a_get_thread',
    description: 'Get a message thread',
    args: ['<parentMessageId>'],
    parse: ([parentMessageId]) => {
      if (!parentMessageId) throw new Error('Usage: a2a-platform thread <parentMessageId>');
      return { params: { parentMessageId } };
    },
  },
  'messaging-metrics': {
    tool: 'a2a_messaging_metrics',
    description: 'Get messaging metrics',
    args: [],
    parse: () => ({ params: {} }),
  },
  'batch-pay': {
    tool: 'a2a_batch_pay',
    description: 'Execute batch payments',
    args: ['<paymentsJson>', '[concurrency]'],
    parse: ([paymentsJson, concurrencyRaw]) => {
      if (!paymentsJson)
        throw new Error('Usage: a2a-platform batch-pay <paymentsJson> [concurrency]');
      return {
        params: {
          payments: parseJsonArg(paymentsJson, 'payments'),
          concurrency: parseOptionalInteger(
            concurrencyRaw,
            'Usage: a2a-platform batch-pay <paymentsJson> [concurrency]',
          ),
        },
      };
    },
  },
  'batch-request-quotes': {
    tool: 'a2a_batch_request_quotes',
    description: 'Request quotes in batch',
    args: ['<requestsJson>'],
    parse: ([requestsJson]) => {
      if (!requestsJson) throw new Error('Usage: a2a-platform batch-request-quotes <requestsJson>');
      return { params: { requests: parseJsonArg(requestsJson, 'requests') } };
    },
  },
  'save-checkpoint': {
    tool: 'a2a_save_checkpoint',
    description: 'Save an agent checkpoint',
    args: ['<agentAddress>', '[dataJson]'],
    parse: ([agentAddress, dataJson]) => {
      if (!agentAddress)
        throw new Error('Usage: a2a-platform save-checkpoint <agentAddress> [dataJson]');
      return {
        agentAddress,
        params: {
          data: dataJson ? parseJsonArg(dataJson, 'data') : undefined,
        },
      };
    },
  },
  'load-checkpoint': {
    tool: 'a2a_load_checkpoint',
    description: 'Load an agent checkpoint',
    args: ['<agentAddress>'],
    parse: ([agentAddress]) => {
      if (!agentAddress) throw new Error('Usage: a2a-platform load-checkpoint <agentAddress>');
      return { agentAddress, params: {} };
    },
  },
  checkpoints: {
    tool: 'a2a_list_checkpoints',
    description: 'List saved checkpoints',
    args: [],
    parse: () => ({ params: {} }),
  },
  'export-agent-data': {
    tool: 'a2a_export_agent_data',
    description: 'Export agent data',
    args: ['<agentAddress>', '[redact]'],
    parse: ([agentAddress, redactRaw]) => {
      if (!agentAddress)
        throw new Error('Usage: a2a-platform export-agent-data <agentAddress> [redact]');
      return {
        params: {
          agentAddress,
          redact: parseOptionalBoolean(
            redactRaw,
            'Usage: a2a-platform export-agent-data <agentAddress> [redact]',
          ),
        },
      };
    },
  },
  'commerce-report': {
    tool: 'a2a_commerce_report',
    description: 'Generate an agent commerce report',
    args: ['<agentAddress>', '[since]', '[until]'],
    parse: ([agentAddress, since, until]) => {
      if (!agentAddress)
        throw new Error('Usage: a2a-platform commerce-report <agentAddress> [since] [until]');
      return { params: { agentAddress, since: since || undefined, until: until || undefined } };
    },
  },
  'data-stats': {
    tool: 'a2a_data_stats',
    description: 'Get A2A data stats',
    args: [],
    parse: () => ({ params: {} }),
  },
  'verify-webhook': {
    tool: 'a2a_verify_webhook',
    description: 'Verify webhook signature',
    args: ['<rawBody>', '<signatureHeader>', '<secret>', '[timestampHeader]'],
    parse: ([rawBody, signatureHeader, secret, timestampHeader]) => {
      if (!rawBody || !signatureHeader || !secret) {
        throw new Error(
          'Usage: a2a-platform verify-webhook <rawBody> <signatureHeader> <secret> [timestampHeader]',
        );
      }
      return {
        params: {
          rawBody,
          signatureHeader,
          secret,
          timestampHeader: timestampHeader || undefined,
        },
      };
    },
  },
  'tick-metrics': {
    tool: 'a2a_tick_metrics',
    description: 'Get tick optimizer metrics',
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
  if (!config) throw createUnknownActionError('a2a-platform', ACTIONS, action);

  const { params = {}, agentAddress } = config.parse(args);
  const result = await invokeTool(a2aPlatformTools, config.tool, context, params, { agentAddress });
  return formatToolResult(result, context, 'No platform data found.');
}

export const metadata = createMetadata(
  'a2a-platform',
  ['a2ap', 'messaging'],
  'A2A messaging, batch, checkpoint, export, and webhook commands',
  ACTIONS,
);

export default { execute, metadata };

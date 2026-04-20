/**
 * Agent Cards Commands Module
 */

import { agentCardTools } from '../tools/agent-cards.js';
import {
  createMetadata,
  createUnknownActionError,
  formatToolResult,
  invokeTool,
  parseOptionalBoolean,
  parseOptionalInteger,
  parseJsonArg,
} from '../command-tooling.js';

const ACTIONS = {
  register: {
    tool: 'register_agent_card',
    description: 'Register an agent card',
    args: ['<payloadJson>'],
    parse: ([payloadJson]) => ({
      params: parseJsonArg(payloadJson, 'payload'),
    }),
  },
  discover: {
    tool: 'discover_agents',
    description: 'Discover agent cards',
    args: ['[network]', '[asset]', '[skill]', '[trustLevel]'],
    parse: ([network, asset, skill, trustLevel]) => ({
      params: {
        network: network || undefined,
        asset: asset || undefined,
        skill: skill || undefined,
        trustLevel: trustLevel || undefined,
      },
    }),
  },
  get: {
    tool: 'get_agent_card',
    description: 'Get agent card details',
    args: ['<agentIdOrWalletAddress>'],
    parse: ([agentIdOrWalletAddress]) => {
      if (!agentIdOrWalletAddress) {
        throw new Error('Usage: agent-cards get <agentIdOrWalletAddress>');
      }
      return {
        params: agentIdOrWalletAddress.startsWith('0x')
          ? { walletAddress: agentIdOrWalletAddress }
          : { agentId: agentIdOrWalletAddress },
      };
    },
  },
  verify: {
    tool: 'verify_agent',
    description: 'Verify an agent card',
    args: ['<agentId>'],
    parse: ([agentId]) => {
      if (!agentId) throw new Error('Usage: agent-cards verify <agentId>');
      return { params: { agentId } };
    },
  },
  list: {
    tool: 'list_agent_cards',
    description: 'List agent cards',
    args: ['[active]', '[trustLevel]', '[limit]'],
    parse: ([activeRaw, trustLevel, limitRaw]) => ({
      params: {
        active: parseOptionalBoolean(
          activeRaw,
          'Usage: agent-cards list [active] [trustLevel] [limit]',
        ),
        trustLevel: trustLevel || undefined,
        limit: parseOptionalInteger(
          limitRaw,
          'Usage: agent-cards list [active] [trustLevel] [limit]',
        ),
      },
    }),
  },
};

export const toolActionMap = Object.entries(ACTIONS).map(([action, config]) => ({
  action,
  tool: config.tool,
}));

export async function execute(action, args, context) {
  const config = ACTIONS[action];
  if (!config) throw createUnknownActionError('agent-cards', ACTIONS, action);

  const { params = {}, agentAddress } = config.parse(args);
  const result = await invokeTool(agentCardTools, config.tool, context, params, { agentAddress });
  return formatToolResult(result, context, 'No agent cards found.');
}

export const metadata = createMetadata(
  'agent-cards',
  ['cards', 'agent-card'],
  'A2A agent card registration and discovery commands',
  ACTIONS,
);

export default { execute, metadata };

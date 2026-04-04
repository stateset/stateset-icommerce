/**
 * Agent Card Tools Module
 *
 * MCP tool definitions for A2A (Agent-to-Agent) commerce agent card operations.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { resolveCommerceApi } from '../commerce.js';

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

/**
 * Agent card tool definitions
 */
export const agentCardTools = [
  {
    name: 'register_agent_card',
    description:
      'Register an AI agent card for A2A commerce. Advertises capabilities, supported networks, and payment assets.',
    inputSchema: {
      name: z.string().min(1).describe('Agent name'),
      walletAddress: z.string().min(1).describe('Agent wallet address for receiving payments'),
      publicKey: z.string().min(1).describe('Ed25519 public key for verifying signatures'),
      supportedNetworks: z
        .array(z.string())
        .optional()
        .describe('Networks: set_chain, base, ethereum, arbitrum, bitcoin, zcash'),
      supportedAssets: z
        .array(z.string())
        .optional()
        .describe('Assets: usdc, ssusd, usdt, btc, zec'),
      paymentAddresses: z
        .record(z.string(), z.string())
        .optional()
        .describe(
          'Network-specific receive addresses (e.g., { bitcoin: "bc1...", zcash: "u1..." })',
        ),
      skills: z
        .array(z.string())
        .optional()
        .describe('A2A skills: sell, buy, quote, fulfill, deliver'),
      endpointUrl: z.string().url().optional().describe('A2A endpoint URL (must be https)'),
      description: z.string().optional().describe('Agent description'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Registering agent card requires --apply flag.',
          wouldRegister: { name: params.name, walletAddress: params.walletAddress },
        };
      }

      if (params.endpointUrl) {
        try {
          const parsed = new URL(params.endpointUrl);
          if (parsed.protocol !== 'https:' && parsed.protocol !== 'http:') {
            return { success: false, error: 'endpointUrl must use http or https protocol.' };
          }
        } catch (err) {
          console.debug('[agent-cards] Endpoint URL validation failed:', err.message || err);
          return { success: false, error: 'endpointUrl is not a valid URL.' };
        }
      }

      const x402 = resolveCommerceApi(commerce, 'x402');

      const card = await x402.registerAgent({
        name: params.name,
        walletAddress: params.walletAddress,
        wallet_address: params.walletAddress,
        publicKey: params.publicKey,
        public_key: params.publicKey,
        supportedNetworks: params.supportedNetworks,
        supported_networks: params.supportedNetworks,
        supportedAssets: params.supportedAssets,
        supported_assets: params.supportedAssets,
        paymentAddresses: params.paymentAddresses,
        payment_addresses: params.paymentAddresses,
        a2aSkills: params.skills,
        a2a_skills: params.skills,
        endpointUrl: params.endpointUrl,
        endpoint_url: params.endpointUrl,
        description: params.description,
      });
      return {
        success: true,
        message: 'Agent card registered.',
        agent: {
          id: card.id,
          name: card.name,
          walletAddress: card.wallet_address,
          trustLevel: card.trust_level,
          active: card.active,
          supportedNetworks: card.supported_networks,
          supportedAssets: card.supported_assets,
          paymentAddresses: parseJsonObject(card.payment_addresses),
          skills: card.a2a_skills,
        },
      };
    },
  },

  {
    name: 'discover_agents',
    description:
      'Discover AI agents with specific commerce capabilities. Find sellers, buyers, or agents supporting specific networks/assets.',
    inputSchema: {
      network: z
        .string()
        .optional()
        .describe('Filter by network: set_chain, base, ethereum, bitcoin, zcash'),
      asset: z.string().optional().describe('Filter by asset: usdc, ssusd, usdt, btc, zec'),
      skill: z.string().optional().describe('Filter by skill: sell, buy, quote, fulfill'),
      trustLevel: z
        .string()
        .optional()
        .describe('Minimum trust level: sandbox, standard, verified, enterprise'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const x402 = resolveCommerceApi(commerce, 'x402');
      const agents = await x402.discoverAgents(
        params.network,
        params.asset,
        params.skill,
        params.trustLevel,
      );
      return {
        success: true,
        count: agents.length,
        agents: agents.map((a) => ({
          id: a.id,
          name: a.name,
          walletAddress: a.wallet_address,
          trustLevel: a.trust_level,
          supportedNetworks: a.supported_networks,
          supportedAssets: a.supported_assets,
          paymentAddresses: parseJsonObject(a.payment_addresses),
          skills: a.a2a_skills,
          endpointUrl: a.endpoint_url,
        })),
      };
    },
  },

  {
    name: 'get_agent_card',
    description: 'Get details of a registered AI agent card.',
    inputSchema: {
      agentId: z.string().optional().describe('Agent ID (UUID)'),
      walletAddress: z.string().optional().describe('Agent wallet address'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const x402 = resolveCommerceApi(commerce, 'x402');
      const { agentId, walletAddress } = params;
      let agent;
      if (agentId) {
        agent = await x402.getAgent(agentId);
      } else if (walletAddress) {
        agent = await x402.getAgentByWallet(walletAddress);
      } else {
        return { success: false, error: 'Must provide agentId or walletAddress' };
      }
      if (!agent) {
        return { success: false, error: 'Agent not found' };
      }
      return {
        success: true,
        agent: {
          id: agent.id,
          name: agent.name,
          description: agent.description,
          walletAddress: agent.wallet_address,
          publicKey: agent.public_key,
          trustLevel: agent.trust_level,
          active: agent.active,
          supportedNetworks: agent.supported_networks,
          supportedAssets: agent.supported_assets,
          paymentAddresses: parseJsonObject(agent.payment_addresses),
          skills: agent.a2a_skills,
          endpointUrl: agent.endpoint_url,
          createdAt: agent.created_at,
        },
      };
    },
  },

  {
    name: 'verify_agent',
    description: 'Verify an AI agent card (admin operation). Upgrades trust level to Verified.',
    inputSchema: {
      agentId: z.string().min(1).describe('Agent ID to verify'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { agentId } = params;
      if (!allowApply) {
        return {
          success: false,
          error: 'Verifying agent requires --apply flag.',
          wouldVerify: { agentId },
        };
      }

      const x402 = resolveCommerceApi(commerce, 'x402');
      const verified = await x402.verifyAgent(agentId);
      return {
        success: true,
        message: 'Agent verified.',
        agent: {
          id: verified.id,
          name: verified.name,
          trustLevel: verified.trust_level,
        },
      };
    },
  },

  {
    name: 'list_agent_cards',
    description: 'List all registered AI agent cards.',
    inputSchema: {
      active: z.boolean().optional().describe('Filter by active status'),
      trustLevel: z.string().optional().describe('Filter by trust level'),
      limit: z.number().optional().describe('Maximum results (default: 50)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const x402 = resolveCommerceApi(commerce, 'x402');
      const agents = await x402.listAgents({
        active: params.active,
        trustLevel: params.trustLevel,
        trust_level: params.trustLevel,
        limit: params.limit || 50,
      });
      return {
        success: true,
        count: agents.length,
        agents: agents.map((a) => ({
          id: a.id,
          name: a.name,
          walletAddress: a.wallet_address,
          trustLevel: a.trust_level,
          active: a.active,
          paymentAddresses: parseJsonObject(a.payment_addresses),
        })),
      };
    },
  },
];

export default agentCardTools;

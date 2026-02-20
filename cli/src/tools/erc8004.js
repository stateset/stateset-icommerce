/**
 * ERC-8004 Identity Registry Tools Module
 *
 * MCP tool definitions for ERC-8004 agent identity registration and lookup.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';

/**
 * ERC-8004 tool definitions
 */
export const erc8004Tools = [
  {
    name: 'erc8004_register_identity',
    description: 'Register or update an ERC-8004 agent identity record.',
    inputSchema: {
      registry: z.string().describe('Agent registry URI'),
      agentId: z.string().describe('Agent ID'),
      agentUri: z.string().describe('Agent URI'),
      agentWallet: z.string().optional().describe('Agent wallet address'),
      ownerAddress: z.string().optional().describe('Owner address'),
      agentCardId: z.string().optional().describe('Agent card ID'),
      registration: z.string().optional().describe('Registration payload'),
      registrationHash: z.string().optional().describe('Registration hash'),
      walletProofType: z.enum(['eip712', 'erc1271']).optional().describe('Wallet proof type'),
      walletProof: z.string().optional().describe('Wallet proof signature'),
      walletProofChainId: z.number().optional().describe('Wallet proof chain id'),
      walletProofDeadline: z.string().optional().describe('Wallet proof deadline (ISO)'),
      active: z.boolean().optional().describe('Active flag'),
    },
    permission: 'write',
    handler: async ({ params, allowApply, dbPath }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Registering ERC-8004 identity requires --apply flag.',
          wouldRegister: params,
        };
      }

      const { registerIdentity } = await import('../erc8004/index.js');
      const identity = registerIdentity(dbPath, {
        agentRegistry: params.registry,
        agentId: params.agentId,
        agentUri: params.agentUri,
        agentWallet: params.agentWallet || null,
        ownerAddress: params.ownerAddress || null,
        agentCardId: params.agentCardId || null,
        registration: params.registration || null,
        registrationHash: params.registrationHash || null,
        walletProofType: params.walletProofType || null,
        walletProof: params.walletProof || null,
        walletProofChainId: params.walletProofChainId || null,
        walletProofDeadline: params.walletProofDeadline || null,
        active: params.active,
      });
      return { success: true, identity };
    },
  },

  {
    name: 'erc8004_link_wallet',
    description: 'Link a wallet to an existing ERC-8004 identity record.',
    inputSchema: {
      registry: z.string().describe('Agent registry URI'),
      agentId: z.string().describe('Agent ID'),
      agentWallet: z.string().describe('Wallet address'),
      walletProofType: z.enum(['eip712', 'erc1271']).optional().describe('Wallet proof type'),
      walletProof: z.string().optional().describe('Wallet proof signature'),
      walletProofChainId: z.number().optional().describe('Wallet proof chain id'),
      walletProofDeadline: z.string().optional().describe('Wallet proof deadline (ISO)'),
    },
    permission: 'write',
    handler: async ({ params, allowApply, dbPath }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Linking ERC-8004 wallet requires --apply flag.',
          wouldLink: params,
        };
      }

      const { setAgentWallet } = await import('../erc8004/index.js');
      const identity = setAgentWallet(dbPath, {
        agentRegistry: params.registry,
        agentId: params.agentId,
        agentWallet: params.agentWallet,
        walletProofType: params.walletProofType || null,
        walletProof: params.walletProof || null,
        walletProofChainId: params.walletProofChainId || null,
        walletProofDeadline: params.walletProofDeadline || null,
      });
      return { success: true, identity };
    },
  },

  {
    name: 'erc8004_get_identity',
    description: 'Get an ERC-8004 identity by registry + agent id.',
    inputSchema: {
      registry: z.string().describe('Agent registry URI'),
      agentId: z.string().describe('Agent ID'),
    },
    permission: 'read',
    handler: async ({ params, dbPath }) => {
      const { registry, agentId } = params;
      const { getIdentity } = await import('../erc8004/index.js');
      const identity = getIdentity(dbPath, registry, agentId);
      return { success: true, identity };
    },
  },

  {
    name: 'erc8004_get_by_wallet',
    description: 'Get an ERC-8004 identity by wallet address.',
    inputSchema: {
      wallet: z.string().describe('Wallet address'),
    },
    permission: 'read',
    handler: async ({ params, dbPath }) => {
      const { wallet } = params;
      const { getIdentityByWallet } = await import('../erc8004/index.js');
      const identity = getIdentityByWallet(dbPath, wallet);
      return { success: true, identity };
    },
  },

  {
    name: 'erc8004_list_identities',
    description: 'List ERC-8004 identities.',
    inputSchema: {
      registry: z.string().optional().describe('Agent registry URI'),
      agentId: z.string().optional().describe('Agent ID'),
      wallet: z.string().optional().describe('Wallet address'),
      active: z.boolean().optional().describe('Only active identities'),
      limit: z.number().optional().default(50).describe('Max results'),
    },
    permission: 'read',
    handler: async ({ params, dbPath }) => {
      const { registry, agentId, wallet, active, limit } = params;
      const { listIdentities } = await import('../erc8004/index.js');
      const identities = listIdentities(dbPath, {
        agentRegistry: registry || null,
        agentId: agentId || null,
        agentWallet: wallet || null,
        active: active === undefined ? null : active,
        limit,
      });
      return { success: true, count: identities.length, identities };
    },
  },
];

export default erc8004Tools;

/**
 * ERC-8004 Commands Module
 */

function parseOptionalInt(value, usage) {
  if (value === undefined) return undefined;
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed)) throw new Error(usage);
  return parsed;
}

export async function execute(action, args, { output, jsonOutput, dbPath = './store.db' }) {
  const erc8004 = await import('../erc8004/index.js');

  switch (action) {
    case 'register': {
      const [
        registry,
        agentId,
        agentUri,
        agentWallet,
        ownerAddress,
        agentCardId,
        registration,
        registrationHash,
        walletProofType,
        walletProof,
        walletProofChainIdRaw,
        walletProofDeadline,
        activeRaw,
      ] = args;
      if (!registry || !agentId || !agentUri) {
        throw new Error(
          'Usage: erc8004 register <registry> <agentId> <agentUri> [agentWallet] [ownerAddress] [agentCardId] [registration] [registrationHash] [walletProofType] [walletProof] [walletProofChainId] [walletProofDeadline] [active]',
        );
      }
      const identity = erc8004.registerIdentity(dbPath, {
        agentRegistry: registry,
        agentId,
        agentUri,
        agentWallet: agentWallet || null,
        ownerAddress: ownerAddress || null,
        agentCardId: agentCardId || null,
        registration: registration || null,
        registrationHash: registrationHash || null,
        walletProofType: walletProofType || null,
        walletProof: walletProof || null,
        walletProofChainId:
          parseOptionalInt(walletProofChainIdRaw, 'walletProofChainId must be an integer') || null,
        walletProofDeadline: walletProofDeadline || null,
        active:
          activeRaw === undefined
            ? undefined
            : ['true', '1', 'yes', 'y'].includes(String(activeRaw).toLowerCase()),
      });
      return { identity, formatted: `Registered ERC-8004 identity ${agentId}` };
    }

    case 'link-wallet': {
      const [
        registry,
        agentId,
        agentWallet,
        walletProofType,
        walletProof,
        walletProofChainIdRaw,
        walletProofDeadline,
      ] = args;
      if (!registry || !agentId || !agentWallet) {
        throw new Error(
          'Usage: erc8004 link-wallet <registry> <agentId> <agentWallet> [walletProofType] [walletProof] [walletProofChainId] [walletProofDeadline]',
        );
      }
      const identity = erc8004.setAgentWallet(dbPath, {
        agentRegistry: registry,
        agentId,
        agentWallet,
        walletProofType: walletProofType || null,
        walletProof: walletProof || null,
        walletProofChainId:
          parseOptionalInt(walletProofChainIdRaw, 'walletProofChainId must be an integer') || null,
        walletProofDeadline: walletProofDeadline || null,
      });
      return { identity, formatted: `Linked wallet for ERC-8004 identity ${agentId}` };
    }

    case 'get': {
      const [registry, agentId] = args;
      if (!registry || !agentId) throw new Error('Usage: erc8004 get <registry> <agentId>');
      const identity = erc8004.getIdentity(dbPath, registry, agentId);
      return jsonOutput ? identity : { identity, formatted: `ERC-8004 identity ${agentId}` };
    }

    case 'wallet': {
      const wallet = args[0];
      if (!wallet) throw new Error('Usage: erc8004 wallet <wallet>');
      const identity = erc8004.getIdentityByWallet(dbPath, wallet);
      return jsonOutput
        ? identity
        : { identity, formatted: `ERC-8004 wallet lookup for ${wallet}` };
    }

    case 'list': {
      const [registry, agentId, wallet, activeRaw, limitRaw] = args;
      const identities = erc8004.listIdentities(dbPath, {
        agentRegistry: registry || null,
        agentId: agentId || null,
        agentWallet: wallet || null,
        active:
          activeRaw === undefined
            ? null
            : ['true', '1', 'yes', 'y'].includes(String(activeRaw).toLowerCase()),
        limit: parseOptionalInt(limitRaw, 'limit must be an integer') || 50,
      });
      return formatRows(identities, { output, jsonOutput, empty: 'No ERC-8004 identities found.' });
    }

    default:
      throw new Error(
        `Unknown action: erc8004 ${action}\n\n` +
          'Available actions:\n' +
          '  register <registry> <agentId> <agentUri> [agentWallet] [ownerAddress] [agentCardId] [registration] [registrationHash] [walletProofType] [walletProof] [walletProofChainId] [walletProofDeadline] [active]\n' +
          '  link-wallet <registry> <agentId> <agentWallet> [walletProofType] [walletProof] [walletProofChainId] [walletProofDeadline]\n' +
          '  get <registry> <agentId>                            Get identity\n' +
          '  wallet <wallet>                                     Get identity by wallet\n' +
          '  list [registry] [agentId] [wallet] [active] [limit] List identities',
      );
  }
}

function formatRows(rows, { output, jsonOutput, empty }) {
  if (jsonOutput) return rows;
  if (!rows || rows.length === 0) return { formatted: empty };
  return {
    rows,
    formatted: output.table(rows, [
      { key: 'agentRegistry', header: 'Registry' },
      { key: 'agentId', header: 'Agent' },
      { key: 'agentUri', header: 'URI' },
      { key: 'agentWallet', header: 'Wallet' },
      { key: 'active', header: 'Active' },
    ]),
  };
}

export const metadata = {
  name: 'erc8004',
  aliases: ['identity', 'registry'],
  description: 'ERC-8004 identity registry commands',
  actions: {
    register: {
      description: 'Register identity',
      args: [
        '<registry>',
        '<agentId>',
        '<agentUri>',
        '[agentWallet]',
        '[ownerAddress]',
        '[agentCardId]',
        '[registration]',
        '[registrationHash]',
        '[walletProofType]',
        '[walletProof]',
        '[walletProofChainId]',
        '[walletProofDeadline]',
        '[active]',
      ],
    },
    'link-wallet': {
      description: 'Link wallet',
      args: [
        '<registry>',
        '<agentId>',
        '<agentWallet>',
        '[walletProofType]',
        '[walletProof]',
        '[walletProofChainId]',
        '[walletProofDeadline]',
      ],
    },
    get: { description: 'Get identity', args: ['<registry>', '<agentId>'] },
    wallet: { description: 'Get identity by wallet', args: ['<wallet>'] },
    list: {
      description: 'List identities',
      args: ['[registry]', '[agentId]', '[wallet]', '[active]', '[limit]'],
    },
  },
};

export default { execute, metadata };

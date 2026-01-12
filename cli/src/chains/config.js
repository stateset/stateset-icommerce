/**
 * Blockchain Chain Configuration for StateSet iCommerce
 *
 * Supports:
 * - Solana (mainnet & devnet) - USDC stablecoin
 * - SET Chain (mainnet & testnet) - ssUSD native stablecoin
 * - Base L2 - USDC
 * - Ethereum - USDC
 */

// =============================================================================
// CHAIN CONFIGURATIONS
// =============================================================================

/**
 * @typedef {Object} ChainConfig
 * @property {string} name - Human-readable name
 * @property {string} network - Network identifier
 * @property {string} rpcUrl - RPC endpoint
 * @property {number} [chainId] - EVM chain ID (for EVM chains)
 * @property {string} explorerUrl - Block explorer URL
 * @property {Object.<string, TokenConfig>} tokens - Supported tokens
 * @property {number} confirmations - Required confirmations
 * @property {number} blockTimeMs - Average block time in milliseconds
 */

/**
 * @typedef {Object} TokenConfig
 * @property {string} symbol - Token symbol
 * @property {string} name - Token name
 * @property {string} address - Contract/mint address
 * @property {number} decimals - Token decimals
 * @property {string} type - Token type (native, spl, erc20)
 */

export const CHAINS = {
  // ===========================================================================
  // SOLANA
  // ===========================================================================
  solana: {
    name: 'Solana',
    network: 'mainnet-beta',
    rpcUrl: process.env.SOLANA_RPC_URL || 'https://api.mainnet-beta.solana.com',
    explorerUrl: 'https://explorer.solana.com',
    confirmations: 31, // Finality on Solana
    blockTimeMs: 400,
    derivationPath: "m/44'/501'/0'/0'", // BIP-44 for Solana
    tokens: {
      USDC: {
        symbol: 'USDC',
        name: 'USD Coin',
        address: 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v',
        decimals: 6,
        type: 'spl',
      },
      USDT: {
        symbol: 'USDT',
        name: 'Tether USD',
        address: 'Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB',
        decimals: 6,
        type: 'spl',
      },
      SOL: {
        symbol: 'SOL',
        name: 'Solana',
        address: 'native',
        decimals: 9,
        type: 'native',
      },
    },
  },

  solana_devnet: {
    name: 'Solana Devnet',
    network: 'devnet',
    rpcUrl: process.env.SOLANA_DEVNET_RPC_URL || 'https://api.devnet.solana.com',
    explorerUrl: 'https://explorer.solana.com',
    explorerSuffix: '?cluster=devnet',
    confirmations: 31,
    blockTimeMs: 400,
    derivationPath: "m/44'/501'/0'/0'",
    tokens: {
      USDC: {
        symbol: 'USDC',
        name: 'USD Coin (Devnet)',
        address: '4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU',
        decimals: 6,
        type: 'spl',
      },
      SOL: {
        symbol: 'SOL',
        name: 'Solana',
        address: 'native',
        decimals: 9,
        type: 'native',
      },
    },
  },

  // ===========================================================================
  // SET CHAIN (StateSet Native L2)
  // ===========================================================================
  set_chain: {
    name: 'SET Chain',
    network: 'mainnet',
    chainId: 84532001,
    rpcUrl: process.env.SET_CHAIN_RPC_URL || 'https://rpc.setchain.io',
    explorerUrl: 'https://explorer.setchain.io',
    confirmations: 1, // 2-second blocks, fast finality
    blockTimeMs: 2000,
    derivationPath: "m/44'/60'/0'/0/0", // EVM compatible
    tokens: {
      ssUSD: {
        symbol: 'ssUSD',
        name: 'StateSet USD',
        address: process.env.SSUSD_ADDRESS || '0x0000000000000000000000000000000000000000',
        decimals: 18,
        type: 'erc20',
        isYieldBearing: true,
        apyEstimate: 0.05, // ~5% APY from T-Bills
      },
      wssUSD: {
        symbol: 'wssUSD',
        name: 'Wrapped StateSet USD',
        address: process.env.WSSUSD_ADDRESS || '0x0000000000000000000000000000000000000000',
        decimals: 18,
        type: 'erc20',
        isERC4626: true,
      },
      ETH: {
        symbol: 'ETH',
        name: 'Ether',
        address: 'native',
        decimals: 18,
        type: 'native',
      },
    },
  },

  set_chain_testnet: {
    name: 'SET Chain Testnet',
    network: 'testnet',
    chainId: 84532002,
    rpcUrl: process.env.SET_CHAIN_TESTNET_RPC_URL || 'https://rpc.testnet.setchain.io',
    explorerUrl: 'https://explorer.testnet.setchain.io',
    confirmations: 1,
    blockTimeMs: 2000,
    derivationPath: "m/44'/60'/0'/0/0",
    tokens: {
      ssUSD: {
        symbol: 'ssUSD',
        name: 'StateSet USD (Testnet)',
        address: process.env.SSUSD_TESTNET_ADDRESS || '0x0000000000000000000000000000000000000000',
        decimals: 18,
        type: 'erc20',
      },
      ETH: {
        symbol: 'ETH',
        name: 'Ether',
        address: 'native',
        decimals: 18,
        type: 'native',
      },
    },
  },

  // ===========================================================================
  // BASE L2 (Coinbase)
  // ===========================================================================
  base: {
    name: 'Base',
    network: 'mainnet',
    chainId: 8453,
    rpcUrl: process.env.BASE_RPC_URL || 'https://mainnet.base.org',
    explorerUrl: 'https://basescan.org',
    confirmations: 10,
    blockTimeMs: 2000,
    derivationPath: "m/44'/60'/0'/0/0",
    tokens: {
      USDC: {
        symbol: 'USDC',
        name: 'USD Coin',
        address: '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
        decimals: 6,
        type: 'erc20',
      },
      ETH: {
        symbol: 'ETH',
        name: 'Ether',
        address: 'native',
        decimals: 18,
        type: 'native',
      },
    },
  },

  // ===========================================================================
  // ETHEREUM MAINNET
  // ===========================================================================
  ethereum: {
    name: 'Ethereum',
    network: 'mainnet',
    chainId: 1,
    rpcUrl: process.env.ETH_RPC_URL || 'https://eth.llamarpc.com',
    explorerUrl: 'https://etherscan.io',
    confirmations: 12,
    blockTimeMs: 12000,
    derivationPath: "m/44'/60'/0'/0/0",
    tokens: {
      USDC: {
        symbol: 'USDC',
        name: 'USD Coin',
        address: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
        decimals: 6,
        type: 'erc20',
      },
      USDT: {
        symbol: 'USDT',
        name: 'Tether USD',
        address: '0xdAC17F958D2ee523a2206206994597C13D831ec7',
        decimals: 6,
        type: 'erc20',
      },
      ETH: {
        symbol: 'ETH',
        name: 'Ether',
        address: 'native',
        decimals: 18,
        type: 'native',
      },
    },
  },

  // ===========================================================================
  // ARBITRUM
  // ===========================================================================
  arbitrum: {
    name: 'Arbitrum One',
    network: 'mainnet',
    chainId: 42161,
    rpcUrl: process.env.ARB_RPC_URL || 'https://arb1.arbitrum.io/rpc',
    explorerUrl: 'https://arbiscan.io',
    confirmations: 10,
    blockTimeMs: 250,
    derivationPath: "m/44'/60'/0'/0/0",
    tokens: {
      USDC: {
        symbol: 'USDC',
        name: 'USD Coin',
        address: '0xaf88d065e77c8cC2239327C5EDb3A432268e5831',
        decimals: 6,
        type: 'erc20',
      },
      ETH: {
        symbol: 'ETH',
        name: 'Ether',
        address: 'native',
        decimals: 18,
        type: 'native',
      },
    },
  },

  // ===========================================================================
  // ZCASH
  // ===========================================================================
  zcash: {
    name: 'Zcash',
    network: 'mainnet',
    rpcUrl: process.env.ZCASH_RPC_URL || 'https://mainnet.lightwalletd.com:9067',
    explorerUrl: 'https://zcashblockexplorer.com',
    confirmations: 10,
    blockTimeMs: 75000, // ~75 seconds
    derivationPath: "m/44'/133'/0'/0/0", // BIP-44 coin type 133 for Zcash
    tokens: {
      ZEC: {
        symbol: 'ZEC',
        name: 'Zcash',
        address: 'native',
        decimals: 8,
        type: 'native',
      },
    },
  },

  zcash_testnet: {
    name: 'Zcash Testnet',
    network: 'testnet',
    rpcUrl: process.env.ZCASH_TESTNET_RPC_URL || 'https://testnet.lightwalletd.com:9067',
    explorerUrl: 'https://testnet.zcashblockexplorer.com',
    confirmations: 6,
    blockTimeMs: 75000,
    derivationPath: "m/44'/1'/0'/0/0", // BIP-44 coin type 1 for testnet
    tokens: {
      ZEC: {
        symbol: 'ZEC',
        name: 'Zcash (Testnet)',
        address: 'native',
        decimals: 8,
        type: 'native',
      },
    },
  },

  // ===========================================================================
  // BITCOIN
  // ===========================================================================
  bitcoin: {
    name: 'Bitcoin',
    network: 'mainnet',
    rpcUrl: process.env.BITCOIN_RPC_URL || 'https://blockstream.info/api',
    explorerUrl: 'https://blockstream.info',
    confirmations: 6,
    blockTimeMs: 600000, // ~10 minutes
    derivationPath: "m/44'/0'/0'/0/0", // BIP-44 coin type 0 for Bitcoin
    tokens: {
      BTC: {
        symbol: 'BTC',
        name: 'Bitcoin',
        address: 'native',
        decimals: 8,
        type: 'native',
      },
    },
  },

  bitcoin_testnet: {
    name: 'Bitcoin Testnet',
    network: 'testnet',
    rpcUrl: process.env.BITCOIN_TESTNET_RPC_URL || 'https://blockstream.info/testnet/api',
    explorerUrl: 'https://blockstream.info/testnet',
    confirmations: 3,
    blockTimeMs: 600000,
    derivationPath: "m/44'/1'/0'/0/0", // BIP-44 coin type 1 for testnet
    tokens: {
      BTC: {
        symbol: 'BTC',
        name: 'Bitcoin (Testnet)',
        address: 'native',
        decimals: 8,
        type: 'native',
      },
    },
  },
};

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/**
 * Get chain configuration by ID
 * @param {string} chainId - Chain identifier (solana, set_chain, base, etc.)
 * @returns {ChainConfig|null}
 */
export function getChain(chainId) {
  return CHAINS[chainId] || null;
}

/**
 * Get token configuration
 * @param {string} chainId - Chain identifier
 * @param {string} tokenSymbol - Token symbol (USDC, ssUSD, etc.)
 * @returns {TokenConfig|null}
 */
export function getToken(chainId, tokenSymbol) {
  const chain = CHAINS[chainId];
  if (!chain) return null;
  return chain.tokens[tokenSymbol.toUpperCase()] || null;
}

/**
 * Get the default stablecoin for a chain
 * @param {string} chainId - Chain identifier
 * @returns {TokenConfig|null}
 */
export function getDefaultStablecoin(chainId) {
  const chain = CHAINS[chainId];
  if (!chain) return null;

  // Priority: ssUSD > USDC > USDT
  if (chain.tokens.ssUSD) return chain.tokens.ssUSD;
  if (chain.tokens.USDC) return chain.tokens.USDC;
  if (chain.tokens.USDT) return chain.tokens.USDT;

  return null;
}

/**
 * Get explorer URL for a transaction
 * @param {string} chainId - Chain identifier
 * @param {string} txHash - Transaction hash/signature
 * @returns {string}
 */
export function getExplorerTxUrl(chainId, txHash) {
  const chain = CHAINS[chainId];
  if (!chain) return '';

  const suffix = chain.explorerSuffix || '';

  if (chainId.startsWith('solana')) {
    return `${chain.explorerUrl}/tx/${txHash}${suffix}`;
  }

  if (isZcashChain(chainId)) {
    return `${chain.explorerUrl}/tx/${txHash}`;
  }

  if (isBitcoinChain(chainId)) {
    return `${chain.explorerUrl}/tx/${txHash}`;
  }

  // EVM chains
  return `${chain.explorerUrl}/tx/${txHash}`;
}

/**
 * Get explorer URL for an address
 * @param {string} chainId - Chain identifier
 * @param {string} address - Wallet address
 * @returns {string}
 */
export function getExplorerAddressUrl(chainId, address) {
  const chain = CHAINS[chainId];
  if (!chain) return '';

  const suffix = chain.explorerSuffix || '';

  if (chainId.startsWith('solana')) {
    return `${chain.explorerUrl}/address/${address}${suffix}`;
  }

  if (isZcashChain(chainId)) {
    return `${chain.explorerUrl}/address/${address}`;
  }

  if (isBitcoinChain(chainId)) {
    return `${chain.explorerUrl}/address/${address}`;
  }

  // EVM chains
  return `${chain.explorerUrl}/address/${address}`;
}

/**
 * Convert amount to smallest unit (e.g., dollars to cents/lamports)
 * @param {number|string} amount - Amount in human-readable units
 * @param {number} decimals - Token decimals
 * @returns {bigint}
 */
export function toSmallestUnit(amount, decimals) {
  const amountNum = typeof amount === 'string' ? parseFloat(amount) : amount;
  return BigInt(Math.floor(amountNum * Math.pow(10, decimals)));
}

/**
 * Convert amount from smallest unit to human-readable
 * @param {bigint|number|string} amount - Amount in smallest units
 * @param {number} decimals - Token decimals
 * @returns {string}
 */
export function fromSmallestUnit(amount, decimals) {
  const amountBigInt = typeof amount === 'bigint' ? amount : BigInt(amount);
  const divisor = BigInt(Math.pow(10, decimals));
  const whole = amountBigInt / divisor;
  const remainder = amountBigInt % divisor;
  const remainderStr = remainder.toString().padStart(decimals, '0');
  return `${whole}.${remainderStr}`;
}

/**
 * Format amount with token symbol
 * @param {number|string} amount - Amount
 * @param {string} tokenSymbol - Token symbol
 * @param {number} [decimals] - Decimal places to show (default 2)
 * @returns {string}
 */
export function formatAmount(amount, tokenSymbol, decimals = 2) {
  const amountNum = typeof amount === 'string' ? parseFloat(amount) : amount;
  return `${amountNum.toFixed(decimals)} ${tokenSymbol}`;
}

/**
 * Check if chain uses Ed25519 keys natively
 * @param {string} chainId - Chain identifier
 * @returns {boolean}
 */
export function isEd25519Chain(chainId) {
  return chainId.startsWith('solana') || chainId === 'near' || chainId === 'cosmos';
}

/**
 * Check if chain is EVM-compatible
 * @param {string} chainId - Chain identifier
 * @returns {boolean}
 */
export function isEvmChain(chainId) {
  const chain = CHAINS[chainId];
  return chain && chain.chainId !== undefined;
}

/**
 * Check if chain is Zcash
 * @param {string} chainId - Chain identifier
 * @returns {boolean}
 */
export function isZcashChain(chainId) {
  return chainId === 'zcash' || chainId === 'zcash_testnet';
}

/**
 * Check if chain is Bitcoin
 * @param {string} chainId - Chain identifier
 * @returns {boolean}
 */
export function isBitcoinChain(chainId) {
  return chainId === 'bitcoin' || chainId === 'bitcoin_testnet';
}

/**
 * List all supported chains
 * @returns {string[]}
 */
export function listChains() {
  return Object.keys(CHAINS);
}

/**
 * Get recommended chain for stablecoin payments
 * @param {Object} [options]
 * @param {boolean} [options.testnet] - Use testnet
 * @param {boolean} [options.preferNative] - Prefer StateSet native ssUSD
 * @returns {string}
 */
export function getRecommendedChain(options = {}) {
  if (options.preferNative) {
    return options.testnet ? 'set_chain_testnet' : 'set_chain';
  }
  // Default to Solana for USDC liquidity
  return options.testnet ? 'solana_devnet' : 'solana';
}

// =============================================================================
// EXPORT DEFAULT CONFIGURATION
// =============================================================================

export default {
  CHAINS,
  getChain,
  getToken,
  getDefaultStablecoin,
  getExplorerTxUrl,
  getExplorerAddressUrl,
  toSmallestUnit,
  fromSmallestUnit,
  formatAmount,
  isEd25519Chain,
  isEvmChain,
  isZcashChain,
  isBitcoinChain,
  listChains,
  getRecommendedChain,
};

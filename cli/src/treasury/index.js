/**
 * Treasury Engine
 *
 * Handles funding, balances, and token purchase swaps for agent wallets.
 */

import { randomUUID } from 'node:crypto';
import { Contract, JsonRpcProvider } from 'ethers';

import {
  CHAINS,
  getChain,
  getDefaultStablecoin,
  toSmallestUnit,
  fromSmallestUnit,
  isEvmChain,
} from '../chains/config.js';
import { getOrCreateWallet } from '../chains/wallet.js';
import { TreasuryStore, defaultTreasuryDbPath } from './store.js';
import {
  defaultRegistryPath,
  loadTokenRegistry,
  saveTokenRegistry,
  upsertToken,
  removeToken,
} from './registry.js';
import { normalizeSymbol, isStablecoinSymbol, resolveTokenPriceUsd } from './pricing.js';
import {
  defaultPricingPath,
  loadPricing,
  savePricing,
  upsertPricingRule,
  removePricingRule,
  getPricingRule,
} from './pricing-store.js';

const ERC20_ABI = ['function balanceOf(address owner) view returns (uint256)'];

function trimTrailingZeros(value) {
  if (!value || typeof value !== 'string') return value;
  if (!value.includes('.')) return value;
  return value.replace(/\.?0+$/, '');
}

function formatSmallest(amountSmallest, decimals) {
  return trimTrailingZeros(fromSmallestUnit(amountSmallest, decimals));
}

export async function loadTreasuryContext(options = {}) {
  const dbPath = options.dbPath || defaultTreasuryDbPath();
  const registryPath = options.registryPath || defaultRegistryPath();
  const pricingPath = options.pricingPath || defaultPricingPath();

  const store = new TreasuryStore({ dbPath });
  store.init();

  const registry = await loadTokenRegistry(registryPath);
  const pricing = await loadPricing(pricingPath);

  return { store, registry, registryPath, pricing, pricingPath, dbPath };
}

export function resolveToken(chainId, symbol, registry) {
  const normalized = normalizeSymbol(symbol);
  const chain = CHAINS[chainId];
  if (!chain) {
    throw new Error(`Unknown chain: ${chainId}`);
  }

  const chainToken = chain.tokens?.[normalized];
  if (chainToken) {
    return {
      ...chainToken,
      symbol: normalized,
      chainId,
      source: 'chain',
    };
  }

  const custom = (registry?.tokens || []).find(
    (t) => normalizeSymbol(t.symbol) === normalized && t.chainId === chainId,
  );
  if (custom) {
    const resolved = {
      ...custom,
      symbol: normalized,
      chainId,
      source: 'registry',
    };
    if (resolved.decimals === undefined || resolved.decimals === null) {
      throw new Error(
        `Token ${normalized} on ${chainId} is missing decimals. Update the registry entry.`,
      );
    }
    return resolved;
  }

  return null;
}

export function listTokens(chainId, registry) {
  const tokens = [];
  if (chainId && CHAINS[chainId]) {
    const chain = CHAINS[chainId];
    for (const token of Object.values(chain.tokens || {})) {
      tokens.push({
        ...token,
        symbol: normalizeSymbol(token.symbol),
        chainId,
        source: 'chain',
      });
    }
  }

  for (const token of registry?.tokens || []) {
    if (chainId && token.chainId !== chainId) continue;
    tokens.push({
      ...token,
      symbol: normalizeSymbol(token.symbol),
      chainId: token.chainId,
      source: 'registry',
    });
  }

  const seen = new Set();
  return tokens.filter((token) => {
    const key = `${token.chainId}:${token.symbol}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

export async function addRegistryToken(registryPath, registry, token) {
  const updated = upsertToken(registry, token);
  await saveTokenRegistry(registryPath, updated);
  return updated;
}

export async function removeRegistryToken(registryPath, registry, symbol, chainId) {
  const updated = removeToken(registry, symbol, chainId);
  await saveTokenRegistry(registryPath, updated);
  return updated;
}

export async function addPricingRule(pricingPath, pricing, rule) {
  const updated = upsertPricingRule(pricing, rule);
  await savePricing(pricingPath, updated);
  return updated;
}

export async function removePricingRuleEntry(pricingPath, pricing, tool, chainId) {
  const updated = removePricingRule(pricing, tool, chainId);
  await savePricing(pricingPath, updated);
  return updated;
}

export function getToolPricing(pricing, toolName) {
  return getPricingRule(pricing, toolName);
}

export function computeBalanceDisplay(balanceSmallest, decimals) {
  if (balanceSmallest === null || balanceSmallest === undefined) return '0';
  return formatSmallest(balanceSmallest, decimals);
}

export async function recordDeposit(options, context) {
  const { store, registry } = context;
  const {
    agentId,
    chainId,
    tokenSymbol,
    amount,
    txId = null,
    fromAddress = null,
    source = 'manual',
    metadata = {},
    taskId = null,
    sessionId = null,
    toolName = null,
    requestId = null,
  } = options;

  const token = resolveToken(chainId, tokenSymbol, registry);
  if (!token) {
    throw new Error(`Unknown token ${tokenSymbol} on ${chainId}. Add it to the registry first.`);
  }

  const amountSmallest = toSmallestUnit(amount, token.decimals);

  const entry = {
    event_id: randomUUID(),
    agent_id: agentId,
    chain_id: chainId,
    token_symbol: token.symbol,
    token_address: token.address,
    token_decimals: token.decimals,
    direction: 'deposit',
    amount_smallest: amountSmallest.toString(),
    amount_display: formatSmallest(amountSmallest, token.decimals),
    tx_id: txId,
    source,
    task_id: taskId,
    session_id: sessionId,
    tool_name: toolName,
    request_id: requestId,
    metadata: {
      ...metadata,
      fromAddress,
    },
  };

  return store.record(entry);
}

export async function recordWithdrawal(options, context) {
  const { store, registry } = context;
  const {
    agentId,
    chainId,
    tokenSymbol,
    amount,
    txId = null,
    toAddress = null,
    source = 'manual',
    metadata = {},
    taskId = null,
    sessionId = null,
    toolName = null,
    requestId = null,
  } = options;

  const token = resolveToken(chainId, tokenSymbol, registry);
  if (!token) {
    throw new Error(`Unknown token ${tokenSymbol} on ${chainId}. Add it to the registry first.`);
  }

  const amountSmallest = toSmallestUnit(amount, token.decimals);

  const entry = {
    event_id: randomUUID(),
    agent_id: agentId,
    chain_id: chainId,
    token_symbol: token.symbol,
    token_address: token.address,
    token_decimals: token.decimals,
    direction: 'withdraw',
    amount_smallest: amountSmallest.toString(),
    amount_display: formatSmallest(amountSmallest, token.decimals),
    tx_id: txId,
    source,
    task_id: taskId,
    session_id: sessionId,
    tool_name: toolName,
    request_id: requestId,
    metadata: {
      ...metadata,
      toAddress,
    },
  };

  return store.record(entry);
}

export async function buyTokens(options, context) {
  const { store, registry } = context;
  const {
    agentId,
    chainId,
    fromSymbol,
    toSymbol,
    amount,
    priceUsd = null,
    slippagePct = 1,
    metadata = {},
    taskId = null,
    sessionId = null,
    toolName = null,
    requestId = null,
  } = options;

  const fromTokenSymbol = fromSymbol || getDefaultStablecoin(chainId)?.symbol;
  if (!fromTokenSymbol) {
    throw new Error(`No default stablecoin configured for ${chainId}`);
  }

  const fromToken = resolveToken(chainId, fromTokenSymbol, registry);
  if (!fromToken) {
    throw new Error(`Unknown funding token ${fromTokenSymbol} on ${chainId}`);
  }

  if (!isStablecoinSymbol(fromToken.symbol)) {
    throw new Error(
      `Funding token ${fromToken.symbol} is not a stablecoin. Only stablecoin swaps are supported.`,
    );
  }

  const toToken = resolveToken(chainId, toSymbol, registry);
  if (!toToken) {
    throw new Error(
      `Unknown target token ${toSymbol} on ${chainId}. Add it to the registry first.`,
    );
  }

  const price = resolveTokenPriceUsd(toToken, { priceUsd });
  if (!price) {
    throw new Error(
      `Missing USD price for ${toToken.symbol}. Provide --price or set price in registry.`,
    );
  }

  const amountIn = typeof amount === 'string' ? Number(amount) : amount;
  if (!Number.isFinite(amountIn) || amountIn <= 0) {
    throw new Error('Amount must be a positive number');
  }

  const amountInSmallest = toSmallestUnit(amountIn, fromToken.decimals);
  const balance = store.getBalance({
    agentId,
    chainId,
    tokenSymbol: fromToken.symbol,
    tokenDecimals: fromToken.decimals,
  });

  if (balance.balanceSmallest < amountInSmallest) {
    throw new Error(
      `Insufficient ${fromToken.symbol} balance. Available: ${formatSmallest(balance.balanceSmallest, fromToken.decimals)}`,
    );
  }

  const slippage = Number.isFinite(slippagePct) ? slippagePct : 0;
  const tokensOut = (amountIn / price) * (1 - slippage / 100);
  if (!Number.isFinite(tokensOut) || tokensOut <= 0) {
    throw new Error('Calculated token output is invalid');
  }

  const tokensOutSmallest = toSmallestUnit(tokensOut, toToken.decimals);
  const swapId = randomUUID();

  const outEntry = {
    event_id: randomUUID(),
    related_event_id: swapId,
    agent_id: agentId,
    chain_id: chainId,
    token_symbol: fromToken.symbol,
    token_address: fromToken.address,
    token_decimals: fromToken.decimals,
    direction: 'swap_out',
    amount_smallest: amountInSmallest.toString(),
    amount_display: formatSmallest(amountInSmallest, fromToken.decimals),
    price_usd: price.toString(),
    source: 'swap',
    task_id: taskId,
    session_id: sessionId,
    tool_name: toolName,
    request_id: requestId,
    metadata: {
      ...metadata,
      toToken: toToken.symbol,
      slippagePct: slippage,
    },
  };

  const inEntry = {
    event_id: randomUUID(),
    related_event_id: swapId,
    agent_id: agentId,
    chain_id: chainId,
    token_symbol: toToken.symbol,
    token_address: toToken.address,
    token_decimals: toToken.decimals,
    direction: 'swap_in',
    amount_smallest: tokensOutSmallest.toString(),
    amount_display: formatSmallest(tokensOutSmallest, toToken.decimals),
    price_usd: price.toString(),
    source: 'swap',
    task_id: taskId,
    session_id: sessionId,
    tool_name: toolName,
    request_id: requestId,
    metadata: {
      ...metadata,
      fromToken: fromToken.symbol,
      slippagePct: slippage,
    },
  };

  store.record(outEntry);
  store.record(inEntry);

  return {
    swapId,
    from: {
      symbol: fromToken.symbol,
      amount: outEntry.amount_display,
    },
    to: {
      symbol: toToken.symbol,
      amount: inEntry.amount_display,
    },
    priceUsd: price,
    slippagePct: slippage,
  };
}

export async function recordFee(options, context) {
  const { store, registry } = context;
  const {
    agentId,
    chainId,
    tokenSymbol,
    amount,
    source = 'task',
    metadata = {},
    taskId = null,
    sessionId = null,
    toolName = null,
    requestId = null,
  } = options;

  const token = resolveToken(chainId, tokenSymbol, registry);
  if (!token) {
    throw new Error(`Unknown token ${tokenSymbol} on ${chainId}. Add it to the registry first.`);
  }

  const amountSmallest = toSmallestUnit(amount, token.decimals);

  const entry = {
    event_id: randomUUID(),
    agent_id: agentId,
    chain_id: chainId,
    token_symbol: token.symbol,
    token_address: token.address,
    token_decimals: token.decimals,
    direction: 'fee',
    amount_smallest: amountSmallest.toString(),
    amount_display: formatSmallest(amountSmallest, token.decimals),
    source,
    task_id: taskId,
    session_id: sessionId,
    tool_name: toolName,
    request_id: requestId,
    metadata: {
      ...metadata,
    },
  };

  return store.record(entry);
}

async function fetchEvmBalance(chainId, token, address) {
  const chain = getChain(chainId);
  if (!chain?.rpcUrl) {
    throw new Error(`RPC URL not configured for ${chainId}`);
  }

  const provider = new JsonRpcProvider(chain.rpcUrl);

  if (!token.address || token.address === 'native') {
    const rawBalance = await provider.getBalance(address);
    return typeof rawBalance === 'bigint' ? rawBalance : BigInt(rawBalance.toString());
  }

  const contract = new Contract(token.address, ERC20_ABI, provider);
  const rawBalance = await contract.balanceOf(address);
  return typeof rawBalance === 'bigint' ? rawBalance : BigInt(rawBalance.toString());
}

export async function syncOnChainBalance(options, context) {
  const { store, registry } = context;
  const {
    agentId,
    chainId,
    tokenSymbol,
    configDir = '.stateset',
    source = 'sync',
    apply = true,
  } = options;

  if (!isEvmChain(chainId)) {
    throw new Error(
      `On-chain sync currently supported only for EVM chains. ${chainId} is not EVM.`,
    );
  }

  const token = resolveToken(chainId, tokenSymbol, registry);
  if (!token) {
    throw new Error(`Unknown token ${tokenSymbol} on ${chainId}`);
  }

  const wallet = await getOrCreateWallet(agentId, chainId, { configDir });
  const address = wallet.address;
  const onChainBalance = await fetchEvmBalance(chainId, token, address);

  const ledgerBalance = store.getBalance({
    agentId,
    chainId,
    tokenSymbol: token.symbol,
    tokenDecimals: token.decimals,
  });

  const delta = onChainBalance - ledgerBalance.balanceSmallest;

  if (delta === 0n) {
    return {
      updated: false,
      agentId,
      chainId,
      token: token.symbol,
      onChain: formatSmallest(onChainBalance, token.decimals),
      ledger: formatSmallest(ledgerBalance.balanceSmallest, token.decimals),
      delta: '0',
    };
  }

  const direction = delta > 0n ? 'deposit' : 'withdraw';
  const amountSmallest = delta > 0n ? delta : -delta;

  const entry = {
    event_id: randomUUID(),
    agent_id: agentId,
    chain_id: chainId,
    token_symbol: token.symbol,
    token_address: token.address,
    token_decimals: token.decimals,
    direction,
    amount_smallest: amountSmallest.toString(),
    amount_display: formatSmallest(amountSmallest, token.decimals),
    source,
    metadata: {
      onChainBalance: onChainBalance.toString(),
      previousLedgerBalance: ledgerBalance.balanceSmallest.toString(),
    },
  };

  if (apply) {
    store.record(entry);
  }

  return {
    updated: true,
    applied: apply,
    direction,
    agentId,
    chainId,
    token: token.symbol,
    onChain: formatSmallest(onChainBalance, token.decimals),
    ledger: formatSmallest(ledgerBalance.balanceSmallest, token.decimals),
    delta: formatSmallest(amountSmallest, token.decimals),
    wouldRecord: apply ? null : entry,
  };
}

export async function ensureAgentWallet(agentId, chainId, configDir = '.stateset') {
  return getOrCreateWallet(agentId, chainId, { configDir });
}

export default {
  loadTreasuryContext,
  resolveToken,
  listTokens,
  addRegistryToken,
  removeRegistryToken,
  addPricingRule,
  removePricingRuleEntry,
  getToolPricing,
  computeBalanceDisplay,
  recordDeposit,
  recordWithdrawal,
  recordFee,
  buyTokens,
  syncOnChainBalance,
  ensureAgentWallet,
};

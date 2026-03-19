import fs from 'node:fs/promises';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import { getChain, getExplorerTxUrl } from './config.js';
import { toSmallestUnit } from './config.js';

const DEFAULT_OPERATION_POLL_MS = 5_000;
const DEFAULT_OPERATION_ATTEMPTS = 120;
const DEFAULT_CONFIRMATION_POLL_MS = 15_000;
const DEFAULT_CONFIRMATION_ATTEMPTS = 40;

function isTestnetChain(chainId) {
  return chainId === 'zcash_testnet';
}

function isLikelyLightwalletdEndpoint(url) {
  return /lightwalletd\.com:9067/i.test(url);
}

function getWalletRpcConfig(chainId) {
  const testnet = isTestnetChain(chainId);
  const url =
    (testnet ? process.env.ZCASH_TESTNET_WALLET_RPC_URL : process.env.ZCASH_WALLET_RPC_URL) ||
    (testnet ? process.env.ZCASH_TESTNET_RPC_URL : process.env.ZCASH_RPC_URL) ||
    '';

  if (!url || isLikelyLightwalletdEndpoint(url)) {
    return null;
  }

  return {
    url,
    username: (testnet ? process.env.ZCASH_TESTNET_RPC_USER : process.env.ZCASH_RPC_USER) || null,
    password:
      (testnet ? process.env.ZCASH_TESTNET_RPC_PASSWORD : process.env.ZCASH_RPC_PASSWORD) || null,
  };
}

function createAuthHeader(config) {
  if (!config?.username && !config?.password) {
    return null;
  }
  const credentials = `${config.username || ''}:${config.password || ''}`;
  return `Basic ${Buffer.from(credentials, 'utf8').toString('base64')}`;
}

async function rpcRequest(chainId, method, params = []) {
  const config = getWalletRpcConfig(chainId);
  if (!config) {
    throw new Error(
      `Shielded Zcash support requires a wallet-enabled JSON-RPC endpoint via ${
        isTestnetChain(chainId) ? 'ZCASH_TESTNET_WALLET_RPC_URL' : 'ZCASH_WALLET_RPC_URL'
      }. The bundled lightwalletd endpoint is read-only and cannot create shielded transactions.`,
    );
  }

  const headers = {
    'content-type': 'application/json',
  };
  const authHeader = createAuthHeader(config);
  if (authHeader) {
    headers.authorization = authHeader;
  }

  const response = await fetch(config.url, {
    method: 'POST',
    headers,
    body: JSON.stringify({
      jsonrpc: '1.0',
      id: `${method}:${Date.now()}`,
      method,
      params,
    }),
  });

  const payload = await response.json().catch(async () => {
    const body = await response.text();
    throw new Error(`Invalid Zcash RPC response for ${method}: ${body}`);
  });

  if (!response.ok) {
    throw new Error(
      `Zcash RPC ${method} failed (${response.status}): ${payload?.error?.message || response.statusText}`,
    );
  }

  if (payload?.error) {
    throw new Error(`Zcash RPC ${method} failed: ${payload.error.message || 'unknown error'}`);
  }

  return payload?.result;
}

function registryPath(configDir) {
  return path.join(configDir, 'chains', 'zcash-wallets.json');
}

async function readRegistry(configDir) {
  try {
    const raw = await fs.readFile(registryPath(configDir), 'utf8');
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === 'object' ? parsed : { version: 1, chains: {} };
  } catch (error) {
    if (error?.code === 'ENOENT') {
      return { version: 1, chains: {} };
    }
    throw error;
  }
}

async function writeRegistry(configDir, data) {
  const filePath = registryPath(configDir);
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await fs.writeFile(filePath, JSON.stringify(data, null, 2));
}

function normalizeAddressResult(result) {
  if (typeof result === 'string') {
    return result;
  }
  if (result && typeof result.address === 'string') {
    return result.address;
  }
  throw new Error(`Unexpected z_getaddressforaccount result: ${JSON.stringify(result)}`);
}

function isTransparentAddress(address) {
  return /^(?:t1|t3|tm|t2)/.test(address);
}

function isShieldedAddress(address) {
  return /^(?:u1|utest1|zs1|ztestsapling1)/.test(address);
}

function encodeMemoHex(text) {
  const input = Buffer.from(String(text), 'utf8');
  if (input.length > 512) {
    throw new Error('Zcash memo exceeds 512-byte limit');
  }
  const memo = Buffer.alloc(512);
  input.copy(memo);
  return memo.toString('hex');
}

async function createShieldedAddressForAgent(agentId, chainId) {
  const account = await rpcRequest(chainId, 'z_getnewaccount');
  let addressResult;
  let lastError = null;

  for (const receiverTypes of [['orchard', 'sapling'], ['sapling'], []]) {
    try {
      addressResult = await rpcRequest(chainId, 'z_getaddressforaccount', [account, receiverTypes]);
      break;
    } catch (error) {
      lastError = error;
    }
  }

  if (!addressResult) {
    throw lastError || new Error('Failed to derive a shielded Zcash address');
  }

  const address = normalizeAddressResult(addressResult);
  if (!isShieldedAddress(address)) {
    throw new Error(`Expected a shielded or unified Zcash address, received ${address}`);
  }

  return {
    agentId,
    account,
    address,
    createdAt: new Date().toISOString(),
  };
}

async function getStoredAgentRecord(agentId, chainId, configDir) {
  const registry = await readRegistry(configDir);
  const record = registry?.chains?.[chainId]?.[agentId] || null;
  return { registry, record };
}

async function storeAgentRecord(agentId, chainId, configDir, record) {
  const registry = await readRegistry(configDir);
  registry.version = 1;
  registry.chains ||= {};
  registry.chains[chainId] ||= {};
  registry.chains[chainId][agentId] = record;
  await writeRegistry(configDir, registry);
}

async function findRecordByAddress(address, chainId, configDir) {
  const registry = await readRegistry(configDir);
  const chainRecords = registry?.chains?.[chainId] || {};
  for (const [agentId, record] of Object.entries(chainRecords)) {
    if (record?.address === address) {
      return {
        agentId,
        record,
      };
    }
  }
  return null;
}

export function isZcashWalletRpcConfigured(chainId) {
  return Boolean(getWalletRpcConfig(chainId));
}

export async function getPreferredZcashAddress(agentId, chainId, options = {}) {
  const { configDir = '.stateset', createIfMissing = true } = options;
  const { record } = await getStoredAgentRecord(agentId, chainId, configDir);
  if (record?.address) {
    return record.address;
  }

  if (!isZcashWalletRpcConfigured(chainId)) {
    return null;
  }

  if (!createIfMissing) {
    return null;
  }

  const newRecord = await createShieldedAddressForAgent(agentId, chainId);
  await storeAgentRecord(agentId, chainId, configDir, newRecord);
  return newRecord.address;
}

function sumBalancePools(result) {
  const pools = result?.pools || {};
  let total = 0n;
  for (const pool of Object.values(pools)) {
    if (pool && pool.valueZat !== undefined && pool.valueZat !== null) {
      total += BigInt(pool.valueZat);
    }
  }
  return total;
}

export async function getZcashBalance(address, chainId, options = {}) {
  const { configDir = '.stateset', minConfirmations = 1 } = options;
  const record = await findRecordByAddress(address, chainId, configDir);

  if (record?.record?.account !== undefined) {
    const result = await rpcRequest(chainId, 'z_getbalanceforaccount', [
      record.record.account,
      minConfirmations,
    ]);
    return sumBalancePools(result);
  }

  try {
    const result = await rpcRequest(chainId, 'z_getbalance', [address, minConfirmations, true]);
    return toSmallestUnit(result, 8);
  } catch (error) {
    throw new Error(
      `Could not resolve a shielded Zcash wallet balance for ${address}: ${error.message}`,
    );
  }
}

async function waitForOperationResult(operationId, chainId, onProgress = () => {}) {
  for (let attempt = 0; attempt < DEFAULT_OPERATION_ATTEMPTS; attempt++) {
    const results = await rpcRequest(chainId, 'z_getoperationstatus', [[operationId]]);
    const result = Array.isArray(results) ? results[0] : null;

    if (!result) {
      await new Promise((resolve) => setTimeout(resolve, DEFAULT_OPERATION_POLL_MS));
      continue;
    }

    if (result.status === 'success') {
      return result.result?.txid || result.result?.txid_hex || result.result?.txidHex || null;
    }

    if (result.status === 'failed' || result.error) {
      throw new Error(result.error?.message || `Zcash operation ${operationId} failed`);
    }

    onProgress({
      step: 'zcash_operation_pending',
      message: `Waiting for shielded Zcash operation ${operationId} (${result.status})...`,
    });
    await new Promise((resolve) => setTimeout(resolve, DEFAULT_OPERATION_POLL_MS));
  }

  throw new Error(`Timed out waiting for Zcash operation ${operationId}`);
}

async function getTransactionDetails(txHash, chainId) {
  try {
    return await rpcRequest(chainId, 'gettransaction', [txHash]);
  } catch {
    return null;
  }
}

export async function getZcashTransactionStatus(txHash, chainId) {
  const details = await getTransactionDetails(txHash, chainId);
  const confirmations = Math.max(0, Number(details?.confirmations || 0));

  if (!details || confirmations <= 0) {
    return {
      confirmed: false,
      confirmations: 0,
    };
  }

  return {
    confirmed: true,
    blockNumber:
      details?.blockheight !== undefined && details?.blockheight !== null
        ? Number(details.blockheight)
        : null,
    confirmations,
    confirmedAt:
      details?.blocktime !== undefined && details?.blocktime !== null
        ? new Date(Number(details.blocktime) * 1000).toISOString()
        : new Date().toISOString(),
  };
}

async function waitForTransactionConfirmation(txHash, chainId, onProgress = () => {}) {
  const chain = getChain(chainId);
  const requiredConfirmations = chain?.executionConfirmations || 1;
  const pollInterval = chain?.confirmationPollIntervalMs || DEFAULT_CONFIRMATION_POLL_MS;
  const maxAttempts = chain?.maxConfirmationAttempts || DEFAULT_CONFIRMATION_ATTEMPTS;

  for (let attempt = 0; attempt < maxAttempts; attempt++) {
    const details = await getTransactionDetails(txHash, chainId);
    const confirmations = Math.max(0, Number(details?.confirmations || 0));

    if (confirmations >= requiredConfirmations) {
      return {
        confirmations,
        blockNumber:
          details?.blockheight !== undefined && details?.blockheight !== null
            ? Number(details.blockheight)
            : null,
      };
    }

    onProgress({
      step: 'zcash_confirmation_pending',
      message: `Waiting for Zcash confirmations (${confirmations}/${requiredConfirmations})...`,
      confirmations,
      required: requiredConfirmations,
    });
    await new Promise((resolve) => setTimeout(resolve, pollInterval));
  }

  throw new Error(`Timed out waiting for Zcash confirmation for ${txHash}`);
}

export async function executeZcashShieldedPayment(params, options = {}) {
  const { agentId, chainId, toAddress, amount, tokenSymbol, metadata = {} } = params;
  const { configDir = '.stateset', simulate = false, onProgress = () => {} } = options;

  const symbol = String(tokenSymbol || 'ZEC').toUpperCase();
  if (symbol !== 'ZEC') {
    throw new Error(`Shielded Zcash payments only support ZEC, received ${symbol}`);
  }

  if (isTransparentAddress(toAddress)) {
    throw new Error(
      `Shielded Zcash payments require a unified or shielded recipient address. Transparent recipients are not supported: ${toAddress}`,
    );
  }

  const fromAddress = await getPreferredZcashAddress(agentId, chainId, {
    configDir,
    createIfMissing: true,
  });

  if (!fromAddress) {
    throw new Error('Could not resolve a shielded Zcash address for this agent');
  }

  const intentId = `zcash:${randomUUID()}`;
  const amountNumber = typeof amount === 'number' ? amount : Number(amount);
  const memoHex = metadata.memo ? encodeMemoHex(metadata.memo) : null;
  const recipients = [
    {
      address: toAddress,
      amount: amountNumber,
      ...(memoHex ? { memo: memoHex } : {}),
    },
  ];

  if (simulate) {
    return {
      success: true,
      simulated: true,
      intentId,
      intent: {
        intentId,
        chainId,
        tokenSymbol: 'ZEC',
        fromAddress,
        toAddress,
        amount: amountNumber.toString(),
        createdAt: new Date().toISOString(),
        status: 'preview',
        metadata,
      },
      txPreview: {
        type: 'zcash_shielded_rpc_payment',
        fromAddress,
        recipients,
        rpcManaged: true,
      },
      vesEventIds: [],
    };
  }

  onProgress({
    step: 'zcash_operation_create',
    message: 'Submitting shielded Zcash payment operation...',
  });

  const operationId = await rpcRequest(chainId, 'z_sendmany', [
    fromAddress,
    recipients,
    undefined,
    undefined,
    undefined,
    'FullPrivacy',
  ]);
  const txHash = await waitForOperationResult(operationId, chainId, onProgress);

  if (!txHash) {
    throw new Error(`Zcash operation ${operationId} completed without a transaction id`);
  }

  const confirmation = await waitForTransactionConfirmation(txHash, chainId, onProgress);

  return {
    success: true,
    intentId,
    txHash,
    txSignature: txHash,
    explorerUrl: getExplorerTxUrl(chainId, txHash),
    blockNumber: confirmation.blockNumber,
    confirmations: confirmation.confirmations,
    intent: {
      intentId,
      chainId,
      tokenSymbol: 'ZEC',
      fromAddress,
      toAddress,
      amount: amountNumber.toString(),
      createdAt: new Date().toISOString(),
      status: 'confirmed',
      metadata: {
        ...metadata,
        operationId,
      },
    },
    vesEventIds: [],
  };
}

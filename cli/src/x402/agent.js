/**
 * x402 Agent Helpers
 */

import crypto from 'crypto';
import { createBudgetState, getDefaultBudgetStateFile } from './budget.js';
import {
  computeX402SigningHash,
  encodeBase64Json,
  decodeBase64Json,
  hashToHex,
  hexToBytes,
  normalizeAsset,
  normalizeNetwork,
  networkChainId,
  signX402Hash,
  verifyX402Signature,
} from './crypto.js';

function randomNonce() {
  const buf = crypto.randomBytes(8);
  const value = BigInt('0x' + buf.toString('hex'));
  const maxSafe = BigInt(Number.MAX_SAFE_INTEGER);
  return Number(value % maxSafe);
}

function getPaymentRequiredHeader(response) {
  const paymentRequired = response.headers.get('payment-required');
  if (paymentRequired) {
    return { value: paymentRequired, version: 'v2' };
  }
  const legacyRequired = response.headers.get('x-payment-required');
  if (legacyRequired) {
    return { value: legacyRequired, version: 'v1' };
  }
  return { value: null, version: null };
}

function pickRequirement(candidates, preferredNetworks = []) {
  if (!Array.isArray(candidates) || candidates.length === 0) return null;
  if (preferredNetworks.length > 0) {
    const preferred = candidates.find(candidate => {
      const networks = candidate?.networks || (candidate?.network ? [candidate.network] : []);
      return networks.some(n => preferredNetworks.map(String).includes(String(n)));
    });
    if (preferred) return preferred;
  }
  return candidates[0];
}

function normalizeRequirements(payload, preferredNetworks = []) {
  if (!payload) return null;
  if (payload.requirements) return payload.requirements;
  if (Array.isArray(payload.accepts)) {
    const selected = pickRequirement(payload.accepts, preferredNetworks);
    if (!selected) return null;
    return {
      ...selected,
      resource_uri: selected.resource || payload.resource || payload.resource_uri,
      description: selected.description ?? payload.description,
      metadata: selected.metadata ?? payload.metadata,
      merchant_id: selected.merchant_id ?? payload.merchant_id,
      idempotency_key: selected.idempotency_key ?? payload.idempotency_key,
    };
  }
  if (Array.isArray(payload.paymentRequirements)) {
    return pickRequirement(payload.paymentRequirements, preferredNetworks);
  }
  return payload;
}

async function parsePaymentRequired(response, preferredNetworks = []) {
  const header = getPaymentRequiredHeader(response);
  if (header.value) {
    const payload = decodeBase64Json(header.value);
    return {
      requirements: normalizeRequirements(payload, preferredNetworks),
      version: header.version,
      raw: payload,
    };
  }
  try {
    const body = await response.clone().json();
    return {
      requirements: normalizeRequirements(body, preferredNetworks),
      version: null,
      raw: body,
    };
  } catch (_) {
    return { requirements: null, version: null, raw: null };
  }
}

export class BudgetExceededError extends Error {
  constructor(message) {
    super(message);
    this.name = 'BudgetExceededError';
  }
}

function selectNetwork(requirements, preferred = []) {
  const networks = requirements?.networks || (requirements?.network ? [requirements.network] : []);
  if (!networks || networks.length === 0) {
    throw new Error('No networks provided in payment requirements');
  }
  if (preferred.length > 0) {
    const match = networks.find(n => preferred.map(String).includes(String(n)));
    if (match) return match;
  }
  return networks[0];
}

function resolveAmount(requirements) {
  const amount = requirements?.amount ??
    requirements?.amount_required ??
    requirements?.max_amount_required ??
    requirements?.amountRequired ??
    requirements?.maxAmountRequired;
  if (!amount) {
    throw new Error('Payment requirements missing amount');
  }
  return Number(amount);
}

export async function x402Fetch(url, options, config) {
  const {
    sequencerClient,
    tenantId,
    storeId,
    agentId,
    agentKeyId,
    payerAddress,
    signingKey,
    preferredNetworks = [],
    requireReceipt = false,
    autoBatch = true,
    receiptTimeoutMs = 300_000,
    receiptPollMs = 2_000,
    maxAmount,
    maxAmountPerCall,
    dailyBudget,
    budgetState,
    budgetStateFile,
    startingBalance,
  } = config;

  if (!sequencerClient) throw new Error('sequencerClient is required');
  if (!tenantId || !storeId || !agentId) throw new Error('tenantId, storeId, and agentId are required');
  if (!payerAddress) throw new Error('payerAddress is required');
  if (!signingKey?.privateKey || !signingKey?.publicKey) throw new Error('signingKey with privateKey/publicKey is required');

  const baseHeaders = options?.headers ? { ...options.headers } : {};

  const response = await fetch(url, {
    ...options,
    headers: baseHeaders,
  });

  if (response.status !== 402) {
    return response;
  }

  const parsed = await parsePaymentRequired(response, preferredNetworks);
  const requirements = parsed?.requirements;
  if (!requirements) {
    throw new Error('Failed to parse x402 payment requirements');
  }
  const useV2Headers = parsed?.version === 'v2'
    || requirements?.x402Version === 2
    || requirements?.version === 2;

  const network = normalizeNetwork(selectNetwork(requirements, preferredNetworks));
  const asset = normalizeAsset(requirements.asset || requirements.token || 'usdc');
  const amount = resolveAmount(requirements);
  const shouldTrackBudget = Boolean(
    budgetState ||
    budgetStateFile ||
    maxAmountPerCall !== undefined ||
    dailyBudget !== undefined ||
    startingBalance !== undefined
  );
  const resolvedBudgetState = budgetState || (shouldTrackBudget
    ? createBudgetState({
      filePath: budgetStateFile || getDefaultBudgetStateFile(),
      startingBalance,
    })
    : null);
  if (maxAmount !== undefined && amount > maxAmount) {
    throw new Error(`Required amount ${amount} exceeds maxAmount ${maxAmount}`);
  }
  if (maxAmountPerCall !== undefined && amount > maxAmountPerCall) {
    throw new BudgetExceededError(`Amount ${amount} exceeds per-call limit ${maxAmountPerCall}`);
  }
  if (dailyBudget !== undefined && resolvedBudgetState) {
    const spentToday = resolvedBudgetState.getSpentToday();
    if (spentToday + amount > dailyBudget) {
      throw new BudgetExceededError(
        `Would exceed daily budget. Spent: ${spentToday}, limit: ${dailyBudget}`
      );
    }
  }
  if (resolvedBudgetState) {
    const balance = resolvedBudgetState.getBalance();
    if (balance !== null && amount > balance) {
      throw new BudgetExceededError(
        `Insufficient x402 balance. Required: ${amount}, available: ${balance}`
      );
    }
  }

  const now = Math.floor(Date.now() / 1000);
  const validitySeconds = Number(requirements.validity_seconds ?? requirements.validitySeconds ?? 3600);
  const validUntil = now + validitySeconds;
  const nonce = randomNonce();
  const chainId = networkChainId(network);
  const payeeAddress = requirements.payee_address
    || requirements.payment_address
    || requirements.recipient
    || requirements.payeeAddress
    || requirements.paymentAddress;
  if (!payeeAddress) {
    throw new Error('Payment requirements missing payee address');
  }

  const signingHash = computeX402SigningHash({
    payerAddress,
    payeeAddress,
    amount,
    asset,
    network,
    chainId,
    validUntil,
    nonce,
  });

  const signature = signX402Hash(signingHash, signingKey.privateKey);

  const submitPayload = {
    tenant_id: tenantId,
    store_id: storeId,
    agent_id: agentId,
    agent_key_id: agentKeyId ?? signingKey.keyId ?? 1,
    payer_address: payerAddress,
    payee_address: payeeAddress,
    amount,
    asset,
    network,
    valid_until: validUntil,
    nonce,
    signing_hash: hashToHex(signingHash),
    payer_signature: hashToHex(signature),
    payer_public_key: hashToHex(signingKey.publicKey),
    resource_uri: requirements.resource_uri || requirements.resourceUri || requirements.resource || url,
    description: requirements.description,
    merchant_id: requirements.merchant_id,
    idempotency_key: requirements.idempotency_key || requirements.idempotencyKey || `x402-${crypto.randomUUID()}`,
    metadata: requirements.metadata || null,
  };

  const submitResponse = await sequencerClient.submitPaymentIntent(submitPayload);
  let receipt = null;
  if (requireReceipt && autoBatch) {
    try {
      await sequencerClient.createBatch({
        tenant_id: tenantId,
        store_id: storeId,
        network,
      });
    } catch (_) {
      // batching is best-effort; receipts may still arrive via worker
    }
  }

  if (requireReceipt) {
    receipt = await sequencerClient.waitForReceipt(submitResponse.intent_id, {
      timeoutMs: receiptTimeoutMs,
      intervalMs: receiptPollMs,
    });
  }

  const paymentHeader = useV2Headers ? 'PAYMENT-SIGNATURE' : 'X-Payment';
  const receiptHeader = useV2Headers ? 'PAYMENT-RESPONSE' : 'X-Payment-Receipt';
  const retryHeaders = {
    ...baseHeaders,
    ...(receipt
      ? { [receiptHeader]: encodeBase64Json(receipt) }
      : { [paymentHeader]: encodeBase64Json(submitPayload) }),
  };

  const finalResponse = await fetch(url, {
    ...options,
    headers: retryHeaders,
  });

  if (resolvedBudgetState && finalResponse.ok) {
    resolvedBudgetState.recordSpend(amount, { url, intentId: submitResponse.intent_id });
  }

  return finalResponse;
}

export function createX402Agent(config) {
  const shouldTrackBudget = Boolean(
    config?.budgetState ||
    config?.budgetStateFile ||
    config?.maxAmountPerCall !== undefined ||
    config?.dailyBudget !== undefined ||
    config?.startingBalance !== undefined
  );
  const resolvedBudgetState = config?.budgetState || (shouldTrackBudget
    ? createBudgetState({
      filePath: config?.budgetStateFile || getDefaultBudgetStateFile(),
      startingBalance: config?.startingBalance,
    })
    : null);

  return {
    fetch: (url, options = {}) => x402Fetch(url, options, {
      ...config,
      budgetState: resolvedBudgetState,
    }),
    budget: resolvedBudgetState,
  };
}

export function decodePaymentHeader(value) {
  return decodeBase64Json(value);
}

export function decodeReceiptHeader(value) {
  return decodeBase64Json(value);
}

export function verifyPaymentHeader(payload) {
  const now = Math.floor(Date.now() / 1000);
  if (payload.valid_until && Number(payload.valid_until) < now) {
    return { ok: false, reason: 'Payment intent expired' };
  }

  const expectedChainId = networkChainId(payload.network);
  if (payload.chain_id && Number(payload.chain_id) !== expectedChainId) {
    return { ok: false, reason: 'Chain id mismatch' };
  }

  const signingHash = computeX402SigningHash({
    payerAddress: payload.payer_address,
    payeeAddress: payload.payee_address,
    amount: payload.amount,
    asset: payload.asset,
    network: payload.network,
    chainId: payload.chain_id ?? expectedChainId,
    validUntil: payload.valid_until,
    nonce: payload.nonce,
  });

  const providedHash = hexToBytes(payload.signing_hash);
  if (Buffer.compare(providedHash, signingHash) != 0) {
    return { ok: false, reason: 'Signing hash mismatch' };
  }

  if (!payload.payer_public_key) {
    return { ok: false, reason: 'Missing payer_public_key' };
  }

  const signatureOk = verifyX402Signature(
    signingHash,
    hexToBytes(payload.payer_signature),
    hexToBytes(payload.payer_public_key),
  );

  if (!signatureOk) {
    return { ok: false, reason: 'Signature verification failed' };
  }

  return { ok: true, signingHash };
}

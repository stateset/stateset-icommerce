/**
 * x402 Agent Helpers
 */

import crypto from 'crypto';
import { fetchWithValidatedRedirects } from '../utils/url-validator.js';
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
import { createExactEvmPaymentPayload, isExactEvmRequirement } from './exact-evm.js';

/**
 * @typedef {{
 *   networks?: string[],
 *   network?: string,
 *   amount?: number | string,
 *   amount_required?: number | string,
 *   max_amount_required?: number | string,
 *   amountRequired?: number | string,
 *   maxAmountRequired?: number | string,
 *   scheme?: string,
 *   asset?: string,
 *   token?: string,
 *   x402Version?: number,
 *   version?: number,
 *   payTo?: string,
 *   maxTimeoutSeconds?: number | string,
 *   validity_seconds?: number | string,
 *   validitySeconds?: number | string,
 *   payee_address?: string,
 *   payment_address?: string,
 *   recipient?: string,
 *   payeeAddress?: string,
 *   paymentAddress?: string,
 *   resource_uri?: string,
 *   resourceUri?: string,
 *   resource?: string,
 *   description?: string,
 *   merchant_id?: string,
 *   idempotency_key?: string,
 *   idempotencyKey?: string,
 *   extra?: Record<string, unknown> | null,
 *   metadata?: Record<string, unknown> | null,
 * }} PaymentRequirement
 * @typedef {{
 *   requirements?: PaymentRequirement,
 *   accepts?: PaymentRequirement[],
 *   paymentRequirements?: PaymentRequirement[],
 *   resource?: string,
 *   resource_uri?: string,
 *   description?: string,
 *   metadata?: Record<string, unknown> | null,
 *   merchant_id?: string,
 *   idempotency_key?: string,
 * }} PaymentRequiredPayload
 * @typedef {{ keyId?: number, privateKey: Buffer, publicKey: Buffer }} SigningKey
 * @typedef {{
 *   filePath: string,
 *   getSpentToday: () => number,
 *   getBalance: () => number | null,
 *   recordSpend: (amount: number, metadata?: Record<string, unknown>) => void,
 *   listHistory: (limit?: number) => unknown[],
 * }} BudgetState
 * @typedef {{
 *   submitPaymentIntent: (payload: unknown) => Promise<{ intent_id: string }>,
 *   createBatch: (payload: unknown) => Promise<unknown>,
 *   waitForReceipt: (intentId: string, options?: { timeoutMs?: number, intervalMs?: number }) => Promise<unknown>,
 * }} X402SequencerClientLike
 * @typedef {{
 *   sequencerClient?: X402SequencerClientLike | null,
 *   tenantId?: string,
 *   storeId?: string,
 *   agentId: string,
 *   agentKeyId?: number,
 *   payerAddress: string,
 *   signingKey: SigningKey,
 *   preferredNetworks?: string[],
 *   requireReceipt?: boolean,
 *   autoBatch?: boolean,
 *   receiptTimeoutMs?: number,
 *   receiptPollMs?: number,
 *   maxAmount?: number,
 *   maxAmountPerCall?: number,
 *   dailyBudget?: number,
 *   budgetState?: BudgetState | null,
 *   budgetStateFile?: string,
 *   startingBalance?: number,
 *   validateUrl?: boolean,
 *   urlLookup?: (hostname: string, options: { all: boolean, verbatim: boolean }) => Promise<Array<string | { address?: string, family?: number }> | string | { address?: string, family?: number }>,
 * }} X402FetchConfig
 * @typedef {{ requirements: PaymentRequirement | null, version: 'v1' | 'v2' | null, raw: unknown }} ParsedPaymentRequired
 */

/**
 * @param {unknown} error
 * @returns {string}
 */
function messageFromError(error) {
  return error instanceof Error ? error.message : String(error);
}

/**
 * @param {HeadersInit | undefined} headers
 * @returns {Record<string, string>}
 */
function headersToObject(headers) {
  if (!headers) return {};
  if (headers instanceof Headers) {
    return Object.fromEntries(headers.entries());
  }
  if (Array.isArray(headers)) {
    return Object.fromEntries(headers.map(([key, value]) => [key, String(value)]));
  }
  return Object.fromEntries(Object.entries(headers).map(([key, value]) => [key, String(value)]));
}

/**
 * @returns {number}
 */
function randomNonce() {
  const buf = crypto.randomBytes(8);
  const value = BigInt('0x' + buf.toString('hex'));
  const maxSafe = BigInt(Number.MAX_SAFE_INTEGER);
  return Number(value % maxSafe);
}

/**
 * @param {Response} response
 * @returns {{ value: string | null, version: 'v1' | 'v2' | null }}
 */
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

/**
 * @param {PaymentRequirement[] | null | undefined} candidates
 * @param {string[]} [preferredNetworks]
 * @returns {PaymentRequirement | null}
 */
function pickRequirement(candidates, preferredNetworks = []) {
  if (!Array.isArray(candidates) || candidates.length === 0) return null;
  if (preferredNetworks.length > 0) {
    const preferred = candidates.find((candidate) => {
      const networks = candidate?.networks || (candidate?.network ? [candidate.network] : []);
      return networks.some((n) => preferredNetworks.map(String).includes(String(n)));
    });
    if (preferred) return preferred;
  }
  return candidates[0];
}

/**
 * @param {PaymentRequiredPayload | PaymentRequirement | null | undefined} payload
 * @param {string[]} [preferredNetworks]
 * @returns {PaymentRequirement | null}
 */
function normalizeRequirements(payload, preferredNetworks = []) {
  if (!payload) return null;
  const candidatePayload = /** @type {PaymentRequiredPayload} */ (payload);
  if (candidatePayload.requirements) return candidatePayload.requirements;
  if (Array.isArray(candidatePayload.accepts)) {
    const selected = pickRequirement(candidatePayload.accepts, preferredNetworks);
    if (!selected) return null;
    return {
      ...selected,
      resource_uri: selected.resource || candidatePayload.resource || candidatePayload.resource_uri,
      description: selected.description ?? candidatePayload.description,
      metadata: selected.metadata ?? candidatePayload.metadata,
      merchant_id: selected.merchant_id ?? candidatePayload.merchant_id,
      idempotency_key: selected.idempotency_key ?? candidatePayload.idempotency_key,
    };
  }
  if (Array.isArray(candidatePayload.paymentRequirements)) {
    return pickRequirement(candidatePayload.paymentRequirements, preferredNetworks);
  }
  return /** @type {PaymentRequirement} */ (payload);
}

/**
 * @param {Response} response
 * @param {string[]} [preferredNetworks]
 * @returns {Promise<ParsedPaymentRequired>}
 */
async function parsePaymentRequired(response, preferredNetworks = []) {
  const header = getPaymentRequiredHeader(response);
  if (header.value) {
    const payload = /** @type {PaymentRequiredPayload} */ (decodeBase64Json(header.value));
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
  } catch (err) {
    console.debug('[x402-agent] Payment requirements parse failed:', messageFromError(err));
    return { requirements: null, version: null, raw: null };
  }
}

export class BudgetExceededError extends Error {
  /**
   * @param {string} message
   */
  constructor(message) {
    super(message);
    this.name = 'BudgetExceededError';
  }
}

/**
 * @param {PaymentRequirement | null | undefined} requirements
 * @param {string[]} [preferred]
 * @returns {string}
 */
function selectNetwork(requirements, preferred = []) {
  const networks = requirements?.networks || (requirements?.network ? [requirements.network] : []);
  if (!networks || networks.length === 0) {
    throw new Error('No networks provided in payment requirements');
  }
  if (preferred.length > 0) {
    const match = networks.find((n) => preferred.map(String).includes(String(n)));
    if (match) return match;
  }
  return networks[0];
}

/**
 * @param {PaymentRequirement | null | undefined} requirements
 * @returns {number}
 */
function resolveAmount(requirements) {
  const amount =
    requirements?.amount ??
    requirements?.amount_required ??
    requirements?.max_amount_required ??
    requirements?.amountRequired ??
    requirements?.maxAmountRequired;
  if (!amount) {
    throw new Error('Payment requirements missing amount');
  }
  return Number(amount);
}

/**
 * @param {PaymentRequirement | null | undefined} requirements
 * @returns {string | null}
 */
function resolvePayeeAddress(requirements) {
  return (
    requirements?.payTo ||
    requirements?.payee_address ||
    requirements?.payment_address ||
    requirements?.recipient ||
    requirements?.payeeAddress ||
    requirements?.paymentAddress ||
    null
  );
}

/**
 * @param {string} url
 * @param {RequestInit | undefined} options
 * @param {X402FetchConfig} config
 * @returns {Promise<Response>}
 */
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
    validateUrl = true,
    urlLookup,
  } = config;

  if (!agentId) throw new Error('agentId is required');
  if (!payerAddress) throw new Error('payerAddress is required');
  if (!signingKey?.privateKey || !signingKey?.publicKey)
    throw new Error('signingKey with privateKey/publicKey is required');

  const baseHeaders = headersToObject(options?.headers);
  /**
   * @param {string} requestUrl
   * @param {RequestInit} requestOptions
   * @returns {Promise<Response>}
   */
  const request = (requestUrl, requestOptions) =>
    validateUrl === false
      ? fetch(requestUrl, requestOptions)
      : fetchWithValidatedRedirects(requestUrl, requestOptions, { lookup: urlLookup });

  const response = await request(url, {
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

  if (isExactEvmRequirement(requirements)) {
    const amount = Number(requirements.amount);
    const shouldTrackBudget = Boolean(
      budgetState ||
      budgetStateFile ||
      maxAmountPerCall !== undefined ||
      dailyBudget !== undefined ||
      startingBalance !== undefined,
    );
    const resolvedBudgetState =
      budgetState ||
      (shouldTrackBudget
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
          `Would exceed daily budget. Spent: ${spentToday}, limit: ${dailyBudget}`,
        );
      }
    }
    if (resolvedBudgetState) {
      const balance = resolvedBudgetState.getBalance();
      if (balance !== null && amount > balance) {
        throw new BudgetExceededError(
          `Insufficient x402 balance. Required: ${amount}, available: ${balance}`,
        );
      }
    }

    const paymentPayload = await createExactEvmPaymentPayload({
      requirement: requirements,
      paymentRequired: /** @type {any} */ (parsed?.raw),
      signingKey,
      payerAddress,
      resourceUrl: url,
      method: options?.method || 'GET',
    });
    const retryHeaders = {
      ...baseHeaders,
      'PAYMENT-SIGNATURE': encodeBase64Json(paymentPayload),
    };
    const finalResponse = await request(url, {
      ...options,
      headers: retryHeaders,
    });

    if (resolvedBudgetState && finalResponse.ok) {
      resolvedBudgetState.recordSpend(amount, {
        url,
        scheme: 'exact',
        network: requirements.network,
      });
    }

    return finalResponse;
  }

  if (!sequencerClient) {
    throw new Error('sequencerClient is required for legacy sequencer-backed x402 payments');
  }
  if (!tenantId || !storeId) {
    throw new Error('tenantId and storeId are required for legacy sequencer-backed x402 payments');
  }
  const useV2Headers =
    parsed?.version === 'v2' || requirements?.x402Version === 2 || requirements?.version === 2;

  const network = normalizeNetwork(selectNetwork(requirements, preferredNetworks));
  const asset = normalizeAsset(requirements.asset || requirements.token || 'usdc');
  const amount = resolveAmount(requirements);
  const shouldTrackBudget = Boolean(
    budgetState ||
    budgetStateFile ||
    maxAmountPerCall !== undefined ||
    dailyBudget !== undefined ||
    startingBalance !== undefined,
  );
  const resolvedBudgetState =
    budgetState ||
    (shouldTrackBudget
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
        `Would exceed daily budget. Spent: ${spentToday}, limit: ${dailyBudget}`,
      );
    }
  }
  if (resolvedBudgetState) {
    const balance = resolvedBudgetState.getBalance();
    if (balance !== null && amount > balance) {
      throw new BudgetExceededError(
        `Insufficient x402 balance. Required: ${amount}, available: ${balance}`,
      );
    }
  }

  const now = Math.floor(Date.now() / 1000);
  const validitySeconds = Number(
    requirements.validity_seconds ?? requirements.validitySeconds ?? 3600,
  );
  const validUntil = now + validitySeconds;
  const nonce = randomNonce();
  const chainId = networkChainId(network);
  const payeeAddress = resolvePayeeAddress(requirements);
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
    resourceUri:
      requirements.resource_uri || requirements.resourceUri || requirements.resource || url,
    resourceMethod: options?.method || 'GET',
  });

  const signature = signX402Hash(signingHash, signingKey.privateKey);

  /** @type {Record<string, unknown>} */
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
    resource_uri:
      requirements.resource_uri || requirements.resourceUri || requirements.resource || url,
    resource_method: options?.method || 'GET',
    description: requirements.description,
    merchant_id: requirements.merchant_id,
    idempotency_key:
      requirements.idempotency_key || requirements.idempotencyKey || `x402-${crypto.randomUUID()}`,
    metadata: requirements.metadata || null,
  };

  /** @type {{ intent_id: string }} */
  const submitResponse = await sequencerClient.submitPaymentIntent(submitPayload);
  /** @type {unknown | null} */
  let receipt = null;
  if (requireReceipt && autoBatch) {
    try {
      await sequencerClient.createBatch({
        tenant_id: tenantId,
        store_id: storeId,
        network,
      });
    } catch (err) {
      console.debug('[x402-agent] Batch creation failed (best-effort):', messageFromError(err));
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

  const finalResponse = await request(url, {
    ...options,
    headers: retryHeaders,
  });

  if (resolvedBudgetState && finalResponse.ok) {
    resolvedBudgetState.recordSpend(amount, { url, intentId: submitResponse.intent_id });
  }

  return finalResponse;
}

/**
 * @param {X402FetchConfig} config
 * @returns {{ fetch: (url: string, options?: RequestInit) => Promise<Response>, budget: BudgetState | null }}
 */
export function createX402Agent(config) {
  const shouldTrackBudget = Boolean(
    config?.budgetState ||
    config?.budgetStateFile ||
    config?.maxAmountPerCall !== undefined ||
    config?.dailyBudget !== undefined ||
    config?.startingBalance !== undefined,
  );
  const resolvedBudgetState =
    config?.budgetState ||
    (shouldTrackBudget
      ? createBudgetState({
          filePath: config?.budgetStateFile || getDefaultBudgetStateFile(),
          startingBalance: config?.startingBalance,
        })
      : null);

  return {
    fetch: (url, options = {}) =>
      x402Fetch(url, options, {
        ...config,
        budgetState: resolvedBudgetState,
      }),
    budget: resolvedBudgetState,
  };
}

/**
 * @param {string} value
 * @returns {unknown}
 */
export function decodePaymentHeader(value) {
  return decodeBase64Json(value);
}

/**
 * @param {string} value
 * @returns {unknown}
 */
export function decodeReceiptHeader(value) {
  return decodeBase64Json(value);
}

/**
 * @param {Record<string, unknown>} payload
 * @returns {{ ok: false, reason: string } | { ok: true, signingHash: Buffer }}
 */
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
    resourceUri: payload.resource_uri,
    resourceMethod: payload.resource_method,
  });

  const providedHash = hexToBytes(payload.signing_hash);
  if (Buffer.compare(providedHash, signingHash) !== 0) {
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

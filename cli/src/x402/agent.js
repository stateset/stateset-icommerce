/**
 * x402 Agent Helpers
 */

import crypto from 'crypto';
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

async function parsePaymentRequired(response) {
  const header = response.headers.get('x-payment-required') || response.headers.get('X-Payment-Required');
  if (header) {
    return decodeBase64Json(header);
  }
  try {
    const body = await response.clone().json();
    if (body?.requirements) return body.requirements;
    return body;
  } catch (_) {
    return null;
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
  const amount = requirements?.amount ?? requirements?.max_amount_required;
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

  const requirements = await parsePaymentRequired(response);
  if (!requirements) {
    throw new Error('Failed to parse x402 payment requirements');
  }

  const network = normalizeNetwork(selectNetwork(requirements, preferredNetworks));
  const asset = normalizeAsset(requirements.asset || requirements.token || 'usdc');
  const amount = resolveAmount(requirements);
  if (maxAmount !== undefined && amount > maxAmount) {
    throw new Error(`Required amount ${amount} exceeds maxAmount ${maxAmount}`);
  }

  const now = Math.floor(Date.now() / 1000);
  const validitySeconds = Number(requirements.validity_seconds || 3600);
  const validUntil = now + validitySeconds;
  const nonce = randomNonce();
  const chainId = networkChainId(network);

  const signingHash = computeX402SigningHash({
    payerAddress,
    payeeAddress: requirements.payee_address || requirements.payment_address || requirements.recipient,
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
    payee_address: requirements.payee_address || requirements.payment_address || requirements.recipient,
    amount,
    asset,
    network,
    valid_until: validUntil,
    nonce,
    signing_hash: hashToHex(signingHash),
    payer_signature: hashToHex(signature),
    payer_public_key: hashToHex(signingKey.publicKey),
    resource_uri: requirements.resource_uri || requirements.resource || url,
    description: requirements.description,
    merchant_id: requirements.merchant_id,
    idempotency_key: requirements.idempotency_key || `x402-${crypto.randomUUID()}`,
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

  const retryHeaders = {
    ...baseHeaders,
    ...(receipt
      ? { 'X-Payment-Receipt': encodeBase64Json(receipt) }
      : { 'X-Payment': encodeBase64Json(submitPayload) }),
  };

  return fetch(url, {
    ...options,
    headers: retryHeaders,
  });
}

export function createX402Agent(config) {
  return {
    fetch: (url, options = {}) => x402Fetch(url, options, config),
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

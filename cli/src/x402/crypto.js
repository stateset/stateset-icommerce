/**
 * x402 Cryptographic Utilities (Sequencer-compatible)
 *
 * Implements the signing hash and Ed25519 signature flow expected by
 * stateset-sequencer's /api/v1/x402 endpoints.
 */

import crypto from 'crypto';
import {
  u64BE,
  bufferToHex,
  hexToBuffer,
  signEventHash,
  verifyEventSignature
} from '../sync/crypto.js';

export const X402_DOMAIN_SEPARATOR = 'X402_PAYMENT_V1';

const MAX_U64 = (1n << 64n) - 1n;

const NETWORK_CHAIN_ID = {
  set_chain: 84532001,
  set_chain_testnet: 84532002,
  arc: 5042001,
  arc_testnet: 5042002,
  base: 8453,
  base_sepolia: 84532,
  ethereum: 1,
};

const NETWORK_ALIASES = {
  set: 'set_chain',
  setchain: 'set_chain',
  set_chain: 'set_chain',
  set_chain_testnet: 'set_chain_testnet',
  set_testnet: 'set_chain_testnet',
  arc: 'arc',
  arc_testnet: 'arc_testnet',
  base: 'base',
  base_sepolia: 'base_sepolia',
  ethereum: 'ethereum',
  eth: 'ethereum',
};

const ASSET_ALIASES = {
  usdc: 'usdc',
  usdt: 'usdt',
  ssusd: 'ssusd',
  ss_usd: 'ssusd',
  dai: 'dai',
};

function toU64(value, name) {
  const n = typeof value === 'bigint' ? value : BigInt(value);
  if (n < 0n || n > MAX_U64) {
    throw new Error(`${name} must be a u64`);
  }
  return n;
}

export function normalizeNetwork(network) {
  if (!network) throw new Error('network is required');
  const key = String(network).toLowerCase();
  const normalized = NETWORK_ALIASES[key];
  if (!normalized) {
    throw new Error(`Unsupported x402 network: ${network}`);
  }
  return normalized;
}

export function normalizeAsset(asset) {
  if (!asset) throw new Error('asset is required');
  const key = String(asset).toLowerCase();
  const normalized = ASSET_ALIASES[key];
  if (!normalized) {
    throw new Error(`Unsupported x402 asset: ${asset}`);
  }
  return normalized;
}

export function networkChainId(network) {
  const normalized = normalizeNetwork(network);
  const chainId = NETWORK_CHAIN_ID[normalized];
  if (!chainId) {
    throw new Error(`Missing chain id for network: ${normalized}`);
  }
  return chainId;
}

export function computeX402SigningHash({
  payerAddress,
  payeeAddress,
  amount,
  asset,
  network,
  chainId,
  validUntil,
  nonce,
}) {
  if (!payerAddress || !payeeAddress) {
    throw new Error('payerAddress and payeeAddress are required');
  }

  const normalizedAsset = normalizeAsset(asset);
  const normalizedNetwork = normalizeNetwork(network);
  const resolvedChainId = chainId ?? networkChainId(normalizedNetwork);

  const amt = toU64(amount, 'amount');
  const valid = toU64(validUntil, 'validUntil');
  const n = toU64(nonce, 'nonce');

  const hasher = crypto.createHash('sha256');
  hasher.update(Buffer.from(X402_DOMAIN_SEPARATOR, 'utf8'));
  hasher.update(Buffer.from(String(payerAddress), 'utf8'));
  hasher.update(Buffer.from(String(payeeAddress), 'utf8'));
  hasher.update(u64BE(amt));
  hasher.update(Buffer.from(normalizedAsset, 'utf8'));
  hasher.update(Buffer.from(normalizedNetwork, 'utf8'));
  hasher.update(u64BE(toU64(resolvedChainId, 'chainId')));
  hasher.update(u64BE(valid));
  hasher.update(u64BE(n));

  return hasher.digest();
}

export function signX402Hash(signingHash, privateKey) {
  return signEventHash(signingHash, privateKey);
}

export function verifyX402Signature(signingHash, signature, publicKey) {
  return verifyEventSignature(signingHash, signature, publicKey);
}

export function encodeBase64Json(value) {
  return Buffer.from(JSON.stringify(value)).toString('base64');
}

export function decodeBase64Json(value) {
  const raw = Buffer.from(String(value), 'base64').toString('utf8');
  return JSON.parse(raw);
}

export function hashToHex(hash) {
  return bufferToHex(hash);
}

export function hexToBytes(value) {
  return hexToBuffer(value);
}

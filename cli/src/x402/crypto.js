/**
 * x402 Cryptographic Utilities (Sequencer-compatible)
 *
 * Implements the signing hash and Ed25519 signature flow expected by
 * stateset-sequencer's /api/v1/x402 endpoints.
 */

import crypto from 'crypto';

export const X402_DOMAIN_SEPARATOR = 'X402_PAYMENT_V1';

const MAX_U64 = (1n << 64n) - 1n;

/** @type {Record<string, number>} */
const NETWORK_CHAIN_ID = {
  set_chain: 84532001,
  set_chain_testnet: 84532002,
  arc: 5042001,
  arc_testnet: 5042002,
  base: 8453,
  base_sepolia: 84532,
  ethereum: 1,
  ethereum_sepolia: 11155111,
  arbitrum: 42161,
  optimism: 10,
};

/** @type {Record<string, string>} */
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
  ethereum_sepolia: 'ethereum_sepolia',
  sepolia: 'ethereum_sepolia',
  arbitrum: 'arbitrum',
  arb: 'arbitrum',
  optimism: 'optimism',
  op: 'optimism',
};

/** @type {Record<string, string>} */
const ASSET_ALIASES = {
  usdc: 'usdc',
  usdt: 'usdt',
  ssusd: 'ssusd',
  ss_usd: 'ssusd',
  wssusd: 'wssusd',
  wss_usd: 'wssusd',
  dai: 'dai',
  eth: 'eth',
};

/**
 * @typedef {{
 *   payerAddress: unknown,
 *   payeeAddress: unknown,
 *   amount: unknown,
 *   asset: unknown,
 *   network: unknown,
 *   chainId?: unknown,
 *   validUntil: unknown,
 *   nonce: unknown,
 *   resourceUri?: unknown,
 *   resourceMethod?: unknown,
 * }} X402SigningHashInput
 */

/**
 * @param {unknown} error
 * @returns {string}
 */
function messageFromError(error) {
  return error instanceof Error ? error.message : String(error);
}

/**
 * @param {number | bigint} n
 * @returns {Buffer}
 */
function u64BE(n) {
  const buf = Buffer.alloc(8);
  buf.writeBigUInt64BE(BigInt(n));
  return buf;
}

/**
 * @param {string} hex
 * @returns {Buffer}
 */
function hexToBuffer(hex) {
  const normalized = hex.startsWith('0x') ? hex.slice(2) : hex;
  return Buffer.from(normalized, 'hex');
}

/**
 * @param {Buffer} buf
 * @returns {string}
 */
function bufferToHex(buf) {
  return `0x${buf.toString('hex')}`;
}

/**
 * @param {Buffer} eventSigningHash
 * @param {Buffer} privateKey
 * @returns {Buffer}
 */
function signEventHash(eventSigningHash, privateKey) {
  const keyObj = crypto.createPrivateKey({
    key: Buffer.concat([Buffer.from('302e020100300506032b657004220420', 'hex'), privateKey]),
    format: 'der',
    type: 'pkcs8',
  });

  return crypto.sign(null, eventSigningHash, keyObj);
}

/**
 * @param {Buffer} eventSigningHash
 * @param {Buffer} signature
 * @param {Buffer} publicKey
 * @returns {boolean}
 */
function verifyEventSignature(eventSigningHash, signature, publicKey) {
  try {
    const keyObj = crypto.createPublicKey({
      key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), publicKey]),
      format: 'der',
      type: 'spki',
    });

    return crypto.verify(null, eventSigningHash, keyObj, signature);
  } catch (error) {
    console.debug('[x402-crypto] Signature verification failed:', messageFromError(error));
    return false;
  }
}

/**
 * @param {unknown} value
 * @param {string} name
 * @returns {bigint}
 */
function toU64(value, name) {
  const n = typeof value === 'bigint' ? value : BigInt(String(value));
  if (n < 0n || n > MAX_U64) {
    throw new Error(`${name} must be a u64`);
  }
  return n;
}

/**
 * @param {unknown} network
 * @returns {string}
 */
export function normalizeNetwork(network) {
  if (!network) throw new Error('network is required');
  const key = String(network).toLowerCase();
  const normalized = NETWORK_ALIASES[key];
  if (!normalized) {
    throw new Error(`Unsupported x402 network: ${network}`);
  }
  return normalized;
}

/**
 * @param {unknown} asset
 * @returns {string}
 */
export function normalizeAsset(asset) {
  if (!asset) throw new Error('asset is required');
  const key = String(asset).toLowerCase();
  const normalized = ASSET_ALIASES[key];
  if (!normalized) {
    throw new Error(`Unsupported x402 asset: ${asset}`);
  }
  return normalized;
}

/**
 * @param {unknown} network
 * @returns {number}
 */
export function networkChainId(network) {
  const normalized = normalizeNetwork(network);
  const chainId = NETWORK_CHAIN_ID[normalized];
  if (!chainId) {
    throw new Error(`Missing chain id for network: ${normalized}`);
  }
  return chainId;
}

/**
 * @param {X402SigningHashInput} input
 * @returns {Buffer}
 */
export function computeX402SigningHash({
  payerAddress,
  payeeAddress,
  amount,
  asset,
  network,
  chainId,
  validUntil,
  nonce,
  resourceUri,
  resourceMethod,
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
  if (resourceUri === undefined || resourceUri === null || resourceUri === '') {
    hasher.update(Buffer.from([0]));
  } else {
    const uri = Buffer.from(String(resourceUri), 'utf8');
    hasher.update(Buffer.from([1]));
    hasher.update(u64BE(uri.length));
    hasher.update(uri);
  }
  if (resourceMethod === undefined || resourceMethod === null || resourceMethod === '') {
    hasher.update(Buffer.from([0]));
  } else {
    const method = Buffer.from(String(resourceMethod), 'utf8');
    hasher.update(Buffer.from([1]));
    hasher.update(u64BE(method.length));
    hasher.update(method);
  }

  return hasher.digest();
}

/**
 * @param {Buffer} signingHash
 * @param {Buffer} privateKey
 * @returns {Buffer}
 */
export function signX402Hash(signingHash, privateKey) {
  return signEventHash(signingHash, privateKey);
}

/**
 * @param {Buffer} signingHash
 * @param {Buffer} signature
 * @param {Buffer} publicKey
 * @returns {boolean}
 */
export function verifyX402Signature(signingHash, signature, publicKey) {
  return verifyEventSignature(signingHash, signature, publicKey);
}

/**
 * @param {unknown} value
 * @returns {string}
 */
export function encodeBase64Json(value) {
  return Buffer.from(JSON.stringify(value)).toString('base64');
}

/**
 * @param {unknown} value
 * @returns {unknown}
 */
export function decodeBase64Json(value) {
  const raw = Buffer.from(String(value), 'base64').toString('utf8');
  return JSON.parse(raw);
}

/**
 * @param {Buffer} hash
 * @returns {string}
 */
export function hashToHex(hash) {
  return bufferToHex(hash);
}

/**
 * @param {unknown} value
 * @returns {Buffer}
 */
export function hexToBytes(value) {
  return hexToBuffer(String(value));
}

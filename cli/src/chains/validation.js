/**
 * Input validation for blockchain payments.
 *
 * Validates payment parameters before execution to catch errors early.
 */

import {
  CHAINS,
  getChain,
  getToken,
  getDefaultPaymentToken,
  isEd25519Chain,
  isEvmChain,
  isZcashChain,
  isBitcoinChain,
} from './config.js';
import { isValidEthAddress } from './crypto-utils.js';

// =============================================================================
// VALIDATION ERRORS
// =============================================================================

/**
 * Custom validation error with code and context
 */
export class ValidationError extends Error {
  constructor(code, message, context = {}) {
    super(message);
    this.name = 'ValidationError';
    this.code = code;
    this.context = context;
  }

  toJSON() {
    return {
      error: this.name,
      code: this.code,
      message: this.message,
      context: this.context,
    };
  }
}

// Error codes
export const ValidationErrorCodes = {
  INVALID_CHAIN: 'INVALID_CHAIN',
  INVALID_TOKEN: 'INVALID_TOKEN',
  INVALID_AMOUNT: 'INVALID_AMOUNT',
  AMOUNT_TOO_SMALL: 'AMOUNT_TOO_SMALL',
  AMOUNT_TOO_LARGE: 'AMOUNT_TOO_LARGE',
  INVALID_ADDRESS: 'INVALID_ADDRESS',
  INVALID_ADDRESS_FORMAT: 'INVALID_ADDRESS_FORMAT',
  SELF_TRANSFER: 'SELF_TRANSFER',
  MISSING_REQUIRED: 'MISSING_REQUIRED',
  INVALID_AGENT_ID: 'INVALID_AGENT_ID',
  INVALID_METADATA: 'INVALID_METADATA',
};

// =============================================================================
// CHAIN VALIDATION
// =============================================================================

/**
 * Validate chain ID
 * @param {string} chainId
 * @returns {true}
 * @throws {ValidationError}
 */
export function validateChainId(chainId) {
  if (!chainId || typeof chainId !== 'string') {
    throw new ValidationError(ValidationErrorCodes.MISSING_REQUIRED, 'Chain ID is required', {
      field: 'chainId',
    });
  }

  const chain = getChain(chainId);
  if (!chain) {
    const validChains = Object.keys(CHAINS);
    throw new ValidationError(
      ValidationErrorCodes.INVALID_CHAIN,
      `Unknown chain: ${chainId}. Valid chains: ${validChains.join(', ')}`,
      { chainId, validChains },
    );
  }

  return true;
}

/**
 * Validate token symbol for a chain
 * @param {string} chainId
 * @param {string} [tokenSymbol]
 * @returns {true}
 * @throws {ValidationError}
 */
export function validateToken(chainId, tokenSymbol) {
  validateChainId(chainId);

  // If no token specified, check a default payment asset exists
  if (!tokenSymbol) {
    const defaultToken = getDefaultPaymentToken(chainId);
    if (!defaultToken) {
      throw new ValidationError(
        ValidationErrorCodes.INVALID_TOKEN,
        `No default payment token configured for chain ${chainId}`,
        { chainId },
      );
    }
    return true;
  }

  const token = getToken(chainId, tokenSymbol);
  if (!token) {
    const chain = getChain(chainId);
    const validTokens = Object.keys(chain.tokens || {});
    throw new ValidationError(
      ValidationErrorCodes.INVALID_TOKEN,
      `Unknown token: ${tokenSymbol} on chain ${chainId}. Valid tokens: ${validTokens.join(', ')}`,
      { chainId, tokenSymbol, validTokens },
    );
  }

  return true;
}

// =============================================================================
// AMOUNT VALIDATION
// =============================================================================

// Configuration
const MIN_PAYMENT_AMOUNT = 0.000001; // Minimum $0.000001
const MAX_PAYMENT_AMOUNT = 1_000_000_000; // Maximum $1 billion

/**
 * Validate payment amount
 * @param {string|number} amount
 * @param {Object} [options]
 * @param {number} [options.minAmount] - Minimum allowed amount
 * @param {number} [options.maxAmount] - Maximum allowed amount
 * @returns {number} Parsed amount
 * @throws {ValidationError}
 */
export function validateAmount(amount, options = {}) {
  const { minAmount = MIN_PAYMENT_AMOUNT, maxAmount = MAX_PAYMENT_AMOUNT } = options;

  // Check presence
  if (amount === null || amount === undefined || amount === '') {
    throw new ValidationError(ValidationErrorCodes.MISSING_REQUIRED, 'Amount is required', {
      field: 'amount',
    });
  }

  // Parse to number
  const numAmount = typeof amount === 'string' ? parseFloat(amount) : amount;

  // Check it's a valid number
  if (!Number.isFinite(numAmount)) {
    throw new ValidationError(
      ValidationErrorCodes.INVALID_AMOUNT,
      `Invalid amount: ${amount}. Must be a valid number.`,
      { amount },
    );
  }

  // Check positive
  if (numAmount <= 0) {
    throw new ValidationError(
      ValidationErrorCodes.INVALID_AMOUNT,
      `Amount must be positive, got: ${numAmount}`,
      { amount: numAmount },
    );
  }

  // Check minimum
  if (numAmount < minAmount) {
    throw new ValidationError(
      ValidationErrorCodes.AMOUNT_TOO_SMALL,
      `Amount ${numAmount} is below minimum of ${minAmount}`,
      { amount: numAmount, minAmount },
    );
  }

  // Check maximum
  if (numAmount > maxAmount) {
    throw new ValidationError(
      ValidationErrorCodes.AMOUNT_TOO_LARGE,
      `Amount ${numAmount} exceeds maximum of ${maxAmount}`,
      { amount: numAmount, maxAmount },
    );
  }

  // Check reasonable decimal precision (max 18 decimals)
  const decimalStr = numAmount.toString();
  const decimalPart = decimalStr.includes('.') ? decimalStr.split('.')[1] : '';
  if (decimalPart.length > 18) {
    throw new ValidationError(
      ValidationErrorCodes.INVALID_AMOUNT,
      `Amount has too many decimal places (max 18): ${numAmount}`,
      { amount: numAmount, decimals: decimalPart.length },
    );
  }

  return numAmount;
}

// =============================================================================
// ADDRESS VALIDATION
// =============================================================================

/**
 * Validate blockchain address format
 * @param {string} address
 * @param {string} chainId
 * @returns {true}
 * @throws {ValidationError}
 */
export function validateAddress(address, chainId) {
  if (!address || typeof address !== 'string') {
    throw new ValidationError(ValidationErrorCodes.MISSING_REQUIRED, 'Address is required', {
      field: 'address',
    });
  }

  const trimmed = address.trim();

  if (isEvmChain(chainId)) {
    return validateEvmAddress(trimmed);
  } else if (isZcashChain(chainId)) {
    return validateZcashAddress(trimmed);
  } else if (isBitcoinChain(chainId)) {
    return validateBitcoinAddress(trimmed);
  } else if (isEd25519Chain(chainId)) {
    return validateSolanaAddress(trimmed);
  }

  // For unknown chain types, just check it's non-empty
  if (trimmed.length === 0) {
    throw new ValidationError(ValidationErrorCodes.INVALID_ADDRESS, 'Address cannot be empty', {
      address,
    });
  }

  return true;
}

/**
 * Validate EVM address (Ethereum, Base, Arbitrum, SET Chain)
 * @param {string} address
 * @returns {true}
 * @throws {ValidationError}
 */
export function validateEvmAddress(address) {
  // Check format: 0x followed by 40 hex characters
  if (!/^0x[0-9a-fA-F]{40}$/.test(address)) {
    throw new ValidationError(
      ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
      `Invalid EVM address format: ${address}. Must be 0x followed by 40 hex characters.`,
      { address, expectedFormat: '0x' + '0'.repeat(40) },
    );
  }

  // Check checksum if mixed case
  if (!isValidEthAddress(address)) {
    throw new ValidationError(
      ValidationErrorCodes.INVALID_ADDRESS,
      `Invalid EVM address checksum: ${address}. The address may have incorrect capitalization.`,
      { address },
    );
  }

  return true;
}

/**
 * Validate Solana address (base58 encoded, 32-44 characters)
 * @param {string} address
 * @returns {true}
 * @throws {ValidationError}
 */
export function validateSolanaAddress(address) {
  // Solana addresses are base58 encoded, typically 32-44 characters
  // Valid base58 characters: 123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  const base58Regex = /^[1-9A-HJ-NP-Za-km-z]{32,44}$/;

  if (!base58Regex.test(address)) {
    throw new ValidationError(
      ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
      `Invalid Solana address format: ${address}. Must be 32-44 base58 characters.`,
      { address },
    );
  }

  return true;
}

/**
 * Validate Zcash address
 * Supports:
 * - t-addresses (transparent): t1... or t3... (base58check, 35 chars)
 * - z-addresses (shielded Sapling): zs1... (bech32, 78 chars)
 * - Unified addresses: u1... (bech32m)
 *
 * @param {string} address
 * @returns {true}
 * @throws {ValidationError}
 */
export function validateZcashAddress(address) {
  // Transparent t-addresses (mainnet: t1, t3; testnet: tm, t2)
  // Format: base58check encoded, typically 35 characters
  const tAddressRegex = /^t[13m2][1-9A-HJ-NP-Za-km-z]{33}$/;

  // Shielded z-addresses (Sapling): zs1... (mainnet) or ztestsapling1... (testnet)
  // Format: bech32 encoded, 78 characters for mainnet
  const zAddressRegex = /^zs1[a-z0-9]{75}$/;
  const zTestAddressRegex = /^ztestsapling1[a-z0-9]{65,}$/;

  // Unified addresses: u1... (mainnet) or utest1... (testnet)
  // Variable length bech32m
  const unifiedAddressRegex = /^u1[a-z0-9]{50,200}$/;
  const unifiedTestRegex = /^utest1[a-z0-9]{50,200}$/;

  if (
    tAddressRegex.test(address) ||
    zAddressRegex.test(address) ||
    zTestAddressRegex.test(address) ||
    unifiedAddressRegex.test(address) ||
    unifiedTestRegex.test(address)
  ) {
    return true;
  }

  throw new ValidationError(
    ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    `Invalid Zcash address format: ${address}. Supported: t-addresses (t1.../t3...), z-addresses (zs1...), unified (u1...)`,
    { address },
  );
}

/**
 * Validate Bitcoin address
 * Supports:
 * - P2PKH addresses (legacy): 1... (mainnet, 25-34 chars) or m.../n... (testnet)
 * - P2SH addresses: 3... (mainnet) or 2... (testnet)
 * - Bech32 addresses (SegWit): bc1... (mainnet) or tb1... (testnet)
 *
 * @param {string} address
 * @returns {true}
 * @throws {ValidationError}
 */
export function validateBitcoinAddress(address) {
  // P2PKH legacy addresses (mainnet: 1..., testnet: m... or n...)
  // Base58Check encoded, 25-34 characters
  const p2pkhMainnetRegex = /^1[1-9A-HJ-NP-Za-km-z]{25,34}$/;
  const p2pkhTestnetRegex = /^[mn][1-9A-HJ-NP-Za-km-z]{25,34}$/;

  // P2SH addresses (mainnet: 3..., testnet: 2...)
  const p2shMainnetRegex = /^3[1-9A-HJ-NP-Za-km-z]{25,34}$/;
  const p2shTestnetRegex = /^2[1-9A-HJ-NP-Za-km-z]{25,34}$/;

  // Bech32 native SegWit addresses
  // mainnet: bc1q... (P2WPKH) or bc1p... (P2TR/Taproot)
  // testnet: tb1q... or tb1p...
  const bech32MainnetRegex = /^bc1[qpzry9x8gf2tvdw0s3jn54khce6mua7l]{39,59}$/;
  const bech32TestnetRegex = /^tb1[qpzry9x8gf2tvdw0s3jn54khce6mua7l]{39,59}$/;

  if (
    p2pkhMainnetRegex.test(address) ||
    p2pkhTestnetRegex.test(address) ||
    p2shMainnetRegex.test(address) ||
    p2shTestnetRegex.test(address) ||
    bech32MainnetRegex.test(address) ||
    bech32TestnetRegex.test(address)
  ) {
    return true;
  }

  throw new ValidationError(
    ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    `Invalid Bitcoin address format: ${address}. Supported: P2PKH (1.../m.../n...), P2SH (3.../2...), Bech32 (bc1.../tb1...)`,
    { address },
  );
}

// =============================================================================
// PAYMENT PARAMS VALIDATION
// =============================================================================

/**
 * Validate full payment parameters
 * @param {Object} params
 * @param {string} params.agentId
 * @param {string} params.chainId
 * @param {string} params.toAddress
 * @param {string|number} params.amount
 * @param {string} [params.tokenSymbol]
 * @param {Object} [params.metadata]
 * @param {string} [params.fromAddress] - For self-transfer check
 * @returns {Object} Validated and normalized params
 * @throws {ValidationError}
 */
export function validatePaymentParams(params) {
  const errors = [];

  // Validate agent ID
  if (!params.agentId || typeof params.agentId !== 'string') {
    errors.push({
      code: ValidationErrorCodes.INVALID_AGENT_ID,
      message: 'Agent ID is required and must be a string',
      field: 'agentId',
    });
  }

  // Validate chain
  try {
    validateChainId(params.chainId);
  } catch (e) {
    errors.push({ code: e.code, message: e.message, field: 'chainId' });
  }

  // Validate token (if chain is valid)
  if (!errors.find((e) => e.field === 'chainId')) {
    try {
      validateToken(params.chainId, params.tokenSymbol);
    } catch (e) {
      errors.push({ code: e.code, message: e.message, field: 'tokenSymbol' });
    }
  }

  // Validate amount
  let validatedAmount;
  try {
    validatedAmount = validateAmount(params.amount);
  } catch (e) {
    errors.push({ code: e.code, message: e.message, field: 'amount' });
  }

  // Validate address (if chain is valid)
  if (!errors.find((e) => e.field === 'chainId')) {
    try {
      validateAddress(params.toAddress, params.chainId);
    } catch (e) {
      errors.push({ code: e.code, message: e.message, field: 'toAddress' });
    }
  }

  // Check for self-transfer
  if (params.fromAddress && params.toAddress) {
    const from = params.fromAddress.toLowerCase();
    const to = params.toAddress.toLowerCase();
    if (from === to) {
      errors.push({
        code: ValidationErrorCodes.SELF_TRANSFER,
        message: 'Cannot transfer to the same address',
        field: 'toAddress',
      });
    }
  }

  // Validate metadata (if provided)
  if (params.metadata !== undefined && params.metadata !== null) {
    if (typeof params.metadata !== 'object' || Array.isArray(params.metadata)) {
      errors.push({
        code: ValidationErrorCodes.INVALID_METADATA,
        message: 'Metadata must be an object',
        field: 'metadata',
      });
    }
  }

  // Throw if any errors
  if (errors.length > 0) {
    const firstError = errors[0];
    const error = new ValidationError(
      firstError.code,
      errors.length === 1
        ? firstError.message
        : `Multiple validation errors: ${errors.map((e) => e.message).join('; ')}`,
      { errors },
    );
    throw error;
  }

  // Return normalized params
  return {
    agentId: params.agentId.trim(),
    chainId: params.chainId.trim().toLowerCase(),
    toAddress: params.toAddress.trim(),
    amount: validatedAmount,
    tokenSymbol: params.tokenSymbol?.trim().toUpperCase(),
    metadata: params.metadata || {},
  };
}

// =============================================================================
// EXPORTS
// =============================================================================

export default {
  ValidationError,
  ValidationErrorCodes,
  validateChainId,
  validateToken,
  validateAmount,
  validateAddress,
  validateEvmAddress,
  validateSolanaAddress,
  validateZcashAddress,
  validateBitcoinAddress,
  validatePaymentParams,
};

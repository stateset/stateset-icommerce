/**
 * Tests for chains/validation.js — payment parameter validation.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
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
} from '../../src/chains/validation.js';

// =============================================================================
// ValidationError
// =============================================================================

describe('ValidationError', () => {
  it('has code, message, and context', () => {
    const err = new ValidationError('TEST_CODE', 'test message', { foo: 'bar' });
    assert.equal(err.code, 'TEST_CODE');
    assert.equal(err.message, 'test message');
    assert.deepEqual(err.context, { foo: 'bar' });
    assert.equal(err.name, 'ValidationError');
  });

  it('serializes to JSON', () => {
    const err = new ValidationError('CODE', 'msg', {});
    const json = err.toJSON();
    assert.equal(json.error, 'ValidationError');
    assert.equal(json.code, 'CODE');
    assert.equal(json.message, 'msg');
  });
});

// =============================================================================
// validateChainId
// =============================================================================

describe('validateChainId', () => {
  it('accepts valid chain IDs', () => {
    assert.ok(validateChainId('solana'));
    assert.ok(validateChainId('set_chain'));
    assert.ok(validateChainId('ethereum'));
    assert.ok(validateChainId('bitcoin'));
  });

  it('throws for unknown chain', () => {
    assert.throws(
      () => validateChainId('unknown_chain'),
      (err) => err.code === ValidationErrorCodes.INVALID_CHAIN,
    );
  });

  it('throws for empty string', () => {
    assert.throws(
      () => validateChainId(''),
      (err) => err.code === ValidationErrorCodes.MISSING_REQUIRED,
    );
  });

  it('throws for null', () => {
    assert.throws(
      () => validateChainId(null),
      (err) => err.code === ValidationErrorCodes.MISSING_REQUIRED,
    );
  });
});

// =============================================================================
// validateToken
// =============================================================================

describe('validateToken', () => {
  it('accepts valid token on chain', () => {
    assert.ok(validateToken('solana', 'USDC'));
  });

  it('accepts null tokenSymbol when default stablecoin exists', () => {
    assert.ok(validateToken('solana'));
  });

  it('accepts omitted token when native payment token exists on bitcoin', () => {
    assert.ok(validateToken('bitcoin'));
  });

  it('throws for unknown token', () => {
    assert.throws(
      () => validateToken('solana', 'FAKECOIN'),
      (err) => err.code === ValidationErrorCodes.INVALID_TOKEN,
    );
  });
});

// =============================================================================
// validateAmount
// =============================================================================

describe('validateAmount', () => {
  it('accepts valid positive amount', () => {
    assert.equal(validateAmount(100), 100);
  });

  it('accepts string amount', () => {
    assert.equal(validateAmount('50.5'), 50.5);
  });

  it('throws for zero', () => {
    assert.throws(
      () => validateAmount(0),
      (err) => err.code === ValidationErrorCodes.INVALID_AMOUNT,
    );
  });

  it('throws for negative', () => {
    assert.throws(
      () => validateAmount(-10),
      (err) => err.code === ValidationErrorCodes.INVALID_AMOUNT,
    );
  });

  it('throws for NaN', () => {
    assert.throws(
      () => validateAmount('not a number'),
      (err) => err.code === ValidationErrorCodes.INVALID_AMOUNT,
    );
  });

  it('throws for null', () => {
    assert.throws(
      () => validateAmount(null),
      (err) => err.code === ValidationErrorCodes.MISSING_REQUIRED,
    );
  });

  it('throws below minimum', () => {
    assert.throws(
      () => validateAmount(0.0000001, { minAmount: 0.001 }),
      (err) => err.code === ValidationErrorCodes.AMOUNT_TOO_SMALL,
    );
  });

  it('throws above maximum', () => {
    assert.throws(
      () => validateAmount(2_000_000_000),
      (err) => err.code === ValidationErrorCodes.AMOUNT_TOO_LARGE,
    );
  });

  it('respects custom max', () => {
    assert.throws(
      () => validateAmount(200, { maxAmount: 100 }),
      (err) => err.code === ValidationErrorCodes.AMOUNT_TOO_LARGE,
    );
  });
});

// =============================================================================
// Address validation
// =============================================================================

describe('validateEvmAddress', () => {
  it('accepts valid all-lowercase address', () => {
    assert.ok(validateEvmAddress('0x0000000000000000000000000000000000000000'));
  });

  it('accepts valid all-uppercase address', () => {
    assert.ok(validateEvmAddress('0x0000000000000000000000000000000000000000'));
  });

  it('rejects non-hex characters', () => {
    assert.throws(
      () => validateEvmAddress('0xZZZZ000000000000000000000000000000000000'),
      (err) => err.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });

  it('rejects too-short address', () => {
    assert.throws(
      () => validateEvmAddress('0x1234'),
      (err) => err.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });

  it('rejects address without 0x prefix', () => {
    assert.throws(
      () => validateEvmAddress('0000000000000000000000000000000000000000'),
      (err) => err.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });
});

describe('validateSolanaAddress', () => {
  it('accepts valid base58 address', () => {
    assert.ok(validateSolanaAddress('11111111111111111111111111111111'));
  });

  it('rejects too-short address', () => {
    assert.throws(
      () => validateSolanaAddress('short'),
      (err) => err.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });

  it('rejects invalid characters (0, O, I, l)', () => {
    assert.throws(
      () => validateSolanaAddress('0OIl0OIl0OIl0OIl0OIl0OIl0OIl0OIl'),
      (err) => err.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });
});

describe('validateZcashAddress', () => {
  it('accepts valid t-address (t1 prefix)', () => {
    assert.ok(validateZcashAddress('t1' + '1'.repeat(33)));
  });

  it('accepts valid t-address (t3 prefix)', () => {
    assert.ok(validateZcashAddress('t3' + 'A'.repeat(33)));
  });

  it('rejects invalid prefix', () => {
    assert.throws(
      () => validateZcashAddress('x1' + '1'.repeat(33)),
      (err) => err.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });
});

describe('validateBitcoinAddress', () => {
  it('accepts valid P2PKH address (1... prefix)', () => {
    assert.ok(validateBitcoinAddress('1' + 'A'.repeat(33)));
  });

  it('accepts valid P2SH address (3... prefix)', () => {
    assert.ok(validateBitcoinAddress('3' + 'A'.repeat(33)));
  });

  it('accepts valid bech32 address (bc1q)', () => {
    assert.ok(validateBitcoinAddress('bc1q' + 'a'.repeat(38)));
  });

  it('accepts valid testnet bech32 address', () => {
    assert.ok(validateBitcoinAddress('tb1q' + 'a'.repeat(38)));
  });

  it('rejects invalid address', () => {
    assert.throws(
      () => validateBitcoinAddress('xyz123'),
      (err) => err.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });
});

describe('validateAddress (dispatches by chain)', () => {
  it('validates EVM address for ethereum', () => {
    assert.ok(validateAddress('0x0000000000000000000000000000000000000000', 'ethereum'));
  });

  it('validates Solana address', () => {
    assert.ok(validateAddress('11111111111111111111111111111111', 'solana'));
  });

  it('validates Bitcoin address', () => {
    assert.ok(validateAddress('1' + 'A'.repeat(33), 'bitcoin'));
  });

  it('throws for missing address', () => {
    assert.throws(
      () => validateAddress(null, 'ethereum'),
      (err) => err.code === ValidationErrorCodes.MISSING_REQUIRED,
    );
  });
});

// =============================================================================
// validatePaymentParams
// =============================================================================

describe('validatePaymentParams', () => {
  const validParams = {
    agentId: 'agent-1',
    chainId: 'set_chain',
    toAddress: '0x0000000000000000000000000000000000000000',
    amount: 100,
    tokenSymbol: 'ssUSD',
  };

  it('accepts valid params', () => {
    const result = validatePaymentParams(validParams);
    assert.equal(result.agentId, 'agent-1');
    assert.equal(result.amount, 100);
    assert.equal(result.tokenSymbol, 'SSUSD');
  });

  it('normalizes chain to lowercase', () => {
    const result = validatePaymentParams({ ...validParams, chainId: 'set_chain' });
    assert.equal(result.chainId, 'set_chain');
  });

  it('throws for missing agentId', () => {
    assert.throws(
      () => validatePaymentParams({ ...validParams, agentId: '' }),
      (err) => err instanceof ValidationError,
    );
  });

  it('throws for invalid chain', () => {
    assert.throws(
      () => validatePaymentParams({ ...validParams, chainId: 'fake' }),
      (err) => err instanceof ValidationError,
    );
  });

  it('throws for self-transfer', () => {
    assert.throws(
      () =>
        validatePaymentParams({
          ...validParams,
          fromAddress: '0x0000000000000000000000000000000000000000',
          toAddress: '0x0000000000000000000000000000000000000000',
        }),
      (err) => err.context.errors.some((e) => e.code === ValidationErrorCodes.SELF_TRANSFER),
    );
  });

  it('throws for invalid metadata type', () => {
    assert.throws(
      () => validatePaymentParams({ ...validParams, metadata: 'not an object' }),
      (err) => err instanceof ValidationError,
    );
  });

  it('accepts metadata as object', () => {
    const result = validatePaymentParams({ ...validParams, metadata: { orderId: '123' } });
    assert.deepEqual(result.metadata, { orderId: '123' });
  });
});

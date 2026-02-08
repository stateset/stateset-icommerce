/**
 * Tests for cli/src/chains/validation.js
 *
 * Covers: ValidationError, ValidationErrorCodes, validateChainId,
 * validateToken, validateAmount, validateAddress (EVM, Solana, Zcash, Bitcoin),
 * validatePaymentParams.
 */

import { describe, it, beforeEach } from 'node:test';
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

// ---------------------------------------------------------------------------
// ValidationError
// ---------------------------------------------------------------------------

describe('ValidationError', () => {
  it('extends Error', () => {
    const err = new ValidationError('CODE', 'msg');
    assert.ok(err instanceof Error);
    assert.ok(err instanceof ValidationError);
  });

  it('stores code, message, context', () => {
    const ctx = { field: 'amount' };
    const err = new ValidationError('INVALID_AMOUNT', 'bad amount', ctx);
    assert.equal(err.code, 'INVALID_AMOUNT');
    assert.equal(err.message, 'bad amount');
    assert.deepEqual(err.context, ctx);
    assert.equal(err.name, 'ValidationError');
  });

  it('defaults context to empty object', () => {
    const err = new ValidationError('CODE', 'msg');
    assert.deepEqual(err.context, {});
  });

  it('toJSON returns structured object', () => {
    const err = new ValidationError('C', 'M', { x: 1 });
    const json = err.toJSON();
    assert.equal(json.error, 'ValidationError');
    assert.equal(json.code, 'C');
    assert.equal(json.message, 'M');
    assert.deepEqual(json.context, { x: 1 });
  });
});

// ---------------------------------------------------------------------------
// ValidationErrorCodes
// ---------------------------------------------------------------------------

describe('ValidationErrorCodes', () => {
  it('has all expected codes', () => {
    const expected = [
      'INVALID_CHAIN',
      'INVALID_TOKEN',
      'INVALID_AMOUNT',
      'AMOUNT_TOO_SMALL',
      'AMOUNT_TOO_LARGE',
      'INVALID_ADDRESS',
      'INVALID_ADDRESS_FORMAT',
      'SELF_TRANSFER',
      'MISSING_REQUIRED',
      'INVALID_AGENT_ID',
      'INVALID_METADATA',
    ];
    for (const code of expected) {
      assert.ok(code in ValidationErrorCodes, `missing code: ${code}`);
    }
  });
});

// ---------------------------------------------------------------------------
// validateChainId
// ---------------------------------------------------------------------------

describe('validateChainId', () => {
  it('returns true for valid chain', () => {
    assert.equal(validateChainId('solana'), true);
  });

  it('accepts solana_devnet', () => {
    assert.equal(validateChainId('solana_devnet'), true);
  });

  it('accepts EVM chains', () => {
    assert.equal(validateChainId('ethereum'), true);
    assert.equal(validateChainId('base'), true);
  });

  it('throws MISSING_REQUIRED for falsy input', () => {
    assert.throws(
      () => validateChainId(null),
      (e) => e.code === ValidationErrorCodes.MISSING_REQUIRED,
    );
    assert.throws(
      () => validateChainId(''),
      (e) => e.code === ValidationErrorCodes.MISSING_REQUIRED,
    );
    assert.throws(
      () => validateChainId(undefined),
      (e) => e.code === ValidationErrorCodes.MISSING_REQUIRED,
    );
  });

  it('throws MISSING_REQUIRED for non-string', () => {
    assert.throws(
      () => validateChainId(42),
      (e) => e.code === ValidationErrorCodes.MISSING_REQUIRED,
    );
  });

  it('throws INVALID_CHAIN for unknown chain', () => {
    assert.throws(
      () => validateChainId('fantom'),
      (e) => {
        assert.equal(e.code, ValidationErrorCodes.INVALID_CHAIN);
        assert.ok(e.context.validChains.length > 0);
        return true;
      },
    );
  });
});

// ---------------------------------------------------------------------------
// validateToken
// ---------------------------------------------------------------------------

describe('validateToken', () => {
  it('returns true when token exists on chain', () => {
    assert.equal(validateToken('solana', 'USDC'), true);
  });

  it('returns true when no token specified but default exists', () => {
    assert.equal(validateToken('solana'), true);
  });

  it('throws INVALID_TOKEN for unknown token on valid chain', () => {
    assert.throws(
      () => validateToken('solana', 'DOGE'),
      (e) => {
        assert.equal(e.code, ValidationErrorCodes.INVALID_TOKEN);
        assert.ok(e.context.validTokens.length > 0);
        return true;
      },
    );
  });

  it('throws INVALID_CHAIN for invalid chain (cascading)', () => {
    assert.throws(
      () => validateToken('badchain', 'USDC'),
      (e) => e.code === ValidationErrorCodes.INVALID_CHAIN,
    );
  });
});

// ---------------------------------------------------------------------------
// validateAmount
// ---------------------------------------------------------------------------

describe('validateAmount', () => {
  it('returns parsed number for valid amount', () => {
    assert.equal(validateAmount(100), 100);
    assert.equal(validateAmount(0.5), 0.5);
  });

  it('parses string amounts', () => {
    assert.equal(validateAmount('42.5'), 42.5);
  });

  it('throws MISSING_REQUIRED for null/undefined/empty', () => {
    for (const val of [null, undefined, '']) {
      assert.throws(
        () => validateAmount(val),
        (e) => e.code === ValidationErrorCodes.MISSING_REQUIRED,
      );
    }
  });

  it('throws INVALID_AMOUNT for NaN / Infinity', () => {
    assert.throws(
      () => validateAmount('abc'),
      (e) => e.code === ValidationErrorCodes.INVALID_AMOUNT,
    );
    assert.throws(
      () => validateAmount(Infinity),
      (e) => e.code === ValidationErrorCodes.INVALID_AMOUNT,
    );
    assert.throws(
      () => validateAmount(-Infinity),
      (e) => e.code === ValidationErrorCodes.INVALID_AMOUNT,
    );
  });

  it('throws INVALID_AMOUNT for zero or negative', () => {
    assert.throws(
      () => validateAmount(0),
      (e) => e.code === ValidationErrorCodes.INVALID_AMOUNT,
    );
    assert.throws(
      () => validateAmount(-5),
      (e) => e.code === ValidationErrorCodes.INVALID_AMOUNT,
    );
  });

  it('throws AMOUNT_TOO_SMALL below minimum', () => {
    assert.throws(
      () => validateAmount(0.0000001, { minAmount: 1 }),
      (e) => e.code === ValidationErrorCodes.AMOUNT_TOO_SMALL,
    );
  });

  it('throws AMOUNT_TOO_LARGE above maximum', () => {
    assert.throws(
      () => validateAmount(999, { maxAmount: 100 }),
      (e) => e.code === ValidationErrorCodes.AMOUNT_TOO_LARGE,
    );
  });

  it('uses default bounds', () => {
    // default min is 0.000001
    assert.equal(validateAmount(0.000001), 0.000001);
    // default max is 1 billion
    assert.equal(validateAmount(1_000_000_000), 1_000_000_000);
  });

  it('rejects amount above default max', () => {
    assert.throws(
      () => validateAmount(1_000_000_001),
      (e) => e.code === ValidationErrorCodes.AMOUNT_TOO_LARGE,
    );
  });

  it('accepts custom min/max', () => {
    assert.equal(validateAmount(50, { minAmount: 10, maxAmount: 100 }), 50);
  });
});

// ---------------------------------------------------------------------------
// validateEvmAddress
// ---------------------------------------------------------------------------

describe('validateEvmAddress', () => {
  // Known valid checksummed EVM address
  const VALID_LOWER = '0x' + 'ab'.repeat(20);
  const VALID_UPPER = '0x' + 'AB'.repeat(20);

  it('accepts lowercase address', () => {
    assert.equal(validateEvmAddress(VALID_LOWER), true);
  });

  it('accepts uppercase address', () => {
    assert.equal(validateEvmAddress(VALID_UPPER), true);
  });

  it('rejects address without 0x prefix', () => {
    assert.throws(
      () => validateEvmAddress('ab'.repeat(20)),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });

  it('rejects address with wrong length', () => {
    assert.throws(
      () => validateEvmAddress('0xabc'),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });

  it('rejects non-hex characters', () => {
    assert.throws(
      () => validateEvmAddress('0x' + 'zz'.repeat(20)),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });
});

// ---------------------------------------------------------------------------
// validateSolanaAddress
// ---------------------------------------------------------------------------

describe('validateSolanaAddress', () => {
  // A realistic base58 address (Solana System Program)
  const VALID = '11111111111111111111111111111111';

  it('accepts valid base58 address', () => {
    assert.equal(validateSolanaAddress(VALID), true);
  });

  it('rejects too short address', () => {
    assert.throws(
      () => validateSolanaAddress('abc'),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });

  it('rejects invalid base58 characters (0, O, I, l)', () => {
    const invalid = '0' + '1'.repeat(43);
    assert.throws(
      () => validateSolanaAddress(invalid),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });
});

// ---------------------------------------------------------------------------
// validateZcashAddress
// ---------------------------------------------------------------------------

describe('validateZcashAddress', () => {
  it('accepts t1 transparent address', () => {
    // t1 + 33 base58check chars
    const addr = 't1' + 'K'.repeat(33);
    assert.equal(validateZcashAddress(addr), true);
  });

  it('accepts t3 transparent address', () => {
    const addr = 't3' + 'K'.repeat(33);
    assert.equal(validateZcashAddress(addr), true);
  });

  it('accepts zs1 shielded address', () => {
    const addr = 'zs1' + 'a'.repeat(75);
    assert.equal(validateZcashAddress(addr), true);
  });

  it('accepts u1 unified address', () => {
    const addr = 'u1' + 'a'.repeat(60);
    assert.equal(validateZcashAddress(addr), true);
  });

  it('rejects invalid prefix', () => {
    assert.throws(
      () => validateZcashAddress('x1' + 'a'.repeat(33)),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });

  it('rejects too short address', () => {
    assert.throws(
      () => validateZcashAddress('t1abc'),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });
});

// ---------------------------------------------------------------------------
// validateBitcoinAddress
// ---------------------------------------------------------------------------

describe('validateBitcoinAddress', () => {
  it('accepts P2PKH mainnet (1...)', () => {
    const addr = '1' + 'A'.repeat(33);
    assert.equal(validateBitcoinAddress(addr), true);
  });

  it('accepts P2SH mainnet (3...)', () => {
    const addr = '3' + 'J'.repeat(33);
    assert.equal(validateBitcoinAddress(addr), true);
  });

  it('accepts P2PKH testnet (m...)', () => {
    const addr = 'm' + 'N'.repeat(33);
    assert.equal(validateBitcoinAddress(addr), true);
  });

  it('accepts P2PKH testnet (n...)', () => {
    const addr = 'n' + 'N'.repeat(33);
    assert.equal(validateBitcoinAddress(addr), true);
  });

  it('accepts bech32 mainnet (bc1q...)', () => {
    // bc1q + 39 bech32 chars
    const addr = 'bc1q' + 'p'.repeat(38);
    assert.equal(validateBitcoinAddress(addr), true);
  });

  it('accepts bech32 testnet (tb1q...)', () => {
    const addr = 'tb1q' + 'p'.repeat(38);
    assert.equal(validateBitcoinAddress(addr), true);
  });

  it('rejects invalid prefix', () => {
    assert.throws(
      () => validateBitcoinAddress('x' + 'A'.repeat(33)),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });

  it('rejects empty', () => {
    assert.throws(
      () => validateBitcoinAddress(''),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });
});

// ---------------------------------------------------------------------------
// validateAddress (dispatcher)
// ---------------------------------------------------------------------------

describe('validateAddress', () => {
  it('throws MISSING_REQUIRED for falsy address', () => {
    assert.throws(
      () => validateAddress(null, 'solana'),
      (e) => e.code === ValidationErrorCodes.MISSING_REQUIRED,
    );
    assert.throws(
      () => validateAddress('', 'solana'),
      (e) => e.code === ValidationErrorCodes.MISSING_REQUIRED,
    );
  });

  it('dispatches to Solana validator for Ed25519 chains', () => {
    const addr = '11111111111111111111111111111111';
    assert.equal(validateAddress(addr, 'solana'), true);
  });

  it('dispatches to EVM validator for EVM chains', () => {
    const addr = '0x' + 'ab'.repeat(20);
    assert.equal(validateAddress(addr, 'ethereum'), true);
  });

  it('dispatches to Zcash validator', () => {
    const addr = 't1' + 'K'.repeat(33);
    assert.equal(validateAddress(addr, 'zcash'), true);
  });

  it('dispatches to Bitcoin validator', () => {
    const addr = '1' + 'A'.repeat(33);
    assert.equal(validateAddress(addr, 'bitcoin'), true);
  });

  it('trims whitespace', () => {
    const addr = '  11111111111111111111111111111111  ';
    assert.equal(validateAddress(addr, 'solana'), true);
  });
});

// ---------------------------------------------------------------------------
// validatePaymentParams
// ---------------------------------------------------------------------------

describe('validatePaymentParams', () => {
  const validSolanaAddr = '11111111111111111111111111111111';

  const validParams = {
    agentId: 'agent-1',
    chainId: 'solana',
    toAddress: validSolanaAddr,
    amount: 10,
    tokenSymbol: 'USDC',
  };

  it('returns normalized params on success', () => {
    const result = validatePaymentParams(validParams);
    assert.equal(result.agentId, 'agent-1');
    assert.equal(result.chainId, 'solana');
    assert.equal(result.toAddress, validSolanaAddr);
    assert.equal(result.amount, 10);
    assert.equal(result.tokenSymbol, 'USDC');
    assert.deepEqual(result.metadata, {});
  });

  it('trims and normalizes fields', () => {
    // chainId and tokenSymbol are validated before trimming, so they must be clean
    const result = validatePaymentParams({
      ...validParams,
      agentId: '  agent-1  ',
      toAddress: `  ${validSolanaAddr}  `,
    });
    assert.equal(result.agentId, 'agent-1');
    assert.equal(result.chainId, 'solana');
    assert.equal(result.toAddress, validSolanaAddr);
    assert.equal(result.tokenSymbol, 'USDC');
  });

  it('throws for missing agentId', () => {
    assert.throws(
      () => validatePaymentParams({ ...validParams, agentId: '' }),
      (e) => e.code === ValidationErrorCodes.INVALID_AGENT_ID,
    );
  });

  it('throws for invalid chain', () => {
    assert.throws(
      () => validatePaymentParams({ ...validParams, chainId: 'nope' }),
      (e) => e.code === ValidationErrorCodes.INVALID_CHAIN,
    );
  });

  it('throws for invalid amount', () => {
    assert.throws(
      () => validatePaymentParams({ ...validParams, amount: -1 }),
      (e) => e instanceof ValidationError,
    );
  });

  it('throws for invalid address', () => {
    assert.throws(
      () => validatePaymentParams({ ...validParams, toAddress: 'bad' }),
      (e) => e instanceof ValidationError,
    );
  });

  it('detects self-transfer', () => {
    assert.throws(
      () =>
        validatePaymentParams({
          ...validParams,
          fromAddress: validSolanaAddr,
          toAddress: validSolanaAddr,
        }),
      (e) => {
        // The errors array should include a SELF_TRANSFER
        const selfErr = e.context.errors.find(
          (x) => x.code === ValidationErrorCodes.SELF_TRANSFER,
        );
        assert.ok(selfErr);
        return true;
      },
    );
  });

  it('throws INVALID_METADATA for non-object metadata', () => {
    assert.throws(
      () => validatePaymentParams({ ...validParams, metadata: 'string' }),
      (e) => {
        const metaErr = e.context.errors.find(
          (x) => x.code === ValidationErrorCodes.INVALID_METADATA,
        );
        assert.ok(metaErr);
        return true;
      },
    );
  });

  it('throws INVALID_METADATA for array metadata', () => {
    assert.throws(
      () => validatePaymentParams({ ...validParams, metadata: [1, 2] }),
      (e) => e instanceof ValidationError,
    );
  });

  it('accepts object metadata', () => {
    const result = validatePaymentParams({
      ...validParams,
      metadata: { orderId: 'ORD-1' },
    });
    assert.deepEqual(result.metadata, { orderId: 'ORD-1' });
  });

  it('accepts null/undefined metadata', () => {
    const r1 = validatePaymentParams({ ...validParams, metadata: null });
    assert.deepEqual(r1.metadata, {});
    const r2 = validatePaymentParams({ ...validParams, metadata: undefined });
    assert.deepEqual(r2.metadata, {});
  });

  it('collects multiple errors', () => {
    assert.throws(
      () =>
        validatePaymentParams({
          agentId: '',
          chainId: 'badchain',
          toAddress: '',
          amount: -1,
        }),
      (e) => {
        assert.ok(e.context.errors.length >= 2);
        assert.ok(e.message.includes('Multiple validation errors'));
        return true;
      },
    );
  });

  it('parses string amounts', () => {
    const result = validatePaymentParams({ ...validParams, amount: '25.5' });
    assert.equal(result.amount, 25.5);
  });
});

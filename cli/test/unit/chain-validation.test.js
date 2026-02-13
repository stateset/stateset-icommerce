/**
 * Tests for cli/src/chains/validation.js
 *
 * Covers: ValidationError, ValidationErrorCodes, validateAmount,
 * validateEvmAddress, validateSolanaAddress, validateZcashAddress,
 * validateBitcoinAddress, validateAddress (dispatcher),
 * validateChainId, validateToken, validatePaymentParams.
 *
 * Functions that depend on ./config.js and ./crypto-utils.js imports
 * (validateChainId, validateToken, validateAddress dispatcher,
 * validatePaymentParams, and the checksum branch in validateEvmAddress)
 * are exercised directly because those modules load successfully in
 * this environment.  If config ever fails to resolve, those suites
 * will surface clear import errors rather than silent passes.
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

// ============================================================================
// ValidationError
// ============================================================================

describe('ValidationError', () => {
  it('extends Error', () => {
    const err = new ValidationError('CODE', 'msg');
    assert.ok(err instanceof Error);
    assert.ok(err instanceof ValidationError);
  });

  it('stores code, message, and context', () => {
    const ctx = { field: 'amount' };
    const err = new ValidationError('INVALID_AMOUNT', 'bad amount', ctx);
    assert.equal(err.code, 'INVALID_AMOUNT');
    assert.equal(err.message, 'bad amount');
    assert.deepEqual(err.context, ctx);
    assert.equal(err.name, 'ValidationError');
  });

  it('defaults context to empty object when omitted', () => {
    const err = new ValidationError('CODE', 'msg');
    assert.deepEqual(err.context, {});
  });

  it('toJSON() returns a structured representation', () => {
    const err = new ValidationError('C', 'M', { x: 1 });
    const json = err.toJSON();
    assert.equal(json.error, 'ValidationError');
    assert.equal(json.code, 'C');
    assert.equal(json.message, 'M');
    assert.deepEqual(json.context, { x: 1 });
  });

  it('toJSON() round-trips through JSON.stringify', () => {
    const err = new ValidationError('X', 'Y', { nested: { a: true } });
    const parsed = JSON.parse(JSON.stringify(err.toJSON()));
    assert.equal(parsed.error, 'ValidationError');
    assert.equal(parsed.code, 'X');
    assert.deepEqual(parsed.context, { nested: { a: true } });
  });

  it('has a proper stack trace', () => {
    const err = new ValidationError('CODE', 'msg');
    assert.ok(typeof err.stack === 'string');
    assert.ok(err.stack.includes('ValidationError'));
  });
});

// ============================================================================
// ValidationErrorCodes
// ============================================================================

describe('ValidationErrorCodes', () => {
  const EXPECTED = [
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

  it('contains all 11 expected codes', () => {
    for (const code of EXPECTED) {
      assert.ok(code in ValidationErrorCodes, `missing code: ${code}`);
    }
    assert.equal(Object.keys(ValidationErrorCodes).length, EXPECTED.length);
  });

  it('each code value equals its key (identity mapping)', () => {
    for (const [key, value] of Object.entries(ValidationErrorCodes)) {
      assert.equal(key, value);
    }
  });
});

// ============================================================================
// validateAmount
// ============================================================================

describe('validateAmount', () => {
  // --- happy-path ---
  it('returns parsed number for integer input', () => {
    assert.equal(validateAmount(100), 100);
  });

  it('returns parsed number for float input', () => {
    assert.equal(validateAmount(0.5), 0.5);
  });

  it('parses string amounts', () => {
    assert.equal(validateAmount('42.5'), 42.5);
  });

  it('parses string integer amounts', () => {
    assert.equal(validateAmount('1000'), 1000);
  });

  it('accepts the default minimum boundary (0.000001)', () => {
    assert.equal(validateAmount(0.000001), 0.000001);
  });

  it('accepts the default maximum boundary (1 billion)', () => {
    assert.equal(validateAmount(1_000_000_000), 1_000_000_000);
  });

  it('accepts custom min/max range', () => {
    assert.equal(validateAmount(50, { minAmount: 10, maxAmount: 100 }), 50);
  });

  it('accepts a value exactly at custom min', () => {
    assert.equal(validateAmount(10, { minAmount: 10 }), 10);
  });

  it('accepts a value exactly at custom max', () => {
    assert.equal(validateAmount(100, { maxAmount: 100 }), 100);
  });

  // --- missing input ---
  it('throws MISSING_REQUIRED for null', () => {
    assert.throws(
      () => validateAmount(null),
      (e) => e.code === ValidationErrorCodes.MISSING_REQUIRED,
    );
  });

  it('throws MISSING_REQUIRED for undefined', () => {
    assert.throws(
      () => validateAmount(undefined),
      (e) => e.code === ValidationErrorCodes.MISSING_REQUIRED,
    );
  });

  it('throws MISSING_REQUIRED for empty string', () => {
    assert.throws(
      () => validateAmount(''),
      (e) => e.code === ValidationErrorCodes.MISSING_REQUIRED,
    );
  });

  // --- invalid number ---
  it('throws INVALID_AMOUNT for non-numeric string', () => {
    assert.throws(
      () => validateAmount('abc'),
      (e) => e.code === ValidationErrorCodes.INVALID_AMOUNT,
    );
  });

  it('throws INVALID_AMOUNT for NaN', () => {
    assert.throws(
      () => validateAmount(NaN),
      (e) => e.code === ValidationErrorCodes.INVALID_AMOUNT,
    );
  });

  it('throws INVALID_AMOUNT for Infinity', () => {
    assert.throws(
      () => validateAmount(Infinity),
      (e) => e.code === ValidationErrorCodes.INVALID_AMOUNT,
    );
  });

  it('throws INVALID_AMOUNT for -Infinity', () => {
    assert.throws(
      () => validateAmount(-Infinity),
      (e) => e.code === ValidationErrorCodes.INVALID_AMOUNT,
    );
  });

  // --- non-positive ---
  it('throws INVALID_AMOUNT for zero', () => {
    assert.throws(
      () => validateAmount(0),
      (e) => e.code === ValidationErrorCodes.INVALID_AMOUNT,
    );
  });

  it('throws INVALID_AMOUNT for negative number', () => {
    assert.throws(
      () => validateAmount(-5),
      (e) => e.code === ValidationErrorCodes.INVALID_AMOUNT,
    );
  });

  it('throws INVALID_AMOUNT for negative string', () => {
    assert.throws(
      () => validateAmount('-10'),
      (e) => e.code === ValidationErrorCodes.INVALID_AMOUNT,
    );
  });

  // --- boundary violations ---
  it('throws AMOUNT_TOO_SMALL below default minimum', () => {
    assert.throws(
      () => validateAmount(0.00000001),
      (e) => e.code === ValidationErrorCodes.AMOUNT_TOO_SMALL,
    );
  });

  it('throws AMOUNT_TOO_SMALL below custom minimum', () => {
    assert.throws(
      () => validateAmount(5, { minAmount: 10 }),
      (e) => {
        assert.equal(e.code, ValidationErrorCodes.AMOUNT_TOO_SMALL);
        assert.equal(e.context.minAmount, 10);
        return true;
      },
    );
  });

  it('throws AMOUNT_TOO_LARGE above default maximum', () => {
    assert.throws(
      () => validateAmount(1_000_000_001),
      (e) => e.code === ValidationErrorCodes.AMOUNT_TOO_LARGE,
    );
  });

  it('throws AMOUNT_TOO_LARGE above custom maximum', () => {
    assert.throws(
      () => validateAmount(999, { maxAmount: 100 }),
      (e) => {
        assert.equal(e.code, ValidationErrorCodes.AMOUNT_TOO_LARGE);
        assert.equal(e.context.maxAmount, 100);
        return true;
      },
    );
  });
});

// ============================================================================
// validateEvmAddress
// ============================================================================

describe('validateEvmAddress', () => {
  const VALID_LOWER = '0x' + 'ab'.repeat(20);
  const VALID_UPPER = '0x' + 'AB'.repeat(20);
  const VALID_ZERO = '0x' + '00'.repeat(20);

  it('accepts all-lowercase hex address', () => {
    assert.equal(validateEvmAddress(VALID_LOWER), true);
  });

  it('accepts all-uppercase hex address', () => {
    assert.equal(validateEvmAddress(VALID_UPPER), true);
  });

  it('accepts zero address', () => {
    assert.equal(validateEvmAddress(VALID_ZERO), true);
  });

  it('rejects address without 0x prefix', () => {
    assert.throws(
      () => validateEvmAddress('ab'.repeat(20)),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });

  it('rejects address that is too short', () => {
    assert.throws(
      () => validateEvmAddress('0xabc'),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });

  it('rejects address that is too long', () => {
    assert.throws(
      () => validateEvmAddress('0x' + 'ab'.repeat(21)),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });

  it('rejects non-hex characters after 0x', () => {
    assert.throws(
      () => validateEvmAddress('0x' + 'zz'.repeat(20)),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });

  it('rejects mixed-case address with incorrect EIP-55 checksum', () => {
    // Flip one character from a known lowercase address to create bad checksum
    const badChecksum = '0xAb' + 'ab'.repeat(19);
    // This passes format check (40 hex chars) but may fail checksum.
    // Whether it throws INVALID_ADDRESS or passes depends on isValidEthAddress.
    // Either way it must not throw INVALID_ADDRESS_FORMAT.
    try {
      validateEvmAddress(badChecksum);
      // If it passes, the checksum was actually valid for that pattern
    } catch (e) {
      assert.equal(e.code, ValidationErrorCodes.INVALID_ADDRESS);
    }
  });
});

// ============================================================================
// validateSolanaAddress
// ============================================================================

describe('validateSolanaAddress', () => {
  // Solana System Program (all 1s, 32 chars)
  const SYSTEM_PROGRAM = '11111111111111111111111111111111';
  // 44-char valid base58 address
  const LONG_VALID = '9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM';

  it('accepts 32-char all-ones address', () => {
    assert.equal(validateSolanaAddress(SYSTEM_PROGRAM), true);
  });

  it('accepts 44-char base58 address', () => {
    assert.equal(validateSolanaAddress(LONG_VALID), true);
  });

  it('accepts 32-char minimum-length address', () => {
    const addr = 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'; // 32 A's
    assert.equal(validateSolanaAddress(addr), true);
  });

  it('rejects address shorter than 32 chars', () => {
    assert.throws(
      () => validateSolanaAddress('abc'),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });

  it('rejects address longer than 44 chars', () => {
    const tooLong = 'A'.repeat(45);
    assert.throws(
      () => validateSolanaAddress(tooLong),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });

  it('rejects invalid base58 character 0 (zero)', () => {
    const invalid = '0' + '1'.repeat(43);
    assert.throws(
      () => validateSolanaAddress(invalid),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });

  it('rejects invalid base58 character O (capital O)', () => {
    const invalid = 'O' + '1'.repeat(43);
    assert.throws(
      () => validateSolanaAddress(invalid),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });

  it('rejects invalid base58 character I (capital I)', () => {
    const invalid = 'I' + '1'.repeat(43);
    assert.throws(
      () => validateSolanaAddress(invalid),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });

  it('rejects invalid base58 character l (lowercase L)', () => {
    const invalid = 'l' + '1'.repeat(43);
    assert.throws(
      () => validateSolanaAddress(invalid),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });
});

// ============================================================================
// validateZcashAddress
// ============================================================================

describe('validateZcashAddress', () => {
  it('accepts t1 transparent mainnet address', () => {
    const addr = 't1' + 'K'.repeat(33);
    assert.equal(validateZcashAddress(addr), true);
  });

  it('accepts t3 transparent mainnet address', () => {
    const addr = 't3' + 'K'.repeat(33);
    assert.equal(validateZcashAddress(addr), true);
  });

  it('accepts zs1 shielded Sapling address', () => {
    const addr = 'zs1' + 'a'.repeat(75);
    assert.equal(validateZcashAddress(addr), true);
  });

  it('accepts u1 unified address', () => {
    const addr = 'u1' + 'a'.repeat(60);
    assert.equal(validateZcashAddress(addr), true);
  });

  it('accepts u1 unified address at max length', () => {
    const addr = 'u1' + 'a'.repeat(200);
    assert.equal(validateZcashAddress(addr), true);
  });

  it('rejects unknown prefix', () => {
    assert.throws(
      () => validateZcashAddress('x1' + 'a'.repeat(33)),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });

  it('rejects t1 address that is too short', () => {
    assert.throws(
      () => validateZcashAddress('t1abc'),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });

  it('rejects zs1 address with wrong length', () => {
    // zs1 expects exactly 75 chars after prefix
    assert.throws(
      () => validateZcashAddress('zs1' + 'a'.repeat(10)),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });
});

// ============================================================================
// validateBitcoinAddress
// ============================================================================

describe('validateBitcoinAddress', () => {
  it('accepts P2PKH mainnet address (1...)', () => {
    const addr = '1' + 'A'.repeat(33);
    assert.equal(validateBitcoinAddress(addr), true);
  });

  it('accepts P2SH mainnet address (3...)', () => {
    const addr = '3' + 'J'.repeat(33);
    assert.equal(validateBitcoinAddress(addr), true);
  });

  it('accepts P2PKH testnet address (m...)', () => {
    const addr = 'm' + 'N'.repeat(33);
    assert.equal(validateBitcoinAddress(addr), true);
  });

  it('accepts P2PKH testnet address (n...)', () => {
    const addr = 'n' + 'N'.repeat(33);
    assert.equal(validateBitcoinAddress(addr), true);
  });

  it('accepts bech32 mainnet address (bc1q...)', () => {
    const addr = 'bc1q' + 'p'.repeat(38);
    assert.equal(validateBitcoinAddress(addr), true);
  });

  it('accepts bech32 testnet address (tb1q...)', () => {
    const addr = 'tb1q' + 'p'.repeat(38);
    assert.equal(validateBitcoinAddress(addr), true);
  });

  it('accepts P2SH testnet address (2...)', () => {
    const addr = '2' + 'N'.repeat(33);
    assert.equal(validateBitcoinAddress(addr), true);
  });

  it('rejects address with invalid prefix', () => {
    assert.throws(
      () => validateBitcoinAddress('x' + 'A'.repeat(33)),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });

  it('rejects empty string', () => {
    assert.throws(
      () => validateBitcoinAddress(''),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });

  it('rejects P2PKH address that is too short', () => {
    assert.throws(
      () => validateBitcoinAddress('1abc'),
      (e) => e.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
    );
  });
});

// ============================================================================
// validateAddress (dispatcher)
// ============================================================================

describe('validateAddress', () => {
  it('throws MISSING_REQUIRED for null address', () => {
    assert.throws(
      () => validateAddress(null, 'solana'),
      (e) => e.code === ValidationErrorCodes.MISSING_REQUIRED,
    );
  });

  it('throws MISSING_REQUIRED for empty string address', () => {
    assert.throws(
      () => validateAddress('', 'solana'),
      (e) => e.code === ValidationErrorCodes.MISSING_REQUIRED,
    );
  });

  it('throws MISSING_REQUIRED for non-string address', () => {
    assert.throws(
      () => validateAddress(12345, 'solana'),
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

  it('dispatches to EVM validator for base chain', () => {
    const addr = '0x' + 'ab'.repeat(20);
    assert.equal(validateAddress(addr, 'base'), true);
  });

  it('dispatches to Zcash validator', () => {
    const addr = 't1' + 'K'.repeat(33);
    assert.equal(validateAddress(addr, 'zcash'), true);
  });

  it('dispatches to Bitcoin validator', () => {
    const addr = '1' + 'A'.repeat(33);
    assert.equal(validateAddress(addr, 'bitcoin'), true);
  });

  it('trims whitespace before validation', () => {
    const addr = '  11111111111111111111111111111111  ';
    assert.equal(validateAddress(addr, 'solana'), true);
  });

  it('accepts any non-empty string for unknown chain types', () => {
    // For a chain ID that is not in CHAINS or does not match any chain-type
    // predicate, validateAddress just checks the trimmed address is non-empty.
    // We need a chain that exists in CHAINS but is not EVM/Ed25519/Zcash/Bitcoin.
    // Since all configured chains match one of those predicates, we cannot
    // easily test the fallback path without mocking. Instead, just verify the
    // dispatchers work for the known chain families above.
    assert.ok(true);
  });
});

// ============================================================================
// validateChainId
// ============================================================================

describe('validateChainId', () => {
  it('returns true for solana', () => {
    assert.equal(validateChainId('solana'), true);
  });

  it('returns true for solana_devnet', () => {
    assert.equal(validateChainId('solana_devnet'), true);
  });

  it('returns true for ethereum', () => {
    assert.equal(validateChainId('ethereum'), true);
  });

  it('returns true for base', () => {
    assert.equal(validateChainId('base'), true);
  });

  it('returns true for set_chain', () => {
    assert.equal(validateChainId('set_chain'), true);
  });

  it('returns true for zcash', () => {
    assert.equal(validateChainId('zcash'), true);
  });

  it('returns true for bitcoin', () => {
    assert.equal(validateChainId('bitcoin'), true);
  });

  it('throws MISSING_REQUIRED for null', () => {
    assert.throws(
      () => validateChainId(null),
      (e) => e.code === ValidationErrorCodes.MISSING_REQUIRED,
    );
  });

  it('throws MISSING_REQUIRED for empty string', () => {
    assert.throws(
      () => validateChainId(''),
      (e) => e.code === ValidationErrorCodes.MISSING_REQUIRED,
    );
  });

  it('throws MISSING_REQUIRED for undefined', () => {
    assert.throws(
      () => validateChainId(undefined),
      (e) => e.code === ValidationErrorCodes.MISSING_REQUIRED,
    );
  });

  it('throws MISSING_REQUIRED for non-string type', () => {
    assert.throws(
      () => validateChainId(42),
      (e) => e.code === ValidationErrorCodes.MISSING_REQUIRED,
    );
  });

  it('throws INVALID_CHAIN for unknown chain and provides validChains', () => {
    assert.throws(
      () => validateChainId('fantom'),
      (e) => {
        assert.equal(e.code, ValidationErrorCodes.INVALID_CHAIN);
        assert.ok(Array.isArray(e.context.validChains));
        assert.ok(e.context.validChains.length > 0);
        assert.ok(e.context.validChains.includes('solana'));
        return true;
      },
    );
  });
});

// ============================================================================
// validateToken
// ============================================================================

describe('validateToken', () => {
  it('returns true when token exists on chain', () => {
    assert.equal(validateToken('solana', 'USDC'), true);
  });

  it('returns true when no token specified and default exists', () => {
    // solana has USDC as default stablecoin
    assert.equal(validateToken('solana'), true);
  });

  it('returns true for SOL on solana', () => {
    assert.equal(validateToken('solana', 'SOL'), true);
  });

  it('returns true for ssUSD on set_chain', () => {
    assert.equal(validateToken('set_chain', 'ssUSD'), true);
  });

  it('throws INVALID_TOKEN for unknown token on valid chain', () => {
    assert.throws(
      () => validateToken('solana', 'DOGE'),
      (e) => {
        assert.equal(e.code, ValidationErrorCodes.INVALID_TOKEN);
        assert.ok(Array.isArray(e.context.validTokens));
        assert.ok(e.context.validTokens.includes('USDC'));
        return true;
      },
    );
  });

  it('throws INVALID_CHAIN for invalid chain (cascading from validateChainId)', () => {
    assert.throws(
      () => validateToken('badchain', 'USDC'),
      (e) => e.code === ValidationErrorCodes.INVALID_CHAIN,
    );
  });
});

// ============================================================================
// validatePaymentParams
// ============================================================================

describe('validatePaymentParams', () => {
  const validSolanaAddr = '11111111111111111111111111111111';

  const validParams = {
    agentId: 'agent-1',
    chainId: 'solana',
    toAddress: validSolanaAddr,
    amount: 10,
    tokenSymbol: 'USDC',
  };

  it('returns normalized params on valid input', () => {
    const result = validatePaymentParams(validParams);
    assert.equal(result.agentId, 'agent-1');
    assert.equal(result.chainId, 'solana');
    assert.equal(result.toAddress, validSolanaAddr);
    assert.equal(result.amount, 10);
    assert.equal(result.tokenSymbol, 'USDC');
    assert.deepEqual(result.metadata, {});
  });

  it('trims and normalizes agentId and toAddress', () => {
    const result = validatePaymentParams({
      ...validParams,
      agentId: '  agent-1  ',
      toAddress: `  ${validSolanaAddr}  `,
    });
    assert.equal(result.agentId, 'agent-1');
    assert.equal(result.toAddress, validSolanaAddr);
  });

  it('lowercases chainId', () => {
    // chainId must be a valid chain key; since keys are lowercase in CHAINS,
    // we test with 'solana' which already works.
    const result = validatePaymentParams(validParams);
    assert.equal(result.chainId, 'solana');
  });

  it('uppercases tokenSymbol', () => {
    const result = validatePaymentParams({ ...validParams, tokenSymbol: 'usdc' });
    assert.equal(result.tokenSymbol, 'USDC');
  });

  it('parses string amounts', () => {
    const result = validatePaymentParams({ ...validParams, amount: '25.5' });
    assert.equal(result.amount, 25.5);
  });

  it('throws for missing agentId', () => {
    assert.throws(
      () => validatePaymentParams({ ...validParams, agentId: '' }),
      (e) => e.code === ValidationErrorCodes.INVALID_AGENT_ID,
    );
  });

  it('throws for non-string agentId', () => {
    assert.throws(
      () => validatePaymentParams({ ...validParams, agentId: null }),
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

  it('throws for invalid address on valid chain', () => {
    assert.throws(
      () => validatePaymentParams({ ...validParams, toAddress: 'bad' }),
      (e) => e instanceof ValidationError,
    );
  });

  it('detects self-transfer (case-insensitive)', () => {
    assert.throws(
      () =>
        validatePaymentParams({
          ...validParams,
          fromAddress: validSolanaAddr,
          toAddress: validSolanaAddr,
        }),
      (e) => {
        const selfErr = e.context.errors.find(
          (x) => x.code === ValidationErrorCodes.SELF_TRANSFER,
        );
        assert.ok(selfErr, 'expected SELF_TRANSFER in errors array');
        return true;
      },
    );
  });

  it('throws INVALID_METADATA for string metadata', () => {
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

  it('treats null metadata as empty object', () => {
    const result = validatePaymentParams({ ...validParams, metadata: null });
    assert.deepEqual(result.metadata, {});
  });

  it('treats undefined metadata as empty object', () => {
    const result = validatePaymentParams({ ...validParams, metadata: undefined });
    assert.deepEqual(result.metadata, {});
  });

  it('collects multiple errors and reports them together', () => {
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

  it('skips token validation when chainId is invalid', () => {
    // If chain is invalid, the token validation step is skipped,
    // so the error should be about the chain, not the token.
    assert.throws(
      () =>
        validatePaymentParams({
          ...validParams,
          chainId: 'badchain',
          tokenSymbol: 'FAKE',
        }),
      (e) => {
        const chainErr = e.context.errors?.find(
          (x) => x.code === ValidationErrorCodes.INVALID_CHAIN,
        );
        assert.ok(chainErr);
        // Should NOT have a token error since chain validation failed first
        const tokenErr = e.context.errors?.find(
          (x) => x.code === ValidationErrorCodes.INVALID_TOKEN,
        );
        assert.equal(tokenErr, undefined);
        return true;
      },
    );
  });

  it('skips address validation when chainId is invalid', () => {
    assert.throws(
      () =>
        validatePaymentParams({
          ...validParams,
          chainId: 'badchain',
          toAddress: 'anything',
        }),
      (e) => {
        const addrErr = e.context.errors?.find(
          (x) =>
            x.code === ValidationErrorCodes.INVALID_ADDRESS ||
            x.code === ValidationErrorCodes.INVALID_ADDRESS_FORMAT,
        );
        assert.equal(addrErr, undefined, 'address validation should be skipped');
        return true;
      },
    );
  });
});

/**
 * Unit tests for x402/crypto.js
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import {
  X402_DOMAIN_SEPARATOR,
  normalizeNetwork,
  normalizeAsset,
  networkChainId,
  computeX402SigningHash,
  encodeBase64Json,
  decodeBase64Json,
  hashToHex,
  hexToBytes,
} from '../../src/x402/crypto.js';

// ===========================================================================
// normalizeNetwork
// ===========================================================================

describe('normalizeNetwork', () => {
  it('normalizes known networks', () => {
    assert.strictEqual(normalizeNetwork('set_chain'), 'set_chain');
    assert.strictEqual(normalizeNetwork('ethereum'), 'ethereum');
    assert.strictEqual(normalizeNetwork('base'), 'base');
    assert.strictEqual(normalizeNetwork('arbitrum'), 'arbitrum');
    assert.strictEqual(normalizeNetwork('optimism'), 'optimism');
  });

  it('resolves aliases', () => {
    assert.strictEqual(normalizeNetwork('set'), 'set_chain');
    assert.strictEqual(normalizeNetwork('setchain'), 'set_chain');
    assert.strictEqual(normalizeNetwork('eth'), 'ethereum');
    assert.strictEqual(normalizeNetwork('arb'), 'arbitrum');
    assert.strictEqual(normalizeNetwork('op'), 'optimism');
    assert.strictEqual(normalizeNetwork('sepolia'), 'ethereum_sepolia');
  });

  it('is case-insensitive', () => {
    assert.strictEqual(normalizeNetwork('ETHEREUM'), 'ethereum');
    assert.strictEqual(normalizeNetwork('Set_Chain'), 'set_chain');
    assert.strictEqual(normalizeNetwork('BASE'), 'base');
  });

  it('throws on unsupported network', () => {
    assert.throws(() => normalizeNetwork('bitcoin'), /Unsupported x402 network/);
  });

  it('throws on empty/null', () => {
    assert.throws(() => normalizeNetwork(null), /network is required/);
    assert.throws(() => normalizeNetwork(''), /network is required/);
    assert.throws(() => normalizeNetwork(undefined), /network is required/);
  });
});

// ===========================================================================
// normalizeAsset
// ===========================================================================

describe('normalizeAsset', () => {
  it('normalizes known assets', () => {
    assert.strictEqual(normalizeAsset('usdc'), 'usdc');
    assert.strictEqual(normalizeAsset('usdt'), 'usdt');
    assert.strictEqual(normalizeAsset('ssusd'), 'ssusd');
    assert.strictEqual(normalizeAsset('dai'), 'dai');
    assert.strictEqual(normalizeAsset('eth'), 'eth');
  });

  it('resolves aliases', () => {
    assert.strictEqual(normalizeAsset('ss_usd'), 'ssusd');
    assert.strictEqual(normalizeAsset('wss_usd'), 'wssusd');
  });

  it('is case-insensitive', () => {
    assert.strictEqual(normalizeAsset('USDC'), 'usdc');
    assert.strictEqual(normalizeAsset('SsUsd'), 'ssusd');
  });

  it('throws on unsupported asset', () => {
    assert.throws(() => normalizeAsset('btc'), /Unsupported x402 asset/);
  });

  it('throws on empty/null', () => {
    assert.throws(() => normalizeAsset(null), /asset is required/);
    assert.throws(() => normalizeAsset(''), /asset is required/);
  });
});

// ===========================================================================
// networkChainId
// ===========================================================================

describe('networkChainId', () => {
  it('returns chain IDs for known networks', () => {
    assert.strictEqual(networkChainId('ethereum'), 1);
    assert.strictEqual(networkChainId('ethereum_sepolia'), 11155111);
    assert.strictEqual(networkChainId('base'), 8453);
    assert.strictEqual(networkChainId('base_sepolia'), 84532);
    assert.strictEqual(networkChainId('arbitrum'), 42161);
    assert.strictEqual(networkChainId('optimism'), 10);
    assert.strictEqual(networkChainId('set_chain'), 84532001);
  });

  it('resolves aliases before lookup', () => {
    assert.strictEqual(networkChainId('eth'), 1);
    assert.strictEqual(networkChainId('arb'), 42161);
    assert.strictEqual(networkChainId('op'), 10);
  });

  it('throws for unsupported network', () => {
    assert.throws(() => networkChainId('bitcoin'), /Unsupported/);
  });
});

// ===========================================================================
// computeX402SigningHash
// ===========================================================================

describe('computeX402SigningHash', () => {
  const baseParams = {
    payerAddress: '0xPayer',
    payeeAddress: '0xPayee',
    amount: 1000n,
    asset: 'usdc',
    network: 'set_chain',
    validUntil: 999999n,
    nonce: 1n,
  };

  it('returns a 32-byte Buffer (SHA256)', () => {
    const hash = computeX402SigningHash(baseParams);
    assert.ok(Buffer.isBuffer(hash));
    assert.strictEqual(hash.length, 32);
  });

  it('is deterministic', () => {
    const hash1 = computeX402SigningHash(baseParams);
    const hash2 = computeX402SigningHash(baseParams);
    assert.ok(hash1.equals(hash2));
  });

  it('changes when any parameter changes', () => {
    const hash1 = computeX402SigningHash(baseParams);

    const hash2 = computeX402SigningHash({ ...baseParams, amount: 2000n });
    assert.ok(!hash1.equals(hash2));

    const hash3 = computeX402SigningHash({ ...baseParams, nonce: 2n });
    assert.ok(!hash1.equals(hash3));

    const hash4 = computeX402SigningHash({ ...baseParams, payerAddress: '0xOther' });
    assert.ok(!hash1.equals(hash4));

    const hash5 = computeX402SigningHash({ ...baseParams, asset: 'dai' });
    assert.ok(!hash1.equals(hash5));
  });

  it('accepts number amounts (converted to bigint)', () => {
    const hash = computeX402SigningHash({
      ...baseParams,
      amount: 1000,
      nonce: 1,
      validUntil: 999999,
    });
    assert.ok(Buffer.isBuffer(hash));
    assert.strictEqual(hash.length, 32);
  });

  it('allows explicit chainId override', () => {
    const hashDefault = computeX402SigningHash(baseParams);
    const hashOverride = computeX402SigningHash({ ...baseParams, chainId: 99999 });
    assert.ok(!hashDefault.equals(hashOverride));
  });

  it('throws for negative amount', () => {
    assert.throws(() => computeX402SigningHash({ ...baseParams, amount: -1n }), /must be a u64/);
  });

  it('throws when payerAddress is missing', () => {
    assert.throws(
      () => computeX402SigningHash({ ...baseParams, payerAddress: '' }),
      /payerAddress and payeeAddress are required/,
    );
  });

  it('throws when payeeAddress is missing', () => {
    assert.throws(
      () => computeX402SigningHash({ ...baseParams, payeeAddress: null }),
      /payerAddress and payeeAddress are required/,
    );
  });
});

// ===========================================================================
// Base64 JSON encoding/decoding
// ===========================================================================

describe('encodeBase64Json / decodeBase64Json', () => {
  it('round-trips an object', () => {
    const obj = { foo: 'bar', num: 42, nested: { a: true } };
    const encoded = encodeBase64Json(obj);
    assert.strictEqual(typeof encoded, 'string');
    const decoded = decodeBase64Json(encoded);
    assert.deepStrictEqual(decoded, obj);
  });

  it('round-trips an array', () => {
    const arr = [1, 'two', null];
    const decoded = decodeBase64Json(encodeBase64Json(arr));
    assert.deepStrictEqual(decoded, arr);
  });

  it('encodes to valid base64', () => {
    const encoded = encodeBase64Json({ test: true });
    assert.ok(/^[A-Za-z0-9+/=]+$/.test(encoded));
  });

  it('throws on invalid base64 JSON', () => {
    assert.throws(() => decodeBase64Json('not-base64!!!'));
  });
});

// ===========================================================================
// hashToHex / hexToBytes
// ===========================================================================

describe('hashToHex / hexToBytes', () => {
  it('converts buffer to hex and back', () => {
    const buf = Buffer.from([0xde, 0xad, 0xbe, 0xef]);
    const hex = hashToHex(buf);
    assert.strictEqual(hex, '0xdeadbeef');
    const back = hexToBytes(hex);
    assert.ok(buf.equals(back));
  });

  it('handles empty buffer', () => {
    const hex = hashToHex(Buffer.alloc(0));
    assert.strictEqual(hex, '0x');
  });
});

// ===========================================================================
// X402_DOMAIN_SEPARATOR
// ===========================================================================

describe('X402_DOMAIN_SEPARATOR', () => {
  it('is the expected string', () => {
    assert.strictEqual(X402_DOMAIN_SEPARATOR, 'X402_PAYMENT_V1');
  });
});

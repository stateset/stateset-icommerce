/**
 * Tests for chains/config.js — chain configuration, token lookup, and unit conversion.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  CHAINS,
  getChain,
  getToken,
  getDefaultStablecoin,
  getExplorerTxUrl,
  getExplorerAddressUrl,
  toSmallestUnit,
  fromSmallestUnit,
  formatAmount,
  isEd25519Chain,
  isEvmChain,
  isZcashChain,
  isBitcoinChain,
  listChains,
  getRecommendedChain,
} from '../../src/chains/config.js';

// =============================================================================
// CHAINS object
// =============================================================================

describe('CHAINS', () => {
  it('contains all expected chains', () => {
    const expected = [
      'solana', 'solana_devnet', 'set_chain', 'set_chain_testnet',
      'base', 'ethereum', 'arbitrum', 'arc', 'arc_testnet',
      'zcash', 'zcash_testnet', 'bitcoin', 'bitcoin_testnet',
    ];
    for (const chain of expected) {
      assert.ok(CHAINS[chain], `Expected chain ${chain} to exist`);
    }
  });

  it('each chain has required fields', () => {
    for (const [id, chain] of Object.entries(CHAINS)) {
      assert.ok(chain.name, `${id} missing name`);
      assert.ok(chain.network, `${id} missing network`);
      assert.ok(chain.rpcUrl, `${id} missing rpcUrl`);
      assert.ok(chain.explorerUrl, `${id} missing explorerUrl`);
      assert.ok(typeof chain.confirmations === 'number', `${id} missing confirmations`);
      assert.ok(typeof chain.blockTimeMs === 'number', `${id} missing blockTimeMs`);
      assert.ok(chain.tokens && typeof chain.tokens === 'object', `${id} missing tokens`);
    }
  });

  it('each token has required fields', () => {
    for (const [chainId, chain] of Object.entries(CHAINS)) {
      for (const [sym, token] of Object.entries(chain.tokens)) {
        assert.ok(token.symbol, `${chainId}.${sym} missing symbol`);
        assert.ok(token.name, `${chainId}.${sym} missing name`);
        assert.ok(token.address, `${chainId}.${sym} missing address`);
        assert.ok(typeof token.decimals === 'number', `${chainId}.${sym} missing decimals`);
        assert.ok(token.type, `${chainId}.${sym} missing type`);
      }
    }
  });
});

// =============================================================================
// getChain
// =============================================================================

describe('getChain', () => {
  it('returns chain config for valid id', () => {
    const chain = getChain('solana');
    assert.equal(chain.name, 'Solana');
  });

  it('returns null for unknown id', () => {
    assert.equal(getChain('unknown'), null);
  });

  it('returns null for empty string', () => {
    assert.equal(getChain(''), null);
  });
});

// =============================================================================
// getToken
// =============================================================================

describe('getToken', () => {
  it('returns token for exact symbol match', () => {
    const token = getToken('solana', 'USDC');
    assert.equal(token.symbol, 'USDC');
    assert.equal(token.decimals, 6);
  });

  it('returns token for case-insensitive match', () => {
    const token = getToken('solana', 'usdc');
    assert.equal(token.symbol, 'USDC');
  });

  it('returns null for unknown token', () => {
    assert.equal(getToken('solana', 'UNKNOWN'), null);
  });

  it('returns null for unknown chain', () => {
    assert.equal(getToken('unknown', 'USDC'), null);
  });

  it('returns null for null tokenSymbol', () => {
    assert.equal(getToken('solana', null), null);
  });

  it('returns null for empty tokenSymbol', () => {
    assert.equal(getToken('solana', ''), null);
  });

  it('trims whitespace', () => {
    const token = getToken('solana', '  USDC  ');
    assert.equal(token.symbol, 'USDC');
  });

  it('finds ssUSD on set_chain', () => {
    const token = getToken('set_chain', 'ssUSD');
    assert.equal(token.symbol, 'ssUSD');
    assert.equal(token.isYieldBearing, true);
  });
});

// =============================================================================
// getDefaultStablecoin
// =============================================================================

describe('getDefaultStablecoin', () => {
  it('returns ssUSD for set_chain', () => {
    const token = getDefaultStablecoin('set_chain');
    assert.equal(token.symbol, 'ssUSD');
  });

  it('returns USDC for solana', () => {
    const token = getDefaultStablecoin('solana');
    assert.equal(token.symbol, 'USDC');
  });

  it('returns null for unknown chain', () => {
    assert.equal(getDefaultStablecoin('unknown'), null);
  });

  it('returns null for bitcoin (no stablecoin)', () => {
    assert.equal(getDefaultStablecoin('bitcoin'), null);
  });
});

// =============================================================================
// getExplorerTxUrl / getExplorerAddressUrl
// =============================================================================

describe('getExplorerTxUrl', () => {
  it('generates Solana tx URL', () => {
    const url = getExplorerTxUrl('solana', '5abc123');
    assert.ok(url.includes('explorer.solana.com/tx/5abc123'));
  });

  it('appends devnet suffix for solana_devnet', () => {
    const url = getExplorerTxUrl('solana_devnet', 'txhash');
    assert.ok(url.includes('?cluster=devnet'));
  });

  it('generates EVM tx URL', () => {
    const url = getExplorerTxUrl('base', '0xabc');
    assert.ok(url.includes('basescan.org/tx/0xabc'));
  });

  it('generates Zcash tx URL', () => {
    const url = getExplorerTxUrl('zcash', 'txid');
    assert.ok(url.includes('zcashblockexplorer.com/tx/txid'));
  });

  it('generates Bitcoin tx URL', () => {
    const url = getExplorerTxUrl('bitcoin', 'txid');
    assert.ok(url.includes('blockstream.info/tx/txid'));
  });

  it('returns empty string for unknown chain', () => {
    assert.equal(getExplorerTxUrl('unknown', 'hash'), '');
  });
});

describe('getExplorerAddressUrl', () => {
  it('generates Solana address URL', () => {
    const url = getExplorerAddressUrl('solana', 'addr123');
    assert.ok(url.includes('explorer.solana.com/address/addr123'));
  });

  it('returns empty string for unknown chain', () => {
    assert.equal(getExplorerAddressUrl('unknown', 'addr'), '');
  });
});

// =============================================================================
// toSmallestUnit / fromSmallestUnit
// =============================================================================

describe('toSmallestUnit', () => {
  it('converts 1.00 USDC (6 decimals) to 1000000', () => {
    assert.equal(toSmallestUnit('1.00', 6), 1_000_000n);
  });

  it('converts 100 (integer) to correct units', () => {
    assert.equal(toSmallestUnit(100, 6), 100_000_000n);
  });

  it('converts 0.000001 to 1', () => {
    assert.equal(toSmallestUnit('0.000001', 6), 1n);
  });

  it('handles 18 decimals (ETH)', () => {
    assert.equal(toSmallestUnit('1.0', 18), 1_000_000_000_000_000_000n);
  });

  it('handles 0 decimals', () => {
    assert.equal(toSmallestUnit('42', 0), 42n);
  });

  it('throws on negative amount', () => {
    assert.throws(() => toSmallestUnit('-1', 6), /non-negative/);
  });

  it('throws on too many decimal places', () => {
    assert.throws(() => toSmallestUnit('1.0000001', 6), /too many decimal/);
  });

  it('throws on invalid decimals', () => {
    assert.throws(() => toSmallestUnit('1', -1), /Invalid decimals/);
  });

  it('throws on invalid amount', () => {
    assert.throws(() => toSmallestUnit('abc', 6), /Invalid/);
  });

  it('handles scientific notation', () => {
    assert.equal(toSmallestUnit('1e2', 6), 100_000_000n);
  });
});

describe('fromSmallestUnit', () => {
  it('converts 1000000 lamports to 1.000000', () => {
    assert.equal(fromSmallestUnit(1_000_000n, 6), '1.000000');
  });

  it('converts 1 to 0.000001', () => {
    assert.equal(fromSmallestUnit(1n, 6), '0.000001');
  });

  it('handles 0 decimals', () => {
    assert.equal(fromSmallestUnit(42n, 0), '42');
  });

  it('handles numeric input', () => {
    assert.equal(fromSmallestUnit(1000000, 6), '1.000000');
  });

  it('handles string input', () => {
    assert.equal(fromSmallestUnit('1000000', 6), '1.000000');
  });
});

// =============================================================================
// formatAmount
// =============================================================================

describe('formatAmount', () => {
  it('formats amount with symbol', () => {
    assert.equal(formatAmount(10.5, 'USDC'), '10.50 USDC');
  });

  it('respects custom decimals', () => {
    assert.equal(formatAmount(10.123456, 'ETH', 4), '10.1235 ETH');
  });

  it('handles string amounts', () => {
    assert.equal(formatAmount('10.5', 'USDC'), '10.50 USDC');
  });
});

// =============================================================================
// Chain type checks
// =============================================================================

describe('isEd25519Chain', () => {
  it('returns true for solana', () => {
    assert.ok(isEd25519Chain('solana'));
  });

  it('returns true for solana_devnet', () => {
    assert.ok(isEd25519Chain('solana_devnet'));
  });

  it('returns false for ethereum', () => {
    assert.ok(!isEd25519Chain('ethereum'));
  });
});

describe('isEvmChain', () => {
  it('returns true for chains with chainId', () => {
    assert.ok(isEvmChain('ethereum'));
    assert.ok(isEvmChain('base'));
    assert.ok(isEvmChain('set_chain'));
    assert.ok(isEvmChain('arbitrum'));
  });

  it('returns false for non-EVM chains', () => {
    assert.ok(!isEvmChain('solana'));
    assert.ok(!isEvmChain('zcash'));
    assert.ok(!isEvmChain('bitcoin'));
  });
});

describe('isZcashChain', () => {
  it('identifies zcash chains', () => {
    assert.ok(isZcashChain('zcash'));
    assert.ok(isZcashChain('zcash_testnet'));
    assert.ok(!isZcashChain('bitcoin'));
  });
});

describe('isBitcoinChain', () => {
  it('identifies bitcoin chains', () => {
    assert.ok(isBitcoinChain('bitcoin'));
    assert.ok(isBitcoinChain('bitcoin_testnet'));
    assert.ok(!isBitcoinChain('zcash'));
  });
});

// =============================================================================
// listChains / getRecommendedChain
// =============================================================================

describe('listChains', () => {
  it('returns all chain IDs', () => {
    const chains = listChains();
    assert.ok(chains.includes('solana'));
    assert.ok(chains.includes('set_chain'));
    assert.ok(chains.includes('bitcoin'));
    assert.ok(chains.length >= 13);
  });
});

describe('getRecommendedChain', () => {
  it('returns base by default', () => {
    assert.equal(getRecommendedChain(), 'base');
  });

  it('returns testnet when requested', () => {
    assert.equal(getRecommendedChain({ testnet: true }), 'solana_devnet');
  });

  it('returns set_chain for preferNative', () => {
    assert.equal(getRecommendedChain({ preferNative: true }), 'set_chain');
  });
});

/**
 * Unit tests for treasury/registry.js — token registry CRUD
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs';
import {
  loadTokenRegistry,
  saveTokenRegistry,
  upsertToken,
  removeToken,
} from '../../src/treasury/registry.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function tmpRegistryPath() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'reg-test-'));
  return path.join(dir, 'tokens.json');
}

// ===========================================================================
// upsertToken
// ===========================================================================

describe('upsertToken', () => {
  it('adds a new token', () => {
    const reg = { tokens: [] };
    const result = upsertToken(reg, { symbol: 'USDC', chainId: 1, decimals: 6 });
    assert.strictEqual(result.tokens.length, 1);
    assert.strictEqual(result.tokens[0].symbol, 'USDC');
    assert.strictEqual(result.tokens[0].chainId, 1);
  });

  it('uppercases symbol', () => {
    const reg = { tokens: [] };
    const result = upsertToken(reg, { symbol: 'usdc', chainId: 1 });
    assert.strictEqual(result.tokens[0].symbol, 'USDC');
  });

  it('updates existing token (same symbol + chainId)', () => {
    const reg = { tokens: [{ symbol: 'USDC', chainId: 1, decimals: 6, address: '0xOld' }] };
    const result = upsertToken(reg, { symbol: 'usdc', chainId: 1, address: '0xNew' });
    assert.strictEqual(result.tokens.length, 1);
    assert.strictEqual(result.tokens[0].address, '0xNew');
    assert.strictEqual(result.tokens[0].decimals, 6); // Preserved from merge
  });

  it('different chainId creates new entry', () => {
    const reg = { tokens: [{ symbol: 'USDC', chainId: 1, decimals: 6 }] };
    const result = upsertToken(reg, { symbol: 'USDC', chainId: 8453 });
    assert.strictEqual(result.tokens.length, 2);
  });

  it('handles empty tokens array gracefully', () => {
    const result = upsertToken({ tokens: undefined }, { symbol: 'ETH', chainId: 1 });
    assert.strictEqual(result.tokens.length, 1);
  });
});

// ===========================================================================
// removeToken
// ===========================================================================

describe('removeToken', () => {
  it('removes token by symbol and chainId', () => {
    const reg = {
      tokens: [
        { symbol: 'USDC', chainId: 1 },
        { symbol: 'DAI', chainId: 1 },
      ],
    };
    const result = removeToken(reg, 'USDC', 1);
    assert.strictEqual(result.tokens.length, 1);
    assert.strictEqual(result.tokens[0].symbol, 'DAI');
  });

  it('is case-insensitive on symbol', () => {
    const reg = { tokens: [{ symbol: 'USDC', chainId: 1 }] };
    const result = removeToken(reg, 'usdc', 1);
    assert.strictEqual(result.tokens.length, 0);
  });

  it('only removes matching chainId', () => {
    const reg = {
      tokens: [
        { symbol: 'USDC', chainId: 1 },
        { symbol: 'USDC', chainId: 8453 },
      ],
    };
    const result = removeToken(reg, 'USDC', 1);
    assert.strictEqual(result.tokens.length, 1);
    assert.strictEqual(result.tokens[0].chainId, 8453);
  });

  it('no-op for nonexistent token', () => {
    const reg = { tokens: [{ symbol: 'USDC', chainId: 1 }] };
    const result = removeToken(reg, 'ETH', 1);
    assert.strictEqual(result.tokens.length, 1);
  });
});

// ===========================================================================
// loadTokenRegistry / saveTokenRegistry
// ===========================================================================

describe('loadTokenRegistry', () => {
  it('returns empty tokens for nonexistent file', async () => {
    const reg = await loadTokenRegistry('/tmp/nonexistent-tokens-file.json');
    assert.deepStrictEqual(reg, { tokens: [] });
  });

  it('round-trips through save and load', async () => {
    const p = tmpRegistryPath();
    const reg = { tokens: [{ symbol: 'USDC', chainId: 1, decimals: 6 }] };
    await saveTokenRegistry(p, reg);
    const loaded = await loadTokenRegistry(p);
    assert.strictEqual(loaded.tokens.length, 1);
    assert.strictEqual(loaded.tokens[0].symbol, 'USDC');
  });

  it('handles malformed JSON gracefully', async () => {
    const p = tmpRegistryPath();
    fs.writeFileSync(p, 'not-json');
    const reg = await loadTokenRegistry(p);
    assert.deepStrictEqual(reg, { tokens: [] });
  });

  it('handles missing tokens array in file', async () => {
    const p = tmpRegistryPath();
    fs.writeFileSync(p, JSON.stringify({ foo: 'bar' }));
    const reg = await loadTokenRegistry(p);
    assert.deepStrictEqual(reg, { tokens: [] });
  });
});

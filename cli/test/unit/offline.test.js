/**
 * Unit tests for offline.js — OfflineManager, OfflineCache, isApiError, showOfflineWarning
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import { OfflineManager, OfflineCache, createOfflineManager } from '../../src/offline.js';

// ===========================================================================
// OfflineManager
// ===========================================================================

describe('OfflineManager', () => {
  it('creates with defaults', () => {
    const mgr = new OfflineManager({ apiKey: 'test-key' });
    assert.strictEqual(mgr.forceOffline, false);
    assert.strictEqual(mgr.checkInterval, 30000);
    assert.strictEqual(mgr.cachedStatus, null);
  });

  it('forceOffline returns true from isOffline', async () => {
    const mgr = new OfflineManager({ forceOffline: true });
    const result = await mgr.isOffline();
    assert.strictEqual(result, true);
  });

  it('setOffline toggles force mode', () => {
    const mgr = new OfflineManager({ apiKey: 'test-key' });
    assert.strictEqual(mgr.forceOffline, false);
    mgr.setOffline(true);
    assert.strictEqual(mgr.forceOffline, true);
    mgr.setOffline(false);
    assert.strictEqual(mgr.forceOffline, false);
  });

  it('getStatus returns cached status', () => {
    const mgr = new OfflineManager();
    assert.strictEqual(mgr.getStatus(), null);
    mgr.cachedStatus = { available: true, reason: 'ok' };
    assert.strictEqual(mgr.getStatus().available, true);
  });

  it('uses cached status within check interval', async () => {
    const mgr = new OfflineManager({ apiKey: 'test-key', checkInterval: 60000 });
    mgr.cachedStatus = { available: true, reason: 'ok' };
    mgr.lastCheck = Date.now();
    // Should use cache, not make API call
    const result = await mgr.isOffline();
    assert.strictEqual(result, false);
  });

  it('isApiError detects known patterns', () => {
    const mgr = new OfflineManager();
    assert.strictEqual(mgr.isApiError(new Error('ECONNREFUSED')), true);
    assert.strictEqual(mgr.isApiError(new Error('ETIMEDOUT')), true);
    assert.strictEqual(mgr.isApiError(new Error('ENOTFOUND')), true);
    assert.strictEqual(mgr.isApiError(new Error('fetch failed')), true);
    assert.strictEqual(mgr.isApiError(new Error('network error')), true);
    assert.strictEqual(mgr.isApiError(new Error('API error 500')), true);
    assert.strictEqual(mgr.isApiError(new Error('rate limit exceeded')), true);
    assert.strictEqual(mgr.isApiError(new Error('502 Bad Gateway')), true);
    assert.strictEqual(mgr.isApiError(new Error('503 Service Unavailable')), true);
  });

  it('isApiError rejects unrelated errors', () => {
    const mgr = new OfflineManager();
    assert.strictEqual(mgr.isApiError(new Error('TypeError: undefined is not a function')), false);
    assert.strictEqual(mgr.isApiError(new Error('SyntaxError')), false);
    assert.strictEqual(mgr.isApiError(new Error('Invalid JSON')), false);
  });

  it('isApiError handles missing message', () => {
    const mgr = new OfflineManager();
    const err = {};
    assert.strictEqual(mgr.isApiError(err), false);
  });

  it('createFallbackWrapper uses direct handler when offline', async () => {
    const mgr = new OfflineManager({ forceOffline: true });
    const wrapper = mgr.createFallbackWrapper(
      async () => 'ai-result',
      async () => 'direct-result',
    );
    const result = await wrapper();
    assert.strictEqual(result, 'direct-result');
  });

  it('createFallbackWrapper uses AI handler when online', async () => {
    const mgr = new OfflineManager({ apiKey: 'test-key', checkInterval: 60000 });
    mgr.cachedStatus = { available: true, reason: 'ok' };
    mgr.lastCheck = Date.now();

    const wrapper = mgr.createFallbackWrapper(
      async () => 'ai-result',
      async () => 'direct-result',
    );
    const result = await wrapper();
    assert.strictEqual(result, 'ai-result');
  });

  it('createFallbackWrapper falls back on API error', async () => {
    const mgr = new OfflineManager({ apiKey: 'test-key', checkInterval: 60000 });
    mgr.cachedStatus = { available: true, reason: 'ok' };
    mgr.lastCheck = Date.now();

    const wrapper = mgr.createFallbackWrapper(
      async () => {
        throw new Error('ECONNREFUSED');
      },
      async () => 'fallback-result',
    );
    const result = await wrapper();
    assert.strictEqual(result, 'fallback-result');
  });

  it('createFallbackWrapper rethrows non-API errors', async () => {
    const mgr = new OfflineManager({ apiKey: 'test-key', checkInterval: 60000 });
    mgr.cachedStatus = { available: true, reason: 'ok' };
    mgr.lastCheck = Date.now();

    const wrapper = mgr.createFallbackWrapper(
      async () => {
        throw new Error('RangeError: stack overflow');
      },
      async () => 'fallback',
    );
    await assert.rejects(() => wrapper(), /stack overflow/);
  });
});

// ===========================================================================
// OfflineCache
// ===========================================================================

describe('OfflineCache', () => {
  it('get/set round-trip', () => {
    const cache = new OfflineCache();
    cache.set('key1', { data: 'hello' });
    assert.deepStrictEqual(cache.get('key1'), { data: 'hello' });
  });

  it('get returns null for missing key', () => {
    const cache = new OfflineCache();
    assert.strictEqual(cache.get('nope'), null);
  });

  it('get returns null for expired entry', () => {
    const cache = new OfflineCache({ maxAge: 1 }); // 1ms TTL
    cache.set('key1', 'value');
    // Force expiration
    cache.cache.get('key1').timestamp = Date.now() - 1000;
    assert.strictEqual(cache.get('key1'), null);
  });

  it('evicts oldest when at capacity', () => {
    const cache = new OfflineCache({ maxSize: 2 });
    cache.set('a', 1);
    // Ensure 'a' is oldest
    cache.cache.get('a').timestamp = Date.now() - 5000;
    cache.set('b', 2);
    cache.set('c', 3); // Should evict 'a'
    assert.strictEqual(cache.get('a'), null);
    assert.strictEqual(cache.get('b'), 2);
    assert.strictEqual(cache.get('c'), 3);
  });

  it('clear removes all entries', () => {
    const cache = new OfflineCache();
    cache.set('a', 1);
    cache.set('b', 2);
    cache.clear();
    assert.strictEqual(cache.get('a'), null);
    assert.strictEqual(cache.get('b'), null);
  });

  it('getStats returns correct counts', () => {
    const cache = new OfflineCache({ maxAge: 60000, maxSize: 100 });
    cache.set('valid1', 'v');
    cache.set('valid2', 'v');
    cache.set('expired1', 'v');
    // Manually expire one entry
    cache.cache.get('expired1').timestamp = Date.now() - 120000;

    const stats = cache.getStats();
    assert.strictEqual(stats.total, 3);
    assert.strictEqual(stats.valid, 2);
    assert.strictEqual(stats.expired, 1);
    assert.strictEqual(stats.maxSize, 100);
    assert.strictEqual(stats.maxAge, 60000);
  });

  it('getStats on empty cache', () => {
    const cache = new OfflineCache();
    const stats = cache.getStats();
    assert.strictEqual(stats.total, 0);
    assert.strictEqual(stats.valid, 0);
    assert.strictEqual(stats.expired, 0);
  });
});

// ===========================================================================
// createOfflineManager
// ===========================================================================

describe('createOfflineManager', () => {
  it('returns an OfflineManager instance', () => {
    const mgr = createOfflineManager({ apiKey: 'test' });
    assert.ok(mgr instanceof OfflineManager);
  });

  it('passes options through', () => {
    const mgr = createOfflineManager({ forceOffline: true, checkInterval: 5000 });
    assert.strictEqual(mgr.forceOffline, true);
    assert.strictEqual(mgr.checkInterval, 5000);
  });
});

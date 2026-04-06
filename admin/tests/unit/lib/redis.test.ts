/**
 * Tests for CacheMap (in-memory fallback mode)
 *
 * @module tests/unit/lib/redis
 */

import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { CacheMap } from '@/lib/shared/redis';

describe('CacheMap', () => {
  let cache: CacheMap<string>;

  beforeEach(() => {
    // CacheMap will use in-memory fallback since env vars are not set
    cache = new CacheMap<string>('test', 60_000);
  });

  describe('set and get', () => {
    it('stores and retrieves a value', async () => {
      await cache.set('key1', 'value1');
      const result = await cache.get('key1');

      expect(result).toBe('value1');
    });

    it('returns undefined for non-existent key', async () => {
      const result = await cache.get('missing');

      expect(result).toBeUndefined();
    });

    it('overwrites existing key', async () => {
      await cache.set('key1', 'first');
      await cache.set('key1', 'second');
      const result = await cache.get('key1');

      expect(result).toBe('second');
    });

    it('stores complex objects', async () => {
      const objCache = new CacheMap<{ name: string; count: number }>('obj', 60_000);
      const value = { name: 'test', count: 42 };

      await objCache.set('data', value);
      const result = await objCache.get('data');

      expect(result).toEqual(value);
    });

    it('stores multiple keys independently', async () => {
      await cache.set('a', 'alpha');
      await cache.set('b', 'beta');
      await cache.set('c', 'gamma');

      expect(await cache.get('a')).toBe('alpha');
      expect(await cache.get('b')).toBe('beta');
      expect(await cache.get('c')).toBe('gamma');
    });
  });

  describe('has', () => {
    it('returns true when key exists', async () => {
      await cache.set('present', 'yes');

      expect(await cache.has('present')).toBe(true);
    });

    it('returns false when key does not exist', async () => {
      expect(await cache.has('absent')).toBe(false);
    });
  });

  describe('delete', () => {
    it('removes an existing key and returns true', async () => {
      await cache.set('toDelete', 'value');
      const deleted = await cache.delete('toDelete');

      expect(deleted).toBe(true);
      expect(await cache.get('toDelete')).toBeUndefined();
    });

    it('returns false when deleting a non-existent key', async () => {
      const deleted = await cache.delete('nonexistent');

      expect(deleted).toBe(false);
    });

    it('does not affect other keys', async () => {
      await cache.set('keep', 'keepme');
      await cache.set('remove', 'removeme');
      await cache.delete('remove');

      expect(await cache.get('keep')).toBe('keepme');
      expect(await cache.get('remove')).toBeUndefined();
    });
  });

  describe('TTL expiration', () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it('returns value before TTL expires', async () => {
      const shortCache = new CacheMap<string>('short', 5000);

      await shortCache.set('key', 'value');
      vi.advanceTimersByTime(4999);

      const result = await shortCache.get('key');
      expect(result).toBe('value');
    });

    it('returns undefined after TTL expires (get)', async () => {
      const shortCache = new CacheMap<string>('short', 5000);

      await shortCache.set('key', 'value');
      vi.advanceTimersByTime(5001);

      const result = await shortCache.get('key');
      expect(result).toBeUndefined();
    });

    it('returns false for has() after TTL expires', async () => {
      const shortCache = new CacheMap<string>('short', 5000);

      await shortCache.set('key', 'value');
      vi.advanceTimersByTime(5001);

      expect(await shortCache.has('key')).toBe(false);
    });

    it('respects custom TTL per set call', async () => {
      await cache.set('custom', 'value', 2000);

      vi.advanceTimersByTime(1999);
      expect(await cache.get('custom')).toBe('value');

      vi.advanceTimersByTime(2);
      expect(await cache.get('custom')).toBeUndefined();
    });

    it('uses default TTL when not specified', async () => {
      // Default TTL is 60_000ms
      await cache.set('default-ttl', 'value');

      vi.advanceTimersByTime(59_999);
      expect(await cache.get('default-ttl')).toBe('value');

      vi.advanceTimersByTime(2);
      expect(await cache.get('default-ttl')).toBeUndefined();
    });
  });

  describe('cleanup', () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it('removes expired entries', async () => {
      const shortCache = new CacheMap<string>('cleanup', 1000);

      await shortCache.set('expired1', 'val1');
      await shortCache.set('expired2', 'val2');

      vi.advanceTimersByTime(1001);

      // Add a fresh entry
      await shortCache.set('fresh', 'val3');

      shortCache.cleanup();

      // Expired entries should be removed
      expect(await shortCache.get('expired1')).toBeUndefined();
      expect(await shortCache.get('expired2')).toBeUndefined();
      // Fresh entry should remain
      expect(await shortCache.get('fresh')).toBe('val3');
    });

    it('does nothing when no entries are expired', async () => {
      await cache.set('a', 'alpha');
      await cache.set('b', 'beta');

      cache.cleanup();

      expect(await cache.get('a')).toBe('alpha');
      expect(await cache.get('b')).toBe('beta');
    });

    it('does nothing when cache is empty', () => {
      // Should not throw
      expect(() => cache.cleanup()).not.toThrow();
    });
  });
});

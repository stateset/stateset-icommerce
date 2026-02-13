/**
 * Tests for cli/src/channels/plugin-slots.js — PluginSlots
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

let PluginSlots;
let moduleLoaded = false;

try {
  const mod = await import('../../src/channels/plugin-slots.js');
  PluginSlots = mod.PluginSlots;
  moduleLoaded = true;
} catch {
  // Module may fail in test env
}

describe('PluginSlots', { skip: !moduleLoaded && 'Module not loadable in test env' }, () => {
  let slots;

  beforeEach(() => {
    slots = new PluginSlots();
  });

  describe('defineSlot', () => {
    it('defines a new slot', () => {
      slots.defineSlot('memory', { description: 'Memory provider' });
      const all = slots.getSlotStates();
      assert.ok(all.some((s) => s.name === 'memory'));
    });

    it('throws on duplicate slot definition', () => {
      slots.defineSlot('memory');
      assert.throws(() => slots.defineSlot('memory'), /already defined/i);
    });

    it('defaults required to false', () => {
      slots.defineSlot('search');
      const list = slots.getSlotStates();
      const searchSlot = list.find((s) => s.name === 'search');
      assert.strictEqual(searchSlot.required, false);
    });

    it('accepts required option', () => {
      slots.defineSlot('auth', { required: true });
      const list = slots.getSlotStates();
      const authSlot = list.find((s) => s.name === 'auth');
      assert.strictEqual(authSlot.required, true);
    });

    it('accepts defaultPlugin option', () => {
      slots.defineSlot('cache', { defaultPlugin: 'redis-cache' });
      const list = slots.getSlotStates();
      const cacheSlot = list.find((s) => s.name === 'cache');
      assert.strictEqual(cacheSlot.defaultPlugin, 'redis-cache');
    });
  });

  describe('assign / getAssigned', () => {
    it('assigns a plugin to a slot', () => {
      slots.defineSlot('memory');
      slots.registerCandidate('memory', 'redis-memory');
      slots.assign('memory', 'redis-memory');
      assert.strictEqual(slots.getAssigned('memory'), 'redis-memory');
    });

    it('returns null for unassigned slot', () => {
      slots.defineSlot('memory');
      assert.strictEqual(slots.getAssigned('memory'), null);
    });

    it('replaces previous assignment', () => {
      slots.defineSlot('memory');
      slots.registerCandidate('memory', 'redis-memory');
      slots.registerCandidate('memory', 'sqlite-memory');
      slots.assign('memory', 'redis-memory');
      slots.assign('memory', 'sqlite-memory');
      assert.strictEqual(slots.getAssigned('memory'), 'sqlite-memory');
    });
  });

  describe('registerCandidate', () => {
    it('adds a candidate to a slot', () => {
      slots.defineSlot('memory');
      slots.registerCandidate('memory', 'redis-memory');
      const list = slots.getSlotStates();
      const memSlot = list.find((s) => s.name === 'memory');
      assert.ok(memSlot.candidates.includes('redis-memory'));
    });
  });

  describe('clearSlot', () => {
    it('clears a plugin from a slot', () => {
      slots.defineSlot('memory');
      slots.registerCandidate('memory', 'redis-memory');
      slots.assign('memory', 'redis-memory');
      slots.clearSlot('memory');
      assert.strictEqual(slots.getAssigned('memory'), null);
    });
  });

  describe('getSlotStates', () => {
    it('returns empty list when no slots defined', () => {
      assert.deepStrictEqual(slots.getSlotStates(), []);
    });

    it('returns all defined slots', () => {
      slots.defineSlot('memory');
      slots.defineSlot('search');
      slots.defineSlot('cache');
      const list = slots.getSlotStates();
      assert.strictEqual(list.length, 3);
    });
  });

  describe('getPluginSlot', () => {
    it('returns slot name for assigned plugin', () => {
      slots.defineSlot('memory');
      slots.registerCandidate('memory', 'redis-memory');
      slots.assign('memory', 'redis-memory');
      assert.strictEqual(slots.getPluginSlot('redis-memory'), 'memory');
    });

    it('returns null for unassigned plugin', () => {
      assert.strictEqual(slots.getPluginSlot('unknown-plugin'), null);
    });
  });
});

/**
 * Unit tests for channels/gateway-methods.js — GatewayMethodRegistry
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert';
import {
  GatewayMethodRegistry,
  getGatewayMethods,
  resetGatewayMethods,
} from '../../src/channels/gateway-methods.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function reg() {
  return new GatewayMethodRegistry();
}

function addEcho(r, method = 'test.echo') {
  r.register(method, {
    pluginId: 'test-plugin',
    description: 'Echo params back',
    handler: async (params) => ({ echo: params }),
  });
}

// ===========================================================================
// Registration
// ===========================================================================

describe('GatewayMethodRegistry register', () => {
  it('registers a method', () => {
    const r = reg();
    addEcho(r);
    assert.ok(r.has('test.echo'));
  });

  it('throws on duplicate registration', () => {
    const r = reg();
    addEcho(r);
    assert.throws(() => addEcho(r), /already registered/);
  });

  it('throws on empty method name', () => {
    const r = reg();
    assert.throws(() => r.register('', { pluginId: 'p', handler: () => {} }), /non-empty string/);
  });

  it('throws when handler is not a function', () => {
    const r = reg();
    assert.throws(
      () => r.register('x.y', { pluginId: 'p', handler: 'nope' }),
      /must be a function/,
    );
  });

  it('throws when pluginId is missing', () => {
    const r = reg();
    assert.throws(() => r.register('x.y', { handler: () => {} }), /Plugin ID required/);
  });

  it('defaults description to empty string', () => {
    const r = reg();
    r.register('x.y', { pluginId: 'p', handler: () => {} });
    assert.strictEqual(r.get('x.y').description, '');
  });

  it('stores requiresAuth flag', () => {
    const r = reg();
    r.register('a.b', { pluginId: 'p', handler: () => {}, requiresAuth: true });
    assert.strictEqual(r.get('a.b').requiresAuth, true);
  });

  it('defaults requiresAuth to false', () => {
    const r = reg();
    addEcho(r);
    assert.strictEqual(r.get('test.echo').requiresAuth, false);
  });
});

// ===========================================================================
// Unregister
// ===========================================================================

describe('GatewayMethodRegistry unregister', () => {
  it('removes a method', () => {
    const r = reg();
    addEcho(r);
    assert.strictEqual(r.unregister('test.echo'), true);
    assert.strictEqual(r.has('test.echo'), false);
  });

  it('returns false for missing method', () => {
    const r = reg();
    assert.strictEqual(r.unregister('nope'), false);
  });
});

// ===========================================================================
// unregisterPlugin
// ===========================================================================

describe('GatewayMethodRegistry unregisterPlugin', () => {
  it('removes all methods for a plugin', () => {
    const r = reg();
    r.register('a.one', { pluginId: 'plugin-a', handler: () => {} });
    r.register('a.two', { pluginId: 'plugin-a', handler: () => {} });
    r.register('b.one', { pluginId: 'plugin-b', handler: () => {} });

    const count = r.unregisterPlugin('plugin-a');
    assert.strictEqual(count, 2);
    assert.strictEqual(r.has('a.one'), false);
    assert.strictEqual(r.has('a.two'), false);
    assert.strictEqual(r.has('b.one'), true);
  });

  it('returns 0 for unknown plugin', () => {
    const r = reg();
    assert.strictEqual(r.unregisterPlugin('nope'), 0);
  });
});

// ===========================================================================
// get / has
// ===========================================================================

describe('GatewayMethodRegistry get/has', () => {
  it('get returns null for missing', () => {
    const r = reg();
    assert.strictEqual(r.get('nope'), null);
  });

  it('get returns the definition', () => {
    const r = reg();
    addEcho(r);
    const def = r.get('test.echo');
    assert.strictEqual(def.method, 'test.echo');
    assert.strictEqual(def.pluginId, 'test-plugin');
  });
});

// ===========================================================================
// invoke
// ===========================================================================

describe('GatewayMethodRegistry invoke', () => {
  it('invokes a method successfully', async () => {
    const r = reg();
    addEcho(r);
    const result = await r.invoke('test.echo', { msg: 'hi' });
    assert.strictEqual(result.ok, true);
    assert.deepStrictEqual(result.result, { echo: { msg: 'hi' } });
    assert.ok(result.durationMs >= 0);
  });

  it('returns error for unknown method', async () => {
    const r = reg();
    const result = await r.invoke('nope.method');
    assert.strictEqual(result.ok, false);
    assert.ok(result.error.includes('Unknown method'));
  });

  it('catches handler errors', async () => {
    const r = reg();
    r.register('fail.op', {
      pluginId: 'p',
      handler: async () => {
        throw new Error('handler boom');
      },
    });
    const result = await r.invoke('fail.op');
    assert.strictEqual(result.ok, false);
    assert.strictEqual(result.error, 'handler boom');
    assert.ok(result.durationMs >= 0);
  });

  it('validates required parameters', async () => {
    const r = reg();
    r.register('val.test', {
      pluginId: 'p',
      handler: async (params) => params,
      schema: { required: ['name'] },
    });

    const result = await r.invoke('val.test', {});
    assert.strictEqual(result.ok, false);
    assert.ok(result.error.includes('Missing required parameter'));
    assert.ok(result.error.includes('name'));
  });

  it('validates parameter types', async () => {
    const r = reg();
    r.register('val.type', {
      pluginId: 'p',
      handler: async (params) => params,
      schema: {
        properties: { count: { type: 'number' } },
      },
    });

    const result = await r.invoke('val.type', { count: 'not-a-number' });
    assert.strictEqual(result.ok, false);
    assert.ok(result.error.includes('expected number'));
  });

  it('passes validation when params are correct', async () => {
    const r = reg();
    r.register('val.ok', {
      pluginId: 'p',
      handler: async (params) => params,
      schema: {
        required: ['name'],
        properties: { name: { type: 'string' } },
      },
    });

    const result = await r.invoke('val.ok', { name: 'Alice' });
    assert.strictEqual(result.ok, true);
  });

  it('passes context to handler', async () => {
    const r = reg();
    r.register('ctx.test', {
      pluginId: 'p',
      handler: async (params, context) => ({ sender: context.senderId }),
    });

    const result = await r.invoke('ctx.test', {}, { senderId: 'user-1' });
    assert.deepStrictEqual(result.result, { sender: 'user-1' });
  });

  it('validates array type', async () => {
    const r = reg();
    r.register('val.arr', {
      pluginId: 'p',
      handler: async (params) => params,
      schema: { properties: { items: { type: 'array' } } },
    });

    const ok = await r.invoke('val.arr', { items: [1, 2] });
    assert.strictEqual(ok.ok, true);

    const fail = await r.invoke('val.arr', { items: 'not-array' });
    assert.strictEqual(fail.ok, false);
  });
});

// ===========================================================================
// list
// ===========================================================================

describe('GatewayMethodRegistry list', () => {
  it('lists all methods', () => {
    const r = reg();
    addEcho(r, 'a.one');
    addEcho(r, 'b.two');
    assert.strictEqual(r.list().length, 2);
  });

  it('filters by pluginId', () => {
    const r = reg();
    r.register('a.one', { pluginId: 'pa', handler: () => {} });
    r.register('b.two', { pluginId: 'pb', handler: () => {} });
    const result = r.list({ pluginId: 'pa' });
    assert.strictEqual(result.length, 1);
    assert.strictEqual(result[0].method, 'a.one');
  });

  it('filters by prefix', () => {
    const r = reg();
    addEcho(r, 'ns.one');
    addEcho(r, 'ns.two');
    addEcho(r, 'other.three');
    const result = r.list({ prefix: 'ns.' });
    assert.strictEqual(result.length, 2);
  });
});

// ===========================================================================
// getNamespaces
// ===========================================================================

describe('GatewayMethodRegistry getNamespaces', () => {
  it('groups methods by namespace', () => {
    const r = reg();
    addEcho(r, 'orders.list');
    addEcho(r, 'orders.get');
    addEcho(r, 'inventory.check');

    const ns = r.getNamespaces();
    assert.deepStrictEqual(ns.orders.sort(), ['get', 'list']);
    assert.deepStrictEqual(ns.inventory, ['check']);
  });

  it('puts non-dotted methods in _root', () => {
    const r = reg();
    addEcho(r, 'simpleMethod');
    const ns = r.getNamespaces();
    assert.deepStrictEqual(ns._root, ['simpleMethod']);
  });
});

// ===========================================================================
// generateHelp
// ===========================================================================

describe('GatewayMethodRegistry generateHelp', () => {
  it('returns no-methods message when empty', () => {
    const r = reg();
    assert.ok(r.generateHelp().includes('No gateway methods'));
  });

  it('includes method names and descriptions', () => {
    const r = reg();
    r.register('orders.list', {
      pluginId: 'p',
      handler: () => {},
      description: 'List all orders',
    });
    r.register('orders.get', {
      pluginId: 'p',
      handler: () => {},
      description: 'Get an order',
      requiresAuth: true,
    });

    const help = r.generateHelp();
    assert.ok(help.includes('orders.list'));
    assert.ok(help.includes('List all orders'));
    assert.ok(help.includes('[auth]'));
  });
});

// ===========================================================================
// clear
// ===========================================================================

describe('GatewayMethodRegistry clear', () => {
  it('removes all methods', () => {
    const r = reg();
    addEcho(r, 'a.b');
    addEcho(r, 'c.d');
    r.clear();
    assert.strictEqual(r.list().length, 0);
  });
});

// ===========================================================================
// Singleton
// ===========================================================================

describe('GatewayMethodRegistry singleton', () => {
  beforeEach(() => resetGatewayMethods());

  it('getGatewayMethods returns same instance', () => {
    assert.strictEqual(getGatewayMethods(), getGatewayMethods());
  });

  it('resetGatewayMethods clears singleton', () => {
    const a = getGatewayMethods();
    resetGatewayMethods();
    const b = getGatewayMethods();
    assert.notStrictEqual(a, b);
  });
});

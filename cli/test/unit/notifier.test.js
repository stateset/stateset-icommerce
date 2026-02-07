/**
 * Unit tests for channels/notifier.js — ChannelNotifier
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert';
import { ChannelNotifier } from '../../src/channels/notifier.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function mockAdapter(log = []) {
  return {
    send: async (target, text) => log.push({ target, text }),
    sendRichMessage: async (target, rich) => log.push({ target, rich }),
    formatForPlatform: (text) => `[formatted] ${text}`,
    log,
  };
}

function plainAdapter(log = []) {
  return {
    send: async (target, text) => log.push({ target, text }),
    log,
  };
}

function failAdapter() {
  return {
    send: async () => {
      throw new Error('send failed');
    },
  };
}

// ===========================================================================
// Channel registration
// ===========================================================================

describe('ChannelNotifier channel registration', () => {
  it('registerChannel adds a channel', () => {
    const n = new ChannelNotifier();
    n.registerChannel('slack', mockAdapter());
    assert.deepStrictEqual(n.getRegisteredChannels(), ['slack']);
  });

  it('unregisterChannel removes a channel', () => {
    const n = new ChannelNotifier();
    n.registerChannel('slack', mockAdapter());
    n.unregisterChannel('slack');
    assert.deepStrictEqual(n.getRegisteredChannels(), []);
  });

  it('lists multiple registered channels', () => {
    const n = new ChannelNotifier();
    n.registerChannel('slack', mockAdapter());
    n.registerChannel('telegram', mockAdapter());
    const channels = n.getRegisteredChannels();
    assert.ok(channels.includes('slack'));
    assert.ok(channels.includes('telegram'));
  });
});

// ===========================================================================
// Routes
// ===========================================================================

describe('ChannelNotifier routes', () => {
  it('addRoute creates a route', () => {
    const n = new ChannelNotifier();
    n.addRoute('order.shipped', 'slack', '#orders');
    const routes = n.getRoutes();
    assert.strictEqual(routes['order.shipped'].length, 1);
    assert.strictEqual(routes['order.shipped'][0].channel, 'slack');
    assert.strictEqual(routes['order.shipped'][0].target, '#orders');
  });

  it('addRoute deduplicates', () => {
    const n = new ChannelNotifier();
    n.addRoute('order.shipped', 'slack', '#orders');
    n.addRoute('order.shipped', 'slack', '#orders');
    assert.strictEqual(n.getRoutes()['order.shipped'].length, 1);
  });

  it('addRoute allows different targets for same event', () => {
    const n = new ChannelNotifier();
    n.addRoute('order.shipped', 'slack', '#orders');
    n.addRoute('order.shipped', 'slack', '#alerts');
    assert.strictEqual(n.getRoutes()['order.shipped'].length, 2);
  });

  it('loadRoutes bulk loads config', () => {
    const n = new ChannelNotifier();
    n.loadRoutes({
      'order.shipped': [
        { channel: 'slack', target: '#orders' },
        { channel: 'telegram', target: 'chat123' },
      ],
      '*': [{ channel: 'slack', target: '#all' }],
    });
    const routes = n.getRoutes();
    assert.strictEqual(routes['order.shipped'].length, 2);
    assert.strictEqual(routes['*'].length, 1);
  });

  it('loadRoutes ignores invalid config', () => {
    const n = new ChannelNotifier();
    n.loadRoutes(null);
    n.loadRoutes({ bad: 'not-array' });
    n.loadRoutes({ ok: [{ channel: 'slack' }] }); // missing target
    assert.deepStrictEqual(n.getRoutes(), {});
  });
});

// ===========================================================================
// sendNotification
// ===========================================================================

describe('ChannelNotifier sendNotification', () => {
  it('sends to exact route', async () => {
    const log = [];
    const n = new ChannelNotifier();
    n.registerChannel('slack', plainAdapter(log));
    n.addRoute('order.shipped', 'slack', '#orders');

    const result = await n.sendNotification({
      type: 'order.shipped',
      message: 'Order shipped!',
    });

    assert.strictEqual(result.sent, 1);
    assert.strictEqual(result.errors, 0);
    assert.strictEqual(log[0].target, '#orders');
    assert.strictEqual(log[0].text, 'Order shipped!');
  });

  it('sends to wildcard routes too', async () => {
    const log = [];
    const n = new ChannelNotifier();
    n.registerChannel('slack', plainAdapter(log));
    n.addRoute('*', 'slack', '#all');

    const result = await n.sendNotification({
      type: 'order.shipped',
      message: 'Ship it!',
    });

    assert.strictEqual(result.sent, 1);
    assert.strictEqual(log[0].target, '#all');
  });

  it('deduplicates exact + wildcard routes', async () => {
    const log = [];
    const n = new ChannelNotifier();
    n.registerChannel('slack', plainAdapter(log));
    n.addRoute('order.shipped', 'slack', '#orders');
    n.addRoute('*', 'slack', '#orders'); // same channel:target

    const result = await n.sendNotification({
      type: 'order.shipped',
      message: 'Dedup test',
    });

    assert.strictEqual(result.sent, 1); // not 2
  });

  it('prefers rich message when adapter supports it', async () => {
    const log = [];
    const n = new ChannelNotifier();
    n.registerChannel('slack', mockAdapter(log));
    n.addRoute('order.shipped', 'slack', '#orders');

    await n.sendNotification({
      type: 'order.shipped',
      message: 'plain',
      richMessage: { title: 'Rich' },
    });

    assert.ok(log[0].rich); // used sendRichMessage
    assert.strictEqual(log[0].rich.title, 'Rich');
  });

  it('falls back to plain text when adapter lacks sendRichMessage', async () => {
    const log = [];
    const n = new ChannelNotifier();
    n.registerChannel('basic', plainAdapter(log));
    n.addRoute('ev', 'basic', 'target1');

    await n.sendNotification({
      type: 'ev',
      message: 'plain message',
      richMessage: { title: 'Rich' },
    });

    assert.strictEqual(log[0].text, 'plain message');
  });

  it('uses formatForPlatform when available and no rich adapter', async () => {
    const log = [];
    const adapter = {
      send: async (target, text) => log.push({ target, text }),
      formatForPlatform: (text) => `**${text}**`,
    };
    const n = new ChannelNotifier();
    n.registerChannel('fmt', adapter);
    n.addRoute('ev', 'fmt', 't1');

    await n.sendNotification({ type: 'ev', message: 'hello' });
    assert.strictEqual(log[0].text, '**hello**');
  });

  it('counts errors for unregistered channels', async () => {
    const n = new ChannelNotifier();
    n.addRoute('ev', 'missing-channel', 'target');

    const result = await n.sendNotification({ type: 'ev', message: 'fail' });
    assert.strictEqual(result.sent, 0);
    assert.strictEqual(result.errors, 1);
  });

  it('counts errors when adapter throws', async () => {
    const n = new ChannelNotifier();
    n.registerChannel('fail', failAdapter());
    n.addRoute('ev', 'fail', 't1');

    const result = await n.sendNotification({ type: 'ev', message: 'boom' });
    assert.strictEqual(result.sent, 0);
    assert.strictEqual(result.errors, 1);
  });

  it('returns zeros when no routes match', async () => {
    const n = new ChannelNotifier();
    const result = await n.sendNotification({ type: 'unrouted', message: 'nope' });
    assert.strictEqual(result.sent, 0);
    assert.strictEqual(result.errors, 0);
  });
});

// ===========================================================================
// broadcast
// ===========================================================================

describe('ChannelNotifier broadcast', () => {
  it('sends to wildcard routes', async () => {
    const log = [];
    const n = new ChannelNotifier();
    n.registerChannel('slack', plainAdapter(log));
    n.addRoute('*', 'slack', '#broadcast');

    const result = await n.broadcast('Hello everyone!');
    assert.strictEqual(result.sent, 1);
    assert.strictEqual(log[0].text, 'Hello everyone!');
  });
});

// ===========================================================================
// sendToCustomer
// ===========================================================================

describe('ChannelNotifier sendToCustomer', () => {
  it('returns zeros when no identityStore', async () => {
    const n = new ChannelNotifier();
    const result = await n.sendToCustomer('cust-1', { message: 'hi' }, null);
    assert.strictEqual(result.sent, 0);
  });

  it('sends to customer linked channels', async () => {
    const log = [];
    const n = new ChannelNotifier();
    n.registerChannel('telegram', plainAdapter(log));

    const identityStore = {
      getChannelsForCustomer: () => [{ channel: 'telegram', senderId: 'tg-user-1' }],
    };

    const result = await n.sendToCustomer(
      'cust-1',
      { message: 'Your order shipped!' },
      identityStore,
    );
    assert.strictEqual(result.sent, 1);
    assert.strictEqual(log[0].target, 'tg-user-1');
    assert.strictEqual(log[0].text, 'Your order shipped!');
  });

  it('returns zeros when customer has no linked channels', async () => {
    const n = new ChannelNotifier();
    const identityStore = { getChannelsForCustomer: () => [] };
    const result = await n.sendToCustomer('cust-1', { message: 'hi' }, identityStore);
    assert.strictEqual(result.sent, 0);
  });

  it('skips unregistered adapter channels', async () => {
    const n = new ChannelNotifier();
    const identityStore = {
      getChannelsForCustomer: () => [{ channel: 'unknown', senderId: 'u1' }],
    };
    const result = await n.sendToCustomer('cust-1', { message: 'hi' }, identityStore);
    assert.strictEqual(result.sent, 0);
  });

  it('counts errors when adapter throws', async () => {
    const n = new ChannelNotifier();
    n.registerChannel('fail', failAdapter());
    const identityStore = {
      getChannelsForCustomer: () => [{ channel: 'fail', senderId: 'u1' }],
    };
    const result = await n.sendToCustomer('cust-1', { message: 'hi' }, identityStore);
    assert.strictEqual(result.errors, 1);
  });
});

// ===========================================================================
// getRoutes
// ===========================================================================

describe('ChannelNotifier getRoutes', () => {
  it('returns a defensive copy', () => {
    const n = new ChannelNotifier();
    n.addRoute('ev', 'slack', '#ch');
    const routes = n.getRoutes();
    routes['ev'][0].channel = 'mutated';
    // Original should not be affected
    const routes2 = n.getRoutes();
    assert.strictEqual(routes2['ev'][0].channel, 'slack');
  });
});

/**
 * Unit tests for channels/identity.js — CustomerIdentityStore
 */

import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert';
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs';
import { CustomerIdentityStore, buildCustomerContext } from '../../src/channels/identity.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function tmpDbPath() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'ident-test-'));
  return path.join(dir, 'channel-identity.db');
}

// ===========================================================================
// CustomerIdentityStore
// ===========================================================================

describe('CustomerIdentityStore', () => {
  /** @type {CustomerIdentityStore|null} */
  let store = null;

  afterEach(() => {
    if (store) {
      try {
        store.close();
      } catch {}
      store = null;
    }
  });

  it('creates database on construction', () => {
    const dbPath = tmpDbPath();
    store = new CustomerIdentityStore({ dbPath });
    assert.ok(fs.existsSync(dbPath));
  });

  it('getCustomerId returns null for unknown sender', () => {
    store = new CustomerIdentityStore({ dbPath: tmpDbPath() });
    assert.strictEqual(store.getCustomerId('telegram', '12345'), null);
  });

  it('link and getCustomerId round-trip', () => {
    store = new CustomerIdentityStore({ dbPath: tmpDbPath() });
    store.link('telegram', 'user-123', 'cust-abc');
    const result = store.getCustomerId('telegram', 'user-123');
    assert.ok(result);
    assert.strictEqual(result.customerId, 'cust-abc');
    assert.strictEqual(result.linkedBy, 'auto');
  });

  it('link with manual linkedBy', () => {
    store = new CustomerIdentityStore({ dbPath: tmpDbPath() });
    store.link('discord', 'user-456', 'cust-def', 'manual');
    const result = store.getCustomerId('discord', 'user-456');
    assert.strictEqual(result.linkedBy, 'manual');
  });

  it('link upserts (overwrites existing link)', () => {
    store = new CustomerIdentityStore({ dbPath: tmpDbPath() });
    store.link('telegram', 'user-1', 'cust-old');
    store.link('telegram', 'user-1', 'cust-new');
    const result = store.getCustomerId('telegram', 'user-1');
    assert.strictEqual(result.customerId, 'cust-new');
  });

  it('different channels have independent identity links', () => {
    store = new CustomerIdentityStore({ dbPath: tmpDbPath() });
    store.link('telegram', 'user-1', 'cust-a');
    store.link('discord', 'user-1', 'cust-b');

    const telResult = store.getCustomerId('telegram', 'user-1');
    const discResult = store.getCustomerId('discord', 'user-1');
    assert.strictEqual(telResult.customerId, 'cust-a');
    assert.strictEqual(discResult.customerId, 'cust-b');
  });

  it('unlink removes identity mapping', () => {
    store = new CustomerIdentityStore({ dbPath: tmpDbPath() });
    store.link('telegram', 'user-1', 'cust-a');
    store.unlink('telegram', 'user-1');
    assert.strictEqual(store.getCustomerId('telegram', 'user-1'), null);
  });

  it('getChannelsForCustomer returns all linked channels', () => {
    store = new CustomerIdentityStore({ dbPath: tmpDbPath() });
    store.link('telegram', 'tg-user', 'cust-1');
    store.link('discord', 'dc-user', 'cust-1');
    store.link('slack', 'sl-user', 'cust-2'); // different customer

    const channels = store.getChannelsForCustomer('cust-1');
    assert.strictEqual(channels.length, 2);
    const channelNames = channels.map((c) => c.channel);
    assert.ok(channelNames.includes('telegram'));
    assert.ok(channelNames.includes('discord'));
  });

  it('getChannelsForCustomer returns empty for unknown customer', () => {
    store = new CustomerIdentityStore({ dbPath: tmpDbPath() });
    const channels = store.getChannelsForCustomer('nonexistent');
    assert.deepStrictEqual(channels, []);
  });

  it('persists data across instances', () => {
    const dbPath = tmpDbPath();
    store = new CustomerIdentityStore({ dbPath });
    store.link('whatsapp', '+15551234567', 'cust-wa');
    store.close();

    const store2 = new CustomerIdentityStore({ dbPath });
    const result = store2.getCustomerId('whatsapp', '+15551234567');
    assert.strictEqual(result.customerId, 'cust-wa');
    store2.close();
    store = null;
  });
});

// ===========================================================================
// buildCustomerContext
// ===========================================================================

describe('buildCustomerContext', () => {
  it('builds context string from customer fields', async () => {
    const customer = {
      firstName: 'Alice',
      lastName: 'Smith',
      email: 'alice@example.com',
      phone: '+15551234567',
    };
    const ctx = await buildCustomerContext(customer);
    assert.ok(ctx.includes('Alice Smith'));
    assert.ok(ctx.includes('alice@example.com'));
    assert.ok(ctx.includes('+15551234567'));
  });

  it('handles snake_case fields', async () => {
    const customer = {
      first_name: 'Bob',
      last_name: 'Jones',
    };
    const ctx = await buildCustomerContext(customer);
    assert.ok(ctx.includes('Bob Jones'));
  });

  it('handles missing name', async () => {
    const customer = { email: 'unknown@test.org' };
    const ctx = await buildCustomerContext(customer);
    assert.ok(ctx.includes('Unknown'));
    assert.ok(ctx.includes('unknown@test.org'));
  });
});

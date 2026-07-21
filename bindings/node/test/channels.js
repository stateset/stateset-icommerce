/**
 * Channel API tests for @stateset/embedded Node.js bindings.
 *
 * Sales / fulfillment channel lifecycle: create, get, update, list,
 * lock, product mappings, and soft delete.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { randomUUID } = require('node:crypto');

test('Channels: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('channels API exists and is supported', async () => {
    assert.ok(commerce.channels, 'channels API should exist');
    assert.equal(await commerce.channels.isSupported(), true);
  });

  let channel;
  await t.test('create returns an active, unlocked channel', async () => {
    channel = await commerce.channels.create({
      name: 'Shopify US',
      channelType: 'sales_channel',
      integration: 'shopify',
      tags: ['us'],
      metadata: JSON.stringify({ region: 'us' }),
    });
    assert.ok(channel.id);
    assert.equal(channel.name, 'Shopify US');
    assert.equal(channel.channelType, 'sales_channel');
    assert.equal(channel.status, 'active');
    assert.equal(channel.apiLocked, false);
    assert.deepEqual(channel.tags, ['us']);
    assert.ok(channel.createdAt);
  });

  await t.test('create rejects an invalid channel type', async () => {
    await assert.rejects(
      () => commerce.channels.create({ name: 'x', channelType: 'nope' }),
      /Invalid channel type/,
    );
  });

  await t.test('get returns the channel, and null when missing', async () => {
    const found = await commerce.channels.get(channel.id);
    assert.equal(found.name, 'Shopify US');
    assert.equal(await commerce.channels.get(randomUUID()), null);
  });

  await t.test('update applies patch semantics', async () => {
    const updated = await commerce.channels.update(channel.id, {
      name: 'Shopify NA',
      status: 'paused',
    });
    assert.equal(updated.name, 'Shopify NA');
    assert.equal(updated.status, 'paused');
    assert.equal(updated.channelType, 'sales_channel');
    assert.equal(updated.integration, 'shopify');
  });

  await t.test('list filters by status and integration', async () => {
    const paused = await commerce.channels.list({ status: 'paused' });
    assert.equal(paused.length, 1);
    assert.equal((await commerce.channels.list({ status: 'active' })).length, 0);
    assert.equal((await commerce.channels.list({ integration: 'shopify' })).length, 1);
    assert.equal((await commerce.channels.list()).length, 1);
  });

  await t.test('setLock toggles the API lock', async () => {
    const locked = await commerce.channels.setLock(channel.id, true);
    assert.equal(locked.apiLocked, true);
    const unlocked = await commerce.channels.setLock(channel.id, false);
    assert.equal(unlocked.apiLocked, false);
  });

  await t.test('product mappings start empty', async () => {
    const mappings = await commerce.channels.listProductMappings(channel.id);
    assert.deepEqual(mappings, []);
  });

  await t.test('delete soft-deletes the channel', async () => {
    await commerce.channels.delete(channel.id);
    const after = await commerce.channels.get(channel.id);
    if (after !== null) {
      assert.equal(after.status, 'deleted');
    }
  });
});

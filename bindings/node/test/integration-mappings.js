/**
 * Integration mapping API tests for @stateset/embedded Node.js bindings.
 *
 * Create, get, update, list, resolve, bulk upsert, delete.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { randomUUID } = require('node:crypto');

test('IntegrationMappings: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('API exists and is supported', async () => {
    assert.ok(commerce.integrationMappings, 'integrationMappings API should exist');
    assert.equal(await commerce.integrationMappings.isSupported(), true);
  });

  let mapping;
  await t.test('create returns an active mapping', async () => {
    mapping = await commerce.integrationMappings.create({
      integration: 'shopify',
      mappingGroup: 'order_status',
      fieldName: 'status',
      externalValue: 'fulfilled',
      internalValue: 'shipped',
    });
    assert.ok(mapping.id);
    assert.equal(mapping.internalValue, 'shipped');
    assert.equal(mapping.isActive, true);
    assert.ok(mapping.createdAt);
  });

  await t.test('get returns the mapping, and null when missing', async () => {
    const found = await commerce.integrationMappings.get(mapping.id);
    assert.equal(found.id, mapping.id);
    assert.equal(await commerce.integrationMappings.get(randomUUID()), null);
  });

  await t.test('resolve translates an external value', async () => {
    const resolved = await commerce.integrationMappings.resolve({
      integration: 'shopify',
      mappingGroup: 'order_status',
      fieldName: 'status',
      externalValue: 'fulfilled',
    });
    assert.equal(resolved, 'shipped');
  });

  await t.test('update changes the internal value', async () => {
    const updated = await commerce.integrationMappings.update(mapping.id, {
      internalValue: 'delivered',
    });
    assert.equal(updated.internalValue, 'delivered');
  });

  await t.test('bulkUpsert reports rows affected', async () => {
    const affected = await commerce.integrationMappings.bulkUpsert([
      {
        integration: 'amazon',
        mappingGroup: 'order_status',
        fieldName: 'status',
        externalValue: 'Shipped',
        internalValue: 'shipped',
      },
    ]);
    assert.equal(affected, '1');
  });

  await t.test('list supports filtering and pagination', async () => {
    const all = await commerce.integrationMappings.list();
    assert.ok(all.length >= 2);
    const shopify = await commerce.integrationMappings.list({ integration: 'shopify' });
    assert.equal(shopify.length, 1);
    const paged = await commerce.integrationMappings.list({ limit: 1, offset: 0 });
    assert.equal(paged.length, 1);
  });

  await t.test('get rejects a malformed id', async () => {
    await assert.rejects(
      () => commerce.integrationMappings.get('not-a-uuid'),
      /Invalid integration_mapping UUID/,
    );
  });

  await t.test('delete removes the mapping', async () => {
    await commerce.integrationMappings.delete(mapping.id);
    assert.equal(await commerce.integrationMappings.get(mapping.id), null);
  });
});

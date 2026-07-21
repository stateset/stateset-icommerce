/**
 * Integration field-mapping API tests for @stateset/embedded Node.js bindings.
 *
 * Create, get, update, list, bulk create/delete, distinct groups, delete.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { randomUUID } = require('node:crypto');

test('IntegrationFieldMappings: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('API exists and is supported', async () => {
    assert.ok(commerce.integrationFieldMappings, 'integrationFieldMappings API should exist');
    assert.equal(await commerce.integrationFieldMappings.isSupported(), true);
  });

  let mapping;
  await t.test('create defaults the transform to none', async () => {
    mapping = await commerce.integrationFieldMappings.create({
      integrationAccount: 'acct-1',
      mappingGroup: 'order',
      sourceField: 'customer.email',
      destinationField: 'email',
    });
    assert.ok(mapping.id);
    assert.equal(mapping.transform, 'none');
    assert.equal(mapping.isActive, true);
    assert.equal(mapping.template, undefined);
  });

  await t.test('create rejects an unknown transform', async () => {
    await assert.rejects(
      () =>
        commerce.integrationFieldMappings.create({
          integrationAccount: 'acct-1',
          mappingGroup: 'order',
          sourceField: 'x',
          destinationField: 'y',
          transform: 'shout',
        }),
      /Invalid field transform: shout/,
    );
  });

  await t.test('get returns the mapping, and null when missing', async () => {
    const found = await commerce.integrationFieldMappings.get(mapping.id);
    assert.equal(found.id, mapping.id);
    assert.equal(await commerce.integrationFieldMappings.get(randomUUID()), null);
  });

  await t.test('update applies a snake_case transform', async () => {
    const updated = await commerce.integrationFieldMappings.update(mapping.id, {
      transform: 'uppercase',
      fallback: 'unknown@example.com',
    });
    assert.equal(updated.transform, 'uppercase');
    assert.equal(updated.fallback, 'unknown@example.com');
  });

  await t.test('bulkCreate reports rows affected', async () => {
    const affected = await commerce.integrationFieldMappings.bulkCreate([
      {
        integrationAccount: 'acct-1',
        mappingGroup: 'shipment',
        sourceField: 'tracking',
        destinationField: 'tracking_number',
        transform: 'trim',
      },
    ]);
    assert.equal(affected, '1');
  });

  await t.test('list filters by group and paginates', async () => {
    const all = await commerce.integrationFieldMappings.list();
    assert.ok(all.length >= 2);
    const orders = await commerce.integrationFieldMappings.list({ mappingGroup: 'order' });
    assert.equal(orders.length, 1);
    const paged = await commerce.integrationFieldMappings.list({ limit: 1, offset: 0 });
    assert.equal(paged.length, 1);
  });

  await t.test('distinctGroups lists the account groups', async () => {
    const groups = await commerce.integrationFieldMappings.distinctGroups('acct-1');
    assert.deepEqual([...groups].sort(), ['order', 'shipment']);
  });

  await t.test('bulkDelete and delete remove mappings', async () => {
    const others = await commerce.integrationFieldMappings.list({ mappingGroup: 'shipment' });
    assert.equal(await commerce.integrationFieldMappings.bulkDelete([others[0].id]), '1');
    await commerce.integrationFieldMappings.delete(mapping.id);
    assert.equal(await commerce.integrationFieldMappings.get(mapping.id), null);
  });
});

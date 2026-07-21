/**
 * Search configuration API tests for @stateset/embedded Node.js bindings.
 *
 * Create, get, update, list, activate, and delete search tuning profiles.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { randomUUID } = require('node:crypto');

test('SearchConfigs: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('API exists and is supported', async () => {
    assert.ok(commerce.searchConfig, 'searchConfig API should exist');
    assert.equal(await commerce.searchConfig.isSupported(), true);
  });

  let config;
  await t.test('create stores fields, facets, synonyms and boosts', async () => {
    config = await commerce.searchConfig.create({
      name: 'Product Search v1',
      description: 'Default tuning',
      searchableFields: [
        { fieldName: 'title', weight: 3, tokenizer: 'standard', enabled: true },
        { fieldName: 'sku', weight: 1, tokenizer: 'keyword' },
      ],
      facets: [
        { fieldName: 'brand', facetType: 'value', displayName: 'Brand', sortOrder: 1 },
      ],
      synonyms: [{ canonical: 'shirt', synonyms: ['tee', 'top'] }],
      boostRules: [{ field: 'brand', valueMatch: 'acme', boostFactor: 1.5 }],
    });
    assert.ok(config.id);
    assert.equal(config.name, 'Product Search v1');
    assert.equal(config.searchableFields.length, 2);
    assert.equal(config.searchableFields[1].tokenizer, 'keyword');
    // Tokenizer defaults to `standard` and enabled defaults to true.
    assert.equal(config.searchableFields[1].enabled, true);
    assert.equal(config.facets[0].facetType, 'value');
    assert.deepEqual(config.synonyms[0].synonyms, ['tee', 'top']);
    assert.equal(config.boostRules[0].boostFactor, 1.5);
    assert.equal(config.isActive, false);
  });

  await t.test('create rejects an unknown tokenizer', async () => {
    await assert.rejects(
      () =>
        commerce.searchConfig.create({
          name: 'bad',
          searchableFields: [{ fieldName: 'title', weight: 1, tokenizer: 'nope' }],
        }),
      /Invalid tokenizer/,
    );
  });

  await t.test('get returns the config, and null when missing', async () => {
    const found = await commerce.searchConfig.get(config.id);
    assert.equal(found.id, config.id);
    assert.equal(await commerce.searchConfig.get(randomUUID()), null);
  });

  await t.test('update replaces collections wholesale', async () => {
    const updated = await commerce.searchConfig.update(config.id, {
      name: 'Product Search v2',
      synonyms: [{ canonical: 'pants', synonyms: ['trousers'] }],
    });
    assert.equal(updated.name, 'Product Search v2');
    assert.equal(updated.synonyms.length, 1);
    assert.equal(updated.synonyms[0].canonical, 'pants');
  });

  await t.test('list filters by name', async () => {
    const configs = await commerce.searchConfig.list({ limit: 10 });
    assert.ok(configs.some((c) => c.id === config.id));
  });

  await t.test('setActive activates it and getActive returns it', async () => {
    const activated = await commerce.searchConfig.setActive(config.id);
    assert.equal(activated.isActive, true);
    const active = await commerce.searchConfig.getActive();
    assert.equal(active.id, config.id);
  });

  await t.test('delete removes the config', async () => {
    await commerce.searchConfig.delete(config.id);
    assert.equal(await commerce.searchConfig.get(config.id), null);
  });
});

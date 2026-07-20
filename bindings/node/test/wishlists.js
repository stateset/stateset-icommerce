/**
 * Wishlists API tests for @stateset/embedded Node.js bindings.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

test('Wishlists: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('wishlists API exists and is supported', async () => {
    assert.ok(commerce.wishlists, 'wishlists API should exist');
    assert.equal(await commerce.wishlists.isSupported(), true);
  });

  const customer = await commerce.customers.create({
    email: 'wisher@example.com',
    firstName: 'Wish',
    lastName: 'Er',
  });
  const product = await commerce.products.create({
    name: 'Wished Widget',
    variants: [{ sku: 'WISH-001', name: 'Default', price: 9.99 }],
  });

  let wishlist;
  await t.test('create returns an empty wishlist', async () => {
    wishlist = await commerce.wishlists.create({
      customerId: customer.id,
      name: 'Holiday picks',
      isPublic: true,
    });
    assert.equal(wishlist.customerId, customer.id);
    assert.equal(wishlist.name, 'Holiday picks');
    assert.equal(wishlist.isPublic, true);
    assert.deepEqual(wishlist.items, []);
    assert.ok(wishlist.id);
  });

  await t.test('addItem places a product on the list', async () => {
    const item = await commerce.wishlists.addItem(wishlist.id, {
      productId: product.id,
      quantity: 2,
      note: 'the blue one',
      priority: 1,
    });
    assert.equal(item.productId, product.id);
    assert.equal(item.quantity, 2);
    assert.equal(item.note, 'the blue one');
    assert.equal(item.priority, 1);

    const fetched = await commerce.wishlists.get(wishlist.id);
    assert.equal(fetched.items.length, 1);
    assert.equal(fetched.items[0].productId, product.id);
  });

  await t.test('update renames and unpublishes the list', async () => {
    const updated = await commerce.wishlists.update(wishlist.id, {
      name: 'Renamed',
      isPublic: false,
    });
    assert.equal(updated.name, 'Renamed');
    assert.equal(updated.isPublic, false);
  });

  await t.test('list finds the customer\'s wishlists', async () => {
    const lists = await commerce.wishlists.list({ customerId: customer.id });
    assert.ok(lists.some((w) => w.id === wishlist.id));
  });

  await t.test('removeItem takes the product back off the list', async () => {
    await commerce.wishlists.removeItem(wishlist.id, product.id);
    const fetched = await commerce.wishlists.get(wishlist.id);
    assert.equal(fetched.items.length, 0);
  });

  await t.test('delete removes the wishlist', async () => {
    await commerce.wishlists.delete(wishlist.id);
    const gone = await commerce.wishlists.get(wishlist.id);
    assert.equal(gone, null);
  });
});

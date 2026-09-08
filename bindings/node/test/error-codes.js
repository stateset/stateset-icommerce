/**
 * Machine-readable error codes at the Node binding boundary.
 *
 * Every error thrown by the native module carries a stable `err.code` that
 * callers may branch on. The human sentence stays in `err.message`; the napi
 * status that produced it stays in `err.napiStatus`.
 */

const assert = require('assert');
const { test } = require('node:test');

const native = require('../index.js');
const { Commerce } = native;

const MISSING_UUID = '11111111-1111-4111-8111-111111111111';

async function rejects(fn) {
  try {
    await fn();
  } catch (error) {
    return error;
  }
  throw new assert.AssertionError({ message: 'expected the call to reject' });
}

test('binding errors carry stable machine codes', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('VALIDATION for an unparsable id', async () => {
    const error = await rejects(() => commerce.checkoutSnapshot('not-a-uuid'));
    assert.strictEqual(error.code, 'VALIDATION');
    assert.strictEqual(error.message, 'Invalid cart UUID');
    assert.strictEqual(
      error.napiStatus,
      'InvalidArg',
      'validation failures map to the InvalidArg napi status',
    );
    assert.ok(error instanceof Error, 'still a real Error');
  });

  await t.test('NOT_FOUND for a get-by-id miss', async () => {
    const error = await rejects(() => commerce.checkoutSnapshot(MISSING_UUID));
    assert.strictEqual(error.code, 'NOT_FOUND');
    assert.strictEqual(error.message, 'Cart not found');
  });

  await t.test('NOT_FOUND is derived from the CommerceError variant', async () => {
    const error = await rejects(() =>
      commerce.inventory.reserve('NO-SUCH-SKU', 1, 'order', MISSING_UUID),
    );
    assert.strictEqual(error.code, 'NOT_FOUND');
    assert.match(error.message, /Inventory item not found: NO-SUCH-SKU$/);
    assert.strictEqual(error.details.httpStatus, 404);
  });

  await t.test('CONFLICT for a duplicate create', async () => {
    await commerce.inventory.createItem({ sku: 'DUP-1', name: 'Dup', initialQuantity: 5 });
    const error = await rejects(() =>
      commerce.inventory.createItem({ sku: 'DUP-1', name: 'Dup', initialQuantity: 5 }),
    );
    assert.strictEqual(error.code, 'CONFLICT');
    assert.match(error.message, /Duplicate SKU: DUP-1$/);
    assert.strictEqual(error.details.httpStatus, 409);
  });

  await t.test('CONFLICT for a duplicate product slug', async () => {
    await commerce.products.create({ name: 'P', sku: 'P-1', price: 10, slug: 'p-1' });
    const error = await rejects(() =>
      commerce.products.create({ name: 'P', sku: 'P-2', price: 10, slug: 'p-1' }),
    );
    assert.strictEqual(error.code, 'CONFLICT');
  });

  await t.test('INSUFFICIENT_STOCK keeps its published invariant code', async () => {
    const error = await rejects(() =>
      commerce.inventory.reserve('DUP-1', 9999, 'order', MISSING_UUID),
    );
    assert.strictEqual(error.code, 'INSUFFICIENT_STOCK');
    assert.strictEqual(error.details.invariant, 'commerce.inventory.insufficient_available');
    assert.match(error.message, /requested 9999, available 5$/);
  });

  // Slow on purpose: an unopenable path fails through the connection pool, which
  // gives up after its 30s timeout. Not a hang.
  await t.test('constructor failures are coded too', async () => {
    const error = await rejects(() => new Commerce('/nonexistent-dir-xyz/commerce.db'));
    assert.ok(
      ['DATABASE', 'INTERNAL'].includes(error.code),
      `constructor failure should be coded, got ${error.code}`,
    );
    assert.ok(!error.message.startsWith('{'), 'the JSON envelope must not leak into the message');
  });

  await t.test('the raw envelope never reaches callers', async () => {
    const error = await rejects(() => commerce.checkoutSnapshot('nope'));
    assert.ok(!error.message.includes('"code"'), error.message);
    assert.ok(error.stack.startsWith('Error: Invalid cart UUID'), error.stack.split('\n')[0]);
  });
});

test('index.js re-exports everything the native binding exposes', () => {
  const raw = require('../native-binding.js');
  const missing = Object.keys(raw).filter((key) => !(key in native));
  assert.deepStrictEqual(missing, [], `index.js is missing exports: ${missing.join(', ')}`);
});

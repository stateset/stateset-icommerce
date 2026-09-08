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
    assert.strictEqual(error.details.httpStatus, 400);
    // The numbers come out of `details`, not out of the sentence.
    assert.strictEqual(error.details.sku, 'DUP-1');
    assert.strictEqual(error.details.requested, '9999');
    assert.strictEqual(error.details.available, '5');
  });

  await t.test('the raw envelope never reaches callers', async () => {
    const error = await rejects(() => commerce.checkoutSnapshot('nope'));
    assert.ok(!error.message.includes('"code"'), error.message);
    assert.ok(error.stack.startsWith('Error: Invalid cart UUID'), error.stack.split('\n')[0]);
  });
});

test('PRECONDITION_FAILED for a rule the record state forbids', async () => {
  const commerce = new Commerce(':memory:');
  const customer = await commerce.customers.create({
    email: 'precondition@example.com',
    firstName: 'Pre',
    lastName: 'Condition',
  });
  await commerce.inventory.createItem({ sku: 'PRE-1', name: 'Pre', initialQuantity: 10 });
  const order = await commerce.orders.create({
    customerId: customer.id,
    items: [{ sku: 'PRE-1', name: 'Pre', quantity: 1, unitPrice: 5 }],
  });
  await commerce.orders.cancel(order.id, 'test');

  const error = await rejects(() => commerce.orders.updateStatus(order.id, 'delivered'));
  // Before the explicit arm this reported INTERNAL — "the binding is broken"
  // for what is really "the order is already cancelled".
  assert.strictEqual(error.code, 'PRECONDITION_FAILED');
  assert.strictEqual(error.details.httpStatus, 400);
  assert.match(error.message, /Invalid order status transition from cancelled to delivered$/);
});

test('every wrapper path decodes the envelope', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('free function', async () => {
    const error = await rejects(() => native.jcsCanonicalize('{not json'));
    assert.strictEqual(error.code, 'VALIDATION');
    assert.strictEqual(error.napiStatus, 'InvalidArg');
    assert.match(error.message, /^Invalid JSON: /);
  });

  // Slow on purpose: an unopenable path fails through the connection pool, which
  // gives up after its 30s timeout. Not a hang.
  await t.test('constructor', async () => {
    const error = await rejects(() => new Commerce('/nonexistent-dir-xyz/commerce.db'));
    assert.ok(['DATABASE', 'INTERNAL'].includes(error.code), error.code);
    assert.ok(!error.message.startsWith('{'), 'the JSON envelope must not leak');
  });

  await t.test('async method', async () => {
    const error = await rejects(() => commerce.checkoutSnapshot(MISSING_UUID));
    assert.strictEqual(error.code, 'NOT_FOUND');
  });

  await t.test('getter chain stays wrapped', async () => {
    // `commerce.tax` is a getter; the instance it returns must still be a real
    // Tax (the static shim rewires `prototype`, so this is the regression guard)
    // and its methods must still decode.
    assert.ok(commerce.tax instanceof native.Tax, 'getter result keeps its identity');
    const error = await rejects(() => commerce.customers.get('not-a-uuid'));
    assert.strictEqual(error.code, 'VALIDATION');
  });

  await t.test('static method is wrapped, not the raw native one', () => {
    const raw = require('../native-binding.js');
    assert.notStrictEqual(
      native.Tax.getUsStateInfo,
      raw.Tax.getUsStateInfo,
      'statics must go through the decoding shim',
    );
    // ...and still work.
    assert.strictEqual(native.Tax.isEuCountry('DE'), true);
    assert.strictEqual(native.Tax.isEuCountry('US'), false);
    assert.ok(native.Tax.getUsStateInfo('CA'), 'a known state still resolves');
    assert.strictEqual(native.Tax.getUsStateInfo('ZZ'), null);
  });
});

// A commerce binding has no business mutating the host's realm. `adopt` used to
// patch the prototype of WHATEVER a native call handed back, and several
// exports return Buffers (`merkleRoot`, `ed25519Sign`, `aesGcmEncrypt`,
// `vesX402ComputeSigningHash`, …): one call replaced all 93 own methods of
// `Buffer.prototype`, process-wide, for every library sharing the process.
test('a Buffer-returning export leaves the realm alone', async (t) => {
  const { wrapNativeExports } = require('../errors.js');

  const before = {
    toString: Buffer.prototype.toString,
    slice: Buffer.prototype.slice,
    equals: Buffer.prototype.equals,
    names: Object.getOwnPropertyNames(Buffer.prototype).sort(),
    uint8ArrayNames: Object.getOwnPropertyNames(Uint8Array.prototype).sort(),
    objectToString: Object.prototype.toString,
    arrayMap: Array.prototype.map,
    promiseThen: Promise.prototype.then,
    dateToISOString: Date.prototype.toISOString,
  };

  const assertRealmUntouched = (what) => {
    assert.strictEqual(Buffer.prototype.toString, before.toString, `${what}: Buffer#toString`);
    assert.strictEqual(Buffer.prototype.slice, before.slice, `${what}: Buffer#slice`);
    assert.strictEqual(Buffer.prototype.equals, before.equals, `${what}: Buffer#equals`);
    assert.deepStrictEqual(
      Object.getOwnPropertyNames(Buffer.prototype).sort(),
      before.names,
      `${what}: Buffer.prototype own property names`,
    );
    assert.deepStrictEqual(
      Object.getOwnPropertyNames(Uint8Array.prototype).sort(),
      before.uint8ArrayNames,
      `${what}: Uint8Array.prototype own property names`,
    );
    assert.strictEqual(Object.prototype.toString, before.objectToString, `${what}: Object`);
    assert.strictEqual(Array.prototype.map, before.arrayMap, `${what}: Array`);
    assert.strictEqual(Promise.prototype.then, before.promiseThen, `${what}: Promise`);
    assert.strictEqual(Date.prototype.toISOString, before.dateToISOString, `${what}: Date`);
  };

  await t.test('through a stand-in module returning builtins', () => {
    const fake = wrapNativeExports({
      makeBuffer: () => Buffer.from('deadbeef', 'hex'),
      makeTypedArray: () => new Uint8Array([1, 2, 3]),
      makeArrayBuffer: () => new ArrayBuffer(8),
      makeDataView: () => new DataView(new ArrayBuffer(8)),
      makeDate: () => new Date(0),
      makeMap: () => new Map([['a', 1]]),
      makeRegExp: () => /x/u,
      makeError: () => new TypeError('not thrown, returned'),
      makePromise: async () => Buffer.alloc(4),
    });
    for (const key of Object.keys(fake)) {
      const value = fake[key]();
      assert.ok(value, `${key} returned something`);
    }
    return fake.makePromise().then(() => assertRealmUntouched('stand-in module'));
  });

  await t.test('through the real binding', () => {
    // `merkleRoot` is a free function on the native module and hands back a
    // Buffer, which is the exact shape that used to poison the prototype.
    const root = native.merkleRoot([Buffer.alloc(32), Buffer.alloc(32)]);
    assert.ok(Buffer.isBuffer(root) || root instanceof Uint8Array, 'merkleRoot returns bytes');
    assertRealmUntouched('real binding');

    // And the Buffer still behaves like a Buffer, not like a wrapped shell.
    assert.strictEqual(Buffer.from('ab', 'hex').toString('hex'), 'ab');
  });
});

// The native module has no fallible synchronous instance method and no static
// that can be made to throw with ordinary input, so the sync method / getter /
// static decode paths are proved here against a stand-in module shaped exactly
// like napi's output: non-configurable statics, prototype methods, getters.
test('wrapNativeExports decodes every shape napi produces', async (t) => {
  const { wrapNativeExports } = require('../errors.js');

  const envelope = (code, message, details) =>
    JSON.stringify(details ? { code, message, details } : { code, message });

  function thrower(code, message, details) {
    const error = new Error(envelope(code, message, details));
    error.code = 'GenericFailure';
    return error;
  }

  class Fake {
    constructor(fail) {
      if (fail) throw thrower('DATABASE', 'boom ctor');
    }
    syncMethod() {
      throw thrower('CONFLICT', 'boom sync', { httpStatus: 409 });
    }
    async asyncMethod() {
      throw thrower('NOT_FOUND', 'boom async');
    }
    get failingGetter() {
      throw thrower('POLICY_REJECTED', 'boom getter');
    }
  }
  Object.defineProperty(Fake, 'staticMethod', {
    value: () => {
      throw thrower('VALIDATION', 'boom static');
    },
    writable: false,
    enumerable: false,
    configurable: false,
  });
  const freeFn = () => {
    throw thrower('EXTERNAL_SERVICE', 'boom free');
  };

  const wrapped = wrapNativeExports({ Fake: Fake, freeFn: freeFn });
  const instance = new wrapped.Fake(false);

  await t.test('constructor', () => {
    const error = (() => {
      try {
        // eslint-disable-next-line no-new
        new wrapped.Fake(true);
      } catch (caught) {
        return caught;
      }
      throw new assert.AssertionError({ message: 'expected a throw' });
    })();
    assert.strictEqual(error.code, 'DATABASE');
    assert.strictEqual(error.message, 'boom ctor');
  });

  await t.test('sync method', async () => {
    const error = await rejects(() => instance.syncMethod());
    assert.strictEqual(error.code, 'CONFLICT');
    assert.strictEqual(error.message, 'boom sync');
    assert.strictEqual(error.details.httpStatus, 409);
    assert.strictEqual(error.napiStatus, 'GenericFailure');
  });

  await t.test('async method', async () => {
    const error = await rejects(() => instance.asyncMethod());
    assert.strictEqual(error.code, 'NOT_FOUND');
    assert.strictEqual(error.message, 'boom async');
  });

  await t.test('getter', async () => {
    const error = await rejects(() => instance.failingGetter);
    assert.strictEqual(error.code, 'POLICY_REJECTED');
    assert.strictEqual(error.message, 'boom getter');
  });

  await t.test('non-configurable static', async () => {
    const error = await rejects(() => wrapped.Fake.staticMethod());
    assert.strictEqual(error.code, 'VALIDATION');
    assert.strictEqual(error.message, 'boom static');
  });

  await t.test('free function', async () => {
    const error = await rejects(() => wrapped.freeFn());
    assert.strictEqual(error.code, 'EXTERNAL_SERVICE');
    assert.strictEqual(error.message, 'boom free');
  });

  await t.test('identity survives the static shim', () => {
    assert.ok(instance instanceof wrapped.Fake, 'instanceof still holds');
    assert.ok(instance instanceof Fake, 'and against the raw class');
    assert.strictEqual(wrapped.Fake.name, 'Fake');
  });
});

test('index.js re-exports everything the native binding exposes', () => {
  const raw = require('../native-binding.js');
  const missing = Object.keys(raw).filter((key) => !(key in native));
  assert.deepStrictEqual(missing, [], `index.js is missing exports: ${missing.join(', ')}`);
});

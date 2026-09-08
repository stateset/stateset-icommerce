/**
 * Panic containment at the Node binding boundary.
 *
 * The workspace release profile is `panic = "abort"`, so a panic reached from a
 * `#[napi]` call would take the host Node process down. `[profile.release-node]`
 * keeps every release optimisation but restores `panic = "unwind"`, and the
 * guards in `bindings/node/src/errors.rs` turn the unwind into a JavaScript
 * error with `code: 'INTERNAL_PANIC'`.
 *
 * `__testPanic` / `__testPanicAsync` are compiled only under `debug_assertions`,
 * so this file skips against a release binary.
 */

const assert = require('assert');
const { test } = require('node:test');

const native = require('../index.js');
const { Commerce } = native;

async function rejects(fn) {
  try {
    await fn();
  } catch (error) {
    return error;
  }
  throw new assert.AssertionError({ message: 'expected the call to reject' });
}

test('panics inside the native module become JS errors', async (t) => {
  if (typeof native.__testPanic !== 'function') {
    t.skip('binary built without debug assertions — no panic probe compiled in');
    return;
  }

  await t.test('a synchronous panic', async () => {
    const error = await rejects(() => native.__testPanic('boom-sync'));
    assert.strictEqual(error.code, 'INTERNAL_PANIC');
    assert.match(error.message, /boom-sync/);
  });

  await t.test('a panic inside an async entry point', async () => {
    const error = await rejects(() => native.__testPanicAsync('boom-async'));
    assert.strictEqual(error.code, 'INTERNAL_PANIC');
    assert.match(error.message, /boom-async/);
  });

  await t.test('the process survives a panic', async () => {
    await rejects(() => native.__testPanic('still-alive'));
    const commerce = new Commerce(':memory:');
    const item = await commerce.inventory.createItem({
      sku: 'AFTER-PANIC',
      name: 'After panic',
      initialQuantity: 1,
    });
    assert.strictEqual(item.sku, 'AFTER-PANIC');
  });
});

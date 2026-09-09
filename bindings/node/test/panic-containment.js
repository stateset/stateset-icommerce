/**
 * Panic containment at the Node binding boundary.
 *
 * The workspace release profile is `panic = "abort"`, so a panic reached from a
 * `#[napi]` call would take the host Node process down. `[profile.release-node]`
 * keeps every release optimisation but restores `panic = "unwind"`, and the
 * guards in `bindings/node/src/errors.rs` turn the unwind into a JavaScript
 * error with `code: 'INTERNAL_PANIC'`.
 *
 * The async entry points have a second net underneath that one:
 * `napi::tokio_runtime::execute_tokio_future` watches every spawned task and
 * rejects the promise with `Status::GenericFailure` when it panics, so all 728
 * async methods already fail soft — they just carry no code until `decorate()`
 * in `errors.js` supplies one. Both paths are asserted here.
 *
 * `__testPanic*` are compiled only with the `test-panic` cargo feature, which
 * `npm run build:debug` turns on; this file skips against a build without it.
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

  await t.test('an UNGUARDED async entry point still reports INTERNAL_PANIC', async () => {
    // No `guard_async` here — the rejection comes from napi's own task watchdog
    // and reaches JavaScript as a bare `GenericFailure`. `decorate()` infers the
    // code from the fact that a rejected GenericFailure without an envelope can
    // only be a contained panic.
    const error = await rejects(() => native.__testPanicAsyncUnguarded());
    assert.strictEqual(error.code, 'INTERNAL_PANIC');
    assert.strictEqual(error.napiStatus, 'GenericFailure');
    assert.strictEqual(error.details.contained, 'napi-async-watchdog');
    // napi forwards a `&'static str` payload verbatim and replaces anything else
    // with a fixed sentence; either is acceptable, an envelope is not.
    assert.ok(
      error.message === 'unguarded async panic' || error.message === 'Panic in async function',
      error.message,
    );
    assert.ok(!error.message.startsWith('{'), 'no envelope should leak');
  });

  await t.test('a normal async rejection is NOT mistaken for a panic', async () => {
    const commerce = new Commerce(':memory:');
    const error = await rejects(() =>
      commerce.checkoutSnapshot('11111111-1111-4111-8111-111111111111'),
    );
    assert.strictEqual(error.code, 'NOT_FOUND');
  });

  await t.test('the process survives a panic', async () => {
    await rejects(() => native.__testPanic('still-alive'));
    await rejects(() => native.__testPanicAsyncUnguarded());
    const commerce = new Commerce(':memory:');
    const item = await commerce.inventory.createItem({
      sku: 'AFTER-PANIC',
      name: 'After panic',
      initialQuantity: 1,
    });
    assert.strictEqual(item.sku, 'AFTER-PANIC');
  });
});

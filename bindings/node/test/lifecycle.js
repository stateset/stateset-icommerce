/**
 * Instance lifecycle: asynchronous open, stable sub-API identity, and an
 * explicit close that releases the engine and refuses further calls.
 */

'use strict';

const assert = require('assert');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { test } = require('node:test');
const { Commerce } = require('../index.js');

test('Commerce.open resolves to a working instance without blocking the constructor', async () => {
  const commerce = await Commerce.open(':memory:');
  assert.ok(commerce instanceof Commerce);
  assert.strictEqual(await commerce.customers.count(), 0);
});

test('Commerce.open accepts options', async () => {
  const commerce = await Commerce.open(':memory:', { maxConnections: 2 });
  assert.strictEqual(await commerce.customers.count(), 0);
});

test('Commerce.open rejects with a coded error for an unusable path', async () => {
  await assert.rejects(
    Commerce.open(path.join(os.tmpdir(), 'does-not-exist-' + process.pid, 'nested', 'store.db')),
    (err) => typeof err.code === 'string' && err.code !== 'GenericFailure',
  );
});

test('sub-API getters return the same object on every access', async () => {
  const commerce = new Commerce(':memory:');
  assert.strictEqual(commerce.orders, commerce.orders);
  assert.strictEqual(commerce.customers, commerce.customers);
  assert.strictEqual(commerce.events, commerce.events);
  const other = new Commerce(':memory:');
  assert.notStrictEqual(commerce.orders, other.orders);
});

test('close() refuses further calls with PRECONDITION_FAILED and is idempotent', async () => {
  const commerce = new Commerce(':memory:');
  const customers = commerce.customers;
  await customers.create({ email: 'c@example.com', firstName: 'C', lastName: 'L' });
  await commerce.close();
  await commerce.close();
  await assert.rejects(customers.count(), (err) => err.code === 'PRECONDITION_FAILED' && /closed/i.test(err.message));
  await assert.rejects(commerce.orders.list(), (err) => err.code === 'PRECONDITION_FAILED');
  assert.strictEqual(commerce.isClosed, true);
});

test('close() releases a file database so it can be reopened', async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-lifecycle-'));
  const file = path.join(dir, 'store.db');
  const first = await Commerce.open(file);
  await first.customers.create({ email: 'f@example.com', firstName: 'F', lastName: 'L' });
  await first.close();
  const second = await Commerce.open(file);
  assert.strictEqual(await second.customers.count(), 1);
  await second.close();
  fs.rmSync(dir, { recursive: true, force: true });
});

test('close() resolves only after the database files are released', async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-close-release-'));
  const file = path.join(dir, 'store.db');
  const wal = `${file}-wal`;
  const shm = `${file}-shm`;
  const writes = 600;
  try {
    const first = await Commerce.open(file);
    for (let batch = 0; batch < writes / 50; batch++) {
      await Promise.all(
        Array.from({ length: 50 }, (_, i) =>
          first.customers.create({ email: `w${batch}-${i}@example.com`, firstName: 'W', lastName: 'L' }),
        ),
      );
    }
    // The engine checkpoints only every 10000 pages, so the writes are still
    // in the WAL: the main file alone does not hold them yet.
    assert.ok(fs.existsSync(wal) && fs.statSync(wal).size > 0, 'precondition: the writes left a WAL');

    // Both closes (idempotent) resolve only after the engine has dropped.
    await Promise.all([first.close(), first.close()]);

    // Closing the last connection checkpoints the WAL into the main file and
    // removes the WAL and shared-memory files — before close() resolved.
    assert.strictEqual(fs.existsSync(wal), false, 'WAL still present after close() resolved');
    assert.strictEqual(fs.existsSync(shm), false, 'shared-memory file still present after close() resolved');
    await first.close();
    await assert.rejects(first.customers.count(), (err) => err.code === 'PRECONDITION_FAILED');

    // So the main file alone is now complete: a byte copy taken immediately
    // holds every write, and the original reopens in this same process.
    const copy = path.join(dir, 'copy.db');
    fs.copyFileSync(file, copy);
    const fromCopy = await Commerce.open(copy);
    assert.strictEqual(await fromCopy.customers.count(), writes);
    await fromCopy.close();
    const reopened = await Commerce.open(file);
    assert.strictEqual(await reopened.customers.count(), writes);
    await reopened.close();
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test('close() waits for in-flight writes and releases the file after them', async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-close-inflight-'));
  const file = path.join(dir, 'store.db');
  try {
    const commerce = await Commerce.open(file);
    const inFlight = Promise.allSettled(
      Array.from({ length: 20 }, (_, i) =>
        commerce.customers.create({ email: `i${i}@example.com`, firstName: 'I', lastName: 'F' }),
      ),
    );
    await commerce.close();
    // Nothing may still hold the file: every write that will ever land has
    // been checkpointed into the main database.
    assert.strictEqual(fs.existsSync(`${file}-wal`), false, 'WAL still present after close() resolved');
    let created = 0;
    for (const outcome of await inFlight) {
      if (outcome.status === 'fulfilled') created += 1;
      // A call that had not yet taken the engine when close() emptied the slot.
      else assert.strictEqual(outcome.reason.code, 'PRECONDITION_FAILED', outcome.reason.message);
    }
    const reopened = await Commerce.open(file);
    assert.strictEqual(await reopened.customers.count(), created);
    await reopened.close();
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test('Symbol.asyncDispose closes the instance', async () => {
  const commerce = new Commerce(':memory:');
  assert.strictEqual(typeof commerce[Symbol.asyncDispose], 'function');
  await commerce[Symbol.asyncDispose]();
  assert.strictEqual(commerce.isClosed, true);
});

test('calls racing close() either complete or reject with PRECONDITION_FAILED', async () => {
  const commerce = new Commerce(':memory:');
  const pending = Promise.allSettled(
    Array.from({ length: 20 }, (_, i) =>
      commerce.customers.create({ email: `p${i}@example.com`, firstName: 'P', lastName: 'L' }),
    ),
  );
  await commerce.close();
  const outcomes = await pending;
  for (const outcome of outcomes) {
    if (outcome.status === 'fulfilled') continue;
    assert.strictEqual(outcome.reason.code, 'PRECONDITION_FAILED', outcome.reason.message);
  }
  assert.strictEqual(commerce.isClosed, true);
});

test('index.d.ts carries the hand-written surface (postbuild-types ran)', () => {
  const declarations = fs.readFileSync(path.join(__dirname, '..', 'index.d.ts'), 'utf8');
  assert.match(declarations, /BEGIN hand-written additions/);
  assert.match(declarations, /interface CommerceEventSubscription extends AsyncIterable<CommerceEvent>/);
  assert.match(declarations, /recv\(\): Promise<CommerceEvent \| null>/);
  assert.match(declarations, /open\(dbPath: string, options\?: OpenOptions \| undefined \| null\): Promise<Commerce>/);
});

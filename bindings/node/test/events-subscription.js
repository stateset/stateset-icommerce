/**
 * Event subscriptions: async iteration, explicit close, and no lingering
 * process at exit.
 */

'use strict';

const assert = require('assert');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { test } = require('node:test');
const { Commerce } = require('../index.js');

test('subscription is async-iterable and yields typed events', async () => {
  const commerce = new Commerce(':memory:');
  const subscription = await commerce.events.subscribeFiltered(['customer_created']);
  const seen = [];
  const consumer = (async () => {
    for await (const event of subscription) {
      seen.push(event.event_type);
      if (seen.length === 2) break;
    }
  })();
  await commerce.customers.create({ email: 'e1@example.com', firstName: 'E', lastName: 'L' });
  await commerce.customers.create({ email: 'e2@example.com', firstName: 'E', lastName: 'L' });
  await consumer;
  assert.deepStrictEqual(seen, ['customer_created', 'customer_created']);
  assert.strictEqual(subscription.isClosed, true, 'breaking out of the loop closes the subscription');
});

test('subscription.close() resolves a pending recv() with null', async () => {
  const commerce = new Commerce(':memory:');
  const subscription = await commerce.events.subscribe();
  const pending = subscription.recv();
  subscription.close();
  assert.strictEqual(await pending, null);
  assert.strictEqual(await subscription.recv(), null);
});

test('closing the Commerce ends its subscriptions', async () => {
  const commerce = new Commerce(':memory:');
  const subscription = await commerce.events.subscribe();
  const pending = subscription.recv();
  await commerce.close();
  assert.strictEqual(await pending, null);
});

function exitTimeOf(body) {
  const script = `
    const { Commerce } = require(${JSON.stringify(path.join(__dirname, '..', 'index.js'))});
    (async () => {
      const commerce = new Commerce(':memory:');
      const subscription = await commerce.events.subscribe();
      ${body}
      console.log('READY');
    })();
  `;
  const started = Date.now();
  const result = spawnSync(process.execPath, ['-e', script], { encoding: 'utf8', timeout: 20000 });
  return { elapsed: Date.now() - started, result };
}

test('an open subscription with no recv() outstanding does not hold the process open', () => {
  const { elapsed, result } = exitTimeOf('');
  assert.strictEqual(result.status, 0, result.stderr);
  assert.match(result.stdout, /READY/);
  assert.ok(elapsed < 6000, `process took ${elapsed}ms to exit after its work was done`);
});

test('an awaited recv() keeps the process alive until an event arrives', () => {
  const { elapsed, result } = exitTimeOf(`
      const next = subscription.recv();
      setTimeout(() => commerce.customers.create({ email: 'x@example.com', firstName: 'X', lastName: 'Y' }), 300).unref();
      const event = await next;
      console.log('EVENT ' + event.event_type);
  `);
  assert.strictEqual(result.status, 0, result.stderr);
  assert.match(result.stdout, /EVENT customer_created/);
  assert.ok(elapsed < 6000, `process took ${elapsed}ms to exit`);
});

test('close() releases a pending recv() so the process exits promptly', () => {
  const { elapsed, result } = exitTimeOf(`
      const next = subscription.recv();
      subscription.close();
      console.log('NEXT ' + (await next));
  `);
  assert.strictEqual(result.status, 0, result.stderr);
  assert.match(result.stdout, /NEXT null/);
  assert.ok(elapsed < 6000, `process took ${elapsed}ms to exit after close()`);
});

test('unref() lets the process exit even with a recv() outstanding', () => {
  const { elapsed, result } = exitTimeOf(`
      subscription.recv();
      subscription.unref();
  `);
  assert.strictEqual(result.status, 0, result.stderr);
  assert.match(result.stdout, /READY/);
  assert.ok(elapsed < 6000, `process took ${elapsed}ms to exit after unref()`);
});

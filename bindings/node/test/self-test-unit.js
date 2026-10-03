const assert = require('node:assert/strict');
const { test } = require('node:test');
const { spawnSync } = require('node:child_process');
const { copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } = require('node:fs');
const os = require('node:os');
const path = require('node:path');

function fakeCommerce({ failAt, wrongAmount = false, duplicatePayment = false } = {}) {
  const calls = [];
  let closed = false;
  let paymentStatus = 'pending';
  let orderPaymentStatus = 'pending';
  let refunded = '0';
  const call = async (name, input, operation) => {
    calls.push({ name, input });
    if (name === failAt) throw Object.assign(new Error(`Failed ${name}`), { code: 'TEST_FAILURE' });
    return operation();
  };
  let paymentCreated = false;
  const payment = () => ({
    id: 'payment',
    amountExact: wrongAmount ? '0.31' : '0.30',
    currency: 'USD',
    status: paymentStatus,
    amountRefundedExact: refunded,
  });
  const instance = {
    get isClosed() {
      return closed;
    },
    customers: { create: (input) => call('customer', input, () => ({ id: 'customer' })) },
    orders: {
      createExact: (input) =>
        call('order', input, () => ({ id: 'order', totalAmountExact: '0.30' })),
      get: (id) => call('get_order', id, () => ({ paymentStatus: orderPaymentStatus })),
    },
    payments: {
      createExact: (input) =>
        call('payment', input, () => {
          if (input.amount !== '0.30')
            throw Object.assign(new Error('Changed amount'), { code: 'CONFLICT' });
          if (duplicatePayment && paymentCreated) return { ...payment(), id: 'duplicate' };
          paymentCreated = true;
          return payment();
        }),
      count: () => call('count', null, () => 1),
      get: (id) => call('get_payment', id, payment),
      markCompleted: (id) =>
        call('complete_payment', id, () => {
          paymentStatus = 'completed';
          orderPaymentStatus = 'paid';
          return payment();
        }),
      createRefundExact: (input) =>
        call('refund', input, () => ({
          id: 'refund',
          status: 'pending',
          amountExact: '0.30',
          currency: 'USD',
        })),
      getRefunds: (id) => call('get_refunds', id, () => [{ id: 'refund' }]),
      completeRefund: (id) =>
        call('complete_refund', id, () => {
          paymentStatus = 'refunded';
          orderPaymentStatus = 'refunded';
          refunded = '0.30';
          return { status: 'completed' };
        }),
    },
    close: () =>
      call('close', null, () => {
        closed = true;
      }),
  };
  return {
    calls,
    instance,
    loadCommerce: async () => ({ open: (db) => call('open', db, () => instance) }),
  };
}

test('offline check uses only memory, exact amounts, stable retries, and always closes', async () => {
  const { runEmbeddedCheck } = await import('../self-test.mjs');
  const fake = fakeCommerce();
  const report = await runEmbeddedCheck(fake);
  assert.equal(report.ok, true);
  assert.equal(report.schemaVersion, 1);
  assert.equal(report.scope, 'local-engine-only');
  assert.equal(report.database, ':memory:');
  assert.equal(report.externalSettlementVerified, false);
  assert.equal(fake.calls[0].input, ':memory:');
  assert.equal(fake.instance.isClosed, true);
  const payments = fake.calls.filter(({ name }) => name === 'payment');
  assert.deepEqual(payments[0].input, payments[1].input);
  assert.equal(payments[0].input.amount, '0.30');
  assert.equal(payments[0].input.paymentMethod, 'other');
  const order = fake.calls.find(({ name }) => name === 'order').input;
  assert.equal(order.items[0].unitPrice, '0.10');
  assert.equal(order.items[0].quantity, 3);
  assert.equal(fake.calls.filter(({ name }) => name === 'complete_refund').length, 2);
  assert.deepEqual(
    report.checks.map(({ id }) => id),
    [
      'native_binding',
      'database',
      'order',
      'payment',
      'payment_idempotency',
      'payment_completion',
      'refund',
      'refund_idempotency',
      'refund_completion',
      'cleanup',
    ],
  );
});

test('native loading failures are actionable and serializable', async () => {
  const { runEmbeddedCheck } = await import('../self-test.mjs');
  for (const code of ['MODULE_NOT_FOUND', 'ERR_DLOPEN_FAILED']) {
    const report = await runEmbeddedCheck({
      loadCommerce: async () => {
        throw Object.assign(new Error('Native binding unavailable'), { code });
      },
    });
    assert.equal(report.ok, false);
    assert.equal(report.checks.length, 1);
    assert.equal(report.checks[0].id, 'native_binding');
    assert.equal(report.checks[0].code, code);
    assert.match(report.checks[0].hint, /npm install --include=optional @stateset\/embedded@/);
    assert.match(report.checks[0].hint, /libc/);
    assert.deepEqual(JSON.parse(JSON.stringify(report)), report);
  }
});

test('invalid Commerce export reports the loading stage', async () => {
  const { runEmbeddedCheck } = await import('../self-test.mjs');
  const report = await runEmbeddedCheck({ loadCommerce: async () => ({}) });
  assert.equal(report.checks[0].id, 'native_binding');
  assert.equal(report.checks[0].status, 'failed');
});

for (const [failAt, stage] of [
  ['open', 'database'],
  ['customer', 'order'],
  ['order', 'order'],
  ['payment', 'payment'],
  ['count', 'payment_idempotency'],
  ['complete_payment', 'payment_completion'],
  ['refund', 'refund'],
  ['get_refunds', 'refund_idempotency'],
  ['complete_refund', 'refund_completion'],
  ['close', 'cleanup'],
]) {
  test(`reports ${stage} failure and attempts cleanup after ${failAt}`, async () => {
    const { runEmbeddedCheck } = await import('../self-test.mjs');
    const fake = fakeCommerce({ failAt });
    const report = await runEmbeddedCheck(fake);
    assert.equal(report.ok, false);
    const failures = report.checks.filter(({ status }) => status === 'failed');
    assert.equal(failures.length, 1);
    assert.equal(failures[0].id, stage);
    assert.equal(failures[0].code, 'TEST_FAILURE');
    assert.equal(
      fake.calls.some(({ name }) => name === 'close'),
      failAt !== 'open',
    );
  });
}

test('cleanup failure preserves the original failure', async () => {
  const { runEmbeddedCheck } = await import('../self-test.mjs');
  const fake = fakeCommerce({ failAt: 'payment' });
  fake.instance.close = async () => {
    throw new Error('Close failed');
  };
  const report = await runEmbeddedCheck(fake);
  assert.equal(report.ok, false);
  assert.deepEqual(
    report.checks.filter(({ status }) => status === 'failed').map(({ id }) => id),
    ['payment', 'cleanup'],
  );
});

for (const option of ['wrongAmount', 'duplicatePayment']) {
  test(`refuses success when the engine returns ${option}`, async () => {
    const { runEmbeddedCheck } = await import('../self-test.mjs');
    const fake = fakeCommerce({ [option]: true });
    const report = await runEmbeddedCheck(fake);
    assert.equal(report.ok, false);
    assert.ok(report.checks.some(({ code }) => code === 'ERR_ASSERTION'));
    assert.equal(fake.instance.isClosed, true);
  });
}

test('CLI help, argument validation, and JSON failures work without a native binding', (t) => {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'stateset-check-missing-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  mkdirSync(path.join(dir, 'bin'));
  for (const file of ['self-test.mjs', 'bin/stateset-embedded-check.mjs']) {
    copyFileSync(path.join(__dirname, '..', file), path.join(dir, file));
  }
  writeFileSync(path.join(dir, 'package.json'), JSON.stringify({ version: '0.0.0-test' }));
  const run = (...args) =>
    spawnSync(process.execPath, [path.join(dir, 'bin/stateset-embedded-check.mjs'), ...args], {
      cwd: dir,
      encoding: 'utf8',
      timeout: 10_000,
    });
  const help = run('--help');
  assert.equal(help.status, 0, help.stderr);
  assert.match(help.stdout, /Usage:/);
  assert.match(help.stdout, /not payment providers or external settlement/);
  for (const args of [
    ['--json', '--db', 'store.db'],
    ['--json', 'store.db'],
    ['--json', '--unknown'],
  ]) {
    const result = run(...args);
    assert.equal(result.status, 2, result.stderr);
    assert.equal(result.stderr, '');
    const report = JSON.parse(result.stdout);
    assert.equal(report.ok, false);
    assert.equal(report.checks[0].id, 'arguments');
  }
  const missing = run('--json');
  assert.equal(missing.status, 1, missing.stderr);
  assert.equal(missing.stderr, '');
  const report = JSON.parse(missing.stdout);
  assert.equal(report.ok, false);
  assert.equal(report.checks[0].id, 'native_binding');
  assert.equal(report.checks[0].code, 'ERR_MODULE_NOT_FOUND');
  const text = run();
  assert.equal(text.status, 1);
  assert.match(text.stdout, /FAIL native_binding/);
  assert.match(text.stdout, /npm install --include=optional/);
});

import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { PassThrough } from 'node:stream';
import { EventEmitter } from 'node:events';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import {
  OrderOperations,
  orderAction,
  orderReceiptIssues,
} from '../../src/tui/order-operations.js';
import { orderWorkflowLines, orderWorkflowLimit } from '../../src/tui/order-workflow.js';
import { initialState, navigate, handleKey, paletteCommands } from '../../src/tui/model.js';
import { render } from '../../src/tui/render.js';
import { startTerminal } from '../../src/tui/app.js';

const id = 'd66d2dcb-cab8-4ff5-b5b3-b2c68f83ff47';
const params = { orderId: id, status: 'confirmed' };
const kernel = () => ({
  strict: true,
  principal: { id: 'user:operator', capabilities: ['orders.transition'] },
  storeId: 'store:owned',
  policy: { version: 'one' },
});
const order = {
  id,
  orderNumber: 'ORD-TEST',
  status: 'pending',
  totalAmountExact: '25.02',
  currency: 'USD',
  version: 2,
};
const receipt = (status, key) => ({
  status,
  command_id: id,
  command_type: 'orders.transition',
  idempotency_key: key,
  aggregate_id: id,
  version_before: 2,
  version_after: status === 'succeeded' ? 3 : 2,
  audit_hash: 'native-audit',
  result: status === 'succeeded' ? { id, status: 'confirmed', version: 3 } : null,
  economic_context: { principal: { id: 'user:operator' }, store_id: 'store:owned' },
});
const review = {
  order,
  params,
  ticketId: id,
  principalId: 'user:operator',
  storeId: 'store:owned',
  applyAllowed: true,
  receipt: receipt('previewed', 'native-key'),
};
const tick = () => new Promise((resolve) => setImmediate(resolve));
const key = (state, text, name = text) => handleKey(state, text, { name });

async function service(t, options = {}) {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-order-service-'));
  t.after(() => fs.rm(directory, { recursive: true, force: true }));
  const calls = [];
  const operations = new OrderOperations({
    dbPath: path.join(directory, 'store.db'),
    directory,
    allowApply: true,
    loadKernel: kernel,
    invoke: async (operation, payload, config) => {
      calls.push({ operation, payload: structuredClone(payload), kernel: structuredClone(config) });
      if (operation === 'order-preview')
        return {
          order: structuredClone(order),
          params: structuredClone(params),
          receipt: receipt('previewed', payload.idempotencyKey),
        };
      if (operation === 'order-apply')
        return { receipt: receipt('succeeded', payload.idempotencyKey) };
      return { receipt: null };
    },
    ...options,
  });
  return { operations, calls, directory };
}

test('order boundary refuses arbitrary writes, UI authority and missing trusted configuration', async (t) => {
  for (const value of [
    null,
    [],
    { ...params, voidPayments: true },
    { ...params, status: 'shipped' },
    { ...params, orderId: 'wrong' },
    { ...params, kernel: kernel() },
  ])
    assert.throws(() => orderAction(value), /supported order/);
  const { operations, calls } = await service(t, { loadKernel: () => null });
  await assert.rejects(operations.preview(params), /operator-owned/);
  operations.loadKernel = () => ({ ...kernel(), strict: false });
  await assert.rejects(operations.preview(params), /operator-owned/);
  operations.dbPath = null;
  await assert.rejects(operations.preview(params), /demo remains read-only/);
  assert.equal(calls.length, 0);
});

test('preview-only service cannot apply, including with an invented ticket', async (t) => {
  const { operations, calls } = await service(t, { allowApply: false });
  const preview = await operations.preview(params);
  assert.equal(preview.applyAllowed, false);
  await assert.rejects(operations.apply({ ticketId: preview.ticketId }), /Preview only/);
  assert.deepEqual(
    calls.map((c) => c.operation),
    ['order-preview'],
  );
});

test('apply captures native version and host key, journals before invocation and caches completed outcome', async (t) => {
  const { operations, calls, directory } = await service(t);
  const preview = await operations.preview(params),
    invoke = operations.invoke;
  await assert.rejects(operations.apply({ ticketId: 'invented' }), /unavailable/);
  await assert.rejects(
    operations.apply({ ticketId: preview.ticketId, params: { status: 'cancelled' } }),
    /only a host-issued/,
  );
  // Mutating the response must never change the host-held action or version.
  preview.params.status = 'cancelled';
  preview.receipt.version_before = 400;
  operations.invoke = async (operation, payload, config) => {
    if (operation === 'order-apply') {
      const saved = JSON.parse(await fs.readFile(path.join(directory, 'last-order-action.json')));
      assert.equal(saved.state, 'pending');
      assert.equal(saved.expectedVersion, 2);
      assert.equal(saved.params.status, 'confirmed');
      assert.equal(saved.idempotencyKey, payload.idempotencyKey);
      assert.equal(
        (await fs.stat(path.join(directory, `order-action-${saved.id}.json`))).mode & 0o777,
        0o600,
      );
    }
    return invoke(operation, payload, config);
  };
  const result = await operations.apply({ ticketId: preview.ticketId });
  assert.equal(result.receipt.status, 'succeeded');
  assert.equal((await operations.apply({ ticketId: preview.ticketId })).replayed, true);
  assert.equal(calls.filter((c) => c.operation === 'order-apply').length, 1);
  assert.deepEqual(calls[1].payload.params, params);
  assert.equal(calls[1].payload.expectedVersion, 2);
  assert.equal(calls[1].payload.idempotencyKey, preview.idempotencyKey);
});

test('policy changes and journal failure refuse before a live command', async (t) => {
  const { operations, calls } = await service(t);
  const preview = await operations.preview(params);
  operations.loadKernel = () => ({ ...kernel(), policy: { version: 'two' } });
  await assert.rejects(operations.apply({ ticketId: preview.ticketId }), /changed/);
  operations.loadKernel = kernel;
  operations.journal = async () => {
    throw new Error('no durable storage');
  };
  await assert.rejects(operations.apply({ ticketId: preview.ticketId }), /no durable storage/);
  assert.deepEqual(
    calls.map((c) => c.operation),
    ['order-preview'],
  );
});

test('duplicate Enter cannot start a second apply and settle drains the whole action', async (t) => {
  const { operations } = await service(t);
  const preview = await operations.preview(params);
  let release, started;
  const active = new Promise((resolve) => {
    started = resolve;
  });
  operations.invoke = async (_, payload) => {
    started();
    await new Promise((resolve) => {
      release = resolve;
    });
    return { receipt: receipt('succeeded', payload.idempotencyKey) };
  };
  const first = operations.run('apply', { ticketId: preview.ticketId });
  await active;
  await assert.rejects(operations.apply({ ticketId: preview.ticketId }), /already running/);
  await assert.rejects(operations.preview(params), /active order/);
  await assert.rejects(operations.reconcile(), /active order/);
  let settled = false;
  const drained = operations.settle().then(() => {
    settled = true;
  });
  await tick();
  assert.equal(settled, false);
  release();
  await first;
  await drained;
  assert.equal(settled, true);
});

test('transport failure retains pending intent and retries the same host-held key', async (t) => {
  const { operations, directory } = await service(t);
  const preview = await operations.preview(params),
    keys = [];
  operations.invoke = async (_, payload) => {
    keys.push(payload.idempotencyKey);
    if (keys.length === 1) throw new Error('connection lost');
    return { receipt: receipt('succeeded', payload.idempotencyKey) };
  };
  await assert.rejects(
    operations.apply({ ticketId: preview.ticketId }),
    (error) => error.outcomeUnknown && /connection lost/.test(error.message),
  );
  assert.equal(
    JSON.parse(await fs.readFile(path.join(directory, 'last-order-action.json'))).state,
    'pending',
  );
  await operations.apply({ ticketId: preview.ticketId });
  assert.deepEqual(keys, [preview.idempotencyKey, preview.idempotencyKey]);
});

test('post-commit history failure retains the native receipt and cannot re-execute', async (t) => {
  const { operations, calls } = await service(t);
  const preview = await operations.preview(params),
    journal = operations.journal.bind(operations);
  operations.journal = async (record) => {
    if (record.state === 'finished') throw new Error('disk full');
    await journal(record);
  };
  const applied = await operations.apply({ ticketId: preview.ticketId });
  assert.equal(applied.receipt.status, 'succeeded');
  assert.match(applied.journalWarning, /local history/);
  await operations.apply({ ticketId: preview.ticketId });
  assert.equal(calls.filter((c) => c.operation === 'order-apply').length, 1);
});

test('restart reconciliation is read-only, scoped and never rehydrates workspace commands', async (t) => {
  const { operations, directory } = await service(t);
  const preview = await operations.preview(params);
  await operations.apply({ ticketId: preview.ticketId });
  const calls = [],
    recovered = receipt('succeeded', preview.idempotencyKey);
  const restarted = new OrderOperations({
    dbPath: operations.dbPath,
    directory,
    loadKernel: () => ({ ...kernel(), policy: { version: 'updated' } }),
    invoke: async (operation, payload) => {
      calls.push(operation);
      assert.deepEqual(payload, { idempotencyKey: preview.idempotencyKey });
      return { receipt: recovered };
    },
  });
  assert.equal((await restarted.reconcile()).receipt.status, 'succeeded');
  await assert.rejects(restarted.apply({ ticketId: preview.ticketId }), /Preview only/);
  await assert.rejects(restarted.reconcile({ command: params }), /no caller-supplied/);
  restarted.loadKernel = () => ({ ...kernel(), principal: { id: 'user:someone-else' } });
  await assert.rejects(restarted.reconcile(), /does not match/);
  assert.deepEqual(calls, ['order-reconcile']);
});

test('every intent survives last-action replacement; unsupported history refuses without lookup', async (t) => {
  const { operations, directory, calls } = await service(t);
  for (let i = 0; i < 2; i++)
    await operations.apply({ ticketId: (await operations.preview(params)).ticketId });
  assert.equal(
    (await fs.readdir(directory)).filter((name) => /^order-action-/.test(name)).length,
    2,
  );
  const file = path.join(directory, 'last-order-action.json'),
    record = JSON.parse(await fs.readFile(file));
  await fs.writeFile(file, JSON.stringify({ ...record, params: { ...params, force: true } }));
  await assert.rejects(operations.reconcile(), /supported order/);
  assert.equal(calls.filter((c) => c.operation === 'order-reconcile').length, 0);
});

const orderState = () => ({
  ...navigate(initialState(), 1),
  data: { orders: [order], errors: [], counts: {} },
});
test('order review owns keys, captures selected order and requires exact typed APPLY', () => {
  const opened = key(orderState(), 'W');
  assert.equal(opened.state.input.orderId, id);
  assert.equal(opened.effect, undefined);
  const previewing = key(opened.state, '', 'return');
  assert.equal(previewing.effect, 'preview-order');
  assert.equal(key(previewing.state, '', 'return').effect, undefined);
  assert.equal(key(previewing.state, '', 'escape').state, previewing.state);
  let state = {
    ...opened.state,
    input: { ...opened.state.input, phase: 'review', preview: review },
  };
  for (const value of ['q', 's', '2', 'W', 'P']) assert.equal(key(state, value).effect, undefined);
  assert.equal(key(state, 'v').state.input.raw, true);
  state = key(state, '', 'return').state;
  assert.equal(state.input.phase, 'confirm');
  assert.equal(key(state, '', 'return').effect, undefined);
  assert.equal(key(key(state, 'apply').state, '', 'return').effect, undefined);
  const confirmed = key(key(state, 'APPLY').state, '', 'return');
  assert.equal(confirmed.effect, 'apply-order');
  assert.equal(key(confirmed.state, '', 'return').effect, undefined);
  assert.equal(key(state, '', 'escape').state.input.phase, 'review');
  const denied = {
    ...state,
    input: { ...state.input, phase: 'review', preview: { ...review, applyAllowed: false } },
  };
  assert.equal(key(denied, '', 'return').state.input.phase, 'review');
  assert.ok(paletteCommands('selected order', orderState()).some((command) => command.key === 'W'));
  assert.equal(key({ ...orderState(), loading: true }, 'W').state.input, null);
});

test('exact review evidence and native refusal remain scrollable at 20x8 and after resizing', () => {
  const input = { ...key(orderState(), 'W').state.input, phase: 'review', preview: review };
  const lines = orderWorkflowLines(input, 118).join('\n');
  for (const value of [
    '25.02',
    'pending → confirmed',
    'user:operator',
    'store:owned',
    'native-key',
    'native-audit',
    'refund',
  ])
    assert.ok(lines.includes(value));
  const narrow = orderWorkflowLimit(input, 20, 8),
    wide = orderWorkflowLimit(input, 140, 44);
  assert.ok(narrow > wide);
  const frame = render({ ...orderState(), input: { ...input, offset: narrow } }, 20, 8, {
    color: false,
  });
  assert.match(frame, /live change has/);
  const confirm = render(
    { ...orderState(), input: { ...input, phase: 'confirm', text: 'APPL', cursor: 4 } },
    20,
    8,
    { color: false },
  );
  assert.match(confirm, /Type APPLY/);
  assert.match(confirm, /APPL▏/);
});

async function terminal(t, applied = { receipt: receipt('succeeded', 'native-key') }) {
  const input = new PassThrough(),
    output = new PassThrough(),
    signals = new EventEmitter(),
    calls = [];
  let frame = '',
    resolveApply,
    resolveClose;
  const raw = [];
  input.setRawMode = (value) => raw.push(value);
  output.columns = 140;
  output.rows = 44;
  output.on('data', (value) => {
    frame = value.toString();
  });
  const backend = {
    close: () =>
      new Promise((resolve) => {
        resolveClose = resolve;
      }),
    request: async (operation, payload) => {
      calls.push({ operation, payload });
      if (operation === 'snapshot') return { orders: [order], errors: [], counts: {} };
      if (operation === 'preview-order') return structuredClone(review);
      if (operation === 'apply-order')
        return new Promise((resolve) => {
          resolveApply = resolve;
        });
      throw new Error('unexpected request');
    },
  };
  const done = startTerminal({
    dbPath: '/owned.db',
    input,
    output,
    signals,
    backend,
    color: false,
    allowApply: true,
  });
  const send = (text, name = text) => input.emit('keypress', text, { name });
  t.after(async () => {
    input.emit('keypress', '', { ctrl: true, name: 'c' });
    resolveClose?.();
    await done;
  });
  await tick();
  send('2');
  send('W');
  send('', 'return');
  await tick();
  return {
    send,
    output,
    calls,
    done,
    input,
    raw,
    frame: () => frame,
    finishApply: () => resolveApply(applied),
    finishClose: () => resolveClose(),
  };
}

test('app review survives resize, cancellation has no side effect, typed apply is single-flight and refreshes', async (t) => {
  const app = await terminal(t);
  assert.match(app.frame(), /25.02/);
  app.send('', 'return');
  app.send('', 'escape');
  assert.equal(app.calls.filter((c) => c.operation === 'apply-order').length, 0);
  app.output.columns = 20;
  app.output.rows = 8;
  app.output.emit('resize');
  app.send('', 'end');
  const end = app.frame();
  app.send('', 'up');
  assert.notEqual(app.frame(), end, 'End then Up must move immediately');
  app.send('', 'return');
  app.send('APPLY');
  app.send('', 'return');
  app.send('', 'return');
  app.output.columns = 140;
  app.output.rows = 44;
  app.output.emit('resize');
  assert.match(app.frame(), /Waiting for native receipt/);
  assert.equal(app.calls.filter((c) => c.operation === 'apply-order').length, 1);
  assert.deepEqual(app.calls.find((c) => c.operation === 'apply-order').payload, { ticketId: id });
  app.finishApply();
  await tick();
  assert.match(app.frame(), /Native receipt: succeeded/);
  assert.equal(app.calls.filter((c) => c.operation === 'snapshot').length, 2);
});

test('closing a pending apply restores terminal immediately and waits for backend drain', async (t) => {
  const app = await terminal(t);
  app.send('', 'return');
  app.send('APPLY');
  app.send('', 'return');
  app.input.emit('keypress', '', { ctrl: true, name: 'c' });
  assert.equal(app.raw.at(-1), false);
  let finished = false;
  void app.done.then(() => {
    finished = true;
  });
  await tick();
  assert.equal(finished, false);
  app.finishApply();
  await tick();
  app.finishClose();
  await app.done;
  assert.equal(finished, true);
});

test('CLI refuses apply in demo, snapshot, JSON and unconfigured sessions before opening a store', async () => {
  const execute = promisify(execFile),
    entry = new URL('../../bin/stateset-tui.js', import.meta.url).pathname;
  for (const args of [
    ['--apply'],
    ['--db', '/uncreated.db', '--apply', '--snapshot'],
    ['--db', '/uncreated.db', '--apply', '--json'],
  ]) {
    await assert.rejects(
      execute(process.execPath, [entry, ...args]),
      (error) =>
        error.code === 1 && /--apply requires an interactive --db session/.test(error.stderr),
    );
  }
  await assert.rejects(
    execute(process.execPath, [entry, '--db', '/uncreated.db', '--apply'], {
      env: {
        ...process.env,
        STATESET_KERNEL_POLICY: '',
        STATESET_KERNEL_PRINCIPAL: '',
        STATESET_KERNEL_STORE_ID: '',
      },
    }),
    (error) => error.code === 1 && /trusted kernel configuration/.test(error.stderr),
  );
});

test('receipt consistency checks preserve native truth while distinguishing conflicts from contradictory success', () => {
  const reviewed = {
    params,
    expectedVersion: 2,
    principalId: 'user:operator',
    storeId: 'store:owned',
  };
  const original = receipt('succeeded', 'native-key');
  assert.deepEqual(orderReceiptIssues(original, reviewed), []);
  assert.deepEqual(
    orderReceiptIssues({ ...original, status: 'rejected', version_before: 9 }, reviewed),
    [],
  );
  for (const corrupted of [
    { ...original, version_before: 9 },
    { ...original, version_after: 9 },
    { ...original, result: { ...original.result, version: 9 } },
    { ...original, aggregate_id: 'another-order' },
    { ...original, result: { ...original.result, id: 'another-order' } },
    { ...original, result: { ...original.result, status: 'cancelled' } },
    { ...original, economic_context: { ...original.economic_context, store_id: 'store:other' } },
    {
      ...original,
      economic_context: { ...original.economic_context, principal: { id: 'user:other' } },
    },
  ]) {
    assert.ok(orderReceiptIssues(corrupted, reviewed).length);
  }
  assert.equal(original.version_before, 2);
  assert.equal(original.result.status, 'confirmed');
});

test('contradictory native success remains visible and journaled, cached retries cannot execute it again', async (t) => {
  const { operations, calls, directory } = await service(t);
  const preview = await operations.preview(params),
    invoke = operations.invoke;
  operations.invoke = async (operation, payload, config) => {
    const data = await invoke(operation, payload, config);
    if (operation === 'order-apply') data.receipt.version_before = 8;
    return data;
  };
  const applied = await operations.apply({ ticketId: preview.ticketId });
  assert.equal(
    applied.receipt.status,
    'succeeded',
    'never claim that an already returned commit was refused',
  );
  assert.equal(applied.receipt.version_before, 8);
  assert.match(applied.reviewIssues.join(' '), /Reviewed version 2.*started from 8/);
  const saved = JSON.parse(await fs.readFile(path.join(directory, 'last-order-action.json')));
  assert.equal(saved.receipt.version_before, 8);
  assert.deepEqual(saved.reviewIssues, applied.reviewIssues);
  applied.reviewIssues.length = 0;
  const cached = await operations.apply({ ticketId: preview.ticketId });
  assert.ok(cached.reviewIssues.length);
  assert.equal(calls.filter((call) => call.operation === 'order-apply').length, 1);
  operations.invoke = async (operation, payload) => {
    assert.equal(operation, 'order-reconcile');
    return { receipt: { ...receipt('succeeded', payload.idempotencyKey), version_before: 8 } };
  };
  assert.ok(
    (await operations.reconcile()).reviewIssues.length,
    'restart lookup must retain the discrepancy',
  );
});

test('conflict recovery discards prior authority and confirmation while retaining the captured order', () => {
  const input = {
    ...key(orderState(), 'W').state.input,
    phase: 'result',
    preview: review,
    receipt: { ...receipt('rejected', 'native-key'), error_code: 'kernel.version_conflict' },
    text: 'APPLY',
    cursor: 5,
  };
  const state = { ...orderState(), input, selected: 99 };
  const restarted = key(state, 'r');
  assert.equal(restarted.effect, undefined);
  assert.equal(restarted.state.input.phase, 'choose');
  assert.equal(restarted.state.input.orderId, id);
  for (const field of ['preview', 'receipt', 'text'])
    assert.equal(restarted.state.input[field], undefined);
  assert.equal(key(restarted.state, '', 'return').effect, 'preview-order');
  for (const phase of ['uncertain', 'attention']) {
    const held = key(
      {
        ...state,
        input: { ...input, phase, reviewIssues: phase === 'attention' ? ['version mismatch'] : [] },
      },
      'r',
    );
    assert.equal(held.effect, undefined);
    assert.equal(held.state.input.phase, phase);
  }
  assert.equal(
    key({ ...state, input: { ...input, phase: 'uncertain' } }, '', 'return').effect,
    'apply-order',
  );
});

test('receipt mismatch is prominent at 20x8 and the app offers inspection without a replay shortcut', async (t) => {
  const app = await terminal(t, {
    receipt: receipt('succeeded', 'native-key'),
    reviewIssues: ['Native version differs from review.'],
  });
  app.send('', 'return');
  app.send('APPLY');
  app.send('', 'return');
  app.finishApply();
  await tick();
  assert.match(app.frame(), /RECEIPT MISMATCH/);
  app.output.columns = 20;
  app.output.rows = 8;
  app.output.emit('resize');
  assert.match(app.frame(), /RECEIPT MISMATCH/);
  app.send('r');
  app.send('', 'return');
  assert.equal(app.calls.filter((call) => call.operation === 'preview-order').length, 1);
  assert.equal(app.calls.filter((call) => call.operation === 'apply-order').length, 1);
});

test('non-object history refuses cleanly before any native receipt lookup', async (t) => {
  const { operations, directory, calls } = await service(t);
  for (const value of [null, [], 42]) {
    await fs.writeFile(path.join(directory, 'last-order-action.json'), JSON.stringify(value));
    await assert.rejects(operations.reconcile(), /history does not match/);
  }
  assert.equal(calls.length, 0);
});

test('unknown apply outcome stays prominent in compact panes and its preview is labeled as prior evidence', () => {
  const input = { ...key(orderState(), 'W').state.input, phase: 'uncertain', preview: review };
  const frame = render({ ...orderState(), input }, 20, 8, { color: false });
  assert.match(frame, /OUTCOME UNKNOWN/);
  const lines = orderWorkflowLines(input, 118).join('\n');
  assert.match(lines, /Prior preview receipt: previewed/);
  assert.doesNotMatch(lines, /Native receipt: previewed/);
  assert.equal(key({ ...orderState(), input }, 'r').state.input.phase, 'uncertain');
});

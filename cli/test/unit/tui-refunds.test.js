import { refundCommitment } from '../../src/tui/refund-engine.js';
import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import { PassThrough } from 'node:stream';
import { EventEmitter } from 'node:events';
import {
  RefundOperations,
  refundAction,
  refundReceiptIssues,
} from '../../src/tui/refund-operations.js';
import { OrderOperations } from '../../src/tui/order-operations.js';
import { initialState, navigate, handleKey, paletteCommands } from '../../src/tui/model.js';
import { refundWorkflowLines, refundWorkflowLimit } from '../../src/tui/refund-workflow.js';
import { render } from '../../src/tui/render.js';
import { startTerminal } from '../../src/tui/app.js';
import { refundKernel } from '../fixtures/tui-refunds.js';

const id = 'b734166a-c7c8-42ae-b991-eb1974a3f13f';
const refundId = 'e29e51a4-299c-4f0f-b738-f8adad0b7964';
const params = { paymentId: id, amount: '0.10', reason: 'Customer request' };
const payment = {
  id,
  paymentNumber: 'PAY-EXACT',
  currency: 'USD',
  amountExact: '25.02',
  amountRefundedExact: '0.00',
  status: 'completed',
};
const receipt = (status, key) => ({
  status,
  command_id: refundId,
  command_type: 'payments.create_refund',
  idempotency_key: key,
  version_before: null,
  version_after: status === 'succeeded' ? 1 : null,
  aggregate_id: status === 'succeeded' ? refundId : null,
  audit_hash: 'native-audit',
  result:
    status === 'succeeded'
      ? {
          id: refundId,
          payment_id: id,
          amount: '0.1',
          currency: 'USD',
          status: 'pending',
          reason: params.reason,
        }
      : null,
  economic_context: { principal: { id: 'user:tui' }, store_id: 'store:tui' },
});
const key = (state, text, name = text, extra = {}) => handleKey(state, text, { name, ...extra });
const tick = () => new Promise((resolve) => setImmediate(resolve));
async function service(t, options = {}) {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-refund-service-'));
  t.after(() => fs.rm(directory, { recursive: true, force: true }));
  const calls = [];
  const operations = new RefundOperations({
    dbPath: path.join(directory, 'owned.db'),
    directory,
    allowApply: true,
    loadKernel: refundKernel,
    invoke: async (operation, payload) => {
      calls.push({ operation, payload: structuredClone(payload) });
      return operation === 'refund-preview'
        ? {
            payment: structuredClone(payment),
            params: structuredClone(payload.params),
            receipt: receipt('previewed', payload.idempotencyKey),
          }
        : { receipt: receipt('succeeded', payload.idempotencyKey) };
    },
    ...options,
  });
  return { operations, directory, calls };
}

test('refund requests require exact positive money, explicit bounded reasons and no caller-supplied authority', () => {
  for (const change of [
    { amount: 0.1 },
    { amount: '0' },
    { amount: '-1' },
    { amount: '1e-2' },
    { amount: '1.12345678901234567890123456789' },
    { reason: '' },
    { reason: 'x'.repeat(241) },
    { reason: 'a\ncommand' },
    { idempotencyKey: 'override' },
    { kernel: refundKernel() },
    { paymentId: '../db' },
  ])
    assert.throws(() => refundAction({ ...params, ...change }), /exact decimal/);
  assert.deepEqual(refundAction({ ...params, reason: ' Customer request ' }), params);
  assert.equal(
    refundAction({ ...params, amount: '0.0000000000000000000000000001' }).amount,
    '0.0000000000000000000000000001',
  );
});

test('refund preview-only mode and trusted configuration refuse writes before native invocation', async (t) => {
  const { operations, calls } = await service(t, { allowApply: false });
  const preview = await operations.preview(params);
  assert.equal(preview.applyAllowed, false);
  await assert.rejects(operations.apply({ ticketId: preview.ticketId }), /Preview only/);
  operations.loadKernel = () => null;
  await assert.rejects(operations.preview(params), /operator-owned/);
  assert.equal(calls.length, 1);
});

test('refund host captures exact reviewed values, journals before submission and cannot execute a cached success again', async (t) => {
  const { operations, directory, calls } = await service(t);
  const preview = await operations.preview(params),
    invoke = operations.invoke;
  preview.params.amount = '20.00';
  preview.payment.currency = 'EUR';
  operations.invoke = async (operation, payload) => {
    const saved = JSON.parse(await fs.readFile(path.join(directory, 'last-refund-action.json')));
    assert.equal(saved.state, 'pending');
    assert.deepEqual(saved.params, params);
    assert.deepEqual(saved.context, { currency: 'USD' });
    assert.equal(payload.expectedVersion, null);
    return invoke(operation, payload);
  };
  const applied = await operations.apply({ ticketId: preview.ticketId });
  assert.equal(applied.receipt.result.status, 'pending');
  assert.deepEqual(applied.reviewIssues, []);
  assert.equal((await operations.apply({ ticketId: preview.ticketId })).replayed, true);
  assert.equal(calls.filter((c) => c.operation === 'refund-apply').length, 1);
  await assert.rejects(
    operations.apply({ ticketId: preview.ticketId, params }),
    /only a host-issued/,
  );
  assert.equal(
    (await fs.stat(path.join(directory, `refund-action-${preview.ticketId}.json`))).mode & 0o777,
    0o600,
  );
});

test('unknown refund outcomes retain the same key; restart lookup and forged local summaries never submit a refund', async (t) => {
  const { operations, directory } = await service(t);
  const preview = await operations.preview(params),
    invoke = operations.invoke,
    keys = [];
  operations.invoke = async (operation, payload) => {
    keys.push(payload.idempotencyKey);
    if (keys.length === 1) throw new Error('connection lost');
    return invoke(operation, payload);
  };
  await assert.rejects(
    operations.apply({ ticketId: preview.ticketId }),
    (error) => error.outcomeUnknown,
  );
  assert.equal(
    JSON.parse(await fs.readFile(path.join(directory, 'last-refund-action.json'))).state,
    'pending',
  );
  await operations.apply({ ticketId: preview.ticketId });
  assert.deepEqual(keys, [preview.idempotencyKey, preview.idempotencyKey]);
  const file = path.join(directory, `refund-action-${preview.ticketId}.json`),
    before = await fs.readFile(file, 'utf8');
  operations.allowApply = false;
  operations.tickets.clear();
  operations.invoke = async (operation, payload) => {
    assert.equal(operation, 'refund-reconcile');
    assert.deepEqual(payload, { idempotencyKey: preview.idempotencyKey });
    return { receipt: null };
  };
  assert.equal((await operations.reconcile({ historyId: preview.ticketId })).receipt, null);
  assert.equal(await fs.readFile(file, 'utf8'), before);
  const history = await operations.history();
  assert.equal(history.entries[0].paymentId, id);
  assert.equal(history.entries[0].amount, '0.10');
  operations.loadKernel = () => ({
    ...refundKernel(),
    principal: { ...refundKernel().principal, id: 'user:other' },
  });
  await assert.rejects(operations.reconcile({ historyId: preview.ticketId }), /does not match/);
});

test('contradictory refund success retains native truth and flags amount, payment, currency, status, version and scope mismatches', async (t) => {
  const review = {
    params,
    context: { currency: 'USD' },
    principalId: 'user:tui',
    storeId: 'store:tui',
  };
  assert.deepEqual(refundReceiptIssues(receipt('succeeded', 'key'), review), []);
  assert.deepEqual(refundReceiptIssues(receipt('rejected', 'key'), review), []);
  for (const change of [
    { amount: 0.1 },
    { amount: '20' },
    { amount: 'NaN' },
    { payment_id: randomUUID() },
    { currency: 'EUR' },
    { status: 'completed' },
    { reason: 'other' },
  ]) {
    const native = receipt('succeeded', 'key');
    Object.assign(native.result, change);
    assert.ok(refundReceiptIssues(native, review).length);
  }
  for (const change of [
    { aggregate_id: id },
    { version_after: 2 },
    { economic_context: {} },
    { version_before: 1 },
  ])
    assert.ok(refundReceiptIssues({ ...receipt('succeeded', 'key'), ...change }, review).length);
  const { operations, directory } = await service(t);
  const preview = await operations.preview(params);
  operations.invoke = async (_, payload) => {
    const native = receipt('succeeded', payload.idempotencyKey);
    native.result.amount = '20';
    return { receipt: native };
  };
  const result = await operations.apply({ ticketId: preview.ticketId });
  assert.ok(result.reviewIssues.length);
  assert.equal(result.receipt.status, 'succeeded');
  assert.ok(
    JSON.parse(await fs.readFile(path.join(directory, 'last-refund-action.json'))).reviewIssues
      .length,
  );
  operations.invoke = async () => {
    throw new Error('must not execute again');
  };
  assert.equal((await operations.apply({ ticketId: preview.ticketId })).replayed, true);
});

test('order and refund actions share one live gate and refund intents cannot become order replay authority', async (t) => {
  const gate = { active: false },
    { operations, directory } = await service(t, { gate });
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
  const applying = operations.run('apply', { ticketId: preview.ticketId });
  await active;
  const orders = new OrderOperations({
    gate,
    dbPath: operations.dbPath,
    directory,
    allowApply: true,
    loadKernel: refundKernel,
    invoke: async () => {
      throw new Error('cross-domain execution');
    },
  });
  await assert.rejects(orders.preview({ orderId: id, status: 'confirmed' }), /active order action/);
  await assert.rejects(orders.apply({ ticketId: preview.ticketId }), /already running/);
  await assert.rejects(operations.reconcile(), /active refund action/);
  release();
  await applying;
  await operations.settle();
  await assert.rejects(orders.apply({ ticketId: preview.ticketId }), /unavailable/);
  assert.deepEqual((await orders.history()).entries, []);
  operations.loadKernel = () => ({
    ...refundKernel(),
    policy: { ...refundKernel().policy, version: 'changed' },
  });
  await assert.rejects(operations.apply({ ticketId: preview.ticketId }), /policy changed/);
});

const paymentState = () => ({
  ...navigate(initialState(), 4),
  data: { payments: [payment], errors: [], counts: {} },
});
test('refund amount/reason editing, exact confirmation, compact resize and history own their keys', () => {
  let state = key(paymentState(), 'W').state;
  assert.equal(state.input.kind, 'refund-action');
  assert.match(render(state, 20, 8, { color: false }), /Amount USD/);
  state = key(state, '0.10').state;
  state = key(state, '', 'tab').state;
  state = key(state, 'Customer request').state;
  const previewing = key(state, '', 'return');
  assert.equal(previewing.effect, 'preview-refund');
  assert.deepEqual(previewing.state.input.params, params);
  assert.equal(key(previewing.state, '', 'return').effect, undefined);
  const preview = {
    payment,
    params,
    receipt: receipt('previewed', 'key'),
    ticketId: id,
    applyAllowed: true,
    principalId: 'user:tui',
    storeId: 'store:tui',
  };
  state = {
    ...previewing.state,
    input: { ...previewing.state.input, phase: 'review', busy: false, preview },
  };
  assert.match(refundWorkflowLines(state.input, 118).join('\n'), /USD 0.10/);
  assert.ok(refundWorkflowLimit(state.input, 20, 8) > 0);
  state = key(state, '', 'return').state;
  assert.match(render(state, 20, 8, { color: false }), /Type APPLY/);
  state = key(state, 'apply').state;
  assert.equal(key(state, '', 'return').effect, undefined);
  state = key(state, '', 'u', { ctrl: true }).state;
  state = key(state, 'APPLY').state;
  assert.equal(key(state, '', 'return').effect, 'apply-refund');
  const uncertain = { ...state, input: { ...state.input, phase: 'uncertain', text: '' } };
  assert.match(render(uncertain, 20, 8, { color: false }), /OUTCOME UNKNOWN/);
  assert.equal(key(uncertain, 'r').state.input.phase, 'uncertain');
  assert.equal(key(uncertain, '', 'return').effect, 'apply-refund');
  const completed = {
    ...state,
    input: { ...state.input, phase: 'result', receipt: receipt('succeeded', 'key') },
  };
  assert.match(render(completed, 20, 8, { color: false }), /PENDING REFUND/);
  const attention = {
    ...state,
    input: {
      ...state.input,
      phase: 'attention',
      reviewIssues: ['mismatch'],
      receipt: receipt('succeeded', 'key'),
    },
  };
  assert.match(render(attention, 20, 8, { color: false }), /RECEIPT MISMATCH/);
  assert.equal(key(attention, 'r').state.input.phase, 'attention');
  assert.doesNotMatch(
    refundWorkflowLines(attention.input, 118).join('\n'),
    /PENDING REFUND CREATED/,
  );
  assert.equal(key(paymentState(), 'H').effect, 'refund-history');
  assert.equal(key(paymentState(), 'P').effect, 'reconcile-refund');
  assert.ok(paletteCommands('pending refund', paymentState()).some((c) => c.key === 'W'));
});

test('refund terminal applies once, retains the native pending result and drains an active apply on exit', async (t) => {
  const input = new PassThrough(),
    output = new PassThrough(),
    signals = new EventEmitter(),
    calls = [],
    raw = [];
  input.setRawMode = (value) => raw.push(value);
  output.columns = 140;
  output.rows = 44;
  let frame = '',
    release,
    close;
  output.on('data', (value) => {
    frame = value.toString();
  });
  const backend = {
    request: async (operation, payload) => {
      calls.push({ operation, payload });
      if (operation === 'snapshot') return { payments: [payment], errors: [], counts: {} };
      if (operation === 'preview-refund')
        return {
          payment,
          params,
          receipt: receipt('previewed', 'key'),
          ticketId: id,
          applyAllowed: true,
        };
      if (operation === 'apply-refund')
        return new Promise((resolve) => {
          release = () => resolve({ receipt: receipt('succeeded', 'key'), reviewIssues: [] });
        });
      throw new Error('Unexpected operation');
    },
    close: () =>
      new Promise((resolve) => {
        close = resolve;
      }),
  };
  const done = startTerminal({
    input,
    output,
    signals,
    backend,
    dbPath: '/owned.db',
    color: false,
    allowApply: true,
  });
  t.after(async () => {
    input.emit('keypress', '', { ctrl: true, name: 'c' });
    release?.();
    close?.();
    await done;
  });
  const send = (text, name = text, extra = {}) => input.emit('keypress', text, { name, ...extra });
  await tick();
  send('5');
  send('W');
  send('0.10');
  send('', 'tab');
  send('Customer request');
  send('', 'return');
  await tick();
  assert.match(frame, /USD 0.10/);
  send('', 'return');
  send('APPLY');
  send('', 'return');
  send('', 'return');
  assert.equal(calls.filter((c) => c.operation === 'apply-refund').length, 1);
  send('', 'c', { ctrl: true });
  assert.equal(raw.at(-1), false);
  let finished = false;
  void done.then(() => {
    finished = true;
  });
  await tick();
  assert.equal(finished, false);
  release();
  close();
  await done;
});

test('refund native commitment binds reviewed currency and exact amount while preserving operator budget and counterparty constraints', () => {
  assert.deepEqual(refundCommitment({}, '0.00000001', 'BTC'), {
    amount: { amount: '0.00000001', currency: 'BTC' },
  });
  assert.throws(() => refundCommitment({}, '0.000001', 'USDC'), /currency is unavailable/);
  assert.throws(() => refundCommitment({ commitment: false }, '0.10', 'USD'), /operator-owned/);
  assert.deepEqual(refundCommitment({}, '0.10', 'USD'), {
    amount: { amount: '0.10', currency: 'USD' },
  });
  const configured = {
    budget_id: 'budget:refunds',
    counterparty_id: 'customer:owned',
    evidence: ['review:owned'],
    amount: { amount: '0.1', currency: 'USD' },
  };
  assert.deepEqual(refundCommitment({ commitment: configured }, '0.10', 'USD'), {
    ...configured,
    amount: { amount: '0.10', currency: 'USD' },
  });
  assert.throws(
    () => refundCommitment({ commitment: configured }, '0.10', 'EUR'),
    /does not match/,
  );
  assert.throws(
    () => refundCommitment({ commitment: configured }, '20.00', 'USD'),
    /does not match/,
  );
  assert.throws(() => refundCommitment({}, '0.10', undefined), /currency is unavailable/);
});

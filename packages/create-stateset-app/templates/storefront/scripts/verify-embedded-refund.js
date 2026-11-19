// Run with npm run verify:embedded-refund. All amounts are local ledger entries in
// a private temporary SQLite file. The example never opens the merchant store.
import assert from 'node:assert/strict';
import { fork } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { once } from 'node:events';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Commerce } from '@stateset/embedded';

const commandType = 'payments.create_refund';
const tenant = 'tenant:pilot';
const store = 'store:pilot';
const refundKey = 'return:fixture-101:refund';
const principal = {
  id: 'agent:returns',
  kind: 'agent',
  tenant_id: tenant,
  delegated_by: 'user:merchant',
  capabilities: [commandType],
};
const policy = {
  version: 'fixture-refunds-v1',
  trusted_authority_keys: {},
  commands: {
    [commandType]: {
      required_capabilities: [commandType],
      requires_approval: false,
      requires_tenant: true,
      requires_store: true,
      allowed_tenant_ids: [tenant],
      allowed_store_ids: [store],
      requires_agent_delegation: true,
      requires_signed_authority: false,
    },
  },
};

// The application owns the policy and principal. Reuse the business operation's
// key across retries, even though each invocation gets its own command ID.
function requestRefund(commerce, paymentId, amount, key, mode) {
  return commerce.executeKernelCommand(
    {
      contract_version: '1.0',
      command_id: randomUUID(),
      command_type: commandType,
      idempotency_key: key,
      issued_at: new Date().toISOString(),
      principal,
      store_id: store,
      policy_version: policy.version,
      mode,
      payload: { payment_id: paymentId, amount, reason: 'Fixture return' },
    },
    policy,
  );
}

async function applyFixture(database) {
  const commerce = new Commerce(database);
  try {
    // A local stand-in for a payment captured by the existing shop.
    const payment = await commerce.payments.createExact({
      idempotencyKey: 'fixture:payment-101',
      currency: 'USD',
      amount: '100.00',
      paymentMethod: 'credit_card',
    });
    await commerce.payments.markCompleted(payment.id);
    const denied = await requestRefund(commerce, payment.id, '250.00', 'return:overage', 'apply');
    assert.equal(denied.status, 'rejected');
    assert.equal(denied.error_code, 'commerce.refund.exceeds_captured');
    const preview = await requestRefund(commerce, payment.id, '40.00', refundKey, 'preview');
    assert.equal(preview.status, 'previewed');
    assert.equal((await commerce.payments.getRefunds(payment.id)).length, 0);
    const applied = await requestRefund(commerce, payment.id, '40.00', refundKey, 'apply');
    assert.equal(applied.status, 'succeeded');
    assert.equal(applied.policy?.allowed, true);
    assert.equal(applied.event_ids.length, 1);
    assert.match(applied.audit_hash ?? '', /^[a-f0-9]{64}$/);
    return {
      paymentId: payment.id,
      refundId: applied.result.id,
      receiptId: applied.receipt_id,
      auditHash: applied.audit_hash,
      denied: denied.error_code,
      preview: preview.status,
    };
  } finally {
    await commerce.close();
  }
}

async function verifyReplay(database, initial) {
  const commerce = new Commerce(database);
  try {
    const replay = await requestRefund(commerce, initial.paymentId, '40.00', refundKey, 'apply');
    assert.equal(replay.status, 'succeeded');
    assert.equal(replay.result.id, initial.refundId);
    assert.equal(replay.receipt_id, initial.receiptId);
    assert.equal(replay.audit_hash, initial.auditHash);
    const changed = await requestRefund(commerce, initial.paymentId, '41.00', refundKey, 'apply');
    assert.equal(changed.status, 'rejected');
    assert.equal(changed.error_code, 'kernel.idempotency_conflict');
    const refunds = await commerce.payments.getRefunds(initial.paymentId);
    assert.equal(refunds.length, 1);
    assert.equal(refunds[0].id, initial.refundId);
    assert.equal(refunds[0].amountExact, '40.00');
    return {
      schemaVersion: 1,
      result: 'passed',
      settlement: 'local-ledger-only',
      currency: 'USD',
      capturedAmount: '100.00',
      declinedOverRefund: initial.denied,
      preview: initial.preview,
      refundAmount: refunds[0].amountExact,
      refundStatus: refunds[0].status,
      refundCount: refunds.length,
      refundId: replay.result.id,
      receiptId: replay.receipt_id,
      auditHash: replay.audit_hash,
      replayedAfterProcessExit: true,
      conflictingRetry: changed.error_code,
    };
  } finally {
    await commerce.close();
  }
}

// Let the process issuing the refund finish before reopening the file. The
// public command takes no database path and ignores STATESET_DB_PATH.
if (process.send) {
  const fixture = await mkdtemp(join(tmpdir(), 'capture-'));
  const database = join(fixture, 'refund.db');
  process.send({ database, ...(await applyFixture(database)) });
  process.disconnect();
} else {
  const directory = await mkdtemp(join(tmpdir(), 'stateset-embedded-refund-'));
  try {
    const child = fork(new URL(import.meta.url), [], {
      env: { ...process.env, TMPDIR: directory, TMP: directory, TEMP: directory },
      stdio: ['ignore', 'inherit', 'inherit', 'ipc'],
    });
    let initial;
    child.on('message', (message) => {
      initial = message;
    });
    const [code] = await once(child, 'exit');
    assert.equal(code, 0, 'The process applying the fixture refund must complete');
    assert.ok(initial?.receiptId, 'The fixture must produce a kernel receipt');
    console.log(JSON.stringify(await verifyReplay(initial.database, initial), null, 2));
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}

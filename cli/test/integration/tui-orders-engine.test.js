import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { createHash } from 'node:crypto';
import Database from 'better-sqlite3';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
const execute = promisify(execFile);
const nativeCall = async (script, args) =>
  JSON.parse(
    (
      await execute(process.execPath, ['--input-type=module', '-e', script, ...args], {
        timeout: 45000,
      })
    ).stdout,
  );
import { TerminalBackend } from '../../src/tui/backend.js';
import { reconcileOrder } from '../../src/tui/order-engine.js';
import { operatorKernel } from '../fixtures/tui-orders.js';

const hash = async (file) =>
  createHash('sha256')
    .update(await fs.readFile(file))
    .digest('hex');

test(
  'governed order preview, native apply, stale version, refusal, idempotency and restart reconciliation',
  { timeout: 360000 },
  async () => {
    const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-order-engine-'));
    const database = path.join(directory, 'owned.db'),
      journal = path.join(directory, 'history');
    let backend,
      previewOnly,
      denied,
      writer,
      passed = false;
    try {
      // The native close drops its pool on a background thread. Own fixture
      // construction in a child so no deferred checkpoint can race WAL hashes.
      const fixture = await nativeCall(
          `import {orderFixture} from ${JSON.stringify(new URL('../fixtures/tui-orders.js', import.meta.url).href)}; console.log(JSON.stringify(await orderFixture(process.argv[1])));`,
          [database],
        ),
        kernel = operatorKernel();
      writer = new Database(database);
      writer.pragma('journal_mode = WAL');
      writer.pragma('wal_autocheckpoint = 0');
      writer.prepare('UPDATE customers SET last_name = ?').run('Committed in WAL');
      const before = await Promise.all([hash(database), hash(database + '-wal')]);
      backend = new TerminalBackend(database, {
        allowApply: true,
        loadKernel: () => kernel,
        operationsDirectory: journal,
      });
      const params = { orderId: fixture.orders[0].id, status: 'confirmed' };
      const preview = await backend.request('preview-order', params);
      assert.equal(preview.receipt.status, 'previewed');
      assert.equal(preview.order.totalAmountExact, '25.02');
      assert.equal(preview.order.status, 'pending');
      assert.equal(preview.applyAllowed, true);
      assert.deepEqual(
        await Promise.all([hash(database), hash(database + '-wal')]),
        before,
        'preview must not write its native receipt/audit to the source',
      );
      assert.equal(writer.prepare('SELECT count(*) AS n FROM kernel_receipts').get().n, 0);
      const version = preview.receipt.version_before;
      const applied = await backend.request('apply-order', { ticketId: preview.ticketId });
      assert.equal(applied.receipt.status, 'succeeded');
      assert.ok(applied.receipt.audit_hash);
      assert.equal(
        writer
          .prepare(
            'SELECT count(*) AS n FROM kernel_receipt_audit_log WHERE audit_hash = ? AND command_id = ?',
          )
          .get(applied.receipt.audit_hash, applied.receipt.command_id).n,
        1,
      );
      assert.ok(applied.receipt.event_ids.length > 0);
      for (const eventId of applied.receipt.event_ids)
        assert.equal(
          writer.prepare('SELECT command_id FROM kernel_outbox WHERE id = ?').get(eventId)
            .command_id,
          applied.receipt.command_id,
        );
      assert.equal(applied.receipt.version_after, version + 1);
      assert.equal(
        writer.prepare('SELECT status FROM orders WHERE id = ?').get(params.orderId).status,
        'confirmed',
      );
      const saved = JSON.parse(await fs.readFile(path.join(journal, 'last-order-action.json')));
      assert.equal(saved.state, 'finished');
      assert.equal(saved.idempotencyKey, applied.receipt.idempotency_key);
      assert.deepEqual(
        (await backend.request('apply-order', { ticketId: preview.ticketId })).receipt,
        applied.receipt,
      );
      // Replay also reaches the native idempotency guard, independent of host caching.
      const replay = await nativeCall(
        `import {applyOrder} from ${JSON.stringify(new URL('../../src/tui/order-engine.js', import.meta.url).href)}; console.log(JSON.stringify(await applyOrder(process.argv[1], JSON.parse(process.argv[2]), JSON.parse(process.argv[3]))));`,
        [
          database,
          JSON.stringify(kernel),
          JSON.stringify({
            params,
            expectedVersion: version,
            idempotencyKey: preview.idempotencyKey,
          }),
        ],
      );
      assert.equal(replay.receipt.command_id, applied.receipt.command_id);
      assert.equal(
        writer
          .prepare('SELECT count(*) AS n FROM kernel_receipt_audit_log WHERE idempotency_key = ?')
          .get(preview.idempotencyKey).n,
        1,
      );
      assert.equal(
        writer
          .prepare('SELECT count(*) AS n FROM kernel_outbox WHERE command_id = ?')
          .get(applied.receipt.command_id).n,
        applied.receipt.event_ids.length,
      );
      assert.equal(
        writer
          .prepare('SELECT count(*) AS n FROM kernel_receipts WHERE idempotency_key = ?')
          .get(preview.idempotencyKey).n,
        1,
      );
      assert.equal(
        writer.prepare('SELECT version FROM orders WHERE id = ?').get(params.orderId).version,
        version + 1,
      );
      assert.deepEqual(
        reconcileOrder(database, kernel, { idempotencyKey: preview.idempotencyKey }).receipt,
        applied.receipt,
      );
      const otherPrincipal = { ...kernel, principal: { ...kernel.principal, id: 'user:other' } };
      assert.throws(
        () => reconcileOrder(database, otherPrincipal, { idempotencyKey: preview.idempotencyKey }),
        /does not match/,
      );
      await backend.close();
      backend = new TerminalBackend(database, {
        loadKernel: () => kernel,
        operationsDirectory: journal,
      });
      const preReconcile = await Promise.all([hash(database), hash(database + '-wal')]);
      const recovered = await backend.request('reconcile-order');
      assert.equal(recovered.receipt.command_id, applied.receipt.command_id);
      assert.match(recovered.message, /No command was repeated/);
      assert.deepEqual(
        await Promise.all([hash(database), hash(database + '-wal')]),
        preReconcile,
        'recovery must be a read-only lookup',
      );
      await assert.rejects(backend.request('order-apply', {}), /Unknown terminal operation/);
      await assert.rejects(backend.request('constructor', {}), /Unknown terminal operation/);
      await backend.close();
      backend = new TerminalBackend(database, {
        allowApply: true,
        loadKernel: () => kernel,
        operationsDirectory: journal,
      });
      // Repeat the independent SQL writer case across native reopen boundaries.
      for (let attempt = 0; attempt < 3; attempt++) {
        const stale = await backend.request('preview-order', {
          orderId: fixture.orders[1].id,
          status: 'confirmed',
        });
        const staleBefore = writer
          .prepare('SELECT status, version FROM orders WHERE id = ?')
          .get(fixture.orders[1].id);
        assert.equal(
          staleBefore.version,
          stale.receipt.version_before,
          JSON.stringify({ staleBefore, stale }),
        );
        const mutation = writer
          .prepare("UPDATE orders SET status = 'confirmed', version = version + 1 WHERE id = ?")
          .run(fixture.orders[1].id);
        assert.equal(mutation.changes, 1);
        const staleAfter = writer
          .prepare('SELECT status, version FROM orders WHERE id = ?')
          .get(fixture.orders[1].id);
        assert.equal(staleAfter.version, stale.receipt.version_before + 1);
        const conflict = await backend.request('apply-order', { ticketId: stale.ticketId });
        assert.notEqual(
          conflict.receipt.status,
          'succeeded',
          JSON.stringify({ attempt, staleBefore, staleAfter, stale, conflict }),
        );
        assert.equal(
          conflict.receipt.error_code,
          'kernel.version_conflict',
          JSON.stringify(conflict),
        );
        assert.equal(
          writer.prepare('SELECT version FROM orders WHERE id = ?').get(fixture.orders[1].id)
            .version,
          staleAfter.version,
        );
        if (attempt < 2)
          writer
            .prepare("UPDATE orders SET status = 'pending', version = version + 1 WHERE id = ?")
            .run(fixture.orders[1].id);
      }
      const paid = await backend.request('preview-order', {
        orderId: fixture.orders[3].id,
        status: 'cancelled',
      });
      assert.notEqual(paid.receipt.status, 'previewed');
      assert.equal(paid.applyAllowed, false);
      await assert.rejects(
        backend.request('apply-order', { ticketId: paid.ticketId }),
        /unavailable/,
      );
      assert.equal(
        writer.prepare('SELECT status FROM orders WHERE id = ?').get(fixture.orders[3].id).status,
        'pending',
      );
      previewOnly = new TerminalBackend(database, { loadKernel: () => kernel });
      const readonly = await previewOnly.request('preview-order', {
        orderId: fixture.orders[2].id,
        status: 'confirmed',
      });
      assert.equal(readonly.receipt.status, 'previewed');
      assert.equal(readonly.applyAllowed, false);
      await assert.rejects(
        previewOnly.request('apply-order', { ticketId: readonly.ticketId }),
        /Preview only/,
      );
      denied = new TerminalBackend(database, {
        allowApply: true,
        loadKernel: () => ({ ...kernel, principal: { ...kernel.principal, capabilities: [] } }),
        operationsDirectory: journal,
      });
      const refused = await denied.request('preview-order', {
        orderId: fixture.orders[2].id,
        status: 'confirmed',
      });
      assert.notEqual(refused.receipt.status, 'previewed');
      assert.equal(refused.applyAllowed, false);
      assert.match(JSON.stringify(refused.receipt), /capabilit/i);
      const processing = await backend.request('preview-order', {
        orderId: params.orderId,
        status: 'processing',
      });
      const competing = await previewOnly.request('preview-order', {
        orderId: params.orderId,
        status: 'processing',
      });
      // Separate sessions share no host ticket cache. Apply authority still
      // comes from this session's reviewed configuration, then from the kernel.
      const otherSession = new TerminalBackend(database, {
        allowApply: true,
        loadKernel: () => kernel,
        operationsDirectory: path.join(directory, 'other-history'),
      });
      try {
        const second = await otherSession.request('preview-order', {
          orderId: params.orderId,
          status: 'processing',
        });
        assert.equal(second.receipt.version_before, processing.receipt.version_before);
        assert.equal(competing.receipt.version_before, processing.receipt.version_before);
        assert.equal(
          (await backend.request('apply-order', { ticketId: processing.ticketId })).receipt.status,
          'succeeded',
        );
        const rejected = await otherSession.request('apply-order', { ticketId: second.ticketId });
        assert.equal(
          rejected.receipt.error_code,
          'kernel.version_conflict',
          JSON.stringify(rejected),
        );
      } finally {
        await otherSession.close();
      }
      const beforeCancel = await backend.request('snapshot');
      const cancellation = await backend.request('preview-order', {
        orderId: fixture.orders[2].id,
        status: 'cancelled',
      });
      assert.equal(cancellation.receipt.status, 'previewed');
      assert.equal(
        (await backend.request('apply-order', { ticketId: cancellation.ticketId })).receipt.status,
        'succeeded',
      );
      const afterCancel = await backend.request('snapshot');
      const available = (snapshot) =>
        Number(snapshot.inventory.find((row) => row.sku === 'OPERATOR-FIXTURE').available);
      assert.equal(
        available(afterCancel),
        available(beforeCancel) + 2,
        'cancellation releases native reservations',
      );
      assert.equal(
        writer.prepare('SELECT status FROM orders WHERE id = ?').get(fixture.orders[2].id).status,
        'cancelled',
      );
      passed = true;
    } finally {
      for (const active of [backend, previewOnly, denied]) if (active) await active.close();
      writer?.close();
      if (passed) await fs.rm(directory, { recursive: true, force: true });
      else console.error('Preserved order fixture:', directory);
    }
  },
);

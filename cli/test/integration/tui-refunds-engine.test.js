import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import Database from 'better-sqlite3';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { TerminalBackend } from '../../src/tui/backend.js';
import { refundKernel } from '../fixtures/tui-refunds.js';
import { hashFiles } from '../fixtures/sqlite-file-evidence.js';
import { addDecimals } from '../../src/utils/exact-decimal.js';
import { reconcileRefund } from '../../src/tui/refund-engine.js';
const execute = promisify(execFile);
const native = async (script, args) =>
  JSON.parse(
    (
      await execute(process.execPath, ['--input-type=module', '-e', script, ...args], {
        timeout: 45000,
      })
    ).stdout,
  );

test(
  'native refunds preview privately, reserve pending amounts, refuse stale over-refunds, replay once and recover read-only after restart',
  { timeout: 240000 },
  async () => {
    const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-refund-engine-'));
    const database = path.join(directory, 'owned.db'),
      journal = path.join(directory, 'history'),
      kernel = refundKernel();
    let backend,
      writer,
      passed = false;
    try {
      const fixture = await native(
        `import {refundFixture} from ${JSON.stringify(new URL('../fixtures/tui-refunds.js', import.meta.url).href)}; console.log(JSON.stringify(await refundFixture(process.argv[1])));`,
        [database],
      );
      writer = new Database(database);
      writer.pragma('journal_mode = WAL');
      writer.pragma('wal_autocheckpoint = 0');
      writer.prepare('UPDATE customers SET last_name = ?').run('Committed WAL fixture');
      const files = [database, database + '-wal'],
        before = await hashFiles(files);
      const params = { paymentId: fixture.payment.id, amount: '12.51', reason: 'Customer request' };
      backend = new TerminalBackend(database, {
        allowApply: true,
        loadKernel: () => kernel,
        operationsDirectory: journal,
      });
      const first = await backend.request('preview-refund', params);
      const competitor = await backend.request('preview-refund', { ...params, amount: '12.52' });
      assert.equal(first.receipt.status, 'previewed');
      assert.equal(competitor.receipt.status, 'previewed');
      assert.equal(first.payment.amountExact, '25.02');
      assert.equal(first.receipt.version_before, null);
      assert.deepEqual(await hashFiles(files), before);
      assert.equal(writer.prepare('SELECT count(*) AS n FROM refunds').get().n, 0);
      assert.equal(writer.prepare('SELECT count(*) AS n FROM kernel_receipts').get().n, 0);
      const applied = await backend.request('apply-refund', { ticketId: first.ticketId });
      assert.equal(applied.receipt.status, 'succeeded');
      assert.deepEqual(applied.reviewIssues, [], JSON.stringify(applied));
      assert.equal(applied.receipt.result.status, 'pending');
      assert.equal(applied.receipt.result.amount, '12.51');
      assert.equal(applied.receipt.result.payment_id, fixture.payment.id);
      assert.equal(
        writer.prepare('SELECT status FROM refunds WHERE id = ?').get(applied.receipt.aggregate_id)
          .status,
        'pending',
      );
      assert.equal(
        writer.prepare('SELECT amount_refunded FROM payments WHERE id = ?').get(fixture.payment.id)
          .amount_refunded,
        '0',
      );
      assert.equal(
        writer.prepare('SELECT payment_status FROM orders WHERE id = ?').get(fixture.orders[3].id)
          .payment_status,
        'paid',
      );
      const refused = await backend.request('apply-refund', { ticketId: competitor.ticketId });
      assert.notEqual(refused.receipt.status, 'succeeded', JSON.stringify(refused));
      assert.equal(
        refused.receipt.error_code,
        'commerce.refund.exceeds_captured',
        JSON.stringify(refused),
      );
      assert.equal(writer.prepare('SELECT count(*) AS n FROM refunds').get().n, 1);
      assert.equal(
        (await backend.request('apply-refund', { ticketId: first.ticketId })).replayed,
        true,
      );
      const replay = await native(
        `import {applyRefund} from ${JSON.stringify(new URL('../../src/tui/refund-engine.js', import.meta.url).href)}; console.log(JSON.stringify(await applyRefund(process.argv[1], JSON.parse(process.argv[2]), JSON.parse(process.argv[3]))));`,
        [
          database,
          JSON.stringify(kernel),
          JSON.stringify({
            params,
            context: { currency: 'USD' },
            expectedVersion: null,
            idempotencyKey: first.idempotencyKey,
          }),
        ],
      );
      assert.equal(replay.receipt.command_id, applied.receipt.command_id);
      assert.equal(
        writer
          .prepare('SELECT count(*) AS n FROM kernel_receipt_audit_log WHERE command_id = ?')
          .get(applied.receipt.command_id).n,
        1,
      );
      assert.ok(applied.receipt.event_ids.length > 0);
      for (const event of applied.receipt.event_ids)
        assert.equal(
          writer.prepare('SELECT command_id FROM kernel_outbox WHERE id = ?').get(event).command_id,
          applied.receipt.command_id,
        );
      const changedCurrency = await backend.request('preview-refund', params);
      assert.equal(changedCurrency.receipt.status, 'previewed');
      writer
        .prepare("UPDATE payments SET currency = 'EUR', version = version + 1 WHERE id = ?")
        .run(fixture.payment.id);
      const currencyRefusal = await backend.request('apply-refund', {
        ticketId: changedCurrency.ticketId,
      });
      assert.equal(
        currencyRefusal.receipt.error_code,
        'kernel.commitment_amount_mismatch',
        JSON.stringify(currencyRefusal),
      );
      assert.equal(writer.prepare('SELECT count(*) AS n FROM refunds').get().n, 1);
      writer
        .prepare("UPDATE payments SET currency = 'USD', version = version + 1 WHERE id = ?")
        .run(fixture.payment.id);
      const saved = JSON.parse(
        await fs.readFile(path.join(journal, `refund-action-${first.ticketId}.json`)),
      );
      await backend.close();
      backend = new TerminalBackend(database, {
        loadKernel: () => kernel,
        operationsDirectory: journal,
      });
      const recoveryBefore = await hashFiles(files);
      assert.equal(
        (await backend.request('reconcile-refund', { historyId: first.ticketId })).receipt
          .command_id,
        applied.receipt.command_id,
      );
      assert.deepEqual(await hashFiles(files), recoveryBefore);
      assert.deepEqual(
        JSON.parse(await fs.readFile(path.join(journal, `refund-action-${first.ticketId}.json`))),
        saved,
      );
      assert.equal((await backend.request('refund-history')).entries.length, 3);
      assert.deepEqual((await backend.request('order-history')).entries, []);
      await assert.rejects(
        backend.request('apply-refund', { ticketId: first.ticketId }),
        /Preview only/,
      );
      const wrong = { ...kernel, principal: { ...kernel.principal, id: 'user:other' } };
      assert.throws(
        () => reconcileRefund(database, wrong, { idempotencyKey: first.idempotencyKey }),
        /does not match/,
      );
      await backend.close();
      backend = new TerminalBackend(database, {
        allowApply: true,
        loadKernel: () => kernel,
        operationsDirectory: journal,
      });
      const remaining = await backend.request('preview-refund', params);
      assert.equal(remaining.receipt.status, 'previewed');
      assert.equal(
        (await backend.request('apply-refund', { ticketId: remaining.ticketId })).receipt.status,
        'succeeded',
      );
      const over = await backend.request('preview-refund', { ...params, amount: '0.01' });
      assert.notEqual(over.receipt.status, 'previewed');
      assert.equal(over.applyAllowed, false);
      await assert.rejects(
        backend.request('apply-refund', { ticketId: over.ticketId }),
        /unavailable/,
      );
      const amounts = writer
        .prepare('SELECT amount FROM refunds WHERE payment_id = ?')
        .all(fixture.payment.id)
        .map((row) => row.amount);
      assert.equal(addDecimals(amounts), '25.02');
      assert.equal(writer.prepare('SELECT count(*) AS n FROM kernel_receipts').get().n, 4);
      assert.equal(
        writer.prepare('SELECT amount_refunded FROM payments WHERE id = ?').get(fixture.payment.id)
          .amount_refunded,
        '0',
      );
      await backend.close();
      backend = new TerminalBackend(database, {
        allowApply: true,
        loadKernel: () => ({ ...kernel, principal: { ...kernel.principal, capabilities: [] } }),
        operationsDirectory: journal,
      });
      const denied = await backend.request('preview-refund', params);
      assert.notEqual(denied.receipt.status, 'previewed');
      assert.equal(denied.applyAllowed, false);
      assert.equal(writer.prepare('SELECT count(*) AS n FROM refunds').get().n, 2);
      await backend.close();
      backend = new TerminalBackend(database, {
        allowApply: true,
        loadKernel: () => kernel,
        operationsDirectory: journal,
      });
      const stable = await backend.request('preview-refund', {
        paymentId: fixture.precisionPayment.id,
        amount: '0.00000001',
        reason: 'Exact high-precision request',
      });
      assert.equal(stable.receipt.status, 'previewed', JSON.stringify(stable));
      assert.equal(stable.payment.currency, 'BTC');
      const stableRefund = await backend.request('apply-refund', { ticketId: stable.ticketId });
      assert.equal(stableRefund.receipt.status, 'succeeded', JSON.stringify(stableRefund));
      assert.deepEqual(stableRefund.reviewIssues, []);
      assert.equal(stableRefund.receipt.result.amount, '0.00000001');
      assert.equal(stableRefund.receipt.result.currency, 'BTC');
      assert.equal(stableRefund.receipt.result.status, 'pending');
      passed = true;
    } catch (error) {
      error.message += `\nPreserved refund fixture: ${directory}`;
      throw error;
    } finally {
      await backend?.close();
      writer?.close();
      if (passed) await fs.rm(directory, { recursive: true, force: true });
    }
  },
);

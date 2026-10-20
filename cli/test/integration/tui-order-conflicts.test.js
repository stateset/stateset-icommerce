import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import Database from 'better-sqlite3';
import { createRequire } from 'node:module';
import { TerminalBackend } from '../../src/tui/backend.js';
import { reconcileOrder } from '../../src/tui/order-engine.js';
import { hashFiles } from '../fixtures/sqlite-file-evidence.js';
import { operatorKernel } from '../fixtures/tui-orders.js';

const execute = promisify(execFile);
const require = createRequire(import.meta.url);

test(
  'stale order tickets refuse external commits without an intervening database read',
  { timeout: 240000 },
  async () => {
    const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-order-conflict-'));
    const database = path.join(directory, 'owned.db');
    let backend,
      writer,
      passed = false;
    try {
      const { stdout } = await execute(
        process.execPath,
        [
          '--input-type=module',
          '-e',
          `import {orderFixture} from ${JSON.stringify(new URL('../fixtures/tui-orders.js', import.meta.url).href)}; console.log(JSON.stringify(await orderFixture(process.argv[1])));`,
          database,
        ],
        { timeout: 45000 },
      );
      const fixture = JSON.parse(stdout),
        kernel = operatorKernel(),
        orderId = fixture.orders[1].id;
      writer = new Database(database);
      writer.pragma('journal_mode = WAL');
      writer.pragma('wal_autocheckpoint = 0');
      backend = new TerminalBackend(database, {
        allowApply: true,
        loadKernel: () => kernel,
        operationsDirectory: path.join(directory, 'history'),
      });
      writer.prepare('UPDATE customers SET last_name = ?').run('WAL fixture');
      await hashFiles([database, database + '-wal']);
      // Match the read/reopen sequence that preceded the CI failure. In
      // particular, reconciliation opens and closes another SQLite handle in
      // the writer process before a child reopens the native store.
      const first = await backend.request('preview-order', {
        orderId: fixture.orders[0].id,
        status: 'confirmed',
      });
      await backend.request('apply-order', { ticketId: first.ticketId });
      await execute(
        process.execPath,
        [
          '--input-type=module',
          '-e',
          `import {applyOrder} from ${JSON.stringify(new URL('../../src/tui/order-engine.js', import.meta.url).href)}; await applyOrder(process.argv[1],JSON.parse(process.argv[2]),JSON.parse(process.argv[3]));`,
          database,
          JSON.stringify(kernel),
          JSON.stringify({
            params: { orderId: fixture.orders[0].id, status: 'confirmed' },
            expectedVersion: first.receipt.version_before,
            idempotencyKey: first.idempotencyKey,
          }),
        ],
        { timeout: 45000 },
      );
      reconcileOrder(database, kernel, { idempotencyKey: first.idempotencyKey });
      assert.throws(
        () =>
          reconcileOrder(
            database,
            { ...kernel, principal: { ...kernel.principal, id: 'user:other' } },
            { idempotencyKey: first.idempotencyKey },
          ),
        /does not match/,
      );
      await backend.close();
      backend = new TerminalBackend(database, {
        loadKernel: () => kernel,
        operationsDirectory: path.join(directory, 'history'),
      });
      await backend.request('reconcile-order');
      await backend.close();
      backend = new TerminalBackend(database, {
        allowApply: true,
        loadKernel: () => kernel,
        operationsDirectory: path.join(directory, 'history'),
      });
      for (let attempt = 0; attempt < 8; attempt++) {
        const preview = await backend.request('preview-order', { orderId, status: 'confirmed' });
        assert.equal(preview.receipt.status, 'previewed', JSON.stringify(preview));
        const expectedVersion = preview.receipt.version_before;
        // Do not insert a diagnostic SELECT, await or checkpoint between this
        // independent commit and apply: the earlier anomaly occurred here.
        const changedStatus = attempt % 2 === 0;
        const mutation = writer
          .prepare(
            changedStatus
              ? "UPDATE orders SET status = 'confirmed', version = version + 1 WHERE id = ?"
              : 'UPDATE orders SET version = version + 1 WHERE id = ?',
          )
          .run(orderId);
        const inTransaction = writer.inTransaction;
        const applied = await backend.request('apply-order', { ticketId: preview.ticketId });
        const row = writer
          .prepare('SELECT id, status, version FROM orders WHERE id = ?')
          .get(orderId);
        const stored = writer
          .prepare('SELECT receipt, request_hash FROM kernel_receipts WHERE idempotency_key = ?')
          .get(preview.idempotencyKey);
        const evidence = {
          attempt,
          changedStatus,
          expectedVersion,
          mutation,
          inTransaction,
          row,
          preview,
          applied,
          stored: stored ? JSON.parse(stored.receipt) : null,
          nativeRequestHash: stored?.request_hash,
          audit: writer
            .prepare('SELECT * FROM kernel_receipt_audit_log WHERE command_id = ?')
            .all(applied.receipt.command_id),
          files: await fileEvidence(database),
          intent: JSON.parse(
            await fs.readFile(
              path.join(directory, 'history', `order-action-${preview.ticketId}.json`),
              'utf8',
            ),
          ),
        };
        await fs.writeFile(
          path.join(directory, `attempt-${attempt}.json`),
          JSON.stringify(evidence, null, 2) + '\n',
          { mode: 0o600 },
        );
        assert.equal(mutation.changes, 1, JSON.stringify(evidence));
        assert.ok(
          !evidence.files.descriptors.some((file) => file.target.endsWith('(deleted)')),
          JSON.stringify(evidence),
        );
        assert.equal(inTransaction, false, JSON.stringify(evidence));
        assert.equal(applied.receipt.status, 'rejected', JSON.stringify(evidence));
        assert.equal(
          applied.receipt.error_code,
          'kernel.version_conflict',
          JSON.stringify(evidence),
        );
        assert.equal(applied.receipt.version_before, expectedVersion + 1, JSON.stringify(evidence));
        assert.equal(row.version, expectedVersion + 1, JSON.stringify(evidence));
        assert.equal(row.status, changedStatus ? 'confirmed' : 'pending', JSON.stringify(evidence));
        assert.deepEqual(evidence.stored, applied.receipt);
        // Reset only this owned fixture for another native reopen boundary.
        if (changedStatus)
          writer
            .prepare("UPDATE orders SET status = 'pending', version = version + 1 WHERE id = ?")
            .run(orderId);
      }
      passed = true;
    } finally {
      await backend?.close();
      writer?.close();
      if (passed) {
        for (const suffix of ['', '-wal', '-shm']) await fs.rm(database + suffix, { force: true });
        await fs.rm(path.join(directory, 'history'), { recursive: true, force: true });
        console.error('Order conflict evidence:', directory);
      } else console.error('Preserved order conflict evidence:', directory);
    }
  },
);

async function fileEvidence(database) {
  const result = {
    paths: [],
    descriptors: [],
    nativeModules: Object.keys(require.cache).filter((file) => file.endsWith('.node')),
  };
  for (const suffix of ['', '-wal', '-shm']) {
    try {
      const info = await fs.stat(database + suffix, { bigint: true });
      result.paths.push({
        path: database + suffix,
        inode: String(info.ino),
        device: String(info.dev),
        bytes: String(info.size),
      });
    } catch (error) {
      if (error.code !== 'ENOENT') throw error;
    }
  }
  // Linux evidence is optional; absent /proc is normal on other platforms.
  try {
    for (const fd of await fs.readdir('/proc/self/fd')) {
      try {
        const file = '/proc/self/fd/' + fd,
          target = await fs.readlink(file);
        if (!target.startsWith(database)) continue;
        const info = await fs.stat(file, { bigint: true });
        result.descriptors.push({ fd, target, inode: String(info.ino), device: String(info.dev) });
      } catch {
        /* The directory scan can race unrelated descriptor closes. */
      }
    }
  } catch {
    /* Non-Linux hosts do not expose /proc/self/fd. */
  }
  return result;
}

test(
  'file evidence hashing preserves live WAL ownership and visibility across a peer close',
  { timeout: 90000 },
  async () => {
    const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-evidence-lock-'));
    const database = path.join(directory, 'owned.db'),
      writer = new Database(database);
    const reader = `const Database=require(process.argv[1]);const db=new Database(process.argv[2]);try{console.log(JSON.stringify(db.prepare('SELECT version FROM counter').get()));}finally{db.close();}`;
    const observe = async () =>
      JSON.parse(
        (
          await execute(
            process.execPath,
            ['-e', reader, require.resolve('better-sqlite3'), database],
            { timeout: 45000 },
          )
        ).stdout,
      );
    try {
      writer.pragma('journal_mode=WAL');
      writer.pragma('wal_autocheckpoint=0');
      writer.exec('CREATE TABLE counter(version INTEGER NOT NULL);INSERT INTO counter VALUES(1)');
      const hashes = await hashFiles([database, database + '-wal']);
      assert.equal(hashes.length, 2);
      assert.ok(hashes.every((value) => /^[0-9a-f]{64}$/.test(value)));
      // With an in-process fs.readFile hash this peer can acquire the writer's
      // lost file lock and unlink its WAL. Its next write then becomes invisible.
      assert.equal((await observe()).version, 1);
      writer.prepare('UPDATE counter SET version=version+1').run();
      assert.equal(writer.inTransaction, false);
      assert.equal(writer.prepare('SELECT version FROM counter').get().version, 2);
      assert.equal((await observe()).version, 2, 'hashing must not orphan an active writer WAL');
    } finally {
      writer.close();
      await fs.rm(directory, { recursive: true, force: true });
    }
  },
);

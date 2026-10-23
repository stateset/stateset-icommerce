import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs/promises';
import path from 'node:path';
import { createHash } from 'node:crypto';
import Database from 'better-sqlite3';
import { TerminalBackend } from '../../src/tui/backend.js';

const hash = async (file) =>
  createHash('sha256')
    .update(await fs.readFile(file))
    .digest('hex');

test(
  'real native demo, WAL snapshot, missing source and isolated planning',
  // Five separately bounded (45s) requests may run sequentially on a busy
  // host. This outer budget does not extend any individual engine deadline.
  { timeout: 240000 },
  async () => {
    const demo = new TerminalBackend();
    let live;
    try {
      // An interrupted initial constructor/seed must not turn a later retry into
      // an apparently healthy, empty demo. Only the private demo can be rebuilt.
      await fs.writeFile(path.join(await demo.directory, 'demo.db'), 'interrupted setup');
      console.error('TUI acceptance: rebuild interrupted private demo');
      const data = await demo.request('snapshot');
      assert.deepEqual(data.errors, []);
      assert.equal(data.counts.orders, 12);
      assert.equal(data.counts.customers, 3);
      assert.ok(data.orders.every((order) => typeof order.totalAmountExact === 'string'));
      assert.ok(data.inventory.some((item) => item.sku === 'MUG-STONE'));
      const directory = await demo.directory,
        source = path.join(directory, 'demo.db');
      const writer = new Database(source);
      try {
        writer.pragma('journal_mode = WAL');
        writer.pragma('wal_autocheckpoint = 0');
        writer
          .prepare("UPDATE customers SET email = ? WHERE first_name = 'Ada'")
          .run('wal-visible@demo.example');
        const before = await Promise.all([hash(source), hash(source + '-wal')]);
        live = new TerminalBackend(source);
        console.error('TUI acceptance: WAL-consistent source snapshot');
        const snapshot = await live.request('snapshot');
        assert.deepEqual(snapshot.errors, []);
        assert.ok(
          snapshot.customers.some((customer) => customer.email === 'wal-visible@demo.example'),
        );
        assert.deepEqual(
          await Promise.all([hash(source), hash(source + '-wal')]),
          before,
          'Source database and WAL must be unchanged',
        );
      } finally {
        writer.close();
      }
      const missing = path.join(directory, 'does-not-exist.db');
      const absent = new TerminalBackend(missing);
      try {
        console.error('TUI acceptance: missing source refuses without creation');
        await assert.rejects(absent.request('snapshot'), /exist|open|unable/i);
        await assert.rejects(fs.stat(missing), { code: 'ENOENT' });
        console.error('TUI acceptance: isolated supplier-delay simulation');
        const simulation = await absent.request('simulate', { preset: 'delay', seed: 42 });
        assert.equal(simulation.success, true);
        assert.equal(simulation.liveStoreRead, false);
        assert.equal(simulation.results.length, 2);
        assert.ok(simulation.results.every((r) => typeof r.summary.endingCash === 'string'));
        console.error('TUI acceptance: simulation left original demo unchanged');
        assert.equal((await demo.request('snapshot')).counts.orders, 12);
      } finally {
        await absent.close();
      }
    } finally {
      if (live) await live.close();
      await demo.close();
      await assert.rejects(fs.stat(await demo.directory), { code: 'ENOENT' });
    }
  },
);

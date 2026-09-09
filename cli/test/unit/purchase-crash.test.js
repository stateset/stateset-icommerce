import assert from 'node:assert/strict';
import test from 'node:test';
import { fork } from 'node:child_process';
import { once } from 'node:events';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

test(
  'SIGKILL after provider payment commit recovers one purchase without duplicate effects',
  { timeout: 60000 },
  async () => {
    const directory = mkdtempSync(join(tmpdir(), 'purchase-crash-'));
    const children = [];
    function worker(mode) {
      const child = fork(
        new URL('../fixtures/purchase-crash-worker.mjs', import.meta.url),
        [directory, mode],
        { stdio: ['ignore', 'ignore', 'pipe', 'ipc'] },
      );
      children.push(child);
      let stderr = '';
      child.stderr.on('data', (chunk) => {
        stderr += chunk;
      });
      const exit = once(child, 'exit');
      const message = new Promise((resolve, reject) => {
        child.once('message', resolve);
        child.once('error', reject);
        child.once('exit', (code, signal) =>
          reject(new Error(`worker exited ${code}/${signal}: ${stderr}`)),
        );
      });
      return { child, exit, message };
    }
    try {
      const first = worker('crash');
      assert.equal((await first.message).event, 'payment_committed');
      first.child.kill('SIGKILL');
      assert.equal((await first.exit)[1], 'SIGKILL');
      const second = worker('recover');
      const result = await second.message;
      assert.equal((await second.exit)[0], 0);
      assert.equal(result.before.reserved, '40');
      assert.equal(result.before.spent, '0');
      const operation = result.recovery.results[0].operation;
      assert.equal(operation.status, 'completed');
      assert.equal(result.replay.id, operation.id);
      assert.deepEqual(result.replay.receipt, operation.receipt);
      assert.equal(operation.receipt.evidence.pay.transaction_id, 'local-provider:one');
      assert.deepEqual(Object.fromEntries(result.counts.map(({ step, count }) => [step, count])), {
        reserve_inventory: 1,
        pay: 1,
        create_order: 1,
        confirm_inventory: 1,
      });
      assert.equal(result.stock.available, 8);
      assert.equal(result.budget.reserved, '0');
      assert.equal(result.budget.spent, '40');
    } finally {
      for (const child of children) {
        if (child.exitCode === null && child.signalCode === null) {
          const exit = once(child, 'exit');
          child.kill('SIGKILL');
          await exit;
        }
      }
      rmSync(directory, { recursive: true, force: true });
    }
  },
);

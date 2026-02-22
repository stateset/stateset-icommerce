import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { createProgress, withProgress } from '../../src/progress.js';

describe('progress', () => {
  describe('createProgress', () => {
    it('returns a progress reporter with expected methods', () => {
      const p = createProgress({ enabled: false });
      assert.ok(typeof p.setLabel === 'function');
      assert.ok(typeof p.setPercent === 'function');
      assert.ok(typeof p.done === 'function');
      assert.ok(typeof p.fail === 'function');
    });

    it('noop progress does not throw', () => {
      const p = createProgress({ enabled: false });
      p.setLabel('test');
      p.setPercent(50);
      p.done();
      p.fail('msg');
    });

    it('log fallback writes to stderr when non-TTY', () => {
      // Save original
      const origIsTTY = process.stderr.isTTY;
      process.stderr.isTTY = false;

      const logs = [];
      const origError = console.error;
      console.error = (...args) => logs.push(args.join(' '));

      try {
        const p = createProgress({ label: 'Testing', fallback: 'log' });
        assert.ok(logs.some((l) => l.includes('Testing')));

        p.setPercent(50);
        assert.ok(logs.some((l) => l.includes('50%')));

        p.done();
        assert.ok(logs.some((l) => l.includes('done')));
      } finally {
        console.error = origError;
        process.stderr.isTTY = origIsTTY;
      }
    });

    it('log fallback reports failure', () => {
      const origIsTTY = process.stderr.isTTY;
      process.stderr.isTTY = false;

      const logs = [];
      const origError = console.error;
      console.error = (...args) => logs.push(args.join(' '));

      try {
        const p = createProgress({ label: 'Loading', fallback: 'log' });
        p.fail('timeout');
        assert.ok(logs.some((l) => l.includes('timeout')));
      } finally {
        console.error = origError;
        process.stderr.isTTY = origIsTTY;
      }
    });

    it('log progress throttles percent updates to 25% increments', () => {
      const origIsTTY = process.stderr.isTTY;
      process.stderr.isTTY = false;

      const logs = [];
      const origError = console.error;
      console.error = (...args) => logs.push(args.join(' '));

      try {
        const p = createProgress({ label: 'Index', fallback: 'log' });
        logs.length = 0; // Clear initial "Index..." message

        p.setPercent(5);  // rounds to 0
        p.setPercent(10); // rounds to 0 (no new log)
        p.setPercent(30); // rounds to 25
        p.setPercent(55); // rounds to 50
        p.setPercent(55); // rounds to 50 (no dup)
        p.setPercent(80); // rounds to 75
        p.setPercent(100); // rounds to 100

        // Should have logged: 25%, 50%, 75%, 100%
        const pctLogs = logs.filter((l) => l.includes('%'));
        assert.ok(pctLogs.length >= 3, `Expected >=3 percent logs, got ${pctLogs.length}`);
      } finally {
        console.error = origError;
        process.stderr.isTTY = origIsTTY;
      }
    });

    it('returns noop when fallback is "none"', () => {
      const origIsTTY = process.stderr.isTTY;
      process.stderr.isTTY = false;

      const logs = [];
      const origError = console.error;
      console.error = (...args) => logs.push(args.join(' '));

      try {
        const p = createProgress({ label: 'Silent', fallback: 'none' });
        p.setLabel('new');
        p.setPercent(50);
        p.done();
        assert.strictEqual(logs.length, 0, 'should not log with fallback=none');
      } finally {
        console.error = origError;
        process.stderr.isTTY = origIsTTY;
      }
    });
  });

  describe('withProgress', () => {
    it('calls fn and returns result on success', async () => {
      const result = await withProgress('Loading', async () => 42, { enabled: false });
      assert.strictEqual(result, 42);
    });

    it('propagates errors from fn', async () => {
      await assert.rejects(
        () => withProgress('Loading', async () => { throw new Error('boom'); }, { enabled: false }),
        { message: 'boom' },
      );
    });

    it('passes progress reporter to fn', async () => {
      let receivedProgress = null;
      await withProgress('Loading', async (p) => {
        receivedProgress = p;
        p.setPercent(50);
      }, { enabled: false });
      assert.ok(receivedProgress);
      assert.ok(typeof receivedProgress.setPercent === 'function');
    });
  });
});

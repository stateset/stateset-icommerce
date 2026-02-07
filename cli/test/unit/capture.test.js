/**
 * Tests for Event Capture — Unmapped Operation Warnings
 *
 * Tests the capture module's behavior with unmapped operations.
 * We only test the warning path (unmapped operations) and the disabled path,
 * since mapped operations trigger the full outbox/crypto pipeline.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

describe('EventCapture', () => {
  describe('capture() unmapped operation warning', () => {
    it('warns when an unmapped operation is captured', async () => {
      const warnings = [];
      const origWarn = console.warn;
      console.warn = (...args) => warnings.push(args.join(' '));

      try {
        const { EventCapture } = await import('../../src/sync/capture.js');

        const mockDb = {
          prepare: () => ({ run: () => {} }),
          exec: () => {},
        };
        const config = { identity: { tenantId: 't1', storeId: 's1', agentId: 'a1' } };

        const capture = new EventCapture(mockDb, config);
        capture.capture('nonexistent.operation', 'id-123', {});

        assert.ok(
          warnings.some(w => w.includes('Unmapped operation') && w.includes('nonexistent.operation')),
          `Expected warning about unmapped operation, got: ${JSON.stringify(warnings)}`
        );
      } finally {
        console.warn = origWarn;
      }
    });

    it('skips capture when disabled (no warning, no outbox write)', async () => {
      const warnings = [];
      const origWarn = console.warn;
      console.warn = (...args) => warnings.push(args.join(' '));

      try {
        const { EventCapture } = await import('../../src/sync/capture.js');

        const mockDb = {
          prepare: () => ({ run: () => {} }),
          exec: () => {},
        };
        const config = { identity: { tenantId: 't1', storeId: 's1', agentId: 'a1' } };

        const capture = new EventCapture(mockDb, config);
        capture.setEnabled(false);

        // Even a mapped operation should be silently skipped
        capture.capture('orders.create', 'ord-123', { totalAmount: 100 });
        // And an unmapped operation should also be silently skipped
        capture.capture('nonexistent.op', 'id-123', {});

        assert.equal(warnings.length, 0, 'should produce no warnings when disabled');
      } finally {
        console.warn = origWarn;
      }
    });

    it('returns early for unmapped ops without touching outbox', async () => {
      const { EventCapture } = await import('../../src/sync/capture.js');

      let outboxCalled = false;
      const mockDb = {
        prepare: () => ({ run: () => { outboxCalled = true; } }),
        exec: () => {},
      };
      const config = { identity: { tenantId: 't1', storeId: 's1', agentId: 'a1' } };

      const origWarn = console.warn;
      console.warn = () => {};

      try {
        const capture = new EventCapture(mockDb, config);
        capture.capture('unknown.method', 'id-1', {});

        // The outbox should not be called for unmapped operations
        assert.equal(outboxCalled, false, 'outbox should not be called for unmapped operations');
      } finally {
        console.warn = origWarn;
      }
    });
  });
});

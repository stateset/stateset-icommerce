/**
 * Unit tests for a2a/fan-out.js — Fan-Out/Join Coordinator
 *
 * Covers: scatter, registerResponse, join (all/first/majority/quorum/best),
 * getStatus, timeout handling, quote aggregation, coordination independence,
 * error handling, and cleanup.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { createFanOutCoordinator } from '../../src/a2a/fan-out.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeTargets(n = 3) {
  return Array.from({ length: n }, (_, i) => `0xAgent${i + 1}`);
}

/**
 * Create a coordinator, run the test body, and destroy the coordinator
 * afterwards to prevent timer leaks.
 */
async function withCoordinator(fn) {
  const coordinator = createFanOutCoordinator();
  try {
    await fn(coordinator);
  } finally {
    coordinator.destroy();
  }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('Fan-Out Coordinator', () => {

  // 1. scatter creates coordination with N targets
  describe('scatter', () => {
    it('creates a coordination with all targets listed as pending', async () => {
      await withCoordinator((coordinator) => {
        const targets = makeTargets(3);
        const id = coordinator.scatter({
          agentAddress: '0xRequester',
          targets,
          taskType: 'quote',
          payload: { items: ['widget'] },
          timeoutMs: 60000,
          joinStrategy: 'all',
        });

        assert.ok(typeof id === 'string' && id.length > 0);

        const status = coordinator.getStatus(id);
        assert.equal(status.status, 'pending');
        assert.deepEqual(status.targets, targets);
        assert.deepEqual(status.pending, targets);
        assert.equal(status.completedCount, 0);
        assert.equal(status.totalCount, 3);
      });
    });

    it('throws when targets is empty', async () => {
      await withCoordinator((coordinator) => {
        assert.throws(
          () => coordinator.scatter({
            agentAddress: '0x1', targets: [], taskType: 'quote', payload: {},
          }),
          /non-empty/,
        );
      });
    });

    it('throws when agentAddress is missing', async () => {
      await withCoordinator((coordinator) => {
        assert.throws(
          () => coordinator.scatter({ targets: ['0x1'], taskType: 'quote', payload: {} }),
          /agentAddress/,
        );
      });
    });

    it('throws when taskType is missing', async () => {
      await withCoordinator((coordinator) => {
        assert.throws(
          () => coordinator.scatter({ agentAddress: '0x1', targets: ['0x1'], payload: {} }),
          /taskType/,
        );
      });
    });
  });

  // 2. registerResponse records agent response
  describe('registerResponse', () => {
    it('records a valid response and removes from pending', async () => {
      await withCoordinator((coordinator) => {
        const targets = makeTargets(3);
        const id = coordinator.scatter({
          agentAddress: '0xRequester',
          targets,
          taskType: 'custom',
          payload: {},
          timeoutMs: 60000,
          joinStrategy: 'all',
        });

        const result = coordinator.registerResponse(id, '0xAgent1', { data: 'hello' });
        assert.equal(result.accepted, true);
        assert.equal(result.completedCount, 1);
        assert.equal(result.totalCount, 3);

        const status = coordinator.getStatus(id);
        assert.equal(status.responses.length, 1);
        assert.equal(status.responses[0].responderAddress, '0xAgent1');
        assert.deepEqual(status.responses[0].response, { data: 'hello' });
        assert.ok(!status.pending.includes('0xAgent1'));
      });
    });

    it('rejects duplicate responses from the same agent', async () => {
      await withCoordinator((coordinator) => {
        const id = coordinator.scatter({
          agentAddress: '0x1',
          targets: ['0xA', '0xB'],
          taskType: 'custom',
          payload: {},
          timeoutMs: 60000,
          joinStrategy: 'all',
        });

        coordinator.registerResponse(id, '0xA', { v: 1 });
        const dup = coordinator.registerResponse(id, '0xA', { v: 2 });
        assert.equal(dup.accepted, false);
        assert.equal(dup.completedCount, 1);
      });
    });

    it('throws for non-target responder', async () => {
      await withCoordinator((coordinator) => {
        const id = coordinator.scatter({
          agentAddress: '0x1',
          targets: ['0xA'],
          taskType: 'custom',
          payload: {},
          timeoutMs: 60000,
          joinStrategy: 'all',
        });

        assert.throws(
          () => coordinator.registerResponse(id, '0xIntruder', {}),
          /not a target/,
        );
      });
    });

    it('throws for unknown coordination ID', async () => {
      await withCoordinator((coordinator) => {
        assert.throws(
          () => coordinator.registerResponse('fake-id', '0xA', {}),
          /not found/i,
        );
      });
    });
  });

  // 3. join with 'all' waits for all responses
  describe('join — all strategy', () => {
    it('resolves when all targets respond', async () => {
      await withCoordinator(async (coordinator) => {
        const targets = makeTargets(3);
        const id = coordinator.scatter({
          agentAddress: '0xRequester',
          targets,
          taskType: 'custom',
          payload: {},
          timeoutMs: 60000,
          joinStrategy: 'all',
        });

        // Register all responses synchronously before join
        coordinator.registerResponse(id, '0xAgent1', { ok: true });
        coordinator.registerResponse(id, '0xAgent2', { ok: true });
        coordinator.registerResponse(id, '0xAgent3', { ok: true });

        const result = await coordinator.join(id);
        assert.equal(result.status, 'completed');
        assert.equal(result.completedCount, 3);
        assert.equal(result.timedOutCount, 0);
      });
    });
  });

  // 4. join with 'first' returns on first response
  describe('join — first strategy', () => {
    it('resolves as soon as one target responds', async () => {
      await withCoordinator(async (coordinator) => {
        const targets = makeTargets(3);
        const id = coordinator.scatter({
          agentAddress: '0xRequester',
          targets,
          taskType: 'custom',
          payload: {},
          timeoutMs: 60000,
          joinStrategy: 'first',
        });

        // Start waiting for join
        const joinPromise = coordinator.join(id);

        // Send one response — triggers completion synchronously
        coordinator.registerResponse(id, '0xAgent2', { fast: true });

        const result = await joinPromise;
        assert.equal(result.status, 'completed');
        assert.equal(result.completedCount, 1);
        assert.equal(result.responses[0].responderAddress, '0xAgent2');
      });
    });
  });

  // 5. join with 'majority' returns when >50% respond
  describe('join — majority strategy', () => {
    it('resolves when more than half of targets respond', async () => {
      await withCoordinator(async (coordinator) => {
        const targets = makeTargets(5);
        const id = coordinator.scatter({
          agentAddress: '0xRequester',
          targets,
          taskType: 'custom',
          payload: {},
          timeoutMs: 60000,
          joinStrategy: 'majority',
        });

        const joinPromise = coordinator.join(id);

        // 2 out of 5 — not enough
        coordinator.registerResponse(id, '0xAgent1', { v: 1 });
        coordinator.registerResponse(id, '0xAgent2', { v: 2 });

        // Status should still be pending
        const midStatus = coordinator.getStatus(id);
        assert.equal(midStatus.status, 'pending');

        // 3rd response -> 3/5 > 50% — should resolve
        coordinator.registerResponse(id, '0xAgent3', { v: 3 });

        const result = await joinPromise;
        assert.equal(result.status, 'completed');
        assert.equal(result.completedCount, 3);
      });
    });
  });

  // 6. join with 'quorum(2)' returns after 2 responses
  describe('join — quorum strategy', () => {
    it('resolves after N responses', async () => {
      await withCoordinator(async (coordinator) => {
        const targets = makeTargets(5);
        const id = coordinator.scatter({
          agentAddress: '0xRequester',
          targets,
          taskType: 'custom',
          payload: {},
          timeoutMs: 60000,
          joinStrategy: 'quorum(2)',
        });

        const joinPromise = coordinator.join(id);

        coordinator.registerResponse(id, '0xAgent4', { v: 4 });
        // 1/5 — not enough for quorum(2)
        const s1 = coordinator.getStatus(id);
        assert.equal(s1.status, 'pending');

        coordinator.registerResponse(id, '0xAgent5', { v: 5 });

        const result = await joinPromise;
        assert.equal(result.status, 'completed');
        assert.equal(result.completedCount, 2);
      });
    });
  });

  // 7. Timeout marks non-responders as timed_out
  describe('timeout handling', () => {
    it('marks non-responders as timed_out after timeout', async () => {
      const coordinator = createFanOutCoordinator();
      try {
        const targets = makeTargets(3);
        const id = coordinator.scatter({
          agentAddress: '0xRequester',
          targets,
          taskType: 'custom',
          payload: {},
          timeoutMs: 50,
          joinStrategy: 'all',
        });

        // Only one responds before timeout
        coordinator.registerResponse(id, '0xAgent1', { ok: true });

        const result = await coordinator.join(id);
        assert.equal(result.status, 'completed');
        assert.equal(result.completedCount, 1);
        assert.equal(result.timedOutCount, 2);
        assert.ok(result.timedOut.includes('0xAgent2'));
        assert.ok(result.timedOut.includes('0xAgent3'));
      } finally {
        coordinator.destroy();
      }
    });

    it('rejects late responses after timeout', async () => {
      const coordinator = createFanOutCoordinator();
      try {
        const targets = makeTargets(2);
        const id = coordinator.scatter({
          agentAddress: '0xRequester',
          targets,
          taskType: 'custom',
          payload: {},
          timeoutMs: 50,
          joinStrategy: 'all',
        });

        // Wait for timeout
        await coordinator.join(id);

        // Late response should be rejected
        const late = coordinator.registerResponse(id, '0xAgent1', { late: true });
        assert.equal(late.accepted, false);
      } finally {
        coordinator.destroy();
      }
    });
  });

  // 8. getStatus returns current progress
  describe('getStatus', () => {
    it('returns all fields for an in-progress coordination', async () => {
      await withCoordinator((coordinator) => {
        const targets = makeTargets(3);
        const id = coordinator.scatter({
          agentAddress: '0xRequester',
          targets,
          taskType: 'quote',
          payload: { sku: 'W1' },
          timeoutMs: 60000,
          joinStrategy: 'all',
        });

        coordinator.registerResponse(id, '0xAgent1', { price: 100 });

        const status = coordinator.getStatus(id);
        assert.equal(status.id, id);
        assert.equal(status.status, 'pending');
        assert.equal(status.taskType, 'quote');
        assert.equal(status.agentAddress, '0xRequester');
        assert.deepEqual(status.targets, targets);
        assert.equal(status.responses.length, 1);
        assert.deepEqual(status.pending, ['0xAgent2', '0xAgent3']);
        assert.equal(status.completedCount, 1);
        assert.equal(status.totalCount, 3);
        assert.ok(status.createdAt);
      });
    });

    it('throws for unknown coordination ID', async () => {
      await withCoordinator((coordinator) => {
        assert.throws(() => coordinator.getStatus('nonexistent'), /not found/i);
      });
    });
  });

  // 9. Quote aggregation sorts by price
  describe('quote aggregation', () => {
    it('sorts responses by price ascending', async () => {
      await withCoordinator(async (coordinator) => {
        const targets = makeTargets(3);
        const id = coordinator.scatter({
          agentAddress: '0xRequester',
          targets,
          taskType: 'quote',
          payload: { items: ['widget'] },
          timeoutMs: 60000,
          joinStrategy: 'all',
        });

        coordinator.registerResponse(id, '0xAgent1', { price: 150 });
        coordinator.registerResponse(id, '0xAgent2', { price: 85 });
        coordinator.registerResponse(id, '0xAgent3', { price: 120 });

        const result = await coordinator.join(id);
        assert.equal(result.aggregation.type, 'ranked_quotes');
        assert.equal(result.aggregation.bestPrice, 85);
        assert.equal(result.aggregation.bestResponder, '0xAgent2');
        assert.equal(result.aggregation.data[0].response.price, 85);
        assert.equal(result.aggregation.data[1].response.price, 120);
        assert.equal(result.aggregation.data[2].response.price, 150);
      });
    });
  });

  // 10. Multiple coordinations are independent
  describe('coordination independence', () => {
    it('manages multiple coordinations without cross-contamination', async () => {
      await withCoordinator(async (coordinator) => {
        const id1 = coordinator.scatter({
          agentAddress: '0xReq1',
          targets: ['0xA', '0xB'],
          taskType: 'custom',
          payload: { group: 1 },
          timeoutMs: 60000,
          joinStrategy: 'all',
        });

        const id2 = coordinator.scatter({
          agentAddress: '0xReq2',
          targets: ['0xC', '0xD'],
          taskType: 'quote',
          payload: { group: 2 },
          timeoutMs: 60000,
          joinStrategy: 'all',
        });

        // Respond to coordination 1
        coordinator.registerResponse(id1, '0xA', { v: 'a' });
        coordinator.registerResponse(id1, '0xB', { v: 'b' });

        // Respond to coordination 2
        coordinator.registerResponse(id2, '0xC', { price: 50 });
        coordinator.registerResponse(id2, '0xD', { price: 75 });

        const result1 = await coordinator.join(id1);
        const result2 = await coordinator.join(id2);

        assert.equal(result1.completedCount, 2);
        assert.equal(result1.taskType, 'custom');
        assert.equal(result2.completedCount, 2);
        assert.equal(result2.taskType, 'quote');

        // Verify no cross-contamination
        const responders1 = result1.responses.map((r) => r.responderAddress);
        assert.ok(responders1.includes('0xA'));
        assert.ok(responders1.includes('0xB'));
        assert.ok(!responders1.includes('0xC'));

        const responders2 = result2.responses.map((r) => r.responderAddress);
        assert.ok(responders2.includes('0xC'));
        assert.ok(responders2.includes('0xD'));
        assert.ok(!responders2.includes('0xA'));
      });
    });
  });

  // Additional coverage
  describe('status aggregation', () => {
    it('merges status responses into a unified object', async () => {
      await withCoordinator(async (coordinator) => {
        const targets = ['0xNode1', '0xNode2'];
        const id = coordinator.scatter({
          agentAddress: '0xReq',
          targets,
          taskType: 'status',
          payload: {},
          timeoutMs: 60000,
          joinStrategy: 'all',
        });

        coordinator.registerResponse(id, '0xNode1', { healthy: true, cpu: 42 });
        coordinator.registerResponse(id, '0xNode2', { healthy: false, cpu: 95 });

        const result = await coordinator.join(id);
        assert.equal(result.aggregation.type, 'merged_status');
        assert.deepEqual(result.aggregation.data['0xNode1'], { healthy: true, cpu: 42 });
        assert.deepEqual(result.aggregation.data['0xNode2'], { healthy: false, cpu: 95 });
      });
    });
  });

  describe('best strategy', () => {
    it('returns highest-scored response', async () => {
      await withCoordinator(async (coordinator) => {
        const targets = makeTargets(3);
        const id = coordinator.scatter({
          agentAddress: '0xReq',
          targets,
          taskType: 'custom',
          payload: {},
          timeoutMs: 60000,
          joinStrategy: 'best',
        });

        coordinator.registerResponse(id, '0xAgent1', { score: 70, answer: 'A' });
        coordinator.registerResponse(id, '0xAgent2', { score: 95, answer: 'B' });
        coordinator.registerResponse(id, '0xAgent3', { score: 80, answer: 'C' });

        const result = await coordinator.join(id);
        assert.equal(result.aggregation.type, 'best');
        assert.equal(result.aggregation.winner.response.score, 95);
        assert.equal(result.aggregation.winner.responderAddress, '0xAgent2');
      });
    });
  });

  describe('cleanup', () => {
    it('removes a coordination and clears its timer', async () => {
      await withCoordinator((coordinator) => {
        const id = coordinator.scatter({
          agentAddress: '0x1',
          targets: ['0xA'],
          taskType: 'custom',
          payload: {},
          timeoutMs: 60000,
          joinStrategy: 'all',
        });

        assert.ok(coordinator.getStatus(id));
        const removed = coordinator.cleanup(id);
        assert.equal(removed, true);
        assert.throws(() => coordinator.getStatus(id), /not found/i);
      });
    });

    it('returns false for nonexistent coordination', async () => {
      await withCoordinator((coordinator) => {
        assert.equal(coordinator.cleanup('fake'), false);
      });
    });
  });

  describe('join on already-completed coordination', () => {
    it('resolves immediately', async () => {
      await withCoordinator(async (coordinator) => {
        const id = coordinator.scatter({
          agentAddress: '0x1',
          targets: ['0xA'],
          taskType: 'custom',
          payload: {},
          timeoutMs: 60000,
          joinStrategy: 'all',
        });

        coordinator.registerResponse(id, '0xA', { done: true });

        // First join
        const r1 = await coordinator.join(id);
        assert.equal(r1.status, 'completed');

        // Second join — should also resolve immediately
        const r2 = await coordinator.join(id);
        assert.equal(r2.status, 'completed');
        assert.equal(r2.completedCount, 1);
      });
    });
  });
});

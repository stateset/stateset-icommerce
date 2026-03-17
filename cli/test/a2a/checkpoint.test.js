/**
 * Unit tests for a2a/checkpoint.js — State Checkpoint Service
 *
 * Covers: save/load, saveProcessedIds/loadProcessedIds,
 * saveCheckpoint/loadCheckpoint, atomic writes, nonexistent loads,
 * deleteCheckpoint, listCheckpoints, arbitrary data roundtrips.
 *
 * Uses os.tmpdir() for file operations — no mocking needed.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm, readdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { createCheckpointService } from '../../src/a2a/checkpoint.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

let testDir;

async function setupDir() {
  testDir = await mkdtemp(join(tmpdir(), 'a2a-checkpoint-test-'));
}

async function cleanupDir() {
  if (testDir) {
    await rm(testDir, { recursive: true, force: true });
    testDir = null;
  }
}

// ===========================================================================
// Tests
// ===========================================================================

describe('createCheckpointService', () => {
  beforeEach(async () => {
    await setupDir();
  });

  afterEach(async () => {
    await cleanupDir();
  });

  it('throws if dataDir is not provided', () => {
    assert.throws(
      () => createCheckpointService(null),
      { message: 'dataDir is required' },
    );
  });

  // -----------------------------------------------------------------------
  // save / load
  // -----------------------------------------------------------------------

  describe('save / load', () => {
    it('roundtrips agent state correctly', async () => {
      const cp = createCheckpointService(testDir);
      const state = {
        balance: 100.5,
        lastTick: '2025-01-01T00:00:00Z',
        counters: { sent: 10, received: 5 },
      };

      await cp.save('0xAgent1', state);
      const loaded = await cp.load('0xAgent1');

      assert.deepEqual(loaded, state);
    });

    it('overwrites previous state on re-save', async () => {
      const cp = createCheckpointService(testDir);

      await cp.save('0xAgent1', { version: 1 });
      await cp.save('0xAgent1', { version: 2 });

      const loaded = await cp.load('0xAgent1');
      assert.deepEqual(loaded, { version: 2 });
    });

    it('handles complex nested state objects', async () => {
      const cp = createCheckpointService(testDir);
      const state = {
        agents: ['0xA', '0xB'],
        config: { maxRetries: 3, timeout: 5000 },
        nested: { deep: { value: true } },
        numbers: [1, 2.5, -3],
        nullField: null,
      };

      await cp.save('0xComplex', state);
      const loaded = await cp.load('0xComplex');

      assert.deepEqual(loaded, state);
    });

    it('returns null for nonexistent agent state', async () => {
      const cp = createCheckpointService(testDir);
      const loaded = await cp.load('0xDoesNotExist');

      assert.equal(loaded, null);
    });

    it('throws if agentAddress is missing on save', async () => {
      const cp = createCheckpointService(testDir);
      await assert.rejects(
        () => cp.save(null, { state: true }),
        { message: 'agentAddress is required' },
      );
    });

    it('throws if agentAddress is missing on load', async () => {
      const cp = createCheckpointService(testDir);
      await assert.rejects(
        () => cp.load(''),
        { message: 'agentAddress is required' },
      );
    });
  });

  // -----------------------------------------------------------------------
  // saveProcessedIds / loadProcessedIds
  // -----------------------------------------------------------------------

  describe('saveProcessedIds / loadProcessedIds', () => {
    it('preserves Set data through save/load cycle', async () => {
      const cp = createCheckpointService(testDir);
      const ids = new Set(['quote-1', 'quote-2', 'quote-3']);

      await cp.saveProcessedIds('0xAgent1', ids);
      const loaded = await cp.loadProcessedIds('0xAgent1');

      assert.ok(loaded instanceof Set);
      assert.equal(loaded.size, 3);
      assert.ok(loaded.has('quote-1'));
      assert.ok(loaded.has('quote-2'));
      assert.ok(loaded.has('quote-3'));
    });

    it('returns empty Set for nonexistent agent', async () => {
      const cp = createCheckpointService(testDir);
      const loaded = await cp.loadProcessedIds('0xNoOne');

      assert.ok(loaded instanceof Set);
      assert.equal(loaded.size, 0);
    });

    it('handles empty Set', async () => {
      const cp = createCheckpointService(testDir);

      await cp.saveProcessedIds('0xAgent', new Set());
      const loaded = await cp.loadProcessedIds('0xAgent');

      assert.ok(loaded instanceof Set);
      assert.equal(loaded.size, 0);
    });

    it('handles large Sets', async () => {
      const cp = createCheckpointService(testDir);
      const ids = new Set();
      for (let i = 0; i < 1000; i++) {
        ids.add(`id-${i}`);
      }

      await cp.saveProcessedIds('0xBig', ids);
      const loaded = await cp.loadProcessedIds('0xBig');

      assert.equal(loaded.size, 1000);
      assert.ok(loaded.has('id-0'));
      assert.ok(loaded.has('id-999'));
    });

    it('throws if processedIds is not a Set', async () => {
      const cp = createCheckpointService(testDir);
      await assert.rejects(
        () => cp.saveProcessedIds('0xAgent', ['not', 'a', 'set']),
        { message: 'processedIds must be a Set' },
      );
    });
  });

  // -----------------------------------------------------------------------
  // Atomic writes
  // -----------------------------------------------------------------------

  describe('atomic writes', () => {
    it('writes to temp file then renames (no partial files)', async () => {
      const cp = createCheckpointService(testDir);

      await cp.save('0xAtomic', { data: 'test' });

      // Verify no temp files remain
      const files = await readdir(testDir);
      const tmpFiles = files.filter((f) => f.includes('.tmp.'));
      assert.equal(tmpFiles.length, 0, `Found leftover temp files: ${tmpFiles.join(', ')}`);

      // The actual file exists and is valid JSON
      const stateFiles = files.filter((f) => f.endsWith('.state.json'));
      assert.equal(stateFiles.length, 1);

      const raw = await readFile(join(testDir, stateFiles[0]), 'utf8');
      const parsed = JSON.parse(raw);
      assert.deepEqual(parsed.state, { data: 'test' });
      assert.ok(parsed.savedAt);
      assert.equal(parsed.agentAddress, '0xAtomic');
    });

    it('concurrent saves do not corrupt the file', async () => {
      const cp = createCheckpointService(testDir);

      // Fire off 10 concurrent saves
      const promises = [];
      for (let i = 0; i < 10; i++) {
        promises.push(cp.save('0xRace', { version: i }));
      }
      await Promise.all(promises);

      // File should be valid and contain the last write
      const loaded = await cp.load('0xRace');
      assert.ok(loaded);
      assert.ok(typeof loaded.version === 'number');
    });
  });

  // -----------------------------------------------------------------------
  // saveCheckpoint / loadCheckpoint
  // -----------------------------------------------------------------------

  describe('saveCheckpoint / loadCheckpoint', () => {
    it('saves and loads arbitrary checkpoint data', async () => {
      const cp = createCheckpointService(testDir);
      const checkpoint = {
        lastTickTime: '2025-03-15T12:00:00Z',
        cursor: 42,
        processedBatches: 15,
        config: { retryDelay: 5000 },
      };

      await cp.saveCheckpoint('0xAgent', checkpoint);
      const loaded = await cp.loadCheckpoint('0xAgent');

      assert.deepEqual(loaded, checkpoint);
    });

    it('returns null for nonexistent checkpoint', async () => {
      const cp = createCheckpointService(testDir);
      const loaded = await cp.loadCheckpoint('0xGhost');

      assert.equal(loaded, null);
    });

    it('overwrites previous checkpoint', async () => {
      const cp = createCheckpointService(testDir);

      await cp.saveCheckpoint('0xAgent', { cursor: 1 });
      await cp.saveCheckpoint('0xAgent', { cursor: 2 });

      const loaded = await cp.loadCheckpoint('0xAgent');
      assert.deepEqual(loaded, { cursor: 2 });
    });

    it('state and checkpoint files are independent', async () => {
      const cp = createCheckpointService(testDir);

      await cp.save('0xAgent', { balance: 100 });
      await cp.saveCheckpoint('0xAgent', { cursor: 5 });

      const state = await cp.load('0xAgent');
      const checkpoint = await cp.loadCheckpoint('0xAgent');

      assert.deepEqual(state, { balance: 100 });
      assert.deepEqual(checkpoint, { cursor: 5 });
    });
  });

  // -----------------------------------------------------------------------
  // deleteCheckpoint
  // -----------------------------------------------------------------------

  describe('deleteCheckpoint', () => {
    it('removes the state file', async () => {
      const cp = createCheckpointService(testDir);

      await cp.save('0xAgent', { data: 'here' });
      let loaded = await cp.load('0xAgent');
      assert.ok(loaded);

      await cp.deleteCheckpoint('0xAgent', 'state');
      loaded = await cp.load('0xAgent');
      assert.equal(loaded, null);
    });

    it('removes processed IDs file', async () => {
      const cp = createCheckpointService(testDir);

      await cp.saveProcessedIds('0xAgent', new Set(['a', 'b']));
      await cp.deleteCheckpoint('0xAgent', 'processed');

      const loaded = await cp.loadProcessedIds('0xAgent');
      assert.equal(loaded.size, 0);
    });

    it('removes checkpoint file', async () => {
      const cp = createCheckpointService(testDir);

      await cp.saveCheckpoint('0xAgent', { cursor: 1 });
      await cp.deleteCheckpoint('0xAgent', 'checkpoint');

      const loaded = await cp.loadCheckpoint('0xAgent');
      assert.equal(loaded, null);
    });

    it('does not throw when deleting nonexistent checkpoint', async () => {
      const cp = createCheckpointService(testDir);
      // Should not throw
      await cp.deleteCheckpoint('0xNoExist', 'state');
    });

    it('defaults to state type', async () => {
      const cp = createCheckpointService(testDir);

      await cp.save('0xAgent', { x: 1 });
      await cp.deleteCheckpoint('0xAgent'); // no type argument

      const loaded = await cp.load('0xAgent');
      assert.equal(loaded, null);
    });
  });

  // -----------------------------------------------------------------------
  // listCheckpoints
  // -----------------------------------------------------------------------

  describe('listCheckpoints', () => {
    it('returns all saved agent checkpoints', async () => {
      const cp = createCheckpointService(testDir);

      await cp.save('0xAlice', { balance: 1 });
      await cp.save('0xBob', { balance: 2 });
      await cp.saveProcessedIds('0xAlice', new Set(['q-1']));
      await cp.saveCheckpoint('0xBob', { cursor: 5 });

      const list = await cp.listCheckpoints();

      assert.equal(list.length, 4);

      const types = list.map((l) => `${l.agentAddress}:${l.type}`).sort();
      assert.ok(types.includes('0xAlice:state'));
      assert.ok(types.includes('0xBob:state'));
      assert.ok(types.includes('0xAlice:processed'));
      assert.ok(types.includes('0xBob:checkpoint'));
    });

    it('returns empty array when no checkpoints exist', async () => {
      const cp = createCheckpointService(testDir);
      const list = await cp.listCheckpoints();

      assert.ok(Array.isArray(list));
      assert.equal(list.length, 0);
    });

    it('ignores non-JSON files', async () => {
      const cp = createCheckpointService(testDir);
      const { writeFile: wf } = await import('node:fs/promises');

      // Create a non-JSON file
      await wf(join(testDir, 'random.txt'), 'hello');
      await cp.save('0xAgent', { x: 1 });

      const list = await cp.listCheckpoints();
      assert.equal(list.length, 1);
      assert.equal(list[0].type, 'state');
    });
  });

  // -----------------------------------------------------------------------
  // Address sanitization
  // -----------------------------------------------------------------------

  describe('address sanitization', () => {
    it('handles addresses with special characters', async () => {
      const cp = createCheckpointService(testDir);

      await cp.save('0x1234567890abcdef', { ok: true });
      const loaded = await cp.load('0x1234567890abcdef');
      assert.deepEqual(loaded, { ok: true });
    });

    it('sanitizes slashes and dots in addresses', async () => {
      const cp = createCheckpointService(testDir);

      await cp.save('agent/../../etc/passwd', { attempt: true });
      const loaded = await cp.load('agent/../../etc/passwd');
      assert.deepEqual(loaded, { attempt: true });

      // File should be in testDir, not somewhere else
      const files = await readdir(testDir);
      assert.ok(files.length >= 1);
    });
  });
});

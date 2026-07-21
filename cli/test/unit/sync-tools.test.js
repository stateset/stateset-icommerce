/**
 * Sync Tools Test Suite
 *
 * Tests for tool definitions, schemas, permissions, and "not configured" paths
 * in src/tools/sync.js.
 *
 * sync.js imports isSyncConfigured at module level. isSyncConfigured() checks
 * for .stateset/sync.json relative to process.cwd(). When the config exists,
 * handlers proceed to use commerce.db; when absent, they return early with a
 * "Sync not configured" message.
 *
 * To test the "not configured" path, we temporarily change cwd to a temp dir
 * that has no .stateset/sync.json.
 */

import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';
import os from 'os';
import fs from 'fs';
import path from 'path';

// We need to import syncTools, but the module also imports sync config/outbox/engine/client.
// These are just function imports and do not execute at import time.
import { syncTools } from '../../src/tools/sync.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function findTool(name) {
  const tool = syncTools.find((t) => t.name === name);
  assert.ok(tool, `Tool "${name}" not found in syncTools`);
  return tool;
}

function getSchema(name) {
  return z.object(findTool(name).inputSchema);
}

function expectFail(schema, data, msg) {
  const result = schema.safeParse(data);
  assert.ok(!result.success, msg || `Expected parse to fail for: ${JSON.stringify(data)}`);
}

function expectPass(schema, data, msg) {
  const result = schema.safeParse(data);
  assert.ok(
    result.success,
    msg ||
      `Expected parse to pass for: ${JSON.stringify(data)}, errors: ${JSON.stringify(result.error?.issues)}`,
  );
}

// ---------------------------------------------------------------------------
// All tool names
// ---------------------------------------------------------------------------

const ALL_TOOL_NAMES = [
  'sync_status',
  'sync_push',
  'sync_pull',
  'sync_outbox',
  'sync_pulled_events',
  'sync_decrypt_event',
  'sync_retry_failed',
  'sync_entity_history',
  'sync_full',
  'sync_conflicts',
  'sync_resolve',
  'sync_rebase',
  'sync_verify_receipt',
  'sync_verify_inclusion',
  'sync_inspect_commitment',
  'agent_key_generate',
  'agent_key_list',
  'agent_key_info',
  'agent_key_rotate',
  'agent_key_export',
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('Sync Tools — definitions', () => {
  it('exports exactly 20 tools', () => {
    assert.strictEqual(syncTools.length, 20);
  });

  for (const name of ALL_TOOL_NAMES) {
    it(`includes tool "${name}"`, () => {
      assert.ok(findTool(name));
    });
  }

  it('every tool has a handler function', () => {
    for (const tool of syncTools) {
      assert.strictEqual(
        typeof tool.handler,
        'function',
        `${tool.name} handler should be a function`,
      );
    }
  });
});

describe('Sync Tools — permissions', () => {
  const readTools = [
    'sync_status',
    'sync_outbox',
    'sync_pulled_events',
    'sync_decrypt_event',
    'sync_entity_history',
    'sync_conflicts',
  ];

  const writeTools = ['sync_pull', 'sync_push'];

  const adminTools = ['sync_retry_failed', 'sync_full', 'sync_resolve', 'sync_rebase'];

  for (const name of readTools) {
    it(`${name} has read permission`, () => {
      assert.strictEqual(findTool(name).permission, 'read');
    });
  }

  for (const name of writeTools) {
    it(`${name} has write permission`, () => {
      assert.strictEqual(findTool(name).permission, 'write');
    });
  }

  for (const name of adminTools) {
    it(`${name} has admin permission`, () => {
      assert.strictEqual(findTool(name).permission, 'admin');
    });
  }
});

describe('Sync Tools — sync_push schema', () => {
  it('batchSize is optional number', () => {
    expectPass(getSchema('sync_push'), {});
    expectPass(getSchema('sync_push'), { batchSize: 50 });
  });

  it('dryRun is optional boolean', () => {
    expectPass(getSchema('sync_push'), { dryRun: true });
    expectPass(getSchema('sync_push'), { dryRun: false });
  });

  it('rejects non-boolean dryRun', () => {
    expectFail(getSchema('sync_push'), { dryRun: 'yes' });
  });
});

describe('Sync Tools — sync_entity_history schema', () => {
  it('entityType has min 1', () => {
    expectFail(getSchema('sync_entity_history'), { entityType: '', entityId: 'id1' });
  });

  it('entityId has min 1', () => {
    expectFail(getSchema('sync_entity_history'), { entityType: 'order', entityId: '' });
  });

  it('requires both entityType and entityId', () => {
    expectFail(getSchema('sync_entity_history'), { entityType: 'order' });
    expectFail(getSchema('sync_entity_history'), { entityId: 'id1' });
  });

  it('accepts valid entity history params', () => {
    expectPass(getSchema('sync_entity_history'), { entityType: 'order', entityId: 'ord-123' });
  });

  it('accepts local history options', () => {
    expectPass(getSchema('sync_entity_history'), {
      entityType: 'order',
      entityId: 'ord-123',
      source: 'local',
      limit: 25,
      includePayloads: true,
      decryptPayloads: true,
      keyId: 7,
    });
  });

  it('rejects invalid local history option types', () => {
    expectFail(getSchema('sync_entity_history'), {
      entityType: 'order',
      entityId: 'ord-123',
      source: 'cache',
    });
    expectFail(getSchema('sync_entity_history'), {
      entityType: 'order',
      entityId: 'ord-123',
      includePayloads: 'yes',
    });
    expectFail(getSchema('sync_entity_history'), {
      entityType: 'order',
      entityId: 'ord-123',
      decryptPayloads: 'yes',
    });
  });
});

describe('Sync Tools — sync_resolve schema', () => {
  it('conflictId has min 1', () => {
    expectFail(getSchema('sync_resolve'), { conflictId: '' });
  });

  it('strategy is an enum', () => {
    expectPass(getSchema('sync_resolve'), { conflictId: 'c1', strategy: 'remote-wins' });
    expectPass(getSchema('sync_resolve'), { conflictId: 'c1', strategy: 'local-wins' });
    expectPass(getSchema('sync_resolve'), { conflictId: 'c1', strategy: 'merge' });
    expectFail(getSchema('sync_resolve'), { conflictId: 'c1', strategy: 'invalid' });
  });

  it('strategy is optional', () => {
    expectPass(getSchema('sync_resolve'), { conflictId: 'c1' });
  });
});

describe('Sync Tools — sync_rebase schema', () => {
  it('strategy is an enum with 3 values', () => {
    expectPass(getSchema('sync_rebase'), { strategy: 'remote-wins' });
    expectPass(getSchema('sync_rebase'), { strategy: 'local-wins' });
    expectPass(getSchema('sync_rebase'), { strategy: 'merge' });
    expectFail(getSchema('sync_rebase'), { strategy: 'discard' });
  });

  it('strategy is optional', () => {
    expectPass(getSchema('sync_rebase'), {});
  });
});

describe('Sync Tools — sync_outbox schema', () => {
  it('status is enum with 5 values', () => {
    const validStatuses = ['pending', 'synced', 'failed', 'rejected', 'all'];
    for (const status of validStatuses) {
      expectPass(getSchema('sync_outbox'), { status });
    }
    expectFail(getSchema('sync_outbox'), { status: 'unknown' });
  });

  it('status is optional', () => {
    expectPass(getSchema('sync_outbox'), {});
  });

  it('limit is optional number', () => {
    expectPass(getSchema('sync_outbox'), { limit: 50 });
  });
});

describe('Sync Tools — sync_decrypt_event schema', () => {
  it('accepts eventId lookup', () => {
    expectPass(getSchema('sync_decrypt_event'), { eventId: 'evt-123' });
  });

  it('accepts sequenceNumber lookup', () => {
    expectPass(getSchema('sync_decrypt_event'), { sequenceNumber: 42, source: 'pulled' });
  });

  it('source is constrained to known values', () => {
    expectPass(getSchema('sync_decrypt_event'), { eventId: 'evt-123', source: 'auto' });
    expectPass(getSchema('sync_decrypt_event'), { eventId: 'evt-123', source: 'outbox' });
    expectPass(getSchema('sync_decrypt_event'), { eventId: 'evt-123', source: 'pulled' });
    expectFail(getSchema('sync_decrypt_event'), { eventId: 'evt-123', source: 'remote' });
  });
});

describe('Sync Tools — sync_pulled_events schema', () => {
  it('accepts empty params', () => {
    expectPass(getSchema('sync_pulled_events'), {});
  });

  it('accepts limit and boolean payload flags', () => {
    expectPass(getSchema('sync_pulled_events'), { limit: 50 });
    expectPass(getSchema('sync_pulled_events'), { includePayloads: true });
    expectPass(getSchema('sync_pulled_events'), { decryptPayloads: true });
    expectPass(getSchema('sync_pulled_events'), { keyId: 7 });
  });

  it('rejects invalid payload flag types', () => {
    expectFail(getSchema('sync_pulled_events'), { includePayloads: 'yes' });
    expectFail(getSchema('sync_pulled_events'), { decryptPayloads: 'yes' });
  });
});

describe('Sync Tools — sync_pull schema', () => {
  it('accepts empty params', () => {
    expectPass(getSchema('sync_pull'), {});
  });

  it('fromSequence is optional number', () => {
    expectPass(getSchema('sync_pull'), { fromSequence: 42 });
  });

  it('limit is optional number', () => {
    expectPass(getSchema('sync_pull'), { limit: 500 });
  });

  it('accepts pulled-event response options', () => {
    expectPass(getSchema('sync_pull'), { includeEvents: true });
    expectPass(getSchema('sync_pull'), { includePayloads: true });
    expectPass(getSchema('sync_pull'), { decryptPayloads: true });
    expectPass(getSchema('sync_pull'), { keyId: 7 });
  });

  it('rejects invalid pulled-event response option types', () => {
    expectFail(getSchema('sync_pull'), { includeEvents: 'yes' });
    expectFail(getSchema('sync_pull'), { includePayloads: 'yes' });
    expectFail(getSchema('sync_pull'), { decryptPayloads: 'yes' });
  });
});

describe('Sync Tools — sync_full schema', () => {
  it('accepts empty params', () => {
    expectPass(getSchema('sync_full'), {});
  });

  it('pushBatchSize is optional number', () => {
    expectPass(getSchema('sync_full'), { pushBatchSize: 200 });
  });

  it('pullLimit is optional number', () => {
    expectPass(getSchema('sync_full'), { pullLimit: 2000 });
  });
});

describe('Sync Tools — "not configured" handler path', () => {
  // To test the "sync not configured" path, we temporarily change process.cwd()
  // to a temp directory that has no .stateset/sync.json.
  let originalCwd;
  let tmpDir;

  before(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'sync-tools-test-'));
    originalCwd = process.cwd();
    process.chdir(tmpDir);
  });

  after(() => {
    process.chdir(originalCwd);
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  // Only test sync-related tools for "not configured" path.
  // Agent key and VES receipt tools have their own test files.
  const SYNC_ONLY_TOOLS = [
    'sync_status',
    'sync_push',
    'sync_pull',
    'sync_outbox',
    'sync_pulled_events',
    'sync_decrypt_event',
    'sync_retry_failed',
    'sync_entity_history',
    'sync_full',
    'sync_conflicts',
    'sync_resolve',
    'sync_rebase',
    'sync_inspect_commitment',
  ];

  for (const name of SYNC_ONLY_TOOLS) {
    it(`${name} returns sync-not-configured response`, async () => {
      const tool = findTool(name);
      const mockCommerce = { db: {} };
      const result = await tool.handler({
        commerce: mockCommerce,
        params: {
          entityType: 'order',
          entityId: 'id1',
          conflictId: 'c1',
          strategy: 'remote-wins',
          batchSize: 10,
          dryRun: false,
        },
        allowApply: true,
      });

      assert.ok(result, `${name} should return a result`);
      const hasNotConfigured =
        result.configured === false ||
        (result.error && result.error.includes('Sync not configured')) ||
        (result.message && result.message.includes('Sync not configured'));
      assert.ok(
        hasNotConfigured,
        `${name} should indicate sync not configured, got: ${JSON.stringify(result)}`,
      );
    });
  }
});

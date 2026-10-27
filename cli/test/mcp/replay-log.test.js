// Unit tests for the agentic replay log (cli/src/mcp/replay-log.js) and where
// the MCP server persists it: beside a file-backed store, never into the
// working directory of a server whose store is in memory.

import { strict as assert } from 'node:assert';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { after, describe, it } from 'node:test';

import { DEFAULT_REPLAY_LOG_FILE, createReplayLog } from '../../src/mcp/replay-log.js';

const { createStatesetMcpServer } = await import('../../src/mcp-server.js');

const signAuditArtifact = () => ({ signature: 'sig' });
const event = (tool) => ({
  eventId: `evt-${tool}`,
  tool,
  status: 'ok',
  occurredAt: new Date().toISOString(),
  params: { sku: 'SKU-1' },
});

const dirs = [];
const tempDir = (prefix) => {
  const dir = mkdtempSync(path.join(tmpdir(), prefix));
  dirs.push(dir);
  return dir;
};
after(() => {
  for (const dir of dirs) rmSync(dir, { recursive: true, force: true });
});

describe('replay-log · persistence', () => {
  it('appends each event to the JSONL file at logPath', async () => {
    const logPath = path.join(tempDir('replay-log-'), 'calls.jsonl');
    const log = createReplayLog({ logPath, signAuditArtifact });
    await log.addEvent(event('list_products'));
    const lines = readFileSync(logPath, 'utf8').trim().split('\n');
    assert.equal(lines.length, 1);
    assert.equal(JSON.parse(lines[0]).tool, 'list_products');
  });

  it('keeps events in memory only when logPath is null', async () => {
    const log = createReplayLog({ logPath: null, signAuditArtifact });
    await log.addEvent(event('list_products'));
    await log.addEvent(event('get_order'));
    const listed = await log.listEvents({ limit: 10 });
    assert.deepEqual(listed.events.map((e) => e.tool).sort(), ['get_order', 'list_products']);
    assert.equal(listed.source.path, null);
  });

  it('rejects a logPath that is neither a path nor null', () => {
    assert.throws(() => createReplayLog({ logPath: '', signAuditArtifact }), TypeError);
    assert.throws(() => createReplayLog({ logPath: undefined, signAuditArtifact }), TypeError);
  });
});

describe('replay-log · MCP server placement', () => {
  const callFromCwd = async (cwd, options) => {
    const previous = process.cwd();
    process.chdir(cwd);
    try {
      const server = createStatesetMcpServer({ toolProfile: 'all', ...options });
      await server.executeTool('list_products', {});
      await server.dispose?.();
    } finally {
      process.chdir(previous);
    }
  };

  it('writes nothing into the working directory of an in-memory server', async () => {
    const cwd = tempDir('replay-cwd-');
    await callFromCwd(cwd, { dbPath: ':memory:' });
    assert.equal(existsSync(path.join(cwd, '.stateset', DEFAULT_REPLAY_LOG_FILE)), false);
  });

  it('persists the log beside a file-backed store', async () => {
    const cwd = tempDir('replay-cwd-');
    const storeDir = tempDir('replay-store-');
    await callFromCwd(cwd, { dbPath: path.join(storeDir, 'store.db') });
    assert.equal(existsSync(path.join(storeDir, '.stateset', DEFAULT_REPLAY_LOG_FILE)), true);
    assert.equal(existsSync(path.join(cwd, '.stateset', DEFAULT_REPLAY_LOG_FILE)), false);
  });
});

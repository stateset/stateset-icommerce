/**
 * Every MCP tool, called through the real server on a fresh store, either
 * works or refuses cleanly -- it never crashes.
 *
 * Unit tests call handlers with mocks, and a mock can invent any method or
 * accept any shape. That hid tools calling binding methods that never
 * existed, schemas that crash on validation, dates sent as `null`, money
 * sent as strings where the binding takes numbers, and services that were
 * never attached. This drives each tool through `executeTool` -- validation,
 * permissions, policy and the native binding included -- with the minimal
 * input its own schema requires (see scripts/mcp-tool-smoke.mjs).
 *
 * A refusal (not found, not configured, invalid) is fine: the input is
 * synthetic. A crash is a defect. Known defects live in
 * test/fixtures/mcp-tool-smoke-backlog.json, and the backlog only shrinks:
 * a new crash fails, and so does a listed tool that no longer crashes.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import { runSmoke } from '../../scripts/mcp-tool-smoke.mjs';
import { ALL_DOMAIN_TOOLS } from '../../src/tools/domain-registry.js';

const backlog = JSON.parse(
  readFileSync(new URL('../fixtures/mcp-tool-smoke-backlog.json', import.meta.url), 'utf8'),
);
const known = new Map(backlog.defects.map((entry) => [entry.tool, entry.reason]));

let rows = null;
let unavailable = null;
try {
  rows = await runSmoke({ timeoutMs: 30_000 });
} catch (error) {
  unavailable = error;
  console.warn(`[mcp-tool-smoke] could not run: ${error.message}`);
}

describe('every MCP tool works or refuses cleanly on a fresh store', { skip: !rows }, () => {
  it('exercises the whole catalog', () => {
    assert.equal(rows.length, ALL_DOMAIN_TOOLS.length, 'a tool was skipped');
  });

  it('introduces no new crash', () => {
    const fresh = rows
      .filter((r) => (r.kind === 'defect' || r.kind === 'timeout') && !known.has(r.tool))
      .map((r) => `${r.tool}: ${r.kind}: ${r.message}`);
    assert.deepEqual(fresh, [], 'these tools crash on a fresh store');
  });

  it('keeps the backlog honest: a fixed tool must leave it', () => {
    const crashing = new Set(
      rows.filter((r) => r.kind === 'defect' || r.kind === 'timeout').map((r) => r.tool),
    );
    const stale = [...known.keys()].filter((tool) => !crashing.has(tool));
    assert.deepEqual(stale, [], 'these no longer crash; remove them from the backlog');
  });
});

if (unavailable) {
  // Surface why the sweep did not run instead of passing silently.
  describe('mcp tool smoke prerequisites', () => {
    it('can build the server', { todo: unavailable.message }, () => {});
  });
}

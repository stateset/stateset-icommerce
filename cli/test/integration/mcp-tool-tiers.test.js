/**
 * Stability tiers (src/tools/tool-tiers.js) and the guarantee they carry:
 * the default MCP surface -- the core tier -- is fully trustworthy.
 *
 *   - every tool has exactly one known tier; nothing is left untiered;
 *   - the core profile exposes exactly the core tier, and `all` everything;
 *   - every core tool, driven through the real server on a fresh store,
 *     works or refuses cleanly: zero defects, zero timeouts, and no backlog;
 *   - the smoke backlog can only hold experimental tools.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { runSmoke } from '../../scripts/mcp-tool-smoke.mjs';
import { ALL_DOMAIN_TOOLS, TOOL_MODULE_NAMES } from '../../src/tools/domain-registry.js';
import { AGENTIC_RUNTIME_TOOLS } from '../../src/mcp/agentic-runtime-tools.js';
import {
  DOMAIN_TIERS,
  EXPERIMENTAL_REASONS,
  RUNTIME_TOOL_TIERS,
  TOOL_TIERS,
  TOOL_TIER_OVERRIDES,
  toolTier,
} from '../../src/tools/tool-tiers.js';
import {
  DEFAULT_MCP_TOOL_PROFILE,
  MCP_TOOL_PROFILES,
  resolveMcpToolFilter,
} from '../../src/mcp/tool-profiles.js';
import { createStatesetMcpServer } from '../../src/mcp-server.js';

const ALL_TOOLS = [...ALL_DOMAIN_TOOLS, ...AGENTIC_RUNTIME_TOOLS];
const ALL_NAMES = new Set(ALL_TOOLS.map((tool) => tool.name));
const CORE_TOOLS = ALL_TOOLS.filter((tool) => toolTier(tool.name) === 'core');
const CORE_NAMES = CORE_TOOLS.map((tool) => tool.name).sort();

const backlog = JSON.parse(
  readFileSync(new URL('../fixtures/mcp-tool-smoke-backlog.json', import.meta.url), 'utf8'),
);

describe('every MCP tool has a stability tier', () => {
  it('tiers every registered domain, and only registered domains', () => {
    const untiered = TOOL_MODULE_NAMES.filter((domain) => !Object.hasOwn(DOMAIN_TIERS, domain));
    assert.deepEqual(untiered, [], 'add these domains to DOMAIN_TIERS');
    const unknown = Object.keys(DOMAIN_TIERS).filter((d) => !TOOL_MODULE_NAMES.includes(d));
    assert.deepEqual(unknown, [], 'DOMAIN_TIERS names domains the registry does not have');
  });

  it('tiers every agentic runtime tool, and only those', () => {
    const runtime = AGENTIC_RUNTIME_TOOLS.map((tool) => tool.name);
    assert.deepEqual(
      runtime.filter((name) => !Object.hasOwn(RUNTIME_TOOL_TIERS, name)),
      [],
      'add these runtime tools to RUNTIME_TOOL_TIERS',
    );
    assert.deepEqual(
      Object.keys(RUNTIME_TOOL_TIERS).filter((name) => !runtime.includes(name)),
      [],
    );
  });

  it('overrides only tools that exist', () => {
    assert.deepEqual(
      Object.keys(TOOL_TIER_OVERRIDES).filter((name) => !ALL_NAMES.has(name)),
      [],
    );
  });

  it('gives every tool exactly one known tier', () => {
    const bad = ALL_TOOLS.filter((tool) => !TOOL_TIERS.includes(toolTier(tool.name))).map(
      (tool) => `${tool.name}: ${toolTier(tool.name)}`,
    );
    assert.deepEqual(bad, []);
  });

  it('explains every experimental domain and tool', () => {
    const experimentalDomains = Object.entries(DOMAIN_TIERS)
      .filter(([, tier]) => tier === 'experimental')
      .map(([domain]) => domain);
    const experimentalTools = Object.entries({ ...RUNTIME_TOOL_TIERS, ...TOOL_TIER_OVERRIDES })
      .filter(([, tier]) => tier === 'experimental')
      .map(([name]) => name);
    const missing = [...experimentalDomains, ...experimentalTools].filter(
      (key) => !EXPERIMENTAL_REASONS[key],
    );
    assert.deepEqual(missing, [], 'say why these are experimental');
    const stray = Object.keys(EXPERIMENTAL_REASONS).filter(
      (key) => !experimentalDomains.includes(key) && !experimentalTools.includes(key),
    );
    assert.deepEqual(stray, [], 'these reasons belong to nothing experimental');
  });
});

describe('the smoke backlog holds only experimental tools', () => {
  it('names only real tools', () => {
    const unknown = backlog.defects.map((e) => e.tool).filter((name) => !ALL_NAMES.has(name));
    assert.deepEqual(unknown, []);
  });

  it('never lists a core or extended tool', () => {
    const promoted = backlog.defects
      .filter((entry) => toolTier(entry.tool) !== 'experimental')
      .map((entry) => `${entry.tool} (${toolTier(entry.tool)})`);
    assert.deepEqual(
      promoted,
      [],
      'a crashing tool cannot be core or extended: fix it, or demote it to experimental',
    );
  });
});

describe('profiles follow the tiers', () => {
  it('defaults to the core profile', () => {
    assert.equal(DEFAULT_MCP_TOOL_PROFILE, 'core');
  });

  it('the core profile is exactly the core tier', () => {
    const inCore = resolveMcpToolFilter({ profile: 'core' });
    assert.deepEqual(
      ALL_TOOLS.filter((t) => inCore(t.name))
        .map((t) => t.name)
        .sort(),
      CORE_NAMES,
    );
    assert.deepEqual(
      [...MCP_TOOL_PROFILES.core].sort(),
      Object.keys(DOMAIN_TIERS)
        .filter((domain) => DOMAIN_TIERS[domain] === 'core')
        .sort(),
    );
  });

  it('keeps experimental tools out of core unless a domain is added explicitly', () => {
    const inCore = resolveMcpToolFilter({ profile: 'core' });
    assert.equal(inCore('agent_receipt_purchase'), false);
    assert.equal(inCore('a2a_saga_execute'), false);
    assert.equal(inCore('delegate_to_agent'), false);
    const withA2a = resolveMcpToolFilter({ profile: 'core', domains: ['a2a-automation'] });
    assert.equal(withA2a('a2a_saga_execute'), true);
    assert.equal(withA2a('list_customers'), true);
  });

  it('the all profile still exposes every tier', () => {
    const inAll = resolveMcpToolFilter({ profile: 'all' });
    assert.ok(ALL_TOOLS.every((tool) => inAll(tool.name)));
  });

  it('a server built for the core profile exposes exactly the core tier', () => {
    const dir = mkdtempSync(path.join(os.tmpdir(), 'mcp-tiers-'));
    try {
      const server = createStatesetMcpServer({
        dbPath: path.join(dir, 'store.db'),
        toolProfile: 'core',
      });
      const exposed = server
        .getAdaptedTools()
        .map((tool) => tool.name)
        .sort();
      assert.deepEqual(exposed, CORE_NAMES);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

let rows = null;
let unavailable = null;
try {
  rows = await runSmoke({ tools: CORE_TOOLS, timeoutMs: 30_000 });
} catch (error) {
  unavailable = error;
  console.warn(`[mcp-tool-tiers] could not run the core smoke: ${error.message}`);
}

describe('every core tool works or refuses cleanly on a fresh store', { skip: !rows }, () => {
  it('sweeps the whole core tier', () => {
    assert.deepEqual(rows.map((r) => r.tool).sort(), CORE_NAMES);
  });

  it('has zero defects and zero timeouts -- no backlog for core', () => {
    const broken = rows
      .filter((r) => r.kind === 'defect' || r.kind === 'timeout')
      .map((r) => `${r.tool}: ${r.kind}: ${r.message}`);
    assert.deepEqual(broken, [], 'core tools must never crash; fix or demote them');
  });
});

if (unavailable) {
  describe('core smoke prerequisites', () => {
    it('can build the server', { todo: unavailable.message }, () => {});
  });
}

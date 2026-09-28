/**
 * Every MCP tool reads and writes the store its server was started with.
 *
 * The checkout, circuit-breaker, compliance and catalog tools used to open
 * `new A2AStore()` -- `~/.stateset/a2a.db` -- and the audit tools
 * `~/.stateset/audit.db`, whatever `--db` said. Two servers on one machine
 * then shared payment links, GDPR data, breaker events and catalog entries (a
 * cross-store leak), and results depended on the home directory of whoever
 * ran them. This starts two servers on two temp databases with an isolated
 * HOME, writes through each of those tools on server A, and checks that
 * server B sees none of it and that nothing was written under HOME.
 */

import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

// HOME must be isolated before any module computes a path from it.
const dir = mkdtempSync(path.join(os.tmpdir(), 'mcp-db-scope-'));
const home = path.join(dir, 'home');
mkdirSync(home);
const savedHome = { HOME: process.env.HOME, USERPROFILE: process.env.USERPROFILE };
process.env.HOME = home;
process.env.USERPROFILE = home;

const { createStatesetMcpServer } = await import('../../src/mcp-server.js');

const dbA = path.join(dir, 'a', 'store.db');
const dbB = path.join(dir, 'b', 'store.db');
mkdirSync(path.dirname(dbA));
mkdirSync(path.dirname(dbB));

let serverA;
let serverB;

/** Call a tool and return the tool's own result, failing on any refusal. */
async function call(server, name, params = {}) {
  const response = await server.executeTool(name, params);
  const result = response?.result;
  assert.ok(
    response?.success === true && result?.success !== false,
    `${name} failed: ${JSON.stringify(response?.error ?? result).slice(0, 400)}`,
  );
  return result;
}

function filesUnder(root) {
  if (!existsSync(root)) return [];
  return readdirSync(root, { withFileTypes: true, recursive: true })
    .filter((entry) => entry.isFile())
    .map((entry) => path.join(entry.parentPath ?? entry.path, entry.name));
}

before(() => {
  serverA = createStatesetMcpServer({ dbPath: dbA, allowApply: true, toolProfile: 'all' });
  serverB = createStatesetMcpServer({ dbPath: dbB, allowApply: true, toolProfile: 'all' });
});

after(() => {
  for (const [key, value] of Object.entries(savedHome)) {
    if (value === undefined) delete process.env[key];
    else process.env[key] = value;
  }
  rmSync(dir, { recursive: true, force: true });
});

describe('MCP tools are scoped to the server store', () => {
  it('checkout: a payment link made on A is not listed on B', async () => {
    const link = await call(serverA, 'create_payment_link', {
      items: [{ name: 'Widget', quantity: 1, unitPrice: 10 }],
      currency: 'USD',
    });
    assert.ok(link.linkId, 'the link has an id');
    const onA = await call(serverA, 'list_payment_links', {});
    const onB = await call(serverB, 'list_payment_links', {});
    assert.deepEqual(
      onA.links.map((l) => l.id),
      [link.linkId],
    );
    assert.equal(onB.count, 0, 'server B sees server A payment link');
  });

  it('catalog: a product published on A is not in B catalog', async () => {
    await call(serverA, 'publish_product_catalog', {
      productId: 'prod-scope-1',
      name: 'Scoped product',
      capabilities: ['buy'],
    });
    const onA = await call(serverA, 'export_agent_catalog', {});
    const onB = await call(serverB, 'export_agent_catalog', {});
    assert.match(JSON.stringify(onA), /prod-scope-1/);
    assert.doesNotMatch(JSON.stringify(onB), /prod-scope-1/, 'server B sees server A catalog');
  });

  it('circuit-breaker: a breaker tripped on A stays closed on B', async () => {
    await call(serverA, 'agent_trip_breaker', { agentName: 'scope-agent', reason: 'test' });
    const onA = await call(serverA, 'agent_get_breaker_state', { agentName: 'scope-agent' });
    const onB = await call(serverB, 'agent_get_breaker_state', { agentName: 'scope-agent' });
    assert.match(JSON.stringify(onA), /"open"/);
    assert.doesNotMatch(JSON.stringify(onB), /"open"/, 'server B sees server A breaker trip');
  });

  it('compliance: A audit trail holds the trip; B does not', async () => {
    const onA = await call(serverA, 'export_audit_trail', {});
    const onB = await call(serverB, 'export_audit_trail', {});
    assert.match(JSON.stringify(onA), /scope-agent/);
    assert.doesNotMatch(JSON.stringify(onB), /scope-agent/, 'server B sees server A audit trail');
  });

  it('compliance: GDPR export/erasure and SOC2 evidence work on a fresh store', async () => {
    const exported = await call(serverB, 'export_gdpr_data', { customerId: '0xnobody' });
    assert.deepEqual(exported.personalData ?? exported.export?.personalData ?? [], []);
    await call(serverB, 'delete_gdpr_data', { customerId: '0xnobody' });
    const soc2 = await call(serverB, 'soc2_evidence', {
      controls: ['access_control', 'change_management', 'incident_response'],
    });
    assert.match(JSON.stringify(soc2), /access_control/);
  });

  it('audit: the audit log is a sibling of each store, not ~/.stateset/audit.db', async () => {
    await call(serverA, 'audit_query', {});
    await call(serverB, 'audit_summary', {});
    assert.ok(existsSync(path.join(dir, 'a', 'store.audit.db')), 'A audit log next to A store');
    assert.ok(existsSync(path.join(dir, 'b', 'store.audit.db')), 'B audit log next to B store');
  });

  it('writes nothing under HOME', () => {
    assert.deepEqual(filesUnder(home), [], 'a tool wrote under the home directory');
  });
});

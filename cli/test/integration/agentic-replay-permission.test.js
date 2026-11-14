/**
 * `agentic_replay_mutation` re-executes a logged tool call. The tool is
 * declared `write`, so it may re-execute `write` tools only: replaying an
 * `admin` or `delete` tool would let a caller that holds write permission run
 * admin and delete tools it was never granted. An embedded toolkit scoped to
 * `permission:write` is that caller.
 *
 * Each case logs the privileged call from an operator in preview mode, so
 * nothing has happened yet, then has a write-scoped toolkit in apply mode try
 * to replay it for real.
 */

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { Commerce } from '../../../bindings/node/index.js';
import { createEmbeddedAgentToolkit } from '../../src/agent-toolkit.js';
import { createStatesetMcpServer } from '../../src/mcp-server.js';
import { REPLAY_PERMISSION_DENIED } from '../../src/mcp/mutation-simulator.js';

const kernelExample = (name) =>
  JSON.parse(readFileSync(new URL(`../../../kernel/examples/${name}`, import.meta.url), 'utf8'));

function fixture(t) {
  const directory = mkdtempSync(join(tmpdir(), 'stateset-replay-permission-'));
  const dbPath = join(directory, 'store.db');
  const commerce = new Commerce(dbPath);
  const options = { commerce, dbPath, toolProfile: 'all' };
  // Preview mode: every call is logged to the replay log, none executes.
  const operator = createEmbeddedAgentToolkit({ ...options, capabilities: ['*'] });
  const writer = createEmbeddedAgentToolkit({
    ...options,
    allowApply: true,
    capabilities: ['permission:write'],
  });
  t.after(async () => {
    await operator.close?.();
    await writer.close?.();
    commerce.close();
    rmSync(directory, { recursive: true, force: true });
  });
  return { directory, dbPath, commerce, operator, writer };
}

// A scoped toolkit's replay can be refused by either of two guards: the
// replay's own write-only rule (REPLAY_PERMISSION_DENIED), or, where the
// toolkit's capability scope is also bound to nested tools, the scope itself
// (PERMISSION_DENIED: the logged tool is not exposed to that toolkit at all).
// Either way nothing runs. The unscoped-server case below pins the replay
// rule's own code, since no scope applies there.
const REFUSED_BY_REPLAY_RULE = [REPLAY_PERMISSION_DENIED];
const REFUSED_BY_REPLAY_RULE_OR_SCOPE = [REPLAY_PERMISSION_DENIED, 'PERMISSION_DENIED'];

function assertRefused(result, tool, codes = REFUSED_BY_REPLAY_RULE_OR_SCOPE) {
  assert.equal(result.ok, false, JSON.stringify(result));
  assert.ok(codes.includes(result.failure?.code), JSON.stringify(result.failure));
  assert.match(result.error, new RegExp(`'${tool}'`));
  assert.equal(result.result?.replay, undefined, 'nothing was replayed');
}

test('a write-scoped caller cannot replay an admin tool: no export file is written', async (t) => {
  const { directory, operator, writer } = fixture(t);
  const exportPath = join(directory, 'export.json');

  const logged = await operator.executeTool('export_full_data', { exportPath });
  assert.equal(logged.preview, true, 'the operator call only previews');
  assert.equal(existsSync(exportPath), false);

  // Out of scope when called directly ...
  await assert.rejects(
    writer.executeTool('export_full_data', { exportPath }),
    /outside this toolkit's capability scope/,
  );
  // ... and through a replay, real or dry.
  for (const dryRun of [false, true]) {
    assertRefused(
      await writer.executeTool('agentic_replay_mutation', { tool: 'export_full_data', dryRun }),
      'export_full_data',
    );
  }
  // With no filter the newest logged mutation (the export) is still refused,
  // rather than silently replaying an older write instead.
  assertRefused(
    await writer.executeTool('agentic_replay_mutation', { dryRun: false }),
    'export_full_data',
  );
  assert.equal(existsSync(exportPath), false, 'the replay wrote no export file');
});

test('a write-scoped caller cannot replay a delete tool: the cart survives', async (t) => {
  const { commerce, operator, writer } = fixture(t);
  const cart = await commerce.carts.create({ customerEmail: 'replay@example.com' });

  const logged = await operator.executeTool('delete_cart', { cartId: cart.id });
  assert.equal(logged.preview, true, 'the operator call only previews');

  assertRefused(
    await writer.executeTool('agentic_replay_mutation', { tool: 'delete_cart', dryRun: false }),
    'delete_cart',
  );
  assert.equal((await commerce.carts.get(cart.id))?.id, cart.id, 'the cart was not deleted');
});

test('an unscoped apply server still replays only write tools', async (t) => {
  const { directory, dbPath, commerce, operator } = fixture(t);
  const exportPath = join(directory, 'export.json');
  const cart = await commerce.carts.create({ customerEmail: 'replay@example.com' });
  await operator.executeTool('export_full_data', { exportPath });
  await operator.executeTool('delete_cart', { cartId: cart.id });

  const server = createStatesetMcpServer({
    commerce,
    dbPath,
    allowApply: true,
    toolProfile: 'all',
  });
  t.after(async () => {
    server.dispose?.();
    await server.close?.().catch(() => {});
  });
  for (const tool of ['export_full_data', 'delete_cart']) {
    assertRefused(
      await server.executeTool('agentic_replay_mutation', { tool, dryRun: false }),
      tool,
      REFUSED_BY_REPLAY_RULE,
    );
  }
  assert.equal(existsSync(exportPath), false, 'the replay wrote no export file');
  assert.equal((await commerce.carts.get(cart.id))?.id, cart.id, 'the cart was not deleted');
});

test('a write tool still replays for a write-scoped caller', async (t) => {
  const { commerce, operator, writer } = fixture(t);
  const before = (await commerce.carts.list()).length;

  await operator.executeTool('create_cart', { customerEmail: 'replay@example.com' });
  assert.equal((await commerce.carts.list()).length, before, 'the operator call only previews');

  const replayed = await writer.executeTool('agentic_replay_mutation', {
    tool: 'create_cart',
    dryRun: false,
  });
  assert.equal(replayed.ok, true, JSON.stringify(replayed));
  assert.equal(replayed.result?.sourceEvent?.tool, 'create_cart');
  assert.equal((await commerce.carts.list()).length, before + 1, 'the replay created the cart');
});

test('strict kernel mode exposes the replay as a governed composite, and it cannot replay an ungoverned write', async (t) => {
  const { dbPath, commerce, operator } = fixture(t);
  const countLinks = async () =>
    (await operator.executeTool('list_payment_links', {})).result.count;
  // create_payment_link has no kernel command, so strict mode does not expose it.
  await operator.executeTool('create_payment_link', {
    items: [{ name: 'Replay item', quantity: 1, unitPrice: 5 }],
  });
  const before = await countLinks();

  const strict = createStatesetMcpServer({
    commerce,
    dbPath,
    allowApply: true,
    toolProfile: 'all',
    kernel: {
      strict: true,
      policy: kernelExample('strict-policy.json'),
      principal: kernelExample('strict-principal.json'),
      storeId: 'store:replace-me',
    },
  });
  t.after(async () => {
    strict.dispose?.();
    await strict.close?.().catch(() => {});
  });
  const exposed = new Set(strict.getRawToolDefinitions().map((tool) => tool.name));
  assert.equal(exposed.has('agentic_replay_mutation'), true);
  assert.equal(exposed.has('create_payment_link'), false);

  const replayed = await strict.executeTool('agentic_replay_mutation', {
    tool: 'create_payment_link',
    dryRun: false,
  });
  assert.equal(replayed.ok, false, JSON.stringify(replayed));
  assert.equal(await countLinks(), before, 'no payment link was created');
});

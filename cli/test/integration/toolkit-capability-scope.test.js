/**
 * A toolkit's capability scope binds every path that can reach a tool.
 *
 * `createEmbeddedAgentToolkit({ capabilities })` used to enforce its scope at
 * the toolkit boundary only: the listing was filtered and a direct call to an
 * out-of-scope tool was refused, but the composite tools resolved nested tools
 * against the server's whole profile. A toolkit allowed to run
 * `agentic_execute_plan` (or `agentic_replay_mutation`, or with any
 * `permission:write` grant) could therefore run `delete_customer` as a plan
 * step, while calling it directly was refused. These tests run against a real
 * SQLite store and check the row the out-of-scope tool would have changed.
 */

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Commerce } from '../../../bindings/node/index.js';
import { createEmbeddedAgentToolkit } from '../../src/agent-toolkit.js';
import { createStatesetMcpServer, getStaticMcpToolDefinitions } from '../../src/mcp-server.js';

const OUT_OF_SCOPE = /outside this toolkit's capability scope/;

async function fixture(t) {
  const directory = mkdtempSync(join(tmpdir(), 'stateset-toolkit-scope-'));
  const dbPath = join(directory, 'store.db');
  const policyStorePath = join(directory, '.stateset');
  const commerce = new Commerce(dbPath);
  const servers = [];
  t.after(async () => {
    for (const server of servers) {
      await server.close().catch(() => {});
    }
    commerce.close();
    rmSync(directory, { recursive: true, force: true });
  });
  const customer = await commerce.customers.create({
    email: 'scope@example.com',
    firstName: 'Ada',
    lastName: 'L',
  });
  const toolkit = (options = {}) => {
    const created = createEmbeddedAgentToolkit({ commerce, dbPath, policyStorePath, ...options });
    servers.push(created.server);
    return created;
  };
  const server = (options = {}) => {
    const created = createStatesetMcpServer({ commerce, dbPath, policyStorePath, ...options });
    servers.push(created);
    return created;
  };
  const customerStatus = async () => (await commerce.customers.get(customer.id))?.status;
  return { commerce, customer, directory, toolkit, server, customerStatus };
}

const deleteStep = (customer) => ({ tool: 'delete_customer', params: { customerId: customer.id } });

test('a plan step cannot run a tool outside the toolkit capability scope', async (t) => {
  const { customer, toolkit, customerStatus } = await fixture(t);
  const scoped = toolkit({ allowApply: true, capabilities: ['read:*', 'agentic_execute_plan'] });

  await assert.rejects(
    scoped.executeTool('delete_customer', { customerId: customer.id }),
    OUT_OF_SCOPE,
  );

  const viaTool = await scoped.executeTool('agentic_execute_plan', {
    dryRun: false,
    steps: [deleteStep(customer)],
  });
  assert.equal(viaTool.status, 'success', JSON.stringify(viaTool.error));
  const [step] = viaTool.result.steps;
  assert.equal(step.tool, 'delete_customer');
  assert.equal(step.status, 'permission_block');
  assert.match(step.error, OUT_OF_SCOPE);
  assert.equal(step.permission.allowed, false);
  assert.equal(viaTool.result.finalStatus, 'failed');
  assert.equal(await customerStatus(), 'active');

  // The host-side plan helper resolves tools through the same scoped server.
  const viaHelper = await scoped.executePlan({ dryRun: false, steps: [deleteStep(customer)] });
  assert.equal(viaHelper.steps[0].status, 'permission_block');
  assert.match(viaHelper.steps[0].error, OUT_OF_SCOPE);
  assert.equal(await customerStatus(), 'active');
});

test('a permission grant does not widen the scope of nested plan steps', async (t) => {
  const { directory, toolkit } = await fixture(t);
  // `permission:write` admits agentic_execute_plan (a write tool) but not
  // export_full_data (not a write tool), which writes the whole store to disk.
  const scoped = toolkit({ allowApply: true, capabilities: ['permission:write'] });
  assert.ok(scoped.getRawTool('agentic_execute_plan'));
  assert.equal(scoped.getRawTool('export_full_data'), undefined);

  const exportPath = join(directory, 'export.json');
  await assert.rejects(scoped.executeTool('export_full_data', { exportPath }), OUT_OF_SCOPE);

  const result = await scoped.executeTool('agentic_execute_plan', {
    dryRun: false,
    steps: [{ tool: 'export_full_data', params: { exportPath } }],
  });
  assert.equal(result.result.steps[0].status, 'permission_block');
  assert.match(result.result.steps[0].error, OUT_OF_SCOPE);
  assert.equal(existsSync(exportPath), false);
});

test('dry runs and simulations report out-of-scope steps instead of passing them', async (t) => {
  const { customer, toolkit, customerStatus } = await fixture(t);
  const scoped = toolkit({ allowApply: true, capabilities: ['read:*', 'agentic_execute_plan'] });

  const dryRun = await scoped.executeTool('agentic_execute_plan', {
    dryRun: true,
    steps: [deleteStep(customer)],
  });
  assert.equal(dryRun.result.steps[0].status, 'permission_block');
  assert.match(dryRun.result.steps[0].error, OUT_OF_SCOPE);
  assert.equal(dryRun.result.finalStatus, 'failed');

  // agentic_plan is a read tool, so `read:*` admits it.
  const plan = await scoped.executeTool('agentic_plan', { steps: [deleteStep(customer)] });
  assert.equal(plan.status, 'success', JSON.stringify(plan.error));
  assert.equal(plan.result.executable, false);
  assert.equal(plan.result.outcomes[0].status, 'permission_block');
  assert.match(plan.result.outcomes[0].error, OUT_OF_SCOPE);
  assert.match(plan.result.outcomes[0].permission.reason, OUT_OF_SCOPE);

  const simulated = await scoped.executeTool('agentic_simulate_mutation', {
    tool: 'delete_customer',
    params: { customerId: customer.id },
  });
  assert.equal(simulated.result.success, false);
  assert.equal(simulated.failure?.code, 'PERMISSION_DENIED', JSON.stringify(simulated.failure));
  assert.match(simulated.result.error, OUT_OF_SCOPE);

  const helperSimulation = await scoped.simulatePlan({ steps: [deleteStep(customer)] });
  assert.equal(helperSimulation.outcomes[0].status, 'permission_block');
  assert.equal(await customerStatus(), 'active');
});

test('replay refuses a recorded mutation whose tool is outside the scope', async (t) => {
  const { customer, toolkit, customerStatus } = await fixture(t);
  // An unscoped preview toolkit on the same store records the call (as a
  // preview: nothing changes) in the shared replay log.
  const recorder = toolkit({ allowApply: false });
  const recorded = await recorder.executeTool('delete_customer', { customerId: customer.id });
  assert.equal(recorded.status, 'preview');
  assert.equal(await customerStatus(), 'active');

  // Grant the replay tool by name so the test holds whatever its permission.
  const scoped = toolkit({ allowApply: true, capabilities: ['read:*', 'agentic_replay_mutation'] });
  const replay = await scoped.executeTool('agentic_replay_mutation', {
    tool: 'delete_customer',
    dryRun: false,
  });
  // The replay tool ran and reported its own refusal: nothing was replayed.
  assert.equal(replay.success, false);
  assert.equal(replay.failure?.code, 'PERMISSION_DENIED', JSON.stringify(replay.failure));
  assert.equal(replay.result.success, false);
  assert.match(replay.result.error, OUT_OF_SCOPE);
  assert.equal(replay.result.sourceEvent.tool, 'delete_customer');
  assert.equal(replay.result.sourceEvent.requestId, recorded.requestId);
  assert.equal(await customerStatus(), 'active');

  const viaHelper = await scoped.replayMutation({ tool: 'delete_customer', dryRun: false });
  assert.equal(viaHelper.success, false);
  assert.match(viaHelper.error, OUT_OF_SCOPE);
  assert.equal(await customerStatus(), 'active');
});

test('rollback never runs a compensation tool outside the scope and says so', async (t) => {
  const { commerce, customer, toolkit, customerStatus } = await fixture(t);
  const plan = {
    dryRun: false,
    rollbackOnFailure: true,
    steps: [
      { tool: 'create_cart', params: { customerEmail: 'cart@example.com' } },
      deleteStep(customer),
    ],
  };
  const cartOf = async (result) => {
    const cartId = result.steps[0].result?.cart?.id;
    assert.ok(cartId, JSON.stringify(result.steps[0]));
    return commerce.carts.get(cartId);
  };

  // cancel_cart (the compensation for create_cart) is outside this scope.
  const scoped = toolkit({ allowApply: true, capabilities: ['read:*', 'create_cart'] });
  const refused = await scoped.executePlan(plan);
  assert.equal(refused.steps[0].status, 'success');
  assert.equal(refused.steps[1].status, 'permission_block');
  assert.equal(refused.finalStatus, 'failed');
  assert.equal(refused.rollback.attempted, 1);
  assert.equal(refused.rollback.fullyReverted, false);
  const [compensation] = refused.rollback.steps;
  assert.equal(compensation.status, 'rollback_failed');
  assert.equal(compensation.tool, 'cancel_cart');
  assert.match(compensation.error, OUT_OF_SCOPE);
  assert.deepEqual(compensation.compensationTools, []);
  assert.deepEqual(
    compensation.refusedCompensationTools.map((entry) => entry.tool),
    ['cancel_cart'],
  );
  assert.notEqual((await cartOf(refused)).status, 'cancelled');

  // With cancel_cart in scope the same plan compensates, so the refusal above
  // is the scope at work and not a harness that never reaches rollback.
  const compensating = toolkit({
    allowApply: true,
    capabilities: ['read:*', 'create_cart', 'cancel_cart'],
  });
  const reverted = await compensating.executePlan(plan);
  assert.equal(reverted.rollback.steps[0].tool, 'cancel_cart');
  assert.equal(reverted.rollback.steps[0].status, 'rollback_success');
  assert.equal(reverted.rollback.fullyReverted, true);
  assert.equal((await cartOf(reverted)).status, 'cancelled');
  assert.equal(await customerStatus(), 'active');
});

test('a toolkit without capabilities keeps exposing every tool', async (t) => {
  const { customer, toolkit } = await fixture(t);
  const unscoped = toolkit();
  assert.equal(unscoped.getRawTools().length, getStaticMcpToolDefinitions().length);
  assert.ok(unscoped.getRawTool('delete_customer'));

  // Preview mode still applies, but the step is not refused for scope.
  const result = await unscoped.executePlan({ dryRun: true, steps: [deleteStep(customer)] });
  assert.equal(result.steps[0].status, 'dry_run_blocked');
  assert.doesNotMatch(String(result.steps[0].error), OUT_OF_SCOPE);
});

test('the server tool profile binds nested plan steps', async (t) => {
  const { customer, server, customerStatus } = await fixture(t);
  // `finance` exposes the agentic runtime tools but not the customers domain.
  const finance = server({ allowApply: true, toolProfile: 'finance' });
  const names = finance.getRawToolDefinitions().map((tool) => tool.name);
  assert.ok(names.includes('agentic_execute_plan'));
  assert.ok(!names.includes('delete_customer'));

  const viaTool = await finance.executeTool('agentic_execute_plan', {
    dryRun: false,
    steps: [deleteStep(customer)],
  });
  assert.equal(viaTool.result.steps[0].status, 'invalid');
  assert.match(viaTool.result.steps[0].error, /Unknown tool 'delete_customer'/);
  const simulated = await finance.simulatePlan({ steps: [deleteStep(customer)] });
  assert.equal(simulated.outcomes[0].status, 'invalid');
  assert.equal(await customerStatus(), 'active');

  // Adding the domain puts the tool back in reach of the same plan.
  const withCustomers = server({
    allowApply: true,
    toolProfile: 'finance',
    toolDomains: ['customers'],
  });
  const executed = await withCustomers.executePlan({
    dryRun: false,
    steps: [deleteStep(customer)],
  });
  assert.equal(executed.steps[0].status, 'success');
  assert.equal(await customerStatus(), 'deleted');
});

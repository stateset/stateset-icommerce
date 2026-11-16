/**
 * Kernel strict mode runs the two governed composites, `agentic_execute_plan`
 * and `agentic_replay_mutation`, as top-level calls, while every step they run
 * stays governed: a governed write runs as a typed kernel command under the
 * operator's principal and policy, a write the strict boundary withholds is
 * refused, and a composite nested in another composite is refused.
 *
 * Every case runs the real MCP server on a temporary SQLite file, with the
 * example operator policy and principal from kernel/examples.
 */

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { Commerce } from '../../../bindings/node/index.js';
import { createStatesetMcpServer } from '../../src/mcp-server.js';
import { REPLAY_NOT_GOVERNED } from '../../src/mcp/mutation-simulator.js';

const kernelExample = (name) =>
  JSON.parse(readFileSync(new URL(`../../../kernel/examples/${name}`, import.meta.url), 'utf8'));

const STRICT_KERNEL = {
  strict: true,
  policy: kernelExample('strict-policy.json'),
  principal: kernelExample('strict-principal.json'),
  storeId: 'store:replace-me',
};
const OPERATOR_PRINCIPAL_ID = STRICT_KERNEL.principal.id;

/** One temporary store; `server(options)` opens MCP servers on it. */
function store(t) {
  const directory = mkdtempSync(join(tmpdir(), 'stateset-strict-composites-'));
  const dbPath = join(directory, 'store.db');
  const commerce = new Commerce(dbPath);
  const servers = [];
  t.after(async () => {
    for (const server of servers) {
      server.dispose?.();
      await server.close?.().catch(() => {});
    }
    commerce.close();
    rmSync(directory, { recursive: true, force: true });
  });
  const server = (options = {}) => {
    const created = createStatesetMcpServer({
      commerce,
      dbPath,
      allowApply: true,
      toolProfile: 'all',
      ...options,
    });
    servers.push(created);
    return created;
  };
  return { commerce, server };
}

const customerStep = (email) => ({
  tool: 'create_customer',
  params: { email, firstName: 'Ada', lastName: 'Lovelace' },
});

/** create customer -> create cart -> add item, each step reading the last one's output. */
const checkoutPlan = (email) => [
  customerStep(email),
  {
    tool: 'create_cart',
    params: { customerId: '{{ steps.0.result.customer.id }}', currency: 'USD' },
  },
  {
    tool: 'add_cart_item',
    params: {
      cartId: '{{ steps.1.result.cart.id }}',
      sku: 'W-1',
      name: 'Widget',
      quantity: 2,
      unitPrice: '50.00',
    },
  },
];

const paymentLinkStep = {
  tool: 'create_payment_link',
  params: { items: [{ name: 'Strict item', quantity: 1, unitPrice: 5 }] },
};

async function countPaymentLinks(server) {
  return (await server.executeTool('list_payment_links', {})).result.count;
}

async function findCustomer(commerce, email) {
  return (await commerce.customers.list()).find((customer) => customer.email === email) ?? null;
}

/** A step that ran as a typed kernel command, sealed under the operator's principal. */
function assertGovernedStep(step, commandType) {
  assert.equal(step.status, 'success', JSON.stringify(step));
  assert.equal(step.result.kernel, true, `${step.tool} did not run through the kernel`);
  assert.equal(step.result.commandType, commandType);
  const receipt = step.result.receipt;
  assert.equal(receipt.status, 'succeeded', receipt.error_message);
  assert.equal(receipt.policy.allowed, true);
  assert.equal(receipt.policy.policy_version, STRICT_KERNEL.policy.version);
  assert.ok(receipt.audit_hash, `${step.tool} receipt is not sealed into the audit chain`);
  assert.equal(receipt.economic_context.principal.id, OPERATOR_PRINCIPAL_ID);
}

/** A step the strict boundary refused before it ran. */
function assertStrictRefusal(step, boundary, pattern) {
  assert.equal(step.status, 'permission_block', JSON.stringify(step));
  assert.equal(step.notes?.strictKernel, boundary);
  assert.match(step.error, pattern);
  assert.equal(step.result, null);
}

test('a strict plan of governed commands runs end to end, each step a kernel receipt', async (t) => {
  const { commerce, server } = store(t);
  const strict = server({ kernel: STRICT_KERNEL });

  const run = await strict.executeTool('agentic_execute_plan', {
    steps: checkoutPlan('plan@example.com'),
    dryRun: false,
  });
  assert.equal(run.ok, true, JSON.stringify(run));
  const plan = run.result;
  assert.equal(plan.finalStatus, 'success', JSON.stringify(plan.steps));
  assert.equal(plan.completedSteps, 3);
  assertGovernedStep(plan.steps[0], 'customers.create');
  assertGovernedStep(plan.steps[1], 'carts.create');
  assertGovernedStep(plan.steps[2], 'carts.item.add');

  const customer = await findCustomer(commerce, 'plan@example.com');
  assert.ok(customer, 'the plan created the customer');
  const cartId = plan.steps[1].result.cart.id;
  assert.equal((await commerce.carts.get(cartId)).customerId, customer.id);
  assert.deepEqual(
    (await commerce.carts.getItems(cartId)).map((item) => [item.sku, item.quantity]),
    [['W-1', 2]],
  );
});

test('the MCP tool call runs a strict plan the same way', async (t) => {
  const { commerce, server } = store(t);
  const strict = server({ kernel: STRICT_KERNEL });
  const tool = strict.getAdaptedTools().find((entry) => entry.name === 'agentic_execute_plan');

  const response = await tool.handler({
    steps: [customerStep('transport@example.com'), paymentLinkStep],
    dryRun: false,
  });
  const plan = JSON.parse(response.content[0].text);
  assert.equal(plan.finalStatus, 'failed', JSON.stringify(plan));
  assertGovernedStep(plan.steps[0], 'customers.create');
  assertStrictRefusal(plan.steps[1], 'withheld', /outside the governed kernel catalog/);
  assert.ok(await findCustomer(commerce, 'transport@example.com'));
  assert.equal(await countPaymentLinks(strict), 0, 'no payment link was created');
});

test('a strict plan refuses an ungoverned write step and reports failed', async (t) => {
  const { commerce, server } = store(t);
  const strict = server({ kernel: STRICT_KERNEL });

  const run = await strict.executeTool('agentic_execute_plan', {
    steps: [
      customerStep('refused@example.com'),
      paymentLinkStep,
      customerStep('after@example.com'),
    ],
    dryRun: false,
  });
  const plan = run.result;
  assert.equal(plan.finalStatus, 'failed');
  assertGovernedStep(plan.steps[0], 'customers.create');
  assertStrictRefusal(plan.steps[1], 'withheld', /'create_payment_link' is a write mutation/);
  assert.equal(plan.steps.length, 2, 'the plan stopped at the refused step');
  assert.equal(await countPaymentLinks(strict), 0, 'no payment link was created');
  assert.equal(await findCustomer(commerce, 'after@example.com'), null);

  // Called directly, the withheld tool is refused the same way.
  const direct = await strict.executeTool(paymentLinkStep.tool, paymentLinkStep.params);
  assert.equal(direct.ok, false);
  assert.equal(direct.failure.code, 'PERMISSION_DENIED');
  assert.equal(await countPaymentLinks(strict), 0);
});

test('a failed strict plan runs no ungoverned compensation on rollback', async (t) => {
  const { commerce, server } = store(t);
  const strict = server({ kernel: STRICT_KERNEL });

  const plan = (
    await strict.executeTool('agentic_execute_plan', {
      steps: [
        { tool: 'create_cart', params: { customerEmail: 'rollback@example.com' } },
        paymentLinkStep,
      ],
      dryRun: false,
    })
  ).result;
  assert.equal(plan.finalStatus, 'failed');
  assertGovernedStep(plan.steps[0], 'carts.create');
  // cancel_cart, the cart's compensation, has no typed command.
  assert.equal(plan.rollback.attempted, 1);
  assert.equal(plan.rollback.fullyReverted, false);
  assert.equal(plan.rollback.steps[0].status, 'rollback_failed');
  const cart = await commerce.carts.get(plan.steps[0].result.cart.id);
  assert.notEqual(cart.status, 'cancelled', 'the cart was left for the operator');
});

test('identity comes from the operator principal, never from step arguments', async (t) => {
  const { server } = store(t);
  const strict = server({ kernel: STRICT_KERNEL });

  const step = customerStep('identity@example.com');
  step.params.principal = { id: 'agent:attacker', capabilities: ['*'] };
  step.params.storeId = 'store:attacker';
  const plan = (await strict.executeTool('agentic_execute_plan', { steps: [step], dryRun: false }))
    .result;
  assertGovernedStep(plan.steps[0], 'customers.create');
  assert.equal(plan.steps[0].result.receipt.economic_context.store_id, STRICT_KERNEL.storeId);
});

test('a composite nested in a strict plan is refused, live or dry', async (t) => {
  const { commerce, server } = store(t);
  const strict = server({ kernel: STRICT_KERNEL });

  for (const dryRun of [false, true]) {
    const nestedPlan = {
      tool: 'agentic_execute_plan',
      params: { steps: [customerStep(`nested-${dryRun}@example.com`)], dryRun: false },
    };
    const nestedReplay = {
      tool: 'agentic_replay_mutation',
      params: { tool: 'create_customer', dryRun: false },
    };
    for (const nested of [nestedPlan, nestedReplay]) {
      const plan = (await strict.executeTool('agentic_execute_plan', { steps: [nested], dryRun }))
        .result;
      assert.equal(plan.finalStatus, 'failed', `${nested.tool} dryRun=${dryRun}`);
      assertStrictRefusal(plan.steps[0], 'nested_composite', /only as a top-level call/);
    }
    assert.equal(await findCustomer(commerce, `nested-${dryRun}@example.com`), null);
  }
});

test('a strict dry run passes governed steps and predicts a refusal', async (t) => {
  const { commerce, server } = store(t);
  const strict = server({ kernel: STRICT_KERNEL });

  const clean = (
    await strict.executeTool('agentic_execute_plan', {
      steps: [customerStep('dry@example.com'), { tool: 'list_customers', params: {} }],
    })
  ).result;
  assert.equal(clean.dryRun, true, 'plans dry-run by default');
  assert.equal(clean.finalStatus, 'dry_run');
  assert.deepEqual(
    clean.steps.map((step) => step.status),
    ['dry_run_success', 'dry_run_success'],
  );
  assert.equal(await findCustomer(commerce, 'dry@example.com'), null, 'a dry run writes nothing');

  const refused = (
    await strict.executeTool('agentic_execute_plan', {
      steps: [customerStep('dry@example.com'), paymentLinkStep],
      dryRun: true,
    })
  ).result;
  assert.equal(refused.finalStatus, 'failed');
  assert.equal(refused.steps[0].status, 'dry_run_success');
  assertStrictRefusal(refused.steps[1], 'withheld', /outside the governed kernel catalog/);
});

test('strict replay re-runs a governed command through the kernel', async (t) => {
  const { commerce, server } = store(t);
  // An operator call in preview mode logs the command; nothing runs yet.
  const operator = server({ allowApply: false });
  const logged = await operator.executeTool('create_cart', { customerEmail: 'replay@example.com' });
  assert.equal(logged.preview, true);
  const before = (await commerce.carts.list()).length;

  const strict = server({ kernel: STRICT_KERNEL });
  const dry = await strict.executeTool('agentic_replay_mutation', { tool: 'create_cart' });
  assert.equal(dry.ok, true, JSON.stringify(dry));
  assert.equal(dry.result.replay.status, 'dry_run_success');
  assert.equal((await commerce.carts.list()).length, before, 'a dry replay writes nothing');

  const replayed = await strict.executeTool('agentic_replay_mutation', {
    tool: 'create_cart',
    dryRun: false,
  });
  assert.equal(replayed.ok, true, JSON.stringify(replayed));
  assert.equal(replayed.result.sourceEvent.tool, 'create_cart');
  assertGovernedStep(replayed.result.replay, 'carts.create');
  assert.equal((await commerce.carts.list()).length, before + 1, 'the replay created the cart');
});

test('strict replay refuses a logged ungoverned write rather than skipping to an older call', async (t) => {
  const { commerce, server } = store(t);
  const operator = server({ allowApply: false });
  await operator.executeTool('create_cart', { customerEmail: 'older@example.com' });
  await operator.executeTool(paymentLinkStep.tool, paymentLinkStep.params);
  const carts = (await commerce.carts.list()).length;

  const strict = server({ kernel: STRICT_KERNEL });
  for (const filters of [{ tool: 'create_payment_link' }, {}]) {
    for (const dryRun of [false, true]) {
      const replayed = await strict.executeTool('agentic_replay_mutation', { ...filters, dryRun });
      assert.equal(replayed.ok, false, JSON.stringify(replayed));
      assert.equal(replayed.failure.code, REPLAY_NOT_GOVERNED);
      assert.match(replayed.error, /'create_payment_link'/);
      assert.equal(replayed.result.replay, undefined, 'nothing was replayed');
    }
  }
  assert.equal(await countPaymentLinks(strict), 0, 'no payment link was created');
  assert.equal((await commerce.carts.list()).length, carts, 'the older cart call was not replayed');
});

test('without strict mode, plans run ungoverned writes and nested composites as before', async (t) => {
  const { commerce, server } = store(t);
  const unstrict = [
    ['no-kernel', server()],
    ['migration', server({ kernel: { ...STRICT_KERNEL, strict: false } })],
  ];
  for (const [label, legacy] of unstrict) {
    const before = await countPaymentLinks(legacy);
    const nestedEmail = `${label}-nested@example.com`;
    const plan = (
      await legacy.executeTool('agentic_execute_plan', {
        steps: [
          paymentLinkStep,
          {
            tool: 'agentic_execute_plan',
            params: { steps: [customerStep(nestedEmail)], dryRun: false },
          },
        ],
        dryRun: false,
      })
    ).result;
    assert.equal(plan.finalStatus, 'success', `${label}: ${JSON.stringify(plan.steps)}`);
    assert.equal(await countPaymentLinks(legacy), before + 1, `${label}: the write ran`);
    assert.equal(plan.steps[1].result.finalStatus, 'success', `${label}: a nested plan runs`);
    assert.ok(await findCustomer(commerce, nestedEmail), label);
  }
});

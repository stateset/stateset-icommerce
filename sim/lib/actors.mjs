/**
 * One actor: a seeded plan walked through the governed checkout journey.
 *
 * The journey is a generator that yields *steps* and receives their results,
 * so the scheduler decides when each step runs and other actors' steps land in
 * between. A step is a governed command, a checkout-snapshot read, or an array
 * of steps issued together (a burst: two identical submits in flight at once).
 *
 * Journey per purchase:
 *   customers.create (once) -> carts.create -> carts.item.add ->
 *   carts.shipping_address.set -> carts.payment_method.set ->
 *   checkoutSnapshot (read) -> [checkout.commit reusing an earlier key: conflict] ->
 *   checkout.commit (budget commitment = snapshot grand total) ->
 *   [same checkout.commit again: replay, later or concurrently]
 */
import { classifyThrown } from './engine.mjs';
import { CURRENCY, POLICY_VERSION, SKU, STORE_ID } from './plan.mjs';
import { seededUuid } from './rng.mjs';

const ADDRESS = Object.freeze({
  first_name: 'Sim',
  last_name: 'Agent',
  company: null,
  line1: '1 Swarm Way',
  line2: null,
  city: 'Portland',
  state: 'OR',
  postal_code: '97201',
  country: 'US',
  phone: null,
  email: null,
});

/**
 * Test-only fault injection: `SIM_CRASH_ACTORS=2,5` makes those actors'
 * journeys throw before their first step, so no write of theirs ever lands
 * and the store stays consistent. The harness must still fail the run. Read
 * from the environment so forked shards inherit it.
 */
const CRASH_ACTORS = new Set(
  (process.env.SIM_CRASH_ACTORS ?? '')
    .split(',')
    .filter((value) => value.trim() !== '')
    .map(Number),
);

function* crashingJourney(index) {
  throw new Error(`injected journey crash for actor ${index} (SIM_CRASH_ACTORS)`);
}

export function createActor({ plan, principal, config }) {
  const record = {
    index: plan.index,
    principalIndex: plan.principalIndex,
    expectDenied: principal.denied,
    outcome: null,
    unitsOrdered: 0,
    spent: [],
    codes: [],
    purchases: [],
    observed: [],
    thrown: [],
    startedAt: null,
    finishedAt: null,
  };
  const actor = {
    index: plan.index,
    plan,
    principal,
    config,
    record,
    commandSeq: 0,
    keyFor: (label) => `sim:${config.seed}:a${plan.index}:${label}`,
    done: false,
    lastResult: undefined,
  };
  actor.gen = CRASH_ACTORS.has(plan.index) ? crashingJourney(plan.index) : journey(actor);
  return actor;
}

const succeeded = (result) => result?.receipt?.status === 'succeeded';
const codeOf = (result) =>
  result?.receipt
    ? (result.receipt.error_code ?? result.receipt.status)
    : `thrown:${result?.error?.kind ?? 'unknown'}`;

function command(
  actor,
  label,
  commandType,
  payload,
  { key, commitment = null, role = 'primary' } = {},
) {
  return {
    kind: 'command',
    label,
    commandType,
    payload,
    commitment,
    role,
    key: key ?? actor.keyFor(label),
  };
}

function* journey(actor) {
  const { plan, principal, record } = actor;
  const note = (label, result) => record.codes.push(`${label}=${codeOf(result)}`);

  const customer = yield command(actor, 'customer', 'customers.create', {
    email: `sim-a${plan.index}@sim.test`,
    first_name: 'Sim',
    last_name: `Agent${plan.index}`,
    phone: null,
    accepts_marketing: null,
    tags: null,
    metadata: null,
  });
  note('customer', customer);
  if (!succeeded(customer)) {
    record.outcome = 'failed';
    return;
  }
  const customerId = customer.receipt.result.id;
  let previousCheckoutKey = null;

  for (const [p, purchase] of plan.purchases.entries()) {
    const entry = {
      index: p,
      ...purchase,
      result: null,
      cartId: null,
      orderId: null,
      replay: null,
      conflictCode: null,
    };
    record.purchases.push(entry);
    const steps = [
      [
        'cart',
        'carts.create',
        () => ({
          customer_id: customerId,
          customer_email: null,
          customer_name: null,
          currency: CURRENCY,
          items: null,
          shipping_address: null,
          billing_address: null,
          notes: null,
          metadata: null,
          expires_in_minutes: null,
        }),
      ],
      [
        'item',
        'carts.item.add',
        () => ({
          cart_id: entry.cartId,
          item: {
            product_id: null,
            variant_id: null,
            sku: SKU,
            name: 'Swarm Widget',
            description: null,
            image_url: null,
            quantity: purchase.quantity,
            unit_price: actor.config.price,
            original_price: null,
            weight: null,
            requires_shipping: null,
            metadata: null,
          },
        }),
      ],
      [
        'ship-to',
        'carts.shipping_address.set',
        () => ({ cart_id: entry.cartId, address: ADDRESS }),
      ],
      [
        'payment',
        'carts.payment_method.set',
        () => ({
          cart_id: entry.cartId,
          payment: {
            payment_method: 'credit_card',
            payment_token: 'tok_sim',
            billing_address: null,
          },
        }),
      ],
    ];
    let prepared = true;
    for (const [name, commandType, payload] of steps) {
      const label = `p${p}:${name}`;
      const result = yield command(actor, label, commandType, payload());
      note(label, result);
      if (!succeeded(result)) {
        entry.result = `failed:${name}`;
        prepared = false;
        break;
      }
      if (name === 'cart') entry.cartId = result.receipt.result.id;
    }
    if (!prepared) continue;

    const snap = yield { kind: 'snapshot', cartId: entry.cartId };
    if (snap.error) {
      record.codes.push(`p${p}:snapshot=thrown:${snap.error.kind}`);
      entry.result = 'failed:snapshot';
      continue;
    }
    const payload = {
      cart_id: entry.cartId,
      stock_policy: purchase.stockPolicy,
      expected_cart_fingerprint: snap.snapshot.fingerprint,
    };
    const commitment = {
      budget_id: principal.budgetId,
      amount: { amount: snap.snapshot.cart.grand_total, currency: CURRENCY },
      asset_amount: null,
      counterparty_id: null,
      quantity: null,
      evidence: [],
    };

    if (purchase.conflict && previousCheckoutKey) {
      const label = `p${p}:conflict`;
      const conflict = yield command(actor, label, 'checkout.commit', payload, {
        key: previousCheckoutKey,
        commitment,
        role: 'conflict',
      });
      note(label, conflict);
      entry.conflictCode = codeOf(conflict);
    }

    const label = `p${p}:checkout`;
    const step = command(actor, label, 'checkout.commit', payload, { commitment });
    let commit;
    let replay = null;
    if (purchase.duplicate === 'concurrent') {
      [commit, replay] = yield [step, { ...step, role: 'replay' }];
    } else {
      commit = yield step;
      if (purchase.duplicate === 'sequential') replay = yield { ...step, role: 'replay' };
    }
    previousCheckoutKey = step.key;
    note(label, commit);
    if (replay) {
      note(`${label}:replay`, replay);
      entry.replay = {
        mode: purchase.duplicate,
        matched: Boolean(
          commit.receipt &&
          replay.receipt &&
          commit.receipt.receipt_id === replay.receipt.receipt_id,
        ),
      };
    }

    const code = codeOf(commit);
    if (succeeded(commit)) {
      entry.result = 'ordered';
      entry.orderId = commit.receipt.result?.order_id ?? null;
      record.unitsOrdered += purchase.quantity;
      record.spent.push(commit.receipt.result?.total_charged ?? '0');
    } else {
      entry.result = code;
      if (code === 'kernel.policy_denied' || code === 'kernel.budget_exceeded') break;
    }
  }
  record.outcome = classifyOutcome(record);
}

function classifyOutcome(record) {
  const results = record.purchases.map((purchase) => purchase.result);
  if (results.includes('kernel.policy_denied')) return 'denied';
  if (results.includes('kernel.budget_exceeded')) return 'budget_exhausted';
  const ordered = results.filter((result) => result === 'ordered').length;
  if (ordered === results.length && ordered > 0) return 'completed';
  if (ordered > 0) return 'partial';
  if (results.every((result) => result === 'commerce.inventory.insufficient_available'))
    return 'out_of_stock';
  return 'failed';
}

function envelope(actor, step) {
  actor.commandSeq += 1;
  return {
    contract_version: '1.0',
    command_id: seededUuid(actor.config.seed, 'command', actor.index, actor.commandSeq),
    idempotency_key: step.key,
    command_type: step.commandType,
    principal: actor.principal.principal,
    store_id: STORE_ID,
    correlation_id: seededUuid(actor.config.seed, 'actor', actor.index),
    causation_id: null,
    expected_version: null,
    policy_version: POLICY_VERSION,
    approval: null,
    authority: null,
    mandate: null,
    commitment: step.commitment,
    deadline: null,
    trace_id: null,
    mode: 'apply',
    payload: step.payload,
    issued_at: new Date().toISOString(),
  };
}

/**
 * Executes steps against one engine handle and records every outcome:
 * latencies by command type, thrown errors by kind (busy/timeout/other), and
 * the receipts each actor saw (the oracle's ground truth for "what callers
 * were told").
 */
export function createExecutor({ commerce, policy, config, stats }) {
  const time = (type, startNs) => {
    // Replays and conflicting reuses are timed apart from first submissions.
    const ms = Number(process.hrtime.bigint() - startNs) / 1e6;
    (stats.latency[type] ??= []).push(ms);
  };

  async function runCommand(actor, step) {
    for (let attempt = 0; ; attempt += 1) {
      const command = envelope(actor, step);
      const start = process.hrtime.bigint();
      try {
        const receipt = await commerce.executeKernelCommand(command, policy);
        time(
          step.role === 'primary' ? step.commandType : `${step.commandType}#${step.role}`,
          start,
        );
        actor.record.observed.push({
          actor: actor.index,
          key: step.key,
          role: step.role,
          commandType: step.commandType,
          receiptId: receipt.receipt_id,
          status: receipt.status,
          errorCode: receipt.error_code,
          reasonCodes: receipt.policy?.reason_codes ?? [],
          auditHash: receipt.audit_hash,
          aggregateId: receipt.aggregate_id,
          budgetId: receipt.economic_context?.commitment?.budget_id ?? null,
          totalCharged: receipt.result?.total_charged ?? null,
          orderId: receipt.result?.order_id ?? null,
          attempts: attempt + 1,
        });
        return { receipt, attempts: attempt + 1 };
      } catch (error) {
        time('thrown', start);
        const kind = classifyThrown(error);
        stats.thrown[kind] = (stats.thrown[kind] ?? 0) + 1;
        actor.record.thrown.push({
          label: step.label,
          kind,
          code: error.code ?? null,
          message: String(error.message).slice(0, 300),
        });
        if (attempt >= config.retries) {
          return {
            error: { kind, code: error.code ?? null, message: String(error.message) },
            attempts: attempt + 1,
          };
        }
        stats.retries += 1;
      }
    }
  }

  async function runSnapshot(step) {
    const start = process.hrtime.bigint();
    try {
      const snapshot = await commerce.checkoutSnapshot(step.cartId);
      time('snapshot', start);
      return { snapshot };
    } catch (error) {
      time('thrown', start);
      const kind = classifyThrown(error);
      stats.thrown[kind] = (stats.thrown[kind] ?? 0) + 1;
      return { error: { kind, code: error.code ?? null, message: String(error.message) } };
    }
  }

  return async function execute(actor, step) {
    if (Array.isArray(step)) return Promise.all(step.map((inner) => execute(actor, inner)));
    actor.record.startedAt ??= Date.now();
    const result =
      step.kind === 'snapshot' ? await runSnapshot(step) : await runCommand(actor, step);
    actor.record.finishedAt = Date.now();
    return result;
  };
}

export function newStats() {
  return { latency: {}, thrown: {}, retries: 0 };
}

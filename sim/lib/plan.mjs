/**
 * The swarm's configuration and its seeded plans: who each principal is, what
 * it is allowed to do, how much it may spend, and what each actor will try.
 *
 * Plans depend only on `(seed, index, config)`, never on process layout or
 * timing, so `--procs 1` and `--procs 8` run the same intentions.
 */
import { createRng } from './rng.mjs';

export const TENANT_ID = 'tenant:sim';
export const STORE_ID = 'store:sim';
export const DELEGATOR = 'user:sim-operator';
export const SKU = 'SIM-WIDGET-1';
export const CURRENCY = 'USD';
export const POLICY_VERSION = 'sim-swarm-v1';

/** Governed commands one checkout journey issues, each gated by its own capability. */
export const CHECKOUT_CAPABILITIES = Object.freeze([
  'customers.create',
  'carts.create',
  'carts.item.add',
  'carts.shipping_address.set',
  'carts.payment_method.set',
  'checkout.commit',
]);

/** The capability capability-less principals are missing. */
export const WITHHELD_CAPABILITY = 'checkout.commit';
export const EXPECTED_DENIAL = Object.freeze({
  errorCode: 'kernel.policy_denied',
  reasonCode: `policy.capability_missing:${WITHHELD_CAPABILITY}`,
});

export const DEFAULTS = Object.freeze({
  agents: 50,
  seed: 1,
  stock: 10,
  budget: '25.00',
  price: '10.00',
  procs: 1,
  inflight: 1,
  maxPurchases: 2,
  maxQuantity: 3,
  denyFraction: 0.1,
  duplicateFraction: 0.2,
  conflictFraction: 0.1,
  stockPolicy: 'reject',
  actorsPerPrincipal: 1,
  retries: 2,
  db: null,
  keep: false,
  json: false,
});

const STOCK_POLICIES = { reject: 'reject_if_insufficient', backorder: 'allow_backorder' };

export function normalizeConfig(input = {}) {
  const config = {
    ...DEFAULTS,
    ...Object.fromEntries(Object.entries(input).filter(([, v]) => v !== undefined)),
  };
  const ints = [
    'agents',
    'stock',
    'procs',
    'inflight',
    'maxPurchases',
    'maxQuantity',
    'actorsPerPrincipal',
    'retries',
  ];
  for (const key of ints) {
    config[key] = Number(config[key]);
    if (
      !Number.isInteger(config[key]) ||
      config[key] < (key === 'stock' || key === 'retries' ? 0 : 1)
    ) {
      throw new Error(
        `--${kebab(key)} must be a ${key === 'stock' || key === 'retries' ? 'non-negative' : 'positive'} integer`,
      );
    }
  }
  for (const key of ['denyFraction', 'duplicateFraction', 'conflictFraction']) {
    config[key] = Number(config[key]);
    if (!(config[key] >= 0 && config[key] <= 1))
      throw new Error(`--${kebab(key)} must be in [0, 1]`);
  }
  for (const key of ['budget', 'price']) {
    config[key] = String(config[key]);
    if (!/^\d+(\.\d{1,2})?$/.test(config[key]))
      throw new Error(`--${key} must be a USD amount like 25.00`);
  }
  if (!['reject', 'backorder', 'mixed'].includes(config.stockPolicy)) {
    throw new Error('--stock-policy must be reject, backorder or mixed');
  }
  config.seed = String(config.seed);
  config.procs = Math.min(config.procs, config.agents);
  return config;
}

function kebab(key) {
  return key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);
}

export const principalIndexOf = (actorIndex, config) =>
  Math.floor(actorIndex / config.actorsPerPrincipal);
export const principalCount = (config) => Math.ceil(config.agents / config.actorsPerPrincipal);

export function principalPlan(index, config) {
  const rng = createRng(config.seed, `principal:${index}`);
  const denied = rng.chance(config.denyFraction);
  const id = `agent:sim-${index}`;
  return {
    index,
    denied,
    budgetId: `budget:sim-${index}`,
    budgetLimit: config.budget,
    principal: {
      id,
      kind: 'agent',
      tenant_id: TENANT_ID,
      delegated_by: DELEGATOR,
      capabilities: CHECKOUT_CAPABILITIES.filter((cap) => !(denied && cap === WITHHELD_CAPABILITY)),
    },
  };
}

export function actorPlan(index, config) {
  const rng = createRng(config.seed, `actor:${index}`);
  const purchases = Array.from({ length: rng.int(1, config.maxPurchases) }, (_, p) => {
    const stockPolicy =
      config.stockPolicy === 'mixed'
        ? rng.pick(Object.values(STOCK_POLICIES))
        : STOCK_POLICIES[config.stockPolicy];
    const quantity = rng.int(1, config.maxQuantity);
    const duplicate = rng.chance(config.duplicateFraction)
      ? rng.pick(['sequential', 'concurrent'])
      : null;
    // A conflicting reuse needs an earlier checkout key to reuse.
    const conflict = p > 0 && rng.chance(config.conflictFraction);
    return { quantity, stockPolicy, duplicate, conflict };
  });
  return { index, principalIndex: principalIndexOf(index, config), purchases };
}

/** Host-owned policy: every journey command requires its capability; checkout requires a budget. */
export function buildPolicy() {
  const rule = (capability) => ({
    required_capabilities: [capability],
    requires_approval: false,
    requires_tenant: true,
    requires_store: true,
    allowed_tenant_ids: [TENANT_ID],
    allowed_store_ids: [STORE_ID],
    requires_agent_delegation: true,
    requires_signed_authority: false,
  });
  const commands = Object.fromEntries(CHECKOUT_CAPABILITIES.map((cap) => [cap, rule(cap)]));
  commands['checkout.commit'].requires_budget = true;
  return { version: POLICY_VERSION, commands, trusted_authority_keys: {} };
}

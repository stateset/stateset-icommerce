/**
 * Stability tiers for every MCP tool.
 *
 *   core          The default surface (`stateset-mcp` with no `--profile`).
 *                 The everyday commerce loop an agent needs: catalog,
 *                 customers, carts and checkout, orders, payments and refunds,
 *                 returns, shipments, inventory, promotions, tax, gift cards
 *                 and store credit as refund tenders, and read-only analytics.
 *                 Every core tool must pass the real-server smoke gate on a
 *                 fresh store -- it works or refuses cleanly, never crashes --
 *                 with no backlog (test/integration/mcp-tool-tiers.test.js).
 *   extended      Real, supported, but specialised domains: the finance suite,
 *                 manufacturing, warehouse management, B2B, engagement
 *                 (subscriptions, reviews, loyalty), integration plumbing.
 *                 Opt in with `--profile <name>`, `--domains a,b` or
 *                 `--profile all`.
 *   experimental  Demo, external-stack-dependent, or known-incomplete tools:
 *                 anything that needs a wallet, a chain, an API key, a demo
 *                 stack or a database handle the binding does not expose, and
 *                 anything in the smoke backlog. Only `--profile all`, a
 *                 curated profile that names the domain, or `--domains`
 *                 exposes them.
 *
 * The tier is assigned per domain (one row per entry in domain-registry.js),
 * with per-tool overrides where a domain is mostly one tier but a few tools
 * need an external stack. The agentic runtime tools (src/mcp/agentic-runtime-
 * tools.js) belong to no domain and are tiered by name.
 */

import { DOMAIN_TOOL_ENTRIES, TOOL_MODULE_BY_NAME } from './domain-registry.js';

export const TOOL_TIERS = Object.freeze(['core', 'extended', 'experimental']);

/** Why a domain or tool is experimental; rendered into the tool catalog. */
export const EXPERIMENTAL_REASONS = Object.freeze({
  // Domains
  sync: 'needs a configured sync endpoint and a database handle the binding does not expose',
  import: 'IdMapStore needs a database handle the binding does not expose',
  vector: 'needs OPENAI_API_KEY (external embedding service)',
  'agent-receipt': 'shells out to the ves-demo stack (AGENT_RECEIPT_DEMO_DIR)',
  a2a: 'most tools need an agent wallet identity; some are in the smoke backlog',
  'a2a-automation': 'background services not attached on a fresh store (smoke backlog)',
  'a2a-platform': 'background services not attached on a fresh store (smoke backlog)',
  'a2a-intelligence': 'background services not attached on a fresh store (smoke backlog)',
  'a2a-observability': 'webhook DLQ store methods missing (smoke backlog)',
  'agent-runtime': 'autonomous agent loops over wallets, escrow and on-chain settlement',
  'agent-cards': 'agent-to-agent discovery; depends on the A2A wallet identity',
  stablecoin: 'needs an agent signing key and an EVM chain',
  erc8004: 'on-chain identity registry; needs a registry database and chain',
  treasury: 'on-chain stablecoin treasury; needs a configured chain and token',
  checkout:
    'payment links and crypto checkout; state lives in ~/.stateset/a2a.db, not the --db store',
  'circuit-breaker':
    'agent spending breakers; state lives in ~/.stateset/a2a.db, not the --db store',
  compliance:
    'reads ~/.stateset/a2a.db, not the --db store; GDPR/SOC2 exports crash (smoke backlog)',
  catalog: 'agent product catalog; state lives in ~/.stateset/a2a.db, not the --db store',
  // Tools
  x402_settle_intent_onchain: 'submits an on-chain settlement transaction',
  x402_execute_agent_payment: 'needs an agent signing key (wallet)',
  delegate_to_agent: 'needs the autonomous engine, which stateset-mcp does not start',
});

/** One tier per domain in domain-registry.js; a test enforces full coverage. */
export const DOMAIN_TIERS = Object.freeze({
  // core -- the default surface
  customers: 'core',
  orders: 'core',
  products: 'core',
  inventory: 'core',
  carts: 'core',
  payments: 'core',
  returns: 'core',
  shipments: 'core',
  analytics: 'core',
  tax: 'core',
  promotions: 'core',
  'gift-cards': 'core',
  'store-credits': 'core',
  explain: 'core',

  // extended -- real, specialised domains
  'custom-objects': 'extended',
  currency: 'extended',
  subscriptions: 'extended',
  manufacturing: 'extended',
  x402: 'extended',
  suppliers: 'extended',
  invoices: 'extended',
  warranties: 'extended',
  policies: 'extended',
  segments: 'extended',
  'shipping-zones': 'extended',
  'units-of-measure': 'extended',
  'stock-snapshots': 'extended',
  'print-stations': 'extended',
  'integration-mappings': 'extended',
  'integration-field-mappings': 'extended',
  'payment-obligations': 'extended',
  purgatory: 'extended',
  'topology-snapshots': 'extended',
  'vendor-returns': 'extended',
  reviews: 'extended',
  wishlists: 'extended',
  loyalty: 'extended',
  fraud: 'extended',
  connectors: 'extended',
  audit: 'extended',
  proofs: 'extended',
  quality: 'extended',
  lots: 'extended',
  'search-config': 'extended',
  serials: 'extended',
  warehouse: 'extended',
  receiving: 'extended',
  fulfillment: 'extended',
  'accounts-payable': 'extended',
  'accounts-receivable': 'extended',
  'cost-accounting': 'extended',
  credit: 'extended',
  backorders: 'extended',
  'general-ledger': 'extended',
  'fixed-assets': 'extended',
  maintenance: 'extended',
  'revenue-recognition': 'extended',
  'cycle-counts': 'extended',
  'edi-documents': 'extended',
  prepayments: 'extended',
  'activity-logs': 'extended',
  channels: 'extended',
  companies: 'extended',
  'vendor-credits': 'extended',
  'price-schedules': 'extended',
  'price-levels': 'extended',
  'transfer-orders': 'extended',
  'production-batches': 'extended',
  'supplier-skus': 'extended',
  'inbound-shipments': 'extended',

  // experimental -- see EXPERIMENTAL_REASONS
  sync: 'experimental',
  import: 'experimental',
  vector: 'experimental',
  'agent-receipt': 'experimental',
  a2a: 'experimental',
  'a2a-automation': 'experimental',
  'a2a-platform': 'experimental',
  'a2a-intelligence': 'experimental',
  'a2a-observability': 'experimental',
  'agent-runtime': 'experimental',
  'agent-cards': 'experimental',
  stablecoin: 'experimental',
  erc8004: 'experimental',
  treasury: 'experimental',
  checkout: 'experimental',
  'circuit-breaker': 'experimental',
  compliance: 'experimental',
  catalog: 'experimental',
});

/** Tools whose tier differs from their domain's. */
export const TOOL_TIER_OVERRIDES = Object.freeze({
  x402_settle_intent_onchain: 'experimental',
  x402_execute_agent_payment: 'experimental',
});

/**
 * The agentic runtime tools (no domain). The planning, simulation, replay and
 * event tools operate over whatever surface the server exposes, so they are
 * core; the Machine Payments Protocol helpers are extended; delegation needs
 * an autonomous engine the stdio server does not start.
 */
export const RUNTIME_TOOL_TIERS = Object.freeze({
  agentic_runtime_contract: 'core',
  agentic_tool_catalog: 'core',
  agentic_payment_discovery: 'extended',
  agentic_prepare_payment: 'extended',
  agentic_plan: 'core',
  agentic_simulate_mutation: 'core',
  agentic_replay_mutation: 'core',
  agentic_replay: 'core',
  agentic_subscribe_events: 'core',
  agentic_unsubscribe_events: 'core',
  agentic_list_event_subscriptions: 'core',
  agentic_get_event_history: 'core',
  agentic_execute_plan: 'core',
  discover_tools: 'core',
  delegate_to_agent: 'experimental',
});

/**
 * The tier of a tool by name, or `undefined` for a tool nobody tiered (the
 * tier test fails on that; callers treat it as not-core).
 * @param {string} name
 * @returns {'core'|'extended'|'experimental'|undefined}
 */
export function toolTier(name) {
  if (Object.hasOwn(TOOL_TIER_OVERRIDES, name)) return TOOL_TIER_OVERRIDES[name];
  if (Object.hasOwn(RUNTIME_TOOL_TIERS, name)) return RUNTIME_TOOL_TIERS[name];
  const domain = TOOL_MODULE_BY_NAME[name];
  return domain ? DOMAIN_TIERS[domain] : undefined;
}

/** Every domain tool's tier, by name. */
export const TOOL_TIER_BY_NAME = Object.freeze(
  Object.fromEntries(
    DOMAIN_TOOL_ENTRIES.flatMap(([, tools]) =>
      tools.filter((tool) => tool?.name).map((tool) => [tool.name, toolTier(tool.name)]),
    ),
  ),
);

/** A domain's tier, or the mix of tiers when overrides split it. */
export function domainTierLabel(domain) {
  const tools = DOMAIN_TOOL_ENTRIES.find(([name]) => name === domain)?.[1] ?? [];
  const tiers = new Set(tools.map((tool) => toolTier(tool.name)));
  return tiers.size <= 1 ? (DOMAIN_TIERS[domain] ?? 'untiered') : [...tiers].sort().join(' + ');
}

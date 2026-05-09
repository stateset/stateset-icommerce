// Static policy-domain inference for MCP tool names.
//
// Every tool in the MCP catalog is tagged with a `policyDomain` (e.g.
// "orders", "carts", "stablecoin") that the policy engine uses to gate
// invocations. Most tools declare it explicitly via their module
// metadata (`TOOL_POLICY_DOMAIN_BY_NAME`); for the rest we infer it from
// the tool name itself.
//
// Inference rules (in order):
//   1. Exact match in `TOOL_POLICY_DOMAIN_BY_NAME`.
//   2. Multi-part prefix matches (`a2a_*` → "a2a", `agent_card_*` →
//      "agent_cards", `custom_object_*` → "custom_objects").
//   3. First underscore-token that hits `STATIC_POLICY_DOMAIN_BY_TOKEN`.
//   4. Default → "commerce".
//
// Extracted from mcp-server.js. The lookup table is intentionally
// permissive: many tokens (e.g. `create`, `get`, `list`) fall back to
// the umbrella "commerce" domain so we don't crash when a new tool
// lands without a domain tag.

import { TOOL_POLICY_DOMAIN_BY_NAME } from '../tools/domain-registry.js';

/** Token → domain. Single-word tokens lifted from `tool_name.split('_')`. */
export const STATIC_POLICY_DOMAIN_BY_TOKEN = {
  customer: 'customers',
  customers: 'customers',
  order: 'orders',
  orders: 'orders',
  product: 'products',
  products: 'products',
  inventory: 'inventory',
  custom: 'custom_objects',
  custom_object: 'custom_objects',
  custom_objects: 'custom_objects',
  returns: 'returns',
  return: 'returns',
  cart: 'carts',
  carts: 'carts',
  analytics: 'analytics',
  currency: 'currency',
  currencies: 'currency',
  tax: 'tax',
  promotion: 'promotions',
  promotions: 'promotions',
  subscription: 'subscriptions',
  subscriptions: 'subscriptions',
  sync: 'sync',
  manufacturing: 'manufacturing',
  payment: 'payments',
  payments: 'payments',
  stablecoin: 'stablecoin',
  treasury: 'treasury',
  erc8004: 'erc8004',
  x402: 'x402',
  agent: 'agent_cards',
  agent_card: 'agent_cards',
  agent_cards: 'agent_cards',
  a2a: 'a2a',
  shipment: 'shipments',
  shipments: 'shipments',
  supplier: 'suppliers',
  suppliers: 'suppliers',
  invoice: 'invoices',
  invoices: 'invoices',
  warranty: 'warranties',
  warranties: 'warranties',
  vector: 'vector',
  create: 'commerce',
  get: 'commerce',
  list: 'commerce',
  update: 'commerce',
  delete: 'commerce',
  set: 'commerce',
  ship: 'orders',
  cancel: 'orders',
  request: 'a2a',
  provide: 'a2a',
  accept: 'a2a',
  decline: 'a2a',
  pause: 'subscriptions',
  resume: 'subscriptions',
  skip: 'subscriptions',
};

/**
 * Resolve a static policy domain for a given tool name.
 *
 * @param {string} toolName - the tool's snake_case name (e.g. "create_order")
 * @param {Record<string, string>} [byName] - override for the
 *   per-tool domain map (defaults to the registry's map). Passed in
 *   tests to verify priority over token-based inference.
 * @returns {string} the policy domain, never null/undefined; falls back to "commerce"
 */
export function inferStaticPolicyDomain(toolName, byName = TOOL_POLICY_DOMAIN_BY_NAME) {
  if (!toolName || typeof toolName !== 'string') return 'commerce';

  if (byName[toolName]) {
    return byName[toolName];
  }

  const parts = toolName.split('_').filter(Boolean);
  if (parts.length === 0) return 'commerce';

  if (parts.length >= 2 && parts[0] === 'a2a') return 'a2a';
  if (parts.length >= 2 && parts[0] === 'agent' && parts[1] === 'card') return 'agent_cards';
  if (parts.length >= 2 && parts[0] === 'custom' && parts[1] === 'object') {
    return 'custom_objects';
  }

  for (const part of parts) {
    if (STATIC_POLICY_DOMAIN_BY_TOKEN[part]) {
      return STATIC_POLICY_DOMAIN_BY_TOKEN[part];
    }
  }

  return 'commerce';
}

/**
 * Resolve a policy domain using both the per-tool definition (if any) and
 * the static-name inference fallback.
 *
 * Priority:
 *   1. `toolDefsByName.get(toolName)?.policyDomain` (declared on the tool's
 *      MCP module metadata)
 *   2. `inferStaticPolicyDomain(toolName)` (token-based fallback)
 *
 * @param {string} toolName
 * @param {Map<string, {policyDomain?: string}>} toolDefsByName - the
 *   per-tool definition map, keyed by tool name. Pass the orchestrator's
 *   `TOOL_DEFS_BY_NAME` map.
 * @returns {string} the resolved policy domain
 */
export function inferPolicyDomain(toolName, toolDefsByName) {
  const candidate = toolDefsByName?.get?.(toolName);
  if (candidate?.policyDomain) {
    return candidate.policyDomain;
  }
  return inferStaticPolicyDomain(toolName);
}

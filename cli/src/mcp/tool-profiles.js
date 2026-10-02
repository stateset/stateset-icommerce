import { TOOL_MODULE_BY_NAME, TOOL_MODULE_NAMES } from '../tools/domain-registry.js';
import { DOMAIN_TIERS, toolTier } from '../tools/tool-tiers.js';

export { DEFAULT_MCP_TOOL_PROFILE } from './default-tool-profile.js';

/**
 * Profiles are domain lists. `core` is derived from the tier map and, unlike
 * the curated profiles, is filtered tool by tool: it exposes exactly the tools
 * whose tier is `core` (see resolveMcpToolFilter). `all` exposes every tier.
 */
export const MCP_TOOL_PROFILES = Object.freeze({
  all: TOOL_MODULE_NAMES,
  core: Object.freeze(TOOL_MODULE_NAMES.filter((domain) => DOMAIN_TIERS[domain] === 'core')),
  operations: [
    'inventory',
    'manufacturing',
    'shipments',
    'suppliers',
    'warranties',
    'warehouse',
    'receiving',
    'fulfillment',
    'quality',
    'lots',
    'serials',
    'cycle-counts',
    'transfer-orders',
    'production-batches',
    'supplier-skus',
    'inbound-shipments',
    'backorders',
    'vendor-returns',
  ],
  finance: [
    'payments',
    'invoices',
    'treasury',
    'accounts-payable',
    'accounts-receivable',
    'cost-accounting',
    'credit',
    'general-ledger',
    'fixed-assets',
    'revenue-recognition',
    'prepayments',
    'vendor-credits',
    'payment-obligations',
  ],
  agents: [
    'agent-runtime',
    'agent-cards',
    'agent-receipt',
    'a2a',
    'a2a-platform',
    'a2a-automation',
    'a2a-observability',
    'a2a-intelligence',
    'x402',
    'stablecoin',
    'erc8004',
    'treasury',
    'payment-obligations',
    'proofs',
    'audit',
    'policies',
  ],
});

export function resolveMcpToolDomains({ profile = 'all', domains = [] } = {}) {
  if (!Object.hasOwn(MCP_TOOL_PROFILES, profile)) {
    throw new Error(
      `Unknown MCP tool profile "${profile}". Expected one of: ${Object.keys(MCP_TOOL_PROFILES).join(', ')}`,
    );
  }
  const requested = new Set([...MCP_TOOL_PROFILES[profile], ...domains]);
  const unknown = [...requested].filter((domain) => !TOOL_MODULE_NAMES.includes(domain));
  if (unknown.length) throw new Error(`Unknown MCP tool domain(s): ${unknown.join(', ')}`);
  return requested;
}

/**
 * Decide, tool by tool, what a profile exposes.
 *
 *   all       every tool.
 *   core      every tool whose tier is `core` -- domain tools and agentic
 *             runtime tools alike -- plus any `domains` added explicitly.
 *   others    every tool in the profile's domains plus `domains`, and the
 *             agentic runtime tools (which belong to no domain), as before.
 *
 * @param {{ profile?: string, domains?: string[] }} [options]
 * @returns {(toolName: string) => boolean}
 */
export function resolveMcpToolFilter({ profile = 'all', domains = [] } = {}) {
  const selected = resolveMcpToolDomains({ profile, domains });
  if (profile === 'all') return () => true;
  if (profile === 'core') {
    const added = new Set(domains);
    return (name) => toolTier(name) === 'core' || added.has(TOOL_MODULE_BY_NAME[name]);
  }
  return (name) => !TOOL_MODULE_BY_NAME[name] || selected.has(TOOL_MODULE_BY_NAME[name]);
}

import { TOOL_MODULE_NAMES } from '../tools/domain-registry.js';

export const MCP_TOOL_PROFILES = Object.freeze({
  all: TOOL_MODULE_NAMES,
  core: [
    'customers',
    'orders',
    'products',
    'inventory',
    'carts',
    'checkout',
    'payments',
    'returns',
    'shipments',
    'analytics',
    'tax',
    'promotions',
    'subscriptions',
    'gift-cards',
    'store-credits',
    'reviews',
    'wishlists',
    'loyalty',
  ],
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

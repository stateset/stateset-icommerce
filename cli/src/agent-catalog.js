/**
 * Shared agent identifiers used across the CLI, MCP server, and tests.
 *
 * Keep this list aligned with the concrete AGENTS definition in
 * ./agent-definitions.js. Integration coverage tests enforce parity.
 */

export const SUPPORTED_AGENT_NAMES = Object.freeze([
  'customer-service',
  'checkout',
  'orders',
  'inventory',
  'returns',
  'analytics',
  'promotions',
  'subscriptions',
  'storefront',
  'sync',
  'manufacturing',
  'payments',
  'stablecoin',
  'shipments',
  'suppliers',
  'invoices',
  'warranties',
  'currency',
  'agents',
  'tax',
]);

export const SUPPORTED_AGENT_NAMES_DESCRIPTION = SUPPORTED_AGENT_NAMES.join(', ');

export function isSupportedAgentName(agentName) {
  return SUPPORTED_AGENT_NAMES.includes(agentName);
}

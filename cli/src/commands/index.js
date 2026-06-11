/**
 * Command Registry
 *
 * Central registry for all CLI commands, enabling modular command loading
 * and unified help/completion generation.
 */

import * as customers from './customers.js';
import * as orders from './orders.js';
import * as products from './products.js';
import * as inventory from './inventory.js';
import * as returns from './returns.js';
import * as a2a from './a2a.js';
import * as agentCards from './agent-cards.js';
import * as agentReceipt from './agent-receipt.js';
import * as agentRuntime from './agent-runtime.js';
import * as a2aAutomation from './a2a-automation.js';
import * as a2aIntelligence from './a2a-intelligence.js';
import * as a2aObservability from './a2a-observability.js';
import * as a2aPlatform from './a2a-platform.js';
import * as carts from './carts.js';
import * as checkout from './checkout.js';
import * as analytics from './analytics.js';
import * as loyalty from './loyalty.js';
import * as giftCards from './gift-cards.js';
import * as storeCredits from './store-credits.js';
import * as warehouse from './warehouse.js';
import * as receiving from './receiving.js';
import * as fulfillment from './fulfillment.js';
import * as accountsPayable from './accounts-payable.js';
import * as accountsReceivable from './accounts-receivable.js';
import * as generalLedger from './general-ledger.js';
import * as costAccounting from './cost-accounting.js';
import * as credit from './credit.js';
import * as backorders from './backorders.js';
import * as lots from './lots.js';
import * as serials from './serials.js';
import * as quality from './quality.js';
import * as reviews from './reviews.js';
import * as wishlists from './wishlists.js';
import * as segments from './segments.js';
import * as catalog from './catalog.js';
import * as fraud from './fraud.js';
import * as audit from './audit.js';
import * as manufacturing from './manufacturing.js';
import * as customObjects from './custom-objects.js';
import * as connectors from './connectors.js';
import * as shippingZones from './shipping-zones.js';
import * as compliance from './compliance.js';
import * as policies from './policies.js';
import * as sync from './sync.js';
import * as circuitBreaker from './circuit-breaker.js';
import * as erc8004 from './erc8004.js';
import * as importCommands from './import.js';
import * as proofs from './proofs.js';
import * as promotions from './promotions.js';
import * as stablecoin from './stablecoin.js';
import * as subscriptions from './subscriptions.js';
import * as currency from './currency.js';
import * as tax from './tax.js';
import * as treasury from './treasury.js';
import * as vector from './vector.js';
import * as payments from './payments.js';
import * as shipments from './shipments.js';
import * as suppliers from './suppliers.js';
import * as invoices from './invoices.js';
import * as warranties from './warranties.js';
import * as x402 from './x402.js';

/**
 * All registered commands
 */
export const commands = {
  customers,
  orders,
  products,
  inventory,
  returns,
  a2a,
  'agent-cards': agentCards,
  'agent-receipt': agentReceipt,
  'agent-runtime': agentRuntime,
  'a2a-automation': a2aAutomation,
  'a2a-intelligence': a2aIntelligence,
  'a2a-observability': a2aObservability,
  'a2a-platform': a2aPlatform,
  carts,
  checkout,
  analytics,
  loyalty,
  'gift-cards': giftCards,
  'store-credits': storeCredits,
  warehouse,
  receiving,
  fulfillment,
  'accounts-payable': accountsPayable,
  'accounts-receivable': accountsReceivable,
  'general-ledger': generalLedger,
  'cost-accounting': costAccounting,
  credit,
  backorders,
  lots,
  serials,
  quality,
  reviews,
  wishlists,
  segments,
  catalog,
  fraud,
  audit,
  manufacturing,
  'custom-objects': customObjects,
  connectors,
  'shipping-zones': shippingZones,
  compliance,
  policies,
  sync,
  'circuit-breaker': circuitBreaker,
  erc8004,
  import: importCommands,
  proofs,
  promotions,
  stablecoin,
  subscriptions,
  currency,
  tax,
  treasury,
  vector,
  payments,
  shipments,
  suppliers,
  invoices,
  warranties,
  x402,
};

/**
 * Resource aliases for shorthand commands
 */
export const RESOURCE_ALIASES = {
  // Single letter shortcuts
  c: 'customers',
  o: 'orders',
  p: 'products',
  i: 'inventory',
  r: 'returns',
  cart: 'carts',
  a: 'analytics',
  t: 'tax',
  // Common abbreviations
  cust: 'customers',
  ord: 'orders',
  prod: 'products',
  inv: 'inventory',
  ret: 'returns',
  p2p: 'a2a',
  cards: 'agent-cards',
  'agent-card': 'agent-cards',
  rt: 'agent-runtime',
  runtime: 'agent-runtime',
  a2aa: 'a2a-automation',
  ops: 'a2a-automation',
  a2ai: 'a2a-intelligence',
  intel: 'a2a-intelligence',
  a2ao: 'a2a-observability',
  obs: 'a2a-observability',
  a2ap: 'a2a-platform',
  messaging: 'a2a-platform',
  xpay: 'x402',
  basket: 'carts',
  cko: 'checkout',
  paylink: 'checkout',
  rewards: 'loyalty',
  points: 'loyalty',
  giftcard: 'gift-cards',
  gc: 'gift-cards',
  credits: 'store-credits',
  credit: 'store-credits',
  wh: 'warehouse',
  warehouses: 'warehouse',
  receipts: 'receiving',
  recv: 'receiving',
  fulfill: 'fulfillment',
  pick: 'fulfillment',
  ap: 'accounts-payable',
  bills: 'accounts-payable',
  ar: 'accounts-receivable',
  'credit-memos': 'accounts-receivable',
  gl: 'general-ledger',
  ledger: 'general-ledger',
  costs: 'cost-accounting',
  costing: 'cost-accounting',
  'credit-accounts': 'credit',
  lending: 'credit',
  bo: 'backorders',
  backorder: 'backorders',
  lot: 'lots',
  batches: 'lots',
  serial: 'serials',
  sn: 'serials',
  qa: 'quality',
  ncr: 'quality',
  rev: 'reviews',
  review: 'reviews',
  wl: 'wishlists',
  wishlist: 'wishlists',
  seg: 'segments',
  segment: 'segments',
  cat: 'catalog',
  catalogue: 'catalog',
  risk: 'fraud',
  'fraud-review': 'fraud',
  logs: 'audit',
  auditlog: 'audit',
  mfg: 'manufacturing',
  bom: 'manufacturing',
  co: 'custom-objects',
  metaobjects: 'custom-objects',
  conn: 'connectors',
  wasm: 'connectors',
  zones: 'shipping-zones',
  shipzones: 'shipping-zones',
  cmp: 'compliance',
  regulatory: 'compliance',
  policy: 'policies',
  rules: 'policies',
  ves: 'sync',
  sequencer: 'sync',
  cb: 'circuit-breaker',
  breaker: 'circuit-breaker',
  identity: 'erc8004',
  registry: 'erc8004',
  ingest: 'import',
  etl: 'import',
  proof: 'proofs',
  an: 'analytics',
  promo: 'promotions',
  sc: 'stablecoin',
  stable: 'stablecoin',
  subs: 'subscriptions',
  curr: 'currency',
  fx: 'currency',
  vat: 'tax',
  treas: 'treasury',
  cash: 'treasury',
  vec: 'vector',
  semantic: 'vector',
  pay: 'payments',
  pmt: 'payments',
  ship: 'shipments',
  ships: 'shipments',
  shp: 'shipments',
  supp: 'suppliers',
  po: 'suppliers',
  invc: 'invoices',
  bill: 'invoices',
  warranty: 'warranties',
  claims: 'warranties',
  stock: 'inventory',
};

/**
 * Action aliases for shorthand actions
 */
export const ACTION_ALIASES = {
  l: 'list',
  ls: 'list',
  g: 'get',
  s: 'ship',
  x: 'cancel',
  a: 'adjust',
  n: 'count',
  '#': 'count',
};

/**
 * Expand resource alias to full name
 */
export function expandResource(input) {
  if (!input) return input;
  const lower = input.toLowerCase();
  return RESOURCE_ALIASES[lower] || lower;
}

/**
 * Expand action alias to full name
 */
export function expandAction(input) {
  if (!input) return input;
  const lower = input.toLowerCase();
  return ACTION_ALIASES[lower] || lower;
}

/**
 * Get command module by resource name
 */
export function getCommand(resource) {
  const expanded = expandResource(resource);
  return commands[expanded];
}

/**
 * Execute a command
 */
export async function executeCommand(resource, action, args, context) {
  const command = getCommand(resource);

  if (!command) {
    throw new Error(
      `Unknown resource: ${resource}\n\n` +
        'Available resources:\n' +
        Object.keys(commands)
          .map((r) => `  ${r}`)
          .join('\n'),
    );
  }

  const expandedAction = expandAction(action);
  return command.execute(expandedAction, args, context);
}

/**
 * Generate help text for all commands
 */
export function generateHelp() {
  const lines = ['StateSet iCommerce CLI - Direct Mode\n'];
  lines.push('RESOURCES & ACTIONS:\n');

  for (const [name, command] of Object.entries(commands)) {
    const meta = command.metadata;
    lines.push(`  ${name} (${meta.aliases.join(', ')})`);

    for (const [action, info] of Object.entries(meta.actions)) {
      const argsStr = info.args.length > 0 ? ' ' + info.args.join(' ') : '';
      lines.push(`    ${action}${argsStr}`.padEnd(35) + info.description);
    }
    lines.push('');
  }

  return lines.join('\n');
}

/**
 * Get all command completions for shell completion
 */
export function getCompletions() {
  const completions = {
    resources: [],
    actions: {},
  };

  for (const [name, command] of Object.entries(commands)) {
    const meta = command.metadata;
    completions.resources.push(name, ...meta.aliases);
    completions.actions[name] = Object.keys(meta.actions);

    // Also map aliases
    for (const alias of meta.aliases) {
      completions.actions[alias] = Object.keys(meta.actions);
    }
  }

  return completions;
}

export default {
  commands,
  RESOURCE_ALIASES,
  ACTION_ALIASES,
  expandResource,
  expandAction,
  getCommand,
  executeCommand,
  generateHelp,
  getCompletions,
};

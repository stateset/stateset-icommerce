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

/**
 * All registered commands
 */
export const commands = {
  customers,
  orders,
  products,
  inventory,
  returns
};

/**
 * Resource aliases for shorthand commands
 */
export const RESOURCE_ALIASES = {
  // Single letter shortcuts
  'c': 'customers',
  'o': 'orders',
  'p': 'products',
  'i': 'inventory',
  'r': 'returns',
  // Common abbreviations
  'cust': 'customers',
  'ord': 'orders',
  'prod': 'products',
  'inv': 'inventory',
  'ret': 'returns',
  'stock': 'inventory'
};

/**
 * Action aliases for shorthand actions
 */
export const ACTION_ALIASES = {
  'l': 'list',
  'ls': 'list',
  'g': 'get',
  's': 'ship',
  'x': 'cancel',
  'a': 'adjust',
  'n': 'count',
  '#': 'count'
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
      Object.keys(commands).map(r => `  ${r}`).join('\n')
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
    actions: {}
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
  getCompletions
};

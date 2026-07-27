/**
 * Tool-Backed Command Factory
 *
 * Builds a CLI command module directly from a domain's MCP tool definitions,
 * so the command surface cannot drift from the tool surface: every tool the
 * MCP server exposes for the domain is reachable from the CLI, with the same
 * zod validation and the same `--apply` write guard.
 *
 * Actions are derived from tool names by stripping the domain's own tokens:
 *   list_transfer_orders        -> list
 *   add_transfer_order_line     -> add-line
 *   check_channels_supported    -> check-supported
 *
 * Arguments are `key=value` pairs. Values parse as JSON when they look like
 * it (numbers, booleans, arrays, objects), otherwise as strings:
 *   stateset transfer-orders list status=draft limit=10
 *   stateset price-levels create name="Wholesale" 'discountPercent=12.5'
 */

import { z } from 'zod';
import { ValidationError } from '../errors.js';

/** Tokens (singular + plural insensitive) contributed by a domain name. */
function domainTokens(domain) {
  const tokens = new Set();
  for (const raw of domain.split('-')) {
    tokens.add(raw);
    // singular/plural pairs: logs/log, batches/batch, entries/entry
    if (raw.endsWith('ies')) tokens.add(`${raw.slice(0, -3)}y`);
    if (raw.endsWith('es')) tokens.add(raw.slice(0, -2));
    if (raw.endsWith('s')) tokens.add(raw.slice(0, -1));
    tokens.add(`${raw}s`);
    tokens.add(`${raw}es`);
  }
  return tokens;
}

/** Derive the CLI action name for a tool within its domain. */
export function deriveActionName(toolName, domain) {
  const tokens = domainTokens(domain);
  const remaining = toolName.split('_').filter((part) => !tokens.has(part));
  // A tool named exactly after the domain (rare) falls back to its full name.
  return (remaining.length > 0 ? remaining : toolName.split('_')).join('-');
}

/** Parse `key=value` CLI args into a params object with JSON-ish coercion. */
export function parseKeyValueArgs(args) {
  const params = {};
  for (const arg of args) {
    const eq = arg.indexOf('=');
    if (eq <= 0) {
      throw new ValidationError(`Expected key=value argument, got: ${arg}`, {
        details: { hint: 'Pass parameters as key=value pairs, e.g. limit=10 status=draft' },
      });
    }
    const key = arg.slice(0, eq);
    const raw = arg.slice(eq + 1);
    try {
      params[key] = JSON.parse(raw);
    } catch {
      params[key] = raw;
    }
  }
  return params;
}

function describeSchema(inputSchema) {
  return Object.keys(inputSchema || {})
    .map((key) => `${key}=<value>`)
    .join(' ');
}

/**
 * Create a `{ execute, metadata }` command module backed by tool definitions.
 *
 * @param {string} domain - kebab-case domain name matching the tools file
 * @param {Array<object>} tools - the domain's MCP tool definitions
 */
export function createToolBackedCommand(domain, tools) {
  const actions = new Map();
  for (const tool of tools) {
    actions.set(deriveActionName(tool.name, domain), tool);
  }

  function usage() {
    const lines = [`Available ${domain} actions:`];
    for (const [action, tool] of [...actions.entries()].sort()) {
      const params = describeSchema(tool.inputSchema);
      lines.push(`  ${action}${params ? ` ${params}` : ''}`);
      lines.push(`      ${tool.description}`);
    }
    lines.push('');
    lines.push('Parameters are key=value pairs; write operations need --apply.');
    return lines.join('\n');
  }

  async function execute(action, args, context = {}) {
    if (!action || action === 'help' || action === 'actions') {
      return { formatted: usage() };
    }

    const tool = actions.get(action);
    if (!tool) {
      throw new ValidationError(`Unknown ${domain} action: ${action}`, {
        details: { usage: usage() },
      });
    }

    const rawParams = parseKeyValueArgs(args);
    const parsed = z.object(tool.inputSchema || {}).safeParse(rawParams);
    if (!parsed.success) {
      const detail = parsed.error.issues
        .map((issue) => `${issue.path.join('.') || '(root)'}: ${issue.message}`)
        .join('; ');
      throw new ValidationError(`Invalid parameters for ${domain} ${action}: ${detail}`, {
        details: { usage: `${domain} ${action} ${describeSchema(tool.inputSchema)}` },
      });
    }

    const result = await tool.handler({
      commerce: context.commerce,
      params: parsed.data,
      allowApply: Boolean(context.allowApply ?? context.apply),
      agentConfig: context.agentConfig,
    });

    return result;
  }

  return {
    execute,
    metadata: {
      description: `${domain} operations (tool-backed)`,
      actions: [...actions.keys()].sort(),
    },
    // Same shape as the hand-written tool-backed modules (a2a, x402, ...):
    // lets the tool-backed coverage test assert every tool maps exactly once.
    toolActionMap: [...actions.entries()].map(([action, tool]) => ({ action, tool: tool.name })),
  };
}

/**
 * Shared helpers for command modules backed by tool handlers.
 */

export function parseJsonArg(value, label) {
  if (!value) throw new Error(`Missing ${label}.`);
  try {
    return JSON.parse(value);
  } catch (error) {
    throw new Error(`Invalid ${label} JSON: ${error.message}`);
  }
}

export function parseOptionalBoolean(value, usage) {
  if (value === undefined || value === null || value === '') return undefined;
  const normalized = String(value).trim().toLowerCase();
  if (['true', '1', 'yes', 'y', 'on'].includes(normalized)) return true;
  if (['false', '0', 'no', 'n', 'off'].includes(normalized)) return false;
  throw new Error(usage);
}

export function parseOptionalInteger(value, usage) {
  if (value === undefined || value === null || value === '') return undefined;
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed)) throw new Error(usage);
  return parsed;
}

export function parseOptionalNumber(value, usage) {
  if (value === undefined || value === null || value === '') return undefined;
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed)) throw new Error(usage);
  return parsed;
}

export function parseCsvArg(value) {
  if (!value) return undefined;
  return value
    .split(',')
    .map((entry) => entry.trim())
    .filter(Boolean);
}

export async function invokeTool(tools, toolName, context, params = {}, options = {}) {
  const tool = tools.find((entry) => entry.name === toolName);
  if (!tool) throw new Error(`Unknown tool mapping: ${toolName}`);

  const toolContext = {
    commerce: context.commerce,
    params,
    allowApply: options.allowApply !== false,
  };

  if (options.agentAddress) {
    toolContext.agentConfig = { walletAddress: options.agentAddress };
  }

  return tool.handler(toolContext);
}

function stringifyValue(value) {
  if (value === null || value === undefined) return '';
  if (typeof value === 'object') return JSON.stringify(value);
  return String(value);
}

function buildTable(rows, output) {
  if (!output || typeof output.table !== 'function') return null;
  if (!rows.every((row) => row && typeof row === 'object' && !Array.isArray(row))) return null;

  const keys = Object.keys(rows[0]).slice(0, 6);
  if (keys.length === 0) return null;

  return output.table(
    rows.map((row) => Object.fromEntries(keys.map((key) => [key, stringifyValue(row[key])]))),
    keys.map((key) => ({ key, header: key })),
  );
}

export function formatToolResult(
  result,
  { output, jsonOutput },
  emptyMessage = 'No results found.',
) {
  if (jsonOutput) return result;

  if (Array.isArray(result)) {
    if (result.length === 0) return { result, formatted: emptyMessage };
    const table = buildTable(result, output);
    return { result, formatted: table || JSON.stringify(result, null, 2) };
  }

  if (result && typeof result === 'object') {
    if (typeof result.formatted === 'string') return result;

    const entries = Object.entries(result);
    const arrayEntry = entries.find(([, value]) => Array.isArray(value));
    if (arrayEntry) {
      const [key, rows] = arrayEntry;
      if (rows.length === 0) {
        return { result, formatted: `No ${key} found.` };
      }
      const table = buildTable(rows, output);
      if (table) return { result, formatted: table };
    }

    return { result, formatted: JSON.stringify(result, null, 2) };
  }

  return { result, formatted: String(result) };
}

export function createMetadata(name, aliases, description, actions) {
  return {
    name,
    aliases,
    description,
    actions: Object.fromEntries(
      Object.entries(actions).map(([action, config]) => [
        action,
        { description: config.description, args: config.args || [] },
      ]),
    ),
  };
}

export function createUnknownActionError(resource, actions, action) {
  return new Error(
    `Unknown action: ${resource} ${action}\n\n` +
      'Available actions:\n' +
      Object.entries(actions)
        .map(([name, config]) => {
          const args =
            Array.isArray(config.args) && config.args.length > 0 ? ` ${config.args.join(' ')}` : '';
          return `  ${name}${args}`.padEnd(70) + config.description;
        })
        .join('\n'),
  );
}

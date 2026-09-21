import { createNativeToolkit, toCanonicalName } from './native-toolkit.mjs';

let createEmbeddedAgentToolkit = null;
let toolkitModuleLoadError = null;

try {
  ({ createEmbeddedAgentToolkit } = await import('./agent-toolkit.mjs'));
} catch (error) {
  toolkitModuleLoadError = error;
}

function isToolkit(value) {
  return Boolean(
    value &&
      typeof value === 'object' &&
      typeof value.getTools === 'function' &&
      typeof value.executeTool === 'function',
  );
}

/** Whether the optional `@stateset/cli` toolkit resolved at import time. */
export function isCliToolkitAvailable() {
  return typeof createEmbeddedAgentToolkit === 'function';
}

/** Why the CLI toolkit is unavailable, or `null` when it loaded. */
export function getCliToolkitLoadError() {
  return isCliToolkitAvailable() ? null : toolkitModuleLoadError;
}

/**
 * The backend `resolveToolkit` picks for a Commerce instance under
 * `backend: 'auto'`: the CLI toolkit when `@stateset/cli` is installed
 * (policy, budgets, replay, kernel governance), otherwise the native toolkit
 * shipped in this package.
 */
export function getDefaultToolkitBackend() {
  return isCliToolkitAvailable() ? 'cli' : 'native';
}

/**
 * Turn a Commerce instance (or pass through an existing toolkit) into a
 * toolkit. The result carries `backend: 'cli' | 'native'`.
 *
 * `backend` selects the implementation: `'auto'` (default) prefers the CLI
 * toolkit and falls back to native; `'cli'` requires the CLI toolkit and
 * rethrows its load error when absent; `'native'` always uses the built-in
 * toolkit, even when the CLI is installed.
 */
export function resolveToolkit(
  commerceOrToolkit,
  { allowApply = false, toolkitOptions = {}, backend = 'auto' } = {},
) {
  if (isToolkit(commerceOrToolkit)) {
    return commerceOrToolkit;
  }

  if (!commerceOrToolkit) {
    throw new Error('A Commerce instance or embedded toolkit is required.');
  }

  if (backend !== 'auto' && backend !== 'cli' && backend !== 'native') {
    throw new Error(`Unknown toolkit backend '${backend}'. Expected auto, cli or native.`);
  }

  const cliAvailable = isCliToolkitAvailable();
  if (backend === 'cli' && !cliAvailable) {
    throw toolkitModuleLoadError;
  }

  if (backend === 'native' || !cliAvailable) {
    const { filter = null } = toolkitOptions || {};
    return createNativeToolkit(commerceOrToolkit, { allowApply, filter });
  }

  const toolkit = createEmbeddedAgentToolkit({
    ...toolkitOptions,
    allowApply,
    commerce: commerceOrToolkit,
  });
  if (toolkit && typeof toolkit === 'object' && !('backend' in toolkit)) {
    toolkit.backend = 'cli';
  }
  return toolkit;
}

/**
 * Keep the items whose name is in `filter`. Names compare after `__` ↔ `.`
 * normalisation, so `orders.create` matches the wire-safe `orders__create`
 * the native toolkit emits for OpenAI/Anthropic/MCP formats.
 */
export function filterByToolName(items, filter, getName) {
  if (!Array.isArray(filter) || filter.length === 0) {
    return items;
  }

  const allowed = new Set(filter.map((name) => toCanonicalName(name)));
  return items.filter((item) => allowed.has(toCanonicalName(getName(item))));
}

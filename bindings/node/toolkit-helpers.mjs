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

export function resolveToolkit(
  commerceOrToolkit,
  { allowApply = false, toolkitOptions = {} } = {},
) {
  if (isToolkit(commerceOrToolkit)) {
    return commerceOrToolkit;
  }

  if (!commerceOrToolkit) {
    throw new Error('A Commerce instance or embedded toolkit is required.');
  }

  if (typeof createEmbeddedAgentToolkit !== 'function') {
    throw toolkitModuleLoadError;
  }

  return createEmbeddedAgentToolkit({
    ...toolkitOptions,
    allowApply,
    commerce: commerceOrToolkit,
  });
}

export function filterByToolName(items, filter, getName) {
  if (!Array.isArray(filter) || filter.length === 0) {
    return items;
  }

  const allowed = new Set(filter);
  return items.filter((item) => allowed.has(getName(item)));
}

// Deliberately dependency-free: this entrypoint must be safe to bundle in a browser.
export function getWebMCPContext() {
  for (const context of [globalThis.document?.modelContext, globalThis.navigator?.modelContext]) {
    if (typeof context?.registerTool === 'function') return context;
  }
  return null;
}

export function createWebMCPTools(
  descriptors,
  { filter = null, allowApply = false, executeTool } = {},
) {
  if (!Array.isArray(descriptors)) {
    throw new TypeError('WebMCP requires an array of StateSet tool descriptors.');
  }
  if (filter !== null && !Array.isArray(filter)) {
    throw new TypeError('filter must be an array of tool names or null.');
  }
  if (executeTool !== undefined && typeof executeTool !== 'function') {
    throw new TypeError('executeTool must be a function.');
  }
  const normalize = (name) => {
    if (typeof name !== 'string' || !name) {
      throw new TypeError('WebMCP tool names must be nonempty strings.');
    }
    return name.replace(/__/g, '.');
  };
  const allowed = filter === null ? null : new Set(filter.map(normalize));
  const names = new Set();
  return descriptors
    .filter((descriptor) => {
      if (!descriptor || typeof descriptor !== 'object' || Array.isArray(descriptor)) {
        throw new TypeError('WebMCP descriptors must be objects.');
      }
      const name = normalize(descriptor.name);
      return !allowed || allowed.has(name);
    })
    .map((descriptor) => {
      const { name, description, inputSchema, permission } = descriptor;
      if (typeof name !== 'string' || !name || names.has(name)) {
        throw new TypeError('WebMCP tool names must be nonempty and unique.');
      }
      names.add(name);
      if (typeof description !== 'string' || !description) {
        throw new TypeError(`WebMCP tool '${name}' requires a description.`);
      }
      if (!inputSchema || typeof inputSchema !== 'object' || Array.isArray(inputSchema)) {
        throw new TypeError(`WebMCP tool '${name}' requires a JSON Schema inputSchema.`);
      }
      if (!executeTool && typeof descriptor.execute !== 'function') {
        throw new TypeError(`WebMCP tool '${name}' requires an executor.`);
      }
      // Unknown permissions fail closed. Hints supplied by callers cannot grant write access.
      const readOnly = permission === 'read' && descriptor.readOnly !== false;
      return {
        name,
        description,
        inputSchema,
        annotations: {
          readOnlyHint: readOnly,
          consequentialHint: !readOnly && allowApply === true,
        },
        async execute(params = {}, options = {}) {
          options.signal?.throwIfAborted();
          if (!params || typeof params !== 'object' || Array.isArray(params)) {
            throw new TypeError('WebMCP tool arguments must be an object.');
          }
          if (!readOnly && allowApply !== true) {
            return {
              preview: true,
              tool: name,
              params,
              note: 'Write not executed. Enable allowApply in trusted host configuration to apply mutations.',
            };
          }
          const result = executeTool
            ? await executeTool(name, params, options)
            : await descriptor.execute(params, options);
          options.signal?.throwIfAborted();
          return result;
        },
      };
    });
}

export async function registerWebMCPTools(
  descriptors,
  { modelContext = getWebMCPContext(), signal, ...options } = {},
) {
  signal?.throwIfAborted();
  const tools = createWebMCPTools(descriptors, options);
  const controller = new AbortController();
  const registeredNames = new Set();
  const unregisterLegacyTool = (name) => {
    // Early navigator.modelContext implementations ignore registration signals.
    if (typeof modelContext?.unregisterTool === 'function') modelContext.unregisterTool(name);
  };
  let disposed = false;
  const dispose = () => {
    if (disposed) return;
    disposed = true;
    try {
      for (const name of registeredNames) unregisterLegacyTool(name);
    } finally {
      controller.abort();
      signal?.removeEventListener('abort', dispose);
    }
  };
  const supported = typeof modelContext?.registerTool === 'function';
  if (!supported) return { supported: false, toolNames: [], dispose };
  signal?.addEventListener('abort', dispose, { once: true });
  try {
    for (const tool of tools) {
      controller.signal.throwIfAborted();
      await modelContext.registerTool(tool, { signal: controller.signal });
      registeredNames.add(tool.name);
      // The host may abort while an asynchronous legacy registration is pending.
      if (disposed) unregisterLegacyTool(tool.name);
    }
    controller.signal.throwIfAborted();
  } catch (error) {
    // The registration signal removes only our tools, including partial registrations.
    dispose();
    throw error;
  }
  return { supported: true, toolNames: tools.map((tool) => tool.name), dispose };
}

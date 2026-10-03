// Deliberately dependency-free: this entrypoint must be safe to bundle in a browser.
export function getWebMCPContext() {
  for (const context of [globalThis.document?.modelContext, globalThis.navigator?.modelContext]) {
    if (typeof context?.registerTool === 'function') return context;
  }
  return null;
}

export function createWebMCPTools(
  descriptors,
  { filter = null, allowApply = false, executeTool, requireConfirmation = null, onConfirm } = {},
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
  if (
    requireConfirmation !== null &&
    requireConfirmation !== undefined &&
    !Array.isArray(requireConfirmation) &&
    typeof requireConfirmation !== 'function'
  ) {
    throw new TypeError(
      'requireConfirmation must be an array of tool names, a predicate function, or null.',
    );
  }
  if (Array.isArray(requireConfirmation)) {
    for (const entry of requireConfirmation) {
      if (typeof entry !== 'string' || !entry) {
        throw new TypeError('requireConfirmation tool names must be nonempty strings.');
      }
    }
  }
  if (onConfirm !== undefined && typeof onConfirm !== 'function') {
    throw new TypeError('onConfirm must be a function.');
  }
  const normalize = (name) => {
    if (typeof name !== 'string' || !name) {
      throw new TypeError('WebMCP tool names must be nonempty strings.');
    }
    return name.replace(/__/g, '.');
  };
  const allowed = filter === null ? null : new Set(filter.map(normalize));
  // Buyer confirmation: an extra, human-in-the-loop gate for consequential
  // actions (typically the money move, e.g. carts.complete). Array entries
  // accept canonical (`carts.complete`) or wire (`carts__complete`) names.
  // A predicate receives (toolName, params) and gates on any truthy return;
  // if it throws, the call fails without executing. Approval is strict: only
  // an onConfirm return value of exactly `true` proceeds. Without onConfirm,
  // gated tools fail closed with a confirmationRequired preview.
  const confirmationNames =
    Array.isArray(requireConfirmation) && requireConfirmation.length > 0
      ? new Set(requireConfirmation.map(normalize))
      : null;
  const confirmationPredicate =
    typeof requireConfirmation === 'function' ? requireConfirmation : null;
  const needsConfirmation = (toolName, params) => {
    if (confirmationNames !== null && confirmationNames.has(normalize(toolName))) return true;
    if (confirmationPredicate !== null) return Boolean(confirmationPredicate(toolName, params));
    return false;
  };
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
      // The static hint evaluates a predicate once with empty params; a
      // throwing predicate is treated as "no confirmation" for the hint only.
      // Param-dependent predicates still gate at execution time.
      let confirmationHint = confirmationNames !== null && confirmationNames.has(normalize(name));
      if (!confirmationHint && confirmationPredicate !== null) {
        try {
          confirmationHint = Boolean(confirmationPredicate(name, {}));
        } catch {
          confirmationHint = false;
        }
      }
      return {
        name,
        description,
        inputSchema,
        annotations: {
          readOnlyHint: readOnly,
          consequentialHint: (!readOnly && allowApply === true) || confirmationHint,
        },
        async execute(params = {}, options = {}) {
          options.signal?.throwIfAborted();
          if (!params || typeof params !== 'object' || Array.isArray(params)) {
            throw new TypeError('WebMCP tool arguments must be an object.');
          }
          if (needsConfirmation(name, params)) {
            if (typeof onConfirm !== 'function') {
              return {
                preview: true,
                confirmationRequired: true,
                tool: name,
                params,
                note: 'Buyer confirmation is required before this action can run. Provide onConfirm in trusted host configuration to request approval.',
              };
            }
            options.signal?.throwIfAborted();
            const approved = await onConfirm({ tool: name, params, options });
            options.signal?.throwIfAborted();
            if (approved !== true) {
              return {
                confirmed: false,
                tool: name,
                params,
                note: 'The buyer did not confirm this action. Nothing was executed.',
              };
            }
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

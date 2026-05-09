// Pure helpers used by the MCP policy gate.
//
// Extracted from mcp-server.js. None close over runtime state — they
// operate on their arguments only, which makes them trivially testable.

/**
 * Strip the MCP namespace prefix from a tool name.
 *
 * The Claude Agent SDK reports tool names as `mcp__<server>__<tool>`
 * (e.g. `mcp__stateset__create_order`). For policy lookup, telemetry,
 * and matching against `ALL_TOOL_DEFS`, we want the bare tool name
 * (`create_order`).
 *
 * Returns an empty string for non-string / null / undefined input so
 * callers can rely on `.length` and string operators downstream.
 *
 * @param {unknown} toolName
 * @returns {string}
 */
export function normalizeToolName(toolName) {
  if (!toolName || typeof toolName !== 'string') return '';
  return toolName.trim().replace(/^mcp__[a-z0-9_-]+__/, '');
}

/**
 * Apply a policy-engine transform to an input object.
 *
 * The policy engine can return a `transform` object that overrides or
 * augments fields on the inbound parameter object before the tool runs
 * (e.g. to inject a `customerId` from the calling agent's identity, or
 * to lock a `chain` field to a specific blockchain).
 *
 * Behavior:
 *   - non-object/array transforms are no-ops (return input unchanged)
 *   - for each key in the transform:
 *     - if the existing field is a non-array object AND the transform
 *       value is also a non-array object → shallow merge `{...existing, ...transform}`
 *     - otherwise → replace
 *   - every change appends a `{field, before, after, timestamp}` audit
 *     entry to `auditEntries` (caller-provided array; mutated in place)
 *
 * Returns `{output, auditEntries}` so callers can either persist the
 * audit trail or discard it.
 *
 * @param {Record<string, unknown>|null|undefined} input
 * @param {Record<string, unknown>|null|undefined} transform
 * @param {Array<{field: string, before: unknown, after: unknown, timestamp: string}>} [auditEntries]
 * @returns {{output: Record<string, unknown>, auditEntries: Array}}
 */
export function applyPolicyTransform(input, transform, auditEntries = []) {
  if (!transform || typeof transform !== 'object' || Array.isArray(transform)) {
    return { output: input, auditEntries };
  }

  const output = { ...(input || {}) };
  for (const [key, value] of Object.entries(transform)) {
    const before = output[key];
    if (
      output[key] !== null &&
      output[key] !== undefined &&
      typeof output[key] === 'object' &&
      !Array.isArray(output[key]) &&
      value &&
      typeof value === 'object' &&
      !Array.isArray(value)
    ) {
      output[key] = { ...output[key], ...value };
    } else {
      output[key] = value;
    }
    auditEntries.push({
      field: key,
      before,
      after: output[key],
      timestamp: new Date().toISOString(),
    });
  }

  return { output, auditEntries };
}

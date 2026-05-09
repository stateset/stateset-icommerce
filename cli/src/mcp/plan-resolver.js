// Agentic-plan parameter resolution.
//
// MCP plans submitted by agents can reference *prior step output* using
// template syntax like `{{ steps.0.result.orderId }}`. This module is the
// engine that walks the parameter tree, finds those templates, looks them up
// in a step-history context, and substitutes the values in.
//
// Path forms supported by the resolver:
//
//   {{ steps.<idx>.<path...> }}    — output of an indexed step
//   {{ latest.<path...> }}         — most recent step's output
//   {{ tool.<name>.<path...> }}    — most recent invocation of a named tool
//   {{ sla.<path...> }}            — SLA context block
//   {{ slaLevel }}                 — shorthand for sla.level
//
// Bracket index syntax (`foo[0].bar`) is normalized to dot-form internally.
// Anything that resolves to `undefined` is reported in the `unresolved` list
// returned by `resolveAgenticPlanValue` so the caller can surface a helpful
// error to the agent.
//
// Extracted from `cli/src/mcp-server.js` to keep that file focused on
// orchestration. Note: `buildPlanStepRouting` stays in mcp-server.js because
// it depends on the runtime-injected agent router; only the pure resolution
// helpers live here.

/** Maximum step count per plan; enforced by callers. */
export const MAX_PLAN_STEPS = 200;

/** Template form for a parameter that should be replaced by a path lookup. */
export const AGENTIC_PLAN_PARAM_TEMPLATE = /^\{\{\s*([^}]+)\s*\}\}$/;

/** SLA levels accepted by the plan executor. */
export const AGENTIC_SLA_LEVELS = ['standard', 'expedited', 'critical'];

/**
 * Coerce an SLA level string to its canonical lowercase form, or null if
 * unrecognised.
 */
export const normalizeSlaLevel = (value) => {
  if (typeof value !== 'string') return null;
  const normalized = value.trim().toLowerCase();
  return AGENTIC_SLA_LEVELS.includes(normalized) ? normalized : null;
};

/**
 * Walk `pathSegments` through `value`, returning the deepest reached leaf or
 * `undefined` if the path bottoms out on a non-object/non-array.
 */
export const getByPath = (value, pathSegments) => {
  let current = value;
  for (const segment of pathSegments) {
    if (current === null || current === undefined) return undefined;
    if (typeof current === 'object' || Array.isArray(current)) {
      current = current?.[segment];
      continue;
    }
    return undefined;
  }
  return current;
};

/**
 * Resolve a single template path string against a plan context.
 *
 * @param {Object} context - { steps, latest, byTool, sla }
 * @param {string} rawPath - e.g. "steps.0.result.orderId" or "latest.id".
 * @returns {*} the resolved value, or `undefined` if any segment misses.
 */
export const resolveAgenticPlanPath = (context, rawPath) => {
  if (!context || typeof rawPath !== 'string') return undefined;
  const pathExpression = rawPath.trim().replace(/\[(\d+)\]/g, '.$1');
  const pathParts = pathExpression.split('.').filter(Boolean);
  if (!pathParts.length) return undefined;

  if (pathParts[0] === 'steps') {
    if (pathParts.length < 2) return undefined;
    const stepIndex = Number(pathParts[1]);
    if (!Number.isInteger(stepIndex) || stepIndex < 0) return undefined;
    return getByPath(context.steps?.[stepIndex], pathParts.slice(2));
  }

  if (pathParts[0] === 'latest') {
    return getByPath(context.latest, pathParts.slice(1));
  }

  if (pathParts[0] === 'tool') {
    if (pathParts.length < 2) return undefined;
    return getByPath(context.byTool?.[pathParts[1]], pathParts.slice(2));
  }

  if (pathParts[0] === 'sla') {
    return getByPath(context.sla, pathParts.slice(1));
  }

  if (pathParts[0] === 'slaLevel') {
    return context.sla?.level;
  }

  return undefined;
};

/**
 * Recursively resolve a plan value tree.
 *
 * Strings matching the `{{ … }}` template are replaced with the looked-up
 * value (or `null` + an entry in `unresolved` if the lookup misses). Arrays
 * and plain objects are recursed into; Date/Buffer/Map/Set are passed through
 * unchanged.
 *
 * @param {*} value
 * @param {Object} context - same shape as `resolveAgenticPlanPath`.
 * @param {string} [location='$'] - JSONPath-ish breadcrumb for error reporting.
 * @returns {{value: *, unresolved: string[]}}
 */
export const resolveAgenticPlanValue = (value, context, location = '$') => {
  if (value === null || value === undefined) return { value, unresolved: [] };
  if (typeof value === 'string') {
    const match = value.match(AGENTIC_PLAN_PARAM_TEMPLATE);
    if (!match) return { value, unresolved: [] };

    const resolved = resolveAgenticPlanPath(context, match[1]);
    if (resolved === undefined) {
      return {
        value: null,
        unresolved: [`${location} -> ${match[1]}`],
      };
    }

    return { value: resolved, unresolved: [] };
  }

  if (typeof value !== 'object') return { value, unresolved: [] };
  if (
    value instanceof Date ||
    Buffer.isBuffer(value) ||
    value instanceof Map ||
    value instanceof Set
  ) {
    return { value, unresolved: [] };
  }

  if (Array.isArray(value)) {
    const output = [];
    const unresolved = [];
    for (let i = 0; i < value.length; i += 1) {
      const child = resolveAgenticPlanValue(value[i], context, `${location}[${i}]`);
      output.push(child.value);
      unresolved.push(...child.unresolved);
    }
    return { value: output, unresolved };
  }

  const output = {};
  const unresolved = [];
  for (const [key, childValue] of Object.entries(value)) {
    const child = resolveAgenticPlanValue(childValue, context, `${location}.${key}`);
    output[key] = child.value;
    unresolved.push(...child.unresolved);
  }

  return { value: output, unresolved };
};

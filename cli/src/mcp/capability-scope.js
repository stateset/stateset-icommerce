// Tool capability scope for the embedded agent toolkit.
//
// `createEmbeddedAgentToolkit({ capabilities })` narrows the tools an agent
// may reach. The scope used to be enforced only at the toolkit boundary (the
// listing and the top-level tool name), while composite tools such as
// `agentic_execute_plan` resolved their nested tools against the server's
// whole profile. `createStatesetMcpServer({ toolCapabilities })` now applies
// the scope where it builds its exposed tool map, so the listing, direct
// calls, nested plan steps, simulations, replay and rollback compensations
// all resolve against the same filtered map.
//
// This module lives outside `src/tools/`, where every .js file is treated as
// a tool module.

import { KERNEL_CAPABILITY_BY_TOOL } from '../kernel-tool-execution.js';
import { normalizeToolName } from './policy-helpers.js';

/** Refusal reason carried by tools a capability scope excludes. */
export const OUTSIDE_CAPABILITY_SCOPE = 'outside_capability_scope';

/** @param {string} toolName */
export function outsideCapabilityScopeMessage(toolName) {
  return `Tool '${toolName}' is outside this toolkit's capability scope.`;
}

/**
 * Normalize a capability list to the Set the scope checks, or `null` when no
 * scope applies and every tool is admitted.
 *
 * @param {Iterable<unknown> | null | undefined} capabilities
 * @returns {Set<string> | null}
 */
export function normalizeToolCapabilities(capabilities) {
  if (capabilities === null || capabilities === undefined) return null;
  if (!Array.isArray(capabilities) && !(capabilities instanceof Set)) {
    throw new TypeError('toolCapabilities must be an array or a Set of capability strings.');
  }
  return new Set([...capabilities].map((capability) => String(capability).trim()).filter(Boolean));
}

/**
 * Whether a raw tool definition is inside `capabilities`. A `null` scope
 * admits every tool. A capability admits a tool when it is `*`, the tool's
 * name, `permission:<permission>`, `read:*` (read tools only),
 * `<policyDomain>.*`, or the tool's kernel command capability.
 *
 * @param {{ name?: string, permission?: string, policyDomain?: string } | null | undefined} tool
 * @param {Set<string> | null} capabilities
 */
export function capabilityAllowsTool(tool, capabilities) {
  if (!capabilities) return true;
  const name = normalizeToolName(tool?.name);
  const permission = String(tool?.permission || 'read');
  const domain = String(tool?.policyDomain || '').trim();
  return (
    capabilities.has('*') ||
    capabilities.has(name) ||
    capabilities.has(`permission:${permission}`) ||
    (permission === 'read' && capabilities.has('read:*')) ||
    (domain && capabilities.has(`${domain}.*`)) ||
    capabilities.has(KERNEL_CAPABILITY_BY_TOOL[name])
  );
}

/**
 * Split a server's tool definitions into the ones a capability scope admits
 * and a lookup describing the ones it excludes.
 *
 * Each definition is judged as the server's raw listing describes it
 * (`getRawToolDefinitions`): a missing permission reads as `unknown`, and the
 * policy domain falls back to the registry, then to inference. That is the
 * view the toolkit has always filtered, so the scope admits the same tools.
 *
 * @param {{
 *   toolDefs: Array<{ name: string, permission?: string, policyDomain?: string }>,
 *   capabilities?: Iterable<unknown> | null,
 *   toolDomainByName?: Record<string, string>,
 *   inferPolicyDomain?: (toolName: string) => string,
 * }} input
 * @returns {{
 *   toolDefs: Array<object>,
 *   describeExcludedTool: (toolName: string) =>
 *     { reason: string, message: string, sideEffect: 'read' | 'write' } | null,
 * }}
 */
export function applyToolCapabilityScope({
  toolDefs,
  capabilities = null,
  toolDomainByName = {},
  inferPolicyDomain = () => '',
}) {
  const scope = normalizeToolCapabilities(capabilities);
  if (!scope) {
    return { toolDefs, describeExcludedTool: () => null };
  }

  const excluded = new Map();
  const admitted = toolDefs.filter((toolDef) => {
    const allowed = capabilityAllowsTool(
      {
        name: toolDef.name,
        permission: toolDef.permission || 'unknown',
        policyDomain:
          toolDef.policyDomain || toolDomainByName[toolDef.name] || inferPolicyDomain(toolDef.name),
      },
      scope,
    );
    if (!allowed && toolDef?.name) excluded.set(toolDef.name, toolDef);
    return allowed;
  });

  const describeExcludedTool = (toolName) => {
    const toolDef = excluded.get(normalizeToolName(toolName));
    if (!toolDef) return null;
    return {
      reason: OUTSIDE_CAPABILITY_SCOPE,
      message: outsideCapabilityScopeMessage(toolDef.name),
      sideEffect: toolDef.permission === 'read' ? 'read' : 'write',
    };
  };

  return { toolDefs: admitted, describeExcludedTool };
}

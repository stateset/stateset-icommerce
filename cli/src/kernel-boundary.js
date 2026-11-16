import { createHash } from 'node:crypto';

import { KERNEL_CAPABILITY_BY_TOOL } from './kernel-tool-execution.js';

export const KERNEL_BOUNDARY_SCHEMA_VERSION = 1;

// These tools may orchestrate mutations, but never perform one directly. Keep
// this allow-list deliberately small and explicit so an orchestration surface
// cannot masquerade as a read merely to remain discoverable.
//
// In strict mode a composite runs only as a top-level call, and only the
// composite itself is exempt from the typed-command requirement. Every step it
// runs goes back through the step executor (src/mcp/plan-step-executor.js):
// a write the strict boundary withholds is refused before its handler runs, a
// composite nested inside another composite is refused, and every other live
// write must cross executeGovernedTool as a typed kernel command under the
// operator's principal and policy.
//
// agentic_replay_mutation re-executes one logged write through the same step
// executor (src/mcp/mutation-simulator.js); it refuses admin and delete tools,
// and in strict mode it replays governed commands only.
export const KERNEL_GOVERNED_COMPOSITE_TOOLS = Object.freeze([
  'agentic_execute_plan',
  'agentic_replay_mutation',
]);
const KERNEL_GOVERNED_COMPOSITE_TOOL_SET = new Set(KERNEL_GOVERNED_COMPOSITE_TOOLS);

/** Whether a tool is on the governed composite allow-list. */
export function isKernelGovernedComposite(toolName) {
  return KERNEL_GOVERNED_COMPOSITE_TOOL_SET.has(toolName);
}

/**
 * Kernel strict mode is intentionally fail closed. Only an explicit `read`
 * permission is non-mutating; write, delete, admin, missing, and future
 * permission classes all require a governed command mapping.
 */
export function isMutationPermission(permission) {
  return permission !== 'read';
}

export function isMutationToolDefinition(tool) {
  return isMutationPermission(tool?.permission);
}

export function classifyKernelToolBoundary(
  toolDefs,
  capabilityByTool = KERNEL_CAPABILITY_BY_TOOL,
  governedCompositeTools = KERNEL_GOVERNED_COMPOSITE_TOOL_SET,
) {
  const seen = new Set();
  const entries = [];

  for (const tool of toolDefs || []) {
    const name = String(tool?.name || '').trim();
    if (!name) throw new Error('Kernel boundary cannot classify an unnamed tool.');
    if (seen.has(name)) throw new Error(`Kernel boundary found duplicate tool '${name}'.`);
    seen.add(name);

    const permission = String(tool?.permission || 'unknown');
    const mutation = isMutationPermission(permission);
    const commandType = capabilityByTool[name] || null;
    const governedComposite = governedCompositeTools.has(name);
    entries.push({
      name,
      permission,
      mutation,
      disposition: mutation
        ? commandType
          ? 'governed'
          : governedComposite
            ? 'governed_composite'
            : 'blocked'
        : 'read_only',
      commandType,
    });
  }

  entries.sort((left, right) => left.name.localeCompare(right.name));
  const encoded = JSON.stringify(entries);
  const counts = {
    total: entries.length,
    readOnly: entries.filter((entry) => entry.disposition === 'read_only').length,
    mutations: entries.filter((entry) => entry.mutation).length,
    governed: entries.filter((entry) => entry.disposition === 'governed').length,
    governedComposite: entries.filter((entry) => entry.disposition === 'governed_composite').length,
    blocked: entries.filter((entry) => entry.disposition === 'blocked').length,
  };

  return {
    schemaVersion: KERNEL_BOUNDARY_SCHEMA_VERSION,
    digest: `sha256:${createHash('sha256').update(encoded).digest('hex')}`,
    counts,
    entries,
  };
}

export function selectStrictKernelToolDefinitions(
  toolDefs,
  capabilityByTool = KERNEL_CAPABILITY_BY_TOOL,
  governedCompositeTools = KERNEL_GOVERNED_COMPOSITE_TOOL_SET,
) {
  return (toolDefs || []).filter(
    (tool) =>
      !isMutationToolDefinition(tool) ||
      Boolean(capabilityByTool[tool.name]) ||
      governedCompositeTools.has(tool.name),
  );
}

/**
 * Names of the tools strict mode withholds: the mutations with neither a typed
 * command nor a place on the composite allow-list. The exact complement of
 * `selectStrictKernelToolDefinitions` over the same definitions.
 */
export function selectStrictKernelWithheldToolNames(
  toolDefs,
  capabilityByTool = KERNEL_CAPABILITY_BY_TOOL,
  governedCompositeTools = KERNEL_GOVERNED_COMPOSITE_TOOL_SET,
) {
  const exposed = new Set(
    selectStrictKernelToolDefinitions(toolDefs, capabilityByTool, governedCompositeTools).map(
      (tool) => tool.name,
    ),
  );
  return new Set(
    (toolDefs || []).map((tool) => tool?.name).filter((name) => name && !exposed.has(name)),
  );
}

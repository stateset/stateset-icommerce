import { createHash } from 'node:crypto';

import { KERNEL_CAPABILITY_BY_TOOL } from './kernel-tool-execution.js';

export const KERNEL_BOUNDARY_SCHEMA_VERSION = 1;

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

export function classifyKernelToolBoundary(toolDefs, capabilityByTool = KERNEL_CAPABILITY_BY_TOOL) {
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
    entries.push({
      name,
      permission,
      mutation,
      disposition: mutation ? (commandType ? 'governed' : 'blocked') : 'read_only',
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
) {
  return (toolDefs || []).filter(
    (tool) => !isMutationToolDefinition(tool) || Boolean(capabilityByTool[tool.name]),
  );
}

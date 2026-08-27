import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

function readJsonFile(filePath, label) {
  const resolved = resolve(filePath);
  let value;
  try {
    value = JSON.parse(readFileSync(resolved, 'utf8'));
  } catch (error) {
    throw new Error(`Unable to load ${label} JSON from ${resolved}: ${error.message}`);
  }
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} JSON must contain an object.`);
  }
  return value;
}

/**
 * Load trusted kernel configuration from operator-controlled files.
 * No policy, principal, or store scope is accepted from model tool arguments.
 */
export function loadKernelConfig({
  policyPath,
  principalPath,
  storeId,
  allowLegacyWrites = false,
  requireForApply = false,
  env = process.env,
} = {}) {
  const effectivePolicyPath = policyPath || env.STATESET_KERNEL_POLICY || null;
  const effectivePrincipalPath = principalPath || env.STATESET_KERNEL_PRINCIPAL || null;
  const effectiveStoreId = storeId || env.STATESET_KERNEL_STORE_ID || null;
  const configured = Boolean(effectivePolicyPath || effectivePrincipalPath || effectiveStoreId);
  if (!configured) {
    if (requireForApply && !allowLegacyWrites) {
      throw new Error(
        'Apply mode requires trusted kernel configuration. Provide --kernel-policy, ' +
          '--kernel-principal, and --kernel-store-id, or explicitly use ' +
          '--kernel-allow-legacy-writes for a controlled migration.',
      );
    }
    return null;
  }

  const missing = [];
  if (!effectivePolicyPath) missing.push('--kernel-policy');
  if (!effectivePrincipalPath) missing.push('--kernel-principal');
  if (!effectiveStoreId) missing.push('--kernel-store-id');
  if (missing.length > 0) {
    throw new Error(`Incomplete trusted kernel configuration; missing ${missing.join(', ')}.`);
  }

  const policy = readJsonFile(effectivePolicyPath, 'kernel policy');
  const principal = readJsonFile(effectivePrincipalPath, 'kernel principal');
  if (typeof policy.version !== 'string' || !policy.version.trim()) {
    throw new Error('Kernel policy requires a non-empty version.');
  }
  if (!policy.commands || typeof policy.commands !== 'object' || Array.isArray(policy.commands)) {
    throw new Error('Kernel policy requires a commands object.');
  }
  if (typeof principal.id !== 'string' || !principal.id.trim()) {
    throw new Error('Kernel principal requires a non-empty id.');
  }
  if (!Array.isArray(principal.capabilities)) {
    throw new Error('Kernel principal requires a capabilities array.');
  }

  return {
    policy,
    principal,
    storeId: String(effectiveStoreId),
    strict: !allowLegacyWrites,
  };
}

export default { loadKernelConfig };

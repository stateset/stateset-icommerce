// Deterministic mutation manifest construction for the MCP orchestrator.
//
// When a write tool runs, the orchestrator emits a mutation manifest:
// a content-addressed envelope that's safe to persist in audit/replay
// logs and uniquely identifies the (tool, params, policy, permission)
// quadruple. The manifest gives downstream consumers (replay tools,
// idempotency caches, compensation engines) a stable handle.
//
// Read-only tools and unknown-permission tools deliberately return
// `null` — manifests are a write-side construct.
//
// Extracted from mcp-server.js. All exports are pure (no closure
// state). The `replayEventHash` and `buildRollbackContract` helpers
// from `./audit-envelope.js` provide the content-addressing primitives.

import { buildRollbackContract, replayEventHash } from './audit-envelope.js';

/**
 * Parameter keys that, when present as a non-empty string, count as a
 * caller-provided idempotency key. Order matters: callers using the
 * earlier names take precedence over later ones if multiple are set.
 *
 * Both camelCase (`idempotencyKey`) and snake_case (`idempotency_key`)
 * shapes are accepted because some bindings auto-snake-case payloads
 * and we don't want to surprise callers depending on the convention.
 */
const IDEMPOTENCY_KEY_CANDIDATES = Object.freeze([
  'idempotencyKey',
  'idempotency_key',
  'idempotencyToken',
  'requestId',
  'request_id',
  'externalId',
  'external_id',
]);

/**
 * Look up a caller-provided idempotency key in `params`. Returns the
 * trimmed string when one of the candidate keys carries a non-empty
 * string value, else `null`.
 *
 * Non-object/array `params` (and missing) all yield `null` — there's
 * nowhere to find a key in those shapes.
 *
 * @param {unknown} [params={}]
 * @returns {string | null}
 */
export const extractIdempotencyKeyFromParams = (params = {}) => {
  if (!params || typeof params !== 'object' || Array.isArray(params)) return null;
  for (const key of IDEMPOTENCY_KEY_CANDIDATES) {
    const value = params[key];
    if (typeof value === 'string' && value.trim()) return value.trim();
  }
  return null;
};

/**
 * Build a deterministic mutation manifest for a tool invocation.
 *
 * Returns `null` when the tool is read-only or has unknown permission
 * metadata — manifests are emitted only for known mutations.
 *
 * The manifest's `deterministicSignature` is a content-addressed hash
 * over the core fields, so identical (tool, params, policy, permission,
 * phase, sideEffect, idempotent, rollbackContractHash) tuples produce
 * identical signatures. Replay engines use this to detect duplicate
 * intents and short-circuit re-emission.
 *
 * Idempotency keys come from one of:
 *   1. Caller-provided (one of the seven recognized param key names)
 *   2. Generated `ik_<tool>_<paramsHash[:16]>` when the tool is marked
 *      idempotent in its runtime metadata
 *   3. `null` for non-idempotent writes (caller must enforce uniqueness
 *      via a higher-level mechanism)
 *
 * @param {object} [args]
 * @param {string} [args.toolName]
 * @param {object} [args.params={}]
 * @param {object|null} [args.policy=null]
 * @param {object|null} [args.permission=null]
 * @param {{
 *   sideEffect?: 'read'|'write'|'delete',
 *   permission?: string,
 *   policyDomain?: string,
 *   idempotent?: boolean,
 *   compensations?: Array<string>,
 * }|null} [args.runtimeMeta=null]
 * @param {string} [args.phase='execute']
 * @returns {object|null} the manifest, or null for read/unknown tools
 */
export const buildDeterministicMutationManifest = ({
  toolName,
  params = {},
  policy = null,
  permission = null,
  runtimeMeta = null,
  phase = 'execute',
} = {}) => {
  if (!runtimeMeta || runtimeMeta.sideEffect === 'read' || runtimeMeta.permission === 'unknown') {
    return null;
  }

  const paramsHash = replayEventHash(params || {});
  const policyHash = replayEventHash(policy || {});
  const permissionHash = replayEventHash(permission || {});
  const idempotencyKey =
    extractIdempotencyKeyFromParams(params) ||
    (runtimeMeta.idempotent ? `ik_${toolName}_${paramsHash.slice(0, 16)}` : null);
  const rollback = buildRollbackContract(toolName);
  const core = {
    version: '1.0.0',
    tool: toolName,
    phase,
    sideEffect: runtimeMeta.sideEffect,
    policyDomain: runtimeMeta.policyDomain || null,
    idempotent: Boolean(runtimeMeta.idempotent),
    idempotencyKey,
    paramsHash,
    policyHash,
    permissionHash,
    rollbackContractHash: rollback.contractHash,
    compensationTools: runtimeMeta.compensations || [],
  };

  return {
    ...core,
    deterministicSignature: replayEventHash(core),
    rollback,
  };
};

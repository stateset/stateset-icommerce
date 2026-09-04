// Audit-envelope construction helpers for the MCP orchestrator.
//
// Every agentic plan step emits an audit envelope that includes:
//   - hashes of the params/policy/permission/result objects
//     (`replayEventHash`)
//   - normalized policy actions + explanations (so downstream code
//     doesn't have to handle `toJSON()`-bearing class instances)
//   - a rollback contract describing how to reverse the step if it
//     fails after partial commit (`buildRollbackContract`)
//   - any approval stages required by the policy actions
//     (`buildApprovalStagesFromActions`)
//
// All exports are pure — no closure or runtime deps. The compensation
// lookup tables are imported from `./compensation.js`, and the hash
// helpers from `./replay-sanitizer.js`.

import { AGENTIC_COMPENSATION_HINTS, AGENTIC_COMPENSATION_PARAM_HINTS } from './compensation.js';
import { compactReplayValue, sha256, stableStringify } from './replay-sanitizer.js';

/**
 * Hash a value the same way the orchestrator's audit replay log does:
 * compact + canonicalize + sha256. Used as a content-addressing primitive
 * across params/policy/permission/result envelopes.
 *
 * @param {unknown} value
 * @returns {string} 64-char hex SHA-256
 */
export const replayEventHash = (value) => sha256(stableStringify(compactReplayValue(value)));

export const COMMERCE_EXECUTION_EVIDENCE_VERSION = 'stateset.commerce-evidence.v1';
export const COMMERCE_EXECUTION_META_KEY = 'com.stateset/commerce';

/**
 * Project a replay-log event into the privacy-minimal evidence carried across
 * the MCP boundary. Payloads stay in the local audit store; only their hashes
 * and correlation identifiers leave the commerce process.
 *
 * @param {object} event
 * @returns {object}
 */
export const buildCommerceExecutionEvidence = (event) => ({
  version: COMMERCE_EXECUTION_EVIDENCE_VERSION,
  event_id: event.eventId,
  event_type: 'McpToolExecution',
  occurred_at: event.occurredAt,
  tool: event.tool,
  status: event.status,
  request_id: event.requestId ?? null,
  session_id: event.sessionId ?? null,
  params_sha256: `sha256:${event.paramsHash || replayEventHash(event.params || {})}`,
  result_sha256: `sha256:${event.resultHash || replayEventHash(event.result || {})}`,
  policy_sha256: `sha256:${replayEventHash(event.policy || {})}`,
  permission_sha256: `sha256:${replayEventHash(event.permission || {})}`,
  mutation_manifest_sha256: `sha256:${replayEventHash(event.notes?.mutationManifest || {})}`,
});

/** Attach commerce evidence without disturbing existing MCP result metadata. */
export const attachCommerceExecutionEvidence = (response, event) => {
  if (!response || typeof response !== 'object' || Array.isArray(response)) return response;
  return {
    ...response,
    _meta: {
      ...(response._meta && typeof response._meta === 'object' && !Array.isArray(response._meta)
        ? response._meta
        : {}),
      [COMMERCE_EXECUTION_META_KEY]: buildCommerceExecutionEvidence(event),
    },
  };
};

/**
 * Normalize a policy action into a plain JSON-friendly object.
 *
 * Policy actions can arrive as:
 *   - plain objects (returned as-is)
 *   - class instances with a `toJSON()` method (called and result returned)
 *   - arrays / primitives (returned as `null` because they can't carry
 *     an action's structured fields)
 *   - null/undefined (returned as `null`)
 *
 * Throws from `toJSON()` are swallowed and yield `null` so a single
 * malformed action can't sink an entire plan-step audit envelope.
 *
 * @param {unknown} action
 * @returns {object|null}
 */
export const normalizePolicyAction = (action) => {
  if (!action) return null;
  if (typeof action?.toJSON === 'function') {
    try {
      return action.toJSON();
    } catch {
      return null;
    }
  }
  if (typeof action !== 'object' || Array.isArray(action)) return null;
  return action;
};

/**
 * Normalize a policy explanation. Same shape and rules as
 * `normalizePolicyAction` — split for clarity since the two are
 * distinct concepts at the policy-engine level even though they share
 * the same canonicalization logic today.
 *
 * @param {unknown} explanation
 * @returns {object|null}
 */
export const normalizePolicyExplanation = (explanation) => {
  if (!explanation) return null;
  if (typeof explanation?.toJSON === 'function') {
    try {
      return explanation.toJSON();
    } catch {
      return null;
    }
  }
  if (typeof explanation !== 'object' || Array.isArray(explanation)) return null;
  return explanation;
};

/**
 * Build the saga-style rollback contract for a given forward tool.
 *
 * Looks up the tool in the static compensation tables and constructs a
 * `{strategy, sourceTool, compensation, reversible, contractHash}`
 * envelope. The `contractHash` is content-addressed so identical
 * contracts produce identical hashes for replay-log indexing.
 *
 * @param {string} toolName - the forward tool that this contract reverses
 * @returns {{strategy: string, sourceTool: string, compensation: Array, reversible: boolean, contractHash: string}}
 */
export const buildRollbackContract = (toolName) => {
  const compensationTools = AGENTIC_COMPENSATION_HINTS[toolName] || [];
  const compensationContracts = compensationTools.map((tool) => ({
    tool,
    params: AGENTIC_COMPENSATION_PARAM_HINTS[tool] || ['id'],
  }));

  const contract = {
    strategy: compensationContracts.length > 0 ? 'best_effort_compensation' : 'none',
    sourceTool: toolName,
    compensation: compensationContracts,
    reversible: compensationContracts.length > 0,
  };

  return {
    ...contract,
    contractHash: replayEventHash(contract),
  };
};

/**
 * Extract an ordered, deduplicated list of approval stages from a
 * policy action list.
 *
 * Each input action may declare:
 *   - an explicit `approval.stages: [...]` array of pre-shaped stages
 *   - or a single `approval` object that gets promoted to a 1-stage list
 *   - or just `metadata.requiresApproval: true` with no other detail
 *
 * Output stages are deduplicated by `(level, name)` and sorted by
 * `level` ascending. Stage-level defaults: `level` falls through to a
 * sequential counter, `name` falls through to either
 * `metadata.approvalTier` or `'approval-required'`,
 * `requiredApprovals` defaults to 1, `approvers` defaults to `[]`.
 *
 * @param {Array<unknown>} [actions=[]]
 * @returns {Array<{level: number, name: string, requiredApprovals: number, approvers: Array, timeout: unknown, timeoutAction: unknown, source: string}>}
 */
export const buildApprovalStagesFromActions = (actions = []) => {
  const stages = [];
  for (const rawAction of actions) {
    const action = normalizePolicyAction(rawAction);
    if (!action) continue;
    const approval = action.approval || action?.metadata?.approval || null;
    const requiresApproval = Boolean(action?.metadata?.requiresApproval) || Boolean(approval);
    if (!requiresApproval) continue;

    if (Array.isArray(approval?.stages) && approval.stages.length > 0) {
      for (const stage of approval.stages) {
        if (!stage || typeof stage !== 'object') continue;
        stages.push({
          level: Number.isFinite(Number(stage.level)) ? Number(stage.level) : stages.length + 1,
          name: stage.name || `stage-${stages.length + 1}`,
          requiredApprovals: Number(stage.requiredApprovals || 1),
          approvers: Array.isArray(stage.approvers) ? stage.approvers : [],
          timeout: stage.timeout || null,
          timeoutAction: stage.timeoutAction || null,
          source: 'policy_action',
        });
      }
      continue;
    }

    stages.push({
      level: Number.isFinite(Number(approval?.level)) ? Number(approval.level) : stages.length + 1,
      name: approval?.name || action?.metadata?.approvalTier || 'approval-required',
      requiredApprovals: Number(approval?.requiredApprovals || 1),
      approvers: Array.isArray(approval?.approvers) ? approval.approvers : [],
      timeout: approval?.timeout || null,
      timeoutAction: approval?.timeoutAction || null,
      source: 'policy_action',
    });
  }

  const deduped = [];
  const seen = new Set();
  for (const stage of stages) {
    const key = `${stage.level}:${stage.name}`;
    if (seen.has(key)) continue;
    seen.add(key);
    deduped.push(stage);
  }
  return deduped.sort((a, b) => a.level - b.level);
};

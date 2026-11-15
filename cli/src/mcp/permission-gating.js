// Permission gating for MCP tool calls.
//
// Every tool call passes through `checkPermission` before it may execute.
// Three outcomes are possible:
//   1. A configured PermissionGate decides (its result is returned as-is).
//   2. No gate: the call is allowed when `--apply` is set or the tool is
//      read-only.
//   3. Otherwise the call is downgraded to a *preview* — the caller learns
//      what would have run, but nothing executes.
//
// Every decision emits a `permission_decision` telemetry event with the
// same shape as before extraction.
//
// A preview of a catalogued status move (e.g. `mark_return_received`) also
// says which states the engine accepts that move from, so "would execute"
// never reads as "would succeed" (`wouldDo.transition`, from the state
// machine catalog; nothing is read from the store).
//
// Extracted from mcp-server.js (pure move — no behaviour change).

import { previewForMove } from './transition-guidance.js';

/**
 * Add the catalog's precondition to a preview of a catalogued status move.
 * Any other decision is returned as it is.
 */
export function withMovePreview(toolName, params, decision) {
  if (!decision?.preview) return decision;
  const move = previewForMove(toolName, params);
  if (!move) return decision;
  const reason = decision.reason ? `${decision.reason}. ${move.sentence}` : move.sentence;
  return {
    ...decision,
    reason,
    wouldDo: {
      ...(decision.wouldDo ?? { tool: toolName, params }),
      transition: move.transition,
    },
  };
}

/**
 * Derive the set of read-only tool names from tool permission metadata.
 *
 * @param {Array<{name: string, permission?: string}>} toolDefs
 * @returns {Set<string>}
 */
export function buildReadOnlyToolSet(toolDefs) {
  return new Set(toolDefs.filter((t) => t.permission === 'read').map((t) => t.name));
}

/**
 * Build the per-server `checkPermission(toolName, params)` function.
 *
 * @param {{
 *   permissionGate?: { checkPermission: (tool: string, params: unknown) => Promise<object> } | null,
 *   telemetry?: { logCustomEvent: (name: string, payload: object) => void } | null,
 *   allowApply: boolean,
 *   isReadOnly: (toolName: string) => boolean,
 * }} deps
 * @returns {(toolName: string, params: unknown) => Promise<{
 *   allowed: boolean,
 *   preview?: boolean,
 *   reason?: string,
 *   wouldDo?: { tool: string, params: unknown },
 * }>}
 */
export function createCheckPermission({ permissionGate, telemetry, allowApply, isReadOnly }) {
  const checkPermission = async (toolName, params) => {
    if (permissionGate) {
      const result = withMovePreview(
        toolName,
        params,
        await permissionGate.checkPermission(toolName, params),
      );
      if (telemetry) {
        telemetry.logCustomEvent('permission_decision', {
          tool: toolName,
          allowed: result.allowed,
          preview: result.preview || false,
          reason: result.reason || null,
        });
      }
      return result;
    }
    if (allowApply || isReadOnly(toolName)) {
      if (telemetry) {
        telemetry.logCustomEvent('permission_decision', {
          tool: toolName,
          allowed: true,
          preview: false,
        });
      }
      return { allowed: true };
    }
    const result = withMovePreview(toolName, params, {
      allowed: false,
      preview: true,
      reason: `Preview mode: would execute '${toolName}' if --apply flag is set`,
      wouldDo: { tool: toolName, params },
    });
    if (telemetry) {
      telemetry.logCustomEvent('permission_decision', {
        tool: toolName,
        allowed: false,
        preview: true,
        reason: result.reason,
      });
    }
    return result;
  };

  // `inferPolicyDomain` is bound at module scope (see top of file). Its
  // pure logic lives in `./mcp/policy-domain.js`. Kept the same name so
  // the ~17 call sites below continue to work without churn.

  // `normalizeToolName` and `applyPolicyTransform` now live in
  // `./mcp/policy-helpers.js` — both are pure (no closure deps) and are
  // imported at the top of the file.

  return checkPermission;
}

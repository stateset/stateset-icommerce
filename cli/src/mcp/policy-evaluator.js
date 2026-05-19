/**
 * Policy decision bundle builder + tool-level policy evaluator.
 *
 * `buildPolicyDecisionBundle` turns the engine's allow/deny verdict into a
 * signed, hash-stable bundle the audit log can replay. `createEvaluatePolicy`
 * is a factory that returns the per-server policy gate used by `wrapTool`
 * and the plan executor.
 *
 * Pulled out of mcp-server.js so the policy path stays testable in isolation
 * and the orchestrator file can shrink toward the wiring it actually owns.
 */

import {
  buildApprovalStagesFromActions,
  buildRollbackContract,
  normalizePolicyAction,
  normalizePolicyExplanation,
  replayEventHash,
} from './audit-envelope.js';
import { applyPolicyTransform } from './policy-helpers.js';

export const AGENTIC_POLICY_DECISION_BUNDLE_VERSION = '2026-03-01';

/**
 * @typedef {Object} PolicyBundleDeps
 * @property {(toolName: string) => Object} getToolRuntimeMeta
 * @property {(toolName: string) => string} inferPolicyDomain
 * @property {(payload: Object) => Object} signAuditArtifact
 * @property {boolean} allowApply
 * @property {string} [bundleVersion]
 */

/**
 * Build a signed policy decision bundle for an evaluated tool call.
 *
 * @param {Object} args
 * @param {string} args.toolName
 * @param {string} [args.domain]
 * @param {Object} [args.inputParams]
 * @param {Object} [args.outputParams]
 * @param {Array<unknown>} [args.actions]
 * @param {Array<unknown>} [args.explanations]
 * @param {boolean} [args.allowed]
 * @param {string|null} [args.reason]
 * @param {PolicyBundleDeps} deps
 */
export function buildPolicyDecisionBundle(
  {
    toolName,
    domain,
    inputParams = {},
    outputParams = {},
    actions = [],
    explanations = [],
    allowed = true,
    reason = null,
  },
  {
    getToolRuntimeMeta,
    inferPolicyDomain,
    signAuditArtifact,
    allowApply,
    bundleVersion = AGENTIC_POLICY_DECISION_BUNDLE_VERSION,
  },
) {
  const runtimeMeta = getToolRuntimeMeta(toolName);
  const normalizedActions = actions.map((action) => normalizePolicyAction(action)).filter(Boolean);
  const normalizedExplanations = explanations
    .map((explanation) => normalizePolicyExplanation(explanation))
    .filter(Boolean);
  const approvalStages = buildApprovalStagesFromActions(normalizedActions);
  const rollbackContract = buildRollbackContract(toolName);

  const core = {
    version: bundleVersion,
    engine: 'stateset-icommerce',
    tool: toolName,
    domain: domain || inferPolicyDomain(toolName),
    decision: allowed ? 'allow' : 'deny',
    reason: reason || null,
    policyMode: allowApply ? 'apply' : 'preview',
    runtime: {
      sideEffect: runtimeMeta.sideEffect,
      idempotent: runtimeMeta.idempotent,
      compensations: runtimeMeta.compensations,
    },
    actionTypes: normalizedActions.map((action) => action.type).filter(Boolean),
    approval: {
      required: approvalStages.length > 0,
      stages: approvalStages,
    },
    rollback: rollbackContract,
    inputParamsHash: replayEventHash(inputParams || {}),
    outputParamsHash: replayEventHash(outputParams || inputParams || {}),
    explanationsHash: replayEventHash(normalizedExplanations),
  };
  const bundleId = replayEventHash(core);
  const auditArtifact = signAuditArtifact({ bundleId, ...core });

  return {
    ...core,
    bundleId,
    createdAt: new Date().toISOString(),
    auditArtifact,
  };
}

/**
 * @typedef {Object} EvaluatePolicyDeps
 * @property {Object|null} policyEngine - PolicyEngine instance (or null)
 * @property {Promise<unknown>} policyReady - Resolves when policy store is loaded
 * @property {boolean} allowApply
 * @property {Object|null} telemetry - optional `.logCustomEvent`
 * @property {(toolName: string) => string} inferPolicyDomain
 * @property {(toolName: string) => Object} getToolRuntimeMeta
 * @property {(payload: Object) => Object} signAuditArtifact
 * @property {string} [bundleVersion]
 */

/**
 * Create the per-server `evaluatePolicy(toolName, params, extra, policyDomain?)`
 * function used by tool wrappers and the plan executor.
 *
 * Returns `{ allowed, params, ...explanations }` for allowed calls, or
 * `{ allowed: false, reason, remediation, ... }` for denied ones. When no
 * policy engine is configured every call is allowed and a trivial bundle is
 * still produced so downstream audit/telemetry behavior stays consistent.
 *
 * @param {EvaluatePolicyDeps} deps
 */
export function createEvaluatePolicy(deps) {
  const {
    policyEngine,
    policyReady,
    allowApply,
    telemetry,
    inferPolicyDomain,
    getToolRuntimeMeta,
    signAuditArtifact,
    bundleVersion = AGENTIC_POLICY_DECISION_BUNDLE_VERSION,
  } = deps;

  const bundleDeps = {
    getToolRuntimeMeta,
    inferPolicyDomain,
    signAuditArtifact,
    allowApply,
    bundleVersion,
  };
  const buildBundle = (args) => buildPolicyDecisionBundle(args, bundleDeps);

  return async function evaluatePolicy(toolName, params, extra, policyDomain = null) {
    if (!policyEngine) {
      const domain = policyDomain || inferPolicyDomain(toolName);
      return {
        allowed: true,
        params,
        domain,
        policyDecisionBundle: buildBundle({
          toolName,
          domain,
          inputParams: params,
          outputParams: params,
          actions: [],
          explanations: [],
          allowed: true,
        }),
      };
    }

    await policyReady;

    const domain = policyDomain || inferPolicyDomain(toolName);
    const policyContext = {
      domain,
      tool: toolName,
      params,
      allowApply,
      requestId: extra?.requestId || null,
      sessionId: extra?.sessionId || null,
    };

    let result;
    try {
      result = await policyEngine.evaluate(domain, policyContext);
    } catch (error) {
      if (telemetry) {
        telemetry.logCustomEvent('policy_evaluation_failed', {
          tool: toolName,
          domain,
          error: error.message,
        });
      }
      return {
        allowed: true,
        params,
        domain,
        policyDecisionBundle: buildBundle({
          toolName,
          domain,
          inputParams: params,
          outputParams: params,
          actions: [],
          explanations: [],
          allowed: true,
        }),
      };
    }

    const actions = Array.isArray(result?.actions) ? result.actions : [];
    const notifyActions = actions.filter((action) => action?.type === 'notify');

    let transformedParams = params;
    const transformAudit = [];
    for (const action of actions) {
      if (action?.type === 'transform') {
        const { output, auditEntries } = applyPolicyTransform(
          transformedParams,
          action.transform,
          [],
        );
        transformedParams = output;
        for (const entry of auditEntries) {
          transformAudit.push({
            ...entry,
            ruleId: action.metadata?.ruleId || null,
            ruleName: action.metadata?.ruleName || null,
            policySetId: action.metadata?.policySetId || null,
          });
        }
      }
    }

    if (telemetry) {
      telemetry.logCustomEvent('policy_evaluation', {
        tool: toolName,
        domain,
        allowed: !result?.shouldDeny,
        actionCount: actions.length,
        actionTypes: actions.map((action) => action?.type).filter(Boolean),
        transformAuditCount: transformAudit.length,
      });
    }

    if (notifyActions.length > 0) {
      for (const action of notifyActions) {
        if (telemetry) {
          telemetry.logCustomEvent('policy_notify', {
            tool: toolName,
            domain,
            message: action.notification?.message || action.message || null,
          });
        }
      }
    }

    const explanations = result?.explanations || [];
    const policyDecisionBundle = buildBundle({
      toolName,
      domain,
      inputParams: params,
      outputParams: transformedParams,
      actions,
      explanations,
      allowed: !result?.shouldDeny,
      reason: result?.shouldDeny
        ? explanations
            .filter((e) => (e?.actionType || e?.type || '') === 'deny')
            .map((e) => e?.reason)
            .filter(Boolean)
            .join('; ')
        : null,
    });

    if (result?.shouldDeny) {
      const denyExplanations = explanations
        .filter((e) => e.actionType === 'deny')
        .map((e) => (typeof e.toJSON === 'function' ? e.toJSON() : e));

      const reason =
        denyExplanations
          .map((e) => e.reason || `Rule "${e.ruleName}" denied this operation`)
          .filter(Boolean)
          .join('; ') || 'Tool denied by policy';

      const remediation =
        denyExplanations
          .map((e) => e.remediation)
          .filter(Boolean)
          .join('; ') || null;

      return {
        allowed: false,
        params: transformedParams,
        reason,
        remediation,
        explanations: denyExplanations,
        transformAudit,
        actions,
        domain,
        evaluation: result,
        policyDecisionBundle,
      };
    }

    return {
      allowed: true,
      params: transformedParams,
      explanations: explanations.map((e) => (typeof e.toJSON === 'function' ? e.toJSON() : e)),
      transformAudit,
      actions,
      domain,
      evaluation: result,
      policyDecisionBundle,
    };
  };
}

// Single-step execution for agentic plans + direct `server.executeTool`.
//
// `executeToolStepInPlan` runs one tool through the full gauntlet — schema
// validation, before/after hooks, policy, permission, MPP payment, treasury
// charge — and returns a uniform outcome record. The same function is used
// for plan steps, rollback compensations, and direct executions.
//
// Extracted from mcp-server.js (pure move — no behaviour change). The only
// edit is that the shared tool context is read through `getToolContext()`
// so the orchestrator can build the context after this factory runs.

import {
  MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE,
  attachPaymentMetadata,
  createPaymentReceipt,
} from '../mpp/index.js';
import { formatValidationIssues, validateToolInput } from '../tool-schema.js';
import { replayEventHash } from './audit-envelope.js';
import { buildDeterministicMutationManifest } from './mutation-manifest.js';
import { normalizeToolName } from './policy-helpers.js';
import { compactReplayValue } from './replay-sanitizer.js';

/**
 * Build `executeToolStepInPlan` for one server instance.
 *
 * @param {{
 *   toolDefsByName: Map<string, object>,
 *   inferPolicyDomain: (toolName: string) => string,
 *   getToolRuntimeMeta: (toolName: string) => object,
 *   hookRunner?: { hasHooks?: (hook: string) => boolean, run: (hook: string, payload: object) => Promise<object> } | null,
 *   allowApply: boolean,
 *   evaluatePolicy: (toolName: string, params: unknown, extra: object, policyDomain?: string | null) => Promise<object>,
 *   checkPermission: (toolName: string, params: unknown) => Promise<object>,
 *   resolveMppPaymentContext: (input: object) => Promise<object>,
 *   maybeChargeForTool: (toolName: string, extra: object, opts: object) => Promise<object>,
 *   wrapWithTelemetry: (toolName: string, fn: Function) => Function,
 *   getToolContext: () => object,
 * }} deps
 * @returns {(input: {
 *   toolName: string,
 *   params: unknown,
 *   policyDomain?: string | null,
 *   requestId?: string | null,
 *   sessionId?: string | null,
 *   dryRun?: boolean,
 *   stepIndex: number,
 *   includeHooks?: boolean,
 *   isRollback?: boolean,
 *   extra?: object,
 * }) => Promise<object>}
 */
export function createExecuteToolStepInPlan({
  toolDefsByName: TOOL_DEFS_BY_NAME,
  inferPolicyDomain,
  getToolRuntimeMeta,
  hookRunner,
  allowApply,
  evaluatePolicy,
  checkPermission,
  resolveMppPaymentContext,
  maybeChargeForTool,
  wrapWithTelemetry,
  getToolContext,
}) {
  const executeToolStepInPlan = async ({
    toolName,
    params,
    policyDomain,
    requestId,
    sessionId,
    dryRun,
    stepIndex,
    includeHooks = true,
    isRollback = false,
    extra = {},
  }) => {
    const startedAt = Date.now();
    const resolvedToolName = normalizeToolName(toolName);
    const effectivePolicyDomain = policyDomain || inferPolicyDomain(resolvedToolName);
    const baseMeta = getToolRuntimeMeta(resolvedToolName);
    if (baseMeta.permission === 'unknown') {
      return {
        index: stepIndex,
        tool: resolvedToolName,
        status: 'invalid',
        elapsedMs: Date.now() - startedAt,
        error: `Unknown tool '${toolName}'`,
        simulation: false,
      };
    }

    const toolDef = TOOL_DEFS_BY_NAME.get(resolvedToolName);
    if (!toolDef || typeof toolDef.handler !== 'function') {
      return {
        index: stepIndex,
        tool: resolvedToolName,
        status: 'invalid',
        elapsedMs: Date.now() - startedAt,
        error: `No executable handler for tool '${toolName}'`,
        simulation: false,
      };
    }

    const validation = validateToolInput(toolDef.inputSchema || {}, params || {});
    if (!validation.success) {
      return {
        index: stepIndex,
        tool: resolvedToolName,
        status: 'invalid',
        elapsedMs: Date.now() - startedAt,
        error: `Invalid parameters for tool '${resolvedToolName}'`,
        simulation: dryRun,
        runtime: {
          policyDomain: effectivePolicyDomain,
          sideEffect: baseMeta.sideEffect,
          compensations: baseMeta.compensations,
          idempotent: baseMeta.idempotent,
        },
        params: compactReplayValue(params || {}),
        paramsHash: replayEventHash(params || {}),
        result: null,
        resultHash: null,
        notes: {
          validation: formatValidationIssues(validation.error),
        },
      };
    }

    let nextArgs = validation.data;
    let policy = null;
    let permission = null;
    let charge = null;
    const buildStepMutationManifest = (
      paramsValue = nextArgs,
      policyValue = policy,
      permissionValue = permission,
      phase = dryRun ? 'dry_run' : 'execute',
    ) => {
      return buildDeterministicMutationManifest({
        toolName: resolvedToolName,
        params: paramsValue || {},
        policy: policyValue || null,
        permission: permissionValue || null,
        runtimeMeta: baseMeta,
        phase,
      });
    };

    try {
      if (includeHooks && hookRunner?.hasHooks?.('before_tool_call')) {
        const hookResult = await hookRunner.run('before_tool_call', {
          tool: resolvedToolName,
          params: nextArgs,
          allowApply,
          requestId,
          sessionId,
        });
        if (hookResult?.params) nextArgs = hookResult.params;
        if (hookResult?.blocked || hookResult?.allowed === false) {
          return {
            index: stepIndex,
            tool: resolvedToolName,
            status: 'blocked',
            elapsedMs: Date.now() - startedAt,
            policy: null,
            permission: null,
            charge: null,
            result: null,
            error: hookResult?.reason || 'Tool execution blocked by hook',
            runtime: {
              policyDomain: effectivePolicyDomain,
              sideEffect: baseMeta.sideEffect,
              compensations: baseMeta.compensations,
              idempotent: baseMeta.idempotent,
            },
            params: compactReplayValue(nextArgs),
            paramsHash: replayEventHash(nextArgs),
            resultHash: null,
            simulation: false,
            mutationManifest: buildStepMutationManifest(nextArgs, null, null, 'blocked'),
            notes: {
              hook: {
                allowed: hookResult?.allowed,
                reason: hookResult?.reason || null,
                blocked: true,
              },
            },
          };
        }
      }

      policy = await evaluatePolicy(
        resolvedToolName,
        nextArgs,
        { requestId, sessionId },
        effectivePolicyDomain,
      );
      if (!policy.allowed) {
        return {
          index: stepIndex,
          tool: resolvedToolName,
          status: 'policy_block',
          elapsedMs: Date.now() - startedAt,
          policy: {
            allowed: policy.allowed,
            domain: policy.domain,
            reason: policy.reason || null,
            actions: policy.actions || [],
            decisionBundle: policy.policyDecisionBundle || null,
          },
          permission: null,
          charge: null,
          params: compactReplayValue(nextArgs),
          paramsHash: replayEventHash(nextArgs),
          resultHash: null,
          result: null,
          runtime: {
            policyDomain: effectivePolicyDomain,
            sideEffect: baseMeta.sideEffect,
            compensations: baseMeta.compensations,
            idempotent: baseMeta.idempotent,
          },
          simulation: dryRun,
          mutationManifest: buildStepMutationManifest(nextArgs, policy, null, 'policy_block'),
          error: policy.reason || 'Tool execution blocked by policy',
        };
      }

      nextArgs = policy.params;

      permission = await checkPermission(resolvedToolName, nextArgs);
      if (!permission.allowed) {
        const blockedStatus =
          dryRun && permission.preview
            ? 'dry_run_blocked'
            : permission.preview
              ? 'preview'
              : 'permission_block';
        const payload = {
          status: blockedStatus,
          preview: permission.preview || false,
          elapsedMs: Date.now() - startedAt,
          policy: {
            allowed: policy.allowed,
            domain: policy.domain,
            actions: policy.actions || [],
            decisionBundle: policy.policyDecisionBundle || null,
          },
          permission: {
            allowed: permission.allowed,
            preview: permission.preview || false,
            reason: permission.reason || null,
          },
          charge: null,
          params: compactReplayValue(nextArgs),
          paramsHash: replayEventHash(nextArgs),
          result: null,
          resultHash: null,
          runtime: {
            policyDomain: effectivePolicyDomain,
            sideEffect: baseMeta.sideEffect,
            compensations: baseMeta.compensations,
            idempotent: baseMeta.idempotent,
          },
          simulation: dryRun,
          mutationManifest: buildStepMutationManifest(nextArgs, policy, permission, blockedStatus),
          error: permission.reason || 'Permission denied',
          wouldDo: permission.wouldDo || null,
        };
        return {
          index: stepIndex,
          tool: resolvedToolName,
          ...payload,
        };
      }

      const mpp = await resolveMppPaymentContext({
        toolName: resolvedToolName,
        description: toolDef.description,
        params: nextArgs,
        extra,
        requestId,
        sessionId,
      });

      if (mpp?.pricing && !mpp.authorized) {
        return {
          index: stepIndex,
          tool: resolvedToolName,
          status: 'payment_required',
          elapsedMs: Date.now() - startedAt,
          policy: {
            allowed: policy.allowed,
            domain: policy.domain,
            actions: policy.actions || [],
            decisionBundle: policy.policyDecisionBundle || null,
          },
          permission: {
            allowed: permission.allowed,
            preview: permission.preview || false,
            reason: permission.reason || null,
          },
          charge: {
            charged: false,
            blocked: true,
            reason: MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE,
            paymentRequired: true,
            pricing: mpp.pricing,
            challenge: mpp.challenge,
          },
          params: compactReplayValue(nextArgs),
          paramsHash: replayEventHash(nextArgs),
          result: compactReplayValue(mpp.errorPayload),
          resultHash: replayEventHash(mpp.errorPayload),
          runtime: {
            policyDomain: effectivePolicyDomain,
            sideEffect: baseMeta.sideEffect,
            compensations: baseMeta.compensations,
            idempotent: baseMeta.idempotent,
          },
          simulation: dryRun,
          mutationManifest: buildStepMutationManifest(
            nextArgs,
            policy,
            permission,
            'payment_required',
          ),
          error: mpp?.verification?.reason || MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE,
        };
      }

      charge = await maybeChargeForTool(
        resolvedToolName,
        { requestId, sessionId },
        {
          dryRun,
          allowChargeWrite: Boolean(mpp?.authorized),
          paymentCredential: mpp?.credential || null,
        },
      );
      if (charge?.blocked) {
        return {
          index: stepIndex,
          tool: resolvedToolName,
          status: dryRun ? 'dry_run_blocked' : 'treasury_block',
          elapsedMs: Date.now() - startedAt,
          policy: {
            allowed: policy.allowed,
            domain: policy.domain,
            actions: policy.actions || [],
            decisionBundle: policy.policyDecisionBundle || null,
          },
          permission: {
            allowed: permission.allowed,
            preview: permission.preview || false,
            reason: permission.reason || null,
          },
          charge: {
            charged: charge.charged,
            blocked: charge.blocked,
            reason: charge.reason || null,
          },
          params: compactReplayValue(nextArgs),
          paramsHash: replayEventHash(nextArgs),
          result: null,
          resultHash: null,
          runtime: {
            policyDomain: effectivePolicyDomain,
            sideEffect: baseMeta.sideEffect,
            compensations: baseMeta.compensations,
            idempotent: baseMeta.idempotent,
          },
          simulation: dryRun,
          mutationManifest: buildStepMutationManifest(
            nextArgs,
            policy,
            permission,
            dryRun ? 'dry_run_blocked' : 'treasury_block',
          ),
          error: charge.reason || 'Treasury charge blocked',
        };
      }

      if (dryRun) {
        return {
          index: stepIndex,
          tool: resolvedToolName,
          status: 'dry_run_success',
          elapsedMs: Date.now() - startedAt,
          policy: {
            allowed: policy.allowed,
            domain: policy.domain,
            actions: policy.actions || [],
            decisionBundle: policy.policyDecisionBundle || null,
          },
          permission: {
            allowed: permission.allowed,
            preview: permission.preview || false,
            reason: permission.reason || null,
          },
          charge: {
            charged: charge.charged,
            blocked: false,
            rule: charge.rule || null,
          },
          params: compactReplayValue(nextArgs),
          paramsHash: replayEventHash(nextArgs),
          result: {
            dryRun: true,
            wouldExecute: resolvedToolName,
            policyDomain: effectivePolicyDomain,
          },
          resultHash: replayEventHash({ dryRun: true, wouldExecute: resolvedToolName }),
          runtime: {
            policyDomain: effectivePolicyDomain,
            sideEffect: baseMeta.sideEffect,
            compensations: baseMeta.compensations,
            idempotent: baseMeta.idempotent,
          },
          simulation: true,
          mutationManifest: buildStepMutationManifest(
            nextArgs,
            policy,
            permission,
            'dry_run_success',
          ),
          requestId,
        };
      }

      const toolPayload = {
        ...getToolContext(),
        params: nextArgs,
        extra: {
          requestId,
          sessionId,
          ...extra,
        },
      };
      const wrapped = wrapWithTelemetry(resolvedToolName, (payload) => toolDef.handler(payload));
      let result = await wrapped(toolPayload);
      if (mpp?.authorized && charge?.charged) {
        const receipt = createPaymentReceipt({
          challenge: mpp.challenge,
          credential: mpp.credential,
          charge,
          toolName: resolvedToolName,
          requestId,
          sessionId,
        });
        result = attachPaymentMetadata(result, {
          protocol: 'mpp',
          receipt,
          credentialId: mpp?.credential?.credentialId || null,
        });
      }
      if (includeHooks && hookRunner?.hasHooks?.('after_tool_call')) {
        await hookRunner.run('after_tool_call', {
          tool: resolvedToolName,
          params: nextArgs,
          result,
          requestId,
          sessionId,
        });
      }

      const failed = !!(result && typeof result === 'object' && result.error);
      const failure = failed ? result.error : null;
      const finalStatus = isRollback
        ? failed
          ? 'rollback_failed'
          : 'rollback_success'
        : failed
          ? 'error'
          : 'success';
      return {
        index: stepIndex,
        tool: resolvedToolName,
        status: finalStatus,
        elapsedMs: Date.now() - startedAt,
        policy: {
          allowed: policy.allowed,
          domain: policy.domain,
          actions: policy.actions || [],
          decisionBundle: policy.policyDecisionBundle || null,
        },
        permission: {
          allowed: permission.allowed,
          preview: permission.preview || false,
          reason: permission.reason || null,
        },
        charge: {
          charged: charge.charged,
          blocked: charge.blocked || false,
          rule: charge.rule || null,
        },
        params: compactReplayValue(nextArgs),
        paramsHash: replayEventHash(nextArgs),
        result: compactReplayValue(result),
        resultHash: replayEventHash(compactReplayValue(result)),
        runtime: {
          policyDomain: effectivePolicyDomain,
          sideEffect: baseMeta.sideEffect,
          compensations: baseMeta.compensations,
          idempotent: baseMeta.idempotent,
        },
        simulation: false,
        mutationManifest: buildStepMutationManifest(nextArgs, policy, permission, finalStatus),
        resultSuccess: !failed,
        error: failure,
        isRollback: Boolean(isRollback),
        requestId,
      };
    } catch (error) {
      if (includeHooks && hookRunner?.hasHooks?.('after_tool_call')) {
        await hookRunner.run('after_tool_call', {
          tool: resolvedToolName,
          params: nextArgs,
          error: error.message,
          requestId,
          sessionId,
        });
      }
      return {
        index: stepIndex,
        tool: resolvedToolName,
        status: isRollback ? 'rollback_failed' : 'error',
        elapsedMs: Date.now() - startedAt,
        policy: policy
          ? {
              allowed: policy.allowed,
              domain: policy.domain,
              actions: policy.actions || [],
              decisionBundle: policy.policyDecisionBundle || null,
            }
          : null,
        permission: permission
          ? {
              allowed: permission.allowed,
              preview: permission.preview || false,
              reason: permission.reason || null,
            }
          : null,
        charge: charge
          ? {
              charged: charge.charged,
              blocked: charge.blocked || false,
              rule: charge.rule || null,
            }
          : null,
        params: compactReplayValue(nextArgs),
        paramsHash: replayEventHash(nextArgs),
        result: null,
        resultHash: null,
        runtime: {
          policyDomain: effectivePolicyDomain,
          sideEffect: baseMeta.sideEffect,
          compensations: baseMeta.compensations,
          idempotent: baseMeta.idempotent,
        },
        simulation: false,
        mutationManifest: buildStepMutationManifest(nextArgs, policy, permission, 'error'),
        error: error.message,
        isRollback: Boolean(isRollback),
      };
    }
  };

  return executeToolStepInPlan;
}

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
//
// Kernel strict mode. Only a `topLevel` call (`server.executeTool`) may run a
// governed composite (`agentic_execute_plan`, `agentic_replay_mutation`)
// without a typed kernel command. Every step a composite runs comes back here
// as a nested call, where a withheld write or a nested composite is refused
// before its handler runs and every other live write must cross
// `executeGovernedTool` with `requireGoverned`.

import {
  MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE,
  attachPaymentMetadata,
  createPaymentReceipt,
} from '../mpp/index.js';
import { isKernelGovernedComposite } from '../kernel-boundary.js';
import { formatValidationIssues, validateToolInput } from '../tool-schema.js';
import { replayEventHash } from './audit-envelope.js';
import { buildDeterministicMutationManifest } from './mutation-manifest.js';
import { normalizeToolName } from './policy-helpers.js';
import { compactReplayValue } from './replay-sanitizer.js';
import { RAW_TOOL_RESULT, normalizeToolResult } from './tool-result-contract.js';

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
 *   executeGovernedTool?: (toolName: string, params?: object, options?: object) => Promise<object | null>,
 *   strictKernel?: { isWithheld: (toolName: string) => boolean } | null,
 * }} deps `strictKernel` is set only in kernel strict mode; `isWithheld` names
 *   the writes the strict boundary does not expose.
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
 *   topLevel?: boolean,
 *   extra?: object,
 * }) => Promise<object>} `topLevel` is true only for a direct call
 *   (`server.executeTool`), never for a step a plan, replay, or rollback runs.
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
  executeGovernedTool,
  strictKernel = null,
}) {
  /** Why kernel strict mode refuses this call before it runs, or null. */
  const strictKernelRefusal = (toolName, topLevel) => {
    if (!strictKernel) return null;
    if (strictKernel.isWithheld(toolName)) {
      return {
        boundary: 'withheld',
        error:
          `Tool '${toolName}' is a write mutation outside the governed kernel catalog, ` +
          'so kernel strict mode does not run it, directly or as a plan or replay step.',
      };
    }
    if (!topLevel && isKernelGovernedComposite(toolName)) {
      return {
        boundary: 'nested_composite',
        error:
          `Tool '${toolName}' is a governed composite; kernel strict mode runs a composite ` +
          'only as a top-level call, never as a step of another plan or replay.',
      };
    }
    return null;
  };

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
    topLevel = false,
    extra = {},
  }) => {
    const startedAt = Date.now();
    const resolvedToolName = normalizeToolName(toolName);
    const effectivePolicyDomain = policyDomain || inferPolicyDomain(resolvedToolName);
    const baseMeta = getToolRuntimeMeta(resolvedToolName);
    const refusal = strictKernelRefusal(resolvedToolName, topLevel);
    if (refusal) {
      // A governance refusal, so a plan that reaches it reports failed. It is
      // decided by name alone, before hooks, policy, or a dry run, so a dry run
      // predicts the refusal an apply would meet.
      return {
        index: stepIndex,
        tool: resolvedToolName,
        status: 'permission_block',
        elapsedMs: Date.now() - startedAt,
        policy: null,
        permission: null,
        charge: null,
        params: compactReplayValue(params || {}),
        paramsHash: replayEventHash(params || {}),
        result: null,
        resultHash: null,
        simulation: Boolean(dryRun),
        error: refusal.error,
        notes: { strictKernel: refusal.boundary },
      };
    }
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
        const governedPreview =
          permission.preview && executeGovernedTool
            ? await executeGovernedTool(resolvedToolName, nextArgs, {
                requestId,
                sessionId,
                ...extra,
              })
            : null;
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
          result: governedPreview,
          resultHash: governedPreview ? replayEventHash(governedPreview) : null,
          runtime: {
            policyDomain: effectivePolicyDomain,
            sideEffect: baseMeta.sideEffect,
            compensations: baseMeta.compensations,
            idempotent: baseMeta.idempotent,
          },
          simulation: dryRun,
          mutationManifest: buildStepMutationManifest(nextArgs, policy, permission, blockedStatus),
          error: governedPreview ? null : permission.reason || 'Permission denied',
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
      // Every live write must be a typed kernel command (strict mode refuses
      // the rest), except a top-level governed composite: it runs no write
      // itself, and each step it runs comes back through here nested. Set after
      // `extra` so no caller-supplied option can switch the requirement off.
      const governed = executeGovernedTool
        ? await executeGovernedTool(resolvedToolName, nextArgs, {
            requestId,
            sessionId,
            ...extra,
            requireGoverned:
              allowApply &&
              baseMeta.sideEffect === 'write' &&
              !(topLevel && isKernelGovernedComposite(resolvedToolName)),
          })
        : null;
      const wrapped = wrapWithTelemetry(resolvedToolName, (payload) => toolDef.handler(payload));
      let result = governed ?? (await wrapped(toolPayload));
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

      // A tool reports failure in its own result -- `{ success: false }`, a
      // truthy `error`, or a kernel receipt that was rejected -- and the
      // contract reads every one of those shapes.
      const judged = normalizeToolResult(result);
      const legacyFailed = !!(result && typeof result === 'object' && result.error);
      const failed = legacyFailed || !judged.ok;
      const failure = legacyFailed ? result.error : failed ? judged.error.message : null;
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
        [RAW_TOOL_RESULT]: result,
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
        // Keep the thrown error's stable code (binding `NOT_FOUND`, `CONFLICT`, …)
        // so the result contract can report it instead of parsing prose.
        ...(typeof error?.code === 'string' ? { errorCode: error.code } : {}),
        ...(error?.details && typeof error.details === 'object'
          ? { errorDetails: error.details }
          : {}),
        isRollback: Boolean(isRollback),
      };
    }
  };

  return executeToolStepInPlan;
}

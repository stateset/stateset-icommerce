// `agentic_plan` simulation for the MCP orchestrator.
//
// Walks a plan step-by-step *without executing anything*: resolves
// `$plan.*` parameter references, routes each step, evaluates policy +
// permission, projects treasury cost against the budget, and emits a
// deterministic per-step outcome plus a plan signature.
//
// Extracted from mcp-server.js (pure move — no behaviour change).

import { replayEventHash } from './audit-envelope.js';
import {
  addCostSummaryEntry,
  createCostSummary,
  normalizeCostBudget,
  resolveCostBudgetLimit,
} from './cost-budget.js';
import { buildDeterministicMutationManifest } from './mutation-manifest.js';
import { MAX_PLAN_STEPS, normalizeSlaLevel, resolveAgenticPlanValue } from './plan-resolver.js';
import { normalizeToolName } from './policy-helpers.js';
import {
  compactReplayValue,
  sanitizeReplayValue,
  sha256,
  stableStringify,
} from './replay-sanitizer.js';

/**
 * Build `simulateAgenticPlan` for one server instance.
 *
 * @param {{
 *   inferPolicyDomain: (toolName: string) => string,
 *   buildPlanStepRouting: (step: {tool: string, params?: unknown, slaLevel?: string | null}) => object,
 *   getToolRuntimeMeta: (toolName: string) => object,
 *   evaluatePolicy: (toolName: string, params: unknown, extra: object, policyDomain?: string | null) => Promise<object>,
 *   checkPermission: (toolName: string, params: unknown) => Promise<object>,
 *   getAgenticToolPricing: (toolName: string) => Promise<object | null>,
 * }} deps
 * @returns {(input: {steps: Array<object>, slaLevel?: string | null, costBudget?: unknown}) => Promise<object>}
 */
export function createSimulateAgenticPlan({
  inferPolicyDomain,
  buildPlanStepRouting,
  getToolRuntimeMeta,
  evaluatePolicy,
  checkPermission,
  getAgenticToolPricing,
}) {
  const simulateAgenticPlan = async ({ steps, slaLevel = null, costBudget }) => {
    const normalizedSlaLevel = normalizeSlaLevel(slaLevel);
    const costBudgetLimits = normalizeCostBudget(costBudget);
    let budgetExceeded = false;
    const budgetViolations = [];
    const sequence = Array.isArray(steps) ? steps : [];
    const normalizedSteps = sequence
      .map((step) => step || {})
      .map((step, index) => {
        const resolvedToolName = normalizeToolName(typeof step?.tool === 'string' ? step.tool : '');
        const rawParams = step.params && typeof step.params === 'object' ? step.params : {};
        const policyDomain = step.policyDomain || inferPolicyDomain(resolvedToolName);
        return {
          index,
          tool: resolvedToolName,
          params: rawParams,
          policyDomain,
        };
      });

    if (normalizedSteps.length > MAX_PLAN_STEPS) {
      return {
        generatedAt: new Date().toISOString(),
        engine: 'stateset-icommerce',
        tool: 'agentic_plan',
        executable: false,
        slaLevel: normalizedSlaLevel,
        totalSteps: normalizedSteps.length,
        failedSteps: 1,
        costSummary: null,
        outcomes: [
          {
            index: 0,
            tool: 'agentic_plan',
            status: 'invalid',
            error: `agentic_plan currently supports at most ${MAX_PLAN_STEPS} steps.`,
            runtime: {
              policyDomain: 'agentic',
              sideEffect: 'write',
              compensations: [],
              idempotent: false,
            },
            simulation: true,
            params: compactReplayValue({
              maxSteps: normalizedSteps.length,
              limit: MAX_PLAN_STEPS,
            }),
            paramsHash: replayEventHash({
              maxSteps: normalizedSteps.length,
              limit: MAX_PLAN_STEPS,
            }),
            result: null,
            resultHash: null,
          },
        ],
        budgetExceeded: false,
        budgetViolations: [],
        costBudget: costBudgetLimits,
        planSignature: null,
      };
    }

    const outcomes = [];
    let executable = true;
    const costSummary = createCostSummary('simulate');
    const resolvedPlanBlueprint = [];
    const executionContext = {
      steps: [],
      latest: null,
      byTool: {},
      sla: { level: normalizedSlaLevel },
    };

    for (const step of normalizedSteps) {
      const resolvedParamsResult = resolveAgenticPlanValue(
        step.params,
        executionContext,
        `steps.${step.index}.params`,
      );
      const effectiveParams =
        resolvedParamsResult.unresolved.length > 0 ? step.params : resolvedParamsResult.value;
      const stepTemplate = {
        index: step.index,
        tool: step.tool,
        policyDomain: step.policyDomain,
        params: compactReplayValue(effectiveParams),
      };
      const stepRouting = buildPlanStepRouting({
        tool: step.tool,
        params: effectiveParams,
        slaLevel: normalizedSlaLevel,
      });
      resolvedPlanBlueprint.push(stepTemplate);
      const stepSignature = sha256(stableStringify(stepTemplate));
      if (!step.tool) {
        const missing = {
          index: step.index,
          tool: step.tool,
          status: 'invalid',
          error: 'Step.tool is required',
          routing: stepRouting,
          policy: null,
          permission: null,
          treasury: null,
          stepSignature,
          simulation: true,
        };
        executable = false;
        outcomes.push(missing);
        executionContext.steps[step.index] = {
          ...stepTemplate,
          routing: stepRouting,
          status: missing.status,
          result: null,
          error: missing.error,
        };
        executionContext.latest = executionContext.steps[step.index];
        continue;
      }

      if (resolvedParamsResult.unresolved.length > 0) {
        const unresolvedResult = {
          index: step.index,
          tool: step.tool,
          status: 'invalid',
          error: `Unresolved plan parameter reference(s): ${resolvedParamsResult.unresolved.join(', ')}`,
          routing: stepRouting,
          policy: null,
          permission: null,
          treasury: null,
          stepSignature,
          simulation: true,
          notes: {
            unresolvedParams: resolvedParamsResult.unresolved,
            availableContext: {
              latestStep: executionContext.latest ? executionContext.latest.index : null,
              stepsAvailable: executionContext.steps.length,
            },
          },
        };
        executable = false;
        outcomes.push(unresolvedResult);
        executionContext.steps[step.index] = {
          ...stepTemplate,
          routing: stepRouting,
          status: unresolvedResult.status,
          result: null,
          error: unresolvedResult.error,
        };
        executionContext.latest = executionContext.steps[step.index];
        executionContext.byTool[step.tool] = executionContext.steps[step.index];
        continue;
      }

      const meta = getToolRuntimeMeta(step.tool);
      if (meta.permission === 'unknown') {
        const unknown = {
          index: step.index,
          tool: step.tool,
          status: 'invalid',
          error: `Unknown tool '${step.tool}'`,
          routing: stepRouting,
          policy: null,
          permission: null,
          treasury: null,
          runtime: null,
          simulation: true,
          stepSignature,
          ...stepTemplate,
        };
        executable = false;
        outcomes.push(unknown);
        executionContext.steps[step.index] = {
          ...stepTemplate,
          routing: stepRouting,
          status: unknown.status,
          result: null,
          error: unknown.error,
        };
        executionContext.latest = executionContext.steps[step.index];
        executionContext.byTool[step.tool] = executionContext.steps[step.index];
        continue;
      }

      const simulatedRequest = {
        requestId: 'agentic_plan',
        sessionId: 'agentic_plan',
      };
      const policy = await evaluatePolicy(
        step.tool,
        effectiveParams,
        simulatedRequest,
        step.policyDomain,
      );
      const permission = await checkPermission(step.tool, policy?.params || effectiveParams);
      const treasury =
        policy.allowed && permission.allowed ? await getAgenticToolPricing(step.tool) : null;
      let status = !policy?.allowed
        ? 'policy_block'
        : !permission.allowed
          ? permission.preview
            ? 'preview'
            : 'permission_block'
          : 'success';
      let budgetLimit = null;
      let budgetInfo = null;
      let budgetError = null;
      if (status === 'success' && treasury) {
        budgetLimit = resolveCostBudgetLimit(
          costBudgetLimits,
          treasury.chainId,
          treasury.tokenSymbol,
        );
        const treasuryAmount = Number(treasury.amount);
        if (budgetLimit !== null && Number.isFinite(treasuryAmount)) {
          const budgetBucketKey = `${treasury.chainId}:${treasury.tokenSymbol}`;
          const currentTotal = Number(costSummary.totals[budgetBucketKey]?.amount || 0);
          const projectedTotal = currentTotal + treasuryAmount;
          if (
            Number.isFinite(currentTotal) &&
            Number.isFinite(projectedTotal) &&
            projectedTotal > budgetLimit
          ) {
            status = 'treasury_block';
            executable = false;
            budgetExceeded = true;
            budgetInfo = {
              chainId: treasury.chainId,
              tokenSymbol: treasury.tokenSymbol,
              currentTotal,
              projectedTotal,
              budgetLimit,
            };
            budgetError = `Cost budget exceeded for ${treasury.chainId}:${treasury.tokenSymbol}. Estimated total ${projectedTotal} would exceed ${budgetLimit}.`;
            budgetViolations.push({
              step: step.index,
              tool: step.tool,
              ...budgetInfo,
            });
          }
        }
      }

      if (status !== 'success') executable = false;
      if (treasury) {
        const rule = {
          chainId: treasury.chainId,
          tokenSymbol: treasury.tokenSymbol,
          amount: treasury.amount,
        };
        if (budgetLimit !== null) rule.budgetLimit = budgetLimit;
        if (budgetInfo?.projectedTotal !== null && budgetInfo?.projectedTotal !== undefined) {
          rule.projectedTotal = budgetInfo.projectedTotal;
        }
        addCostSummaryEntry(costSummary, {
          stepIndex: step.index,
          tool: step.tool,
          status,
          chainId: treasury.chainId,
          tokenSymbol: treasury.tokenSymbol,
          amount: treasury.amount,
          charged: false,
          blocked: status === 'treasury_block',
          blockedReason: budgetError,
          source: 'simulate',
          rule,
        });
      }

      const outcome = {
        index: step.index,
        tool: step.tool,
        status,
        routing: stepRouting,
        policy: {
          allowed: policy.allowed,
          domain: policy.domain || inferPolicyDomain(step.tool),
          reason: policy.reason || null,
          decisionBundle: policy.policyDecisionBundle || null,
        },
        permission: {
          allowed: permission.allowed,
          preview: permission.preview || false,
          reason: permission.reason || null,
        },
        treasury: treasury
          ? {
              required: true,
              chainId: treasury.chainId,
              tokenSymbol: treasury.tokenSymbol,
              amount: treasury.amount,
            }
          : null,
        replay: {
          paramsHash: replayEventHash(sanitizeReplayValue(effectiveParams)),
          deterministicSignature: sha256(
            stableStringify({
              tool: step.tool,
              policyDomain: step.policyDomain,
              params: sanitizeReplayValue(effectiveParams),
            }),
          ),
          params: compactReplayValue(effectiveParams),
        },
        runtime: {
          policyDomain: meta.policyDomain,
          sideEffect: meta.sideEffect,
          compensations: meta.compensations,
          idempotent: meta.idempotent,
        },
        mutationManifest: buildDeterministicMutationManifest({
          toolName: step.tool,
          params: effectiveParams || {},
          policy,
          permission,
          runtimeMeta: meta,
          phase: 'simulate',
        }),
        stepSignature,
        simulation: true,
        error: budgetError || null,
        params: compactReplayValue(effectiveParams),
        paramsHash: replayEventHash(effectiveParams || {}),
        notes: budgetInfo
          ? {
              budget: budgetInfo,
            }
          : null,
      };
      outcomes.push(outcome);
      executionContext.steps[step.index] = {
        ...stepTemplate,
        routing: stepRouting,
        status,
        result: compactReplayValue({ status: outcome.status, ...outcome.treasury }),
        error:
          status === 'success' ? null : outcome.error || permission.reason || policy.reason || null,
      };
      executionContext.latest = executionContext.steps[step.index];
      executionContext.byTool[step.tool] = executionContext.steps[step.index];
    }

    const planSignature = replayEventHash(
      stableStringify({
        steps: resolvedPlanBlueprint,
        options: { mode: 'simulate', slaLevel: normalizedSlaLevel, costBudget: costBudgetLimits },
      }),
    );

    return {
      generatedAt: new Date().toISOString(),
      engine: 'stateset-icommerce',
      tool: 'agentic_plan',
      executable,
      totalSteps: normalizedSteps.length,
      failedSteps: outcomes.filter((entry) => entry.status !== 'success').length,
      budgetExceeded,
      budgetViolations,
      slaLevel: normalizedSlaLevel,
      costBudget: costBudgetLimits,
      costSummary,
      outcomes,
      planSignature,
    };
  };

  return simulateAgenticPlan;
}

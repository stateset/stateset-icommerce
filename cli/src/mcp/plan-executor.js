// `agentic_execute_plan` — multi-step plan execution for the MCP orchestrator.
//
// For each step: resolve `$plan.*` references, route, pre-check the cost
// budget, execute via `executeToolStepInPlan`, record a replay event, and
// decide whether to continue. On failure the rollback phase
// (`./plan-rollback.js`) compensates completed steps. A final replay event
// summarises the run with plan + execution signatures.
//
// Extracted from mcp-server.js (pure move — no behaviour change).

import { randomUUID } from 'node:crypto';
import { replayEventHash } from './audit-envelope.js';
import { AGENTIC_COMPENSATION_HINTS } from './compensation.js';
import {
  addCostSummaryEntry,
  createCostSummary,
  normalizeCostBudget,
  resolveCostBudgetLimit,
} from './cost-budget.js';
import { MAX_PLAN_STEPS, normalizeSlaLevel, resolveAgenticPlanValue } from './plan-resolver.js';
import { normalizeToolName } from './policy-helpers.js';
import { compactReplayValue, stableStringify } from './replay-sanitizer.js';

/**
 * Build `executeAgenticPlan` for one server instance.
 *
 * @param {{
 *   inferPolicyDomain: (toolName: string) => string,
 *   getToolRuntimeMeta: (toolName: string) => object,
 *   buildPlanStepRouting: (step: {tool: string, params?: unknown, slaLevel?: string | null}) => object,
 *   getAgenticToolPricing: (toolName: string) => Promise<object | null>,
 *   executeToolStepInPlan: (input: object) => Promise<object>,
 *   addAgenticReplayEvent: (event: object) => Promise<unknown>,
 *   runPlanRollback: (input: object) => Promise<object | null>,
 * }} deps
 * @returns {(input: {
 *   steps: Array<object>,
 *   dryRun?: boolean,
 *   stopOnFailure?: boolean,
 *   rollbackOnFailure?: boolean,
 *   requestId?: string | null,
 *   sessionId?: string | null,
 *   slaLevel?: string | null,
 *   costBudget?: unknown,
 * }) => Promise<object>}
 */
export function createExecuteAgenticPlan({
  inferPolicyDomain,
  getToolRuntimeMeta,
  buildPlanStepRouting,
  getAgenticToolPricing,
  executeToolStepInPlan,
  addAgenticReplayEvent,
  runPlanRollback,
}) {
  const executeAgenticPlan = async ({
    steps,
    dryRun = true,
    stopOnFailure = true,
    rollbackOnFailure = true,
    requestId = null,
    sessionId = null,
    slaLevel = null,
    costBudget = null,
  }) => {
    const normalizedSlaLevel = normalizeSlaLevel(slaLevel);
    const costBudgetLimits = normalizeCostBudget(costBudget);
    const normalizedSteps = (Array.isArray(steps) ? steps : []).map((step, index) => {
      const toolName = typeof step?.tool === 'string' ? step.tool : '';
      const resolvedToolName = normalizeToolName(toolName);
      const params = step?.params && typeof step?.params === 'object' ? step.params : {};
      const resolvedPolicyDomain = step?.policyDomain || inferPolicyDomain(resolvedToolName);
      return {
        index,
        tool: resolvedToolName,
        params,
        policyDomain: resolvedPolicyDomain,
      };
    });

    const executionRequestId = requestId || randomUUID();
    const executionSessionId = sessionId || executionRequestId;

    if (normalizedSteps.length > MAX_PLAN_STEPS) {
      return {
        generatedAt: new Date().toISOString(),
        engine: 'stateset-icommerce',
        tool: 'agentic_execute_plan',
        requestId: executionRequestId,
        sessionId: executionSessionId,
        dryRun,
        stopOnFailure,
        rollbackOnFailure,
        slaLevel: normalizedSlaLevel,
        totalSteps: normalizedSteps.length,
        completedSteps: 0,
        failedSteps: 1,
        finalStatus: 'failed',
        steps: [
          {
            index: 0,
            tool: 'agentic_execute_plan',
            status: 'invalid',
            error: `agentic_execute_plan currently supports at most ${MAX_PLAN_STEPS} steps.`,
            runtime: {
              policyDomain: 'agentic',
              sideEffect: 'write',
              compensations: [],
              idempotent: false,
            },
            elapsedMs: 0,
            simulation: false,
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
        rollback: null,
        planSignature: null,
        executionSignature: null,
        costSummary: null,
        costBudget: costBudgetLimits,
        budgetExceeded: false,
        budgetViolations: [],
      };
    }

    const stepResults = [];
    const executedForRollback = [];
    const resolvedPlanBlueprint = [];
    const costSummary = createCostSummary('execute');
    let budgetExceeded = false;
    const budgetViolations = [];
    const executionStartedAt = Date.now();
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
      const resolvedParams = resolvedParamsResult.unresolved.length
        ? null
        : resolvedParamsResult.value;
      const effectiveParams =
        resolvedParamsResult.unresolved.length > 0 ? step.params : resolvedParams;
      const meta = getToolRuntimeMeta(step.tool);
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
      const stepSignature = replayEventHash(stableStringify(stepTemplate));
      const resolvedPlanSignature = replayEventHash(
        stableStringify({
          steps: resolvedPlanBlueprint,
          options: {
            dryRun,
            stopOnFailure,
            rollbackOnFailure,
            slaLevel: normalizedSlaLevel,
            costBudget: costBudgetLimits,
          },
        }),
      );
      let budgetPricing = null;
      let budgetLimit = null;
      let budgetInfo = null;
      let budgetError = null;
      if (resolvedParamsResult.unresolved.length === 0) {
        budgetPricing = await getAgenticToolPricing(step.tool);
        if (budgetPricing) {
          budgetLimit = resolveCostBudgetLimit(
            costBudgetLimits,
            budgetPricing.chainId,
            budgetPricing.tokenSymbol,
          );
          const parsedAmount = Number(budgetPricing.amount);
          if (budgetLimit !== null && Number.isFinite(parsedAmount)) {
            const bucketKey = `${budgetPricing.chainId}:${budgetPricing.tokenSymbol}`;
            const currentTotal = Number(costSummary.totals[bucketKey]?.amount || 0);
            const projectedTotal = currentTotal + parsedAmount;
            if (
              Number.isFinite(currentTotal) &&
              Number.isFinite(projectedTotal) &&
              projectedTotal > budgetLimit
            ) {
              budgetExceeded = true;
              budgetError = `Cost budget exceeded for ${budgetPricing.chainId}:${budgetPricing.tokenSymbol}. Estimated total ${projectedTotal} would exceed ${budgetLimit}.`;
              budgetInfo = {
                chainId: budgetPricing.chainId,
                tokenSymbol: budgetPricing.tokenSymbol,
                currentTotal,
                projectedTotal,
                budgetLimit,
                amount: parsedAmount,
              };
              budgetViolations.push({
                step: step.index,
                tool: step.tool,
                ...budgetInfo,
              });
            }
          }
        }
      }

      let outcome;
      if (resolvedParamsResult.unresolved.length > 0) {
        outcome = {
          index: step.index,
          tool: step.tool,
          status: 'invalid',
          routing: stepRouting,
          elapsedMs: 0,
          policy: null,
          permission: null,
          charge: null,
          params: compactReplayValue(step.params),
          paramsHash: replayEventHash(step.params || {}),
          result: null,
          resultHash: null,
          runtime: {
            policyDomain: meta?.policyDomain || step.policyDomain || inferPolicyDomain(step.tool),
            sideEffect: meta.sideEffect || 'write',
            compensations: meta.compensations || [],
            idempotent: meta.idempotent || false,
          },
          simulation: false,
          error: `Unresolved plan parameter reference(s): ${resolvedParamsResult.unresolved.join(', ')}`,
          notes: {
            unresolvedParams: resolvedParamsResult.unresolved,
            availableContext: {
              latestStep: executionContext.latest ? executionContext.latest.index : null,
              stepsAvailable: executionContext.steps.length,
            },
          },
          requestId: executionRequestId,
        };
      } else if (budgetInfo) {
        outcome = {
          index: step.index,
          tool: step.tool,
          status: 'treasury_block',
          routing: stepRouting,
          elapsedMs: 0,
          policy: null,
          permission: null,
          charge: {
            charged: false,
            blocked: true,
            reason: budgetError,
            rule: {
              chainId: budgetPricing?.chainId || null,
              tokenSymbol: budgetPricing?.tokenSymbol || null,
              amount: budgetPricing?.amount || null,
              budgetLimit,
              currentTotal: budgetInfo.currentTotal,
              projectedTotal: budgetInfo.projectedTotal,
            },
          },
          params: compactReplayValue(effectiveParams),
          paramsHash: replayEventHash(effectiveParams || {}),
          result: null,
          resultHash: null,
          runtime: {
            policyDomain: meta?.policyDomain || step.policyDomain || inferPolicyDomain(step.tool),
            sideEffect: meta.sideEffect || 'write',
            compensations: meta.compensations || [],
            idempotent: meta.idempotent || false,
          },
          simulation: false,
          error: budgetError,
          requestId: executionRequestId,
          notes: {
            budget: budgetInfo,
          },
        };
      } else {
        outcome = await executeToolStepInPlan({
          toolName: step.tool,
          params: resolvedParams,
          policyDomain: step.policyDomain,
          requestId: executionRequestId,
          sessionId: executionSessionId,
          dryRun,
          stepIndex: step.index,
          includeHooks: true,
        });
      }

      outcome.routing = outcome.routing || stepRouting;
      outcome.stepSignature = stepSignature;
      if (outcome?.charge?.rule) {
        addCostSummaryEntry(costSummary, {
          stepIndex: step.index,
          tool: outcome.tool,
          status: outcome.status,
          chainId: outcome?.charge?.rule?.chainId || null,
          tokenSymbol: outcome?.charge?.rule?.tokenSymbol || null,
          amount: outcome?.charge?.rule?.amount || null,
          charged: Boolean(outcome?.charge?.charged),
          blocked: Boolean(outcome?.charge?.blocked),
          blockedReason: outcome?.charge?.reason || null,
          source: 'execute',
          rule: outcome?.charge?.rule || null,
        });
      }

      stepResults.push({
        ...outcome,
        rollbackTarget: AGENTIC_COMPENSATION_HINTS[step.tool] || [],
      });

      executionContext.steps[step.index] = {
        index: step.index,
        tool: step.tool,
        policyDomain: step.policyDomain,
        params: compactReplayValue(effectiveParams),
        routing: stepRouting,
        status: outcome.status,
        result: compactReplayValue(outcome.result),
        error: outcome.error || null,
      };
      executionContext.latest = executionContext.steps[step.index];
      if (step.tool) {
        executionContext.byTool[step.tool] = executionContext.steps[step.index];
      }

      await addAgenticReplayEvent({
        eventId: randomUUID(),
        tool: 'agentic_execute_plan',
        status: outcome.status,
        requestId: executionRequestId,
        sessionId: executionSessionId,
        policyDomain: step.policyDomain,
        occurredAt: new Date().toISOString(),
        elapsedMs: outcome.elapsedMs || 0,
        params: compactReplayValue({
          step: outcome.tool,
          params: effectiveParams,
          resolved: resolvedParamsResult.unresolved.length === 0,
          source: { step: step.index },
        }),
        paramsHash: replayEventHash(effectiveParams || {}),
        result: compactReplayValue(outcome),
        resultHash: replayEventHash(outcome),
        policy: compactReplayValue(outcome.policy || null),
        permission: compactReplayValue(outcome.permission || null),
        charge: compactReplayValue(outcome.charge || null),
        error: outcome.error || null,
        planSignature: resolvedPlanSignature,
        notes: {
          dryRun,
          stopOnFailure,
          rollbackOnFailure,
          slaLevel: normalizedSlaLevel,
          executedBy: 'agentic_execute_plan',
          index: step.index,
          sourceStep: step.tool,
          stepSignature,
          routing: outcome.routing || null,
          mutationManifest: outcome?.mutationManifest || null,
        },
        source: 'agentic_execute_plan',
        agentic: true,
      });

      if (outcome.status === 'success' || outcome.status === 'dry_run_success') {
        executedForRollback.push({
          step,
          outcome,
        });
      }

      const failed = !(
        outcome.status === 'success' ||
        outcome.status === 'dry_run_success' ||
        outcome.status === 'rollback_success'
      );
      if (failed && stopOnFailure) {
        break;
      }
      if (dryRun && outcome.status !== 'dry_run_success') {
        break;
      }
    }

    const planSignature = replayEventHash(
      stableStringify({
        steps: resolvedPlanBlueprint,
        options: {
          dryRun,
          stopOnFailure,
          rollbackOnFailure,
          slaLevel: normalizedSlaLevel,
          costBudget: costBudgetLimits,
        },
      }),
    );
    const executionSignature = replayEventHash(stableStringify(stepResults));

    const finalStatus =
      stepResults.some((entry) => entry.status === 'error') ||
      stepResults.some((entry) => entry.status === 'dry_run_blocked') ||
      stepResults.some((entry) => entry.status === 'preview') ||
      stepResults.some((entry) => entry.status === 'treasury_block') ||
      stepResults.some((entry) => entry.status === 'permission_block') ||
      stepResults.some((entry) => entry.status === 'policy_block') ||
      stepResults.some((entry) => entry.status === 'blocked') ||
      stepResults.some((entry) => entry.status === 'rollback_failed')
        ? 'failed'
        : stepResults.some((entry) => entry.status === 'dry_run_success')
          ? 'dry_run'
          : 'success';

    const rollback = await runPlanRollback({
      dryRun,
      rollbackOnFailure,
      finalStatus,
      executedForRollback,
      costSummary,
      executionRequestId,
      executionSessionId,
      planSignature,
      normalizedSlaLevel,
    });

    const completedSteps = stepResults.filter((entry) =>
      ['success', 'dry_run_success', 'rollback_success'].includes(entry.status),
    ).length;
    const failedSteps = stepResults.filter(
      (entry) => !['success', 'dry_run_success', 'rollback_success'].includes(entry.status),
    ).length;

    await addAgenticReplayEvent({
      eventId: randomUUID(),
      tool: 'agentic_execute_plan',
      status: finalStatus,
      requestId: executionRequestId,
      sessionId: executionSessionId,
      policyDomain: 'agentic',
      occurredAt: new Date().toISOString(),
      elapsedMs: Date.now() - executionStartedAt,
      params: compactReplayValue({
        dryRun,
        stopOnFailure,
        rollbackOnFailure,
        slaLevel: normalizedSlaLevel,
        costBudget: costBudgetLimits,
        totalSteps: normalizedSteps.length,
        completedSteps,
        failedSteps,
      }),
      paramsHash: replayEventHash({
        dryRun,
        stopOnFailure,
        rollbackOnFailure,
        slaLevel: normalizedSlaLevel,
        costBudget: costBudgetLimits,
        totalSteps: normalizedSteps.length,
        completedSteps,
        failedSteps,
      }),
      result: compactReplayValue({
        finalStatus,
        stepStatuses: stepResults.map((entry) => entry.status),
        executionSignature,
        planSignature,
        rollback: rollback
          ? { attempted: rollback.attempted, fullyReverted: rollback.fullyReverted }
          : null,
        slaLevel: normalizedSlaLevel,
        budgetExceeded,
        costBudget: costBudgetLimits,
        costSummary: {
          mode: costSummary.mode,
          totalEntries: costSummary.totalEntries,
          chargedEntries: costSummary.chargedEntries,
          blockedEntries: costSummary.blockedEntries,
        },
      }),
      resultHash: replayEventHash({
        finalStatus,
        stepStatuses: stepResults.map((entry) => entry.status),
        executionSignature,
        slaLevel: normalizedSlaLevel,
        costBudget: costBudgetLimits,
        budgetExceeded,
        costSummary: {
          mode: costSummary.mode,
          totalEntries: costSummary.totalEntries,
          chargedEntries: costSummary.chargedEntries,
          blockedEntries: costSummary.blockedEntries,
        },
      }),
      policy: null,
      permission: null,
      charge: null,
      error: null,
      notes: {
        final: true,
        planSignature,
        executionSignature,
        slaLevel: normalizedSlaLevel,
        costSummary: {
          mode: costSummary.mode,
          totalEntries: costSummary.totalEntries,
          chargedEntries: costSummary.chargedEntries,
          blockedEntries: costSummary.blockedEntries,
          budgetExceeded,
        },
        rollback: rollback
          ? { attempted: rollback.attempted, fullyReverted: rollback.fullyReverted }
          : null,
      },
      executionSignature,
      source: 'agentic_execute_plan',
      agentic: true,
    });

    return {
      generatedAt: new Date().toISOString(),
      engine: 'stateset-icommerce',
      tool: 'agentic_execute_plan',
      requestId: executionRequestId,
      sessionId: executionSessionId,
      dryRun,
      stopOnFailure,
      rollbackOnFailure,
      slaLevel: normalizedSlaLevel,
      totalSteps: normalizedSteps.length,
      completedSteps,
      failedSteps,
      finalStatus,
      steps: stepResults,
      rollback,
      planSignature,
      executionSignature,
      costBudget: costBudgetLimits,
      budgetExceeded,
      budgetViolations,
      costSummary,
    };
  };

  return executeAgenticPlan;
}

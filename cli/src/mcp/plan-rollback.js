// Rollback (compensation) phase of `agentic_execute_plan`.
//
// When a non-dry-run plan fails and `rollbackOnFailure` is set, every
// successfully executed step that has compensation hints is reverted in
// reverse order. Each compensation attempt is recorded in the replay log.
//
// Extracted from mcp-server.js (pure move — no behaviour change).

import { randomUUID } from 'node:crypto';
import { replayEventHash } from './audit-envelope.js';
import { AGENTIC_COMPENSATION_HINTS, buildCompensationParams } from './compensation.js';
import { addCostSummaryEntry } from './cost-budget.js';
import { compactReplayValue } from './replay-sanitizer.js';

/**
 * Build `runPlanRollback` for one server instance.
 *
 * @param {{
 *   toolDefsByName: Map<string, object>,
 *   inferPolicyDomain: (toolName: string) => string,
 *   executeToolStepInPlan: (input: object) => Promise<object>,
 *   addAgenticReplayEvent: (event: object) => Promise<unknown>,
 * }} deps
 * @returns {(input: {
 *   dryRun: boolean,
 *   rollbackOnFailure: boolean,
 *   finalStatus: string,
 *   executedForRollback: Array<{step: object, outcome: object}>,
 *   costSummary: object,
 *   executionRequestId: string,
 *   executionSessionId: string,
 *   planSignature: string,
 *   normalizedSlaLevel: string | null,
 * }) => Promise<{attempted: number, steps: Array<object>, fullyReverted: boolean} | null>}
 *   resolves to `null` when no rollback was attempted.
 */
export function createRunPlanRollback({
  toolDefsByName: TOOL_DEFS_BY_NAME,
  inferPolicyDomain,
  executeToolStepInPlan,
  addAgenticReplayEvent,
}) {
  return async function runPlanRollback({
    dryRun,
    rollbackOnFailure,
    finalStatus,
    executedForRollback,
    costSummary,
    executionRequestId,
    executionSessionId,
    planSignature,
    normalizedSlaLevel,
  }) {
    let rollback = null;
    if (!dryRun && rollbackOnFailure && finalStatus === 'failed') {
      const rollbackCandidates = executedForRollback.filter((entry) => {
        return (AGENTIC_COMPENSATION_HINTS[entry.step.tool] || []).length > 0;
      });

      const rollbackSteps = [];
      for (const completed of rollbackCandidates.reverse()) {
        const compensationTools = AGENTIC_COMPENSATION_HINTS[completed.step.tool] || [];
        const availableCompensationTools = compensationTools.filter((candidate) =>
          TOOL_DEFS_BY_NAME.has(candidate),
        );
        let compensated = false;
        let lastCompensationResult = {
          status: 'rollback_failed',
          reason: 'No compensation tool candidates',
        };
        let lastCompensationParams = null;
        for (const compensationTool of availableCompensationTools) {
          const compensationParams = buildCompensationParams(
            compensationTool,
            completed.step.params,
            completed.outcome.result,
          );
          lastCompensationParams = compensationParams;
          if (!compensationParams) {
            lastCompensationResult = {
              status: 'rollback_failed',
              reason: 'No compensation parameters',
              tool: compensationTool,
            };
            continue;
          }
          const compensationResult = await executeToolStepInPlan({
            toolName: compensationTool,
            params: compensationParams,
            policyDomain: inferPolicyDomain(compensationTool),
            requestId: executionRequestId,
            sessionId: executionSessionId,
            dryRun: false,
            stepIndex: completed.step.index,
            includeHooks: true,
            isRollback: true,
          });
          lastCompensationResult = compensationResult;
          if (compensationResult?.charge?.rule) {
            addCostSummaryEntry(costSummary, {
              stepIndex: completed.step.index,
              tool: compensationResult.tool,
              status: compensationResult.status,
              chainId: compensationResult?.charge?.rule?.chainId || null,
              tokenSymbol: compensationResult?.charge?.rule?.tokenSymbol || null,
              amount: compensationResult?.charge?.rule?.amount || null,
              charged: Boolean(compensationResult?.charge?.charged),
              blocked: Boolean(compensationResult?.charge?.blocked),
              blockedReason: compensationResult?.charge?.reason || null,
              source: 'rollback',
              rule: compensationResult?.charge?.rule || null,
            });
          }
          if (
            compensationResult.status === 'success' ||
            compensationResult.status === 'rollback_success'
          ) {
            compensated = true;
            break;
          }
        }
        rollbackSteps.push({
          ...lastCompensationResult,
          source: completed.step.tool,
          compensationTools: availableCompensationTools,
          compensationParams: lastCompensationParams,
        });
        await addAgenticReplayEvent({
          eventId: randomUUID(),
          tool: 'agentic_execute_plan',
          status: lastCompensationResult?.status || 'rollback_failed',
          requestId: executionRequestId,
          sessionId: executionSessionId,
          policyDomain: inferPolicyDomain(lastCompensationResult?.tool || completed.step.tool),
          occurredAt: new Date().toISOString(),
          elapsedMs: lastCompensationResult?.elapsedMs || 0,
          params: compactReplayValue({
            source: completed.step.tool,
            compensationTool: lastCompensationResult?.tool,
            compensationParams: lastCompensationParams,
          }),
          paramsHash: replayEventHash({
            source: completed.step.tool,
            compensationTool: lastCompensationResult?.tool,
            compensationParams: lastCompensationParams,
          }),
          result: compactReplayValue(lastCompensationResult),
          resultHash: replayEventHash(lastCompensationResult || {}),
          policy: compactReplayValue(lastCompensationResult?.policy || null),
          permission: compactReplayValue(lastCompensationResult?.permission || null),
          charge: compactReplayValue(lastCompensationResult?.charge || null),
          error: lastCompensationResult?.error || null,
          planSignature,
          notes: {
            phase: 'rollback',
            compensated,
            slaLevel: normalizedSlaLevel,
            index: completed.step.index,
            source: completed.step.tool,
          },
          source: 'agentic_execute_plan',
          agentic: true,
        });
        if (compensated) continue;
      }
      rollback = {
        attempted: rollbackCandidates.length,
        steps: rollbackSteps,
        fullyReverted: rollbackSteps.every(
          (step) => step.status === 'success' || step.status === 'rollback_success',
        ),
      };
    }
    return rollback;
  };
}

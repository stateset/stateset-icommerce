/**
 * Mutation tool simulators — dry-run a single write tool, or replay a
 * previously-recorded mutation event for determinism verification.
 *
 * `simulateMutationToolCall` invokes the plan step executor with `dryRun:
 * true` and emits an `agentic_simulate_mutation` audit event.
 *
 * `replayMutationToolCall` finds a prior tool-call event via the replay log,
 * re-executes it (defaults to dry-run), and reports paramsHash / resultHash
 * match status for tamper-detection + determinism checks.
 *
 * Pulled out of mcp-server.js so the orchestrator stays focused on
 * orchestration. The injected deps shape lets these run against mocks in
 * unit tests.
 */

import { randomUUID } from 'node:crypto';

import { compactReplayValue } from './replay-sanitizer.js';
import { replayEventHash } from './audit-envelope.js';
import { normalizeToolName } from './policy-helpers.js';

/**
 * @typedef {Object} MutationSimulatorDeps
 * @property {(toolName: string) => Object} getToolRuntimeMeta
 * @property {(toolName: string) => string} inferPolicyDomain
 * @property {(args: Object) => Promise<Object>} executeToolStepInPlan
 * @property {(event: Object) => Promise<void>} addAgenticReplayEvent
 * @property {(options?: Object) => Promise<{ events: Array<Object> }>} listAgenticReplayEvents
 */

/**
 * Build `simulateMutationToolCall` — a single-write-tool dry-run gateway.
 *
 * @param {MutationSimulatorDeps} deps
 */
export function createSimulateMutationToolCall({
  getToolRuntimeMeta,
  inferPolicyDomain,
  executeToolStepInPlan,
  addAgenticReplayEvent,
}) {
  return async function simulateMutationToolCall({
    tool,
    params = {},
    policyDomain = null,
    requestId = null,
    sessionId = null,
    includeHooks = false,
  }) {
    const targetTool = normalizeToolName(tool);
    const runtime = getToolRuntimeMeta(targetTool);
    if (!targetTool) {
      return {
        success: false,
        error: 'tool is required',
      };
    }
    if (runtime.permission === 'unknown') {
      return {
        success: false,
        error: `Unknown tool '${targetTool}'`,
      };
    }
    if (runtime.sideEffect !== 'write') {
      return {
        success: false,
        error: `Tool '${targetTool}' is read-only. Use agentic_plan for read tool simulation.`,
      };
    }

    const simulationRequestId = requestId || randomUUID();
    const simulationSessionId = sessionId || simulationRequestId;
    const outcome = await executeToolStepInPlan({
      toolName: targetTool,
      params,
      policyDomain: policyDomain || inferPolicyDomain(targetTool),
      requestId: simulationRequestId,
      sessionId: simulationSessionId,
      dryRun: true,
      stepIndex: 0,
      includeHooks,
    });

    const replayContract = {
      generatedAt: new Date().toISOString(),
      source: 'agentic_simulate_mutation',
      targetTool,
      policyDomain: policyDomain || inferPolicyDomain(targetTool),
      requestId: simulationRequestId,
      sessionId: simulationSessionId,
      runtime,
      simulation: outcome,
      simulationHash: replayEventHash(outcome),
      deterministicSignature: replayEventHash({
        tool: targetTool,
        params: compactReplayValue(params || {}),
        status: outcome.status,
        paramsHash: outcome.paramsHash,
      }),
    };

    await addAgenticReplayEvent({
      eventId: randomUUID(),
      tool: 'agentic_simulate_mutation',
      status: outcome.status,
      requestId: simulationRequestId,
      sessionId: simulationSessionId,
      policyDomain: policyDomain || inferPolicyDomain(targetTool),
      occurredAt: new Date().toISOString(),
      elapsedMs: outcome.elapsedMs || 0,
      params: compactReplayValue({
        tool: targetTool,
        params,
        includeHooks,
      }),
      paramsHash: replayEventHash({ tool: targetTool, params }),
      result: compactReplayValue(replayContract),
      resultHash: replayEventHash(replayContract),
      policy: compactReplayValue(outcome.policy || null),
      permission: compactReplayValue(outcome.permission || null),
      charge: compactReplayValue(outcome.charge || null),
      error: outcome.error || null,
      notes: {
        simulation: true,
        targetTool,
      },
      source: 'agentic_simulate_mutation',
      agentic: true,
    });

    return {
      success: true,
      generatedAt: replayContract.generatedAt,
      engine: 'stateset-icommerce',
      tool: 'agentic_simulate_mutation',
      requestId: simulationRequestId,
      sessionId: simulationSessionId,
      targetTool,
      outcome,
      replayContract,
    };
  };
}

/**
 * Build `replayMutationToolCall` — re-execute a recorded mutation event for
 * determinism verification. Defaults to dry-run so it's safe by default.
 *
 * @param {MutationSimulatorDeps} deps
 */
export function createReplayMutationToolCall({
  getToolRuntimeMeta,
  inferPolicyDomain,
  executeToolStepInPlan,
  addAgenticReplayEvent,
  listAgenticReplayEvents,
}) {
  return async function replayMutationToolCall({
    eventId = null,
    requestId = null,
    tool = null,
    dryRun = true,
    includeHooks = false,
    sessionId = null,
  }) {
    const replayEvents = await listAgenticReplayEvents({
      limit: 200,
      eventId,
      requestId,
      tool: tool ? normalizeToolName(tool) : null,
    });
    const sourceEvent = (replayEvents.events || []).find((event) => {
      if (!event?.tool || event.tool.startsWith('agentic_')) return false;
      const runtime = getToolRuntimeMeta(event.tool);
      if (runtime.permission === 'unknown' || runtime.sideEffect !== 'write') return false;
      return event.params && typeof event.params === 'object';
    });

    if (!sourceEvent) {
      return {
        success: false,
        error: 'No replayable mutation event found for the provided filters.',
        filters: {
          eventId,
          requestId,
          tool: tool || null,
        },
      };
    }

    const replayRequestId = randomUUID();
    const replaySessionId = sessionId || replayRequestId;
    const replayOutcome = await executeToolStepInPlan({
      toolName: sourceEvent.tool,
      params: sourceEvent.params || {},
      policyDomain: sourceEvent.policyDomain || inferPolicyDomain(sourceEvent.tool),
      requestId: replayRequestId,
      sessionId: replaySessionId,
      dryRun: dryRun !== false,
      stepIndex: 0,
      includeHooks,
    });

    const originalParamsHash =
      sourceEvent.paramsHash || replayEventHash(compactReplayValue(sourceEvent.params || {}));
    const deterministic = {
      paramsMatch: originalParamsHash === replayOutcome.paramsHash,
      resultHashMatch:
        typeof sourceEvent.resultHash === 'string'
          ? sourceEvent.resultHash === replayOutcome.resultHash
          : null,
      originalParamsHash,
      replayParamsHash: replayOutcome.paramsHash,
      originalResultHash: sourceEvent.resultHash || null,
      replayResultHash: replayOutcome.resultHash || null,
    };

    await addAgenticReplayEvent({
      eventId: randomUUID(),
      tool: 'agentic_replay_mutation',
      status: replayOutcome.status,
      requestId: replayRequestId,
      sessionId: replaySessionId,
      policyDomain: sourceEvent.policyDomain || inferPolicyDomain(sourceEvent.tool),
      occurredAt: new Date().toISOString(),
      elapsedMs: replayOutcome.elapsedMs || 0,
      params: compactReplayValue({
        sourceEventId: sourceEvent.eventId || null,
        sourceTool: sourceEvent.tool,
        dryRun: dryRun !== false,
      }),
      paramsHash: replayEventHash({
        sourceEventId: sourceEvent.eventId || null,
        sourceTool: sourceEvent.tool,
        dryRun: dryRun !== false,
      }),
      result: compactReplayValue({
        replayOutcome,
        deterministic,
      }),
      resultHash: replayEventHash({
        replayOutcome,
        deterministic,
      }),
      policy: compactReplayValue(replayOutcome.policy || null),
      permission: compactReplayValue(replayOutcome.permission || null),
      charge: compactReplayValue(replayOutcome.charge || null),
      error: replayOutcome.error || null,
      notes: {
        phase: 'replay',
        sourceEventId: sourceEvent.eventId || null,
        sourceRequestId: sourceEvent.requestId || null,
      },
      source: 'agentic_replay_mutation',
      agentic: true,
    });

    return {
      success: true,
      generatedAt: new Date().toISOString(),
      engine: 'stateset-icommerce',
      tool: 'agentic_replay_mutation',
      requestId: replayRequestId,
      sessionId: replaySessionId,
      sourceEvent: {
        eventId: sourceEvent.eventId || null,
        requestId: sourceEvent.requestId || null,
        tool: sourceEvent.tool,
        occurredAt: sourceEvent.occurredAt || null,
        status: sourceEvent.status || null,
      },
      replay: replayOutcome,
      deterministic,
    };
  };
}

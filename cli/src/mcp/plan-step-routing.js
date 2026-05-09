// Per-plan-step agent-routing decisions for the MCP orchestrator.
//
// When the orchestrator executes an agentic plan step, it asks the
// agent-router which downstream agent should handle the call. The
// answer carries:
//   - the resolved SLA level (or null if none was specified)
//   - the primary agent + score/confidence/level
//   - alternative candidates (lower-scoring agents)
//   - an `ambiguous` flag set when the router can't pick a clear winner
//
// Extracted from mcp-server.js. The routing function itself is injected
// (rather than imported) so tests can hand in a stub and the orchestrator
// can pass `routeToAgentWithConfidence` from `./agent-router.js` without
// this module reaching across the codebase.

import { normalizeSlaLevel } from './plan-resolver.js';
import { compactReplayValue, stableStringify } from './replay-sanitizer.js';

/**
 * Default primary-agent fallback when the router returns no candidate.
 * `customer-service` is the broadest agent and a safe last resort —
 * losing a routing decision should not abort the whole plan.
 */
const DEFAULT_PRIMARY = Object.freeze({
  agent: 'customer-service',
  score: 0,
  confidence: 0,
  level: 'default',
});

/**
 * Build the agent-routing block for a single plan step.
 *
 * @param {{
 *   tool: string,
 *   params?: unknown,
 *   slaLevel?: string,
 * }} step - the plan step to route
 * @param {(intent: string, opts: {slaLevel?: string}) => unknown} routeFn -
 *   the routing function (typically `routeToAgentWithConfidence` from
 *   `./agent-router.js`). Returns a routing decision the orchestrator
 *   can attach to the step.
 * @returns {{
 *   slaLevel: string | null,
 *   primary: { agent: string, score: number, confidence: number, level: string },
 *   alternatives: Array<{ agent: string, score: number, confidence: number, level: string }>,
 *   ambiguous: boolean,
 * }}
 */
export function buildPlanStepRouting({ tool, params, slaLevel }, routeFn) {
  const normalizedSlaLevel = normalizeSlaLevel(slaLevel);
  const routeIntent = `${String(tool || '').replaceAll('_', ' ')} ${stableStringify(compactReplayValue(params || {}))}`;
  const routing = routeFn(routeIntent, {
    slaLevel: normalizedSlaLevel || undefined,
  });
  return {
    slaLevel: routing?.routingContext?.slaLevel || null,
    primary: routing?.primary
      ? {
          agent: routing.primary.agent,
          score: routing.primary.score,
          confidence: routing.primary.confidence,
          level: routing.primary.level,
        }
      : { ...DEFAULT_PRIMARY },
    alternatives: Array.isArray(routing?.alternatives)
      ? routing.alternatives.map((entry) => ({
          agent: entry.agent,
          score: entry.score,
          confidence: entry.confidence,
          level: entry.level,
        }))
      : [],
    ambiguous: Boolean(routing?.ambiguous),
  };
}

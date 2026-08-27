// Static definitions of the "agentic runtime" tool surface — the meta-tools
// AI agents use to introspect, plan, simulate, replay, and delegate. Each
// entry is a pure-data tool descriptor; the `handler` closures take their
// runtime dependencies (the various `get*`, `prepare*`, `execute*`,
// `simulate*`, etc. functions) as destructured parameters, so this array
// has no module-scope coupling and is safe to extract.
//
// Imported by `cli/src/mcp-server.js`, which is the orchestrator that wires
// up the dependencies, registers permissions, and dispatches tool calls.

import { z } from 'zod';

import { SUPPORTED_AGENT_NAMES, SUPPORTED_AGENT_NAMES_DESCRIPTION } from '../agent-catalog.js';
import { AGENTIC_SLA_LEVELS } from './plan-resolver.js';

/**
 * @typedef {Object} AgenticTool
 * @property {string} name
 * @property {string} description
 * @property {Object} inputSchema   Zod input schema record
 * @property {string} permission    `read` | `write`
 * @property {string} policyDomain  Policy domain key (e.g. `agentic`)
 * @property {(args: Object) => Promise<unknown>} handler
 */

/** All agentic-runtime tool definitions. @type {AgenticTool[]} */
export const AGENTIC_RUNTIME_TOOLS = [
  {
    name: 'agentic_runtime_contract',
    description:
      'Return a deterministic runtime contract for AI agents: capabilities, side effects, and replay metadata.',
    inputSchema: {
      tool: z.string().optional().describe('Optional tool name to filter contract metadata'),
      includeLegacyDefaults: z.boolean().optional().default(false),
    },
    permission: 'read',
    policyDomain: 'agentic',
    handler: async ({ params, getAgenticRuntimeContract }) => {
      const payload = getAgenticRuntimeContract({
        tool: params?.tool,
        includeLegacyDefaults: params?.includeLegacyDefaults || false,
      });
      return payload;
    },
  },
  {
    name: 'agentic_tool_catalog',
    description:
      'Return a machine-readable tool catalog with runtime metadata and optional Machine Payments Protocol pricing info.',
    inputSchema: {
      tool: z.string().optional().describe('Optional tool name to filter the catalog'),
      format: z
        .enum(['generic', 'mcp', 'openai'])
        .optional()
        .default('generic')
        .describe('Catalog output format'),
      payableOnly: z
        .boolean()
        .optional()
        .default(false)
        .describe('Only include tools with configured payment pricing'),
    },
    permission: 'read',
    policyDomain: 'agentic',
    handler: async ({ params, getToolCatalog }) => {
      return getToolCatalog({
        tool: params?.tool || null,
        format: params?.format || 'generic',
        payableOnly: params?.payableOnly ?? false,
      });
    },
  },
  {
    name: 'agentic_payment_discovery',
    description:
      'Discover payable MCP tools with Machine Payments Protocol metadata, pricing, and optional OpenAPI output.',
    inputSchema: {
      tool: z.string().optional().describe('Optional tool name to filter discovery output'),
      format: z
        .enum(['json', 'openapi'])
        .optional()
        .default('json')
        .describe('Discovery output format'),
      pricedOnly: z
        .boolean()
        .optional()
        .default(true)
        .describe('Return only tools with configured pricing'),
    },
    permission: 'read',
    policyDomain: 'agentic',
    handler: async ({ params, getPaymentDiscovery }) => {
      return getPaymentDiscovery({
        tool: params?.tool || null,
        format: params?.format || 'json',
        pricedOnly: params?.pricedOnly ?? true,
      });
    },
  },
  {
    name: 'agentic_prepare_payment',
    description:
      'Prepare a Machine Payments Protocol challenge and retry template for a priced MCP tool call.',
    inputSchema: {
      tool: z.string().min(1).describe('Tool name without server prefix'),
      params: z
        .record(z.string(), z.any())
        .optional()
        .default({})
        .describe('Tool parameters to bind into the challenge'),
      requestId: z.string().optional().describe('Optional correlation id'),
      sessionId: z.string().optional().describe('Optional MCP session id'),
      includeSchema: z
        .boolean()
        .optional()
        .default(false)
        .describe('Include the tool input JSON Schema in the response'),
    },
    permission: 'read',
    policyDomain: 'agentic',
    handler: async ({ params, preparePaymentForTool }) => {
      return preparePaymentForTool({
        tool: params?.tool,
        params: params?.params || {},
        requestId: params?.requestId || null,
        sessionId: params?.sessionId || null,
        includeSchema: params?.includeSchema ?? false,
      });
    },
  },
  {
    name: 'agentic_plan',
    description:
      'Evaluate a proposed tool sequence for deterministic simulation: policy, permission, and replayability checks.',
    inputSchema: {
      steps: z.array(
        z.object({
          tool: z.string().describe('Tool name without server prefix'),
          params: z.record(z.string(), z.any()).default({}).describe('Tool parameters'),
          policyDomain: z.string().optional().describe('Optional override policy domain'),
        }),
      ),
      slaLevel: z
        .enum(AGENTIC_SLA_LEVELS)
        .optional()
        .describe('Optional SLA priority for routing-aware planning'),
      costBudget: z
        .record(
          z.string().describe('Currency key (tokenSymbol or chainId:tokenSymbol)'),
          z.union([z.number(), z.string()]),
        )
        .optional()
        .describe('Optional plan-level per-currency cost cap values'),
    },
    permission: 'read',
    policyDomain: 'agentic',
    handler: async ({ params, simulateAgenticPlan }) => {
      return simulateAgenticPlan({
        steps: params?.steps || [],
        slaLevel: params?.slaLevel || null,
        costBudget: params?.costBudget,
      });
    },
  },
  {
    name: 'agentic_simulate_mutation',
    description:
      'Run deterministic dry-run simulation for a mutating tool with policy, permission, rollback, and replay metadata.',
    inputSchema: {
      tool: z.string().min(1).describe('Mutating tool name without server prefix'),
      params: z.record(z.string(), z.any()).default({}).describe('Tool parameters'),
      policyDomain: z.string().optional().describe('Optional override policy domain'),
      requestId: z.string().optional().describe('Optional correlation id'),
      sessionId: z.string().optional().describe('Optional correlation session id'),
      includeHooks: z
        .boolean()
        .optional()
        .default(false)
        .describe('Include before/after hook execution in simulation'),
    },
    permission: 'read',
    policyDomain: 'agentic',
    handler: async ({ params, simulateMutationToolCall }) => {
      return simulateMutationToolCall({
        tool: params?.tool,
        params: params?.params || {},
        policyDomain: params?.policyDomain || null,
        requestId: params?.requestId || null,
        sessionId: params?.sessionId || null,
        includeHooks: params?.includeHooks ?? false,
      });
    },
  },
  {
    name: 'agentic_replay_mutation',
    description:
      'Replay a previously logged mutating tool call from the deterministic replay log, with dry-run by default.',
    inputSchema: {
      eventId: z.string().optional().describe('Replay a specific event id'),
      requestId: z.string().optional().describe('Replay the latest mutation for this request id'),
      tool: z.string().optional().describe('Replay the latest mutation for this tool'),
      dryRun: z
        .boolean()
        .optional()
        .default(true)
        .describe('Dry-run by default. Set false to execute if --apply is enabled'),
      includeHooks: z
        .boolean()
        .optional()
        .default(false)
        .describe('Include before/after hook execution during replay'),
      sessionId: z.string().optional().describe('Optional correlation session id'),
    },
    permission: 'read',
    policyDomain: 'agentic',
    handler: async ({ params, replayMutationToolCall }) => {
      return replayMutationToolCall({
        eventId: params?.eventId || null,
        requestId: params?.requestId || null,
        tool: params?.tool || null,
        dryRun: params?.dryRun ?? true,
        includeHooks: params?.includeHooks ?? false,
        sessionId: params?.sessionId || null,
      });
    },
  },
  {
    name: 'agentic_replay',
    description: 'Read recent deterministic execution events for auditability and replay tooling.',
    inputSchema: {
      limit: z.number().optional().default(20).describe('Max events to return'),
      tool: z.string().optional().describe('Filter by tool name'),
      eventId: z.string().optional().describe('Filter by replay event id'),
      requestId: z.string().optional().describe('Filter by request/session id'),
      sessionId: z.string().optional().describe('Filter by MCP session id'),
      planSignature: z.string().optional().describe('Filter by plan signature'),
      executionSignature: z.string().optional().describe('Filter by execution signature'),
      status: z
        .enum([
          'success',
          'error',
          'blocked',
          'preview',
          'policy_block',
          'permission_block',
          'treasury_block',
          'rollback_success',
          'rollback_failed',
          'dry_run_success',
          'dry_run_blocked',
          'invalid',
        ])
        .optional(),
    },
    permission: 'read',
    policyDomain: 'agentic',
    handler: async ({ params, getAgenticReplayLog }) => {
      return getAgenticReplayLog({
        limit: params?.limit,
        tool: params?.tool,
        eventId: params?.eventId,
        requestId: params?.requestId,
        sessionId: params?.sessionId,
        planSignature: params?.planSignature,
        executionSignature: params?.executionSignature,
        status: params?.status,
      });
    },
  },
  {
    name: 'agentic_subscribe_events',
    description: 'Subscribe to MCP execution events for a session or global stream.',
    inputSchema: {
      sessionId: z
        .string()
        .optional()
        .describe('Optional session id for filtered subscription; omitted for global stream'),
      eventTypes: z
        .array(z.string())
        .optional()
        .describe('Optional event types to receive, e.g. ["success", "error"] or ["*"]'),
    },
    permission: 'read',
    policyDomain: 'agentic',
    handler: async ({ params, mcpEventStream }) => {
      if (!mcpEventStream || typeof mcpEventStream.subscribe !== 'function') {
        return {
          success: false,
          error: 'MCP event stream service is unavailable',
        };
      }
      return mcpEventStream.subscribe({
        sessionId: params?.sessionId,
        eventTypes: params?.eventTypes,
      });
    },
  },
  {
    name: 'agentic_unsubscribe_events',
    description: 'Unsubscribe from a previously-created MCP event subscription.',
    inputSchema: {
      subscriptionId: z.string().describe('Subscription id returned by agentic_subscribe_events'),
    },
    permission: 'read',
    policyDomain: 'agentic',
    handler: async ({ params, mcpEventStream }) => {
      if (!mcpEventStream || typeof mcpEventStream.unsubscribe !== 'function') {
        return {
          success: false,
          error: 'MCP event stream service is unavailable',
        };
      }
      return mcpEventStream.unsubscribe(params?.subscriptionId);
    },
  },
  {
    name: 'agentic_list_event_subscriptions',
    description: 'List active MCP event subscriptions.',
    inputSchema: {
      sessionId: z
        .string()
        .optional()
        .describe('Optional session id filter; omitted returns global stream subscriptions only'),
    },
    permission: 'read',
    policyDomain: 'agentic',
    handler: async ({ params, mcpEventStream }) => {
      if (!mcpEventStream || typeof mcpEventStream.listSubscriptions !== 'function') {
        return {
          success: false,
          error: 'MCP event stream service is unavailable',
        };
      }
      const subscriptions = await mcpEventStream.listSubscriptions({
        sessionId: params?.sessionId,
      });
      return {
        subscriptions,
        count: Array.isArray(subscriptions) ? subscriptions.length : 0,
      };
    },
  },
  {
    name: 'agentic_get_event_history',
    description: 'Fetch recent MCP event history for debugging and replay.',
    inputSchema: {
      sessionId: z.string().optional().describe('Optional session id filter'),
      eventTypes: z.array(z.string()).optional().describe('Optional event type filters'),
      since: z
        .string()
        .optional()
        .describe('Optional ISO timestamp to fetch events after this time'),
      limit: z.number().optional().describe('Max events to return'),
    },
    permission: 'read',
    policyDomain: 'agentic',
    handler: async ({ params, mcpEventStream }) => {
      if (!mcpEventStream || typeof mcpEventStream.getEventHistory !== 'function') {
        return {
          success: false,
          error: 'MCP event stream service is unavailable',
        };
      }
      return mcpEventStream.getEventHistory({
        sessionId: params?.sessionId,
        eventTypes: params?.eventTypes,
        since: params?.since,
        limit: params?.limit,
      });
    },
  },
  {
    name: 'agentic_execute_plan',
    description:
      'Execute a tool sequence deterministically with optional dry-run and best-effort rollback.',
    inputSchema: {
      steps: z.array(
        z.object({
          tool: z.string().describe('Tool name without server prefix'),
          params: z.record(z.string(), z.any()).default({}).describe('Tool parameters'),
          policyDomain: z.string().optional().describe('Optional override policy domain'),
        }),
      ),
      dryRun: z
        .boolean()
        .optional()
        .default(true)
        .describe('Dry-run only; do not execute write calls'),
      sessionId: z.string().optional().describe('Correlation session id'),
      stopOnFailure: z.boolean().optional().default(true).describe('Stop when a step fails'),
      rollbackOnFailure: z
        .boolean()
        .optional()
        .default(true)
        .describe('Attempt best-effort rollback using compensation hints'),
      requestId: z.string().optional().describe('Optional correlation id'),
      slaLevel: z
        .enum(AGENTIC_SLA_LEVELS)
        .optional()
        .describe('Optional SLA priority for routing-aware execution'),
      costBudget: z
        .record(
          z.string().describe('Currency key (tokenSymbol or chainId:tokenSymbol)'),
          z.union([z.number(), z.string()]),
        )
        .optional()
        .describe('Optional plan-level per-currency cost cap values'),
    },
    permission: 'write',
    policyDomain: 'agentic',
    handler: async ({ params, executeAgenticPlan }) => {
      return executeAgenticPlan({
        steps: params?.steps || [],
        dryRun: params?.dryRun ?? true,
        stopOnFailure: params?.stopOnFailure ?? true,
        rollbackOnFailure: params?.rollbackOnFailure ?? true,
        requestId: params?.requestId,
        sessionId: params?.sessionId,
        slaLevel: params?.slaLevel || null,
        costBudget: params?.costBudget,
      });
    },
  },
  {
    name: 'discover_tools',
    description:
      'Discover relevant MCP tools by intent description. Returns the top matching tools for a given natural language query.',
    inputSchema: {
      intent: z.string().min(1).describe('Natural language description of what you want to do'),
      limit: z
        .number()
        .int()
        .positive()
        .max(20)
        .optional()
        .default(5)
        .describe('Maximum number of tools to return'),
    },
    permission: 'read',
    policyDomain: 'agentic',
    handler: async ({ params, getToolDiscoveryEngine }) => {
      const engine = await getToolDiscoveryEngine();
      const results = engine.discover(params.intent, params.limit || 5);
      return { success: true, tools: results };
    },
  },
  {
    name: 'delegate_to_agent',
    description: `Delegate a sub-task to a specialized commerce agent. Available agents: ${SUPPORTED_AGENT_NAMES_DESCRIPTION}.`,
    inputSchema: {
      agent_name: z
        .enum(SUPPORTED_AGENT_NAMES)
        .describe(
          `Name of the specialized agent to delegate to. One of: ${SUPPORTED_AGENT_NAMES_DESCRIPTION}.`,
        ),
      task_description: z.string().min(1).max(2000).describe('Description of the task to delegate'),
      context: z
        .record(z.string(), z.any())
        .optional()
        .default({})
        .describe('Additional context data for the agent'),
    },
    permission: 'write',
    policyDomain: 'agentic',
    handler: async ({ params, autonomousEngine }) => {
      if (!autonomousEngine) {
        return {
          success: false,
          error:
            'Autonomous engine not available. Agent delegation requires the autonomous engine to be initialized.',
        };
      }
      try {
        const result = await autonomousEngine.executeAgentRequest(
          params.agent_name,
          params.task_description,
          params.context || {},
        );
        return {
          success: true,
          delegatedTo: params.agent_name,
          task: params.task_description,
          result,
        };
      } catch (err) {
        return {
          success: false,
          error: `Delegation to '${params.agent_name}' failed: ${err.message}`,
        };
      }
    },
  },
];

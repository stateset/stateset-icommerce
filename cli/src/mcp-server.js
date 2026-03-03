/**
 * MCP Server for StateSet Commerce operations
 *
 * Thin orchestrator that loads tools from domain modules and wraps them
 * with permission checks, telemetry, treasury charging, and error handling.
 */

import { createSdkMcpServer, tool as sdkTool } from '@anthropic-ai/claude-agent-sdk';
import { createHash, createHmac, randomUUID } from 'node:crypto';
import fs from 'node:fs/promises';
import path from 'path';
import { z } from 'zod';
import { getSharedRuntime } from './channels/plugin-runtime.js';
import { A2AStore } from './a2a/store.js';
import { PolicyEngine } from './policies/engine.js';
import { createMcpEventStreamer } from './mcp-event-streamer.js';
import { ToolDiscoveryEngine } from './mcp-tool-discovery.js';
import { routeToAgentWithConfidence } from './agent-router.js';

// Domain tool modules
import { customerTools } from './tools/customers.js';
import { orderTools } from './tools/orders.js';
import { vectorTools } from './tools/vector.js';
import { productTools } from './tools/products.js';
import { inventoryTools } from './tools/inventory.js';
import { customObjectTools } from './tools/custom-objects.js';
import { returnTools } from './tools/returns.js';
import { cartTools } from './tools/carts.js';
import { analyticsTools } from './tools/analytics.js';
import { currencyTools } from './tools/currency.js';
import { taxTools } from './tools/tax.js';
import { promotionTools } from './tools/promotions.js';
import { subscriptionTools } from './tools/subscriptions.js';
import { syncTools } from './tools/sync.js';
import { manufacturingTools } from './tools/manufacturing.js';
import { paymentTools } from './tools/payments.js';
import { stablecoinTools } from './tools/stablecoin.js';
import { treasuryTools } from './tools/treasury.js';
import { erc8004Tools } from './tools/erc8004.js';
import { x402Tools } from './tools/x402.js';
import { agentCardTools } from './tools/agent-cards.js';
import { a2aTools } from './tools/a2a.js';
import { agentRuntimeTools } from './tools/agent-runtime.js';
import { shipmentTools } from './tools/shipments.js';
import { supplierTools } from './tools/suppliers.js';
import { invoiceTools } from './tools/invoices.js';
import { warrantyTools } from './tools/warranties.js';
import { importTools } from './tools/import.js';
import { policyTools } from './tools/policies.js';
import { giftCardTools } from './tools/gift-cards.js';
import { storeCreditTools } from './tools/store-credits.js';
import { segmentTools } from './tools/segments.js';
import { shippingZoneTools } from './tools/shipping-zones.js';
import { reviewTools } from './tools/reviews.js';
import { wishlistTools } from './tools/wishlists.js';
import { loyaltyTools } from './tools/loyalty.js';
import { fraudTools } from './tools/fraud.js';
import { connectorTools } from './tools/connectors.js';
import { auditTools } from './tools/audit.js';
import { proofTools } from './tools/proofs.js';
import { circuitBreakerTools } from './tools/circuit-breaker.js';
import { checkoutTools } from './tools/checkout.js';
import { complianceTools } from './tools/compliance.js';
import { catalogTools } from './tools/catalog.js';

let toolDiscoveryEngine = null;

const AGENTIC_TOOL_RESULT_SCHEMA_VERSION = '1.0.0';
const AGENTIC_POLICY_DECISION_BUNDLE_VERSION = '2026-03-01';
const AGENTIC_SLA_LEVELS = ['standard', 'expedited', 'critical'];

/**
 * All domain tool definitions, collected from modules.
 */
const AGENTIC_RUNTIME_TOOLS = [
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
    permission: 'read',
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
    handler: async ({ params }) => {
      // Lazy-initialize singleton
      if (!toolDiscoveryEngine) {
        toolDiscoveryEngine = new ToolDiscoveryEngine();
        toolDiscoveryEngine.registerFromToolDefs(ALL_TOOL_DEFS);
      }
      const results = toolDiscoveryEngine.discover(params.intent, params.limit || 5);
      return { success: true, tools: results };
    },
  },
  {
    name: 'delegate_to_agent',
    description:
      'Delegate a sub-task to a specialized commerce agent. Available agents: orders, inventory, returns, checkout, analytics, promotions, subscriptions, customer-service.',
    inputSchema: {
      agent_name: z.string().min(1).describe('Name of the specialized agent to delegate to'),
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

const AGENTIC_REPLAY_LOG_FILE = 'agentic-tool-calls.jsonl';
const AGENTIC_REPLAY_BUFFER_SIZE = 400;

const ALL_TOOL_DEFS = [
  ...customerTools,
  ...orderTools,
  ...productTools,
  ...inventoryTools,
  ...customObjectTools,
  ...returnTools,
  ...cartTools,
  ...analyticsTools,
  ...currencyTools,
  ...taxTools,
  ...promotionTools,
  ...subscriptionTools,
  ...syncTools,
  ...manufacturingTools,
  ...paymentTools,
  ...stablecoinTools,
  ...treasuryTools,
  ...erc8004Tools,
  ...x402Tools,
  ...agentCardTools,
  ...a2aTools,
  ...agentRuntimeTools,
  ...shipmentTools,
  ...supplierTools,
  ...invoiceTools,
  ...warrantyTools,
  ...importTools,
  ...policyTools,
  ...vectorTools,
  ...giftCardTools,
  ...storeCreditTools,
  ...segmentTools,
  ...shippingZoneTools,
  ...reviewTools,
  ...wishlistTools,
  ...loyaltyTools,
  ...fraudTools,
  ...connectorTools,
  ...auditTools,
  ...proofTools,
  ...circuitBreakerTools,
  ...checkoutTools,
  ...complianceTools,
  ...catalogTools,
  ...AGENTIC_RUNTIME_TOOLS,
];

const AGENTIC_COMPENSATION_HINTS = {
  create_order: ['cancel_order'],
  create_cart: ['cancel_cart'],
  ship_order: ['cancel_order'],
  reserve_inventory: ['release_reservation'],
  confirm_reservation: ['release_reservation'],
  add_cart_item: ['remove_cart_item'],
  create_return: ['reject_return'],
  approve_return: ['reject_return'],
  create_payment: ['create_refund'],
};

const AGENTIC_COMPENSATION_PARAM_HINTS = {
  cancel_order: ['orderId'],
  cancel_cart: ['cartId'],
  release_reservation: ['reservationId'],
  remove_cart_item: ['itemId'],
  reject_return: ['returnId'],
  create_refund: ['paymentId'],
};

const AGENTIC_IDEMPOTENCY_HINTS = new Set([
  'create_payment',
  'create_stablecoin_payment',
  'create_refund',
]);

const TOOL_DEFS_BY_NAME = new Map(ALL_TOOL_DEFS.map((tool) => [tool?.name, tool]).filter(Boolean));

const coerceReplayIdSource = (value) => {
  if (value === null || value === undefined) return undefined;
  if (typeof value === 'string' && value.length > 0) return value;
  if (typeof value === 'number') return `${value}`;
  return undefined;
};

const extractReplayIdFromSource = (source, keyCandidates) => {
  if (!source || typeof source !== 'object') return undefined;
  for (const key of keyCandidates) {
    const candidate = coerceReplayIdSource(source[key]);
    if (candidate) return candidate;
  }
  return undefined;
};

const _extractFirstIdLikeValue = (source) => {
  if (!source || typeof source !== 'object') return undefined;
  const directId = coerceReplayIdSource(source.id);
  if (directId) return directId;
  for (const [key, value] of Object.entries(source)) {
    if (!key.toLowerCase().endsWith('id')) continue;
    const candidate = coerceReplayIdSource(value);
    if (candidate) return candidate;
  }
  return undefined;
};

const buildCompensationParams = (compensationTool, params, result) => {
  const sources = [
    params || {},
    result || {},
    result?.order || {},
    result?.cart || {},
    result?.reservation || {},
    result?.item || {},
    result?.payment || {},
    result?.invoice || {},
    result?.customer || {},
    result?.return || {},
    result?.refund || {},
  ];
  const candidates = AGENTIC_COMPENSATION_PARAM_HINTS[compensationTool];
  const output = {};
  if (Array.isArray(candidates) && candidates.length > 0) {
    for (const key of candidates) {
      if (!key || typeof key !== 'string') continue;
      for (const source of sources) {
        const exact = extractReplayIdFromSource(source, [key]);
        if (exact) {
          output[key] = exact;
          break;
        }
        const idLike = extractReplayIdFromSource(source, ['id']);
        if (idLike && key.toLowerCase().endsWith('id')) {
          output[key] = idLike;
          break;
        }
      }
    }
  }

  if (!Object.keys(output).length) {
    const fallback = extractReplayIdFromSource(
      {
        ...params,
        ...(result || {}),
      },
      [
        'id',
        'orderId',
        'paymentId',
        'cartId',
        'reservationId',
        'returnId',
        'invoiceId',
        'customerId',
        'itemId',
      ],
    );
    if (fallback) {
      output.id = fallback;
    }
  }

  if (!Object.keys(output).length) return null;
  return output;
};

const stableStringify = (value) => {
  const normalize = (input) => {
    if (input === null || input === undefined) return input;
    if (Array.isArray(input)) {
      return input.map((item) => normalize(item));
    }
    if (typeof input !== 'object') return input;
    const sorted = Object.keys(input)
      .sort()
      .reduce((acc, key) => {
        acc[key] = normalize(input[key]);
        return acc;
      }, {});
    return sorted;
  };

  return JSON.stringify(normalize(value));
};

const sha256 = (value) => createHash('sha256').update(String(value)).digest('hex');

const REDACT_REPLAY_KEYS = new Set([
  'api_key',
  'apiKey',
  'apikey',
  'auth',
  'authorization',
  'credential',
  'credentials',
  'password',
  'private',
  'private_key',
  'privateKey',
  'secret',
  'secret_key',
  'secretKey',
  'seed',
  'signature',
  'token',
  'wallet_private_key',
]);

const MAX_REPLAY_ARRAY_ITEMS = 25;
const MAX_REPLAY_OBJECT_KEYS = 80;
const MAX_REPLAY_STRING_CHARS = 240;

const sanitizeReplayValue = (value, depth = 4, seen = new Set()) => {
  if (value === null || value === undefined) return value;
  if (typeof value === 'string') {
    if (value.length <= MAX_REPLAY_STRING_CHARS) return value;
    return `${value.slice(0, MAX_REPLAY_STRING_CHARS)}...`;
  }
  if (typeof value === 'number' || typeof value === 'boolean') return value;
  if (typeof value === 'bigint') return `${value.toString()}n`;
  if (typeof value === 'symbol' || typeof value === 'function') return String(value);
  if (value instanceof Date) return value.toISOString();
  if (value instanceof Map)
    return {
      _type: 'Map',
      size: value.size,
      entries: Array.from(value.entries()).map(([k, v]) => [
        sanitizeReplayValue(k, depth - 1, seen),
        sanitizeReplayValue(v, depth - 1, seen),
      ]),
    };
  if (value instanceof Set)
    return {
      _type: 'Set',
      size: value.size,
      values: Array.from(value.values()).map((entry) =>
        sanitizeReplayValue(entry, depth - 1, seen),
      ),
    };
  if (Buffer.isBuffer(value)) return `<Buffer ${value.length}>`;

  if (typeof value !== 'object') return String(value);
  if (depth <= 0 || seen.has(value)) return '[truncated]';
  seen.add(value);

  const output = {};
  const keys = Object.keys(value);
  const keysToCopy = keys.slice(0, MAX_REPLAY_OBJECT_KEYS);
  for (const key of keysToCopy) {
    if (REDACT_REPLAY_KEYS.has(key) || key.toLowerCase().includes('secret')) {
      output[key] = '[REDACTED]';
      continue;
    }
    output[key] = sanitizeReplayValue(value[key], depth - 1, seen);
  }
  if (keys.length > MAX_REPLAY_OBJECT_KEYS) {
    output.__truncatedKeys = keys.length - MAX_REPLAY_OBJECT_KEYS;
  }
  return output;
};

const compactReplayValue = (value, depth = 4, seen = new Set()) => {
  if (value === null || value === undefined) return value;
  if (Array.isArray(value)) {
    if (depth <= 0 || seen.has(value)) return '[truncated]';
    seen.add(value);
    const values = value
      .slice(0, MAX_REPLAY_ARRAY_ITEMS)
      .map((entry) => compactReplayValue(entry, depth - 1, seen));
    if (value.length > MAX_REPLAY_ARRAY_ITEMS) {
      values.push(`[+${value.length - MAX_REPLAY_ARRAY_ITEMS} more items]`);
    }
    return values;
  }
  return sanitizeReplayValue(value, depth, seen);
};

const MAX_PLAN_STEPS = 200;
const AGENTIC_PLAN_PARAM_TEMPLATE = /^\{\{\s*([^}]+)\s*\}\}$/;

const normalizeSlaLevel = (value) => {
  if (typeof value !== 'string') return null;
  const normalized = value.trim().toLowerCase();
  return AGENTIC_SLA_LEVELS.includes(normalized) ? normalized : null;
};

const getByPath = (value, pathSegments) => {
  let current = value;
  for (const segment of pathSegments) {
    if (current === null || current === undefined) return undefined;
    if (typeof current === 'object' || Array.isArray(current)) {
      current = current?.[segment];
      continue;
    }
    return undefined;
  }
  return current;
};

const resolveAgenticPlanPath = (context, rawPath) => {
  if (!context || typeof rawPath !== 'string') return undefined;
  const pathExpression = rawPath.trim().replace(/\[(\d+)\]/g, '.$1');
  const pathParts = pathExpression.split('.').filter(Boolean);
  if (!pathParts.length) return undefined;

  if (pathParts[0] === 'steps') {
    if (pathParts.length < 2) return undefined;
    const stepIndex = Number(pathParts[1]);
    if (!Number.isInteger(stepIndex) || stepIndex < 0) return undefined;
    return getByPath(context.steps?.[stepIndex], pathParts.slice(2));
  }

  if (pathParts[0] === 'latest') {
    return getByPath(context.latest, pathParts.slice(1));
  }

  if (pathParts[0] === 'tool') {
    if (pathParts.length < 2) return undefined;
    return getByPath(context.byTool?.[pathParts[1]], pathParts.slice(2));
  }

  if (pathParts[0] === 'sla') {
    return getByPath(context.sla, pathParts.slice(1));
  }

  if (pathParts[0] === 'slaLevel') {
    return context.sla?.level;
  }

  return undefined;
};

const buildPlanStepRouting = ({ tool, params, slaLevel }) => {
  const normalizedSlaLevel = normalizeSlaLevel(slaLevel);
  const routeIntent = `${String(tool || '').replaceAll('_', ' ')} ${stableStringify(compactReplayValue(params || {}))}`;
  const routing = routeToAgentWithConfidence(routeIntent, {
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
      : {
          agent: 'customer-service',
          score: 0,
          confidence: 0,
          level: 'default',
        },
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
};

const resolveAgenticPlanValue = (value, context, location = '$') => {
  if (value === null || value === undefined) return { value, unresolved: [] };
  if (typeof value === 'string') {
    const match = value.match(AGENTIC_PLAN_PARAM_TEMPLATE);
    if (!match) return { value, unresolved: [] };

    const resolved = resolveAgenticPlanPath(context, match[1]);
    if (resolved === undefined) {
      return {
        value: null,
        unresolved: [`${location} -> ${match[1]}`],
      };
    }

    return { value: resolved, unresolved: [] };
  }

  if (typeof value !== 'object') return { value, unresolved: [] };
  if (
    value instanceof Date ||
    Buffer.isBuffer(value) ||
    value instanceof Map ||
    value instanceof Set
  ) {
    return { value, unresolved: [] };
  }

  if (Array.isArray(value)) {
    const output = [];
    const unresolved = [];
    for (let i = 0; i < value.length; i += 1) {
      const child = resolveAgenticPlanValue(value[i], context, `${location}[${i}]`);
      output.push(child.value);
      unresolved.push(...child.unresolved);
    }
    return { value: output, unresolved };
  }

  const output = {};
  const unresolved = [];
  for (const [key, childValue] of Object.entries(value)) {
    const child = resolveAgenticPlanValue(childValue, context, `${location}.${key}`);
    output[key] = child.value;
    unresolved.push(...child.unresolved);
  }

  return { value: output, unresolved };
};

const addCostSummaryEntry = (summary, entry = {}) => {
  const chainId = entry.chainId || 'unknown';
  const tokenSymbol = entry.tokenSymbol || 'UNKNOWN';
  const key = `${chainId}:${tokenSymbol}`;
  const amount = entry.amount;
  const parsedAmount =
    typeof amount === 'number' || typeof amount === 'string' ? Number(amount) : NaN;
  if (!summary.totals[key]) {
    summary.totals[key] = {
      chainId,
      tokenSymbol,
      amount: 0,
      amountText: null,
      entries: 0,
    };
  }
  const bucket = summary.totals[key];
  bucket.entries += 1;
  if (Number.isFinite(parsedAmount)) {
    bucket.amount += parsedAmount;
  } else if (amount !== undefined && amount !== null) {
    bucket.amountText = amount;
  }

  summary.entries.push({
    step: entry.stepIndex ?? null,
    tool: entry.tool || null,
    status: entry.status || null,
    chainId,
    tokenSymbol,
    amount: amount ?? null,
    amountNumeric: Number.isFinite(parsedAmount) ? parsedAmount : null,
    charged: Boolean(entry.charged),
    blocked: Boolean(entry.blocked),
    blockedReason: entry.blockedReason || null,
    source: entry.source || null,
    rule: entry.rule || null,
  });

  summary.totalEntries = (summary.totalEntries || 0) + 1;
  if (entry.charged) summary.chargedEntries = (summary.chargedEntries || 0) + 1;
  if (entry.blocked) summary.blockedEntries = (summary.blockedEntries || 0) + 1;
};

const normalizeCostBudgetValue = (value) => {
  if (typeof value === 'number') return Number.isFinite(value) && value >= 0 ? value : null;
  if (typeof value === 'string' && value.trim()) {
    const parsed = Number(value.trim());
    return Number.isFinite(parsed) && parsed >= 0 ? parsed : null;
  }
  return null;
};

const normalizeCostBudgetKey = (rawKey) => {
  if (typeof rawKey !== 'string') return null;
  const trimmed = rawKey.trim();
  if (!trimmed) return null;
  const upper = trimmed.toUpperCase();
  if (!upper.includes(':')) return upper;
  const [rawChain, rawToken] = upper.split(':').map((part) => part.trim());
  if (!rawChain || !rawToken) return null;
  return `${rawChain}:${rawToken}`;
};

const normalizeCostBudget = (costBudget = null) => {
  if (!costBudget || typeof costBudget !== 'object' || Array.isArray(costBudget)) return {};
  const normalized = {};
  for (const [rawKey, rawLimit] of Object.entries(costBudget)) {
    const key = normalizeCostBudgetKey(rawKey);
    const limit = normalizeCostBudgetValue(rawLimit);
    if (!key || !Number.isFinite(limit)) continue;
    normalized[key] = limit;
  }
  return normalized;
};

const resolveCostBudgetLimit = (costBudget = {}, chainId = null, tokenSymbol = null) => {
  const chain = String(chainId || '*').trim();
  const token = String(tokenSymbol || '*')
    .trim()
    .toUpperCase();
  const exact = costBudget[`${chain}:${token}`];
  if (Number.isFinite(exact)) return exact;
  const tokenOnly = costBudget[token];
  if (Number.isFinite(tokenOnly)) return tokenOnly;
  const chainOnly = costBudget[`${chain}:*`];
  if (Number.isFinite(chainOnly)) return chainOnly;
  const global = costBudget['*'];
  if (Number.isFinite(global)) return global;
  return null;
};

const createCostSummary = (mode) => ({
  mode,
  totalEntries: 0,
  chargedEntries: 0,
  blockedEntries: 0,
  entries: [],
  totals: {},
});

const replayEventHash = (value) => sha256(stableStringify(compactReplayValue(value)));

const extractIdempotencyKeyFromParams = (params = {}) => {
  if (!params || typeof params !== 'object' || Array.isArray(params)) return null;
  const candidates = [
    'idempotencyKey',
    'idempotency_key',
    'idempotencyToken',
    'requestId',
    'request_id',
    'externalId',
    'external_id',
  ];
  for (const key of candidates) {
    const value = params[key];
    if (typeof value === 'string' && value.trim()) return value.trim();
  }
  return null;
};

const normalizePolicyAction = (action) => {
  if (!action) return null;
  if (typeof action?.toJSON === 'function') {
    try {
      return action.toJSON();
    } catch {
      return null;
    }
  }
  if (typeof action !== 'object' || Array.isArray(action)) return null;
  return action;
};

const normalizePolicyExplanation = (explanation) => {
  if (!explanation) return null;
  if (typeof explanation?.toJSON === 'function') {
    try {
      return explanation.toJSON();
    } catch {
      return null;
    }
  }
  if (typeof explanation !== 'object' || Array.isArray(explanation)) return null;
  return explanation;
};

const buildRollbackContract = (toolName) => {
  const compensationTools = AGENTIC_COMPENSATION_HINTS[toolName] || [];
  const compensationContracts = compensationTools.map((tool) => ({
    tool,
    params: AGENTIC_COMPENSATION_PARAM_HINTS[tool] || ['id'],
  }));

  const contract = {
    strategy: compensationContracts.length > 0 ? 'best_effort_compensation' : 'none',
    sourceTool: toolName,
    compensation: compensationContracts,
    reversible: compensationContracts.length > 0,
  };

  return {
    ...contract,
    contractHash: replayEventHash(contract),
  };
};

const buildApprovalStagesFromActions = (actions = []) => {
  const stages = [];
  for (const rawAction of actions) {
    const action = normalizePolicyAction(rawAction);
    if (!action) continue;
    const approval = action.approval || action?.metadata?.approval || null;
    const requiresApproval = Boolean(action?.metadata?.requiresApproval) || Boolean(approval);
    if (!requiresApproval) continue;

    if (Array.isArray(approval?.stages) && approval.stages.length > 0) {
      for (const stage of approval.stages) {
        if (!stage || typeof stage !== 'object') continue;
        stages.push({
          level: Number.isFinite(Number(stage.level)) ? Number(stage.level) : stages.length + 1,
          name: stage.name || `stage-${stages.length + 1}`,
          requiredApprovals: Number(stage.requiredApprovals || 1),
          approvers: Array.isArray(stage.approvers) ? stage.approvers : [],
          timeout: stage.timeout || null,
          timeoutAction: stage.timeoutAction || null,
          source: 'policy_action',
        });
      }
      continue;
    }

    stages.push({
      level: Number.isFinite(Number(approval?.level)) ? Number(approval.level) : stages.length + 1,
      name: approval?.name || action?.metadata?.approvalTier || 'approval-required',
      requiredApprovals: Number(approval?.requiredApprovals || 1),
      approvers: Array.isArray(approval?.approvers) ? approval.approvers : [],
      timeout: approval?.timeout || null,
      timeoutAction: approval?.timeoutAction || null,
      source: 'policy_action',
    });
  }

  const deduped = [];
  const seen = new Set();
  for (const stage of stages) {
    const key = `${stage.level}:${stage.name}`;
    if (seen.has(key)) continue;
    seen.add(key);
    deduped.push(stage);
  }
  return deduped.sort((a, b) => a.level - b.level);
};

const signAuditArtifact = (payload) => {
  const canonical = stableStringify(payload);
  const payloadHash = sha256(canonical);
  const signingKey =
    process.env.STATESET_AGENTIC_AUDIT_SIGNING_KEY || process.env.STATESET_AUDIT_SIGNING_KEY || '';
  const keyId = process.env.STATESET_AGENTIC_AUDIT_SIGNING_KEY_ID || 'stateset-default';

  if (signingKey) {
    return {
      payloadHash,
      signature: createHmac('sha256', signingKey).update(canonical).digest('hex'),
      algorithm: 'hmac-sha256',
      keyId,
      signed: true,
    };
  }

  return {
    payloadHash,
    signature: sha256(`unsigned:${payloadHash}`),
    algorithm: 'sha256',
    keyId: 'unsigned-deterministic',
    signed: false,
  };
};

const buildDeterministicMutationManifest = ({
  toolName,
  params = {},
  policy = null,
  permission = null,
  runtimeMeta = null,
  phase = 'execute',
} = {}) => {
  if (!runtimeMeta || runtimeMeta.sideEffect === 'read' || runtimeMeta.permission === 'unknown') {
    return null;
  }

  const paramsHash = replayEventHash(params || {});
  const policyHash = replayEventHash(policy || {});
  const permissionHash = replayEventHash(permission || {});
  const idempotencyKey =
    extractIdempotencyKeyFromParams(params) ||
    (runtimeMeta.idempotent ? `ik_${toolName}_${paramsHash.slice(0, 16)}` : null);
  const rollback = buildRollbackContract(toolName);
  const core = {
    version: '1.0.0',
    tool: toolName,
    phase,
    sideEffect: runtimeMeta.sideEffect,
    policyDomain: runtimeMeta.policyDomain || null,
    idempotent: Boolean(runtimeMeta.idempotent),
    idempotencyKey,
    paramsHash,
    policyHash,
    permissionHash,
    rollbackContractHash: rollback.contractHash,
    compensationTools: runtimeMeta.compensations || [],
  };

  return {
    ...core,
    deterministicSignature: replayEventHash(core),
    rollback,
  };
};

const TOOL_DOMAIN_BY_TOOL_NAME = (() => {
  const entries = [
    ['customers', customerTools],
    ['orders', orderTools],
    ['products', productTools],
    ['inventory', inventoryTools],
    ['custom_objects', customObjectTools],
    ['returns', returnTools],
    ['carts', cartTools],
    ['analytics', analyticsTools],
    ['currency', currencyTools],
    ['tax', taxTools],
    ['promotions', promotionTools],
    ['subscriptions', subscriptionTools],
    ['sync', syncTools],
    ['manufacturing', manufacturingTools],
    ['payments', paymentTools],
    ['stablecoin', stablecoinTools],
    ['treasury', treasuryTools],
    ['erc8004', erc8004Tools],
    ['x402', x402Tools],
    ['agent_cards', agentCardTools],
    ['a2a', a2aTools],
    ['shipments', shipmentTools],
    ['suppliers', supplierTools],
    ['invoices', invoiceTools],
    ['warranties', warrantyTools],
    ['vector', vectorTools],
    ['connectors', connectorTools],
  ];

  const map = {};
  for (const [domain, tools] of entries) {
    for (const tool of tools) {
      if (tool?.name) {
        map[tool.name] = domain;
      }
    }
  }
  return map;
})();

/**
 * Set of read-only tool names, derived from module permission metadata.
 */
const READ_ONLY_TOOLS = new Set(
  ALL_TOOL_DEFS.filter((t) => t.permission === 'read').map((t) => t.name),
);

/**
 * Auto-index a newly created entity if vectorAutoIndex is enabled.
 * Runs in the background — failures are logged but do not block the response.
 * @param {'product'|'customer'|'order'} entityType
 * @param {Object} entity - The created entity (must have .id)
 */
function autoIndexEntity(entityType, entity) {
  const vectorAutoIndex = getSharedRuntime()?.vectorAutoIndex;
  if (!vectorAutoIndex || !entity?.id) return;
  const indexFn = {
    product: () => vectorAutoIndex.indexProduct(entity.id.toString()),
    customer: () => vectorAutoIndex.indexCustomer(entity.id.toString()),
    order: () => vectorAutoIndex.indexOrder(entity.id.toString()),
  }[entityType];
  if (indexFn) {
    indexFn().catch((err) =>
      console.error(`[AutoIndex] Failed to index ${entityType} ${entity.id}: ${err.message}`),
    );
  }
}

/**
 * Create the StateSet Commerce MCP server
 * @param {Object} options
 * @param {import('@stateset/embedded').Commerce} options.commerce - Commerce instance
 * @param {boolean} options.allowApply - Whether to allow destructive operations
 * @param {import('./telemetry.js').AgentTelemetry} options.telemetry - Telemetry instance
 * @param {import('./permissions.js').PermissionGate} options.permissionGate - Permission gate instance
 * @param {import('./channels/plugin-api.js').HookRunner} options.hookRunner - Hook runner instance
 * @param {PolicyEngine} options.policyEngine - PolicyEngine instance (optional)
 * @param {string} options.policyStorePath - Policy store root path (optional)
 * @param {string} options.dbPath - Commerce database path (used for ERC-8004 lookups)
 * @param {Object} options.treasury - Treasury configuration (agentId, dbPath, ERC-8004 registry)
 * @param {Object} options.agentConfig - Agent configuration for A2A payments
 * @param {string} options.agentConfig.agentId - This agent's ID
 * @param {string} options.agentConfig.walletAddress - This agent's wallet address
 * @param {Object} options.agentConfig.signingKey - Ed25519 signing key { privateKey, publicKey }
 * @param {Object} options.mcpEventStream - Optional MCP event stream service
 * @param {boolean} options.structuredToolResults - Return MCP tool responses with machine-readable metadata
 */
export function createStatesetMcpServer({
  commerce,
  allowApply = false,
  telemetry = null,
  permissionGate = null,
  hookRunner = null,
  policyEngine = null,
  policyStorePath = null,
  dbPath = './store.db',
  treasury = null,
  agentConfig = null,
  mcpEventStream = null,
  structuredToolResults = false,
}) {
  // ---------------------------------------------------------------------------
  // A2A Store initialization
  // ---------------------------------------------------------------------------
  const a2aStore = new A2AStore({ dbPath: dbPath.replace('.db', '-a2a.db') });

  // Create a commerce wrapper that includes A2A methods
  const commerceWithA2A = {
    ...commerce,
    a2a: () => ({
      createPayment: (p) => a2aStore.createPayment(p),
      getPayment: (id) => a2aStore.getPayment(id),
      updatePayment: (id, u) => a2aStore.updatePayment(id, u),
      listPayments: (f) => a2aStore.listPayments(f),
      sumPayments: (f) => a2aStore.sumPayments(f),
      createPaymentRequest: (r) => a2aStore.createPaymentRequest(r),
      getPaymentRequest: (id) => a2aStore.getPaymentRequest(id),
      updatePaymentRequest: (id, u) => a2aStore.updatePaymentRequest(id, u),
      listPaymentRequests: (f) => a2aStore.listPaymentRequests(f),
      createQuote: (q) => a2aStore.createQuote(q),
      getQuote: (id) => a2aStore.getQuote(id),
      updateQuote: (id, u) => a2aStore.updateQuote(id, u),
      listQuotes: (f) => a2aStore.listQuotes(f),
      // Escrow methods
      createEscrow: (e) => a2aStore.createEscrow(e),
      getEscrow: (id) => a2aStore.getEscrow(id),
      updateEscrow: (id, u) => a2aStore.updateEscrow(id, u),
      listEscrows: (f) => a2aStore.listEscrows(f),
      // Dispute methods
      createDispute: (d) => a2aStore.createDispute(d),
      getDispute: (id) => a2aStore.getDispute(id),
      updateDispute: (id, u) => a2aStore.updateDispute(id, u),
      listDisputes: (f) => a2aStore.listDisputes(f),
      createEvidence: (e) => a2aStore.createEvidence(e),
      getEvidence: (id) => a2aStore.getEvidence(id),
      listEvidenceByDispute: (id) => a2aStore.listEvidenceByDispute(id),
      // Feedback / reputation methods
      createFeedback: (f) => a2aStore.createFeedback(f),
      getFeedback: (id) => a2aStore.getFeedback(id),
      updateFeedback: (id, u) => a2aStore.updateFeedback(id, u),
      listFeedback: (f) => a2aStore.listFeedback(f),
      getReputationScore: (addr) => a2aStore.getReputationScore(addr),
      upsertReputationScore: (s) => a2aStore.upsertReputationScore(s),
      // Service methods
      createService: (s) => a2aStore.createService(s),
      getService: (id) => a2aStore.getService(id),
      updateService: (id, u) => a2aStore.updateService(id, u),
      listServices: (f) => a2aStore.listServices(f),
      // Notification log methods
      createNotificationLog: (n) => a2aStore.createNotificationLog(n),
      getNotificationLog: (id) => a2aStore.getNotificationLog(id),
      updateNotificationLog: (id, u) => a2aStore.updateNotificationLog(id, u),
      listNotificationLog: (f) => a2aStore.listNotificationLog(f),
      getPendingNotifications: (max, lim) => a2aStore.getPendingNotifications(max, lim),
      // Webhook config methods
      upsertWebhookConfig: (c) => a2aStore.upsertWebhookConfig(c),
      getWebhookConfig: (addr) => a2aStore.getWebhookConfig(addr),
      listWebhookConfigs: (f) => a2aStore.listWebhookConfigs(f),
      // Subscription methods
      createSubscription: (s) => a2aStore.createSubscription(s),
      getSubscription: (id) => a2aStore.getSubscription(id),
      updateSubscription: (id, u) => a2aStore.updateSubscription(id, u),
      listSubscriptions: (f) => a2aStore.listSubscriptions(f),
      getDueSubscriptions: (now) => a2aStore.getDueSubscriptions(now),
      getExpiredTrials: (now) => a2aStore.getExpiredTrials(now),
      // Split payment methods
      createSplitPayment: (s) => a2aStore.createSplitPayment(s),
      getSplitPayment: (id) => a2aStore.getSplitPayment(id),
      updateSplitPayment: (id, u) => a2aStore.updateSplitPayment(id, u),
      listSplitPayments: (f) => a2aStore.listSplitPayments(f),
      createSplitRecipient: (r) => a2aStore.createSplitRecipient(r),
      getSplitRecipient: (id) => a2aStore.getSplitRecipient(id),
      updateSplitRecipient: (id, u) => a2aStore.updateSplitRecipient(id, u),
      listSplitRecipients: (f) => a2aStore.listSplitRecipients(f),
      // Event subscription methods
      createEventSubscription: (s) => a2aStore.createEventSubscription(s),
      getEventSubscription: (id) => a2aStore.getEventSubscription(id),
      updateEventSubscription: (id, u) => a2aStore.updateEventSubscription(id, u),
      listEventSubscriptions: (f) => a2aStore.listEventSubscriptions(f),
      // Event log methods
      createEventLog: (e) => a2aStore.createEventLog(e),
      getEventLog: (id) => a2aStore.getEventLog(id),
      listEventLog: (f) => a2aStore.listEventLog(f),

      // RFQ methods (marketplace)
      createRFQ: (r) => a2aStore.createRFQ(r),
      getRFQ: (id) => a2aStore.getRFQ(id),
      updateRFQ: (id, u) => a2aStore.updateRFQ(id, u),
      listRFQs: (f) => a2aStore.listRFQs(f),
      createRFQResponse: (r) => a2aStore.createRFQResponse(r),
      getRFQResponse: (id) => a2aStore.getRFQResponse(id),
      updateRFQResponse: (id, u) => a2aStore.updateRFQResponse(id, u),
      listRFQResponses: (f) => a2aStore.listRFQResponses(f),

      // SLA methods
      createSLADefinition: (s) => a2aStore.createSLADefinition(s),
      getSLADefinition: (id) => a2aStore.getSLADefinition(id),
      updateSLADefinition: (id, u) => a2aStore.updateSLADefinition(id, u),
      listSLADefinitions: (f) => a2aStore.listSLADefinitions(f),
      createSLAViolation: (v) => a2aStore.createSLAViolation(v),
      getSLAViolation: (id) => a2aStore.getSLAViolation(id),
      updateSLAViolation: (id, u) => a2aStore.updateSLAViolation(id, u),
      listSLAViolations: (f) => a2aStore.listSLAViolations(f),

      // Workflow methods
      createWorkflow: (w) => a2aStore.createWorkflow(w),
      getWorkflow: (id) => a2aStore.getWorkflow(id),
      updateWorkflow: (id, u) => a2aStore.updateWorkflow(id, u),
      listWorkflows: (f) => a2aStore.listWorkflows(f),
      createWorkflowStep: (s) => a2aStore.createWorkflowStep(s),
      getWorkflowStep: (id) => a2aStore.getWorkflowStep(id),
      updateWorkflowStep: (id, u) => a2aStore.updateWorkflowStep(id, u),
      listWorkflowSteps: (f) => a2aStore.listWorkflowSteps(f),
    }),
  };
  // ---------------------------------------------------------------------------
  // Permission helpers
  // ---------------------------------------------------------------------------

  const isReadOnly = (toolName) => READ_ONLY_TOOLS.has(toolName);

  const checkPermission = async (toolName, params) => {
    if (permissionGate) {
      const result = await permissionGate.checkPermission(toolName, params);
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
    const result = {
      allowed: false,
      preview: true,
      reason: `Preview mode: would execute '${toolName}' if --apply flag is set`,
      wouldDo: { tool: toolName, params },
    };
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

  const POLICY_DOMAIN_BY_TOKEN = {
    customer: 'customers',
    customers: 'customers',
    order: 'orders',
    orders: 'orders',
    product: 'products',
    products: 'products',
    inventory: 'inventory',
    custom: 'custom_objects',
    custom_object: 'custom_objects',
    custom_objects: 'custom_objects',
    returns: 'returns',
    return: 'returns',
    cart: 'carts',
    carts: 'carts',
    analytics: 'analytics',
    currency: 'currency',
    currencies: 'currency',
    tax: 'tax',
    promotion: 'promotions',
    promotions: 'promotions',
    subscription: 'subscriptions',
    subscriptions: 'subscriptions',
    sync: 'sync',
    manufacturing: 'manufacturing',
    payment: 'payments',
    payments: 'payments',
    stablecoin: 'stablecoin',
    treasury: 'treasury',
    erc8004: 'erc8004',
    x402: 'x402',
    agent: 'agent_cards',
    agent_card: 'agent_cards',
    agent_cards: 'agent_cards',
    a2a: 'a2a',
    shipment: 'shipments',
    shipments: 'shipments',
    supplier: 'suppliers',
    suppliers: 'suppliers',
    invoice: 'invoices',
    invoices: 'invoices',
    warranty: 'warranties',
    warranties: 'warranties',
    vector: 'vector',
    create: 'commerce',
    get: 'commerce',
    list: 'commerce',
    update: 'commerce',
    delete: 'commerce',
    set: 'commerce',
    ship: 'orders',
    cancel: 'orders',
    request: 'a2a',
    provide: 'a2a',
    accept: 'a2a',
    decline: 'a2a',
    pause: 'subscriptions',
    resume: 'subscriptions',
    skip: 'subscriptions',
  };

  const inferPolicyDomain = (toolName) => {
    if (!toolName || typeof toolName !== 'string') return 'commerce';

    if (TOOL_DOMAIN_BY_TOOL_NAME[toolName]) {
      return TOOL_DOMAIN_BY_TOOL_NAME[toolName];
    }

    const parts = toolName.split('_').filter(Boolean);
    if (parts.length === 0) return 'commerce';

    if (parts.length >= 2 && parts[0] === 'a2a') return 'a2a';
    if (parts.length >= 2 && parts[0] === 'agent' && parts[1] === 'card') return 'agent_cards';
    if (parts.length >= 2 && parts[0] === 'custom' && parts[1] === 'object')
      return 'custom_objects';

    for (const part of parts) {
      if (POLICY_DOMAIN_BY_TOKEN[part]) return POLICY_DOMAIN_BY_TOKEN[part];
    }

    return 'commerce';
  };

  const normalizeToolName = (toolName) => {
    if (!toolName || typeof toolName !== 'string') return '';
    return toolName.trim().replace(/^mcp__[a-z0-9_-]+__/, '');
  };

  const applyPolicyTransform = (input, transform, auditEntries = []) => {
    if (!transform || typeof transform !== 'object' || Array.isArray(transform)) {
      return { output: input, auditEntries };
    }

    const output = { ...(input || {}) };
    for (const [key, value] of Object.entries(transform)) {
      const before = output[key];
      if (
        output[key] !== null &&
        output[key] !== undefined &&
        typeof output[key] === 'object' &&
        !Array.isArray(output[key]) &&
        value &&
        typeof value === 'object' &&
        !Array.isArray(value)
      ) {
        output[key] = { ...output[key], ...value };
      } else {
        output[key] = value;
      }
      auditEntries.push({
        field: key,
        before,
        after: output[key],
        timestamp: new Date().toISOString(),
      });
    }

    return { output, auditEntries };
  };

  const resolvePolicyPath =
    policyStorePath || (dbPath ? path.join(path.dirname(path.resolve(dbPath)), '.stateset') : null);

  const policyEngineInstance =
    policyEngine ||
    (resolvePolicyPath
      ? new PolicyEngine({ storePath: resolvePolicyPath, unknownDomainMode: 'allow' })
      : null);

  const policyLoad =
    policyEngineInstance && !policyEngine
      ? policyEngineInstance.load().catch((error) => {
          if (telemetry) {
            telemetry.logCustomEvent('policy_load_failed', {
              error: error.message,
              storePath: resolvePolicyPath,
            });
          }
          return null;
        })
      : Promise.resolve();

  const activeMcpEventStream = mcpEventStream || createMcpEventStreamer();
  const publishToEventStream = (event) => {
    if (!activeMcpEventStream?.publish || typeof activeMcpEventStream.publish !== 'function') {
      return;
    }
    try {
      activeMcpEventStream.publish({
        status: event?.status || 'event',
        tool: event?.tool || null,
        requestId: event?.requestId || null,
        sessionId: event?.sessionId || null,
        timestamp: event?.occurredAt || event?.timestamp || new Date().toISOString(),
        result: event?.result || null,
        error: event?.error || null,
        policy: event?.policy || null,
        permission: event?.permission || null,
        charge: event?.charge || null,
        params: event?.params || null,
        notes: event?.notes || null,
        source: event?.source || 'mcp_server',
      });
    } catch (error) {
      console.warn('[MCP Server] Failed to publish event stream event:', error.message);
    }
  };

  const fallbackAgenticDir = resolvePolicyPath || path.join(process.cwd(), '.stateset');
  const agenticReplayLogPath = path.join(fallbackAgenticDir, AGENTIC_REPLAY_LOG_FILE);
  const agenticReplayRingBuffer = [];
  let pendingReplayAppend = Promise.resolve();
  let agenticPricingCache = null;

  const getAgenticReplayLogPath = () => agenticReplayLogPath;

  const persistAgenticReplayEvent = async (event) => {
    pendingReplayAppend = pendingReplayAppend
      .catch((err) => {
        console.debug('replay log append failed:', err.message);
      })
      .then(async () => {
        await fs.mkdir(path.dirname(agenticReplayLogPath), { recursive: true });
        await fs.appendFile(agenticReplayLogPath, `${JSON.stringify(event)}\n`);
      });
    return pendingReplayAppend;
  };

  const addAgenticReplayEvent = async (event) => {
    if (!event || typeof event !== 'object') return;
    const paramsHash = event.paramsHash || replayEventHash(event.params || {});
    const resultHash = event.resultHash || replayEventHash(event.result || {});
    const signaturePayload = {
      tool: event.tool || null,
      status: event.status || null,
      requestId: event.requestId || null,
      sessionId: event.sessionId || null,
      occurredAt: event.occurredAt || null,
      policyDomain: event.policyDomain || null,
      paramsHash,
      resultHash,
      source: event.source || null,
    };
    const sanitized = {
      ...event,
      paramsHash,
      resultHash,
      eventSignature: event.eventSignature || signAuditArtifact(signaturePayload).signature,
    };
    agenticReplayRingBuffer.push(sanitized);
    if (agenticReplayRingBuffer.length > AGENTIC_REPLAY_BUFFER_SIZE) {
      agenticReplayRingBuffer.shift();
    }
    publishToEventStream(sanitized);
    await persistAgenticReplayEvent(sanitized);
  };

  const listAgenticReplayEvents = async (options = {}) => {
    const limit = Math.max(1, Math.min(AGENTIC_REPLAY_BUFFER_SIZE, Number(options.limit) || 20));
    const targetTool = options?.tool || null;
    const targetEventId = options?.eventId || null;
    const requestId = options?.requestId || null;
    const sessionId = options?.sessionId || null;
    const status = options?.status || null;
    const targetPlanSignature = options?.planSignature || null;
    const targetExecutionSignature = options?.executionSignature || null;

    const matches = (event) => {
      if (targetTool && event?.tool !== targetTool) return false;
      if (targetEventId && event?.eventId !== targetEventId) return false;
      if (requestId && event?.requestId !== requestId) return false;
      if (sessionId && event?.sessionId !== sessionId) return false;
      if (status && event?.status !== status) return false;
      if (targetPlanSignature) {
        const eventPlanSignature = event?.planSignature || event?.notes?.planSignature;
        if (!eventPlanSignature || eventPlanSignature !== targetPlanSignature) {
          return false;
        }
      }
      if (targetExecutionSignature) {
        const eventExecutionSignature =
          event?.executionSignature || event?.notes?.executionSignature;
        if (!eventExecutionSignature || eventExecutionSignature !== targetExecutionSignature) {
          return false;
        }
      }
      return true;
    };

    let fileEvents = [];
    try {
      const raw = await fs.readFile(agenticReplayLogPath, 'utf8');
      if (raw?.trim()) {
        fileEvents = raw
          .split('\n')
          .filter((line) => line.trim())
          .map((line) => {
            try {
              return JSON.parse(line);
            } catch (error) {
              return { _parseError: error.message, raw: line };
            }
          })
          .filter(matches);
      }
    } catch (error) {
      if (error?.code !== 'ENOENT') {
        if (telemetry) {
          telemetry.logCustomEvent('agentic_replay_read_error', {
            error: error.message,
            path: agenticReplayLogPath,
          });
        }
      }
    }

    const merged = [...fileEvents, ...agenticReplayRingBuffer].filter(matches);
    const deduped = [];
    const seen = new Set();
    for (const evt of merged) {
      if (!evt?.eventId) {
        deduped.push(evt);
        continue;
      }
      if (seen.has(evt.eventId)) continue;
      seen.add(evt.eventId);
      deduped.push(evt);
    }

    const order = deduped
      .filter((event) => event.occurredAt)
      .sort((a, b) => (a.occurredAt < b.occurredAt ? 1 : -1));
    const remaining = limit ? order.slice(0, limit) : order;

    return {
      generatedAt: new Date().toISOString(),
      count: remaining.length,
      events: remaining,
      filters: {
        limit,
        tool: targetTool || null,
        eventId: targetEventId,
        requestId,
        sessionId,
        planSignature: targetPlanSignature,
        executionSignature: targetExecutionSignature,
        status,
      },
      source: {
        path: getAgenticReplayLogPath(),
        inMemoryBuffer: agenticReplayRingBuffer.length,
      },
    };
  };

  const loadAgenticPricingState = async () => {
    if (agenticPricingCache !== null) return agenticPricingCache;
    try {
      const { loadTreasuryContext } = await import('./treasury/index.js');
      const ctx = await loadTreasuryContext(treasuryContextOptions);
      agenticPricingCache = {
        loaded: true,
        pricing: ctx.pricing,
        registry: ctx.registry,
        loadedAt: new Date().toISOString(),
      };
    } catch (error) {
      agenticPricingCache = { loaded: false, error: error.message };
    }
    return agenticPricingCache;
  };

  const getToolRuntimeMeta = (toolName) => {
    const candidate = TOOL_DEFS_BY_NAME.get(toolName);
    if (!candidate) {
      return {
        name: toolName,
        permission: 'unknown',
        policyDomain: inferPolicyDomain(toolName),
        sideEffect: 'unknown',
        compensations: [],
        idempotent: false,
      };
    }
    const permission = candidate?.permission || 'unknown';
    return {
      name: candidate.name,
      permission,
      policyDomain:
        candidate?.policyDomain ||
        TOOL_DOMAIN_BY_TOOL_NAME[toolName] ||
        inferPolicyDomain(toolName),
      sideEffect: permission === 'read' ? 'read' : 'write',
      description: candidate.description || '',
      compensations: AGENTIC_COMPENSATION_HINTS[toolName] || [],
      idempotent: AGENTIC_IDEMPOTENCY_HINTS.has(toolName),
      replay: {
        paramsHash: true,
        resultHash: true,
      },
    };
  };

  const getAgenticToolPricing = async (toolName) => {
    const state = await loadAgenticPricingState();
    if (!state?.loaded || !state.pricing || !toolName) {
      return null;
    }
    try {
      const { getToolPricing, resolveToken, toSmallestUnit } = await import('./treasury/index.js');
      const rule = getToolPricing(state.pricing, toolName);
      if (!rule) return null;
      const token = resolveToken(rule.chainId, rule.tokenSymbol, state.registry);
      if (!token) return null;
      const amount = Number(rule.amount);
      const amountSmallest = toSmallestUnit(amount, token.decimals);
      return {
        enabled: true,
        chainId: rule.chainId,
        tokenSymbol: rule.tokenSymbol,
        amount,
        amountSmallest: amountSmallest?.toString?.() || amountSmallest,
        token: {
          symbol: token.symbol,
          chainId: token.chainId,
          address: token.address || null,
          decimals: token.decimals,
        },
      };
    } catch {
      return null;
    }
  };

  const buildPolicyDecisionBundle = ({
    toolName,
    domain,
    inputParams = {},
    outputParams = {},
    actions = [],
    explanations = [],
    allowed = true,
    reason = null,
  }) => {
    const runtimeMeta = getToolRuntimeMeta(toolName);
    const normalizedActions = actions
      .map((action) => normalizePolicyAction(action))
      .filter(Boolean);
    const normalizedExplanations = explanations
      .map((explanation) => normalizePolicyExplanation(explanation))
      .filter(Boolean);
    const approvalStages = buildApprovalStagesFromActions(normalizedActions);
    const rollbackContract = buildRollbackContract(toolName);

    const core = {
      version: AGENTIC_POLICY_DECISION_BUNDLE_VERSION,
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
  };

  const getAgenticRuntimeContract = async ({ tool, includeLegacyDefaults = false } = {}) => {
    const normalizedTools = await Promise.all(
      ALL_TOOL_DEFS.filter((candidate) => !tool || candidate?.name === tool)
        .sort((a, b) => a.name.localeCompare(b.name))
        .map(async (candidate) => {
          const meta = getToolRuntimeMeta(candidate?.name);
          const pricing = await getAgenticToolPricing(candidate?.name);
          return {
            ...meta,
            pricing: pricing
              ? {
                  enabled: pricing.enabled,
                  chainId: pricing.chainId,
                  tokenSymbol: pricing.tokenSymbol,
                  amount: pricing.amount,
                  amountSmallest: pricing.amountSmallest,
                }
              : null,
          };
        }),
    );

    const includeLegacy = includeLegacyDefaults
      ? ['create', 'read', 'update', 'delete', 'list']
      : [];
    const contract = {
      engine: 'stateset-icommerce',
      agenticToolResultSchema: {
        version: AGENTIC_TOOL_RESULT_SCHEMA_VERSION,
        envelope: 'mcp_tool_result',
        metadata: [
          'schemaVersion',
          'status',
          'tool',
          'requestId',
          'sessionId',
          'policy',
          'permission',
          'charge',
          'mutation',
          'timing',
        ],
      },
      purpose: 'agentic_runtime_contract',
      generatedAt: new Date().toISOString(),
      includeLegacyDefaults,
      legacyDefaults: includeLegacy,
      totalTools: normalizedTools.length,
      tools: normalizedTools,
    };
    if (includeLegacy) {
      contract.legacy = {
        deprecatedPrefixes: includeLegacy,
      };
    }
    contract.contractHash = replayEventHash(stableStringify({ tools: contract.tools }));
    return contract;
  };

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

    let nextArgs = params || {};
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
        const payload = {
          status: permission.preview ? 'preview' : 'permission_block',
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
          mutationManifest: buildStepMutationManifest(nextArgs, policy, permission, payload.status),
          error: permission.reason || 'Permission denied',
          wouldDo: permission.wouldDo || null,
        };
        if (dryRun && permission.preview) {
          payload.status = 'dry_run_blocked';
        }
        return {
          index: stepIndex,
          tool: resolvedToolName,
          ...payload,
        };
      }

      charge = await maybeChargeForTool(resolvedToolName, { requestId, sessionId });
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
        ...toolContext,
        params: nextArgs,
        extra: {
          requestId,
          sessionId,
          ...extra,
        },
      };
      const wrapped = wrapWithTelemetry(resolvedToolName, (payload) => toolDef.handler(payload));
      const result = await wrapped(toolPayload);
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

  const simulateMutationToolCall = async ({
    tool,
    params = {},
    policyDomain = null,
    requestId = null,
    sessionId = null,
    includeHooks = false,
  }) => {
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

  const replayMutationToolCall = async ({
    eventId = null,
    requestId = null,
    tool = null,
    dryRun = true,
    includeHooks = false,
    sessionId = null,
  }) => {
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

  const evaluatePolicy = async (toolName, params, extra, policyDomain = null) => {
    if (!policyEngineInstance) {
      const domain = policyDomain || inferPolicyDomain(toolName);
      return {
        allowed: true,
        params,
        domain,
        policyDecisionBundle: buildPolicyDecisionBundle({
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

    await policyLoad;

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
      result = await policyEngineInstance.evaluate(domain, policyContext);
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
        policyDecisionBundle: buildPolicyDecisionBundle({
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
    const policyDecisionBundle = buildPolicyDecisionBundle({
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

  // ---------------------------------------------------------------------------
  // Treasury helpers
  // ---------------------------------------------------------------------------

  const treasuryAgentId = treasury?.agentId || process.env.TREASURY_AGENT || 'default';
  const treasuryDbPath = treasury?.dbPath || process.env.TREASURY_DB || null;
  const treasuryContextOptions = treasuryDbPath ? { dbPath: treasuryDbPath } : {};
  const treasuryRegistry =
    treasury?.erc8004Registry || process.env.TREASURY_ERC8004_REGISTRY || null;
  const treasuryIdentityDbPath = treasury?.erc8004DbPath || dbPath;
  let treasuryIdentityLoaded = false;
  let treasuryIdentityCache = null;

  const resolveTreasuryIdentity = async () => {
    if (!treasuryRegistry) return null;
    if (treasuryIdentityLoaded) return treasuryIdentityCache;
    treasuryIdentityLoaded = true;
    try {
      const { getIdentity } = await import('./erc8004/index.js');
      treasuryIdentityCache = getIdentity(
        treasuryIdentityDbPath,
        treasuryRegistry,
        treasuryAgentId,
      );
    } catch {
      treasuryIdentityCache = null;
    }
    if (!treasuryIdentityCache) {
      throw new Error(`ERC-8004 identity not found for ${treasuryRegistry}:${treasuryAgentId}`);
    }
    return treasuryIdentityCache;
  };

  const resolveTreasuryAgentId = async () => {
    const identity = await resolveTreasuryIdentity();
    return identity?.agent_id || treasuryAgentId;
  };

  const buildTreasuryIdentityMetadata = async () => {
    const identity = await resolveTreasuryIdentity();
    if (!identity) return {};
    return {
      erc8004: {
        registry: treasuryRegistry,
        agentId: identity.agent_id,
        wallet: identity.agent_wallet,
        owner: identity.owner_address,
      },
    };
  };

  // ---------------------------------------------------------------------------
  // Telemetry & audit helpers
  // ---------------------------------------------------------------------------

  const wrapWithTelemetry = (toolName, fn) => {
    return async (params, extra) => {
      const startTime = Date.now();
      try {
        const result = await fn(params, extra);
        if (telemetry) {
          const duration = Date.now() - startTime;
          telemetry.logToolCall(toolName, params, result, duration);
        }
        return result;
      } catch (error) {
        if (telemetry) {
          const duration = Date.now() - startTime;
          telemetry.logToolCall(toolName, params, { error: error.message }, duration);
        }
        throw error;
      }
    };
  };

  const buildAuditContext = (extra, toolName) => ({
    taskId: extra?.requestId || null,
    requestId: extra?.requestId || null,
    sessionId: extra?.sessionId || null,
    toolName,
  });

  const maybeChargeForTool = async (toolName, extra) => {
    try {
      const { loadTreasuryContext, getToolPricing, resolveToken, recordFee } =
        await import('./treasury/index.js');
      const { toSmallestUnit } = await import('./chains/config.js');
      const ctx = await loadTreasuryContext(treasuryContextOptions);
      const rule = getToolPricing(ctx.pricing, toolName);
      if (!rule) return { charged: false };

      if (!allowApply) {
        return {
          charged: false,
          blocked: true,
          reason: `Tool ${toolName} requires a treasury charge. Re-run with --apply.`,
        };
      }

      const token = resolveToken(rule.chainId, rule.tokenSymbol, ctx.registry);
      if (!token) {
        return {
          charged: false,
          blocked: true,
          reason: `Unknown token ${rule.tokenSymbol} on ${rule.chainId}.`,
        };
      }
      const amount = Number(rule.amount);
      if (!Number.isFinite(amount) || amount <= 0) {
        return {
          charged: false,
          blocked: true,
          reason: `Invalid pricing amount for ${toolName}.`,
        };
      }
      const effectiveAgentId = await resolveTreasuryAgentId();
      const identityMeta = await buildTreasuryIdentityMetadata();
      const balance = ctx.store.getBalance({
        agentId: effectiveAgentId,
        chainId: rule.chainId,
        tokenSymbol: token.symbol,
        tokenDecimals: token.decimals,
      });

      const required = toSmallestUnit(amount, token.decimals);

      if (balance.balanceSmallest < required) {
        return {
          charged: false,
          blocked: true,
          reason: `Insufficient ${token.symbol} balance for ${toolName}. Required ${rule.amount} ${token.symbol}.`,
        };
      }

      const audit = buildAuditContext(extra, toolName);
      await recordFee(
        {
          agentId: effectiveAgentId,
          chainId: rule.chainId,
          tokenSymbol: token.symbol,
          amount,
          source: 'task',
          metadata: {
            pricingRule: rule,
            ...identityMeta,
          },
          ...audit,
        },
        ctx,
      );

      return { charged: true, rule };
    } catch (error) {
      return { charged: false, blocked: true, reason: error.message };
    }
  };

  const shouldReturnStructuredResults =
    structuredToolResults ||
    String(process.env.STATESSET_MCP_STRUCTURED_TOOL_RESULTS || '').toLowerCase() === 'true' ||
    String(process.env.STATESSET_MCP_STRUCTURED_TOOL_RESULTS || '').toLowerCase() === '1';

  // ---------------------------------------------------------------------------
  // Tool wrapper helpers — add hooks, permission checks, treasury, and telemetry
  // ---------------------------------------------------------------------------

  const buildToolResultPayload = (basePayload, status, startedAt, toolMeta = {}) => {
    if (!shouldReturnStructuredResults) {
      return basePayload;
    }

    const agenticMeta = {
      schemaVersion: AGENTIC_TOOL_RESULT_SCHEMA_VERSION,
      status,
      tool: basePayload?.tool || toolMeta.name || null,
      requestId: toolMeta.requestId ?? null,
      sessionId: toolMeta.sessionId ?? null,
      policy: compactReplayValue(toolMeta.policy || null),
      permission: compactReplayValue(toolMeta.permission || null),
      charge: compactReplayValue(toolMeta.charge || null),
      mutation: compactReplayValue(toolMeta.mutationManifest || null),
      timing: {
        startedAt: new Date(startedAt).toISOString(),
        completedAt: new Date().toISOString(),
        elapsedMs: Date.now() - startedAt,
      },
    };

    const withType = {
      ...toolMeta.meta,
      ...agenticMeta,
    };

    if (
      basePayload === null ||
      basePayload === undefined ||
      Array.isArray(basePayload) ||
      typeof basePayload !== 'object'
    ) {
      return {
        result: basePayload,
        _agentic: compactReplayValue(withType),
      };
    }

    if (basePayload._agentic) {
      return basePayload;
    }

    return {
      ...basePayload,
      _agentic: compactReplayValue(withType),
    };
  };

  const buildToolResultResponse = (result, status, startedAt, toolMeta = {}, isError = false) => {
    const payload = buildToolResultPayload(result, status, startedAt, toolMeta);
    const response = {
      content: [
        {
          type: 'text',
          text: JSON.stringify(payload),
        },
      ],
    };
    if (isError) response.isError = true;
    return response;
  };

  const attachStructuredToolMetadataToResponse = (response, status, startedAt, toolMeta = {}) => {
    if (
      !shouldReturnStructuredResults ||
      !response ||
      !response.content ||
      !Array.isArray(response.content)
    ) {
      return response;
    }

    const first = response.content[0];
    if (!first || first.type !== 'text' || typeof first.text !== 'string') {
      return response;
    }

    try {
      const parsedPayload = JSON.parse(first.text);
      const payload = buildToolResultPayload(parsedPayload, status, startedAt, toolMeta);
      return {
        ...response,
        content: [{ ...first, text: JSON.stringify(payload) }, ...response.content.slice(1)],
      };
    } catch {
      return response;
    }
  };

  const wrapTool = (name, description, schema, handler, policyDomain = null) => {
    return sdkTool(name, description, schema, async (args, extra) => {
      const startedAt = Date.now();
      let nextArgs = args;
      let policy = null;
      let permission = null;
      let charge = null;
      const runtimeMeta = getToolRuntimeMeta(name);
      const sessionIdFromArgs =
        args &&
        typeof args === 'object' &&
        !Array.isArray(args) &&
        typeof args.sessionId === 'string'
          ? args.sessionId
          : null;
      const effectiveSessionId = extra?.sessionId || sessionIdFromArgs || null;
      const buildMutationManifest = (
        paramsValue = nextArgs,
        policyValue = policy,
        permissionValue = permission,
        phase = 'execute',
      ) => {
        if (runtimeMeta.sideEffect !== 'write') return null;
        return buildDeterministicMutationManifest({
          toolName: name,
          params: paramsValue || {},
          policy: policyValue || null,
          permission: permissionValue || null,
          runtimeMeta,
          phase,
        });
      };
      const logEvent = async (status, payload = {}) => {
        const mutationManifest =
          payload?.mutationManifest !== undefined
            ? payload.mutationManifest
            : buildMutationManifest(
                payload?.params || nextArgs,
                payload?.policy || policy,
                payload?.permission || permission,
                status,
              );
        await addAgenticReplayEvent({
          eventId: randomUUID(),
          tool: name,
          status,
          requestId: extra?.requestId || null,
          sessionId: effectiveSessionId,
          policyDomain: policyDomain || inferPolicyDomain(name),
          occurredAt: new Date().toISOString(),
          elapsedMs: Date.now() - startedAt,
          params: compactReplayValue(payload?.params || args || {}),
          paramsHash: replayEventHash(payload?.params || args || {}),
          result: payload?.result,
          resultHash: replayEventHash(payload?.result || {}),
          policy: compactReplayValue(payload?.policy || null),
          permission: compactReplayValue(payload?.permission || null),
          charge: compactReplayValue(payload?.charge || null),
          error: payload?.error || null,
          notes: compactReplayValue({
            ...(payload?.notes || {}),
            mutationManifest,
          }),
          source: 'mcp_server',
          agentic: true,
        });
      };
      const baseToolContext = {
        tool: name,
        args,
        requestId: extra?.requestId,
        sessionId: effectiveSessionId,
      };

      try {
        if (hookRunner?.hasHooks?.('before_tool_call')) {
          const hookResult = await hookRunner.run('before_tool_call', {
            tool: baseToolContext.tool,
            params: nextArgs,
            allowApply,
            requestId: baseToolContext.requestId,
            sessionId: baseToolContext.sessionId,
          });
          if (hookResult?.params) nextArgs = hookResult.params;
          if (hookResult?.blocked || hookResult?.allowed === false) {
            const payload = {
              error: hookResult?.reason || 'Tool execution blocked by hook',
              tool: name,
            };
            await logEvent('blocked', {
              params: nextArgs,
              error: payload.error,
              notes: {
                hook: {
                  allowed: hookResult?.allowed,
                  reason: hookResult?.reason || null,
                  blocked: true,
                },
              },
            });
            return buildToolResultResponse(
              payload,
              'blocked',
              startedAt,
              {
                requestId: baseToolContext.requestId,
                sessionId: baseToolContext.sessionId,
                policy,
                permission,
                charge,
                mutationManifest: buildMutationManifest(nextArgs, policy, permission, 'blocked'),
                name,
                meta: {
                  hook: {
                    allowed: hookResult?.allowed,
                    reason: hookResult?.reason || null,
                    blocked: true,
                  },
                },
              },
              true,
            );
          }
        }

        policy = await evaluatePolicy(name, nextArgs, extra, policyDomain);
        if (!policy.allowed) {
          const payload = {
            error: policy.reason || 'Tool execution blocked by policy',
            remediation: policy.remediation || null,
            tool: name,
            policy: {
              domain: policy.domain,
              actions: policy.actions || [],
              explanations: policy.explanations || [],
              transformAudit: policy.transformAudit || [],
              evaluation: policy.evaluation || null,
              decisionBundle: policy.policyDecisionBundle || null,
            },
          };
          await logEvent('policy_block', {
            params: nextArgs,
            policy: payload.policy,
            error: payload.error,
            remediation: payload.remediation,
          });
          return buildToolResultResponse(
            payload,
            'policy_block',
            startedAt,
            {
              requestId: baseToolContext.requestId,
              sessionId: baseToolContext.sessionId,
              policy,
              permission,
              charge,
              mutationManifest: buildMutationManifest(nextArgs, policy, permission, 'policy_block'),
              name,
              meta: {
                policy: payload.policy,
              },
            },
            true,
          );
        }

        nextArgs = policy.params;

        permission = await checkPermission(name, nextArgs);
        if (!permission.allowed) {
          const payload = {
            error: permission.reason || 'Permission denied',
            tool: name,
          };
          if (permission.preview) {
            payload.preview = true;
            if (permission.wouldDo) {
              payload.wouldDo = permission.wouldDo;
            }
            await logEvent('preview', {
              params: nextArgs,
              permission,
              policy,
              error: payload.error,
            });
          } else {
            await logEvent('permission_block', {
              params: nextArgs,
              permission,
              policy,
              error: payload.error,
            });
          }
          return buildToolResultResponse(
            payload,
            permission.preview ? 'preview' : 'permission_block',
            startedAt,
            {
              requestId: baseToolContext.requestId,
              sessionId: baseToolContext.sessionId,
              policy,
              permission,
              charge,
              mutationManifest: buildMutationManifest(
                nextArgs,
                policy,
                permission,
                permission.preview ? 'preview' : 'permission_block',
              ),
              name,
            },
            true,
          );
        }

        charge = await maybeChargeForTool(name, extra);
        if (charge?.blocked) {
          await logEvent('treasury_block', {
            params: nextArgs,
            permission,
            charge: {
              blocked: charge.blocked,
              reason: charge.reason || null,
            },
            error: charge.reason || 'Treasury charge blocked',
          });
          return buildToolResultResponse(
            {
              error: charge.reason || 'Treasury charge blocked',
              tool: name,
              charge,
            },
            'treasury_block',
            startedAt,
            {
              requestId: baseToolContext.requestId,
              sessionId: baseToolContext.sessionId,
              policy,
              permission,
              charge,
              mutationManifest: buildMutationManifest(
                nextArgs,
                policy,
                permission,
                'treasury_block',
              ),
              name,
            },
            true,
          );
        }

        const wrapped = wrapWithTelemetry(name, handler);
        const result = await wrapped(nextArgs, extra);
        if (hookRunner?.hasHooks?.('after_tool_call')) {
          await hookRunner.run('after_tool_call', {
            tool: name,
            params: nextArgs,
            result,
            requestId: extra?.requestId,
            sessionId: effectiveSessionId,
          });
        }
        await logEvent('success', {
          params: nextArgs,
          permission,
          charge,
          result: compactReplayValue(result),
          policy: {
            allowed: policy.allowed,
            domain: policy.domain,
            actions: policy.actions || [],
            decisionBundle: policy.policyDecisionBundle || null,
          },
        });
        const maybeStructured = attachStructuredToolMetadataToResponse(
          result,
          'success',
          startedAt,
          {
            requestId: baseToolContext.requestId,
            sessionId: baseToolContext.sessionId,
            policy,
            permission,
            charge,
            mutationManifest: buildMutationManifest(nextArgs, policy, permission, 'success'),
            name,
          },
        );
        return maybeStructured;
      } catch (error) {
        if (hookRunner?.hasHooks?.('after_tool_call')) {
          await hookRunner.run('after_tool_call', {
            tool: name,
            params: nextArgs,
            error: error.message,
            requestId: extra?.requestId,
            sessionId: effectiveSessionId,
          });
        }
        await logEvent('error', {
          params: nextArgs,
          permission,
          charge,
          policy: policy
            ? {
                allowed: policy.allowed,
                domain: policy.domain,
                actions: policy.actions || [],
                decisionBundle: policy.policyDecisionBundle || null,
              }
            : null,
          mutationManifest: buildMutationManifest(nextArgs, policy, permission, 'error'),
          error: error.message,
        });
        throw error;
      }
    });
  };

  // ---------------------------------------------------------------------------
  // Adapt domain tool modules into MCP-formatted tools
  // ---------------------------------------------------------------------------

  /**
   * Context object passed to every domain tool handler.
   */
  const toolContext = {
    commerce: commerceWithA2A,
    allowApply,
    autoIndexEntity,
    resolveTreasuryAgentId,
    treasuryContextOptions,
    buildAuditContext,
    buildTreasuryIdentityMetadata,
    agentConfig,
    mcpEventStream: activeMcpEventStream,
    getAgenticRuntimeContract,
    executeAgenticPlan,
    simulateAgenticPlan,
    simulateMutationToolCall,
    replayMutationToolCall,
    getAgenticReplayLog: listAgenticReplayEvents,
    policyEngine: policyEngineInstance,
  };

  /**
   * Convert a domain tool definition into an SDK-wrapped MCP tool.
   * Bridges the module handler signature `({ commerce, params, ... }) => plainObject`
   * to the MCP format `(args, extra) => { content: [{ type: 'text', ... }] }`.
   */
  const adaptTool = (toolDef) => {
    const { name, description, inputSchema, handler } = toolDef;
    const _policyDomain =
      toolDef?.policyDomain || TOOL_DOMAIN_BY_TOOL_NAME[name] || inferPolicyDomain(name);

    return wrapTool(name, description, inputSchema, async (args, extra) => {
      try {
        const result = await handler({
          ...toolContext,
          params: args,
          extra,
        });
        return {
          content: [{ type: 'text', text: JSON.stringify(result, null, 2) }],
        };
      } catch (error) {
        return {
          content: [
            { type: 'text', text: JSON.stringify({ success: false, error: error.message }) },
          ],
        };
      }
    });
  };

  // ---------------------------------------------------------------------------
  // Build and return the MCP server
  // ---------------------------------------------------------------------------

  const server = createSdkMcpServer({
    name: 'stateset-commerce',
    version: '1.0.0',
    tools: ALL_TOOL_DEFS.map(adaptTool),
  });

  server.mcpEventStream = activeMcpEventStream;
  return server;
}

/**
 * All MCP tool names in the `mcp__<server>__<tool>` format expected by the harness.
 */
export const TOOL_NAMES = ALL_TOOL_DEFS.map((t) => `mcp__stateset-commerce__${t.name}`);

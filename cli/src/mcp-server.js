/**
 * MCP Server for StateSet Commerce operations
 *
 * Thin orchestrator that loads tools from domain modules and wraps them
 * with permission checks, telemetry, treasury charging, and error handling.
 */

import { createSdkMcpServer, tool as sdkTool } from '@anthropic-ai/claude-agent-sdk';
import { randomUUID } from 'node:crypto';
import path from 'path';
// `z` (zod) is now only used inside `./mcp/agentic-runtime-tools.js` and
// the per-domain tool modules; mcp-server.js no longer needs it directly.
import { getSharedRuntime } from './channels/plugin-runtime.js';
import { A2AStore } from './a2a/store.js';
import { PolicyEngine } from './policies/engine.js';
import { createMcpEventStreamer } from './mcp-event-streamer.js';
import { ToolDiscoveryEngine } from './mcp-tool-discovery.js';
import {
  compactReplayValue,
  sanitizeReplayValue,
  sha256,
  stableStringify,
} from './mcp/replay-sanitizer.js';
import {
  addCostSummaryEntry,
  createCostSummary,
  normalizeCostBudget,
  resolveCostBudgetLimit,
} from './mcp/cost-budget.js';
import { MAX_PLAN_STEPS, normalizeSlaLevel, resolveAgenticPlanValue } from './mcp/plan-resolver.js';
import { AGENTIC_RUNTIME_TOOLS } from './mcp/agentic-runtime-tools.js';
import {
  AGENTIC_COMPENSATION_HINTS,
  AGENTIC_IDEMPOTENCY_HINTS,
  buildCompensationParams,
} from './mcp/compensation.js';
import {
  inferPolicyDomain as inferPolicyDomainImpl,
  inferStaticPolicyDomain,
} from './mcp/policy-domain.js';
import { normalizeToolName } from './mcp/policy-helpers.js';
import { autoIndexEntity as autoIndexEntityImpl } from './mcp/auto-index.js';
import { adaptCommerceForTools, extendCommerceWithApis } from './mcp/commerce-adapter.js';
import { buildPlanStepRouting as buildPlanStepRoutingImpl } from './mcp/plan-step-routing.js';
import { signAuditArtifact as signAuditArtifactImpl } from './mcp/audit-signing.js';
import { replayEventHash } from './mcp/audit-envelope.js';
import { buildDeterministicMutationManifest } from './mcp/mutation-manifest.js';
import {
  DEFAULT_REPLAY_BUFFER_SIZE,
  DEFAULT_REPLAY_LOG_FILE,
  createReplayLog,
} from './mcp/replay-log.js';
import { buildToolRuntimeMeta, createPricingCache } from './mcp/pricing.js';
import {
  AGENTIC_TOOL_RESULT_SCHEMA_VERSION as RESULT_SCHEMA_VERSION,
  attachStructuredToolMetadataToResponse as attachStructuredToolMetadataToResponseImpl,
  buildToolResultResponse as buildToolResultResponseImpl,
} from './mcp/result-builders.js';
import {
  AGENTIC_POLICY_DECISION_BUNDLE_VERSION as POLICY_BUNDLE_VERSION,
  createEvaluatePolicy,
} from './mcp/policy-evaluator.js';
import {
  buildAuditContext as buildAuditContextImpl,
  createToolCharger,
  createTreasuryIdentityResolver,
  createWrapWithTelemetry,
} from './mcp/tool-wrappers.js';
import {
  createReplayMutationToolCall,
  createSimulateMutationToolCall,
} from './mcp/mutation-simulator.js';
import { routeToAgentWithConfidence } from './agent-router.js';
import { adaptCommerceApis } from './commerce.js';
import { getCommerce } from './database.js';
// SUPPORTED_AGENT_NAMES* now consumed by `./mcp/agentic-runtime-tools.js`.
import {
  MPP_PROTOCOL,
  MPP_JSONRPC_PAYMENT_REQUIRED_CODE,
  MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE,
  MPP_VERSION,
  attachPaymentMetadata,
  buildMppServiceInfo,
  buildPaymentInfoFromPricing,
  buildPaymentRequiredPayload,
  createPaymentChallenge,
  createPaymentDiscoveryDocument,
  createPaymentReceipt,
  executeMppToolWithPayment,
  extractPaymentCredential,
  listPaymentMethodAdapters,
  verifyPaymentCredential,
} from './mpp/index.js';

// Domain tool registry
import { ALL_DOMAIN_TOOLS, TOOL_POLICY_DOMAIN_BY_NAME } from './tools/domain-registry.js';
import {
  formatValidationIssues,
  inputSchemaDefToJsonSchema,
  validateToolInput,
} from './tool-schema.js';

// AGENTIC_TOOL_RESULT_SCHEMA_VERSION lives in ./mcp/result-builders.js. We
// re-alias it locally so existing call sites in this file (and any reverse
// imports) keep the original name.
const AGENTIC_TOOL_RESULT_SCHEMA_VERSION = RESULT_SCHEMA_VERSION;
// AGENTIC_POLICY_DECISION_BUNDLE_VERSION lives in ./mcp/policy-evaluator.js.
const AGENTIC_POLICY_DECISION_BUNDLE_VERSION = POLICY_BUNDLE_VERSION;
// AGENTIC_SLA_LEVELS now lives in `./mcp/plan-resolver.js` (imported above).
const MPP_SERVICE_INFO = buildMppServiceInfo({
  serviceId: 'stateset-commerce-mcp',
  serviceName: 'StateSet Commerce MCP',
  version: '1.0.0',
  serverName: 'stateset-commerce',
  serverUrl: '/mcp',
});

// `createCallableApiAccessor`, `adaptCommerceForTools`, and
// `extendCommerceWithApis` now live in `./mcp/commerce-adapter.js`. The
// two used by the orchestrator are imported at the top of the file.

// Replay log defaults live in `./mcp/replay-log.js` (DEFAULT_REPLAY_LOG_FILE,
// DEFAULT_REPLAY_BUFFER_SIZE). They're re-exported here for the few call sites
// that still reference the legacy names.
const AGENTIC_REPLAY_LOG_FILE = DEFAULT_REPLAY_LOG_FILE;
const AGENTIC_REPLAY_BUFFER_SIZE = DEFAULT_REPLAY_BUFFER_SIZE;

const ALL_TOOL_DEFS = [...ALL_DOMAIN_TOOLS, ...AGENTIC_RUNTIME_TOOLS];

// AGENTIC_COMPENSATION_HINTS, AGENTIC_IDEMPOTENCY_HINTS, coerceReplayIdSource,
// extractReplayIdFromSource, _extractFirstIdLikeValue, and
// buildCompensationParams now live in `./mcp/compensation.js`. The imported
// helpers are declared at the top of the file.

const TOOL_DEFS_BY_NAME = new Map(ALL_TOOL_DEFS.map((tool) => [tool?.name, tool]).filter(Boolean));

// Module-scope binding for `inferPolicyDomain`. Used by ~17 call sites
// inside `createStatesetMcpServer`. The pure logic lives in
// `./mcp/policy-domain.js`; this just curries the per-tool def map.
const inferPolicyDomain = (toolName) => inferPolicyDomainImpl(toolName, TOOL_DEFS_BY_NAME);

// Replay-log sanitization helpers are now in `./mcp/replay-sanitizer.js`.
// stableStringify, sha256, REDACT_REPLAY_KEYS, sanitizeReplayValue,
// compactReplayValue, MAX_REPLAY_ARRAY_ITEMS, MAX_REPLAY_OBJECT_KEYS, and
// MAX_REPLAY_STRING_CHARS are imported at the top of the file.

// MAX_PLAN_STEPS, AGENTIC_PLAN_PARAM_TEMPLATE, normalizeSlaLevel, getByPath,
// resolveAgenticPlanPath, and resolveAgenticPlanValue now live in
// `./mcp/plan-resolver.js`. Imported at the top of the file.
//
// `buildPlanStepRouting`'s pure logic lives in `./mcp/plan-step-routing.js`.
// We curry the runtime-injected agent router (`routeToAgentWithConfidence`)
// here so the 2 call sites below pass step args only.
const buildPlanStepRouting = (step) => buildPlanStepRoutingImpl(step, routeToAgentWithConfidence);

// Cost budget helpers (addCostSummaryEntry, createCostSummary, normalizeCostBudget,
// normalizeCostBudgetKey, normalizeCostBudgetValue, resolveCostBudgetLimit) now
// live in `./mcp/cost-budget.js`.

// `replayEventHash`, `normalizePolicyAction`, `normalizePolicyExplanation`,
// `buildRollbackContract`, and `buildApprovalStagesFromActions` now live in
// `./mcp/audit-envelope.js`. All five are imported at the top of the file.

// `extractIdempotencyKeyFromParams` and `buildDeterministicMutationManifest`
// now live in `./mcp/mutation-manifest.js`; the orchestrator imports the
// manifest builder above.

// `normalizePolicyAction`, `normalizePolicyExplanation`,
// `buildRollbackContract`, and `buildApprovalStagesFromActions` now live in
// `./mcp/audit-envelope.js`. All four are imported at the top of the file.

// `signAuditArtifact`'s pure logic now lives in `./mcp/audit-signing.js`.
// We read the env vars here so the module stays pure + unit-testable.
const signAuditArtifact = (payload) =>
  signAuditArtifactImpl(payload, {
    signingKey:
      process.env.STATESET_AGENTIC_AUDIT_SIGNING_KEY ||
      process.env.STATESET_AUDIT_SIGNING_KEY ||
      '',
    keyId: process.env.STATESET_AGENTIC_AUDIT_SIGNING_KEY_ID || 'stateset-default',
  });

// `buildDeterministicMutationManifest` now lives in
// `./mcp/mutation-manifest.js` (imported at the top of the file).

// `inferStaticPolicyDomain` and `STATIC_POLICY_DOMAIN_BY_TOKEN` now live in
// `./mcp/policy-domain.js`. `inferStaticPolicyDomain` is imported at the
// top of the file. `TOOL_DOMAIN_BY_TOOL_NAME` is kept as a local alias for
// the same registry map since several call sites reach into it directly.
const TOOL_DOMAIN_BY_TOOL_NAME = TOOL_POLICY_DOMAIN_BY_NAME;

export function getStaticMcpToolDefinitions() {
  return ALL_TOOL_DEFS.map((toolDef) => ({
    name: toolDef.name,
    description: toolDef.description,
    inputSchema: toolDef.inputSchema || {},
    permission: toolDef.permission || 'unknown',
    policyDomain: toolDef.policyDomain || inferStaticPolicyDomain(toolDef.name),
  }));
}

export function getStaticAgenticRuntimeTools() {
  return AGENTIC_RUNTIME_TOOLS.map((toolDef) => ({
    ...toolDef,
    permission: toolDef.permission || 'unknown',
    policyDomain: toolDef.policyDomain || inferStaticPolicyDomain(toolDef.name),
  }));
}

/**
 * Set of read-only tool names, derived from module permission metadata.
 */
const READ_ONLY_TOOLS = new Set(
  ALL_TOOL_DEFS.filter((t) => t.permission === 'read').map((t) => t.name),
);

/**
 * Wrapper that pulls `vectorAutoIndex` off the shared runtime on each call,
 * then delegates to the pure helper in `./mcp/auto-index.js`. This avoids
 * threading the runtime through every tool handler while keeping the core
 * logic injection-friendly + unit-testable.
 *
 * @param {'product'|'customer'|'order'} entityType
 * @param {Object} entity - The created entity (must have .id)
 */
function autoIndexEntity(entityType, entity) {
  autoIndexEntityImpl(getSharedRuntime()?.vectorAutoIndex, entityType, entity);
}

/**
 * Create the StateSet Commerce MCP server
 * @param {Object} options
 * @param {import('@stateset/embedded').Commerce} [options.commerce] - Optional Commerce instance. When omitted, a cached instance is created from dbPath.
 * @param {boolean} options.allowApply - Whether to execute write tools instead of returning preview metadata
 * @param {Object|null} options.autonomousEngine - Optional autonomous engine used by runtime tools such as delegate_to_agent
 * @param {import('./telemetry.js').AgentTelemetry} options.telemetry - Telemetry instance
 * @param {import('./permissions.js').PermissionGate} options.permissionGate - Permission gate instance
 * @param {import('./channels/plugin-api.js').HookRunner} options.hookRunner - Hook runner instance
 * @param {PolicyEngine} options.policyEngine - PolicyEngine instance (optional)
 * @param {string} options.policyStorePath - Policy store root path (optional)
 * @param {string} options.dbPath - Commerce database path (used for ERC-8004 lookups)
 * @param {Object} options.treasury - Treasury configuration (agentId, dbPath, pricingPath, registryPath, ERC-8004 registry)
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
  autonomousEngine = null,
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
  const commerceInstance = commerce || getCommerce(dbPath);

  // ---------------------------------------------------------------------------
  // A2A Store initialization
  // ---------------------------------------------------------------------------
  const a2aStore = new A2AStore({ dbPath: dbPath.replace('.db', '-a2a.db') });

  // Create a commerce wrapper that includes A2A methods
  let createA2AService = () => ({
    createPayment: (p) => a2aStore.createPayment(p),
    getPayment: (id) => a2aStore.getPayment(id),
    updatePayment: (id, u) => a2aStore.updatePayment(id, u),
    listPayments: (f) => a2aStore.listPayments(f),
    sumPayments: (f) => a2aStore.sumPayments(f),
    summarizePayments: (f) => a2aStore.summarizePayments(f),
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
  });

  const commerceWithA2A = adaptCommerceApis(
    extendCommerceWithApis(adaptCommerceForTools(commerceInstance), {
      a2a: () => createA2AService(),
    }),
  );

  // ---------------------------------------------------------------------------
  // Intelligence services initialization (automatic wiring)
  // ---------------------------------------------------------------------------
  // These are lazy-loaded to avoid blocking startup if any module fails.
  Promise.all([
    import('./a2a/agent-memory.js'),
    import('./a2a/rules-engine.js'),
    import('./a2a/idempotency.js'),
    import('./a2a/tracing.js'),
    import('./a2a/cost-analytics.js'),
    import('./a2a/introspection.js'),
    import('./a2a/scheduler.js'),
    import('./a2a/messaging.js'),
    import('./a2a/rate-limiter.js'),
    import('./a2a/integration.js'),
  ])
    .then(
      ([
        { createAgentMemory },
        { createRulesEngine },
        { createIdempotencyGuard },
        { createTracingService },
        { createCostAnalytics },
        { createIntrospectionService },
        { createSchedulerService },
        { createMessagingService },
        { createMcpRateLimiter },
        { createIntegratedA2AService },
      ]) => {
        const memory = createAgentMemory();
        const rules = createRulesEngine();
        const idempotency = createIdempotencyGuard({ ttlMs: 86_400_000 });
        const tracing = createTracingService({ maxSpans: 10_000 });
        const costAnalytics = createCostAnalytics();
        const introspection = createIntrospectionService();
        const scheduler = createSchedulerService();
        const messaging = createMessagingService();
        const rateLimiter = createMcpRateLimiter({
          defaultLimits: { requestsPerMinute: 120 },
          toolOverrides: {
            a2a_pay: { requestsPerMinute: 30 },
            a2a_batch_pay: { requestsPerMinute: 10 },
            a2a_scatter: { requestsPerMinute: 20 },
          },
        });

        commerceWithA2A._agentMemory = memory;
        commerceWithA2A._rulesEngine = rules;
        commerceWithA2A._idempotencyGuard = idempotency;
        commerceWithA2A._tracingService = tracing;
        commerceWithA2A._costAnalytics = costAnalytics;
        commerceWithA2A._introspectionService = introspection;
        commerceWithA2A._schedulerService = scheduler;
        commerceWithA2A._messagingService = messaging;
        commerceWithA2A._rateLimiter = rateLimiter;
        commerceWithA2A._store = a2aStore;

        const originalA2A = commerceWithA2A.a2a;
        if (typeof originalA2A === 'function' && typeof createIntegratedA2AService === 'function') {
          const coreA2AInstance = originalA2A();
          const integratedA2A = createIntegratedA2AService(coreA2AInstance, {
            memory,
            rules,
            idempotency,
            tracing,
            costAnalytics,
            introspection,
          });
          createA2AService = () => integratedA2A;
        }
      },
    )
    .catch((err) => {
      // Graceful degradation — intelligence services are optional
      console.debug('[mcp-server] Intelligence services init skipped:', err.message);
      commerceWithA2A._store = a2aStore;
    });

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

  // `inferPolicyDomain` is bound at module scope (see top of file). Its
  // pure logic lives in `./mcp/policy-domain.js`. Kept the same name so
  // the ~17 call sites below continue to work without churn.

  // `normalizeToolName` and `applyPolicyTransform` now live in
  // `./mcp/policy-helpers.js` — both are pure (no closure deps) and are
  // imported at the top of the file.

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
  // Replay-log primitives — event-stream publisher, ring buffer, persistent
  // JSONL append log, and filtered listing — all live in
  // `./mcp/replay-log.js`. We instantiate one per server with `signAuditArtifact`
  // (module-level) and the active stream injected.
  const fallbackAgenticDir = resolvePolicyPath || path.join(process.cwd(), '.stateset');
  const agenticReplayLogPath = path.join(fallbackAgenticDir, AGENTIC_REPLAY_LOG_FILE);

  const replayLog = createReplayLog({
    logPath: agenticReplayLogPath,
    bufferSize: AGENTIC_REPLAY_BUFFER_SIZE,
    telemetry,
    signAuditArtifact,
    mcpEventStream: activeMcpEventStream,
  });
  const addAgenticReplayEvent = replayLog.addEvent;
  const listAgenticReplayEvents = replayLog.listEvents;

  // Tool runtime metadata + treasury pricing live in `./mcp/pricing.js`.
  // The pricing cache is a per-server factory; `getToolRuntimeMeta` is a
  // pure curry over the registry maps + hint sets. Treasury settings are
  // wired as getters because they're declared later in this function body.
  const pricingCache = createPricingCache({
    treasuryEnabled: () => treasuryEnabled,
    treasuryContextOptions: () => treasuryContextOptions,
  });
  const getAgenticToolPricing = pricingCache.getPricing;
  const getToolRuntimeMeta = (toolName) =>
    buildToolRuntimeMeta(toolName, {
      toolDefsByName: TOOL_DEFS_BY_NAME,
      inferPolicyDomain,
      toolDomainByName: TOOL_DOMAIN_BY_TOOL_NAME,
      compensationHints: AGENTIC_COMPENSATION_HINTS,
      idempotencyHints: AGENTIC_IDEMPOTENCY_HINTS,
    });

  const attachPaymentMetadataToResponse = (response, paymentMetadata = {}) => {
    if (!response || !Array.isArray(response.content) || response.content.length === 0) {
      return response;
    }

    const first = response.content[0];
    if (!first || first.type !== 'text' || typeof first.text !== 'string') {
      return response;
    }

    try {
      const parsed = JSON.parse(first.text);
      const nextPayload = attachPaymentMetadata(parsed, paymentMetadata);
      return {
        ...response,
        content: [{ ...first, text: JSON.stringify(nextPayload) }, ...response.content.slice(1)],
      };
    } catch {
      return response;
    }
  };

  const resolveMppPaymentContext = async ({
    toolName,
    description = '',
    params = {},
    extra = {},
    requestId = null,
    sessionId = null,
  } = {}) => {
    const pricing = await getAgenticToolPricing(toolName);
    if (!pricing) {
      return {
        pricing: null,
        challenge: null,
        credential: null,
        authorized: false,
      };
    }

    const challenge = createPaymentChallenge({
      toolName,
      description,
      pricing,
      params,
      requestId,
      sessionId,
      serviceId: MPP_SERVICE_INFO.id,
      serviceName: MPP_SERVICE_INFO.name,
    });
    const credential = extractPaymentCredential(params, extra);
    if (!credential) {
      return {
        pricing,
        challenge,
        credential: null,
        authorized: false,
        errorPayload: buildPaymentRequiredPayload({ challenge }),
      };
    }

    const verification = verifyPaymentCredential(credential, challenge);
    if (!verification.valid) {
      return {
        pricing,
        challenge,
        credential,
        authorized: false,
        verification,
        errorPayload: buildPaymentRequiredPayload({
          challenge,
          reason: MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE,
          validationError: verification.reason,
        }),
      };
    }

    return {
      pricing,
      challenge,
      credential: verification.credential,
      verification,
      authorized: true,
    };
  };

  const buildPaymentDiscovery = async ({
    format = 'json',
    tool = null,
    pricedOnly = false,
  } = {}) => {
    const normalizedTool = normalizeToolName(tool || '');
    const tools = [];

    for (const toolDef of ALL_TOOL_DEFS) {
      if (normalizedTool && toolDef.name !== normalizedTool) continue;
      const pricing = await getAgenticToolPricing(toolDef.name);
      if (pricedOnly && !pricing) continue;
      tools.push({
        name: toolDef.name,
        description: toolDef.description,
        inputSchema: inputSchemaDefToJsonSchema(toolDef.inputSchema || {}),
        runtime: getToolRuntimeMeta(toolDef.name),
        pricing,
        paymentInfo: buildPaymentInfoFromPricing({
          toolName: toolDef.name,
          description: toolDef.description,
          pricing,
        }),
      });
    }

    if (format === 'openapi') {
      return createPaymentDiscoveryDocument({
        serviceInfo: MPP_SERVICE_INFO,
        tools,
        serverUrl: '/mcp',
      });
    }

    return {
      protocol: 'mpp',
      protocolVersion: MPP_SERVICE_INFO.protocolVersion,
      service: MPP_SERVICE_INFO,
      tools: tools.map((entry) => ({
        name: entry.name,
        description: entry.description,
        runtime: entry.runtime,
        pricing: entry.pricing,
        paymentInfo: entry.paymentInfo,
      })),
    };
  };

  const buildToolCatalog = async ({
    format = 'generic',
    mcpPrefix = null,
    tool = null,
    payableOnly = false,
  } = {}) => {
    const normalizedTool = normalizeToolName(tool || '');
    const normalizedFormat = format || 'generic';
    const tools = [];

    for (const toolDef of ALL_TOOL_DEFS) {
      if (normalizedTool && toolDef.name !== normalizedTool) continue;
      const runtime = getToolRuntimeMeta(toolDef.name);
      const parameters = inputSchemaDefToJsonSchema(toolDef.inputSchema || {});
      const pricing = await getAgenticToolPricing(toolDef.name);
      const paymentInfo = buildPaymentInfoFromPricing({
        toolName: toolDef.name,
        description: toolDef.description,
        pricing,
      });
      const payable = Boolean(paymentInfo);
      if (payableOnly && !payable) continue;

      const resolvedName = mcpPrefix ? `${mcpPrefix}${toolDef.name}` : toolDef.name;
      if (normalizedFormat === 'openai') {
        tools.push({
          type: 'function',
          function: {
            name: toolDef.name,
            description: toolDef.description,
            parameters,
          },
          stateset: {
            permission: toolDef.permission || runtime.permission,
            policyDomain: runtime.policyDomain,
            payable,
            payment: paymentInfo,
          },
        });
        continue;
      }

      tools.push({
        name: normalizedFormat === 'mcp' ? `mcp__stateset-commerce__${toolDef.name}` : resolvedName,
        toolName: toolDef.name,
        description: toolDef.description,
        inputSchema: parameters,
        permission: toolDef.permission || runtime.permission,
        policyDomain: runtime.policyDomain,
        runtime,
        payable,
        paymentInfo,
      });
    }

    return {
      format: normalizedFormat,
      service: MPP_SERVICE_INFO,
      count: tools.length,
      tools,
    };
  };

  let toolDiscoveryEnginePromise = null;
  const getToolDiscoveryEngine = async () => {
    if (toolDiscoveryEnginePromise) return toolDiscoveryEnginePromise;
    toolDiscoveryEnginePromise = (async () => {
      const engine = new ToolDiscoveryEngine();
      const catalog = await buildToolCatalog({ format: 'generic', payableOnly: false });
      for (const tool of catalog.tools) {
        engine.registerTool(tool.toolName || tool.name, {
          name: tool.toolName || tool.name,
          description: tool.description,
          category: tool.policyDomain || 'general',
          purpose: tool.description,
          whenToUse: tool.description,
          inputSchema: tool.inputSchema,
          permission: tool.permission,
          payable: tool.payable || false,
          paymentInfo: tool.paymentInfo || null,
        });
      }
      return engine;
    })();
    return toolDiscoveryEnginePromise;
  };

  const preparePaymentForTool = async ({
    tool,
    params = {},
    requestId = null,
    sessionId = null,
    includeSchema = false,
  } = {}) => {
    const resolvedToolName = normalizeToolName(tool || '');
    if (!resolvedToolName) {
      return {
        success: false,
        payable: false,
        error: 'tool is required',
      };
    }

    const toolDef = TOOL_DEFS_BY_NAME.get(resolvedToolName);
    if (!toolDef) {
      return {
        success: false,
        tool: resolvedToolName,
        payable: false,
        error: `Unknown tool '${resolvedToolName}'`,
      };
    }

    const validation = validateToolInput(toolDef.inputSchema || {}, params || {});
    if (!validation.success) {
      return {
        success: false,
        tool: resolvedToolName,
        payable: false,
        error: `Invalid parameters for tool '${resolvedToolName}'`,
        validation: {
          valid: false,
          issues: formatValidationIssues(validation.error),
        },
      };
    }

    const pricing = await getAgenticToolPricing(resolvedToolName);
    const paymentInfo = buildPaymentInfoFromPricing({
      toolName: resolvedToolName,
      description: toolDef.description,
      pricing,
    });

    if (!pricing || !paymentInfo) {
      return {
        success: true,
        tool: resolvedToolName,
        payable: false,
        service: MPP_SERVICE_INFO,
        validation: { valid: true },
        paymentInfo: null,
        challenge: null,
        reason: 'No pricing configured for this tool.',
        ...(includeSchema
          ? { inputSchema: inputSchemaDefToJsonSchema(toolDef.inputSchema || {}) }
          : {}),
      };
    }

    const challenge = createPaymentChallenge({
      toolName: resolvedToolName,
      description: toolDef.description,
      pricing,
      params: validation.data,
      requestId,
      sessionId,
      serviceId: MPP_SERVICE_INFO.id,
      serviceName: MPP_SERVICE_INFO.name,
    });
    const primaryMethod = Array.isArray(challenge.paymentMethods)
      ? challenge.paymentMethods[0] || null
      : null;
    const credentialTemplate = {
      protocol: MPP_PROTOCOL,
      protocolVersion: MPP_VERSION,
      type: 'credential',
      challengeId: challenge.challengeId,
      payer: '<payer-id>',
      method: primaryMethod
        ? {
            kind: primaryMethod.kind || null,
            asset: primaryMethod.asset || null,
            network: primaryMethod.network || null,
          }
        : null,
      amount: challenge.amount,
      binding: challenge.binding,
      authorization: {
        type: '<signature-or-proof>',
      },
    };

    return {
      success: true,
      tool: resolvedToolName,
      payable: true,
      service: MPP_SERVICE_INFO,
      paymentInfo,
      challenge,
      acceptedPaymentMethods: challenge.paymentMethods || [],
      validation: { valid: true },
      credentialTemplate,
      retryExample: {
        jsonrpc: '2.0',
        id: requestId || '<request-id>',
        method: resolvedToolName,
        params: validation.data,
        _meta: {
          payment: credentialTemplate,
        },
      },
      ...(includeSchema
        ? { inputSchema: inputSchemaDefToJsonSchema(toolDef.inputSchema || {}) }
        : {}),
    };
  };

  // buildPolicyDecisionBundle's body lives in ./mcp/policy-evaluator.js,
  // and is invoked by `createEvaluatePolicy` below — no orchestrator-level
  // call sites remain after the extraction.

  const getAgenticRuntimeContract = async ({ tool, includeLegacyDefaults = false } = {}) => {
    const targetTool = tool ? normalizeToolName(tool) : null;
    const normalizedTools = await Promise.all(
      ALL_TOOL_DEFS.filter((candidate) => !targetTool || candidate?.name === targetTool)
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
      mpp: {
        enabled: true,
        service: MPP_SERVICE_INFO,
        transport: {
          jsonrpc: {
            paymentRequiredCode: MPP_JSONRPC_PAYMENT_REQUIRED_CODE,
            paymentRequiredMessage: MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE,
            credentialMetaKey: 'payment',
            receiptMetaKey: 'payment',
          },
          http: {
            paymentRequiredStatus: 402,
            discoveryExtensions: ['x-payment-info', 'x-service-info'],
          },
        },
        intents: ['charge', 'session'],
        methodAdapters: listPaymentMethodAdapters(),
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
        ...toolContext,
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

  // Mutation simulate/replay bodies live in ./mcp/mutation-simulator.js.
  // We wire the per-server helpers (getToolRuntimeMeta, executeToolStepInPlan,
  // addAgenticReplayEvent, listAgenticReplayEvents) here.
  const mutationDeps = {
    getToolRuntimeMeta,
    inferPolicyDomain,
    executeToolStepInPlan,
    addAgenticReplayEvent,
    listAgenticReplayEvents,
  };
  const simulateMutationToolCall = createSimulateMutationToolCall(mutationDeps);
  const replayMutationToolCall = createReplayMutationToolCall(mutationDeps);

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

  // evaluatePolicy's body lives in ./mcp/policy-evaluator.js. The factory
  // wires the per-server policy engine, readiness promise, and audit deps.
  const evaluatePolicy = createEvaluatePolicy({
    policyEngine: policyEngineInstance,
    policyReady: policyLoad,
    allowApply,
    telemetry,
    inferPolicyDomain,
    getToolRuntimeMeta,
    signAuditArtifact,
    bundleVersion: AGENTIC_POLICY_DECISION_BUNDLE_VERSION,
  });

  // ---------------------------------------------------------------------------
  // Treasury helpers
  // ---------------------------------------------------------------------------

  const treasuryAgentId = treasury?.agentId || process.env.TREASURY_AGENT || 'default';
  const treasuryDbPath = treasury?.dbPath || process.env.TREASURY_DB || null;
  const treasuryPricingPath = treasury?.pricingPath || process.env.TREASURY_PRICING_PATH || null;
  const treasuryRegistryPath = treasury?.registryPath || process.env.TREASURY_REGISTRY_PATH || null;
  const treasuryContextOptions = {
    ...(treasuryDbPath ? { dbPath: treasuryDbPath } : {}),
    ...(treasuryPricingPath ? { pricingPath: treasuryPricingPath } : {}),
    ...(treasuryRegistryPath ? { registryPath: treasuryRegistryPath } : {}),
  };
  const treasuryRegistry =
    treasury?.erc8004Registry || process.env.TREASURY_ERC8004_REGISTRY || null;
  const treasuryEnabled = Boolean(
    treasury?.enabled ||
    treasuryDbPath ||
    treasuryPricingPath ||
    treasuryRegistryPath ||
    treasury?.chainId ||
    treasury?.tokenSymbol ||
    treasuryRegistry ||
    String(process.env.TREASURY_BILLING || '').toLowerCase() === 'true' ||
    process.env.TREASURY_CHAIN ||
    process.env.TREASURY_TOKEN,
  );
  const treasuryIdentityDbPath = treasury?.erc8004DbPath || dbPath;

  // ERC-8004 identity resolution + treasury charging live in
  // ./mcp/tool-wrappers.js. The identity resolver caches per-server; the
  // charger is a closure over treasury config + identity.
  const treasuryIdentity = createTreasuryIdentityResolver({
    registry: treasuryRegistry,
    dbPath: treasuryIdentityDbPath,
    agentId: treasuryAgentId,
  });
  const resolveTreasuryAgentId = treasuryIdentity.getAgentId;
  const buildTreasuryIdentityMetadata = treasuryIdentity.getMetadata;

  // ---------------------------------------------------------------------------
  // Telemetry & audit helpers — bodies in ./mcp/tool-wrappers.js
  // ---------------------------------------------------------------------------
  const wrapWithTelemetry = createWrapWithTelemetry(telemetry);
  const buildAuditContext = buildAuditContextImpl;
  const maybeChargeForTool = createToolCharger({
    treasuryEnabled: () => treasuryEnabled,
    treasuryContextOptions: () => treasuryContextOptions,
    allowApply,
    identity: treasuryIdentity,
  });

  const shouldReturnStructuredResults =
    structuredToolResults ||
    String(process.env.STATESSET_MCP_STRUCTURED_TOOL_RESULTS || '').toLowerCase() === 'true' ||
    String(process.env.STATESSET_MCP_STRUCTURED_TOOL_RESULTS || '').toLowerCase() === '1';

  // ---------------------------------------------------------------------------
  // Tool wrapper helpers — add hooks, permission checks, treasury, and telemetry
  // ---------------------------------------------------------------------------

  // Result-envelope builders (the `_agentic` schema) live in
  // ./mcp/result-builders.js. We curry the `structured` flag here so the
  // ~17 call sites in this file don't have to thread it through.
  const resultOptions = { structured: shouldReturnStructuredResults };
  const buildToolResultResponse = (result, status, startedAt, toolMeta = {}, isError = false) =>
    buildToolResultResponseImpl(result, status, startedAt, toolMeta, isError, resultOptions);
  const attachStructuredToolMetadataToResponse = (response, status, startedAt, toolMeta = {}) =>
    attachStructuredToolMetadataToResponseImpl(
      response,
      status,
      startedAt,
      toolMeta,
      resultOptions,
    );

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

        const mpp = await resolveMppPaymentContext({
          toolName: name,
          description,
          params: nextArgs,
          extra,
          requestId: baseToolContext.requestId,
          sessionId: baseToolContext.sessionId,
        });

        if (mpp?.pricing && !mpp.authorized) {
          await logEvent('payment_required', {
            params: nextArgs,
            permission,
            policy,
            charge: {
              paymentRequired: true,
              pricing: mpp.pricing,
              challenge: mpp.challenge,
            },
            error: mpp?.verification?.reason || MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE,
          });
          return buildToolResultResponse(
            {
              ...mpp.errorPayload,
              tool: name,
            },
            'payment_required',
            startedAt,
            {
              requestId: baseToolContext.requestId,
              sessionId: baseToolContext.sessionId,
              policy,
              permission,
              charge: {
                paymentRequired: true,
                pricing: mpp.pricing,
                challenge: mpp.challenge,
              },
              mutationManifest: buildMutationManifest(
                nextArgs,
                policy,
                permission,
                'payment_required',
              ),
              name,
            },
            true,
          );
        }

        charge = await maybeChargeForTool(name, extra, {
          allowChargeWrite: Boolean(mpp?.authorized),
          paymentCredential: mpp?.credential || null,
        });
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
        let result = await wrapped(nextArgs, extra);
        if (mpp?.authorized && charge?.charged) {
          const receipt = createPaymentReceipt({
            challenge: mpp.challenge,
            credential: mpp.credential,
            charge,
            toolName: name,
            requestId: baseToolContext.requestId,
            sessionId: baseToolContext.sessionId,
          });
          result = attachPaymentMetadata(result, {
            protocol: 'mpp',
            receipt,
            credentialId: mpp?.credential?.credentialId || null,
          });
        }
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
        let maybeStructured = attachStructuredToolMetadataToResponse(result, 'success', startedAt, {
          requestId: baseToolContext.requestId,
          sessionId: baseToolContext.sessionId,
          policy,
          permission,
          charge,
          mutationManifest: buildMutationManifest(nextArgs, policy, permission, 'success'),
          name,
        });
        if (mpp?.authorized && charge?.charged) {
          maybeStructured = attachPaymentMetadataToResponse(maybeStructured, {
            protocol: 'mpp',
            receipt: result?._meta?.payment?.receipt || null,
            credentialId: mpp?.credential?.credentialId || null,
          });
        }
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
    autonomousEngine,
    autoIndexEntity,
    resolveTreasuryAgentId,
    treasuryContextOptions,
    buildAuditContext,
    buildTreasuryIdentityMetadata,
    agentConfig,
    mcpEventStream: activeMcpEventStream,
    getAgenticRuntimeContract,
    getToolCatalog: buildToolCatalog,
    getToolDiscoveryEngine,
    getPaymentDiscovery: buildPaymentDiscovery,
    preparePaymentForTool,
    executeAgenticPlan,
    simulateAgenticPlan,
    simulateMutationToolCall,
    replayMutationToolCall,
    getAgenticReplayLog: listAgenticReplayEvents,
    policyEngine: policyEngineInstance,
  };

  const getToolDefinitions = ({ format = 'generic', mcpPrefix = null } = {}) => {
    return ALL_TOOL_DEFS.map((toolDef) => {
      const runtime = getToolRuntimeMeta(toolDef.name);
      const parameters = inputSchemaDefToJsonSchema(toolDef.inputSchema || {});
      const baseName = toolDef.name;
      const resolvedName = mcpPrefix ? `${mcpPrefix}${baseName}` : baseName;
      const descriptor = {
        name: resolvedName,
        toolName: baseName,
        description: toolDef.description,
        inputSchema: parameters,
        permission: toolDef.permission || runtime.permission,
        policyDomain: runtime.policyDomain,
        runtime,
      };

      if (format === 'openai') {
        return {
          type: 'function',
          function: {
            name: baseName,
            description: toolDef.description,
            parameters,
          },
          stateset: {
            permission: descriptor.permission,
            policyDomain: descriptor.policyDomain,
          },
        };
      }

      if (format === 'anthropic') {
        return {
          name: baseName,
          description: toolDef.description,
          input_schema: parameters,
          stateset: {
            permission: descriptor.permission,
            policyDomain: descriptor.policyDomain,
          },
        };
      }

      if (format === 'mcp') {
        return {
          ...descriptor,
          name: `mcp__stateset-commerce__${baseName}`,
        };
      }

      return descriptor;
    });
  };

  const getRawToolDefinitions = () => {
    return ALL_TOOL_DEFS.map((toolDef) => ({
      name: toolDef.name,
      description: toolDef.description,
      inputSchema: toolDef.inputSchema || {},
      permission: toolDef.permission || 'unknown',
      policyDomain:
        toolDef.policyDomain ||
        TOOL_DOMAIN_BY_TOOL_NAME[toolDef.name] ||
        inferPolicyDomain(toolDef.name),
      runtime: getToolRuntimeMeta(toolDef.name),
    }));
  };

  const executeTool = async (toolName, params = {}, options = {}) => {
    const requestId = options.requestId || randomUUID();
    const sessionId = options.sessionId || requestId;
    const dryRun = options.dryRun === true;
    const normalizedToolName = normalizeToolName(toolName);

    const outcome = await executeToolStepInPlan({
      toolName: normalizedToolName,
      params,
      policyDomain: options.policyDomain || null,
      requestId,
      sessionId,
      dryRun,
      stepIndex: 0,
      includeHooks: options.includeHooks ?? true,
      isRollback: options.isRollback || false,
      extra: options.extra || {},
    });

    await addAgenticReplayEvent({
      eventId: randomUUID(),
      tool: normalizedToolName,
      status: outcome.status,
      requestId,
      sessionId,
      policyDomain:
        outcome?.policy?.domain || options.policyDomain || inferPolicyDomain(normalizedToolName),
      occurredAt: new Date().toISOString(),
      elapsedMs: outcome.elapsedMs || 0,
      params: compactReplayValue(outcome.params || params || {}),
      paramsHash: outcome.paramsHash || replayEventHash(outcome.params || params || {}),
      result: compactReplayValue(outcome.result || null),
      resultHash: outcome.resultHash || replayEventHash(outcome.result || null),
      policy: compactReplayValue(outcome.policy || null),
      permission: compactReplayValue(outcome.permission || null),
      charge: compactReplayValue(outcome.charge || null),
      error: outcome.error || null,
      notes: compactReplayValue({
        directExecution: true,
        dryRun,
        includeHooks: options.includeHooks ?? true,
      }),
      source: 'embedded_agent_toolkit',
      agentic: true,
    });

    return {
      success:
        outcome.status === 'success' ||
        outcome.status === 'dry_run_success' ||
        outcome.status === 'rollback_success',
      requestId,
      sessionId,
      ...outcome,
    };
  };

  const executeToolWithPayment = async (toolName, params = {}, options = {}) => {
    const { payment = {}, ...executionOptions } = options || {};
    return executeMppToolWithPayment({
      executor: executeTool,
      toolName: normalizeToolName(toolName),
      params,
      executionOptions,
      payment,
    });
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
  server.getToolDefinitions = getToolDefinitions;
  server.getRawToolDefinitions = getRawToolDefinitions;
  server.getToolCatalog = buildToolCatalog;
  server.getToolDiscoveryEngine = getToolDiscoveryEngine;
  server.getPaymentDiscovery = buildPaymentDiscovery;
  server.preparePayment = preparePaymentForTool;
  server.executeTool = executeTool;
  server.executeToolWithPayment = executeToolWithPayment;
  server.connect = (...args) => server.instance.connect(...args);
  server.close = (...args) => server.instance.server.close(...args);
  server.getRuntimeContract = getAgenticRuntimeContract;
  server.simulatePlan = simulateAgenticPlan;
  server.executePlan = executeAgenticPlan;
  server.simulateMutation = simulateMutationToolCall;
  server.replayMutation = replayMutationToolCall;
  server.getReplayLog = listAgenticReplayEvents;
  return server;
}

/**
 * All MCP tool names in the `mcp__<server>__<tool>` format expected by the harness.
 */
export const TOOL_NAMES = getStaticMcpToolDefinitions().map(
  (tool) => `mcp__stateset-commerce__${tool.name}`,
);

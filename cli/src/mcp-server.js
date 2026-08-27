/**
 * MCP Server for StateSet Commerce operations
 *
 * Thin orchestrator that loads tools from domain modules and wraps them
 * with permission checks, telemetry, treasury charging, and error handling.
 */

import { createSdkMcpServer } from '@anthropic-ai/claude-agent-sdk';
import path from 'path';
// `z` (zod) is now only used inside `./mcp/agentic-runtime-tools.js` and
// the per-domain tool modules; mcp-server.js no longer needs it directly.
import { getSharedRuntime } from './channels/plugin-runtime.js';
import { A2AStore } from './a2a/store.js';
import { PolicyEngine } from './policies/engine.js';
import { createMcpEventStreamer } from './mcp-event-streamer.js';
import { AGENTIC_RUNTIME_TOOLS } from './mcp/agentic-runtime-tools.js';
import { AGENTIC_COMPENSATION_HINTS, AGENTIC_IDEMPOTENCY_HINTS } from './mcp/compensation.js';
import {
  inferPolicyDomain as inferPolicyDomainImpl,
  inferStaticPolicyDomain,
} from './mcp/policy-domain.js';
import { autoIndexEntity as autoIndexEntityImpl } from './mcp/auto-index.js';
import { adaptCommerceForTools, extendCommerceWithApis } from './mcp/commerce-adapter.js';
import { buildPlanStepRouting as buildPlanStepRoutingImpl } from './mcp/plan-step-routing.js';
import { signAuditArtifact as signAuditArtifactImpl } from './mcp/audit-signing.js';
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
// Aug 2026 extraction — orchestration factories (see each module's header).
import { createA2AServiceBinding, initializeIntelligenceServices } from './mcp/a2a-service.js';
import { buildReadOnlyToolSet, createCheckPermission } from './mcp/permission-gating.js';
import { createPreparePaymentForTool, createResolveMppPaymentContext } from './mcp/mpp-payment.js';
import { createToolCatalogHelpers } from './mcp/tool-catalog.js';
import { createSimulateAgenticPlan } from './mcp/plan-simulator.js';
import { createExecuteToolStepInPlan } from './mcp/plan-step-executor.js';
import { createRunPlanRollback } from './mcp/plan-rollback.js';
import { createExecuteAgenticPlan } from './mcp/plan-executor.js';
import { createToolDispatch } from './mcp/tool-dispatch.js';
import { routeToAgentWithConfidence } from './agent-router.js';
import { adaptCommerceApis } from './commerce.js';
import { getCommerce } from './database.js';
import { KERNEL_CAPABILITY_BY_TOOL, createKernelToolExecutor } from './kernel-tool-execution.js';
import { selectStrictKernelToolDefinitions } from './kernel-boundary.js';
// SUPPORTED_AGENT_NAMES* now consumed by `./mcp/agentic-runtime-tools.js`.
import { buildMppServiceInfo } from './mpp/index.js';

// Domain tool registry
import {
  ALL_DOMAIN_TOOLS,
  TOOL_MODULE_BY_NAME,
  TOOL_POLICY_DOMAIN_BY_NAME,
} from './tools/domain-registry.js';
import { resolveMcpToolDomains } from './mcp/tool-profiles.js';

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
const READ_ONLY_TOOLS = buildReadOnlyToolSet(ALL_TOOL_DEFS);

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
  kernel = null,
  toolProfile = 'all',
  toolDomains = [],
}) {
  const strictKernelBoundary = Boolean(kernel && kernel.strict !== false);
  const selectedDomains = resolveMcpToolDomains({ profile: toolProfile, domains: toolDomains });
  const profileToolDefs = ALL_TOOL_DEFS.filter(
    (tool) =>
      !TOOL_MODULE_BY_NAME[tool.name] || selectedDomains.has(TOOL_MODULE_BY_NAME[tool.name]),
  );
  const exposedToolDefs = strictKernelBoundary
    ? selectStrictKernelToolDefinitions(profileToolDefs, KERNEL_CAPABILITY_BY_TOOL)
    : profileToolDefs;
  const exposedToolDefsByName = new Map(
    exposedToolDefs.map((tool) => [tool?.name, tool]).filter(([name]) => Boolean(name)),
  );
  const commerceInstance = commerce || getCommerce(dbPath);
  const executeGovernedTool = createKernelToolExecutor({
    commerce: commerceInstance,
    kernel,
    allowApply,
    agentConfig,
  });

  // ---------------------------------------------------------------------------
  // A2A Store initialization
  // ---------------------------------------------------------------------------
  // Keep A2A state in the commerce database. The A2A runtime uses distinct
  // names for its quote/card projections while `a2a_escrows` is deliberately
  // shared with the native kernel, so escrow release and its receipt/outbox
  // commit atomically against the record created by the public A2A tools.
  const a2aStore = new A2AStore({ dbPath });

  // The A2A accessor is late-bound: `./mcp/a2a-service.js` starts it as a
  // pass-through over the store and swaps in the integrated service once
  // the intelligence modules finish loading.
  const a2aBinding = createA2AServiceBinding(a2aStore);

  const commerceWithA2A = adaptCommerceApis(
    extendCommerceWithApis(adaptCommerceForTools(commerceInstance), {
      a2a: a2aBinding.a2a,
    }),
  );

  // ---------------------------------------------------------------------------
  // Intelligence services initialization (automatic wiring)
  // ---------------------------------------------------------------------------
  // Lazy-loaded (body in ./mcp/a2a-service.js) so a failing module never
  // blocks startup.
  initializeIntelligenceServices({
    commerceWithA2A,
    a2aStore,
    setA2AServiceFactory: a2aBinding.setFactory,
  });

  // ---------------------------------------------------------------------------
  // Permission helpers — body in ./mcp/permission-gating.js
  // ---------------------------------------------------------------------------

  const isReadOnly = (toolName) => READ_ONLY_TOOLS.has(toolName);
  const checkPermission = createCheckPermission({
    permissionGate,
    telemetry,
    allowApply,
    isReadOnly,
  });

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
      toolDefsByName: exposedToolDefsByName,
      inferPolicyDomain,
      toolDomainByName: TOOL_DOMAIN_BY_TOOL_NAME,
      compensationHints: AGENTIC_COMPENSATION_HINTS,
      idempotencyHints: AGENTIC_IDEMPOTENCY_HINTS,
    });

  // MPP payment context + `prepare_payment` live in ./mcp/mpp-payment.js.
  const resolveMppPaymentContext = createResolveMppPaymentContext({
    getAgenticToolPricing,
    serviceInfo: MPP_SERVICE_INFO,
  });
  const preparePaymentForTool = createPreparePaymentForTool({
    toolDefsByName: exposedToolDefsByName,
    getAgenticToolPricing,
    serviceInfo: MPP_SERVICE_INFO,
  });

  // Catalog / discovery / runtime-contract projections live in
  // ./mcp/tool-catalog.js.
  const {
    buildPaymentDiscovery,
    buildToolCatalog,
    getToolDiscoveryEngine,
    getAgenticRuntimeContract,
    getToolDefinitions,
    getRawToolDefinitions,
  } = createToolCatalogHelpers({
    allToolDefs: exposedToolDefs,
    toolDomainByName: TOOL_DOMAIN_BY_TOOL_NAME,
    serviceInfo: MPP_SERVICE_INFO,
    resultSchemaVersion: AGENTIC_TOOL_RESULT_SCHEMA_VERSION,
    getAgenticToolPricing,
    getToolRuntimeMeta,
    inferPolicyDomain,
  });

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

  // ---------------------------------------------------------------------------
  // Plan execution — bodies in ./mcp/plan-step-executor.js,
  // ./mcp/plan-simulator.js, ./mcp/plan-rollback.js, ./mcp/plan-executor.js
  // ---------------------------------------------------------------------------

  // `toolContext` is assembled below (it references the plan helpers), so the
  // step executor reads it lazily.
  const executeToolStepInPlan = createExecuteToolStepInPlan({
    toolDefsByName: exposedToolDefsByName,
    inferPolicyDomain,
    getToolRuntimeMeta,
    hookRunner,
    allowApply,
    evaluatePolicy,
    checkPermission,
    resolveMppPaymentContext,
    maybeChargeForTool,
    wrapWithTelemetry,
    getToolContext: () => toolContext,
    executeGovernedTool,
  });

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

  const simulateAgenticPlan = createSimulateAgenticPlan({
    inferPolicyDomain,
    buildPlanStepRouting,
    getToolRuntimeMeta,
    evaluatePolicy,
    checkPermission,
    getAgenticToolPricing,
  });

  const runPlanRollback = createRunPlanRollback({
    toolDefsByName: exposedToolDefsByName,
    inferPolicyDomain,
    executeToolStepInPlan,
    addAgenticReplayEvent,
  });

  const executeAgenticPlan = createExecuteAgenticPlan({
    inferPolicyDomain,
    getToolRuntimeMeta,
    buildPlanStepRouting,
    getAgenticToolPricing,
    executeToolStepInPlan,
    addAgenticReplayEvent,
    runPlanRollback,
  });

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
  // ---------------------------------------------------------------------------
  // Tool dispatch — bodies in ./mcp/tool-dispatch.js
  // ---------------------------------------------------------------------------
  const { executeTool, executeToolWithPayment, adaptTool } = createToolDispatch({
    toolDomainByName: TOOL_DOMAIN_BY_TOOL_NAME,
    inferPolicyDomain,
    getToolRuntimeMeta,
    hookRunner,
    allowApply,
    evaluatePolicy,
    checkPermission,
    resolveMppPaymentContext,
    maybeChargeForTool,
    wrapWithTelemetry,
    addAgenticReplayEvent,
    buildToolResultResponse,
    attachStructuredToolMetadataToResponse,
    executeToolStepInPlan,
    toolContext,
    executeGovernedTool,
  });

  // ---------------------------------------------------------------------------
  // Build and return the MCP server
  // ---------------------------------------------------------------------------

  // The adapted tool list is the single source of truth for every transport we
  // serve. The agent-sdk consumes it in-process; `createStatesetV2McpServer`
  // (src/mcp/v2-server.js) registers the same objects on a protocol-2026-07-28
  // server, so the two can never drift apart.
  const adaptedTools = exposedToolDefs.map(adaptTool);

  const server = createSdkMcpServer({
    name: 'stateset-commerce',
    version: '1.0.0',
    tools: adaptedTools,
  });

  server.getAdaptedTools = () => adaptedTools;

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

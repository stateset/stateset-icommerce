/**
 * StateSet CLI - Main Entry Point
 *
 * Exports all public modules for programmatic use.
 */

// Embedded Commerce + Agent Toolkit
export { Commerce, createCommerce, getCommerceCtor } from './commerce.js';
export { createEmbeddedAgentKit, createEmbeddedAgentToolkit } from './agent-toolkit.js';

// Core harness and agent loop
export {
  runAgentLoop,
  runAgentStream,
  createAgentStreamSession,
  createAgentSession,
  getQueueStats,
  removeQueueLane,
  clearQueueLanes,
  routeToAgent,
  routeToAgentWithConfidence,
  listAgents,
  AGENTS,
} from './claude-harness.js';
export {
  SUPPORTED_AGENT_NAMES,
  SUPPORTED_AGENT_NAMES_DESCRIPTION,
  isSupportedAgentName,
} from './agent-catalog.js';

// MCP Server
export { createStatesetMcpServer, TOOL_NAMES } from './mcp-server.js';
export { createMcpEventStreamer } from './mcp-event-streamer.js';
export {
  MPP_HTTP_PAYMENT_REQUIRED_STATUS,
  MPP_JSONRPC_PAYMENT_REQUIRED_CODE,
  MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE,
  MPP_PROTOCOL,
  MPP_SUPPORTED_INTENTS,
  MPP_VERSION,
  attachPaymentMetadata,
  buildHttpPaymentRequiredResponse,
  buildPaymentRetryExtra,
  buildMppServiceInfo,
  buildPaymentInfoFromPricing,
  buildPaymentRequiredPayload,
  createPaymentChallenge,
  createPaymentCredential,
  createPaymentDiscoveryDocument,
  createPaymentReceipt,
  executeMppToolWithPayment,
  extractPaymentChallenge,
  extractPaymentCredential,
  listPaymentMethodAdapters,
  MppPaymentPolicyError,
  resolvePaymentMethodAdapter,
  validatePaymentChallenge,
  verifyPaymentCredential,
} from './mpp/index.js';
export {
  attachPaymentReceiptToHttpResponse,
  buildHttpRouteDiscoveryDocument,
  buildHttpPaymentHeaders,
  createMppHttpRouteHandler,
  extractHttpPaymentCredential,
} from './mpp/http.js';
export {
  createMppHttpAgent,
  discoverMppHttpService,
  extractPayableHttpRoutes,
  fetchMppDiscoveryDocument,
  fetchMppServiceInfo,
  extractHttpPaymentChallenge,
  extractHttpPaymentReceipt,
  mppFetch,
} from './mpp/agent.js';

// Telemetry & Observability
export { AgentTelemetry, noOpTelemetry, createTelemetry, Span } from './telemetry.js';

// Permissions & Guardrails
export {
  PermissionGate,
  createPermissionGate,
  PERMISSION_LEVELS,
  TOOL_PERMISSIONS,
  DEFAULT_GUARDRAILS,
  getLevelFromFlags,
} from './permissions.js';

// Output Formatting
export { RichOutput, createOutput, ICONS, formatStructuredOutput } from './output.js';

// Scaffolding Server (Storefront Creation)
export {
  createScaffoldMcpServer,
  SCAFFOLD_TOOL_NAMES,
  SCAFFOLD_MCP_TOOL_NAMES,
} from './scaffold-server.js';

// x402 MCP Server
export { createX402McpServer, X402_MCP_TOOL_NAMES } from './x402-mcp-server.js';

// ERC-8004 Identity Registry
export {
  registerIdentity as registerErc8004Identity,
  setAgentWallet as setErc8004Wallet,
  getIdentity as getErc8004Identity,
  getIdentityByWallet as getErc8004IdentityByWallet,
  listIdentities as listErc8004Identities,
} from './erc8004/index.js';

// Treasury
export {
  loadTreasuryContext,
  resolveToken as resolveTreasuryToken,
  listTokens as listTreasuryTokens,
  addRegistryToken as addTreasuryToken,
  removeRegistryToken as removeTreasuryToken,
  addPricingRule as addTreasuryPricingRule,
  removePricingRuleEntry as removeTreasuryPricingRule,
  getToolPricing as getTreasuryToolPricing,
  computeBalanceDisplay as computeTreasuryBalanceDisplay,
  recordDeposit as recordTreasuryDeposit,
  recordWithdrawal as recordTreasuryWithdrawal,
  recordFee as recordTreasuryFee,
  buyTokens as buyTreasuryTokens,
  syncOnChainBalance as syncTreasuryOnChainBalance,
  ensureAgentWallet as ensureTreasuryAgentWallet,
} from './treasury/index.js';

// Sync (Verifiable Event Sync)
export {
  Outbox,
  createOutbox,
  SyncConfig,
  loadSyncConfig,
  saveSyncConfig,
  SequencerClient,
  createSequencerClient,
  SyncEngine,
  createSyncEngine,
  wrapCommerceWithEvents,
  EventCapture,
} from './sync/index.js';

// ============================================================================
// New Modules (v0.1.8)
// ============================================================================

// Structured Logging
export { Logger, createLogger, LOG_LEVELS, ToolCallLogger, createRequestLogger } from './logger.js';

// Interactive Prompts
export {
  prompt,
  confirm,
  select,
  promptSchema,
  InteractivePrompts,
  isInteractive,
  interactiveOr,
} from './prompts.js';

// Plugin System
export {
  PluginLoader,
  createPluginLoader,
  scaffoldPlugin,
  PLUGIN_TEMPLATE,
} from './plugins/loader.js';

// Harness Settings & Persistence
export { DEFAULT_AGENT_SETTINGS, loadAgentSettings, resetAgentSettingsCache } from './settings.js';

export {
  collectAgentOsStatus,
  createRunbookSkill,
  formatAgentContext,
  formatAgentStatus,
  formatMemoryList,
  formatNextActions,
  formatRunbookCreated,
  formatSessionList,
  formatSetupResult,
  formatSkillList,
  inspectAgentContext,
  listAgentSessions,
  listAgentSkills,
  saveOperationalMemory,
  setupAgentWorkspace,
  searchOperationalMemory,
} from './agent-os.js';

export {
  CredentialStore,
  getCredentialStore,
  resolveProviderApiKey,
  resetCredentialStore,
} from './credentials.js';

export {
  AgentSessionStore,
  getAgentSessionStore,
  resetAgentSessionStore,
} from './agent-session-store.js';

export { getHarnessHookRunner, ensureHarnessPluginsLoaded } from './harness-hooks.js';

export { redactSensitive, redactObject } from './privacy.js';

// Offline Support
export {
  checkApiAvailability,
  OfflineManager,
  createOfflineManager,
  showOfflineWarning,
  OfflineCache,
} from './offline.js';

// Dry Run Mode
export {
  DryRunManager,
  createDryRunManager,
  formatDryRunResult,
  parseDryRunFlag,
  PREVIEWABLE_OPERATIONS,
} from './dry-run.js';

// Modular Commands
export {
  commands,
  RESOURCE_ALIASES,
  ACTION_ALIASES,
  expandResource,
  expandAction,
  getCommand,
  executeCommand,
  generateHelp,
  getCompletions,
} from './commands/index.js';

// Modular Tools
export {
  ToolRegistry,
  createToolRegistry,
  getToolsForAgent,
  AGENT_TOOL_CATEGORIES,
} from './tools/index.js';

// Configuration
export {
  MODELS,
  DEFAULT_MODEL,
  CLI_DEFAULTS,
  FEATURES,
  getModelForAgent,
  getParseArgsOptions,
  MODEL_FOR_TASK,
  AGENT_MODELS,
} from './config.js';

// ============================================================================
// Enhanced Modules (v0.1.9)
// ============================================================================

// Enhanced Error Handling
export {
  StateSetError,
  ValidationError,
  PermissionError,
  ApiError,
  DatabaseError,
  ToolError,
  ConfigError,
  TimeoutError,
  NotFoundError,
  ErrorHandler,
  createErrorHandler,
  withRetry,
  EXIT_CODES,
} from './errors.js';

// Smart Suggestions
export {
  SuggestionEngine,
  createSuggestionEngine,
  formatSuggestion,
  INTENT_PATTERNS,
  COMMAND_ALIASES,
} from './suggestions.js';

// Session Persistence
export {
  SessionManager,
  createSessionManager,
  CommandHistory,
  createCommandHistory,
  DEFAULT_SESSION_DIR,
} from './session.js';

// Database Management
export {
  DatabaseManager,
  createDatabaseManager,
  getGlobalManager,
  getCommerce,
} from './database.js';

// x402 Payments
export {
  X402SequencerClient,
  computeX402SigningHash,
  normalizeAsset,
  normalizeNetwork,
  networkChainId,
  signX402Hash,
  verifyX402Signature,
  encodeBase64Json,
  decodeBase64Json,
  hashToHex,
  hexToBytes,
  x402Fetch,
  createX402Agent,
  decodePaymentHeader,
  decodeReceiptHeader,
  verifyPaymentHeader,
  BudgetExceededError,
  createBudgetState,
  getDefaultBudgetStateFile,
  getDefaultX402ConfigPath,
  loadX402Config,
  saveX402Config,
  resolveX402ConfigPath,
  pickConfigValue,
  buildExactEvmPaymentRequired,
  createExactEvmResourceServerHandler,
  verifyFacilitatedPayment,
  settleFacilitatedPayment,
  buildFacilitatorSupportedResponse,
  createFacilitatorHttpHandler,
} from './x402/index.js';

// Tutorial System
export {
  TUTORIALS,
  TutorialRunner,
  createTutorialRunner,
  checkFirstRun,
  showWelcome,
} from './tutorial.js';

// Request Context
export {
  RequestContext,
  Span as ContextSpan,
  runWithContext,
  getContext,
  getOrCreateContext,
  withContext,
  withChildContext,
  withSpan,
  ContextLogger,
  createContextLogger,
  createContextMiddleware,
} from './context.js';

// ============================================================================
// A2A Commerce (Agent-to-Agent Payments)
// ============================================================================

export { createA2AService } from './a2a/index.js';
export { A2AStore, defaultA2ADbPath } from './a2a/store.js';
export { createEscrowService } from './a2a/escrow.js';
export { createDisputeService } from './a2a/disputes.js';
export { createReputationService } from './a2a/reputation.js';
export { createNotificationService } from './a2a/notifications.js';
export { createA2ASubscriptionService } from './a2a/subscriptions.js';
export { createSplitPaymentService } from './a2a/splits.js';
export { createEventStreamService } from './a2a/event-stream.js';

// A2A Agent Runtime & Strategies
export { createAgentRuntime, makeCommerceProxy } from './a2a/agent-runtime.js';
export {
  createAlwaysAcceptStrategy,
  createBudgetGatedStrategy,
  createNegotiatorStrategy,
  createBestOfNStrategy,
  createReputationAwareStrategy,
  createDynamicPricingStrategy,
} from './a2a/strategies.js';

// A2A Marketplace, SLA, Workflows
export { createMarketplaceService } from './a2a/marketplace.js';
export { createSLAService } from './a2a/sla.js';
export { createWorkflowService } from './a2a/workflows.js';

// A2A Event Wiring
export { wireRuntimeEvents, createWiredAgentRuntime, EVENT_MAP } from './a2a/event-wiring.js';

// A2A Demo Scenarios
export { runDemoScenario, DEMO_SCENARIOS } from './a2a/demo-scenarios.js';
export {
  runSimulationScenario,
  SIMULATION_SCENARIOS,
  captureSimulationSnapshot,
  withSimulatedClock,
} from './a2a/simulator.js';

// A2A Settlement (On-Chain)
export { createSettlementService } from './a2a/settlement.js';

// A2A Circuit Breakers
export { createCircuitBreaker } from './a2a/circuit-breaker.js';

// ============================================================================
// Competitive Moat Features
// ============================================================================

// Verifiable Commerce Proofs
export { createProofGenerator } from './sync/proof-generator.js';

// Express Checkout & Payment Links
export { createExpressCheckout } from './checkout/express.js';

// Compliance & Regulatory Exports
export { createComplianceService } from './compliance/exports.js';

// Machine-Readable Agent Catalog
export { createAgentCatalog } from './catalog/agent-catalog.js';

/**
 * StateSet CLI - Main Entry Point
 *
 * Exports all public modules for programmatic use.
 */

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

// MCP Server
export { createStatesetMcpServer, TOOL_NAMES } from './mcp-server.js';

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
export { createScaffoldMcpServer, SCAFFOLD_TOOL_NAMES } from './scaffold-server.js';

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

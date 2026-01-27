/**
 * StateSet CLI - Main Entry Point
 *
 * Exports all public modules for programmatic use.
 */

// Core harness and agent loop
export {
  runAgentLoop,
  runAgentStream,
  createAgentSession,
  routeToAgent,
  routeToAgentWithConfidence,
  listAgents,
  AGENTS
} from './claude-harness.js';

// MCP Server
export {
  createStatesetMcpServer,
  TOOL_NAMES
} from './mcp-server.js';

// Telemetry & Observability
export {
  AgentTelemetry,
  noOpTelemetry,
  createTelemetry,
  Span
} from './telemetry.js';

// Permissions & Guardrails
export {
  PermissionGate,
  createPermissionGate,
  PERMISSION_LEVELS,
  TOOL_PERMISSIONS,
  DEFAULT_GUARDRAILS,
  getLevelFromFlags
} from './permissions.js';

// Output Formatting
export {
  RichOutput,
  createOutput,
  ICONS
} from './output.js';

// Scaffolding Server (Storefront Creation)
export {
  createScaffoldMcpServer,
  SCAFFOLD_TOOL_NAMES
} from './scaffold-server.js';

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
  EventCapture
} from './sync/index.js';

// ============================================================================
// New Modules (v0.1.8)
// ============================================================================

// Structured Logging
export {
  Logger,
  createLogger,
  LOG_LEVELS,
  ToolCallLogger,
  createRequestLogger
} from './logger.js';

// Interactive Prompts
export {
  prompt,
  confirm,
  select,
  promptSchema,
  InteractivePrompts,
  isInteractive,
  interactiveOr
} from './prompts.js';

// Plugin System
export {
  PluginLoader,
  createPluginLoader,
  scaffoldPlugin,
  PLUGIN_TEMPLATE
} from './plugins/loader.js';

// Offline Support
export {
  checkApiAvailability,
  OfflineManager,
  createOfflineManager,
  showOfflineWarning,
  OfflineCache
} from './offline.js';

// Dry Run Mode
export {
  DryRunManager,
  createDryRunManager,
  formatDryRunResult,
  parseDryRunFlag,
  PREVIEWABLE_OPERATIONS
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
  getCompletions
} from './commands/index.js';

// Modular Tools
export {
  ToolRegistry,
  createToolRegistry,
  getToolsForAgent,
  AGENT_TOOL_CATEGORIES
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
  AGENT_MODELS
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
  EXIT_CODES
} from './errors.js';

// Smart Suggestions
export {
  SuggestionEngine,
  createSuggestionEngine,
  formatSuggestion,
  INTENT_PATTERNS,
  COMMAND_ALIASES
} from './suggestions.js';

// Session Persistence
export {
  SessionManager,
  createSessionManager,
  CommandHistory,
  createCommandHistory,
  DEFAULT_SESSION_DIR
} from './session.js';

// Database Management
export {
  DatabaseManager,
  createDatabaseManager,
  getGlobalManager,
  getCommerce
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
  verifyPaymentHeader
} from './x402/index.js';

// Tutorial System
export {
  TUTORIALS,
  TutorialRunner,
  createTutorialRunner,
  checkFirstRun,
  showWelcome
} from './tutorial.js';

// Request Context
export {
  RequestContext,
  Span,
  runWithContext,
  getContext,
  getOrCreateContext,
  withContext,
  withChildContext,
  withSpan,
  ContextLogger,
  createContextLogger,
  createContextMiddleware
} from './context.js';

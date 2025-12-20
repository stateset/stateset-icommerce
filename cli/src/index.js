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

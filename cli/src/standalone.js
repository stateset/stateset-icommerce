/**
 * StateSet iCommerce — Standalone Export
 *
 * Exports only the commerce engine, policy engine, and adapter stack.
 * No sync/sequencer/chain modules are loaded.
 *
 * Usage:
 *   import { Commerce, PolicyEngine, getAdapter } from '@stateset/cli/standalone';
 */

// Commerce engine
export { default as Commerce } from './commerce.js';

// Policy engine
export {
  PolicyEngine,
  PolicyTemplates,
  PolicySet,
  PolicyRule,
  PolicyAction,
} from './policies/engine.js';
export { watchPolicies } from './policies/watcher.js';

// Adapters
export { listAdapters, getAdapter, registerAdapter } from './adapters/index.js';
export { BasePlatformAdapter } from './adapters/base-adapter.js';
export { IdMapStore } from './adapters/id-map-store.js';
export { DataImporter } from './adapters/base-importer.js';

// Standalone config + tiers
export {
  loadStandaloneConfig,
  saveStandaloneConfig,
  isStandaloneMode,
  DEFAULT_STANDALONE_CONFIG,
} from './config/standalone.js';
export { TIERS, detectTier, getTierCapabilities, getTierLabel, hasCapability } from './tiers.js';

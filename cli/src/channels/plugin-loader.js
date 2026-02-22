/**
 * Plugin Discovery & Loading for StateSet iCommerce
 *
 * Discovers plugins from multiple origins:
 * - Bundled: built-in plugins from src/plugins/
 * - Global: user's ~/.stateset/plugins/
 * - Workspace: workspace-specific plugins from .stateset/plugins/
 * - Config: declared in stateset.config.json or orchestrator config
 *
 * Each discovered plugin is validated via its manifest, then loaded
 * and registered through the PluginRegistry.
 */

import fs from 'fs';
import path from 'path';
import os from 'os';
import { readManifest, validateConfig, applyConfigDefaults } from './plugin-manifest.js';
import { getPluginRegistry } from './plugin-api.js';

// ============================================================================
// Constants
// ============================================================================

const PLUGIN_ORIGINS = {
  BUNDLED: 'bundled',
  GLOBAL: 'global',
  WORKSPACE: 'workspace',
  CONFIG: 'config',
};

const DEFAULT_GLOBAL_DIR = path.join(os.homedir(), '.stateset', 'plugins');
const DEFAULT_WORKSPACE_DIR = '.stateset/plugins';

// ============================================================================
// Discovery Result Types
// ============================================================================

/**
 * @typedef {Object} DiscoveredPlugin
 * @property {string} id - Plugin ID from manifest
 * @property {string} origin - One of PLUGIN_ORIGINS
 * @property {string} dirPath - Absolute path to plugin directory
 * @property {string} entryPath - Absolute path to entry module
 * @property {import('./plugin-manifest.js').PluginManifest} manifest
 * @property {string[]} warnings - Non-fatal warnings from manifest parse
 */

/**
 * @typedef {Object} PluginLoadResult
 * @property {string} id
 * @property {string} origin
 * @property {boolean} loaded
 * @property {string} [error]
 * @property {string[]} [warnings]
 */

// ============================================================================
// Discovery
// ============================================================================

/**
 * Discover plugins from all configured origins.
 *
 * @param {Object} [opts]
 * @param {string} [opts.bundledDir] - Path to bundled plugins (default: src/plugins)
 * @param {string} [opts.globalDir] - Path to global plugins (default: ~/.stateset/plugins)
 * @param {string} [opts.workspaceDir] - Path to workspace plugins (default: .stateset/plugins)
 * @param {string[]} [opts.loadPaths] - Additional directories to scan
 * @param {Object<string, { path: string }>} [opts.configEntries] - Config-declared plugins
 * @returns {DiscoveredPlugin[]}
 */
export function discoverPlugins(opts = {}) {
  const {
    bundledDir,
    globalDir = DEFAULT_GLOBAL_DIR,
    workspaceDir = DEFAULT_WORKSPACE_DIR,
    loadPaths = [],
    configEntries = {},
  } = opts;

  const discovered = [];
  const seenIds = new Set();

  // 1. Bundled plugins
  if (bundledDir) {
    const bundled = scanDirectory(bundledDir, PLUGIN_ORIGINS.BUNDLED);
    for (const plugin of bundled) {
      if (!seenIds.has(plugin.id)) {
        seenIds.add(plugin.id);
        discovered.push(plugin);
      }
    }
  }

  // 2. Global plugins
  if (globalDir) {
    const global = scanDirectory(globalDir, PLUGIN_ORIGINS.GLOBAL);
    for (const plugin of global) {
      if (!seenIds.has(plugin.id)) {
        seenIds.add(plugin.id);
        discovered.push(plugin);
      }
    }
  }

  // 3. Workspace plugins
  if (workspaceDir) {
    const workspace = scanDirectory(workspaceDir, PLUGIN_ORIGINS.WORKSPACE);
    for (const plugin of workspace) {
      if (!seenIds.has(plugin.id)) {
        seenIds.add(plugin.id);
        discovered.push(plugin);
      }
    }
  }

  // 4. Additional load paths
  for (const loadPath of loadPaths) {
    const extra = scanDirectory(loadPath, PLUGIN_ORIGINS.CONFIG);
    for (const plugin of extra) {
      if (!seenIds.has(plugin.id)) {
        seenIds.add(plugin.id);
        discovered.push(plugin);
      }
    }
  }

  // 5. Config-declared plugins (explicit paths)
  for (const [id, entry] of Object.entries(configEntries)) {
    if (seenIds.has(id)) continue;
    if (!entry || typeof entry.path !== 'string' || !entry.path.trim()) {
      console.warn(`[PluginLoader] Config plugin "${id}" has invalid path`);
      continue;
    }

    let pluginPath;
    try {
      pluginPath = path.resolve(entry.path.trim());
    } catch (err) {
      console.warn(
        `[PluginLoader] Invalid config plugin "${id}" path "${entry.path}":`,
        err.message,
      );
      continue;
    }
    if (!fs.existsSync(pluginPath)) {
      console.warn(`[PluginLoader] Config plugin "${id}" path not found: ${pluginPath}`);
      continue;
    }

    let stat;
    try {
      stat = fs.statSync(pluginPath);
    } catch (err) {
      console.warn(
        `[PluginLoader] Cannot access plugin "${id}" path "${pluginPath}":`,
        err.message,
      );
      continue;
    }

    const dirPath = stat.isDirectory() ? pluginPath : path.dirname(pluginPath);

    const result = readManifest(dirPath);
    if (result.found && result.manifest) {
      try {
        const entryPath = resolvePluginEntryPath(dirPath, result.manifest.entry);
        seenIds.add(result.manifest.id);
        discovered.push({
          id: result.manifest.id,
          origin: PLUGIN_ORIGINS.CONFIG,
          dirPath,
          entryPath,
          manifest: result.manifest,
          warnings: result.warnings || [],
        });
      } catch (error) {
        console.warn(
          `[PluginLoader] Invalid manifest entry in ${path.resolve(dirPath, result.manifest.entry)}:`,
          error.message,
        );
      }
    } else if (!result.found) {
      // Allow config entries without a manifest if the path points to a .js file
      if (stat.isFile() && path.extname(pluginPath).toLowerCase() === '.js') {
        seenIds.add(id);
        discovered.push({
          id,
          origin: PLUGIN_ORIGINS.CONFIG,
          dirPath,
          entryPath: pluginPath,
          manifest: {
            id,
            name: id,
            version: '0.0.0',
            description: '',
            author: '',
            license: '',
            entry: path.basename(pluginPath),
            kind: 'general',
            channels: [],
            provides: [],
            enabledByDefault: true,
            configSchema: null,
            configDefaults: {},
            configHints: [],
          },
          warnings: ['No manifest file found; using defaults'],
        });
      }
    } else {
      console.warn(`[PluginLoader] Invalid manifest for config plugin "${id}":`, result.errors);
    }
  }

  return discovered;
}

/**
 * Scan a directory for plugin subdirectories (each with a manifest).
 *
 * @param {string} dirPath
 * @param {string} origin
 * @returns {DiscoveredPlugin[]}
 */
function scanDirectory(dirPath, origin) {
  const plugins = [];

  if (!fs.existsSync(dirPath)) return plugins;

  let entries;
  try {
    entries = fs.readdirSync(dirPath, { withFileTypes: true });
  } catch (err) {
    console.warn(`[PluginLoader] Failed to read directory ${dirPath}: ${err.message}`);
    return plugins;
  }

  for (const entry of entries) {
    if (!entry.isDirectory()) continue;

    const pluginDir = path.join(dirPath, entry.name);
    const result = readManifest(pluginDir);

    if (result.found && result.manifest) {
      try {
        const entryPath = resolvePluginEntryPath(pluginDir, result.manifest.entry);
        plugins.push({
          id: result.manifest.id,
          origin,
          dirPath: pluginDir,
          entryPath,
          manifest: result.manifest,
          warnings: result.warnings || [],
        });
      } catch (error) {
        console.warn(`[PluginLoader] Invalid manifest entry in ${pluginDir}:`, error.message);
      }
    } else if (result.found && result.errors) {
      console.warn(`[PluginLoader] Invalid manifest in ${pluginDir}:`, result.errors);
    }
    // Silently skip directories without manifests
  }

  return plugins;
}

/**
 * Resolve and validate a plugin entry path.
 * Prevents manifest path traversal so entries cannot escape plugin directory.
 *
 * @param {string} pluginDir
 * @param {string} entry
 * @returns {string}
 */
function resolvePluginEntryPath(pluginDir, entry) {
  if (typeof entry !== 'string' || !entry.trim()) {
    throw new Error('Manifest "entry" must be a non-empty string');
  }

  const rootDir = path.resolve(pluginDir);
  const resolvedEntry = path.resolve(rootDir, entry);
  const relative = path.relative(rootDir, resolvedEntry);

  if (relative.startsWith('..') || path.isAbsolute(relative)) {
    throw new Error(`Manifest entry escapes plugin directory: ${entry}`);
  }

  if (!fs.existsSync(resolvedEntry)) {
    throw new Error(`Manifest entry not found: ${entry}`);
  }

  const stat = fs.statSync(resolvedEntry);
  if (!stat.isFile()) {
    throw new Error(`Manifest entry must be a file: ${entry}`);
  }

  return resolvedEntry;
}

// ============================================================================
// Loading
// ============================================================================

/**
 * Load and register discovered plugins.
 *
 * @param {DiscoveredPlugin[]} plugins - Discovered plugins
 * @param {Object} [opts]
 * @param {Object<string, Object>} [opts.pluginConfigs] - Per-plugin config overrides
 * @param {import('./plugin-config.js').PluginConfigState} [opts.configState] - Enable/disable state
 * @param {import('./plugin-runtime.js').PluginRuntimeContext} [opts.runtime] - Runtime utilities
 * @param {boolean} [opts.verbose=false]
 * @returns {Promise<PluginLoadResult[]>}
 */
export async function loadPlugins(plugins, opts = {}) {
  const { pluginConfigs = {}, configState, runtime, verbose = false } = opts;

  const results = [];
  const registry = getPluginRegistry();

  for (const discovered of plugins) {
    const { id, origin, entryPath, manifest } = discovered;

    // Check if enabled
    if (configState && !configState.isEnabled(id)) {
      if (verbose) {
        console.debug(
          `[PluginLoader] Skipping disabled plugin: ${id} (${configState.getDisableReason(id)})`,
        );
      }
      results.push({
        id,
        origin,
        loaded: false,
        error: `disabled: ${configState.getDisableReason(id)}`,
      });
      continue;
    }

    if (!fs.existsSync(entryPath)) {
      results.push({
        id,
        origin,
        loaded: false,
        error: `Plugin entry not found: ${entryPath}`,
      });
      continue;
    }

    let entryStat;
    try {
      entryStat = fs.statSync(entryPath);
    } catch (err) {
      results.push({
        id,
        origin,
        loaded: false,
        error: `Failed to access plugin entry: ${err.message}`,
      });
      continue;
    }

    if (!entryStat.isFile()) {
      results.push({
        id,
        origin,
        loaded: false,
        error: `Plugin entry must be a file: ${entryPath}`,
      });
      continue;
    }

    // Validate and apply config
    let pluginConfig = pluginConfigs[id] || {};
    if (manifest.configDefaults) {
      pluginConfig = applyConfigDefaults(pluginConfig, manifest.configDefaults);
    }

    if (manifest.configSchema) {
      const validation = validateConfig(pluginConfig, manifest.configSchema);
      if (!validation.valid) {
        console.error(`[PluginLoader] Config validation failed for "${id}":`, validation.errors);
        results.push({
          id,
          origin,
          loaded: false,
          error: `config validation: ${validation.errors.join(', ')}`,
        });
        continue;
      }
    }

    // Load module
    try {
      if (verbose) {
        console.debug(`[PluginLoader] Loading ${id} from ${entryPath} (${origin})`);
      }

      const mod = await import(entryPath);
      const initFn = mod.default || mod.init || mod.activate;

      if (typeof initFn !== 'function') {
        throw new Error('Plugin must export a default function, init(), or activate()');
      }

      // Register through PluginRegistry, passing config and runtime context
      await registry.register(id, (api) => {
        return initFn(api, {
          config: pluginConfig,
          manifest,
          runtime: runtime || null,
          origin,
        });
      });

      if (discovered.warnings.length > 0 && verbose) {
        console.warn(`[PluginLoader] Warnings for "${id}":`, discovered.warnings);
      }

      results.push({ id, origin, loaded: true, warnings: discovered.warnings });
    } catch (err) {
      console.error(`[PluginLoader] Failed to load plugin "${id}":`, err.message);
      results.push({ id, origin, loaded: false, error: err.message });
    }
  }

  return results;
}

// ============================================================================
// Convenience: Discover + Load
// ============================================================================

/**
 * Discover and load all plugins from all origins.
 *
 * @param {Object} [opts] - Combined discover + load options
 * @returns {Promise<{ discovered: DiscoveredPlugin[], results: PluginLoadResult[] }>}
 */
export async function discoverAndLoadPlugins(opts = {}) {
  const discovered = discoverPlugins(opts);

  if (opts.verbose) {
    console.debug(
      `[PluginLoader] Discovered ${discovered.length} plugins from ${new Set(discovered.map((p) => p.origin)).size} origin(s)`,
    );
    for (const p of discovered) {
      console.debug(`  - ${p.id} (${p.origin}) v${p.manifest.version}`);
    }
  }

  const results = await loadPlugins(discovered, opts);

  const loaded = results.filter((r) => r.loaded).length;
  const failed = results.filter((r) => !r.loaded).length;

  console.debug(
    `[PluginLoader] Loaded ${loaded} plugin(s)${failed > 0 ? `, ${failed} failed` : ''}`,
  );

  return { discovered, results };
}

export { PLUGIN_ORIGINS };

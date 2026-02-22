/**
 * Plugin System for StateSet Channel Gateways
 *
 * Provides a structured way for extensions to:
 * - Register custom commands
 * - Hook into the message lifecycle
 * - Register background services
 * - Add HTTP routes (for webhooks, etc.)
 *
 * Inspired by moltbot's plugin architecture.
 *
 * Usage:
 *   getPluginRegistry().register('my-plugin', (api) => {
 *     api.registerCommand({ name: 'hello', ... });
 *     api.on('message_received', async (data) => { ... });
 *   });
 */

import { getCommandRegistry } from './command-registry.js';

// ============================================================================
// HookRunner
// ============================================================================

/**
 * Hooks that run in parallel (fire-and-forget, errors logged but don't block).
 */
const PARALLEL_HOOKS = new Set([
  'message_received', // Inbound message processing
  'message_sent', // Post-delivery hooks
  'agent_end', // After agent completes
  'after_tool_call', // Post-execution analysis
  'after_command', // After command executed
  'session_start', // Session initialization
  'session_end', // Session cleanup
  'gateway_start', // Gateway startup
  'gateway_stop', // Gateway shutdown
  'after_compaction', // Post-compression analysis
  'plugin_loaded', // After plugin is loaded
  'plugin_unloaded', // After plugin is unloaded
]);

/**
 * Hooks that run sequentially (can modify data, priority-ordered).
 */
const SEQUENTIAL_HOOKS = new Set([
  'message_sending', // Outbound message filtering/modification
  'before_agent_start', // Inject system prompt context
  'before_tool_call', // Intercept/block tool calls
  'before_command', // Before command executed (can block)
  'before_compaction', // Pre-compression cleanup
  'tool_result_persist', // Sync hook for result message modification
  'before_send', // Final modification before send
]);

/**
 * @typedef {Object} HookHandler
 * @property {string} hookName
 * @property {Function} handler
 * @property {number} priority - Lower = runs first (default 100)
 * @property {string} pluginId
 */

export class HookRunner {
  static PARALLEL_HOOKS = PARALLEL_HOOKS;
  static SEQUENTIAL_HOOKS = SEQUENTIAL_HOOKS;

  constructor() {
    /** @type {Map<string, HookHandler[]>} */
    this._hooks = new Map();
  }

  /**
   * Add a hook handler.
   *
   * @param {string} hookName
   * @param {Function} handler
   * @param {Object} opts
   * @param {number} [opts.priority=100]
   * @param {string} opts.pluginId
   */
  add(hookName, handler, { priority = 100, pluginId }) {
    if (!this._hooks.has(hookName)) {
      this._hooks.set(hookName, []);
    }

    this._hooks.get(hookName).push({
      hookName,
      handler,
      priority,
      pluginId,
    });

    // Keep sorted by priority (lower first)
    this._hooks.get(hookName).sort((a, b) => a.priority - b.priority);
  }

  /**
   * Remove all hooks for a plugin.
   *
   * @param {string} pluginId
   */
  remove(pluginId) {
    for (const [hookName, handlers] of this._hooks) {
      this._hooks.set(
        hookName,
        handlers.filter((h) => h.pluginId !== pluginId),
      );
    }
  }

  /**
   * Run hooks for an event.
   * Auto-selects parallel vs sequential based on hook type.
   *
   * @param {string} hookName
   * @param {Object} data
   * @returns {Promise<Object>} - Modified data (for sequential) or original data (for parallel)
   */
  async run(hookName, data) {
    const handlers = this._hooks.get(hookName) || [];
    if (handlers.length === 0) return data;

    if (SEQUENTIAL_HOOKS.has(hookName)) {
      return this._runSequential(hookName, handlers, data);
    }

    // Default to parallel
    await this._runParallel(hookName, handlers, data);
    return data;
  }

  /**
   * Run handlers in parallel (fire-and-forget).
   * @private
   */
  async _runParallel(hookName, handlers, data) {
    const promises = handlers.map(async ({ handler, pluginId }) => {
      try {
        await handler(data);
      } catch (err) {
        console.error(
          `[HookRunner] Error in parallel hook ${hookName} (plugin: ${pluginId}):`,
          err.message,
        );
      }
    });

    await Promise.allSettled(promises);
  }

  /**
   * Run handlers sequentially, allowing data modification.
   * @private
   */
  async _runSequential(hookName, handlers, data) {
    let result = { ...data };

    for (const { handler, pluginId } of handlers) {
      try {
        const modification = await handler(result);
        if (modification && typeof modification === 'object') {
          result = { ...result, ...modification };
        }
      } catch (err) {
        console.error(
          `[HookRunner] Error in sequential hook ${hookName} (plugin: ${pluginId}):`,
          err.message,
        );
      }
    }

    return result;
  }

  /**
   * Check if any hooks are registered for an event.
   *
   * @param {string} hookName
   * @returns {boolean}
   */
  hasHooks(hookName) {
    const handlers = this._hooks.get(hookName);
    return handlers && handlers.length > 0;
  }

  /**
   * Get count of registered hooks per event.
   *
   * @returns {Object<string, number>}
   */
  getHookCounts() {
    const counts = {};
    for (const [hookName, handlers] of this._hooks) {
      counts[hookName] = handlers.length;
    }
    return counts;
  }

  /**
   * Clear all hooks.
   */
  clear() {
    this._hooks.clear();
  }
}

// ============================================================================
// PluginAPI
// ============================================================================

/**
 * @typedef {Object} ServiceDefinition
 * @property {string} name
 * @property {Function} start - () => Promise<void>
 * @property {Function} stop - () => Promise<void>
 */

/**
 * @typedef {Object} HttpRouteDefinition
 * @property {string} method - HTTP method (GET, POST, etc.)
 * @property {string} path - Route path
 * @property {string} [level] - Permission level required to access the route (none, read, preview, write, delete, admin)
 * @property {Function} handler - ({ method, pathname, params, body, query, headers, identity }) => Promise<any>
 */

/**
 * API object provided to plugin init functions.
 */
export class PluginAPI {
  /**
   * @param {string} pluginId
   * @param {PluginRegistry} registry
   */
  constructor(pluginId, registry) {
    this._pluginId = pluginId;
    this._registry = registry;
    this._commands = [];
    this._services = [];
    this._routes = [];
  }

  /**
   * Register a command.
   *
   * @param {Object} definition
   * @param {string} definition.name
   * @param {string} definition.description
   * @param {Function} definition.handler
   * @param {boolean} [definition.acceptsArgs=true]
   */
  registerCommand(definition) {
    const source = definition.source || `plugin:${this._pluginId}`;
    const fullDef = { ...definition, source };

    getCommandRegistry().register(fullDef);
    this._commands.push(fullDef.name);
  }

  /**
   * Register a hook handler.
   *
   * @param {string} hookName
   * @param {Function} handler
   * @param {Object} [opts]
   * @param {number} [opts.priority=100]
   */
  on(hookName, handler, opts = {}) {
    this._registry.getHookRunner().add(hookName, handler, {
      priority: opts.priority ?? 100,
      pluginId: this._pluginId,
    });
  }

  /**
   * Register a background service.
   *
   * @param {ServiceDefinition} service
   */
  registerService(service) {
    if (
      !service.name ||
      typeof service.start !== 'function' ||
      typeof service.stop !== 'function'
    ) {
      throw new Error('Service must have name, start, and stop functions');
    }

    this._services.push({
      ...service,
      pluginId: this._pluginId,
    });
    this._registry._services.push(this._services[this._services.length - 1]);
  }

  /**
   * Register an HTTP route.
   *
   * @param {HttpRouteDefinition} route
   */
  registerHttpRoute(route) {
    if (!route.method || !route.path || typeof route.handler !== 'function') {
      throw new Error('Route must have method, path, and handler');
    }

    const VALID_LEVELS = new Set(['none', 'read', 'preview', 'write', 'delete', 'admin']);
    const level = route.level ? String(route.level).toLowerCase() : null;
    if (level && !VALID_LEVELS.has(level)) {
      throw new Error(
        `Invalid route level "${route.level}" (expected one of: ${[...VALID_LEVELS].join(', ')})`,
      );
    }

    this._routes.push({
      ...route,
      ...(level ? { level } : {}),
      pluginId: this._pluginId,
    });
    this._registry._routes.push(this._routes[this._routes.length - 1]);
  }

  /**
   * Get the plugin ID.
   *
   * @returns {string}
   */
  getPluginId() {
    return this._pluginId;
  }

  /**
   * Get list of registered commands.
   *
   * @returns {string[]}
   */
  getRegisteredCommands() {
    return [...this._commands];
  }

  /**
   * Get list of registered services.
   *
   * @returns {string[]}
   */
  getRegisteredServices() {
    return this._services.map((s) => s.name);
  }
}

// ============================================================================
// PluginRegistry
// ============================================================================

/**
 * @typedef {Object} PluginEntry
 * @property {string} id
 * @property {PluginAPI} api
 * @property {string[]} commands
 * @property {string[]} services
 */

export class PluginRegistry {
  constructor() {
    /** @type {Map<string, PluginEntry>} */
    this._plugins = new Map();

    /** @type {HookRunner} */
    this._hookRunner = new HookRunner();

    /** @type {ServiceDefinition[]} */
    this._services = [];

    /** @type {HttpRouteDefinition[]} */
    this._routes = [];
  }

  /**
   * Register a plugin.
   *
   * @param {string} pluginId
   * @param {Function} initFn - (api: PluginAPI) => void | Promise<void>
   * @returns {Promise<PluginEntry>}
   */
  async register(pluginId, initFn) {
    if (this._plugins.has(pluginId)) {
      throw new Error(`Plugin "${pluginId}" is already registered`);
    }

    const api = new PluginAPI(pluginId, this);

    try {
      await initFn(api);
    } catch (err) {
      throw new Error(`Failed to initialize plugin "${pluginId}": ${err.message}`);
    }

    const entry = {
      id: pluginId,
      api,
      commands: api.getRegisteredCommands(),
      services: api.getRegisteredServices(),
    };

    this._plugins.set(pluginId, entry);
    console.debug(`[PluginRegistry] Registered plugin: ${pluginId}`);

    return entry;
  }

  /**
   * Unregister a plugin, tearing down its commands, hooks, and services.
   *
   * @param {string} pluginId
   * @returns {boolean}
   */
  async unregister(pluginId) {
    const entry = this._plugins.get(pluginId);
    if (!entry) return false;

    // Remove commands
    const registry = getCommandRegistry();
    for (const cmdName of entry.commands) {
      registry.unregister(cmdName);
    }

    // Remove hooks
    this._hookRunner.remove(pluginId);

    // Stop and remove services
    const servicesToRemove = this._services.filter((s) => s.pluginId === pluginId);
    for (const service of servicesToRemove) {
      try {
        await service.stop();
      } catch (err) {
        console.error(`[PluginRegistry] Error stopping service "${service.name}":`, err.message);
      }
    }
    this._services = this._services.filter((s) => s.pluginId !== pluginId);

    // Remove routes
    this._routes = this._routes.filter((r) => r.pluginId !== pluginId);

    this._plugins.delete(pluginId);
    console.debug(`[PluginRegistry] Unregistered plugin: ${pluginId}`);

    return true;
  }

  /**
   * List all registered plugins.
   *
   * @returns {Array<{ id: string, commands: string[], hooks: number, services: string[] }>}
   */
  listPlugins() {
    const result = [];
    for (const [id, entry] of this._plugins) {
      result.push({
        id,
        commands: entry.commands,
        hooks: this._getHookCountForPlugin(id),
        services: entry.services,
      });
    }
    return result;
  }

  /**
   * Get the shared HookRunner.
   *
   * @returns {HookRunner}
   */
  getHookRunner() {
    return this._hookRunner;
  }

  /**
   * Get all registered services.
   *
   * @returns {ServiceDefinition[]}
   */
  getServices() {
    return [...this._services];
  }

  /**
   * Get all registered HTTP routes.
   *
   * @returns {HttpRouteDefinition[]}
   */
  getRoutes() {
    return [...this._routes];
  }

  /**
   * Check if a plugin is registered.
   *
   * @param {string} pluginId
   * @returns {boolean}
   */
  has(pluginId) {
    return this._plugins.has(pluginId);
  }

  /**
   * Get hook count for a plugin (for display).
   * @private
   */
  _getHookCountForPlugin(pluginId) {
    let count = 0;
    for (const handlers of this._hookRunner._hooks.values()) {
      count += handlers.filter((h) => h.pluginId === pluginId).length;
    }
    return count;
  }

  /**
   * Clear all plugins and reset state.
   */
  async clear() {
    const pluginIds = [...this._plugins.keys()];
    for (const id of pluginIds) {
      await this.unregister(id);
    }
    this._hookRunner.clear();
    this._services = [];
    this._routes = [];
  }
}

// ============================================================================
// Singleton
// ============================================================================

let _instance = null;

/**
 * Get the global PluginRegistry instance.
 *
 * @returns {PluginRegistry}
 */
export function getPluginRegistry() {
  if (!_instance) {
    _instance = new PluginRegistry();
  }
  return _instance;
}

/**
 * Reset the singleton instance (for testing).
 */
export async function resetPluginRegistry() {
  if (_instance) {
    await _instance.clear();
  }
  _instance = null;
}

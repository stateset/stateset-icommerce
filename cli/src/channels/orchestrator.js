/**
 * Multi-Gateway Orchestrator for StateSet iCommerce
 *
 * Launches multiple channel gateways in a single process with shared
 * session store, middleware stack, and notifier configuration.
 *
 * Configured via a YAML or JSON config file, or programmatically.
 */

import { ChannelSessionStore } from './session-store.js';
import { CustomerIdentityStore } from './identity.js';
import { getNotifier } from './notifier.js';
import { rateLimiter, messageLogger, contentFilter, autoLanguageDetect } from './middleware.js';
import { getMetrics, metricsCollector } from './metrics.js';
import { getPluginRegistry } from './plugin-api.js';
import { registerAutonomousCommands, unregisterAutonomousCommands } from './autonomous-commands.js';
import { EventBridge } from './event-bridge.js';
import { discoverAndLoadPlugins } from './plugin-loader.js';
import { getPluginConfigState } from './plugin-config.js';
import { getPluginSlots } from './plugin-slots.js';
import { initializeSharedRuntime, getSharedRuntime } from './plugin-runtime.js';
import { getGatewayMethods } from './gateway-methods.js';
import { getCliExtensions } from './cli-extensions.js';
import { getCommandRegistry } from './command-registry.js';
import { discoverSkills } from '../skills/loader.js';
import { getSkillRegistry } from '../skills/registry.js';
import { registerSkillHooks } from '../skills/injector.js';
import { createHttpGateway } from './http-gateway.js';

// ============================================================================
// Channel launchers — lazy-loaded to avoid pulling in SDKs until needed
// ============================================================================

const CHANNEL_LAUNCHERS = {
  async telegram(config, shared) {
    const { startTelegramGateway } = await import('../telegram/gateway.js');
    return startTelegramGateway({ ...config, ...shared });
  },
  async discord(config, shared) {
    const { startDiscordGateway } = await import('../discord/gateway.js');
    return startDiscordGateway({ ...config, ...shared });
  },
  async slack(config, shared) {
    const { startSlackGateway } = await import('../slack/gateway.js');
    return startSlackGateway({ ...config, ...shared });
  },
  async whatsapp(config, shared) {
    const { startWhatsAppGateway } = await import('../whatsapp/gateway.js');
    return startWhatsAppGateway({ ...config, ...shared });
  },
  async signal(config, shared) {
    const { startSignalGateway } = await import('../signal/gateway.js');
    return startSignalGateway({ ...config, ...shared });
  },
  async 'google-chat'(config, shared) {
    const { startGoogleChatGateway } = await import('../google-chat/gateway.js');
    return startGoogleChatGateway({ ...config, ...shared });
  },
  async imessage(config, shared) {
    const { startIMessageGateway } = await import('../imessage/gateway.js');
    return startIMessageGateway({ ...config, ...shared });
  },
  async teams(config, shared) {
    const { startTeamsGateway } = await import('../teams/gateway.js');
    return startTeamsGateway({ ...config, ...shared });
  },
  async matrix(config, shared) {
    const { startMatrixGateway } = await import('../matrix/gateway.js');
    return startMatrixGateway({ ...config, ...shared });
  },
  async webchat(config, shared) {
    const { startWebChatChannel } = await import('./webchat.js');
    return startWebChatChannel({ ...config, ...shared });
  },
};

// ============================================================================
// Middleware builder
// ============================================================================

/**
 * Build a middleware stack from config.
 *
 * @param {Object} [middlewareConfig]
 * @param {Object} [middlewareConfig.rateLimiter]
 * @param {Object} [middlewareConfig.contentFilter]
 * @param {boolean} [middlewareConfig.languageDetect]
 * @param {boolean} [middlewareConfig.logger]
 * @returns {Function[]}
 */
function buildMiddleware(middlewareConfig = {}) {
  const stack = [];

  // Metrics collector always first
  stack.push(metricsCollector());

  if (middlewareConfig.logger !== false) {
    stack.push(messageLogger());
  }

  if (middlewareConfig.rateLimiter) {
    stack.push(rateLimiter(middlewareConfig.rateLimiter));
  }

  if (middlewareConfig.contentFilter) {
    stack.push(contentFilter(middlewareConfig.contentFilter));
  }

  if (middlewareConfig.languageDetect) {
    stack.push(autoLanguageDetect());
  }

  return stack;
}

// ============================================================================
// ChannelOrchestrator
// ============================================================================

export class ChannelOrchestrator {
  /**
   * @param {Object} config
   * @param {Object}  config.channels      - Per-channel config keyed by name
   * @param {Object}  [config.shared]      - Shared options (dbPath, allowApply, model, etc.)
   * @param {Object}  [config.middleware]   - Middleware config
   * @param {Object}  [config.notifications] - Notification route config
   * @param {boolean} [config.persistSessions=true] - Enable persistent sessions
   * @param {string}  [config.sessionDbPath] - Custom session DB path
   * @param {Object}  [config.autonomousEngine] - Autonomous engine for workflows/jobs/approvals
   * @param {Object}  [config.plugins] - Plugin system configuration
   * @param {string}  [config.plugins.bundledDir] - Bundled plugins directory
   * @param {string}  [config.plugins.globalDir] - Global plugins directory
   * @param {string}  [config.plugins.workspaceDir] - Workspace plugins directory
   * @param {string[]} [config.plugins.loadPaths] - Additional load paths
   * @param {Object}  [config.plugins.entries] - Config-declared plugins
   * @param {string[]} [config.plugins.allow] - Plugin allow list
   * @param {string[]} [config.plugins.deny] - Plugin deny list
   * @param {Object}  [config.plugins.configs] - Per-plugin configs
   * @param {string}  [config.stateDir] - State directory for plugin storage
   */
  constructor(config) {
    this.config = config;
    this.gateways = new Map();
    this.sessionStore = null;
    this.identityStore = null;
    this.middleware = [];
    this._running = false;
    this._eventBridge = null;
    this._pluginLoadResults = null;
    this._httpGateway = null;
    this._voice = null;
    this._browser = null;
    this._memory = null;
    this._vectorAutoIndex = null;
  }

  /**
   * Start all configured channels.
   *
   * @returns {Promise<{ started: string[], failed: { channel: string, error: string }[] }>}
   */
  async start() {
    if (this._running) throw new Error('Orchestrator is already running');

    const {
      channels,
      shared = {},
      middleware: mwConfig,
      notifications,
      persistSessions = true,
      sessionDbPath,
      identityDbPath,
    } = this.config;

    // 1. Session store
    if (persistSessions) {
      this.sessionStore = new ChannelSessionStore(
        sessionDbPath ? { dbPath: sessionDbPath } : undefined,
      );
    }

    // 2. Identity store
    this.identityStore = new CustomerIdentityStore(
      identityDbPath ? { dbPath: identityDbPath } : undefined,
    );

    // 3. Middleware
    this.middleware = buildMiddleware(mwConfig);

    // 4. Notification routes
    if (notifications?.routes) {
      getNotifier().loadRoutes(notifications.routes);
    }

    // 5. Autonomous engine integration
    const autonomousEngine = this.config.autonomousEngine || null;
    if (autonomousEngine) {
      // Connect notifier to engine for outbound notifications
      autonomousEngine.setNotifier(getNotifier());

      // Start EventBridge to route engine events to channels
      this._eventBridge = new EventBridge({
        engine: autonomousEngine,
        notifier: getNotifier(),
        verbose: shared.verbose ?? false,
      });
      this._eventBridge.start();

      // Register autonomous commands (/workflow, /job, /approve, etc.)
      registerAutonomousCommands(autonomousEngine);
    }

    // 6. Initialize plugin system
    const pluginsConfig = this.config.plugins || {};
    const stateDir = this.config.stateDir || null;

    // Initialize shared runtime for plugins
    initializeSharedRuntime({
      commerce: null, // Lazy-loaded per request
      autonomousEngine,
      stateDir,
      verbose: shared.verbose ?? false,
      dbPath: shared.dbPath || './store.db',
    });

    // Initialize plugin config state (allow/deny lists)
    const configState = getPluginConfigState({
      allow: pluginsConfig.allow,
      deny: pluginsConfig.deny,
      entries: pluginsConfig.configs
        ? Object.fromEntries(
            Object.entries(pluginsConfig.configs).map(([id, config]) => [id, { config }]),
          )
        : {},
      statePath: stateDir ? `${stateDir}/plugin-state.json` : null,
    });

    // Define default plugin slots
    const slots = getPluginSlots();
    if (!slots.hasSlot('memory')) {
      slots.defineSlot('memory', { description: 'Memory/persistence provider', required: false });
    }
    if (!slots.hasSlot('search')) {
      slots.defineSlot('search', { description: 'Search provider', required: false });
    }

    // Discover and load plugins
    if (
      pluginsConfig.bundledDir ||
      pluginsConfig.globalDir ||
      pluginsConfig.workspaceDir ||
      pluginsConfig.loadPaths?.length
    ) {
      try {
        this._pluginLoadResults = await discoverAndLoadPlugins({
          bundledDir: pluginsConfig.bundledDir,
          globalDir: pluginsConfig.globalDir,
          workspaceDir: pluginsConfig.workspaceDir,
          loadPaths: pluginsConfig.loadPaths || [],
          configEntries: pluginsConfig.entries || {},
          pluginConfigs: pluginsConfig.configs || {},
          configState,
          verbose: shared.verbose ?? false,
        });
      } catch (err) {
        console.error('[Orchestrator] Plugin loading failed:', err.message);
      }
    }

    // Auto-assign plugin slots after all plugins have registered
    slots.autoAssign();

    // 6c. Initialize skill system
    const skillRegistry = getSkillRegistry();
    try {
      const skillsDiscovered = discoverSkills({ verbose: shared.verbose ?? false });
      skillRegistry.loadFromDiscovered(skillsDiscovered);
      registerSkillHooks(skillRegistry);
      console.log(
        `[Orchestrator] Loaded ${skillRegistry.count()} skills across ${skillRegistry.getCategories().length} categories`,
      );
    } catch (err) {
      console.error('[Orchestrator] Skill loading failed:', err.message);
    }

    // 7. Build shared options
    const sharedOpts = {
      sessionStore: this.sessionStore,
      identityStore: this.identityStore,
      middleware: this.middleware,
      dbPath: shared.dbPath || './store.db',
      allowApply: shared.allowApply ?? false,
      model: shared.model,
      maxTurns: shared.maxTurns || 10,
      agent: shared.agent,
      verbose: shared.verbose ?? false,
      autonomousEngine,
    };

    // 8. Start HTTP gateway
    const httpConfig = this.config.httpGateway || {};
    if (httpConfig.enabled !== false) {
      try {
        this._httpGateway = createHttpGateway({
          port: httpConfig.port || 0,
          host: httpConfig.host || '127.0.0.1',
          verbose: shared.verbose ?? false,
          configState,
          apiKeys: httpConfig.apiKeys || [],
          allowAnonymous: httpConfig.allowAnonymous === true,
          anonymousIdentity: httpConfig.anonymousIdentity || null,
          allowQueryParamAuth: httpConfig.allowQueryParamAuth === true,
          corsOrigins: httpConfig.corsOrigins,
          allowRemoteAdminEndpoints: httpConfig.allowRemoteAdminEndpoints === true,
          sandbox: httpConfig.sandbox || null,
        });
        const addr = await this._httpGateway.start();
        console.log(`[Orchestrator] HTTP gateway listening on ${addr.host}:${addr.port}`);
      } catch (err) {
        console.error(`[Orchestrator] HTTP gateway failed to start: ${err.message}`);
        this._httpGateway = null;
      }
    }

    // 9. Init subsystems (voice, browser, memory)
    if (this.config.voice?.enabled) {
      try {
        const { getVoiceModeController } = await import('../voice/voice-mode.js');
        this._voice = getVoiceModeController(this.config.voice);
        console.log('[Orchestrator] Voice subsystem initialized.');
      } catch (err) {
        console.error(`[Orchestrator] Voice subsystem failed: ${err.message}`);
      }
    }

    if (this.config.browser?.enabled) {
      try {
        const { getBrowserTools } = await import('../browser/browser-tools.js');
        this._browser = getBrowserTools(this.config.browser);
        console.log('[Orchestrator] Browser subsystem initialized.');
      } catch (err) {
        console.error(`[Orchestrator] Browser subsystem failed: ${err.message}`);
      }
    }

    if (this.config.memory?.enabled) {
      try {
        const { getVectorMemoryStore } = await import('../memory/vector-store.js');
        this._memory = getVectorMemoryStore(this.config.memory);
        console.log('[Orchestrator] Memory subsystem initialized.');
      } catch (err) {
        console.error(`[Orchestrator] Memory subsystem failed: ${err.message}`);
      }
    }

    // Init heartbeat monitor (via autonomous engine)
    if (autonomousEngine && this.config.heartbeat?.enabled) {
      try {
        await autonomousEngine.initHeartbeat(this.config.heartbeat, shared.commerce || null);
        this._heartbeat = autonomousEngine.heartbeat;
        console.log('[Orchestrator] Heartbeat monitor initialized.');
      } catch (err) {
        console.error(`[Orchestrator] Heartbeat init failed: ${err.message}`);
      }
    }

    // Init vector auto-index (auto-embed new entities)
    if (shared.vectorAutoIndex && process.env.OPENAI_API_KEY) {
      try {
        const dbPath = shared.dbPath || './store.db';
        const { Commerce } = await import('@stateset/embedded');
        const autoIndexCommerce = new Commerce(dbPath);
        this._vectorAutoIndex = autoIndexCommerce.vector(process.env.OPENAI_API_KEY);
        // Make auto-indexer available to MCP tool handlers via shared runtime
        const runtime = getSharedRuntime();
        if (runtime) {
          runtime.vectorAutoIndex = this._vectorAutoIndex;
        }
        console.log(
          '[Orchestrator] Vector auto-index enabled — new entities will be embedded automatically.',
        );
      } catch (err) {
        console.error(`[Orchestrator] Vector auto-index init failed: ${err.message}`);
        this._vectorAutoIndex = null;
      }
    }

    // Wire subsystems into HTTP gateway
    if (this._httpGateway) {
      this._httpGateway.setSubsystems({
        voice: this._voice,
        browser: this._browser,
        memory: this._memory,
        heartbeat: this._heartbeat || autonomousEngine?.heartbeat || null,
        vectorAutoIndex: this._vectorAutoIndex || null,
      });

      this._httpGateway.setOrchestratorStatus(() => {
        const channels = {};
        for (const [name] of this.gateways) {
          channels[name] = 'running';
        }
        return channels;
      });
    }

    // 10. Launch channels
    const started = [];
    const failed = [];

    const channelEntries = Object.entries(channels || {});
    if (channelEntries.length === 0) {
      console.warn('[Orchestrator] No channels configured.');
      return { started, failed };
    }

    for (const [name, channelConfig] of channelEntries) {
      if (channelConfig.enabled === false) {
        console.log(`[Orchestrator] Skipping disabled channel: ${name}`);
        continue;
      }

      const launcher = CHANNEL_LAUNCHERS[name];
      if (!launcher) {
        failed.push({ channel: name, error: `Unknown channel type: ${name}` });
        console.error(`[Orchestrator] Unknown channel: ${name}`);
        continue;
      }

      try {
        console.log(`[Orchestrator] Starting ${name}...`);
        // Merge shared opts with per-channel config (channel-specific wins)
        const mergedConfig = { ...sharedOpts, ...channelConfig };
        const gateway = await launcher(channelConfig, mergedConfig);
        this.gateways.set(name, gateway);
        started.push(name);
        console.log(`[Orchestrator] ${name} started successfully.`);
      } catch (err) {
        failed.push({ channel: name, error: err.message });
        console.error(`[Orchestrator] Failed to start ${name}: ${err.message}`);
      }
    }

    // 8. Start plugin services
    const pluginRegistry = getPluginRegistry();
    for (const service of pluginRegistry.getServices()) {
      try {
        await service.start();
        console.log(`[Orchestrator] Started plugin service: ${service.name}`);
      } catch (err) {
        console.error(
          `[Orchestrator] Failed to start plugin service ${service.name}: ${err.message}`,
        );
      }
    }

    // Mount webchat routes into HTTP gateway via plugin registry
    if (this._httpGateway && this.gateways.has('webchat')) {
      try {
        const webchatGw = this.gateways.get('webchat');
        if (typeof webchatGw.getRoutes === 'function') {
          const routes = webchatGw.getRoutes();
          for (const route of routes) {
            pluginRegistry._routes.push(route);
          }
          console.log(`[Orchestrator] Mounted ${routes.length} webchat routes into HTTP gateway.`);
        }
      } catch (err) {
        console.error(`[Orchestrator] Failed to mount webchat routes: ${err.message}`);
      }
    }

    // Fire gateway_start hook
    pluginRegistry.getHookRunner().run('gateway_start', { channels: started });

    this._running = true;
    return { started, failed };
  }

  /**
   * Stop all running gateways.
   */
  async shutdown() {
    console.log('[Orchestrator] Shutting down all channels...');

    const pluginRegistry = getPluginRegistry();

    // Fire gateway_stop hook
    pluginRegistry.getHookRunner().run('gateway_stop', { channels: [...this.gateways.keys()] });

    // Stop HTTP gateway
    if (this._httpGateway) {
      try {
        await this._httpGateway.stop();
        console.log('[Orchestrator] HTTP gateway stopped.');
      } catch (err) {
        console.error(`[Orchestrator] Error stopping HTTP gateway: ${err.message}`);
      }
      this._httpGateway = null;
    }

    // Destroy subsystems
    if (this._voice) {
      try {
        this._voice.destroy();
      } catch (err) {
        console.warn('[Orchestrator] Voice cleanup error:', err.message);
      }
      this._voice = null;
    }
    if (this._browser) {
      try {
        await this._browser.close();
      } catch (err) {
        console.warn('[Orchestrator] Browser cleanup error:', err.message);
      }
      this._browser = null;
    }
    if (this._memory) {
      try {
        this._memory.close();
      } catch (err) {
        console.warn('[Orchestrator] Memory cleanup error:', err.message);
      }
      this._memory = null;
    }

    // Stop event bridge
    if (this._eventBridge) {
      this._eventBridge.stop();
      this._eventBridge = null;
      console.log('[Orchestrator] Event bridge stopped.');
    }

    // Unregister autonomous commands
    unregisterAutonomousCommands();

    // Stop plugin services
    for (const service of pluginRegistry.getServices()) {
      try {
        await service.stop();
        console.log(`[Orchestrator] Stopped plugin service: ${service.name}`);
      } catch (err) {
        console.error(
          `[Orchestrator] Error stopping plugin service ${service.name}: ${err.message}`,
        );
      }
    }

    for (const [name, gateway] of this.gateways) {
      try {
        if (typeof gateway.shutdown === 'function') {
          await gateway.shutdown();
        }
        console.log(`[Orchestrator] ${name} shut down.`);
      } catch (err) {
        console.error(`[Orchestrator] Error shutting down ${name}: ${err.message}`);
      }
    }

    this.gateways.clear();

    if (this.sessionStore) {
      this.sessionStore.close();
      this.sessionStore = null;
    }

    if (this.identityStore) {
      this.identityStore.close();
      this.identityStore = null;
    }

    this._running = false;
    console.log('[Orchestrator] All channels stopped.');
  }

  /**
   * Get status of all channels.
   */
  getStatus() {
    const channels = {};
    for (const [name] of this.gateways) {
      channels[name] = { status: 'running' };
    }
    return {
      running: this._running,
      channels,
      metrics: getMetrics().getSummary(),
      notifier: {
        registeredChannels: getNotifier().getRegisteredChannels(),
        routes: getNotifier().getRoutes(),
      },
      skills: {
        total: getSkillRegistry().count(),
        ...getSkillRegistry().getStats(),
      },
      plugins: {
        loaded: getPluginRegistry().listPlugins(),
        commands: getCommandRegistry().getStats(),
        gatewayMethods: getGatewayMethods().list().length,
        cliExtensions: getCliExtensions().list().length,
        slots: getPluginSlots().getSlotStates(),
      },
      voice: {
        enabled: !!this._voice,
      },
      browser: {
        enabled: !!this._browser,
      },
      memory: {
        enabled: !!this._memory,
      },
      httpGateway: {
        enabled: !!this._httpGateway,
        address: this._httpGateway ? this._httpGateway.getAddress() : null,
      },
    };
  }
}

/**
 * Load orchestrator config from a JSON or YAML file.
 *
 * @param {string} configPath
 * @returns {Object}
 */
export async function loadOrchestratorConfig(configPath) {
  const fs = await import('fs');
  const raw = fs.readFileSync(configPath, 'utf-8');

  if (configPath.endsWith('.yaml') || configPath.endsWith('.yml')) {
    const { parse } = await import('yaml');
    return parse(raw);
  }

  return JSON.parse(raw);
}

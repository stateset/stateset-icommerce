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
   */
  constructor(config) {
    this.config = config;
    this.gateways = new Map();
    this.sessionStore = null;
    this.identityStore = null;
    this.middleware = [];
    this._running = false;
  }

  /**
   * Start all configured channels.
   *
   * @returns {Promise<{ started: string[], failed: { channel: string, error: string }[] }>}
   */
  async start() {
    if (this._running) throw new Error('Orchestrator is already running');

    const { channels, shared = {}, middleware: mwConfig, notifications, persistSessions = true, sessionDbPath, identityDbPath } = this.config;

    // 1. Session store
    if (persistSessions) {
      this.sessionStore = new ChannelSessionStore(sessionDbPath ? { dbPath: sessionDbPath } : undefined);
    }

    // 2. Identity store
    this.identityStore = new CustomerIdentityStore(identityDbPath ? { dbPath: identityDbPath } : undefined);

    // 3. Middleware
    this.middleware = buildMiddleware(mwConfig);

    // 4. Notification routes
    if (notifications?.routes) {
      getNotifier().loadRoutes(notifications.routes);
    }

    // 5. Build shared options
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
    };

    // 5. Launch channels
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

    this._running = true;
    return { started, failed };
  }

  /**
   * Stop all running gateways.
   */
  async shutdown() {
    console.log('[Orchestrator] Shutting down all channels...');

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

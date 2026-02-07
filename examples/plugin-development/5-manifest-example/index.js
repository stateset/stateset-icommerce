/**
 * Manifest Example Plugin
 *
 * A complete example plugin that pairs with `stateset.plugin.json`.
 *
 * Demonstrates:
 * - commands: `api.registerCommand()`
 * - services: `api.registerService()`
 * - HTTP routes: `api.registerHttpRoute()`
 */

export default function init(api, context = {}) {
  const { config = {}, manifest = {}, origin } = context;

  const displayName = manifest.name || 'Manifest Example Plugin';
  const version = manifest.version || '0.0.0';

  function log(level, message) {
    const allowedLevels = ['debug', 'info', 'warn', 'error'];
    const configLevel = config.logLevel || 'info';
    if (allowedLevels.indexOf(level) >= allowedLevels.indexOf(configLevel)) {
      console.log(`[${displayName}] ${level.toUpperCase()}: ${message}`);
    }
  }

  // -------------------------------------------------------------------------
  // Commands
  // -------------------------------------------------------------------------

  api.registerCommand({
    name: 'example-greet',
    description: 'Greet the user with a personalized message',
    acceptsArgs: true,
    handler: async (argText) => {
      const name = (argText || '').trim() || 'User';
      log('info', `Greeting command executed for ${name}`);
      return { response: `Hello, ${name}! Welcome to ${displayName} v${version}.` };
    },
  });

  api.registerCommand({
    name: 'example-status',
    description: 'Display plugin status and configuration (demo)',
    acceptsArgs: false,
    handler: async () => {
      const status = {
        plugin: displayName,
        version,
        enabled: !!config.enabled,
        origin: origin || 'unknown',
        config: {
          enabled: !!config.enabled,
          maxRetries: config.maxRetries ?? 3,
          logLevel: config.logLevel ?? 'info',
          hasApiKey: !!config.apiKey,
        },
      };
      return { response: JSON.stringify(status, null, 2) };
    },
  });

  api.registerCommand({
    name: 'example-metrics',
    description: 'Display plugin performance metrics (demo)',
    acceptsArgs: false,
    handler: async () => {
      const metrics = {
        uptimeS: process.uptime(),
        memoryUsage: {
          rssMB: Math.round(process.memoryUsage().rss / 1024 / 1024),
          heapTotalMB: Math.round(process.memoryUsage().heapTotal / 1024 / 1024),
          heapUsedMB: Math.round(process.memoryUsage().heapUsed / 1024 / 1024),
        },
        timestamp: new Date().toISOString(),
      };
      return { response: JSON.stringify(metrics, null, 2) };
    },
  });

  // -------------------------------------------------------------------------
  // HTTP routes
  // -------------------------------------------------------------------------

  api.registerHttpRoute({
    method: 'GET',
    path: '/example/health',
    level: 'none',
    handler: async () => {
      log('info', 'Health check requested');
      return {
        status: 200,
        body: {
          status: 'healthy',
          plugin: displayName,
          version,
          enabled: !!config.enabled,
          timestamp: new Date().toISOString(),
        },
      };
    },
  });

  api.registerHttpRoute({
    method: 'POST',
    path: '/example/data',
    level: 'write',
    handler: async ({ body }) => {
      try {
        const data = body?.data;
        log('info', `Data received: ${JSON.stringify(data)}`);

        if (!config.enabled) {
          return { status: 403, body: { error: 'Plugin is disabled' } };
        }

        return {
          status: 200,
          body: {
            success: true,
            processedAt: new Date().toISOString(),
            data,
          },
        };
      } catch (error) {
        log('error', `Error processing data: ${error.message}`);
        return { status: 500, body: { error: error.message } };
      }
    },
  });

  // -------------------------------------------------------------------------
  // Background service
  // -------------------------------------------------------------------------

  let timer = null;
  api.registerService({
    name: 'example-background-service',
    start: async () => {
      timer = setInterval(() => {
        log('debug', 'Background service heartbeat');
      }, 60_000);
      if (timer.unref) timer.unref();
    },
    stop: async () => {
      if (timer) clearInterval(timer);
      timer = null;
    },
  });

  log('info', `Loaded v${version}`);
}


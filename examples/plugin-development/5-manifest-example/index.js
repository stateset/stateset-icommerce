const { PluginAPI } = require('@stateset/cli/src/channels/plugin-api');

async function init(api, context) {
  const { config, manifest, origin, runtime } = context;
  const { displayName, version } = manifest;

  async function log(level, message) {
    const allowedLevels = ['debug', 'info', 'warn', 'error'];
    const configLevel = config.logLevel || 'info';
    
    if (allowedLevels.indexOf(level) >= allowedLevels.indexOf(configLevel)) {
      console.log(`[${displayName}] ${level.toUpperCase()}: ${message}`);
    }
  }

  api.registerCommand({
    name: 'example:greet',
    description: 'Greet the user with personalized message',
    options: [
      { name: 'name', type: 'String', description: 'Your name', required: false }
    ]
  }, async (args, req) => {
    const name = args.name || 'User';
    log('info', `Greeting command executed for ${name}`);
    return { message: `Hello, ${name}! Welcome to ${displayName} v${version}.` };
  });

  api.registerCommand({
    name: 'example:status',
    description: 'Display plugin status and configuration',
    options: []
  }, async (args, req) => {
    log('info', 'Status command executed');
    return {
      plugin: displayName,
      version,
      status: config.enabled ? 'enabled' : 'disabled',
      origin,
      runtime: {
        version: runtime.version,
        environment: runtime.environment
      },
      config: {
        enabled: config.enabled,
        maxRetries: config.maxRetries ?? 3,
        logLevel: config.logLevel ?? 'info',
        hasApiKey: !!config.apiKey
      }
    };
  });

  api.registerCommand({
    name: 'example:metrics',
    description: 'Display plugin performance metrics',
    options: []
  }, async (args, req) => {
    log('info', 'Metrics command executed');
    return {
      plugin: displayName,
      metrics: {
        uptime: process.uptime(),
        memoryUsage: {
          rss: Math.round(process.memoryUsage().rss / 1024 / 1024) + 'MB',
          heapTotal: Math.round(process.memoryUsage().heapTotal / 1024 / 1024) + 'MB',
          heapUsed: Math.round(process.memoryUsage().heapUsed / 1024 / 1024) + 'MB'
        },
        commandsExecuted: 0,
        eventsProcessed: 0,
        servicesActive: 1
      },
      timestamp: new Date().toISOString()
    };
  });

  api.registerHttpRoute({
    method: 'GET',
    path: '/example/health',
    description: 'Health check endpoint'
  }, async (req, res) => {
    log('info', 'Health check requested');
    res.json({
      status: 'healthy',
      plugin: displayName,
      version,
      enabled: config.enabled,
      timestamp: new Date().toISOString()
    });
  });

  api.registerHttpRoute({
    method: 'POST',
    path: '/example/data',
    description: 'Accept and process data'
  }, async (req, res) => {
    try {
      const { data } = req.body;
      log('info', `Data received: ${JSON.stringify(data)}`);

      if (!config.enabled) {
        return res.status(403).json({ error: 'Plugin is disabled' });
      }

      res.json({
        success: true,
        processedAt: new Date().toISOString(),
        data
      });
    } catch (error) {
      log('error', `Error processing data: ${error.message}`);
      res.status(500).json({ error: error.message });
    }
  });

  api.on('message_sending', async (data, ctx, next) => {
    log('debug', `Message sending to ${data.channel}`);
    return next();
  });

  api.on('message_sent', async (result, ctx) => {
    log('info', `Message sent to ${result.channel} in stream ${ctx.streamId}`);
  });

  api.on('message_failed', async (error, ctx) => {
    log('error', `Message failed in stream ${ctx.streamId}: ${error.message}`);
  });

  api.on('agent_start', async (agent, ctx) => {
    log('info', `Agent started: ${agent.name} (type: ${agent.type})`);
  });

  api.on('agent_end', async (result, ctx) => {
    const status = result.success ? 'completed' : 'failed';
    log('info', `Agent ${ctx.agentName} ${status} in ${result.duration}ms`);
  });

  const backgroundService = {
    name: `${displayName} Monitor`,
    type: 'monitor',
    handler: async () => {
      log('debug', 'Background service heartbeat');
    }
  };

  api.registerService(backgroundService);

  log('info', `Plugin initialized successfully (version: ${version}, origin: ${origin})`);
}

module.exports = { init };
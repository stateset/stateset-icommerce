/**
 * Example Plugin: Slack Integration (Mock)
 *
 * Demonstrates:
 * - commands via `api.registerCommand()`
 * - an admin HTTP route via `api.registerHttpRoute()`
 * - a background service via `api.registerService()`
 */

export default function init(api, context = {}) {
  const { config = {}, manifest = {} } = context;
  const pluginName = manifest.name || 'Slack Integration Plugin';

  const slackWebhookUrl = config.slackWebhookUrl || null;
  const enabledChannels = Array.isArray(config.enabledChannels) ? config.enabledChannels : [];
  const alertThresholds = config.alertThresholds || { errorRate: 0.1, responseTime: 5000 };

  api.registerCommand({
    name: 'slack-test',
    description: 'Send a test notification to Slack (mock)',
    acceptsArgs: false,
    handler: async () => {
      const lines = [
        'Slack test (mock):',
        `webhookUrl: ${slackWebhookUrl ? 'configured' : 'not configured'}`,
        `enabledChannels: ${enabledChannels.join(', ') || 'none'}`,
      ];
      return { response: lines.join('\n') };
    },
  });

  api.registerCommand({
    name: 'slack-config',
    description: 'Display Slack integration configuration (mock)',
    acceptsArgs: false,
    handler: async () => {
      const cfg = {
        plugin: pluginName,
        version: manifest.version || '1.0.0',
        webhookUrl: slackWebhookUrl ? 'configured' : 'missing',
        enabledChannels,
        alertThresholds,
      };
      return { response: JSON.stringify(cfg, null, 2) };
    },
  });

  api.registerHttpRoute({
    method: 'POST',
    path: '/slack/source/diagnostics',
    level: 'admin',
    handler: async () => {
      const diagnostics = {
        plugin: pluginName,
        version: manifest.version || '1.0.0',
        webhookUrl: slackWebhookUrl ? 'configured' : 'missing',
        channels: {
          total: enabledChannels.length,
          configured: enabledChannels,
        },
        thresholds: alertThresholds,
        uptime: process.uptime(),
      };
      return { status: 200, body: { success: true, diagnostics } };
    },
  });

  let timer = null;
  api.registerService({
    name: 'slack-health-monitor',
    start: async () => {
      timer = setInterval(() => {
        console.log(`[${pluginName}] Health check tick`);
      }, 60_000);
      if (timer.unref) timer.unref();
    },
    stop: async () => {
      if (timer) clearInterval(timer);
      timer = null;
    },
  });

  api.on('agent_end', async (data) => {
    if (data?.success === false) {
      console.log(`[${pluginName}] agent_end failure: ${data.error || 'Unknown error'}`);
    }
  });

  console.log(`${pluginName} initialized with ${enabledChannels.length} channels`);
}

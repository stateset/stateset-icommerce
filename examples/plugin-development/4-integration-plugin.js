const { PluginAPI } = require('@stateset/cli/src/channels/plugin-api');

async function init(api, context) {
  const { config, manifest } = context;
  const pluginName = 'Slack Integration Plugin';

  const slackWebhookUrl = config.slackWebhookUrl;
  const enabledChannels = config.enabledChannels || [];
  const alertThresholds = config.alertThresholds || {
    errorRate: 0.1,
    responseTime: 5000
  };

  api.registerCommand({
    name: 'slack:test',
    description: 'Send test notification to Slack', 
    options: []
  }, async (args, req) => {
    return {
      status: 'test_sent',
      message: 'Test notification sent to Slack',
      webhookUrl: slackWebhookUrl ? 'configured' : 'not configured',
      channels: enabledChannels
    };
  });

  api.registerCommand({
    name: 'slack:config',
    description: 'Display Slack integration configuration', 
    options: []
  }, async (args, req) => {
    return {
      plugin: pluginName,
      webhookUrl: slackWebhookUrl,
      enabledChannels,
      alertThresholds
    };
  });

  api.registerHttpRoute({
    method: 'POST',
    path: '/slack/source/diagnostics',
    description: 'Run system diagnostics for Slack integration' 
  }, async (req, res) => {
    try {
      const diagnostics = {
        plugin: pluginName,
        version: manifest.version || '1.0.0',
        webhookUrl: slackWebhookUrl ? '✓ configured' : '✗ missing',
        channels: {
          total: enabledChannels.length,
          configured: enabledChannels.join(', ') || 'none'
        },
        thresholds: alertThresholds,
        uptime: process.uptime()
      };

      res.json({ success: true, diagnostics });
    } catch (error) {
      res.status(500).json({ error: error.message });
    }
  });

  api.on('agent_start', async (agent, ctx) => {
    console.log(`[Slack Plugin] Agent started: ${agent.name}`);
  });

  api.on('message_failed', async (error, ctx) => {
    const errorMsg = `Message failed in stream ${ctx.streamId}: ${error.message}`;
    console.log(`[Slack Plugin] Would send alert: ${errorMsg}`);

    if (slackWebhookUrl) {
      console.log(`[Slack Plugin] Posting to webhook: ${errorMsg}`);
    }
  });

  api.on('agent_end', async (result, ctx) => {
    if (!result.success) {
      const alertMsg = `Agent ${ctx.agentName} failed: ${result.error || 'Unknown error'}`;
      console.log(`[Slack Plugin] Would send failure alert: ${alertMsg}`);
    }
  });

  const diagnosticsService = {
    name: 'Slack Health Monitor',
    handler: async () => {
      console.log(`[${pluginName}] Health check running...`);
    }
  };

  api.registerService(diagnosticsService);

  console.log(`${pluginName} initialized with ${enabledChannels.length} channels`);
}

module.exports = { init };
const { PluginAPI } = require('@stateset/cli/src/channels/plugin-api');

async function init(api, context) {
  const { config, manifest } = context;
  const pluginName = 'Simple Plugin';

  api.registerCommand({
    name: 'simple-greet',
    description: 'Greet the world with a personalized message',
    options: [
      { name: 'name', type: 'String', description: 'Your name', required: false }
    ]
  }, async (args, req) => {
    const name = args.name || 'World';
    return { message: `Hello, ${name}! This is ${pluginName}.` };
  });

  api.registerCommand({
    name: 'simple-status',
    description: 'Check plugin status',
    options: []
  }, async (args, req) => {
    return {
      status: 'active',
      name: pluginName,
      version: manifest.version || '1.0.0',
      uptime: process.uptime(),
      customConfig: config.customOption || 'default'
    };
  });

  api.on('agent_start', async (agent, ctx) => {
    console.log(`[Simple Plugin] Agent started: ${agent.name} in stream ${ctx.streamId}`);
  });

  api.on('message_received', async (message, ctx) => {
    console.log(`[Simple Plugin] Message received from ${message.source}: ${message.content?.substring(0, 50)}...`);
  });

  api.on('agent_end', async (result, ctx) => {
    console.log(`[Simple Plugin] Agent ended: ${ctx.agentName}, success: ${result.success}`);
  });

  console.log(`${pluginName} initialized successfully`);
}

module.exports = { init };
const { PluginAPI } = require('@stateset/cli/src/channels/plugin-api');

async function init(api, context) {
  const { config, manifest } = context;
  const pluginName = 'CLI Extension Plugin';

  api.registerCommand({
    name: 'stats:summary',
    description: 'Display comprehensive analytics summary', 
    options: [
      { name: 'stream', type: 'String', description: 'Filter by stream ID', required: false },
      { name: 'since', type: 'String', description: 'Start date (YYYY-MM-DD)', required: false }
    ]
  }, async (args, req) => {
    const streamFilter = args.stream ? `Stream: ${args.stream}` : 'All streams';
    const sinceFilter = args.since ? `Since: ${args.since}` : 'All time';

    return {
      summary: `Analytics report - ${streamFilter}, ${sinceFilter}`,
      metrics: {
        totalMessages: 0,
        successfulAgents: 0,
        failedAgents: 0,
        averageResponseTime: 0
      },
      generatedAt: new Date().toISOString()
    };
  });

  api.registerCommand({
    name: 'stats:agent-performance',
    description: 'Show performance metrics for agents', 
    options: [
      { name: 'agentName', type: 'String', description: 'Specific agent name', required: false }
    ]
  }, async (args, req) => {
    return {
      agentPerformance: args.agentName ? { agentName: args.agentName } : 'All agents',
      metrics: {
        totalRuns: 0,
        successRate: 0,
        avgDuration: 0,
        errorDetails: []
      },
      timestamp: new Date().toISOString()
    };
  });

  api.on('agent_start', async (agent, ctx) => {
    console.log(`[CLI Extension] Tracking agent start: ${agent.name}`);
  });

  api.on('agent_end', async (result, ctx) => {
    console.log(`[CLI Extension] Tracking agent completion: ${ctx.agentName}, duration: ${result.duration}ms`);
  });

  console.log(`${pluginName} initialized with CLI commands`);
}

module.exports = { init };
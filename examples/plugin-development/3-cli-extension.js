/**
 * Example Plugin: CLI Extension Commands
 *
 * Demonstrates registering additional commands in the dynamic CommandRegistry.
 */

export default function init(api, context = {}) {
  const pluginName = context.manifest?.name || 'CLI Extension Plugin';

  api.registerCommand({
    name: 'stats-summary',
    description: 'Display a sample analytics summary (demo)',
    acceptsArgs: true,
    handler: async (argText) => {
      const hint = (argText || '').trim();
      const lines = [
        'Analytics summary (demo):',
        hint ? `filters: ${hint}` : 'filters: none',
        '',
        'metrics:',
        '- totalMessages: 0',
        '- successfulAgents: 0',
        '- failedAgents: 0',
        '- averageResponseTimeMs: 0',
        '',
        `generatedAt: ${new Date().toISOString()}`,
      ];
      return { response: lines.join('\n') };
    },
  });

  api.registerCommand({
    name: 'stats-agent-performance',
    description: 'Show sample agent performance metrics (demo)',
    acceptsArgs: true,
    handler: async (argText) => {
      const agentName = (argText || '').trim() || 'all';
      const lines = [
        `Agent performance (demo) for: ${agentName}`,
        '',
        'metrics:',
        '- totalRuns: 0',
        '- successRate: 0',
        '- avgDurationMs: 0',
        '',
        `timestamp: ${new Date().toISOString()}`,
      ];
      return { response: lines.join('\n') };
    },
  });

  api.on('agent_end', async (data) => {
    const agentName = data?.agentName || 'unknown';
    const success = data?.success ?? true;
    console.log(`[${pluginName}] agent_end: ${agentName} success=${success}`);
  });

  console.log(`${pluginName} initialized with CLI commands`);
}

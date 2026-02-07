/**
 * Example Plugin: Simple Commands + Hooks
 *
 * Demonstrates:
 * - `api.registerCommand()` signature: handler(argText, context) => { response }
 * - `api.on()` hooks
 */

export default function init(api, context = {}) {
  const { config = {}, manifest = {} } = context;
  const pluginName = manifest.name || 'Simple Plugin';

  api.registerCommand({
    name: 'simple-greet',
    description: 'Greet the user with a personalized message',
    acceptsArgs: true,
    handler: async (argText) => {
      const name = (argText || '').trim() || 'World';
      return { response: `Hello, ${name}! This is ${pluginName}.` };
    },
  });

  api.registerCommand({
    name: 'simple-status',
    description: 'Check plugin status',
    acceptsArgs: false,
    handler: async () => {
      const lines = [
        `status: active`,
        `name: ${pluginName}`,
        `version: ${manifest.version || '1.0.0'}`,
        `uptime_s: ${Math.round(process.uptime())}`,
        `customOption: ${config.customOption || 'default'}`,
      ];
      return { response: lines.join('\n') };
    },
  });

  api.on('plugin_loaded', async () => {
    console.log(`[${pluginName}] plugin_loaded`);
  });

  api.on('message_received', async (data) => {
    const text = data?.text || data?.message?.content || '';
    console.log(`[${pluginName}] message_received: ${text.slice(0, 80)}`);
  });

  console.log(`${pluginName} initialized successfully`);
}

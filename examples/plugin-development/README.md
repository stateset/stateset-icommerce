# Plugin Development Guide

This guide shows you how to create plugins for StateSet iCommerce. Plugins can extend the system with:
- Custom commands
- Event hooks (parallel or sequential)
- Background services
- HTTP routes (for webhooks)
- CLI extensions

## Quick Start

### 1. Simple Plugin with Commands

Create a basic plugin that registers a single command:

```javascript
// my-plugin.js
export default function init(api) {
  api.registerCommand({
    name: 'hello',
    description: 'Say hello',
    acceptsArgs: true,
    handler: async (argText) => {
      const name = argText.trim() || 'World';
      return { response: `Hello, ${name}!` };
    },
  });
}
```

Load and test it:

```bash
# Load the plugin
stateset plugin load ./my-plugin.js

# Use the command /hello Alice
```

### 2. Plugin with Hooks

Listen to events in the message lifecycle:

```javascript
export default function init(api) {
  // Parallel hook (fire-and-forget)
  api.on('message_received', async ({ text, senderId, channel }) => {
    console.log(`[${channel}] ${senderId}: ${text}`);
  });

  // Sequential hook (can modify data)
  api.on('message_sending', async ({ text, session }) => {
    return {
      text: `[Bot] ${text}`,
      metadata: { modified: true }
    };
  });
}
```

### 3. Plugin with Background Service

Run a background task:

```javascript
export default function init(api) {
  let timer = null;

  api.registerService({
    name: 'my-service',
    start: async () => {
      timer = setInterval(() => {
        console.log('Service tick');
      }, 60000);
    },
    stop: async () => {
      if (timer) clearInterval(timer);
    },
  });
}
```

### 4. Plugin with HTTP Routes

Add webhook endpoints:

```javascript
export default function init(api) {
  api.registerHttpRoute({
    method: 'POST',
    path: '/webhook/notify',
    handler: async (req, res) => {
      const body = await req.json();
      console.log('Webhook received:', body);
      res.writeHead(200);
      res.end(JSON.stringify({ received: true }));
    },
  });
}
```

## Plugin Manifest

For production plugins, create a `stateset-manifest.json`:

```json
{
  "id": "my-plugin",
  "name": "My Plugin",
  "version": "1.0.0",
  "description": "A helpful plugin",
  "author": "Your Name",
  "license": "MIT",
  "entry": "index.js",
  "kind": "general",
  "channels": ["all"],
  "provides": [],
  "enabledByDefault": true,
  "configSchema": {
    "type": "object",
    "properties": {
      "apiKey": { "type": "string", "description": "API key" },
      "interval": { "type": "number", "default": 60 }
    }
  },
  "configDefaults": {
    "interval": 60
  }
}
```

Access config in your plugin:

```javascript
export default function init(api, { config }) {
  const apiKey = config.apiKey;
  const interval = config.interval || 60;
  // ...
}
```

## Hooks Reference

### Parallel Hooks

These run in parallel, errors are logged but don't block:

| Hook Name | When | Data |
|-----------|------|------|
| `message_received` | Inbound message received | `{ text, senderId, channel, timestamp }` |
| `message_sent` | Message sent to user | `{ text, channel, timestamp }` |
| `agent_end` | Agent completes processing | `{ result, session, timestamp }` |
| `after_tool_call` | Tool execution completes | `{ toolName, args, result, timestamp }` |
| `after_command` | Command execution completes | `{ commandName, result, timestamp }` |
| `session_start` | Session initialized | `{ sessionId, channel, timestamp }` |
| `session_end` | Session closed | `{ sessionId, reason, timestamp }` |
| `gateway_start` | Gateway starts | `{ timestamp }` |
| `gateway_stop` | Gateway stops | `{ timestamp }` |
| `after_compaction` | Message compaction done | `{ before, after, saved, timestamp }` |
| `plugin_loaded` | Plugin loaded | `{ pluginId, timestamp }` |
| `plugin_unloaded` | Plugin unloaded | `{ pluginId, timestamp }` |

### Sequential Hooks

These run in order, can modify data:

| Hook Name | When | Data | Return Value |
|-----------|------|------|--------------|
| `message_sending` | Before sending message | `{ text, session }` | Modified object |
| `before_agent_start` | Before agent runs | `{ text, session, tools }` | Modified object |
| `before_tool_call` | Before tool call | `{ toolName, args, session }` | Modified object to continue, false to block |
| `before_command` | Before command runs | `{ commandName, args, session }` | Modified object |
| `before_compaction` | Before compaction | `{ messages, session }` | Modified object |
| `tool_result_persist` | Persisting tool result | `{ toolName, result, message }` | Modified message |
| `before_send` | Final send check | `{ text, channel, metadata }` | Modified object |

## Priority

Use `priority` to control hook execution order (lower = earlier):

```javascript
api.on('message_sending', handler, { priority: 50 });
```

Default priority is 100.

## Plugin Loading

Plugins are discovered from these locations (in priority order):

1. **Bundled**: `src/plugins/` (built-in)
2. **Global**: `~/.stateset/plugins/` (user-wide)
3. **Workspace**: `.stateset/plugins/` (project-specific)
4. **Config**: Paths in `stateset.config.json`

Example config:

```json
{
  "plugins": {
    "my-plugin": {
      "path": "./plugins/my-plugin",
      "config": {
        "apiKey": "sk-xxx",
        "interval": 30
      },
      "enabled": true
    }
  }
}
```

## Examples

Run the examples in this directory:

```bash
node 1-simple-plugin.js
node 2-webhook-plugin.js
node 3-cli-extension.js
node 4-integration-plugin.js
node 5-manifest-example/index.js
```

See `example-manifest/` for a complete plugin with manifest, tests, and config.

## Best Practices

1. **Handle errors gracefully**: Wrap async operations in try/catch
2. **Clean up resources**: Use `api.registerService` to manage timers/intervals
3. **Validate config**: Use schema validation in your manifest
4. **Document dependencies**: List external packages in your plugin's `package.json`
5. **Test thoroughly**: Use hooks responsibly, avoid blocking operations in parallel hooks

## CLI Extensions

Plugins can add custom CLI commands via `getCliExtensions()`:

```javascript
export default function init(api) {
  return {
    getCliExtensions: () => [{
      command: 'my:command',
      description: 'My custom command',
      handler: async (args) => {
        console.log('Running my:command with args:', args);
      },
    }],
  };
}
```

## API Reference

### `api.registerCommand(definition)`

Register a custom slash command.

```typescript
interface CommandDefinition {
  name: string;           // Command name (without /)
  description: string;    // Help text
  acceptsArgs?: boolean;  // Whether command accepts arguments (default: true)
  handler: (argText: string) => Promise<{ response: string }>;
}
```

### `api.on(hookName, handler, opts)`

Register a hook handler.

```typescript
function on(
  hookName: string,
  handler: (data: any) => Promise<any | void>,
  opts?: { priority?: number }
): void;
```

### `api.registerService(service)`

Register a background service.

```typescript
interface ServiceDefinition {
  name: string;
  start: () => Promise<void>;
  stop: () => Promise<void>;
}
```

### `api.registerHttpRoute(route)`

Register an HTTP route.

```typescript
interface HttpRouteDefinition {
  method: 'GET' | 'POST' | 'PUT' | 'DELETE';
  path: string;
  handler: (req: IncomingMessage, res: ServerResponse) => Promise<void>;
}
```

## Support

For questions or issues:
- Check the examples in this directory
- Review existing plugins in `cli/src/plugins/`
- Read the tests in `cli/tests/plugin-system.test.js`
# Plugin Development Guide for StateSet iCommerce

This guide shows you how to create custom plugins for StateSet iCommerce. Plugins extend the gateway with custom commands, hooks, services, and HTTP routes.

## Table of Contents

1. [Plugin Architecture](#plugin-architecture)
2. [Building Your First Plugin](#building-your-first-plugin)
3. [Plugin Manifest](#plugin-manifest)
4. [Plugin API](#plugin-api)
   - [Commands](#commands)
   - [Hooks](#hooks)
   - [Services](#services)
   - [HTTP Routes](#http-routes)
5. [Plugin Configuration](#plugin-configuration)
6. [Examples](#examples)
7. [Testing Plugins](#testing-plugins)
8. [Best Practices](#best-practices)

---

## Plugin Architecture

A plugin is a JavaScript module that exports an initialization function:

```javascript
// my-plugin/index.js
export default function init(api, { config, manifest, runtime }) {
  // Register commands, hooks, services, routes
  api.registerCommand({ ... });
  api.on('message_received', async (data) => { ... });
}
```

### Key Concepts

| Concept | Description |
|---------|-------------|
| **Plugin API** | Object provded to init function for registration |
| **Commands** | Bot commands like `/orders`, `/inventory` |
| **Hooks** | Event handlers for message lifecycle |
| **Services** | Background tasks with start/stop lifecycle |
| **Routes** | HTTP endpoints on the gateway |
| **Manifest** | Plugin metadata (name, version, config schema) |
| **Config** | Plugin-specific configuration |

---

## Building Your First Plugin

### Step 1: Create Plugin Directory

```bash
mkdir my-plugin
cd my-plugin
```

### Step 2: Create the Plugin File

```javascript
// index.js
export default function init(api, { config }) {
  api.registerCommand({
    name: 'hello',
    description: 'Say hello from my plugin',
    acceptsArgs: false,
    handler: async () => {
      const message = config.customMessage || 'Hello from my plugin!';
      return { response: message };
    },
  });
}
```

### Step 3: Create Manifest

```json
// manifest.json
{
  "id": "my-plugin",
  "name": "My Plugin",
  "version": "1.0.0",
  "description": "A simple example plugin",
  "author": "Your Name",
  "license": "MIT",
  "entry": "index.js",
  "kind": "general",
  "configDefaults": {
    "customMessage": "Hello from my plugin!"
  }
}
```

### Step 4: Load the Plugin

Add to your gateway config:

```json
{
  "plugins": {
    "entries": {
      "my-plugin": {
        "path": "./my-plugin",
        "enabled": true
      }
    }
  }
}
```

---

## Plugin Manifest

The manifest describes your plugin and validates configuration:

```json
{
  "id": "my-awesome-plugin",
  "name": "My Awesome Plugin",
  "version": "1.2.3",
  "description": "Does awesome things",
  "author": "Your Name <email@example.com>",
  "license": "MIT",
  "entry": "index.js",
  "kind": "integration",
  "channels": ["slack", "discord"],
  "provides": ["analytics", "reporting"],
  "enabledByDefault": true,
  "configSchema": {
    "type": "object",
    "required": ["apiKey"],
    "properties": {
      "apiKey": { "type": "string" },
      "timeout": { "type": "number", "minimum": 1000 },
      "enabled": { "type": "boolean" }
    }
  },
  "configDefaults": {
    "timeout": 5000,
    "enabled": true
  }
}
```

### Manifest Fields

| Field | Required | Description |
|-------|----------|-------------|
| `id` | ✓ | Unique identifier (kebab-case) |
| `name` | ✓ | Human-readable name |
| `version` | ✓ | SemVer version string |
| `entry` | ✓ | Main module filename |
| `kind` | ✓ | Plugin type: `general`, `integration`, `channel` |
| `description` | | Short description |
| `author` | | Author name/email |
| `license` | | License identifier |
| `channels` | | Targeted channels (empty = all) |
| `provides` | | Capabilities provided |
| `configSchema` | | JSON Schema for config validation |
| `configDefaults` | | Default configuration values |

---

## Plugin API

The plugin provides several methods for extending the gateway:

### Commands

Register bot commands that users can invoke:

```javascript
api.registerCommand({
  name: 'weather',
  description: 'Get current weather for a location',
  acceptsArgs: true,
  aliases: ['forecast'],
  handler: async (argText, context) => {
    if (!argText) {
      return { response: 'Usage: /weather <city>' };
    }
    
    // Fetch weather data
    const temp = await fetchWeather(argText);
    return {
      response: `Weather in ${argText}: ${temp}°C`,
      metadata: { city: argText, temp }
    };
  }
});
```

#### Command Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `name` | string | | Command name (required) |
| `description` | string | | Help text |
| `acceptsArgs` | boolean | false | Whether command takes arguments |
| `aliases` | string[] | | Alternative names |
| `hidden` | boolean | false | Hide from help |
| `handler` | function | | Command handler |

#### Handler Signature

```javascript
async handler(argText, context) {
  // argText: Text after command (e.g., "/weather London" → "London")
  // context: { channel, senderId, session, commerce, ... }
  
  return {
    response: 'Reply text',
    metadata: {}, // Optional metadata
    richMessage: null, // Optional rich message
  };
}
```

---

### Hooks

Hooks let you react to events in the message lifecycle:

#### Parallel Hooks (fire-and-forget)

```javascript
// Log every message
api.on('message_received', async ({ text, senderId, channel }) => {
  console.log(`[${channel}] ${senderId}: ${text}`);
}, { priority: 100 });
```

#### Sequential Hooks (can modify data)

```javascript
// Add context before agent processing
api.on('before_agent_start', async ({ text, session }) => {
  const context = getRelevantContext(text);
  return {
    text: `${text}\n\nContext: ${context}`,
    injectedContext: context
  };
}, { priority: 50 });
```

#### Available Hooks

| Hook | Type | When it Runs | Can Modify |
|------|------|--------------|------------|
| `message_received` | Parallel | Message arrives | No |
| `message_sending` | Sequential | Before sending | Yes |
| `before_agent_start` | Sequential | Before agent processes | Yes |
| `before_tool_call` | Sequential | Before tool execution | Yes |
| `after_tool_call` | Parallel | After tool execution | No |
| `agent_end` | Parallel | Agent completes | No |
| `session_start` | Parallel | New session | No |
| `session_end` | Parallel | Session ends | No |
| `gateway_start` | Parallel | Gateway starts | No |
| `gateway_stop` | Parallel | Gateway stops | No |
| `before_send` | Sequential | Final send check | Yes |

---

### Services

Background tasks with start/stop lifecycle:

```javascript
api.registerService({
  name: 'data-sync',
  
  start: async () => {
    // Periodic sync every 5 minutes
    this._timer = setInterval(async () => {
      console.log('[data-sync] Syncing data...');
      await syncData();
    }, 300000);
    console.log('[data-sync] Service started');
  },
  
  stop: async () => {
    if (this._timer) {
      clearInterval(this._timer);
      this._timer = null;
    }
    console.log('[data-sync] Service stopped');
  }
});
```

---

### HTTP Routes

Add custom endpoints to the HTTP gateway:

```javascript
api.registerHttpRoute({
  method: 'GET',
  path: '/api/plugins/metrics',
  level: 'read',
  handler: async () => {
    const metrics = getPluginMetrics();
    return { status: 200, body: metrics };
  }
});

api.registerHttpRoute({
  method: 'POST',
  path: '/api/plugins/webhook',
  level: 'none', // public webhook endpoint; validate signatures/tokens inside the handler
  handler: async ({ body }) => {
    await processWebhook(body);
    return { status: 200, body: { status: 'ok' } };
  }
});
```

---

## Plugin Configuration

Plugins receive configuration from the gateway config:

### Defining Config Schema

```json
{
  "configSchema": {
    "type": "object",
    "required": ["apiKey"],
    "properties": {
      "apiKey": { "type": "string", "minLength": 10 },
      "endpoint": { "type": "string", "format": "uri" },
      "timeout": { "type": "number", "minimum": 100, "maximum": 60000 },
      "enabled": { "type": "boolean" },
      "mode": { "type": "string", "enum": ["fast", "slow", "balanced"] }
    }
  }
}
```

### Setting Config in Gateway

```json
{
  "plugins": {
    "configs": {
      "my-plugin": {
        "apiKey": "sk_live_1234567890abcdef",
        "endpoint": "https://api.example.com",
        "timeout": 10000,
        "enabled": true,
        "mode": "fast"
      }
    }
  }
}
```

### Using Config in Plugin

```javascript
export default function init(api, { config }) {
  const { apiKey, endpoint, timeout, enabled, mode } = config;
  
  if (!enabled) {
    console.log('[my-plugin] Disabled, skipping init');
    return;
  }
  
  // Use config values
  const client = new APIClient(apiKey, { endpoint, timeout });
}
```

---

## Examples

### Example 1: Analytics Plugin

Logs all messages and provides stats:

```javascript
// analytics-plugin/index.js
let messageCount = 0;
const commandCounts = new Map();

export default function init(api, { config }) {
  // Log all messages
  api.on('message_received', async ({ text, channel }) => {
    messageCount++;
    
    if (text.startsWith('/')) {
      const cmd = text.split(' ')[0].substring(1);
      commandCounts.set(cmd, (commandCounts.get(cmd) || 0) + 1);
    }
  }, { priority: 200 });
  
  // Stats command
  api.registerCommand({
    name: 'stats',
    description: 'Show gateway statistics',
    acceptsArgs: false,
    handler: async () => {
      const topCommands = [...commandCounts.entries()]
        .sort((a, b) => b[1] - a[1])
        .slice(0, 5)
        .map(([cmd, count]) => `/${cmd}: ${count}`)
        .join('\\n');
      
      return {
        response: `Statistics:\\n• Total messages: ${messageCount}\\n• Top commands:\\n${topCommands || 'None'}`
      };
    }
  });
}
```

### Example 2: Translation Plugin

Translates messages using an external API:

```javascript
// translation-plugin/index.js
export default function init(api, { config }) {
  const apiKey = config.apiKey;
  const defaultLanguage = config.defaultLanguage || 'en';
  
  // Translate before processing
  api.on('before_agent_start', async ({ text, session }) => {
    const targetLang = session.language || defaultLanguage;
    
    // Detect language
    const detected = await detectLanguage(text);
    if (detected === targetLang) return {}; // No translation needed
    
    // Translate
    const translated = await translateText(text, targetLang, apiKey);
    
    return {
      text: translated,
      originalText: text,
      translated: true
    };
  }, { priority: 30 });
  
  // Language command
  api.registerCommand({
    name: 'lang',
    description: 'Set your language',
    acceptsArgs: true,
    handler: async (argText, context) => {
      if (!argText) {
        return { response: `Current language: ${context.session.language || defaultLanguage}` };
      }
      
      context.session.language = argText.toLowerCase();
      return { response: `Language set to ${argText}` };
    }
  });
}
```

### Example 3: External API Plugin

Fetches data from external APIs:

```javascript
// external-api-plugin/index.js
export default function init(api, { config }) {
  api.registerCommand({
    name: 'crypto',
    description: 'Get cryptocurrency price',
    acceptsArgs: true,
    handler: async (argText) => {
      const symbol = (argText || 'BTC').toUpperCase();
      
      try {
        const res = await fetch(`https://api.coingecko.com/api/v3/simple/price?ids=${symbol.toLowerCase()}&vs_currencies=usd`);
        const data = await res.json();
        
        if (!data[symbol.toLowerCase()]) {
          return { response: `Unknown symbol: ${symbol}` };
        }
        
        const price = data[symbol.toLowerCase()].usd;
        return { response: `${symbol}: $${price.toLocaleString()}` };
      } catch (err) {
        return { response: `Error fetching price: ${err.message}` };
      }
    }
  });
  
  // HTTP endpoint for external access
  api.registerHttpRoute({
    method: 'GET',
    path: '/api/crypto/:symbol',
    level: 'read',
    handler: async ({ params }) => {
      const symbol = params.symbol.toUpperCase();
      
      const res_api = await fetch(`https://api.coingecko.com/api/v3/simple/price?ids=${symbol.toLowerCase()}&vs_currencies=usd`);
      const data = await res_api.json();
      
      return { status: 200, body: data };
    }
  });
}
```

### Example 4: Scheduled Task Plugin

Runs periodic tasks as a service:

```javascript
// scheduled-tasks-plugin/index.js
export default function init(api, { config }) {
  const intervals = config.intervals || {
    daily: 86400000,
    hourly: 3600000
  };
  
  api.registerService({
    name: 'daily-report',
    
    start: async () => {
      // Run immediately then schedule
      await generateDailyReport();
      
      this._dailyTimer = setInterval(async () => {
        await generateDailyReport();
      }, intervals.daily);
      
      console.log('[scheduled-tasks] Daily reports scheduled');
    },
    
    stop: async () => {
      if (this._dailyTimer) {
        clearInterval(this._dailyTimer);
        this._dailyTimer = null;
      }
    }
  });
  
  api.registerService({
    name: 'health-check',
    
    start: async () => {
      this._healthTimer = setInterval(async () => {
        const health = await checkSystemHealth();
        
        if (!health.healthy) {
          // Send alert - we'll notification system
          console.warn('[scheduled-tasks] Health check failed:', health.issues);
          
          // Trigger hook for alerting
          api.hooks?.run?.('health_alert', health);
        }
      }, intervals.hourly);
    },
    
    stop: async () => {
      if (this._healthTimer) {
        clearInterval(this._healthTimer);
        this._healthTimer = null;
      }
    }
  });
}

async function generateDailyReport() {
  // Generate and send daily report
  await sendReport('/orders -today', '#reports');
}

async function checkSystemHealth() {
  // Check database, API, etc.
  return { healthy: true, issues: [] };
}
```

---

## Testing Plugins

### Create a Test File

```javascript
// my-plugin/test.js
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { getPluginRegistry, resetPluginRegistry } from '@stateset/cli/src/channels/plugin-api.js';

describe('My Plugin', () => {
  beforeEach(async () => {
    await resetPluginRegistry();
  });
  
  it('should register hello command', async () => {
    const registry = getPluginRegistry();
    
    await registry.register('my-plugin', (api) => {
      api.registerCommand({
        name: 'hello',
        description: 'Test',
        handler: async () => ({ response: 'Hello!' })
      });
    });
    
    const plugins = registry.listPlugins();
    assert.equal(plugins.length, 1);
    assert.ok(plugins[0].commands.includes('hello'));
  });
  
  it('should respond to hello command', async () => {
    const result = await executeCommand('/hello');
    assert.equal(result.response, 'Hello!');
  });
});
```

### Run Tests

```bash
node --test my-plugin/test.js
```

---

## Best Practices

### 1. Error Handling

Always handle errors gracefully:

```javascript
api.registerCommand({
  name: 'risky-operation',
  handler: async () => {
    try {
      const result = await riskyOperation();
      return { response: 'Success!' };
    } catch (err) {
      console.error('[my-plugin] Error:', err);
      return { 
        response: `Error: ${err.message}`,
        error: true
      };
    }
  }
});
```

### 2. Priority Ordering

Use appropriate priorities for hooks:

```javascript
// Low priority = runs first
api.on('before_agent_start', earlyHook, { priority: 10 });

// Default priority
api.on('before_agent_start', normalHook, { priority: 100 });

// High priority = runs last
api.on('before_agent_start', lateHook, { priority: 200 });
```

### 3. Resource Cleanup

Always clean up resources in service stop:

```javascript
api.registerService({
  name: 'my-service',
  
  start: async () => {
    this._timer = setInterval(task, 1000);
    this._connections = new Set();
  },
  
  stop: async () => {
    if (this._timer) clearInterval(this._timer);
    if (this._connections) {
      for (const conn of this._connections) {
        await conn.close();
      }
    }
  }
});
```

### 4. Configuration Validation

Always validate config before use:

```javascript
export default function init(api, { config }) {
  if (!config.apiKey) {
    console.warn('[my-plugin] No API key configured, plugin disabled');
    return;
  }
  
  if (config.timeout < 1000) {
    console.warn('[my-plugin] Timeout too low, using minimum of 1000ms');
    config.timeout = 1000;
  }
  
  // Continue init...
}
```

### 5. Hook Types

Use parallel hooks for logging/notifications (don't block):

```javascript
api.on('message_received', logMessage, { priority: 100 });
```

Use sequential hooks when you need to modify data:

```javascript
api.on('before_agent_start', augmentContext, { priority: 50 });
```

---

## Plugin Locations

Plugins can be loaded from multiple locations:

| Location | Path | Priority |
|----------|------|----------|
| Bundled | `cli/src/plugins/` | 1 (highest) |
| Global | `~/.stateset/plugins/` | 2 |
| Workspace | `.stateset/plugins/` | 3 |
| Config | Declared in config | 4 (lowest) |

---

## Advanced Topics

### Plugin Slots

Plugins can define and fulfill "slots" (dependency injection):

```javascript
// Define a slot
function init(api) {
  const slots = getPluginSlots();
  slots.defineSlot('database', {
    description: 'Database provider',
    required: false
  });
  
  // Try to get the slot
  const db = slots.getSlot('database');
  if (db) {
    // Use the database provider
  }
}
```

### CLI Extensions

Add CLI commands for your plugin:

```javascript
import { getCliExtensions } from '@stateset/cli/src/channels/cli-extensions.js';

export default function init(api, { manifest }) {
  const cli = getCliExtensions();
  
  cli.register(manifest.id, {
    description: 'My plugin CLI commands',
    pluginId: manifest.id,
    commands: [
      {
        name: 'sync',
        description: 'Sync external data',
        options: [
          { name: 'force', short: 'f', type: 'boolean', description: 'Force full sync' }
        ],
        handler: async (positional, { parsedOptions }) => {
          if (parsedOptions.force) {
            // Force sync
          }
          return { output: 'Sync complete', exitCode: 0 };
        }
      }
    ]
  });
}
```

Use: `stateset my-plugin sync --force`

---

## Summary

The StateSet plugin system provides a flexible way to extend the gateway:

- **Commands**: Add new bot commands
- **Hooks**: React to and modify message lifecycle
- **Services**: Run background tasks
- **Routes**: Add HTTP endpoints
- **Config**: Define and validate settings
- **CLI**: Add CLI commands for management

Combine these to build powerful integrations with external systems, add custom analytics, implement specialized workflows, and more!

For more examples, see:
- `cli/src/plugins/memory-vector/` - Vector memory plugin
- `cli/tests/plugin-system.test.js` - Plugin system tests

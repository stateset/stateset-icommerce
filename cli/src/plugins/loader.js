/**
 * Plugin System for StateSet CLI
 *
 * Allows users to extend the CLI with custom tools and commands
 * without modifying core code.
 *
 * Plugin locations:
 *   ~/.stateset/plugins/*.js
 *   ./plugins/*.js (project-local)
 *
 * Plugin format:
 *   export default {
 *     name: 'my_tool',
 *     description: 'My custom tool',
 *     inputSchema: { ... },  // Zod schema or JSON Schema
 *     handler: async (input, ctx) => { ... }
 *   };
 */

import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';
import { z } from 'zod';
import { createLogger } from '../logger.js';

const logger = createLogger({ context: { module: 'plugins' } });

/**
 * Plugin schema for validation
 */
const PluginSchema = z.object({
  name: z.string().min(1).regex(/^[a-z][a-z0-9_]*$/, 'Name must be lowercase with underscores'),
  description: z.string().min(1),
  version: z.string().optional().default('1.0.0'),
  inputSchema: z.record(z.any()).optional().default({}),
  handler: z.function(),
  // Optional lifecycle hooks
  onLoad: z.function().optional(),
  onUnload: z.function().optional(),
  // Optional metadata
  author: z.string().optional(),
  tags: z.array(z.string()).optional()
});

/**
 * PluginLoader - Discovers and loads plugins from configured directories
 */
export class PluginLoader {
  constructor(options = {}) {
    this.plugins = new Map();
    this.pluginDirs = options.pluginDirs ?? this._getDefaultPluginDirs();
    this.context = options.context ?? {};
  }

  /**
   * Get default plugin directories
   */
  _getDefaultPluginDirs() {
    const dirs = [];

    // User plugins directory
    const userDir = path.join(os.homedir(), '.stateset', 'plugins');
    if (fs.existsSync(userDir)) {
      dirs.push(userDir);
    }

    // Project-local plugins
    const localDir = path.join(process.cwd(), 'plugins');
    if (fs.existsSync(localDir)) {
      dirs.push(localDir);
    }

    // CLI package plugins (for bundled extensions)
    const packageDir = path.join(import.meta.dirname || '.', '..', 'plugins');
    if (fs.existsSync(packageDir)) {
      dirs.push(packageDir);
    }

    return dirs;
  }

  /**
   * Discover all plugins in configured directories
   */
  async discover() {
    const discovered = [];

    for (const dir of this.pluginDirs) {
      try {
        const files = fs.readdirSync(dir);
        for (const file of files) {
          if (file.endsWith('.js') || file.endsWith('.mjs')) {
            discovered.push(path.join(dir, file));
          }
        }
      } catch (error) {
        logger.debug('Plugin directory not accessible', { dir, error: error.message });
      }
    }

    logger.info('Discovered plugins', { count: discovered.length });
    return discovered;
  }

  /**
   * Load a single plugin from file
   */
  async loadPlugin(filePath) {
    try {
      const module = await import(filePath);
      const plugin = module.default || module;

      // Validate plugin structure
      const validated = PluginSchema.parse(plugin);

      // Check for name conflicts
      if (this.plugins.has(validated.name)) {
        logger.warn('Plugin name conflict, skipping', {
          name: validated.name,
          file: filePath
        });
        return null;
      }

      // Store with metadata
      const pluginEntry = {
        ...validated,
        filePath,
        loadedAt: new Date().toISOString()
      };

      // Call onLoad hook if present
      if (validated.onLoad) {
        await validated.onLoad(this.context);
      }

      this.plugins.set(validated.name, pluginEntry);
      logger.info('Plugin loaded', { name: validated.name, file: filePath });

      return pluginEntry;
    } catch (error) {
      logger.error('Failed to load plugin', {
        file: filePath,
        error: error.message
      });
      return null;
    }
  }

  /**
   * Load all discovered plugins
   */
  async loadAll() {
    const files = await this.discover();
    const results = [];

    for (const file of files) {
      const plugin = await this.loadPlugin(file);
      if (plugin) {
        results.push(plugin);
      }
    }

    return results;
  }

  /**
   * Unload a plugin by name
   */
  async unload(name) {
    const plugin = this.plugins.get(name);
    if (!plugin) return false;

    // Call onUnload hook if present
    if (plugin.onUnload) {
      await plugin.onUnload(this.context);
    }

    this.plugins.delete(name);
    logger.info('Plugin unloaded', { name });
    return true;
  }

  /**
   * Get a plugin by name
   */
  get(name) {
    return this.plugins.get(name);
  }

  /**
   * Get all loaded plugins
   */
  getAll() {
    return Array.from(this.plugins.values());
  }

  /**
   * Check if a plugin is loaded
   */
  has(name) {
    return this.plugins.has(name);
  }

  /**
   * Execute a plugin's handler
   */
  async execute(name, input) {
    const plugin = this.plugins.get(name);
    if (!plugin) {
      throw new Error(`Plugin '${name}' not found`);
    }

    logger.debug('Executing plugin', { name, input });

    try {
      const result = await plugin.handler(input, this.context);
      return result;
    } catch (error) {
      logger.error('Plugin execution failed', {
        name,
        error: error.message
      });
      throw error;
    }
  }

  /**
   * Convert plugins to MCP tool format
   */
  toMcpTools() {
    const tools = [];

    for (const plugin of this.plugins.values()) {
      tools.push({
        name: `plugin_${plugin.name}`,
        description: `[Plugin] ${plugin.description}`,
        inputSchema: plugin.inputSchema,
        handler: async (input) => {
          try {
            const result = await plugin.handler(input, this.context);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({ success: true, result }, null, 2)
              }]
            };
          } catch (error) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({ error: error.message })
              }]
            };
          }
        }
      });
    }

    return tools;
  }
}

/**
 * Create a plugin loader with default configuration
 */
export function createPluginLoader(options = {}) {
  return new PluginLoader(options);
}

/**
 * Plugin scaffold template
 */
export const PLUGIN_TEMPLATE = `/**
 * Example StateSet CLI Plugin
 *
 * Save this file to ~/.stateset/plugins/my-plugin.js
 */

export default {
  name: 'my_custom_tool',
  description: 'A custom tool that does something useful',
  version: '1.0.0',
  author: 'Your Name',
  tags: ['custom', 'example'],

  // Input schema (Zod-compatible)
  inputSchema: {
    message: {
      type: 'string',
      description: 'A message to process'
    },
    count: {
      type: 'number',
      description: 'Number of times to repeat',
      default: 1
    }
  },

  // Main handler function
  handler: async (input, ctx) => {
    const { message, count = 1 } = input;
    const result = Array(count).fill(message).join(' ');

    return {
      processed: result,
      length: result.length,
      timestamp: new Date().toISOString()
    };
  },

  // Optional: Called when plugin is loaded
  onLoad: async (ctx) => {
    console.log('My plugin loaded!');
  },

  // Optional: Called when plugin is unloaded
  onUnload: async (ctx) => {
    console.log('My plugin unloaded!');
  }
};
`;

/**
 * Create plugin scaffold in user's plugin directory
 */
export async function scaffoldPlugin(name) {
  const userPluginDir = path.join(os.homedir(), '.stateset', 'plugins');

  // Ensure directory exists
  fs.mkdirSync(userPluginDir, { recursive: true });

  const filePath = path.join(userPluginDir, `${name}.js`);

  if (fs.existsSync(filePath)) {
    throw new Error(`Plugin file already exists: ${filePath}`);
  }

  const content = PLUGIN_TEMPLATE.replace(/my_custom_tool/g, name)
    .replace(/my-plugin/g, name);

  fs.writeFileSync(filePath, content);

  return filePath;
}

export default PluginLoader;

/**
 * CLI Extension API for StateSet iCommerce Plugins
 *
 * Allows plugins to register namespaced CLI commands that integrate
 * with the stateset CLI. Commands are grouped under the plugin namespace.
 *
 * Inspired by moltbot's registerCli() plugin API.
 *
 * Usage:
 *   const cli = getCliExtensions();
 *   cli.register('memory', {
 *     description: 'Memory plugin commands',
 *     pluginId: 'redis-memory',
 *     commands: [
 *       { name: 'get', description: 'Get a memory value', handler: async (args) => ... },
 *       { name: 'search', description: 'Search memories', handler: async (args) => ... },
 *     ],
 *   });
 *
 *   // Invoke: stateset memory get <key>
 *   await cli.execute('memory', 'get', ['my-key']);
 */

// ============================================================================
// Types
// ============================================================================

/**
 * @typedef {Object} CliNamespaceDef
 * @property {string} namespace - Namespace name (e.g., 'memory', 'analytics')
 * @property {string} [description] - Namespace description
 * @property {string} pluginId - Owning plugin
 * @property {CliCommandDef[]} commands - Commands in this namespace
 */

/**
 * @typedef {Object} CliCommandDef
 * @property {string} name - Command name within namespace
 * @property {string} description - Help text
 * @property {string} [usage] - Usage pattern (e.g., '<key> [--format json]')
 * @property {CliOption[]} [options] - Command options/flags
 * @property {(args: string[], opts: CliContext) => Promise<CliResult>} handler
 */

/**
 * @typedef {Object} CliOption
 * @property {string} name - Option name (without dashes)
 * @property {string} [short] - Short flag (single char)
 * @property {string} description - Help text
 * @property {'string'|'boolean'|'number'} [type='string']
 * @property {*} [defaultValue]
 * @property {boolean} [required=false]
 */

/**
 * @typedef {Object} CliContext
 * @property {Object} parsedOptions - Parsed option values
 * @property {string} namespace
 * @property {string} command
 * @property {Object} [runtime] - Plugin runtime context
 */

/**
 * @typedef {Object} CliResult
 * @property {string} [output] - Text to display
 * @property {Object} [data] - Structured data (for --json output)
 * @property {number} [exitCode=0] - Process exit code
 */

// ============================================================================
// Option Parsing
// ============================================================================

/**
 * Parse command-line arguments with option definitions.
 *
 * @param {string[]} args - Raw arguments
 * @param {CliOption[]} [options=[]] - Option definitions
 * @returns {{ positional: string[], options: Object }}
 */
function parseArgs(args, options = []) {
  const positional = [];
  const parsed = {};

  // Set defaults
  for (const opt of options) {
    if (opt.defaultValue !== undefined) {
      parsed[opt.name] = opt.defaultValue;
    }
  }

  // Build lookup maps
  const longMap = new Map();
  const shortMap = new Map();
  for (const opt of options) {
    longMap.set(`--${opt.name}`, opt);
    if (opt.short) {
      shortMap.set(`-${opt.short}`, opt);
    }
  }

  let i = 0;
  while (i < args.length) {
    const arg = args[i];

    // Long option
    if (arg.startsWith('--')) {
      const eqIdx = arg.indexOf('=');
      const name = eqIdx > 0 ? arg.substring(0, eqIdx) : arg;
      const opt = longMap.get(name);

      if (opt) {
        if (opt.type === 'boolean') {
          parsed[opt.name] = eqIdx > 0 ? arg.substring(eqIdx + 1) !== 'false' : true;
        } else {
          const value = eqIdx > 0 ? arg.substring(eqIdx + 1) : args[++i];
          parsed[opt.name] = opt.type === 'number' ? Number(value) : value;
        }
      } else {
        // Unknown option, treat as boolean flag
        const flagName = name.substring(2);
        parsed[flagName] = true;
      }
    }
    // Short option
    else if (arg.startsWith('-') && arg.length === 2) {
      const opt = shortMap.get(arg);
      if (opt) {
        if (opt.type === 'boolean') {
          parsed[opt.name] = true;
        } else {
          parsed[opt.name] = opt.type === 'number' ? Number(args[++i]) : args[++i];
        }
      }
    }
    // Positional
    else {
      positional.push(arg);
    }

    i++;
  }

  return { positional, options: parsed };
}

// ============================================================================
// CliExtensionRegistry
// ============================================================================

export class CliExtensionRegistry {
  constructor() {
    /** @type {Map<string, CliNamespaceDef>} */
    this._namespaces = new Map();
  }

  /**
   * Register a CLI namespace with commands.
   *
   * @param {string} namespace
   * @param {Object} opts
   * @param {string} [opts.description]
   * @param {string} opts.pluginId
   * @param {CliCommandDef[]} opts.commands
   */
  register(namespace, opts) {
    if (this._namespaces.has(namespace)) {
      throw new Error(`CLI namespace "${namespace}" is already registered`);
    }

    if (!namespace || typeof namespace !== 'string' || !/^[a-z][a-z0-9-]*$/.test(namespace)) {
      throw new Error(`Invalid namespace "${namespace}": must match /^[a-z][a-z0-9-]*$/`);
    }

    if (!opts.pluginId) {
      throw new Error('Plugin ID is required');
    }

    if (!Array.isArray(opts.commands) || opts.commands.length === 0) {
      throw new Error('At least one command is required');
    }

    // Validate commands
    for (const cmd of opts.commands) {
      if (!cmd.name || !cmd.description || typeof cmd.handler !== 'function') {
        throw new Error(`Command must have name, description, and handler`);
      }
    }

    this._namespaces.set(namespace, {
      namespace,
      description: opts.description || '',
      pluginId: opts.pluginId,
      commands: opts.commands,
    });
  }

  /**
   * Unregister a namespace.
   *
   * @param {string} namespace
   * @returns {boolean}
   */
  unregister(namespace) {
    return this._namespaces.delete(namespace);
  }

  /**
   * Unregister all namespaces for a plugin.
   *
   * @param {string} pluginId
   * @returns {number}
   */
  unregisterPlugin(pluginId) {
    let count = 0;
    for (const [namespace, def] of this._namespaces) {
      if (def.pluginId === pluginId) {
        this._namespaces.delete(namespace);
        count++;
      }
    }
    return count;
  }

  /**
   * Execute a CLI command.
   *
   * @param {string} namespace
   * @param {string} commandName
   * @param {string[]} args - Raw arguments
   * @param {Object} [context] - Additional context (runtime, etc.)
   * @returns {Promise<CliResult>}
   */
  async execute(namespace, commandName, args = [], context = {}) {
    const nsDef = this._namespaces.get(namespace);
    if (!nsDef) {
      return { output: `Unknown namespace: ${namespace}`, exitCode: 1 };
    }

    const cmdDef = nsDef.commands.find((c) => c.name === commandName);
    if (!cmdDef) {
      // Show namespace help
      return { output: this.generateNamespaceHelp(namespace), exitCode: 1 };
    }

    const { positional, options } = parseArgs(args, cmdDef.options);

    // Validate required options
    if (cmdDef.options) {
      for (const opt of cmdDef.options) {
        if (opt.required && (options[opt.name] === undefined || options[opt.name] === null)) {
          return { output: `Missing required option: --${opt.name}`, exitCode: 1 };
        }
      }
    }

    try {
      const result = await cmdDef.handler(positional, {
        parsedOptions: options,
        namespace,
        command: commandName,
        ...context,
      });

      return result || { output: '', exitCode: 0 };
    } catch (err) {
      return { output: `Error: ${err.message}`, exitCode: 1 };
    }
  }

  /**
   * Check if a namespace is registered.
   *
   * @param {string} namespace
   * @returns {boolean}
   */
  has(namespace) {
    return this._namespaces.has(namespace);
  }

  /**
   * Check if a specific command exists in a namespace.
   *
   * @param {string} namespace
   * @param {string} commandName
   * @returns {boolean}
   */
  hasCommand(namespace, commandName) {
    const nsDef = this._namespaces.get(namespace);
    return nsDef ? nsDef.commands.some((c) => c.name === commandName) : false;
  }

  /**
   * List all registered namespaces.
   *
   * @returns {Array<{ namespace: string, description: string, pluginId: string, commandCount: number }>}
   */
  list() {
    return [...this._namespaces.values()].map((ns) => ({
      namespace: ns.namespace,
      description: ns.description,
      pluginId: ns.pluginId,
      commandCount: ns.commands.length,
    }));
  }

  /**
   * Generate help text for a specific namespace.
   *
   * @param {string} namespace
   * @returns {string}
   */
  generateNamespaceHelp(namespace) {
    const nsDef = this._namespaces.get(namespace);
    if (!nsDef) return `Unknown namespace: ${namespace}`;

    const lines = [`stateset ${namespace} - ${nsDef.description || nsDef.namespace}`, ''];
    lines.push('Commands:');

    for (const cmd of nsDef.commands) {
      const usage = cmd.usage ? ` ${cmd.usage}` : '';
      lines.push(`  stateset ${namespace} ${cmd.name}${usage}`);
      lines.push(`    ${cmd.description}`);

      if (cmd.options && cmd.options.length > 0) {
        for (const opt of cmd.options) {
          const short = opt.short ? `-${opt.short}, ` : '    ';
          const required = opt.required ? ' (required)' : '';
          const def = opt.defaultValue !== undefined ? ` [default: ${opt.defaultValue}]` : '';
          lines.push(`    ${short}--${opt.name}  ${opt.description}${required}${def}`);
        }
      }

      lines.push('');
    }

    return lines.join('\n').trim();
  }

  /**
   * Generate help text for all extensions.
   *
   * @returns {string}
   */
  generateHelp() {
    if (this._namespaces.size === 0) return '';

    const lines = ['', 'Plugin Extensions:'];

    for (const ns of [...this._namespaces.values()].sort((a, b) =>
      a.namespace.localeCompare(b.namespace),
    )) {
      const desc = ns.description ? ` - ${ns.description}` : '';
      lines.push(`  stateset ${ns.namespace}${desc}`);

      for (const cmd of ns.commands) {
        lines.push(`    ${ns.namespace} ${cmd.name} - ${cmd.description}`);
      }
    }

    return lines.join('\n');
  }

  /**
   * Clear all registrations.
   */
  clear() {
    this._namespaces.clear();
  }
}

// ============================================================================
// Singleton
// ============================================================================

let _instance = null;

/**
 * Get the global CliExtensionRegistry instance.
 *
 * @returns {CliExtensionRegistry}
 */
export function getCliExtensions() {
  if (!_instance) {
    _instance = new CliExtensionRegistry();
  }
  return _instance;
}

/**
 * Reset the singleton (for testing).
 */
export function resetCliExtensions() {
  if (_instance) {
    _instance.clear();
  }
  _instance = null;
}

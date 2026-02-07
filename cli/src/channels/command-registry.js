/**
 * Dynamic Command Registry for StateSet Channel Gateways
 *
 * Enables plugins and modules to register custom bot commands at runtime.
 * Commands are validated and can be listed for /help generation.
 *
 * Features:
 * - Multiple aliases per command
 * - Channel-native command name overrides
 * - Text alias normalization and lookup
 *
 * Singleton: use getCommandRegistry() to access the global instance.
 */

// ============================================================================
// Constants
// ============================================================================

/** Reserved command names that cannot be overridden by plugins */
const RESERVED_COMMANDS = new Set([
  'help',
  'reset',
  'new',
  'status',
  'orders',
  'order',
  'inventory',
  'cart',
  'track',
  'customers',
  'analytics',
  'whoami',
  'link',
  'unlink',
  'stats',
  'escalate',
  'release',
]);

/** Pattern for valid command names */
const NAME_PATTERN = /^[a-z][a-z0-9_-]*$/;

// ============================================================================
// CommandRegistry
// ============================================================================

/**
 * @typedef {Object} CommandDefinition
 * @property {string} name - Primary command name (without leading /)
 * @property {string[]} [aliases] - Alternative names for the command
 * @property {string} description - Help text for the command
 * @property {boolean} [acceptsArgs=true] - Whether command accepts arguments
 * @property {Function} handler - (argText, context) => Promise<{ response, richMessage? }>
 * @property {string} [source] - Source identifier (e.g., 'plugin:my-plugin', 'autonomous')
 * @property {Object} [channelNames] - Channel-specific command names (e.g., { discord: 'speak' })
 * @property {string} [category] - Command category for grouping in help
 * @property {boolean} [hidden=false] - Hide from /help output
 */

/**
 * @typedef {Object} CommandContext
 * @property {string} senderId - Sender identifier
 * @property {string} channel - Channel name
 * @property {Object} session - User session
 * @property {boolean} allowApply - Whether write operations are enabled
 * @property {Object} [commerce] - Commerce instance
 * @property {Object} [identityStore] - Identity store
 * @property {Object} [autonomousEngine] - Autonomous engine instance
 */

export class CommandRegistry {
  constructor() {
    /** @type {Map<string, CommandDefinition>} */
    this._commands = new Map();

    /** @type {Map<string, string>} - Maps aliases to primary command name */
    this._aliasIndex = new Map();

    /** @type {Map<string, Map<string, string>>} - Maps channel -> nativeName -> primaryName */
    this._channelIndex = new Map();

    /** @type {boolean} - Lock to prevent modifications during execution */
    this._locked = false;
  }

  /**
   * Normalize a command name for lookup.
   * @private
   */
  _normalize(name) {
    return name.toLowerCase().replace(/^\//, '').trim();
  }

  /**
   * Validate a command name.
   * @private
   */
  _validateName(name) {
    if (!name || typeof name !== 'string') {
      throw new Error('Command name is required and must be a string');
    }

    const normalized = this._normalize(name);

    if (!NAME_PATTERN.test(normalized)) {
      throw new Error(
        `Invalid command name "${normalized}": must match pattern ${NAME_PATTERN} (lowercase letters, numbers, hyphens, underscores)`,
      );
    }

    if (RESERVED_COMMANDS.has(normalized)) {
      throw new Error(`Command name "${normalized}" is reserved and cannot be registered`);
    }

    return normalized;
  }

  /**
   * Register a new command.
   *
   * @param {CommandDefinition} definition
   * @throws {Error} If validation fails or registry is locked
   */
  register(definition) {
    if (this._locked) {
      throw new Error('Cannot register commands while registry is locked during execution');
    }

    const {
      name,
      aliases = [],
      description,
      handler,
      source,
      acceptsArgs = true,
      channelNames = {},
      category = 'general',
      hidden = false,
    } = definition;

    // Validate primary name
    const normalizedName = this._validateName(name);

    // Check duplicates for primary name
    if (this._commands.has(normalizedName)) {
      throw new Error(`Command "${normalizedName}" is already registered`);
    }

    // Check if primary name conflicts with existing alias
    if (this._aliasIndex.has(normalizedName)) {
      throw new Error(`Command name "${normalizedName}" conflicts with existing alias`);
    }

    // Validate and check aliases
    const normalizedAliases = [];
    for (const alias of aliases) {
      const normalizedAlias = this._normalize(alias);

      if (!NAME_PATTERN.test(normalizedAlias)) {
        throw new Error(`Invalid alias "${normalizedAlias}": must match pattern ${NAME_PATTERN}`);
      }

      if (RESERVED_COMMANDS.has(normalizedAlias)) {
        throw new Error(`Alias "${normalizedAlias}" is reserved`);
      }

      if (this._commands.has(normalizedAlias) || this._aliasIndex.has(normalizedAlias)) {
        throw new Error(`Alias "${normalizedAlias}" conflicts with existing command or alias`);
      }

      normalizedAliases.push(normalizedAlias);
    }

    // Validate handler
    if (typeof handler !== 'function') {
      throw new Error('Command handler must be a function');
    }

    // Validate description
    if (!description || typeof description !== 'string') {
      throw new Error('Command description is required and must be a string');
    }

    // Store command
    const fullDef = {
      name: normalizedName,
      aliases: normalizedAliases,
      description,
      acceptsArgs,
      handler,
      source: source || 'unknown',
      channelNames,
      category,
      hidden,
    };

    this._commands.set(normalizedName, fullDef);

    // Index aliases
    for (const alias of normalizedAliases) {
      this._aliasIndex.set(alias, normalizedName);
    }

    // Index channel-specific names
    for (const [channel, nativeName] of Object.entries(channelNames)) {
      const normalizedNative = this._normalize(nativeName);
      if (!this._channelIndex.has(channel)) {
        this._channelIndex.set(channel, new Map());
      }
      this._channelIndex.get(channel).set(normalizedNative, normalizedName);
    }
  }

  /**
   * Unregister a command and all its aliases.
   *
   * @param {string} name - Primary name or alias
   * @returns {boolean} - True if command was removed
   */
  unregister(name) {
    if (this._locked) {
      throw new Error('Cannot unregister commands while registry is locked');
    }

    const normalizedName = this._normalize(name);

    // Resolve alias to primary name
    const primaryName = this._aliasIndex.get(normalizedName) || normalizedName;

    const def = this._commands.get(primaryName);
    if (!def) return false;

    // Remove all aliases
    for (const alias of def.aliases || []) {
      this._aliasIndex.delete(alias);
    }

    // Remove channel-specific names
    for (const [channel, nativeName] of Object.entries(def.channelNames || {})) {
      const channelMap = this._channelIndex.get(channel);
      if (channelMap) {
        channelMap.delete(this._normalize(nativeName));
      }
    }

    // Remove command
    return this._commands.delete(primaryName);
  }

  /**
   * Get a command definition by name, alias, or channel-native name.
   *
   * @param {string} name - Command name to look up
   * @param {string} [channel] - Channel for native name resolution
   * @returns {CommandDefinition|null}
   */
  get(name, channel) {
    const normalizedName = this._normalize(name);

    // 1. Check channel-specific names first
    if (channel) {
      const channelMap = this._channelIndex.get(channel);
      if (channelMap) {
        const primaryName = channelMap.get(normalizedName);
        if (primaryName) {
          return this._commands.get(primaryName) || null;
        }
      }
    }

    // 2. Check primary command name
    const directMatch = this._commands.get(normalizedName);
    if (directMatch) return directMatch;

    // 3. Check aliases
    const aliasedPrimary = this._aliasIndex.get(normalizedName);
    if (aliasedPrimary) {
      return this._commands.get(aliasedPrimary) || null;
    }

    return null;
  }

  /**
   * Check if a command is registered (by name, alias, or channel-native name).
   *
   * @param {string} name
   * @param {string} [channel]
   * @returns {boolean}
   */
  has(name, channel) {
    return this.get(name, channel) !== null;
  }

  /**
   * Resolve a command name to its canonical (primary) name.
   *
   * @param {string} name - Name, alias, or channel-native name
   * @param {string} [channel] - Channel for native name resolution
   * @returns {string|null} - Primary name or null if not found
   */
  resolve(name, channel) {
    const def = this.get(name, channel);
    return def ? def.name : null;
  }

  /**
   * List all registered commands.
   *
   * @param {Object} [opts]
   * @param {boolean} [opts.includeHidden=false] - Include hidden commands
   * @param {string} [opts.category] - Filter by category
   * @returns {CommandDefinition[]}
   */
  list({ includeHidden = false, category } = {}) {
    let commands = [...this._commands.values()];

    if (!includeHidden) {
      commands = commands.filter((cmd) => !cmd.hidden);
    }

    if (category) {
      commands = commands.filter((cmd) => cmd.category === category);
    }

    return commands;
  }

  /**
   * List commands filtered by source.
   *
   * @param {string} source
   * @returns {CommandDefinition[]}
   */
  listBySource(source) {
    return [...this._commands.values()].filter((cmd) => cmd.source === source);
  }

  /**
   * Get all categories.
   *
   * @returns {string[]}
   */
  getCategories() {
    const categories = new Set();
    for (const cmd of this._commands.values()) {
      categories.add(cmd.category || 'general');
    }
    return [...categories].sort();
  }

  /**
   * Generate formatted help text for all dynamic commands.
   *
   * @param {Object} [opts]
   * @param {string} [opts.channel] - Include channel-specific names
   * @param {boolean} [opts.grouped=false] - Group by category
   * @returns {string}
   */
  generateHelp({ channel, grouped = false } = {}) {
    const commands = this.list();
    if (commands.length === 0) return '';

    if (grouped) {
      return this._generateGroupedHelp(commands, channel);
    }

    const lines = ['', 'Plugin Commands:'];
    for (const cmd of commands) {
      const aliases = this._formatAliases(cmd, channel);
      const args = cmd.acceptsArgs ? ' [args]' : '';
      lines.push(`/${cmd.name}${args} - ${cmd.description}${aliases}`);
    }

    return lines.join('\n');
  }

  /**
   * Generate grouped help text.
   * @private
   */
  _generateGroupedHelp(commands, channel) {
    const categories = new Map();

    for (const cmd of commands) {
      const cat = cmd.category || 'general';
      if (!categories.has(cat)) {
        categories.set(cat, []);
      }
      categories.get(cat).push(cmd);
    }

    const lines = ['', 'Plugin Commands:'];

    for (const [category, cmds] of [...categories.entries()].sort()) {
      lines.push(`\n  ${category.charAt(0).toUpperCase() + category.slice(1)}:`);
      for (const cmd of cmds) {
        const aliases = this._formatAliases(cmd, channel);
        const args = cmd.acceptsArgs ? ' [args]' : '';
        lines.push(`  /${cmd.name}${args} - ${cmd.description}${aliases}`);
      }
    }

    return lines.join('\n');
  }

  /**
   * Format aliases for help display.
   * @private
   */
  _formatAliases(cmd, channel) {
    const parts = [];

    if (cmd.aliases && cmd.aliases.length > 0) {
      parts.push(`aliases: ${cmd.aliases.map((a) => '/' + a).join(', ')}`);
    }

    if (channel && cmd.channelNames && cmd.channelNames[channel]) {
      parts.push(`${channel}: /${cmd.channelNames[channel]}`);
    }

    return parts.length > 0 ? ` (${parts.join('; ')})` : '';
  }

  /**
   * Lock the registry to prevent modifications during execution.
   */
  lock() {
    this._locked = true;
  }

  /**
   * Unlock the registry.
   */
  unlock() {
    this._locked = false;
  }

  /**
   * Execute a command handler with locking.
   *
   * @param {string} name
   * @param {string} argText
   * @param {CommandContext} context
   * @returns {Promise<{ response: string, richMessage?: any }>}
   */
  async execute(name, argText, context) {
    const def = this.get(name, context.channel);
    if (!def) {
      throw new Error(`Command "${name}" not found`);
    }

    this.lock();
    try {
      return await def.handler(argText, context);
    } finally {
      this.unlock();
    }
  }

  /**
   * Clear all registered commands.
   * Useful for testing or full reset.
   */
  clear() {
    this._commands.clear();
    this._aliasIndex.clear();
    this._channelIndex.clear();
    this._locked = false;
  }

  /**
   * Get statistics about registered commands.
   *
   * @returns {{ commands: number, aliases: number, channels: string[] }}
   */
  getStats() {
    return {
      total: this._commands.size,
      commands: this._commands.size,
      aliases: this._aliasIndex.size,
      channels: [...this._channelIndex.keys()],
    };
  }
}

// ============================================================================
// Singleton
// ============================================================================

let _instance = null;

/**
 * Get the global CommandRegistry instance.
 *
 * @returns {CommandRegistry}
 */
export function getCommandRegistry() {
  if (!_instance) {
    _instance = new CommandRegistry();
  }
  return _instance;
}

/**
 * Reset the singleton instance (for testing).
 */
export function resetCommandRegistry() {
  if (_instance) {
    _instance.clear();
  }
  _instance = null;
}

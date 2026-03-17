/**
 * Shared CLI schema for the main `stateset` command.
 *
 * This is the source of truth for:
 * - parseArgs option wiring
 * - shell completion option lists
 */

export const MAIN_CLI_OPTIONS = [
  {
    key: 'db',
    flag: '--db',
    type: 'string',
    description: 'Database path',
    valueName: 'path',
    valueType: 'path',
  },
  { key: 'apply', flag: '--apply', type: 'boolean', default: false, description: 'Enable writes' },
  {
    key: 'agent',
    flag: '--agent',
    type: 'string',
    description: 'Use specific agent',
    valueName: 'agent',
    values: [
      'customer-service',
      'checkout',
      'orders',
      'inventory',
      'returns',
      'analytics',
      'promotions',
      'subscriptions',
      'manufacturing',
      'shipments',
      'suppliers',
      'invoices',
      'warranties',
      'currency',
      'tax',
      'payments',
      'stablecoin',
      'sync',
      'storefront',
    ],
  },
  {
    key: 'profile',
    flag: '--profile',
    type: 'string',
    short: 'p',
    description: 'Use profile',
    valueName: 'profile',
  },
  { key: 'model', flag: '--model', type: 'string', description: 'AI model', valueName: 'model' },
  {
    key: 'provider',
    flag: '--provider',
    type: 'string',
    description: 'AI provider',
    valueName: 'provider',
    values: ['claude', 'openai', 'gemini', 'ollama'],
  },
  {
    key: 'think',
    flag: '--think',
    type: 'string',
    default: 'off',
    description: 'Extended thinking',
    valueName: 'level',
    values: ['off', 'low', 'medium', 'high'],
  },
  {
    key: 'stream',
    flag: '--stream',
    type: 'boolean',
    default: false,
    description: 'Stream output',
  },
  {
    key: 'budget',
    flag: '--budget',
    type: 'string',
    description: 'Max spend (USD)',
    valueName: 'usd',
  },
  {
    key: 'memory',
    flag: '--memory',
    type: 'boolean',
    default: false,
    description: 'Enable memory',
  },
  {
    key: 'noMemory',
    flag: '--no-memory',
    type: 'boolean',
    default: false,
    description: 'Disable memory',
  },
  {
    key: 'x402',
    flag: '--x402',
    type: 'boolean',
    default: false,
    description: 'Enable x402 tools',
  },
  {
    key: 'treasury',
    flag: '--treasury',
    type: 'boolean',
    default: false,
    description: 'Enable treasury billing',
  },
  {
    key: 'treasuryChain',
    flag: '--treasury-chain',
    type: 'string',
    description: 'Treasury chain',
    valueName: 'chain',
  },
  {
    key: 'treasuryToken',
    flag: '--treasury-token',
    type: 'string',
    description: 'Treasury token',
    valueName: 'symbol',
  },
  {
    key: 'treasuryAgent',
    flag: '--treasury-agent',
    type: 'string',
    description: 'Treasury agent id',
    valueName: 'id',
  },
  {
    key: 'treasuryDb',
    flag: '--treasury-db',
    type: 'string',
    description: 'Treasury DB path',
    valueName: 'path',
    valueType: 'path',
  },
  {
    key: 'treasuryErc8004Registry',
    flag: '--treasury-erc8004-registry',
    type: 'string',
    description: 'ERC-8004 registry URI',
    valueName: 'uri',
  },
  {
    key: 'treasuryErc8004Db',
    flag: '--treasury-erc8004-db',
    type: 'string',
    description: 'ERC-8004 DB path',
    valueName: 'path',
    valueType: 'path',
  },
  {
    key: 'resume',
    flag: '--resume',
    type: 'string',
    description: 'Resume session',
    valueName: 'session',
  },
  {
    key: 'queueStatus',
    flag: '--queue-status',
    type: 'boolean',
    default: false,
    description: 'Show queue status',
  },
  {
    key: 'queueClear',
    flag: '--queue-clear',
    type: 'boolean',
    default: false,
    description: 'Clear queue lanes',
  },
  {
    key: 'queueLane',
    flag: '--queue-lane',
    type: 'string',
    description: 'Queue lane id',
    valueName: 'lane',
  },
  {
    key: 'queueForce',
    flag: '--queue-force',
    type: 'boolean',
    default: false,
    description: 'Force queue operation',
  },
  {
    key: 'queueAdmin',
    flag: '--queue-admin',
    type: 'boolean',
    default: false,
    description: 'Acknowledge admin queue ops',
  },
  { key: 'json', flag: '--json', type: 'boolean', default: false, description: 'JSON output' },
  {
    key: 'format',
    flag: '--format',
    type: 'string',
    default: 'table',
    description: 'Output format',
    valueName: 'format',
    values: ['table', 'json', 'csv', 'yaml'],
  },
  {
    key: 'output',
    flag: '--output',
    type: 'string',
    description: 'Write output to file',
    valueName: 'file',
    valueType: 'path',
  },
  {
    key: 'verbose',
    flag: '--verbose',
    type: 'boolean',
    short: 'V',
    default: false,
    description: 'Verbose output',
  },
  { key: 'stats', flag: '--stats', type: 'boolean', default: false, description: 'Show stats' },
  {
    key: 'yes',
    flag: '--yes',
    type: 'boolean',
    short: 'y',
    default: false,
    description: 'Skip confirmation prompts',
  },
  {
    key: 'quiet',
    flag: '--quiet',
    type: 'boolean',
    short: 'q',
    default: false,
    description: 'Quiet output',
  },
  {
    key: 'color',
    flag: '--no-color',
    type: 'boolean',
    default: false,
    negated: true,
    description: 'Disable color',
  },
  {
    key: 'stdin',
    flag: '--stdin',
    type: 'boolean',
    default: false,
    description: 'Read requests from stdin',
  },
  {
    key: 'batch',
    flag: '--batch',
    type: 'string',
    description: 'Read requests from file',
    valueName: 'file',
    valueType: 'path',
  },
  {
    key: 'parallel',
    flag: '--parallel',
    type: 'string',
    description: 'Parallel requests',
    valueName: 'count',
  },
  {
    key: 'timeout',
    flag: '--timeout',
    type: 'string',
    description: 'Request timeout (ms)',
    valueName: 'ms',
  },
  {
    key: 'update',
    flag: '--update',
    type: 'boolean',
    default: false,
    description: 'Run update workflow',
  },
  {
    key: 'help',
    flag: '--help',
    type: 'boolean',
    short: 'h',
    default: false,
    description: 'Show help',
  },
  {
    key: 'version',
    flag: '--version',
    type: 'boolean',
    short: 'v',
    default: false,
    description: 'Show version',
  },
];

function longOptionName(flag) {
  return String(flag).replace(/^--/, '');
}

/**
 * parseArgs-compatible options object.
 */
export function getMainCliParseOptions() {
  return MAIN_CLI_OPTIONS.reduce((options, def) => {
    const entry = { type: def.type };
    if (def.short) entry.short = def.short;
    if (Object.prototype.hasOwnProperty.call(def, 'default')) {
      entry.default = def.default;
    }
    options[longOptionName(def.flag)] = entry;
    return options;
  }, {});
}

/**
 * Normalize parseArgs values into the schema's internal key names.
 */
export function normalizeMainCliValues(rawValues = {}) {
  const normalized = {};
  for (const def of MAIN_CLI_OPTIONS) {
    const longName = longOptionName(def.flag);
    if (Object.prototype.hasOwnProperty.call(rawValues, longName)) {
      normalized[def.key] = def.negated ? !rawValues[longName] : rawValues[longName];
      continue;
    }
    if (Object.prototype.hasOwnProperty.call(rawValues, def.key)) {
      normalized[def.key] = def.negated ? !rawValues[def.key] : rawValues[def.key];
      continue;
    }
    if (Object.prototype.hasOwnProperty.call(def, 'default')) {
      normalized[def.key] = def.negated ? !def.default : def.default;
    }
  }
  return normalized;
}

/**
 * Long-form flags for completion rendering.
 */
export const MAIN_CLI_LONG_FLAGS = MAIN_CLI_OPTIONS.map((def) => def.flag);

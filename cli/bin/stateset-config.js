#!/usr/bin/env node

/**
 * StateSet iCommerce CLI - Configuration Management
 *
 * Usage:
 *   stateset-config list                    List all profiles
 *   stateset-config show [profile]          Show profile settings
 *   stateset-config set <key> <value>       Set a config value
 *   stateset-config create <profile>        Create new profile
 *   stateset-config use <profile>           Set default profile
 */

import { parseArgs } from 'node:util';
import { RichOutput, ICONS } from '../src/output.js';
import { CLI_VERSION, DEFAULT_MODEL } from '../src/config.js';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';

const CONFIG_DIR = path.join(os.homedir(), '.stateset');
const CONFIG_FILE = path.join(CONFIG_DIR, 'config.json');
const PROFILES_DIR = path.join(CONFIG_DIR, 'profiles');
const ENV_FILE = path.join(CONFIG_DIR, '.env');
const CONFIG_RECOVERY_WARNINGS = [];
const PROFILE_NAME_PATTERN = /^[A-Za-z0-9._-]+$/;

const KNOWN_CONFIG_KEYS = {
  db: 'string',
  model: 'string',
  provider: 'string',
  apply: 'boolean',
  verbose: 'boolean',
  memory: 'boolean',
  stream: 'boolean',
  think: 'string',
  format: 'string',
  budget: 'string',
};

// Supported API key providers
const API_KEY_PROVIDERS = {
  anthropic: {
    name: 'Anthropic (Claude)',
    envVar: 'ANTHROPIC_API_KEY',
    prefix: 'sk-ant-',
    getKeyUrl: 'https://console.anthropic.com/settings/keys',
    description: 'Required for AI-powered commands',
  },
  openai: {
    name: 'OpenAI',
    envVar: 'OPENAI_API_KEY',
    prefix: 'sk-',
    getKeyUrl: 'https://platform.openai.com/api-keys',
    description: 'Optional - for OpenAI models and vector search',
  },
  gemini: {
    name: 'Google Gemini',
    envVar: 'GEMINI_API_KEY',
    prefix: '',
    getKeyUrl: 'https://aistudio.google.com/app/apikey',
    description: 'Optional - for Gemini models',
  },
};

const HELP = `
StateSet iCommerce CLI - Configuration Management

USAGE:
  stateset-config <command> [options]

COMMANDS:
  set-key [provider]      Set up API key (interactive) - START HERE!
  show-keys               Show configured API keys (masked)
  list                    List all profiles
  show [profile]          Show profile settings (default: current)
  create <profile>        Create a new profile
  use <profile>           Set the default profile
  set <key> <value>       Set a config value in current profile
  get <key>               Get a config value
  path                    Show config file location

OPTIONS:
  --profile, -p <name>    Target a specific profile
  --force                 Allow unknown config keys (advanced)
  --json                  Output as JSON
  --output <file>         Write output to file (implies --json)
  --help, -h              Show this help message

API KEY SETUP (Quick Start):
  # Set up your Anthropic API key (required for AI mode)
  stateset-config set-key anthropic

  # Or set environment variable directly
  export ANTHROPIC_API_KEY="sk-ant-api03-..."

  # Get your API key from: https://console.anthropic.com/

CONFIG KEYS:
  db                      Database path
  model                   Default Claude model
  apply                   Default apply mode (true/false)
  verbose                 Default verbose mode (true/false)

EXAMPLES:
  # First-time setup - configure API key
  stateset-config set-key anthropic

  # Create and switch to a production profile
  stateset-config create production
  stateset-config set db /var/data/stateset/production.db
  stateset-config set model claude-sonnet-4
  stateset-config use production

  # Use a profile temporarily
  stateset --profile production "list orders"

  # View current config
  stateset-config show
`;

function addRecoveryWarning(message) {
  CONFIG_RECOVERY_WARNINGS.push(message);
}

function secureWriteFile(filePath, content, mode = 0o600) {
  const tempPath = `${filePath}.tmp-${process.pid}-${Date.now()}`;
  fs.writeFileSync(tempPath, content, { mode });
  fs.renameSync(tempPath, filePath);
  try {
    fs.chmodSync(filePath, mode);
  } catch {
    // Best-effort permission hardening.
  }
}

function backupCorruptFile(filePath) {
  const stamp = new Date().toISOString().replace(/[:.]/g, '-');
  const backupPath = `${filePath}.corrupt-${stamp}`;
  fs.renameSync(filePath, backupPath);
  return backupPath;
}

function parseJsonFileSafely(filePath, fallbackFactory) {
  if (!fs.existsSync(filePath)) {
    return fallbackFactory();
  }

  try {
    return JSON.parse(fs.readFileSync(filePath, 'utf-8'));
  } catch {
    try {
      const backupPath = backupCorruptFile(filePath);
      addRecoveryWarning(
        `Recovered from invalid JSON in ${filePath}. Original file moved to ${backupPath}.`,
      );
    } catch (backupError) {
      addRecoveryWarning(
        `Recovered from invalid JSON in ${filePath}, but failed to create backup: ${backupError.message}`,
      );
    }
    return fallbackFactory();
  }
}

function escapeEnvValue(value) {
  return String(value).replace(/\\/g, '\\\\').replace(/"/g, '\\"');
}

// Ensure config directory exists
function ensureConfigDir() {
  if (!fs.existsSync(CONFIG_DIR)) {
    fs.mkdirSync(CONFIG_DIR, { recursive: true });
  }
  try {
    fs.chmodSync(CONFIG_DIR, 0o700);
  } catch {
    // Best-effort permission hardening.
  }
  if (!fs.existsSync(PROFILES_DIR)) {
    fs.mkdirSync(PROFILES_DIR, { recursive: true });
  }
  try {
    fs.chmodSync(PROFILES_DIR, 0o700);
  } catch {
    // Best-effort permission hardening.
  }
}

// Load main config
function loadConfig() {
  ensureConfigDir();
  return parseJsonFileSafely(CONFIG_FILE, () => ({
    defaultProfile: 'default',
    version: CLI_VERSION,
  }));
}

// Save main config
function saveConfig(config) {
  ensureConfigDir();
  secureWriteFile(CONFIG_FILE, JSON.stringify(config, null, 2), 0o600);
}

function validateProfileName(name) {
  if (!name || !PROFILE_NAME_PATTERN.test(name)) {
    throw new Error(
      `Invalid profile name '${name}'. Use only letters, numbers, dots, underscores, and dashes.`,
    );
  }
  return name;
}

function getProfilePath(name) {
  return path.join(PROFILES_DIR, `${validateProfileName(name)}.json`);
}

function createDefaultProfile(name) {
  return {
    name,
    db: './store.db',
    model: DEFAULT_MODEL,
    apply: false,
    verbose: false,
    created: new Date().toISOString(),
  };
}

// Load a profile
function loadProfile(name, { allowMissing = false } = {}) {
  const profilePath = getProfilePath(name);
  if (!fs.existsSync(profilePath)) {
    if (!allowMissing) {
      throw new Error(`Profile '${name}' not found`);
    }
    return createDefaultProfile(name);
  }
  return parseJsonFileSafely(profilePath, () => createDefaultProfile(name));
}

// Save a profile
function saveProfile(name, profile) {
  ensureConfigDir();
  const profilePath = getProfilePath(name);
  profile.updated = new Date().toISOString();
  secureWriteFile(profilePath, JSON.stringify(profile, null, 2), 0o600);
}

function profileExists(name) {
  try {
    return fs.existsSync(getProfilePath(name));
  } catch {
    return false;
  }
}

// List all profiles
function listProfiles() {
  ensureConfigDir();
  if (!fs.existsSync(PROFILES_DIR)) {
    return [];
  }
  return fs
    .readdirSync(PROFILES_DIR)
    .filter((f) => f.endsWith('.json'))
    .map((f) => f.replace('.json', ''));
}

// Load .env file
function loadEnvFile() {
  if (!fs.existsSync(ENV_FILE)) {
    return {};
  }
  const content = fs.readFileSync(ENV_FILE, 'utf-8');
  const env = {};
  for (const line of content.split('\n')) {
    const trimmed = line.trim();
    if (trimmed && !trimmed.startsWith('#')) {
      const [key, ...valueParts] = trimmed.split('=');
      if (key && valueParts.length > 0) {
        let value = valueParts.join('=');
        let normalizedKey = key.trim();
        if (normalizedKey.startsWith('export ')) {
          normalizedKey = normalizedKey.slice('export '.length).trim();
        }
        // Remove surrounding quotes if present
        if (
          (value.startsWith('"') && value.endsWith('"')) ||
          (value.startsWith("'") && value.endsWith("'"))
        ) {
          value = value.slice(1, -1);
        }
        env[normalizedKey] = value;
      }
    }
  }
  return env;
}

// Save .env file
function saveEnvFile(env) {
  ensureConfigDir();
  const hasExisting = fs.existsSync(ENV_FILE);
  const lines = hasExisting
    ? fs.readFileSync(ENV_FILE, 'utf-8').split(/\r?\n/)
    : [
        '# StateSet CLI API Keys',
        '# Managed by stateset-config set-key',
        '# Add this to your shell profile: source ~/.stateset/.env',
        '',
      ];

  const next = [];
  const remaining = new Map(Object.entries(env));

  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('#')) {
      next.push(line);
      continue;
    }

    const eqIdx = line.indexOf('=');
    if (eqIdx < 0) {
      next.push(line);
      continue;
    }

    let key = line.slice(0, eqIdx).trim();
    if (key.startsWith('export ')) {
      key = key.slice('export '.length).trim();
    }

    if (!remaining.has(key)) {
      next.push(line);
      continue;
    }

    next.push(`${key}="${escapeEnvValue(remaining.get(key))}"`);
    remaining.delete(key);
  }

  for (const [key, value] of remaining.entries()) {
    if (next.length > 0 && next[next.length - 1].trim()) {
      next.push('');
    }
    next.push(`${key}="${escapeEnvValue(value)}"`);
  }

  secureWriteFile(ENV_FILE, `${next.join('\n').replace(/\n*$/, '')}\n`, 0o600);
}

// Mask API key for display
function maskApiKey(key) {
  if (!key || key.length < 12) return '***';
  return key.slice(0, 10) + '...' + key.slice(-4);
}

// Check if an API key is configured (in env or .env file)
function getApiKeyStatus(provider) {
  const config = API_KEY_PROVIDERS[provider];
  if (!config) return null;

  // Check environment variable first
  const envValue = process.env[config.envVar];
  if (envValue) {
    return { configured: true, source: 'environment', value: envValue };
  }

  // Check .env file
  const envFile = loadEnvFile();
  if (envFile[config.envVar]) {
    return { configured: true, source: '.env file', value: envFile[config.envVar] };
  }

  return { configured: false };
}

// Interactive API key setup
async function setApiKey(provider, output) {
  const providerConfig = API_KEY_PROVIDERS[provider];
  if (!providerConfig) {
    console.error(`Unknown provider: ${provider}`);
    console.error(`Available providers: ${Object.keys(API_KEY_PROVIDERS).join(', ')}`);
    process.exit(1);
  }

  // Try @clack for beautiful prompts, fall back to readline
  let ui = null;
  if (process.stdin.isTTY) {
    try {
      ui = await import('../src/ui.js');
    } catch {
      // @clack not available — use readline below
    }
  }

  const { theme: t } = await import('../src/theme.js');

  console.log(`\n${output.bold(`Set up ${providerConfig.name} API Key`)}`);
  console.log(output.dim(`${providerConfig.description}\n`));

  // Check current status
  const status = getApiKeyStatus(provider);
  if (status.configured) {
    console.log(output.yellow(`Current key: ${maskApiKey(status.value)} (from ${status.source})`));
  }

  console.log(`Get your API key from: ${output.cyan(providerConfig.getKeyUrl)}\n`);

  let apiKey = '';
  if (ui) {
    apiKey = await ui.password(`Enter your ${providerConfig.name} API key`);
  } else {
    // Readline fallback for non-TTY or missing @clack
    const readline = await import('node:readline');
    const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
    apiKey = await new Promise((resolve) => {
      rl.question(`Enter your ${providerConfig.name} API key: `, (answer) => {
        rl.close();
        resolve(answer.trim());
      });
    });
  }

  if (!apiKey) {
    console.log(output.yellow('\nNo key entered. Skipping.'));
    return false;
  }

  // Validate prefix if specified
  if (providerConfig.prefix && !apiKey.startsWith(providerConfig.prefix)) {
    console.log(
      output.yellow(`\nWarning: Key doesn't start with expected prefix '${providerConfig.prefix}'`),
    );
  }

  // Save to .env file
  const envFile = loadEnvFile();
  envFile[providerConfig.envVar] = apiKey;
  saveEnvFile(envFile);

  console.log(t.success(`\n✓ API key saved to ~/.stateset/.env`));
  console.log(t.success(`✓ Ready to use! The CLI will automatically load your key.\n`));
  console.log(`Try it now:`);
  console.log(t.accent(`  stateset "show me all customers"`));

  return true;
}

// Show all configured API keys
async function showApiKeys(output, json = false, writeJson = null) {
  const results = {};

  for (const [provider, config] of Object.entries(API_KEY_PROVIDERS)) {
    const status = getApiKeyStatus(provider);
    results[provider] = {
      name: config.name,
      envVar: config.envVar,
      configured: status.configured,
      source: status.source || null,
      maskedKey: status.configured ? maskApiKey(status.value) : null,
      getKeyUrl: config.getKeyUrl,
    };
  }

  if (json) {
    if (writeJson) {
      await writeJson(results);
    } else {
      console.log(JSON.stringify(results, null, 2));
    }
    return;
  }

  console.log(`\n${output.bold('API Keys Status')}\n`);

  for (const info of Object.values(results)) {
    const statusIcon = info.configured ? output.green('✓') : output.red('✗');
    const statusText = info.configured
      ? `${info.maskedKey} (${info.source})`
      : output.dim('Not configured');

    console.log(`${statusIcon} ${output.bold(info.name)}`);
    console.log(`   ${output.dim('Variable:')} ${info.envVar}`);
    console.log(`   ${output.dim('Status:')}   ${statusText}`);
    if (!info.configured) {
      console.log(`   ${output.dim('Get key:')}  ${info.getKeyUrl}`);
    }
    console.log();
  }

  // Show quick setup hint if Anthropic not configured
  if (!results.anthropic.configured) {
    console.log(
      output.yellow('Tip: Run `stateset-config set-key anthropic` to set up your API key'),
    );
  }
}

async function main() {
  const { values, positionals } = parseArgs({
    options: {
      profile: { type: 'string', short: 'p' },
      force: { type: 'boolean', default: false },
      json: { type: 'boolean', default: false },
      output: { type: 'string' },
      help: { type: 'boolean', short: 'h', default: false },
    },
    allowPositionals: true,
  });

  if (values.help || positionals.length === 0) {
    console.log(HELP);
    process.exit(0);
  }

  const outputPath = values.output || null;
  if (outputPath) {
    values.json = true;
  }

  const output = new RichOutput({ color: !values.json });
  const writeJson = async (data) => {
    const payload = JSON.stringify(data, null, 2);
    if (outputPath) {
      await fs.promises.writeFile(outputPath, payload);
      return;
    }
    console.log(payload);
  };
  const emitError = async (message) => {
    if (values.json) {
      await writeJson({ error: message });
      return;
    }
    console.error(message);
  };
  const command = positionals[0];
  const args = positionals.slice(1);
  const config = loadConfig();

  if (CONFIG_RECOVERY_WARNINGS.length > 0) {
    const warningText = CONFIG_RECOVERY_WARNINGS.join('\n');
    if (values.json) {
      console.error(warningText);
    } else {
      for (const warning of CONFIG_RECOVERY_WARNINGS) {
        console.warn(output.yellow(`Warning: ${warning}`));
      }
    }
  }

  switch (command) {
    case 'set-key': {
      let provider = args[0];
      if (!provider && process.stdin.isTTY) {
        // Interactive provider selection via @clack
        try {
          const ui = await import('../src/ui.js');
          provider = await ui.select('Which API key do you want to configure?', [
            { value: 'anthropic', label: 'Anthropic (Claude)', hint: 'Required' },
            { value: 'openai', label: 'OpenAI', hint: 'Optional' },
            { value: 'gemini', label: 'Google Gemini', hint: 'Optional' },
          ]);
        } catch {
          provider = 'anthropic';
        }
      } else {
        provider = provider || 'anthropic';
      }
      await setApiKey(provider, output);
      break;
    }

    case 'show-keys': {
      await showApiKeys(output, values.json, writeJson);
      break;
    }

    case 'list': {
      const profiles = listProfiles();
      if (values.json) {
        await writeJson({ profiles, default: config.defaultProfile });
      } else {
        console.log(`\n${ICONS.session} ${output.bold('Configuration Profiles')}\n`);
        if (profiles.length === 0) {
          console.log(
            output.dim('  No profiles found. Create one with: stateset-config create <name>'),
          );
        } else {
          for (const name of profiles) {
            const isDefault = name === config.defaultProfile;
            const marker = isDefault ? output.green(' (default)') : '';
            console.log(`  ${isDefault ? '●' : '○'} ${name}${marker}`);
          }
        }
        console.log();
      }
      break;
    }

    case 'show': {
      const profileName = args[0] || values.profile || config.defaultProfile || 'default';
      let profile;
      try {
        profile = loadProfile(profileName);
      } catch (error) {
        await emitError(`Error: ${error.message}`);
        process.exit(1);
      }
      if (values.json) {
        await writeJson(profile);
      } else {
        console.log(`\n${ICONS.session} ${output.bold(`Profile: ${profileName}`)}\n`);
        for (const [key, value] of Object.entries(profile)) {
          console.log(`  ${output.dim(key + ':')} ${value}`);
        }
        console.log();
      }
      break;
    }

    case 'create': {
      const name = args[0];
      if (!name) {
        await emitError('Error: Profile name required');
        if (!values.json) {
          console.error('Usage: stateset-config create <profile-name>');
        }
        process.exit(1);
      }
      try {
        validateProfileName(name);
      } catch (error) {
        await emitError(`Error: ${error.message}`);
        process.exit(1);
      }
      const profiles = listProfiles();
      if (profiles.includes(name)) {
        await emitError(`Error: Profile '${name}' already exists`);
        process.exit(1);
      }
      const profile = createDefaultProfile(name);
      saveProfile(name, profile);
      if (values.json) {
        await writeJson({ profile: name, created: true });
      } else {
        console.log(output.green(`✓ Created profile: ${name}`));
        console.log(
          output.dim(`  Configure it with: stateset-config --profile ${name} set <key> <value>`),
        );
      }
      break;
    }

    case 'use': {
      const name = args[0];
      if (!name) {
        await emitError('Error: Profile name required');
        if (!values.json) {
          console.error('Usage: stateset-config use <profile-name>');
        }
        process.exit(1);
      }
      try {
        validateProfileName(name);
      } catch (error) {
        await emitError(`Error: ${error.message}`);
        process.exit(1);
      }
      const profiles = listProfiles();
      if (!profiles.includes(name)) {
        await emitError(`Error: Profile '${name}' not found`);
        if (!values.json) {
          console.error(`Available profiles: ${profiles.join(', ') || '(none)'}`);
        }
        process.exit(1);
      }
      config.defaultProfile = name;
      saveConfig(config);
      if (values.json) {
        await writeJson({ defaultProfile: name });
      } else {
        console.log(output.green(`✓ Now using profile: ${name}`));
      }
      break;
    }

    case 'set': {
      const key = args[0];
      const value = args.slice(1).join(' ');
      if (!key || !value) {
        await emitError('Error: Key and value required');
        if (!values.json) {
          console.error('Usage: stateset-config set <key> <value>');
        }
        process.exit(1);
      }

      if (!(key in KNOWN_CONFIG_KEYS) && !values.force) {
        await emitError(
          `Error: Unknown config key '${key}'. Known keys: ${Object.keys(KNOWN_CONFIG_KEYS).join(', ')}. Use --force to set custom keys.`,
        );
        process.exit(1);
      }
      if (!(key in KNOWN_CONFIG_KEYS) && values.force && !values.json) {
        console.warn(output.yellow(`Warning: setting custom key '${key}' due to --force`));
      }

      const profileName = values.profile || config.defaultProfile || 'default';
      let profile;
      if (profileExists(profileName)) {
        profile = loadProfile(profileName);
      } else if (!values.profile && profileName === (config.defaultProfile || 'default')) {
        profile = createDefaultProfile(profileName);
      } else {
        await emitError(`Error: Profile '${profileName}' not found`);
        process.exit(1);
      }

      // Parse and validate by type
      let parsedValue = value;
      const keyType = KNOWN_CONFIG_KEYS[key];

      if (keyType === 'boolean') {
        const lower = value.toLowerCase();
        const truthyValues = ['true', 'yes', '1', 'on'];
        const falsyValues = ['false', 'no', '0', 'off'];
        if (truthyValues.includes(lower)) {
          parsedValue = true;
        } else if (falsyValues.includes(lower)) {
          parsedValue = false;
        } else {
          await emitError(
            `Error: '${key}' expects a boolean value (true/false, yes/no, 1/0, on/off)`,
          );
          process.exit(1);
        }
      } else if (value === 'true') {
        parsedValue = true;
      } else if (value === 'false') {
        parsedValue = false;
      }

      // Key-specific validation (warnings, not errors)
      if (key === 'db' && parsedValue !== ':memory:' && !values.json) {
        const dir = path.dirname(path.resolve(String(parsedValue)));
        if (!fs.existsSync(dir)) {
          console.warn(
            `Warning: Directory '${dir}' does not exist. Create it with: mkdir -p ${dir}`,
          );
        }
      }
      if (key === 'provider' && !values.json) {
        const known = ['claude', 'openai', 'gemini', 'ollama'];
        if (!known.includes(parsedValue)) {
          console.warn(
            `Warning: Unknown provider '${parsedValue}'. Known providers: ${known.join(', ')}`,
          );
        }
      }
      if (key === 'think' && !values.json) {
        const levels = ['off', 'low', 'medium', 'high'];
        if (!levels.includes(parsedValue)) {
          console.warn(
            `Warning: Unknown think level '${parsedValue}'. Valid: ${levels.join(', ')}`,
          );
        }
      }
      if (key === 'format' && !values.json) {
        const fmts = ['table', 'json', 'csv', 'yaml'];
        if (!fmts.includes(parsedValue)) {
          console.warn(`Warning: Unknown format '${parsedValue}'. Valid: ${fmts.join(', ')}`);
        }
      }

      profile[key] = parsedValue;
      saveProfile(profileName, profile);
      if (values.json) {
        await writeJson({ profile: profileName, key, value: parsedValue });
      } else {
        console.log(output.green(`✓ Set ${key}=${parsedValue} in profile '${profileName}'`));
      }
      break;
    }

    case 'get': {
      const key = args[0];
      if (!key) {
        await emitError('Error: Key required');
        if (!values.json) {
          console.error('Usage: stateset-config get <key>');
        }
        process.exit(1);
      }
      const profileName = values.profile || config.defaultProfile || 'default';
      let profile;
      try {
        profile = loadProfile(profileName);
      } catch (error) {
        await emitError(`Error: ${error.message}`);
        process.exit(1);
      }
      const value = profile[key];
      if (value === undefined) {
        await emitError(`Key '${key}' not found in profile '${profileName}'`);
        process.exit(1);
      }
      if (values.json) {
        await writeJson({ [key]: value });
      } else {
        console.log(value);
      }
      break;
    }

    case 'path': {
      if (values.json) {
        await writeJson({
          configDir: CONFIG_DIR,
          configFile: CONFIG_FILE,
          profilesDir: PROFILES_DIR,
        });
      } else {
        console.log(`Config directory: ${CONFIG_DIR}`);
        console.log(`Config file: ${CONFIG_FILE}`);
        console.log(`Profiles directory: ${PROFILES_DIR}`);
      }
      break;
    }

    default:
      await emitError(`Unknown command: ${command}`);
      if (!values.json) {
        console.error('Run stateset-config --help for usage');
      }
      process.exit(1);
  }
}

// Only run main if this is the entry point (not imported)
import { fileURLToPath } from 'node:url';
import { runMain } from '../src/graceful-shutdown.js';
const __filename = fileURLToPath(import.meta.url);
if (
  process.argv[1] === __filename ||
  process.argv[1]?.endsWith('stateset-config.js') ||
  process.argv[1]?.endsWith('stateset-config')
) {
  runMain('stateset-config', main);
}

// Export for use by main CLI
export function getProfileConfig(profileName) {
  ensureConfigDir();
  const config = loadConfig();
  const name = profileName || config.defaultProfile || 'default';
  if (profileName) {
    return loadProfile(name);
  }
  if (!profileExists(name)) {
    return {};
  }
  return loadProfile(name);
}

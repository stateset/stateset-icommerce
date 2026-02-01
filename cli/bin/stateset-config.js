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
import { RichOutput, ICONS } from '../src/claude-harness.js';
import { CLI_VERSION } from '../src/config.js';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';

const CONFIG_DIR = path.join(os.homedir(), '.stateset');
const CONFIG_FILE = path.join(CONFIG_DIR, 'config.json');
const PROFILES_DIR = path.join(CONFIG_DIR, 'profiles');
const ENV_FILE = path.join(CONFIG_DIR, '.env');

// Supported API key providers
const API_KEY_PROVIDERS = {
  anthropic: {
    name: 'Anthropic (Claude)',
    envVar: 'ANTHROPIC_API_KEY',
    prefix: 'sk-ant-',
    getKeyUrl: 'https://console.anthropic.com/settings/keys',
    description: 'Required for AI-powered commands'
  },
  openai: {
    name: 'OpenAI',
    envVar: 'OPENAI_API_KEY',
    prefix: 'sk-',
    getKeyUrl: 'https://platform.openai.com/api-keys',
    description: 'Optional - for OpenAI models and vector search'
  },
  gemini: {
    name: 'Google Gemini',
    envVar: 'GEMINI_API_KEY',
    prefix: '',
    getKeyUrl: 'https://aistudio.google.com/app/apikey',
    description: 'Optional - for Gemini models'
  }
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
  --json                  Output as JSON
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

// Ensure config directory exists
function ensureConfigDir() {
  if (!fs.existsSync(CONFIG_DIR)) {
    fs.mkdirSync(CONFIG_DIR, { recursive: true });
  }
  if (!fs.existsSync(PROFILES_DIR)) {
    fs.mkdirSync(PROFILES_DIR, { recursive: true });
  }
}

// Load main config
function loadConfig() {
  ensureConfigDir();
  if (fs.existsSync(CONFIG_FILE)) {
    return JSON.parse(fs.readFileSync(CONFIG_FILE, 'utf-8'));
  }
  return { defaultProfile: 'default', version: CLI_VERSION };
}

// Save main config
function saveConfig(config) {
  ensureConfigDir();
  fs.writeFileSync(CONFIG_FILE, JSON.stringify(config, null, 2));
}

// Load a profile
function loadProfile(name) {
  const profilePath = path.join(PROFILES_DIR, `${name}.json`);
  if (fs.existsSync(profilePath)) {
    return JSON.parse(fs.readFileSync(profilePath, 'utf-8'));
  }
  // Return defaults for new profiles
  return {
    name,
    db: './store.db',
    model: 'claude-sonnet-4-20250514',
    apply: false,
    verbose: false,
    created: new Date().toISOString()
  };
}

// Save a profile
function saveProfile(name, profile) {
  ensureConfigDir();
  const profilePath = path.join(PROFILES_DIR, `${name}.json`);
  profile.updated = new Date().toISOString();
  fs.writeFileSync(profilePath, JSON.stringify(profile, null, 2));
}

// List all profiles
function listProfiles() {
  ensureConfigDir();
  if (!fs.existsSync(PROFILES_DIR)) {
    return [];
  }
  return fs.readdirSync(PROFILES_DIR)
    .filter(f => f.endsWith('.json'))
    .map(f => f.replace('.json', ''));
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
        // Remove surrounding quotes if present
        if ((value.startsWith('"') && value.endsWith('"')) ||
            (value.startsWith("'") && value.endsWith("'"))) {
          value = value.slice(1, -1);
        }
        env[key.trim()] = value;
      }
    }
  }
  return env;
}

// Save .env file
function saveEnvFile(env) {
  ensureConfigDir();
  const lines = [
    '# StateSet CLI API Keys',
    '# Generated by stateset-config set-key',
    '# Add this to your shell profile: source ~/.stateset/.env',
    ''
  ];
  for (const [key, value] of Object.entries(env)) {
    lines.push(`${key}="${value}"`);
  }
  fs.writeFileSync(ENV_FILE, lines.join('\n') + '\n');
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
  const readline = await import('node:readline');

  const config = API_KEY_PROVIDERS[provider];
  if (!config) {
    console.error(`Unknown provider: ${provider}`);
    console.error(`Available providers: ${Object.keys(API_KEY_PROVIDERS).join(', ')}`);
    process.exit(1);
  }

  console.log(`\n${output.bold(`Set up ${config.name} API Key`)}`);
  console.log(output.dim(`${config.description}\n`));

  // Check current status
  const status = getApiKeyStatus(provider);
  if (status.configured) {
    console.log(output.yellow(`Current key: ${maskApiKey(status.value)} (from ${status.source})`));
  }

  console.log(`Get your API key from: ${output.cyan(config.getKeyUrl)}\n`);

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout
  });

  return new Promise((resolve) => {
    rl.question(`Enter your ${config.name} API key: `, (apiKey) => {
      rl.close();

      apiKey = apiKey.trim();

      if (!apiKey) {
        console.log(output.yellow('\nNo key entered. Skipping.'));
        resolve(false);
        return;
      }

      // Validate prefix if specified
      if (config.prefix && !apiKey.startsWith(config.prefix)) {
        console.log(output.yellow(`\nWarning: Key doesn't start with expected prefix '${config.prefix}'`));
      }

      // Save to .env file
      const envFile = loadEnvFile();
      envFile[config.envVar] = apiKey;
      saveEnvFile(envFile);

      console.log(output.green(`\n✓ API key saved to ~/.stateset/.env`));
      console.log(output.green(`✓ Ready to use! The CLI will automatically load your key.\n`));
      console.log(`Try it now:`);
      console.log(output.cyan(`  stateset "show me all customers"`));

      resolve(true);
    });
  });
}

// Show all configured API keys
function showApiKeys(output, json = false) {
  const results = {};

  for (const [provider, config] of Object.entries(API_KEY_PROVIDERS)) {
    const status = getApiKeyStatus(provider);
    results[provider] = {
      name: config.name,
      envVar: config.envVar,
      configured: status.configured,
      source: status.source || null,
      maskedKey: status.configured ? maskApiKey(status.value) : null,
      getKeyUrl: config.getKeyUrl
    };
  }

  if (json) {
    console.log(JSON.stringify(results, null, 2));
    return;
  }

  console.log(`\n${output.bold('API Keys Status')}\n`);

  for (const [provider, info] of Object.entries(results)) {
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
    console.log(output.yellow('Tip: Run `stateset-config set-key anthropic` to set up your API key'));
  }
}

async function main() {
  const { values, positionals } = parseArgs({
    options: {
      profile: { type: 'string', short: 'p' },
      json: { type: 'boolean', default: false },
      help: { type: 'boolean', short: 'h', default: false }
    },
    allowPositionals: true
  });

  if (values.help || positionals.length === 0) {
    console.log(HELP);
    process.exit(0);
  }

  const output = new RichOutput({ color: !values.json });
  const command = positionals[0];
  const args = positionals.slice(1);
  const config = loadConfig();

  switch (command) {
    case 'set-key': {
      const provider = args[0] || 'anthropic';
      await setApiKey(provider, output);
      break;
    }

    case 'show-keys': {
      showApiKeys(output, values.json);
      break;
    }

    case 'list': {
      const profiles = listProfiles();
      if (values.json) {
        console.log(JSON.stringify({ profiles, default: config.defaultProfile }, null, 2));
      } else {
        console.log(`\n${ICONS.session} ${output.bold('Configuration Profiles')}\n`);
        if (profiles.length === 0) {
          console.log(output.dim('  No profiles found. Create one with: stateset-config create <name>'));
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
      const profile = loadProfile(profileName);
      if (values.json) {
        console.log(JSON.stringify(profile, null, 2));
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
        console.error('Error: Profile name required');
        console.error('Usage: stateset-config create <profile-name>');
        process.exit(1);
      }
      const profiles = listProfiles();
      if (profiles.includes(name)) {
        console.error(`Error: Profile '${name}' already exists`);
        process.exit(1);
      }
      const profile = loadProfile(name);
      saveProfile(name, profile);
      console.log(output.green(`✓ Created profile: ${name}`));
      console.log(output.dim(`  Configure it with: stateset-config --profile ${name} set <key> <value>`));
      break;
    }

    case 'use': {
      const name = args[0];
      if (!name) {
        console.error('Error: Profile name required');
        console.error('Usage: stateset-config use <profile-name>');
        process.exit(1);
      }
      const profiles = listProfiles();
      if (!profiles.includes(name)) {
        console.error(`Error: Profile '${name}' not found`);
        console.error(`Available profiles: ${profiles.join(', ') || '(none)'}`);
        process.exit(1);
      }
      config.defaultProfile = name;
      saveConfig(config);
      console.log(output.green(`✓ Now using profile: ${name}`));
      break;
    }

    case 'set': {
      const key = args[0];
      const value = args.slice(1).join(' ');
      if (!key || !value) {
        console.error('Error: Key and value required');
        console.error('Usage: stateset-config set <key> <value>');
        process.exit(1);
      }
      const profileName = values.profile || config.defaultProfile || 'default';
      const profile = loadProfile(profileName);

      // Parse booleans
      let parsedValue = value;
      if (value === 'true') parsedValue = true;
      else if (value === 'false') parsedValue = false;

      profile[key] = parsedValue;
      saveProfile(profileName, profile);
      console.log(output.green(`✓ Set ${key}=${value} in profile '${profileName}'`));
      break;
    }

    case 'get': {
      const key = args[0];
      if (!key) {
        console.error('Error: Key required');
        console.error('Usage: stateset-config get <key>');
        process.exit(1);
      }
      const profileName = values.profile || config.defaultProfile || 'default';
      const profile = loadProfile(profileName);
      const value = profile[key];
      if (values.json) {
        console.log(JSON.stringify({ [key]: value }));
      } else if (value !== undefined) {
        console.log(value);
      } else {
        console.error(`Key '${key}' not found in profile '${profileName}'`);
        process.exit(1);
      }
      break;
    }

    case 'path': {
      if (values.json) {
        console.log(JSON.stringify({ configDir: CONFIG_DIR, configFile: CONFIG_FILE, profilesDir: PROFILES_DIR }));
      } else {
        console.log(`Config directory: ${CONFIG_DIR}`);
        console.log(`Config file: ${CONFIG_FILE}`);
        console.log(`Profiles directory: ${PROFILES_DIR}`);
      }
      break;
    }

    default:
      console.error(`Unknown command: ${command}`);
      console.error('Run stateset-config --help for usage');
      process.exit(1);
  }
}

// Only run main if this is the entry point (not imported)
import { fileURLToPath } from 'node:url';
const __filename = fileURLToPath(import.meta.url);
if (process.argv[1] === __filename || process.argv[1]?.endsWith('stateset-config.js') || process.argv[1]?.endsWith('stateset-config')) {
  main().catch(error => {
    console.error('Config error:', error.message);
    process.exit(1);
  });
}

// Export for use by main CLI
export function getProfileConfig(profileName) {
  ensureConfigDir();
  const config = loadConfig();
  const name = profileName || config.defaultProfile || 'default';
  return loadProfile(name);
}

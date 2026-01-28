#!/usr/bin/env node

/**
 * StateSet iCommerce CLI - Discord Gateway
 *
 * Connects your StateSet commerce agent to Discord.
 *
 * Usage:
 *   stateset-discord --apply --db ./store.db
 */

import { parseArgs } from 'node:util';
import { startDiscordGateway } from '../src/discord/gateway.js';
import { DEFAULT_MODEL, CLI_VERSION } from '../src/config.js';

const HELP = `
StateSet iCommerce - Discord Gateway v${CLI_VERSION}

Connect your commerce agent to Discord for customer conversations.

USAGE:
  stateset-discord [options]

OPTIONS:
  --db <path>          Path to SQLite database (default: ./store.db)
  --apply              Enable write operations (create orders, carts, etc.)
  --model <model>      Claude model to use (default: ${DEFAULT_MODEL})
  --max-turns <n>      Max agent turns per message (default: 10)
  --agent <name>       Force a specific agent (default: auto-route)
  --allow <ids>        Comma-separated allowlist of Discord user IDs (default: allow all)
  --mention-only       In servers, only respond when @mentioned (always responds in DMs)
  --verbose, -V        Enable verbose logging
  --help, -h           Show this help message

SETUP:
  1. Create an application at https://discord.com/developers/applications
  2. Add a Bot in the Bot section
  3. Enable MESSAGE CONTENT INTENT in the Bot section
  4. Copy the bot token
  5. Set DISCORD_BOT_TOKEN environment variable
  6. Invite the bot to your server with the OAuth2 URL generator
     (scopes: bot; permissions: Send Messages, Read Message History)
  7. Run: stateset-discord --apply

ENVIRONMENT:
  DISCORD_BOT_TOKEN    Bot token from Discord Developer Portal (required)
  ANTHROPIC_API_KEY    Anthropic API key (required)

EXAMPLES:
  stateset-discord --apply
  stateset-discord --apply --mention-only
  stateset-discord --apply --allow 123456789012345678

DISCORD COMMANDS (sent by users in chat):
  /help    - Show available commands
  /reset   - Start new conversation
  /status  - Show session info
`;

async function main() {
  const { values } = parseArgs({
    options: {
      db: { type: 'string', default: './store.db' },
      apply: { type: 'boolean', default: false },
      model: { type: 'string', default: DEFAULT_MODEL },
      'max-turns': { type: 'string', default: '10' },
      agent: { type: 'string' },
      allow: { type: 'string' },
      'mention-only': { type: 'boolean', default: false },
      verbose: { type: 'boolean', short: 'V', default: false },
      help: { type: 'boolean', short: 'h', default: false },
    },
    allowPositionals: false,
    strict: true,
  });

  if (values.help) {
    console.log(HELP);
    process.exit(0);
  }

  if (!process.env.ANTHROPIC_API_KEY) {
    console.error('Error: ANTHROPIC_API_KEY environment variable is required.');
    console.error('Set it with: export ANTHROPIC_API_KEY=sk-ant-...');
    process.exit(1);
  }

  const allowlist = values.allow
    ? values.allow.split(',').map((s) => s.trim()).filter(Boolean)
    : null;

  const maxTurns = parseInt(values['max-turns'], 10) || 10;

  console.log(`\n  StateSet Discord Gateway v${CLI_VERSION}`);
  console.log(`  ─────────────────────────────────────`);
  console.log(`  Database:    ${values.db}`);
  console.log(`  Mode:        ${values.apply ? 'Write enabled' : 'Preview only (use --apply for writes)'}`);
  console.log(`  Model:       ${values.model}`);
  console.log(`  Max turns:   ${maxTurns}`);
  console.log(`  Agent:       ${values.agent || 'auto-route'}`);
  console.log(`  Mention-only: ${values['mention-only'] ? 'yes' : 'no'}`);
  console.log(`  Allowlist:   ${allowlist ? allowlist.join(', ') : 'all (open)'}`);
  console.log();

  try {
    const { shutdown } = await startDiscordGateway({
      dbPath: values.db,
      allowApply: values.apply,
      model: values.model,
      maxTurns,
      verbose: values.verbose,
      allowlist,
      agent: values.agent,
      mentionOnly: values['mention-only'],
    });

    const handleShutdown = () => {
      console.log('\nShutting down Discord gateway...');
      shutdown();
      process.exit(0);
    };

    process.on('SIGINT', handleShutdown);
    process.on('SIGTERM', handleShutdown);
  } catch (err) {
    console.error('Failed to start Discord gateway:', err.message);
    if (values.verbose) console.error(err);
    process.exit(1);
  }
}

main();

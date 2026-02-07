#!/usr/bin/env node

/**
 * StateSet iCommerce CLI - Telegram Gateway
 *
 * Connects your StateSet commerce agent to Telegram.
 *
 * Usage:
 *   stateset-telegram --apply --db ./store.db
 */

import { parseArgs } from 'node:util';
import { startTelegramGateway } from '../src/telegram/gateway.js';
import { DEFAULT_MODEL, CLI_VERSION } from '../src/config.js';

const HELP = `
StateSet iCommerce - Telegram Gateway v${CLI_VERSION}

Connect your commerce agent to Telegram for customer conversations.

USAGE:
  stateset-telegram [options]

OPTIONS:
  --db <path>          Path to SQLite database (default: ./store.db)
  --apply              Enable write operations (create orders, carts, etc.)
  --model <model>      Claude model to use (default: ${DEFAULT_MODEL})
  --max-turns <n>      Max agent turns per message (default: 10)
  --agent <name>       Force a specific agent (default: auto-route)
  --allow <ids>        Comma-separated allowlist of Telegram user IDs (default: allow all)
  --verbose, -V        Enable verbose logging
  --help, -h           Show this help message

SETUP:
  1. Message @BotFather on Telegram to create a bot
  2. Copy the bot token
  3. Set TELEGRAM_BOT_TOKEN environment variable
  4. Run: stateset-telegram --apply

ENVIRONMENT:
  TELEGRAM_BOT_TOKEN   Bot token from @BotFather (required)
  ANTHROPIC_API_KEY    Anthropic API key (required)

EXAMPLES:
  stateset-telegram --apply
  stateset-telegram --apply --allow 123456789,987654321
  stateset-telegram --apply --db /data/commerce.db

TELEGRAM COMMANDS (sent by users in chat):
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
    ? values.allow
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean)
    : null;

  const maxTurns = parseInt(values['max-turns'], 10) || 10;

  console.log(`\n  StateSet Telegram Gateway v${CLI_VERSION}`);
  console.log(`  ─────────────────────────────────────`);
  console.log(`  Database:    ${values.db}`);
  console.log(
    `  Mode:        ${values.apply ? 'Write enabled' : 'Preview only (use --apply for writes)'}`,
  );
  console.log(`  Model:       ${values.model}`);
  console.log(`  Max turns:   ${maxTurns}`);
  console.log(`  Agent:       ${values.agent || 'auto-route'}`);
  console.log(`  Allowlist:   ${allowlist ? allowlist.join(', ') : 'all (open)'}`);
  console.log();

  try {
    const { shutdown } = await startTelegramGateway({
      dbPath: values.db,
      allowApply: values.apply,
      model: values.model,
      maxTurns,
      verbose: values.verbose,
      allowlist,
      agent: values.agent,
    });

    const handleShutdown = () => {
      console.log('\nShutting down Telegram gateway...');
      shutdown();
      process.exit(0);
    };

    process.on('SIGINT', handleShutdown);
    process.on('SIGTERM', handleShutdown);
  } catch (err) {
    console.error('Failed to start Telegram gateway:', err.message);
    if (values.verbose) console.error(err);
    process.exit(1);
  }
}

import { runMain } from '../src/graceful-shutdown.js';
runMain('stateset-telegram', main);

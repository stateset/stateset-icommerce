#!/usr/bin/env node

/**
 * StateSet iCommerce CLI - Slack Gateway
 *
 * Connects your StateSet commerce agent to Slack via Socket Mode.
 *
 * Usage:
 *   stateset-slack --apply --db ./store.db
 */

import { parseArgs } from 'node:util';
import { startSlackGateway } from '../src/slack/gateway.js';
import { DEFAULT_MODEL, CLI_VERSION } from '../src/config.js';

const HELP = `
StateSet iCommerce - Slack Gateway v${CLI_VERSION}

Connect your commerce agent to Slack for customer conversations.

USAGE:
  stateset-slack [options]

OPTIONS:
  --db <path>          Path to SQLite database (default: ./store.db)
  --apply              Enable write operations (create orders, carts, etc.)
  --model <model>      Claude model to use (default: ${DEFAULT_MODEL})
  --max-turns <n>      Max agent turns per message (default: 10)
  --agent <name>       Force a specific agent (default: auto-route)
  --allow <ids>        Comma-separated allowlist of Slack user IDs (default: allow all)
  --verbose, -V        Enable verbose logging
  --help, -h           Show this help message

SETUP:
  1. Create a Slack app at https://api.slack.com/apps
  2. Enable Socket Mode (Settings > Socket Mode)
  3. Generate an app-level token (xapp-...) with connections:write scope
  4. Add Bot Token Scopes: chat:write, app_mentions:read, im:history, channels:history
  5. Install the app to your workspace
  6. Copy the Bot User OAuth Token (xoxb-...)
  7. Set environment variables:
     export SLACK_BOT_TOKEN=xoxb-...
     export SLACK_APP_TOKEN=xapp-...
  8. Run: stateset-slack --apply

BEHAVIOR:
  - In DMs: responds to all messages
  - In channels: only responds when @mentioned or in threads with the bot

ENVIRONMENT:
  SLACK_BOT_TOKEN      Bot User OAuth Token (xoxb-...) (required)
  SLACK_APP_TOKEN      App-level token for Socket Mode (xapp-...) (required)
  ANTHROPIC_API_KEY    Anthropic API key (required)

EXAMPLES:
  stateset-slack --apply
  stateset-slack --apply --allow U12345678,U87654321
  stateset-slack --apply --db /data/commerce.db

SLACK COMMANDS (sent by users in chat):
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

  console.log(`\n  StateSet Slack Gateway v${CLI_VERSION}`);
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
    const { shutdown } = await startSlackGateway({
      dbPath: values.db,
      allowApply: values.apply,
      model: values.model,
      maxTurns,
      verbose: values.verbose,
      allowlist,
      agent: values.agent,
    });

    const handleShutdown = async () => {
      console.log('\nShutting down Slack gateway...');
      await shutdown();
      process.exit(0);
    };

    process.on('SIGINT', handleShutdown);
    process.on('SIGTERM', handleShutdown);
  } catch (err) {
    console.error('Failed to start Slack gateway:', err.message);
    if (values.verbose) console.error(err);
    process.exit(1);
  }
}

import { runMain } from '../src/graceful-shutdown.js';
runMain('stateset-slack', main);

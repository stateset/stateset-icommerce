#!/usr/bin/env node

/**
 * StateSet iCommerce CLI - Signal Gateway
 *
 * Connects your StateSet commerce agent to Signal via signal-cli daemon.
 *
 * Usage:
 *   stateset-signal --apply --db ./store.db --phone +14155551234
 */

import { parseArgs } from 'node:util';
import { startSignalGateway } from '../src/signal/gateway.js';
import { DEFAULT_MODEL, CLI_VERSION } from '../src/config.js';

const HELP = `
StateSet iCommerce - Signal Gateway v${CLI_VERSION}

Connect your commerce agent to Signal for secure customer conversations.

USAGE:
  stateset-signal --phone <number> [options]

OPTIONS:
  --db <path>          Path to SQLite database (default: ./store.db)
  --apply              Enable write operations (create orders, carts, etc.)
  --model <model>      Claude model to use (default: ${DEFAULT_MODEL})
  --max-turns <n>      Max agent turns per message (default: 10)
  --agent <name>       Force a specific agent (default: auto-route)
  --allow <phones>     Comma-separated allowlist of phone numbers (default: allow all)
  --phone <number>     Registered Signal phone number (required)
  --socket <path>      Path to signal-cli daemon socket (default: /tmp/signal-cli.sock)
  --verbose, -V        Enable verbose logging
  --help, -h           Show this help message

PREREQUISITES:
  1. Install signal-cli: https://github.com/AsamK/signal-cli
  2. Register or link a phone number:
     signal-cli -u +14155551234 register
     signal-cli -u +14155551234 verify <code>
  3. Start the daemon:
     signal-cli -u +14155551234 daemon --json --socket /tmp/signal-cli.sock
  4. Run: stateset-signal --apply --phone +14155551234

ENVIRONMENT:
  ANTHROPIC_API_KEY    Anthropic API key (required)

EXAMPLES:
  stateset-signal --apply --phone +14155551234
  stateset-signal --apply --phone +14155551234 --socket /var/run/signal.sock
  stateset-signal --apply --phone +14155551234 --allow +14155555678,+14155559012

SIGNAL COMMANDS (sent by users in chat):
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
      phone: { type: 'string' },
      socket: { type: 'string', default: '/tmp/signal-cli.sock' },
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

  if (!values.phone) {
    console.error('Error: --phone is required. Provide the registered Signal phone number.');
    console.error('Example: stateset-signal --apply --phone +14155551234');
    process.exit(1);
  }

  const allowlist = values.allow
    ? values.allow
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean)
    : null;

  const maxTurns = parseInt(values['max-turns'], 10) || 10;

  console.log(`\n  StateSet Signal Gateway v${CLI_VERSION}`);
  console.log(`  ─────────────────────────────────────`);
  console.log(`  Database:    ${values.db}`);
  console.log(
    `  Mode:        ${values.apply ? 'Write enabled' : 'Preview only (use --apply for writes)'}`,
  );
  console.log(`  Model:       ${values.model}`);
  console.log(`  Max turns:   ${maxTurns}`);
  console.log(`  Agent:       ${values.agent || 'auto-route'}`);
  console.log(`  Phone:       ${values.phone}`);
  console.log(`  Socket:      ${values.socket}`);
  console.log(`  Allowlist:   ${allowlist ? allowlist.join(', ') : 'all (open)'}`);
  console.log();

  try {
    const { shutdown } = await startSignalGateway({
      dbPath: values.db,
      allowApply: values.apply,
      model: values.model,
      maxTurns,
      verbose: values.verbose,
      allowlist,
      agent: values.agent,
      phone: values.phone,
      socket: values.socket,
    });

    const handleShutdown = () => {
      console.log('\nShutting down Signal gateway...');
      shutdown();
      process.exit(0);
    };

    process.on('SIGINT', handleShutdown);
    process.on('SIGTERM', handleShutdown);
  } catch (err) {
    console.error('Failed to start Signal gateway:', err.message);
    if (values.verbose) console.error(err);
    process.exit(1);
  }
}

import { runMain } from '../src/graceful-shutdown.js';
runMain('stateset-signal', main);

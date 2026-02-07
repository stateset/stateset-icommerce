#!/usr/bin/env node

/**
 * StateSet iCommerce CLI - WhatsApp Gateway
 *
 * Connects your StateSet commerce agent to WhatsApp so customers
 * can chat with the agent directly from their phone.
 *
 * Usage:
 *   stateset-whatsapp                         # Start gateway (QR login on first run)
 *   stateset-whatsapp --apply                  # Enable write operations (orders, carts, etc.)
 *   stateset-whatsapp --allow 14155551234      # Only allow specific phone numbers
 *   stateset-whatsapp --allow 14155551234,14155555678
 *   stateset-whatsapp --groups                 # Also respond in group chats
 *   stateset-whatsapp --reset                  # Clear saved WhatsApp credentials
 */

import { parseArgs } from 'node:util';
import { startWhatsAppGateway } from '../src/whatsapp/gateway.js';
import { clearAuth, DEFAULT_AUTH_DIR } from '../src/whatsapp/session.js';
import { DEFAULT_MODEL, CLI_VERSION } from '../src/config.js';

const HELP = `
StateSet iCommerce - WhatsApp Gateway v${CLI_VERSION}

Connect your commerce agent to WhatsApp for customer conversations.

USAGE:
  stateset-whatsapp [options]

OPTIONS:
  --db <path>          Path to SQLite database (default: ./store.db)
  --apply              Enable write operations (create orders, carts, etc.)
  --model <model>      Claude model to use (default: ${DEFAULT_MODEL})
  --max-turns <n>      Max agent turns per message (default: 10)
  --agent <name>       Force a specific agent (default: auto-route)
  --allow <phones>     Comma-separated allowlist of phone numbers (default: allow all)
  --groups             Respond to group chat messages
  --auth-dir <path>    WhatsApp credential storage (default: ~/.stateset/whatsapp-auth)
  --reset              Clear saved WhatsApp credentials and re-scan QR
  --verbose, -V        Enable verbose logging
  --help, -h           Show this help message

FIRST RUN:
  On first launch, a QR code will be displayed in the terminal.
  Scan it with WhatsApp > Settings > Linked Devices > Link a Device.

EXAMPLES:
  # Start with write access enabled
  stateset-whatsapp --apply

  # Only allow specific numbers
  stateset-whatsapp --apply --allow 14155551234,14155555678

  # Use a specific database
  stateset-whatsapp --apply --db /data/commerce.db

  # Reset WhatsApp login
  stateset-whatsapp --reset

WHATSAPP COMMANDS (sent by users in chat):
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
      groups: { type: 'boolean', default: false },
      'auth-dir': { type: 'string' },
      reset: { type: 'boolean', default: false },
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

  // Handle --reset: clear credentials
  if (values.reset) {
    const authDir = values['auth-dir'] || DEFAULT_AUTH_DIR;
    clearAuth(authDir);
    console.log(`Cleared WhatsApp credentials from ${authDir}`);
    console.log('Run stateset-whatsapp again to scan a new QR code.');
    process.exit(0);
  }

  // Check for ANTHROPIC_API_KEY
  if (!process.env.ANTHROPIC_API_KEY) {
    console.error('Error: ANTHROPIC_API_KEY environment variable is required.');
    console.error('Set it with: export ANTHROPIC_API_KEY=sk-ant-...');
    process.exit(1);
  }

  // Parse allowlist
  const allowlist = values.allow
    ? values.allow
        .split(',')
        .map((p) => p.trim())
        .filter(Boolean)
    : null;

  // Parse max turns
  const maxTurns = parseInt(values['max-turns'], 10) || 10;

  // Print startup banner
  console.log(`\n  StateSet WhatsApp Gateway v${CLI_VERSION}`);
  console.log(`  ─────────────────────────────────────`);
  console.log(`  Database:    ${values.db}`);
  console.log(
    `  Mode:        ${values.apply ? 'Write enabled' : 'Preview only (use --apply for writes)'}`,
  );
  console.log(`  Model:       ${values.model}`);
  console.log(`  Max turns:   ${maxTurns}`);
  console.log(`  Agent:       ${values.agent || 'auto-route'}`);
  console.log(`  Groups:      ${values.groups ? 'enabled' : 'disabled'}`);
  console.log(`  Allowlist:   ${allowlist ? allowlist.join(', ') : 'all (open)'}`);
  console.log();

  try {
    const { shutdown } = await startWhatsAppGateway({
      dbPath: values.db,
      allowApply: values.apply,
      model: values.model,
      maxTurns,
      authDir: values['auth-dir'],
      verbose: values.verbose,
      allowlist,
      allowGroups: values.groups,
      agent: values.agent,
    });

    // Graceful shutdown on SIGINT / SIGTERM
    const handleShutdown = () => {
      console.log('\nShutting down WhatsApp gateway...');
      shutdown();
      process.exit(0);
    };

    process.on('SIGINT', handleShutdown);
    process.on('SIGTERM', handleShutdown);
  } catch (err) {
    console.error('Failed to start WhatsApp gateway:', err.message);
    if (values.verbose) console.error(err);
    process.exit(1);
  }
}

import { runMain } from '../src/graceful-shutdown.js';
runMain('stateset-whatsapp', main);

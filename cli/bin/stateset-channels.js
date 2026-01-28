#!/usr/bin/env node

/**
 * StateSet iCommerce - Multi-Channel Gateway Orchestrator
 *
 * Launch multiple channel gateways in a single process with shared
 * session persistence, middleware, and notification routing.
 *
 * Usage:
 *   stateset-channels --config channels.yaml
 *   stateset-channels --config channels.json --verbose
 */

import { parseArgs } from 'node:util';
import fs from 'fs';
import { CLI_VERSION } from '../src/config.js';
import { ChannelOrchestrator, loadOrchestratorConfig } from '../src/channels/orchestrator.js';

const HELP = `
StateSet iCommerce - Multi-Channel Gateway v${CLI_VERSION}

Launch multiple messaging channels in a single process.

USAGE:
  stateset-channels --config <path>

OPTIONS:
  --config <path>    Path to YAML or JSON config file (required)
  --verbose, -V      Enable verbose logging
  --help, -h         Show this help message

CONFIG FILE FORMAT (YAML):
  shared:
    dbPath: ./store.db
    allowApply: true
    model: claude-sonnet-4-5-20250929

  middleware:
    logger: true
    rateLimiter:
      maxPerMinute: 20
      maxPerHour: 200
    languageDetect: true

  notifications:
    routes:
      order.shipped:
        - { channel: whatsapp, target: "+14155551234" }
      inventory.low:
        - { channel: slack, target: "#ops" }
      "*":
        - { channel: slack, target: "#all-alerts" }

  channels:
    telegram:
      enabled: true
    discord:
      enabled: true
      mentionOnly: true
    slack:
      enabled: true
    whatsapp:
      enabled: false
    signal:
      enabled: false
      phone: "+14155551234"
      socket: /tmp/signal-cli.sock
    google-chat:
      enabled: false
      subscription: projects/my-project/subscriptions/chat-sub

EXAMPLES:
  stateset-channels --config channels.yaml
  stateset-channels --config channels.json --verbose

ENVIRONMENT:
  ANTHROPIC_API_KEY      Required for AI agent
  TELEGRAM_BOT_TOKEN     Required if telegram enabled
  DISCORD_BOT_TOKEN      Required if discord enabled
  SLACK_BOT_TOKEN        Required if slack enabled
  SLACK_APP_TOKEN        Required if slack enabled
  GOOGLE_APPLICATION_CREDENTIALS  Required if google-chat enabled
`;

async function main() {
  const { values } = parseArgs({
    options: {
      config: { type: 'string' },
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

  if (!values.config) {
    console.error('Error: --config <path> is required.');
    console.error('Run stateset-channels --help for usage.');
    process.exit(1);
  }

  if (!fs.existsSync(values.config)) {
    console.error(`Error: Config file not found: ${values.config}`);
    process.exit(1);
  }

  if (!process.env.ANTHROPIC_API_KEY) {
    console.error('Error: ANTHROPIC_API_KEY environment variable is required.');
    process.exit(1);
  }

  console.log('');
  console.log('  StateSet Multi-Channel Gateway v' + CLI_VERSION);
  console.log('  ' + '='.repeat(42));
  console.log('');

  try {
    const config = await loadOrchestratorConfig(values.config);

    // Inject verbose flag
    if (values.verbose) {
      config.shared = config.shared || {};
      config.shared.verbose = true;
    }

    const orchestrator = new ChannelOrchestrator(config);
    const { started, failed } = await orchestrator.start();

    console.log('');
    console.log('  Channel Status:');
    for (const name of started) {
      console.log(`    ${name}: running`);
    }
    for (const { channel, error } of failed) {
      console.log(`    ${channel}: FAILED (${error})`);
    }

    if (started.length === 0) {
      console.error('\n  No channels started. Check your config and environment variables.');
      process.exit(1);
    }

    console.log(`\n  ${started.length} channel(s) active. Press Ctrl+C to stop.\n`);

    // Graceful shutdown
    const handleShutdown = async () => {
      console.log('\nShutting down all channels...');
      await orchestrator.shutdown();
      process.exit(0);
    };

    process.on('SIGINT', handleShutdown);
    process.on('SIGTERM', handleShutdown);

    // Keep alive
    await new Promise(() => {});
  } catch (err) {
    console.error(`Error: ${err.message}`);
    if (values.verbose) console.error(err.stack);
    process.exit(1);
  }
}

main();

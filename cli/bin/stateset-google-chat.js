#!/usr/bin/env node

/**
 * StateSet iCommerce CLI - Google Chat Gateway
 *
 * Connects your StateSet commerce agent to Google Chat via Pub/Sub.
 *
 * Usage:
 *   stateset-google-chat --apply --db ./store.db --subscription projects/my-project/subscriptions/chat-sub
 */

import { parseArgs } from 'node:util';
import { startGoogleChatGateway } from '../src/google-chat/gateway.js';
import { DEFAULT_MODEL, CLI_VERSION } from '../src/config.js';

const HELP = `
StateSet iCommerce - Google Chat Gateway v${CLI_VERSION}

Connect your commerce agent to Google Chat for workspace conversations.

USAGE:
  stateset-google-chat --subscription <name> [options]

OPTIONS:
  --db <path>              Path to SQLite database (default: ./store.db)
  --apply                  Enable write operations (create orders, carts, etc.)
  --model <model>          Claude model to use (default: ${DEFAULT_MODEL})
  --max-turns <n>          Max agent turns per message (default: 10)
  --agent <name>           Force a specific agent (default: auto-route)
  --allow <ids>            Comma-separated allowlist of Google user IDs (default: allow all)
  --subscription <name>    Pub/Sub subscription name (required)
  --verbose, -V            Enable verbose logging
  --help, -h               Show this help message

SETUP:
  1. Create a GCP project at https://console.cloud.google.com
  2. Enable the Google Chat API and Cloud Pub/Sub API
  3. Create a service account with roles:
     - Chat Bots (roles/chat.bot)
     - Pub/Sub Subscriber (roles/pubsub.subscriber)
  4. Download the service account JSON key
  5. Configure a Chat app:
     - Go to Google Chat API > Configuration
     - Set connection type to "Cloud Pub/Sub"
     - Enter your Pub/Sub topic name
  6. Create a Pub/Sub subscription for the topic
  7. Set environment variables:
     export GOOGLE_APPLICATION_CREDENTIALS=/path/to/service-account.json
  8. Run: stateset-google-chat --apply --subscription projects/my-project/subscriptions/chat-sub

ENVIRONMENT:
  GOOGLE_APPLICATION_CREDENTIALS   Path to service account JSON key (required)
  ANTHROPIC_API_KEY                Anthropic API key (required)

EXAMPLES:
  stateset-google-chat --apply --subscription projects/myproject/subscriptions/chat-sub
  stateset-google-chat --apply --subscription chat-sub --db /data/commerce.db

GOOGLE CHAT COMMANDS (sent by users in chat):
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
      subscription: { type: 'string' },
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

  if (!values.subscription) {
    console.error('Error: --subscription is required. Provide the Pub/Sub subscription name.');
    console.error(
      'Example: stateset-google-chat --apply --subscription projects/my-project/subscriptions/chat-sub',
    );
    process.exit(1);
  }

  const allowlist = values.allow
    ? values.allow
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean)
    : null;

  const maxTurns = parseInt(values['max-turns'], 10) || 10;

  console.log(`\n  StateSet Google Chat Gateway v${CLI_VERSION}`);
  console.log(`  ─────────────────────────────────────────`);
  console.log(`  Database:       ${values.db}`);
  console.log(
    `  Mode:           ${values.apply ? 'Write enabled' : 'Preview only (use --apply for writes)'}`,
  );
  console.log(`  Model:          ${values.model}`);
  console.log(`  Max turns:      ${maxTurns}`);
  console.log(`  Agent:          ${values.agent || 'auto-route'}`);
  console.log(`  Subscription:   ${values.subscription}`);
  console.log(`  Allowlist:      ${allowlist ? allowlist.join(', ') : 'all (open)'}`);
  console.log();

  try {
    const { shutdown } = await startGoogleChatGateway({
      dbPath: values.db,
      allowApply: values.apply,
      model: values.model,
      maxTurns,
      verbose: values.verbose,
      allowlist,
      agent: values.agent,
      subscription: values.subscription,
    });

    const handleShutdown = async () => {
      console.log('\nShutting down Google Chat gateway...');
      await shutdown();
      process.exit(0);
    };

    process.on('SIGINT', handleShutdown);
    process.on('SIGTERM', handleShutdown);
  } catch (err) {
    console.error('Failed to start Google Chat gateway:', err.message);
    if (values.verbose) console.error(err);
    process.exit(1);
  }
}

import { runMain } from '../src/graceful-shutdown.js';
runMain('stateset-google-chat', main);

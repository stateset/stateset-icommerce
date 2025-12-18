#!/usr/bin/env node

/**
 * StateSet Commerce CLI - AI-powered commerce operations
 *
 * Usage:
 *   stateset "show me all customers"
 *   stateset --apply "create a customer with email alice@example.com"
 *   stateset --db ./mystore.db "list all orders"
 *   stateset --resume <session-id> "now ship that order"
 */

import { runAgentLoop } from '../src/claude-harness.js';
import { parseArgs } from 'node:util';

const HELP = `
StateSet Commerce CLI - AI-powered commerce operations

USAGE:
  stateset [options] "<request>"

OPTIONS:
  --db <path>        Path to SQLite database (default: ./store.db)
  --apply            Enable write operations (create, update, delete)
  --model <model>    Claude model to use (default: claude-sonnet-4-20250514)
  --resume <id>      Resume a previous session
  --json             Output as JSON
  --help, -h         Show this help message
  --version, -v      Show version

SPECIALIZED AGENTS:
  stateset-checkout    Shopping cart & checkout flow (ACP)
  stateset-orders      Order lifecycle management
  stateset-inventory   Stock & reservation management
  stateset-returns     RMA & refund processing

  Use specialized agents for focused workflows with domain-specific tooling.
  The main 'stateset' command auto-routes to the best agent.

EXAMPLES:
  # List customers (read-only)
  stateset "show me all customers"

  # Check inventory
  stateset "how much stock do we have of SKU-001?"

  # Create a customer (requires --apply)
  stateset --apply "create a customer named Alice Smith with email alice@example.com"

  # Create and ship an order
  stateset --apply "create an order for customer X with 2 widgets at $29.99 each"
  stateset --apply --resume <session-id> "now ship that order with tracking ABC123"

  # Shopping cart checkout flow (ACP)
  stateset --apply "create a cart for alice@example.com"
  stateset --apply --resume <session-id> "add 2 widgets at $29.99"
  stateset --apply --resume <session-id> "set shipping to 123 Main St, Anytown, CA"
  stateset --apply --resume <session-id> "complete the checkout"

  # Cart recovery
  stateset "show me abandoned carts"

  # Use a different database
  stateset --db ./production.db "list recent orders"

SAFETY:
  By default, all write operations are blocked. Use --apply to enable them.
  The CLI will always show you what would happen before making changes.
`;

async function main() {
  // Parse arguments
  const { values, positionals } = parseArgs({
    options: {
      db: { type: 'string', default: './store.db' },
      apply: { type: 'boolean', default: false },
      model: { type: 'string', default: 'claude-sonnet-4-20250514' },
      resume: { type: 'string' },
      json: { type: 'boolean', default: false },
      help: { type: 'boolean', short: 'h', default: false },
      version: { type: 'boolean', short: 'v', default: false }
    },
    allowPositionals: true
  });

  // Handle help
  if (values.help) {
    console.log(HELP);
    process.exit(0);
  }

  // Handle version
  if (values.version) {
    console.log('@stateset/cli v0.1.0');
    process.exit(0);
  }

  // Get request from positionals
  const request = positionals.join(' ').trim();
  if (!request) {
    console.error('Error: No request provided');
    console.error('Usage: stateset "<your request>"');
    console.error('Run stateset --help for more information');
    process.exit(1);
  }

  // Show mode indicator
  if (!values.json) {
    console.log(`\n📦 StateSet Commerce CLI`);
    console.log(`   Database: ${values.db}`);
    console.log(`   Mode: ${values.apply ? '✏️  Write enabled' : '👁️  Preview only'}`);
    if (values.resume) {
      console.log(`   Session: ${values.resume}`);
    }
    console.log();
  }

  try {
    const result = await runAgentLoop({
      request,
      dbPath: values.db,
      model: values.model,
      allowApply: values.apply,
      resumeSessionId: values.resume,
      onToolCall: (toolCall) => {
        if (!values.json) {
          const toolName = toolCall.name.replace('mcp__stateset-commerce__', '');
          console.log(`🔧 ${toolName}(${JSON.stringify(toolCall.input)})`);
        }
      }
    });

    if (values.json) {
      // JSON output
      console.log(JSON.stringify({
        request,
        allowApply: values.apply,
        sessionId: result.sessionId,
        response: result.response,
        toolResults: result.toolResults.map(tr => ({
          tool: tr.toolCall.name,
          input: tr.toolCall.input,
          result: tr.result
        }))
      }, null, 2));
    } else {
      // Human-readable output
      console.log('\n' + result.response);

      if (result.sessionId) {
        console.log(`\n💾 Session ID: ${result.sessionId}`);
        console.log(`   Use --resume ${result.sessionId} to continue this conversation`);
      }
    }

    process.exit(0);
  } catch (error) {
    if (values.json) {
      console.log(JSON.stringify({ error: error.message }));
    } else {
      console.error(`\n❌ Error: ${error.message}`);
    }
    process.exit(1);
  }
}

main();

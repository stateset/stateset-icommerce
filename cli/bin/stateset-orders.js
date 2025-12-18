#!/usr/bin/env node

/**
 * StateSet Orders Agent - Order lifecycle management specialist
 *
 * Handles order creation, status updates, shipping, and fulfillment.
 *
 * Usage:
 *   stateset-orders "show me pending orders"
 *   stateset-orders --apply "ship order #12345 with tracking FEDEX123"
 */

import { runAgentLoop, AGENTS } from '../src/claude-harness.js';
import { parseArgs } from 'node:util';

const agentConfig = AGENTS['orders'];

const HELP = `
StateSet Orders Agent - Order Lifecycle Management
${agentConfig.description}

USAGE:
  stateset-orders [options] "<request>"

OPTIONS:
  --db <path>        Path to SQLite database (default: ./store.db)
  --apply            Enable write operations
  --model <model>    Claude model to use (default: claude-sonnet-4-20250514)
  --resume <id>      Resume a previous session
  --json             Output as JSON
  --help, -h         Show this help message

ORDER STATUS FLOW:
  pending → confirmed → processing → shipped → delivered
                    ↘ cancelled / refunded

AVAILABLE TOOLS:
  • list_orders                  - List all orders
  • get_order                    - Get order with items
  • create_order                 - Create new order (--apply)
  • update_order_status          - Change status (--apply)
  • ship_order                   - Ship with tracking (--apply)
  • cancel_order                 - Cancel order (--apply)

EXAMPLES:
  # View orders
  stateset-orders "show me all pending orders"
  stateset-orders "get order #12345"
  stateset-orders "list orders for customer alice@example.com"

  # Create order
  stateset-orders --apply "create order for customer X with 2 widgets at $29.99"

  # Fulfill order
  stateset-orders --apply "confirm order #12345"
  stateset-orders --apply "mark order #12345 as processing"
  stateset-orders --apply "ship order #12345 with tracking FEDEX123456"

  # Cancel order
  stateset-orders --apply "cancel order #12345 - customer requested"

SAFETY:
  Write operations require --apply. Preview mode shows what would happen.
`;

async function main() {
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

  if (values.help) {
    console.log(HELP);
    process.exit(0);
  }

  if (values.version) {
    console.log('@stateset/cli orders-agent v0.1.2');
    process.exit(0);
  }

  const request = positionals.join(' ').trim();
  if (!request) {
    console.error('Error: No request provided');
    console.error('Usage: stateset-orders "<your request>"');
    console.error('Run stateset-orders --help for more information');
    process.exit(1);
  }

  if (!values.json) {
    console.log(`\n📦 StateSet Orders Agent`);
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
      agent: 'orders',
      onToolCall: (toolCall) => {
        if (!values.json) {
          const toolName = toolCall.name.replace('mcp__stateset-commerce__', '');
          console.log(`🔧 ${toolName}(${JSON.stringify(toolCall.input)})`);
        }
      }
    });

    if (values.json) {
      console.log(JSON.stringify({
        agent: 'orders',
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

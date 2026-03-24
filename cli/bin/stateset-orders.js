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

import { AGENTS } from '../src/claude-harness.js';
import { runMain } from '../src/graceful-shutdown.js';
import { createAgentCliMain } from '../src/utils/agent-cli.js';

const agentConfig = AGENTS['orders'];

const HELP = `
StateSet Orders Agent - Order Lifecycle Management
${agentConfig.description}

USAGE:
  stateset-orders [options] "<request>"

OPTIONS:
  --db <path>        Path to SQLite database (default: ./store.db)
  --apply            Enable write operations
  --model <model>    Claude model to use (default: see config.js)
  --provider <name>  Model provider (default: claude)
  --think <level>    Extended thinking: off, low, medium, high
  --stream           Stream partial responses
  --budget <usd>     Maximum spend per query in USD
  --memory           Enable memory
  --no-memory        Disable memory
  --x402             Enable x402 MCP tools (reads X402_* config/env)
  --resume <id>      Resume a previous session
  --json             Output as JSON
  --format <fmt>     Output format: table, json, csv, yaml (default: table)
  --output <file>    Write output to file
  --stats            Show execution stats and prompt budget
  --yes, -y          Skip confirmation prompts
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

const main = createAgentCliMain({
  agent: 'orders',
  commandName: 'stateset-orders',
  title: 'StateSet Orders Agent',
  icon: '📦',
  help: HELP,
});

runMain('stateset-orders', main);

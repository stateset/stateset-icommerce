#!/usr/bin/env node

/**
 * StateSet Inventory Agent - Stock and inventory management specialist
 *
 * Handles stock levels, adjustments, reservations, and allocation.
 *
 * Usage:
 *   stateset-inventory "how much WIDGET-001 do we have?"
 *   stateset-inventory --apply "add 50 units to WIDGET-001"
 */

import { AGENTS } from '../src/claude-harness.js';
import { runMain } from '../src/graceful-shutdown.js';
import { createAgentCliMain } from '../src/utils/agent-cli.js';

const agentConfig = AGENTS['inventory'];

const HELP = `
StateSet Inventory Agent - Stock Management
${agentConfig.description}

USAGE:
  stateset-inventory [options] "<request>"

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

KEY CONCEPTS:
  On-Hand     = Physical units in warehouse
  Allocated   = Reserved for orders (not yet shipped)
  Available   = On-Hand - Allocated (can be sold)

RESERVATION FLOW:
  [Available] → [Reserved] → [Confirmed] (deducted from on-hand)
                    ↓
               [Released] (returned to available)

AVAILABLE TOOLS:
  • get_stock                    - Check stock levels for SKU
  • create_inventory_item        - Create new inventory (--apply)
  • adjust_inventory             - Add/remove stock (--apply)
  • reserve_inventory            - Reserve for order (--apply)
  • confirm_reservation          - Confirm and deduct (--apply)
  • release_reservation          - Release reserved (--apply)

EXAMPLES:
  # Check stock
  stateset-inventory "how much WIDGET-001 do we have?"
  stateset-inventory "show me low stock items"
  stateset-inventory "what's available for SKU-001?"

  # Receive inventory
  stateset-inventory --apply "add 100 units of WIDGET-001 - received from supplier"

  # Adjust for shrinkage
  stateset-inventory --apply "remove 5 units of WIDGET-001 - damaged in warehouse"

  # Reserve for order
  stateset-inventory --apply "reserve 10 WIDGET-001 for order ORD-12345"
  stateset-inventory --apply "confirm reservation RES-67890"
  stateset-inventory --apply "release reservation RES-67890"

SAFETY:
  Write operations require --apply. Preview mode shows what would happen.
  Agent will warn if adjustment would cause negative stock.
`;

const main = createAgentCliMain({
  agent: 'inventory',
  commandName: 'stateset-inventory',
  title: 'StateSet Inventory Agent',
  icon: '📊',
  help: HELP,
});

runMain('stateset-inventory', main);

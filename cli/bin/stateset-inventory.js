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

import { runAgentLoop, AGENTS } from '../src/claude-harness.js';
import { DEFAULT_MODEL, CLI_VERSION } from '../src/config.js';
import { parseArgs } from 'node:util';

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
  --resume <id>      Resume a previous session
  --json             Output as JSON
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

async function main() {
  const { values, positionals } = parseArgs({
    options: {
      db: { type: 'string', default: './store.db' },
      apply: { type: 'boolean', default: false },
      model: { type: 'string', default: DEFAULT_MODEL },
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
    console.log(`@stateset/cli inventory-agent v${CLI_VERSION}`);
    process.exit(0);
  }

  const request = positionals.join(' ').trim();
  if (!request) {
    console.error('Error: No request provided');
    console.error('Usage: stateset-inventory "<your request>"');
    console.error('Run stateset-inventory --help for more information');
    process.exit(1);
  }

  if (!values.json) {
    console.log(`\n📊 StateSet Inventory Agent`);
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
      agent: 'inventory',
      onToolCall: (toolCall) => {
        if (!values.json) {
          const toolName = toolCall.name.replace('mcp__stateset-commerce__', '');
          console.log(`🔧 ${toolName}(${JSON.stringify(toolCall.input)})`);
        }
      }
    });

    if (values.json) {
      console.log(JSON.stringify({
        agent: 'inventory',
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

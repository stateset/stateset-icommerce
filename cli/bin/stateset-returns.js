#!/usr/bin/env node

/**
 * StateSet Returns Agent - Return request processing specialist
 *
 * Handles RMA creation, approval/rejection, and refund workflows.
 *
 * Usage:
 *   stateset-returns "show me pending returns"
 *   stateset-returns --apply "approve return RMA-12345"
 */

import { AGENTS } from '../src/claude-harness.js';
import { runMain } from '../src/graceful-shutdown.js';
import { createAgentCliMain } from '../src/utils/agent-cli.js';

const agentConfig = AGENTS['returns'];

const HELP = `
StateSet Returns Agent - RMA Processing
${agentConfig.description}

USAGE:
  stateset-returns [options] "<request>"

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

RETURN STATUS FLOW:
  requested → approved → received → refunded
          ↘ rejected

RETURN REASONS:
  • defective           - Product defect/malfunction
  • wrong_item          - Incorrect item shipped
  • not_as_described    - Differs from listing
  • changed_mind        - Customer decision
  • better_price_found  - Found cheaper elsewhere
  • no_longer_needed    - No longer wants item
  • damaged             - Arrived damaged
  • other               - Other reason

AVAILABLE TOOLS:
  • list_returns                 - List all returns
  • get_return                   - Get return details
  • create_return                - Create return request (--apply)
  • approve_return               - Approve return (--apply)
  • reject_return                - Reject with reason (--apply)
  • get_order                    - Verify original order

EXAMPLES:
  # View returns
  stateset-returns "show me all pending returns"
  stateset-returns "get return RMA-12345"
  stateset-returns "list returns for order ORD-67890"

  # Create return
  stateset-returns --apply "create return for order ORD-12345 - item is defective"

  # Process return
  stateset-returns --apply "approve return RMA-12345"
  stateset-returns --apply "reject return RMA-12345 - outside return window"

  # Check eligibility
  stateset-returns "is order ORD-12345 eligible for return?"

REFUND METHODS:
  • original_payment  - Credit to original card (3-5 days)
  • store_credit      - Account credit (immediate)
  • exchange          - Ship replacement
  • check             - Mail check (7-10 days)

SAFETY:
  Write operations require --apply. Preview mode shows what would happen.
`;

const main = createAgentCliMain({
  agent: 'returns',
  commandName: 'stateset-returns',
  title: 'StateSet Returns Agent',
  icon: '🔄',
  help: HELP,
});

runMain('stateset-returns', main);

#!/usr/bin/env node

/**
 * StateSet iCheckout Agent - Shopping cart and checkout flow specialist
 *
 * Implements the Agentic Commerce Protocol (ACP) for AI-powered checkout.
 *
 * Usage:
 *   stateset-checkout "create a cart for alice@example.com"
 *   stateset-checkout --apply "add 2 widgets at $29.99"
 *   stateset-checkout --apply --resume <id> "complete the checkout"
 */

import { AGENTS } from '../src/claude-harness.js';
import { runMain } from '../src/graceful-shutdown.js';
import { createAgentCliMain } from '../src/utils/agent-cli.js';

const agentConfig = AGENTS['checkout'];

const HELP = `
StateSet Checkout Agent - Agentic Commerce Protocol (ACP)
${agentConfig.description}

USAGE:
  stateset-checkout [options] "<request>"

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

CHECKOUT FLOW:
  1. Create Cart     - stateset-checkout --apply "create cart for alice@example.com"
  2. Add Items       - stateset-checkout --apply --resume <id> "add 2 widgets at $29.99"
  3. Set Shipping    - stateset-checkout --apply --resume <id> "ship to 123 Main St"
  4. Apply Discount  - stateset-checkout --apply --resume <id> "apply coupon SAVE10"
  5. Complete        - stateset-checkout --apply --resume <id> "complete checkout"

AVAILABLE TOOLS:
  • list_carts, get_cart              - View shopping carts
  • create_cart                       - Start new cart (--apply)
  • add_cart_item, update_cart_item   - Manage items (--apply)
  • remove_cart_item                  - Remove item (--apply)
  • set_cart_shipping_address         - Set shipping (--apply)
  • set_cart_payment                  - Set payment method (--apply)
  • apply_cart_discount               - Apply coupon (--apply)
  • get_shipping_rates                - Get shipping options
  • complete_checkout                 - Convert to order (--apply)
  • cancel_cart, abandon_cart         - End cart (--apply)
  • get_abandoned_carts               - Recovery campaigns

EXAMPLES:
  # Start a new checkout
  stateset-checkout --apply "create a cart for alice@example.com and add 3 widgets"

  # Quick checkout (all in one)
  stateset-checkout --apply "checkout: customer alice@example.com, 2 widgets at $29.99, ship to 123 Main St Anytown CA"

  # Cart recovery
  stateset-checkout "show me abandoned carts from the last week"
  stateset-checkout "what items are in cart CART-123456?"

SAFETY:
  Write operations require --apply. Preview mode shows what would happen.
`;

const main = createAgentCliMain({
  agent: 'checkout',
  commandName: 'stateset-checkout',
  title: 'StateSet Checkout Agent',
  icon: '🛒',
  resumeTarget: 'checkout',
  help: HELP,
});

runMain('stateset-checkout', main);

#!/usr/bin/env node

/**
 * StateSet Promotions Agent - Promotions and discounts management specialist
 *
 * Handles promotions, coupon codes, and discount campaigns.
 *
 * Usage:
 *   stateset-promotions "show me active promotions"
 *   stateset-promotions --apply "create a 20% off promotion called Summer Sale"
 */

import { AGENTS } from '../src/claude-harness.js';
import { runMain } from '../src/graceful-shutdown.js';
import { createAgentCliMain } from '../src/utils/agent-cli.js';

const agentConfig = AGENTS['promotions'];

const HELP = `
StateSet Promotions Agent - Promotions & Discounts Management
${agentConfig.description}

USAGE:
  stateset-promotions [options] "<request>"

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

PROMOTION TYPES:
  - percentage_off   Percentage discount (e.g., 20% off)
  - fixed_amount_off Fixed dollar discount (e.g., $10 off)
  - buy_x_get_y      BOGO promotions
  - free_shipping    Free shipping offers
  - tiered_discount  Spend more, save more

PROMOTION LIFECYCLE:
  draft → active → (paused) → expired

AVAILABLE TOOLS:
  • list_promotions          - List all promotions
  • get_promotion            - Get promotion details
  • create_promotion         - Create promotion (--apply)
  • activate_promotion       - Make promotion live (--apply)
  • deactivate_promotion     - Pause promotion (--apply)
  • create_coupon            - Create coupon code (--apply)
  • validate_coupon          - Check if coupon is valid
  • list_coupons             - List coupon codes
  • get_active_promotions    - Get active promotions
  • apply_cart_promotions    - Apply to cart (--apply)

EXAMPLES:
  # View promotions
  stateset-promotions "show me all active promotions"
  stateset-promotions "list all coupon codes"
  stateset-promotions "is coupon SUMMER25 valid?"

  # Create promotions
  stateset-promotions --apply "create a 20% off promotion called Summer Sale"
  stateset-promotions --apply "create a $10 off promotion for orders over $50"
  stateset-promotions --apply "create a free shipping promotion"

  # Manage promotions
  stateset-promotions --apply "activate the Summer Sale promotion"
  stateset-promotions --apply "pause promotion <id>"

  # Create coupon codes
  stateset-promotions --apply "create coupon code SAVE20 for the Summer Sale promotion with limit 100 uses"

  # Apply to cart
  stateset-promotions --apply "apply promotions to cart <cart-id>"

SAFETY:
  Write operations require --apply. Preview mode shows what would happen.
`;

const main = createAgentCliMain({
  agent: 'promotions',
  commandName: 'stateset-promotions',
  title: 'StateSet Promotions Agent',
  icon: '🏷️',
  help: HELP,
});

runMain('stateset-promotions', main);

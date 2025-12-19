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

import { runAgentLoop, AGENTS } from '../src/claude-harness.js';
import { DEFAULT_MODEL, CLI_VERSION } from '../src/config.js';
import { parseArgs } from 'node:util';

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
  --resume <id>      Resume a previous session
  --json             Output as JSON
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
    console.log(`@stateset/cli promotions-agent v${CLI_VERSION}`);
    process.exit(0);
  }

  const request = positionals.join(' ').trim();
  if (!request) {
    console.error('Error: No request provided');
    console.error('Usage: stateset-promotions "<your request>"');
    console.error('Run stateset-promotions --help for more information');
    process.exit(1);
  }

  if (!values.json) {
    console.log(`\n🏷️  StateSet Promotions Agent`);
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
      agent: 'promotions',
      onToolCall: (toolCall) => {
        if (!values.json) {
          const toolName = toolCall.name.replace('mcp__stateset-commerce__', '');
          console.log(`🔧 ${toolName}(${JSON.stringify(toolCall.input)})`);
        }
      }
    });

    if (values.json) {
      console.log(JSON.stringify({
        agent: 'promotions',
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

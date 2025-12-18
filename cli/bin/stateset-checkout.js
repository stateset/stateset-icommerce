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

import { runAgentLoop, AGENTS } from '../src/claude-harness.js';
import { parseArgs } from 'node:util';

const agentConfig = AGENTS['checkout'];

const HELP = `
StateSet Checkout Agent - Agentic Commerce Protocol (ACP)
${agentConfig.description}

USAGE:
  stateset-checkout [options] "<request>"

OPTIONS:
  --db <path>        Path to SQLite database (default: ./store.db)
  --apply            Enable write operations
  --model <model>    Claude model to use (default: claude-sonnet-4-20250514)
  --resume <id>      Resume a previous session
  --json             Output as JSON
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
    console.log('@stateset/cli checkout-agent v0.1.2');
    process.exit(0);
  }

  const request = positionals.join(' ').trim();
  if (!request) {
    console.error('Error: No request provided');
    console.error('Usage: stateset-checkout "<your request>"');
    console.error('Run stateset-checkout --help for more information');
    process.exit(1);
  }

  if (!values.json) {
    console.log(`\n🛒 StateSet Checkout Agent`);
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
      agent: 'checkout',
      onToolCall: (toolCall) => {
        if (!values.json) {
          const toolName = toolCall.name.replace('mcp__stateset-commerce__', '');
          console.log(`🔧 ${toolName}(${JSON.stringify(toolCall.input)})`);
        }
      }
    });

    if (values.json) {
      console.log(JSON.stringify({
        agent: 'checkout',
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
        console.log(`   Use --resume ${result.sessionId} to continue this checkout`);
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

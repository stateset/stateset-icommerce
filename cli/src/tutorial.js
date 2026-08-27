/**
 * Interactive Tutorial/Onboarding System for StateSet CLI
 *
 * Provides guided walkthroughs for new users to learn the CLI.
 */

import * as readline from 'node:readline';

/**
 * Tutorial steps organized by topic
 */
export const TUTORIALS = {
  quickstart: {
    name: 'Quick Start',
    description: 'Learn the basics in 5 minutes',
    steps: [
      {
        title: 'Welcome to StateSet CLI',
        content: `
StateSet CLI is an AI-powered command-line interface for commerce operations.

You can interact with it using natural language:
  $ stateset "list all customers"
  $ stateset "show me pending orders"
  $ stateset "what's in stock for SKU-001?"

The AI understands your intent and calls the right tools automatically.
`,
        action: null,
      },
      {
        title: 'Read vs Write Operations',
        content: `
StateSet CLI has a safety-first design:

  READ operations (safe, always allowed):
    $ stateset "list customers"
    $ stateset "show order ORD-123"
    $ stateset "check stock levels"

  WRITE operations (require --apply flag):
    $ stateset --apply "create a customer named Alice"
    $ stateset --apply "ship order ORD-123 with tracking FEDEX456"

Without --apply, write operations show a PREVIEW of what would happen.
`,
        action: null,
      },
      {
        title: 'Try a Read Operation',
        content: `
Let's try listing customers. Run this command:

  $ stateset "list all customers"

This will show any customers in your database.
If you don't have any yet, don't worry - we'll create some next!
`,
        tryCommand: 'stateset "list all customers"',
      },
      {
        title: 'Preview Mode',
        content: `
Now try a write operation WITHOUT --apply:

  $ stateset "create a customer named Tutorial User with email tutorial@example.com"

You'll see a preview of what WOULD happen, but no data is changed.
This is great for testing commands before running them for real.
`,
        tryCommand:
          'stateset "create a customer named Tutorial User with email tutorial@example.com"',
      },
      {
        title: 'Executing Write Operations',
        content: `
To actually create data, add the --apply flag:

  $ stateset --apply "create a customer named Tutorial User with email tutorial@example.com"

The AI will create the customer and return the new customer ID.
`,
        tryCommand:
          'stateset --apply "create a customer named Tutorial User with email tutorial@example.com"',
      },
      {
        title: 'Specialized Agents',
        content: `
StateSet CLI has specialized agents for different domains:

  $ stateset-orders "show pending orders"
  $ stateset-inventory "what's low on stock?"
  $ stateset-returns "list pending returns"
  $ stateset-analytics "show me sales this month"
  $ stateset-checkout "create a cart for alice@example.com"

These agents have focused tools and knowledge for their domain.
`,
        action: null,
      },
      {
        title: 'Session Context',
        content: `
For multi-step workflows, use sessions with --resume:

  $ stateset --apply "create a cart for alice@example.com"
  # Returns: Session ID: abc-123-def

  $ stateset --apply --resume abc-123-def "add 2 widgets at $29.99"
  $ stateset --apply --resume abc-123-def "complete the checkout"

Sessions remember context, so you can build on previous operations.
`,
        action: null,
      },
      {
        title: 'Getting Help',
        content: `
You're all set! Here are some helpful commands:

  $ stateset --help           # Show all options
  $ stateset-doctor           # Check your setup
  $ stateset-direct --help    # Direct commands (no AI)

Run specific tutorials for deeper dives:
  $ stateset-tutorial orders      # Order management
  $ stateset-tutorial inventory   # Stock management
  $ stateset-tutorial checkout    # Shopping cart flow
  $ stateset-tutorial analytics   # Business intelligence
`,
        action: null,
      },
    ],
  },

  orders: {
    name: 'Order Management',
    description: 'Learn order lifecycle management',
    steps: [
      {
        title: 'Order Management Overview',
        content: `
Orders flow through these states:
  pending → confirmed → processing → shipped → delivered
                     ↘ cancelled

The orders agent helps you manage this entire lifecycle.
`,
        action: null,
      },
      {
        title: 'Listing Orders',
        content: `
View your orders with natural language:

  $ stateset-orders "show all orders"
  $ stateset-orders "list pending orders"
  $ stateset-orders "find orders for customer alice@example.com"
`,
        tryCommand: 'stateset-orders "list all orders"',
      },
      {
        title: 'Creating Orders',
        content: `
Create orders with the --apply flag:

  $ stateset --apply "create an order for alice@example.com with 2 widgets at $29.99"

The order will be created in 'pending' status.
`,
        action: null,
      },
      {
        title: 'Shipping Orders',
        content: `
Ship orders with tracking:

  $ stateset-orders --apply "ship order ORD-123 with tracking FEDEX456789"

This updates the order status to 'shipped' and records the tracking number.
`,
        action: null,
      },
      {
        title: 'Order Analytics',
        content: `
Get insights about your orders:

  $ stateset-analytics "how many orders this month?"
  $ stateset-analytics "what's my average order value?"
  $ stateset-analytics "show orders by status"
`,
        action: null,
      },
    ],
  },

  inventory: {
    name: 'Inventory Management',
    description: 'Learn stock tracking and reservations',
    steps: [
      {
        title: 'Inventory Concepts',
        content: `
Key inventory concepts:

  On-Hand:   Physical inventory in warehouse
  Allocated: Reserved for orders but not yet shipped
  Available: On-hand minus allocated (what can be sold)

  Formula: Available = On-Hand - Allocated
`,
        action: null,
      },
      {
        title: 'Checking Stock',
        content: `
Check stock levels with natural language:

  $ stateset-inventory "how much WIDGET-001 do we have?"
  $ stateset-inventory "show me low stock items"
  $ stateset-inventory "what's out of stock?"
`,
        tryCommand: 'stateset-inventory "show inventory health"',
      },
      {
        title: 'Adjusting Inventory',
        content: `
Adjust stock with reasons:

  $ stateset-inventory --apply "add 100 units to WIDGET-001 - received shipment"
  $ stateset-inventory --apply "remove 5 units from WIDGET-001 - damaged goods"

Always document the reason for inventory changes.
`,
        action: null,
      },
      {
        title: 'Reservations',
        content: `
Reserve inventory for orders:

  $ stateset-inventory --apply "reserve 10 WIDGET-001 for order ORD-123"
  $ stateset-inventory --apply "confirm reservation for order ORD-123"
  $ stateset-inventory --apply "release reservation for order ORD-123"

Reservations prevent overselling.
`,
        action: null,
      },
    ],
  },

  checkout: {
    name: 'Checkout Flow',
    description: 'Learn the protocol-neutral cart and checkout lifecycle',
    steps: [
      {
        title: 'Cart and Checkout Lifecycle',
        content: `
The checkout agent operates the embedded commerce lifecycle:

  1. Create Cart     → Start a shopping session
  2. Add Items       → Add products to cart
  3. Set Shipping    → Provide shipping address
  4. Apply Discounts → Optional coupon codes
  5. Complete        → Convert cart to order

Each step uses the --apply flag for write operations.
`,
        action: null,
      },
      {
        title: 'Creating a Cart',
        content: `
Start a checkout session:

  $ stateset-checkout --apply "create a cart for alice@example.com"
  # Returns cart ID and session ID

Use --resume for subsequent operations to maintain context.
`,
        tryCommand: 'stateset-checkout "list active carts"',
      },
      {
        title: 'Adding Items',
        content: `
Add products to the cart:

  $ stateset-checkout --apply --resume <session-id> "add 2 widgets at $29.99"
  $ stateset-checkout --apply --resume <session-id> "add 1 premium gadget at $99.99"

The cart total updates automatically.
`,
        action: null,
      },
      {
        title: 'Completing Checkout',
        content: `
Set shipping and complete:

  $ stateset-checkout --apply --resume <id> "set shipping to 123 Main St, Anytown CA 90210"
  $ stateset-checkout --apply --resume <id> "complete the checkout"

The cart becomes an order!
`,
        action: null,
      },
      {
        title: 'Cart Recovery',
        content: `
Recover abandoned carts:

  $ stateset-checkout "show abandoned carts"
  $ stateset-checkout "what's in cart CART-123456?"

Great for cart abandonment campaigns.
`,
        action: null,
      },
    ],
  },

  analytics: {
    name: 'Analytics & Forecasting',
    description: 'Learn business intelligence features',
    steps: [
      {
        title: 'Analytics Overview',
        content: `
The analytics agent provides business intelligence:

  - Sales metrics (revenue, orders, AOV)
  - Customer insights (top customers, retention)
  - Inventory health (stock levels, low stock)
  - Forecasting (demand, revenue predictions)

All analytics are read-only - no --apply needed.
`,
        action: null,
      },
      {
        title: 'Sales Metrics',
        content: `
Get sales performance data:

  $ stateset-analytics "what's my total revenue this month?"
  $ stateset-analytics "show me sales for the last 7 days"
  $ stateset-analytics "what's my average order value?"
`,
        tryCommand: 'stateset-analytics "show sales summary"',
      },
      {
        title: 'Top Performers',
        content: `
Find your best products and customers:

  $ stateset-analytics "what are my top selling products?"
  $ stateset-analytics "who are my VIP customers?"
  $ stateset-analytics "show top 10 customers by spend"
`,
        action: null,
      },
      {
        title: 'Forecasting',
        content: `
Predict future trends:

  $ stateset-analytics "forecast revenue for next month"
  $ stateset-analytics "predict demand for WIDGET-001"

Forecasts include confidence intervals.
`,
        action: null,
      },
    ],
  },
};

/**
 * TutorialRunner - Runs interactive tutorials
 */
export class TutorialRunner {
  constructor(options = {}) {
    this.output = options.output || console;
    this.interactive = options.interactive !== false;
  }

  /**
   * List available tutorials
   */
  listTutorials() {
    console.info('\nAvailable Tutorials:\n');

    for (const [id, tutorial] of Object.entries(TUTORIALS)) {
      console.info(`  ${id.padEnd(15)} ${tutorial.name}`);
      console.info(`  ${' '.repeat(15)} ${this.dim(tutorial.description)}`);
      console.info();
    }

    console.info('Run a tutorial:');
    console.info('  $ stateset-tutorial quickstart');
    console.info('  $ stateset-tutorial orders');
    console.info();
  }

  /**
   * Run a tutorial
   */
  async run(tutorialId) {
    const tutorial = TUTORIALS[tutorialId];

    if (!tutorial) {
      console.error(`Unknown tutorial: ${tutorialId}`);
      this.listTutorials();
      return false;
    }

    console.info(`\n${this.bold('═'.repeat(60))}`);
    console.info(`${this.bold(`  ${tutorial.name}`)}`);
    console.info(`  ${this.dim(tutorial.description)}`);
    console.info(`${this.bold('═'.repeat(60))}\n`);

    for (let i = 0; i < tutorial.steps.length; i++) {
      const step = tutorial.steps[i];

      console.info(
        `${this.cyan(`Step ${i + 1}/${tutorial.steps.length}:`)} ${this.bold(step.title)}`,
      );
      console.info(`${this.dim('─'.repeat(50))}`);
      console.info(step.content);

      if (step.tryCommand) {
        console.info(`${this.yellow('Try it:')} ${step.tryCommand}\n`);
      }

      if (i < tutorial.steps.length - 1) {
        if (this.interactive) {
          const shouldContinue = await this.promptContinue();
          if (!shouldContinue) {
            console.info('\nTutorial paused. Run again to continue.');
            return false;
          }
        }
        console.info();
      }
    }

    console.info(`${this.green('✓')} Tutorial complete!\n`);
    return true;
  }

  /**
   * Prompt user to continue
   */
  async promptContinue() {
    return new Promise((resolve) => {
      const rl = readline.createInterface({
        input: process.stdin,
        output: process.stdout,
      });

      rl.question(`${this.dim('Press Enter to continue (q to quit)...')} `, (answer) => {
        rl.close();
        resolve(answer.toLowerCase() !== 'q');
      });
    });
  }

  // Color helpers
  bold(text) {
    return `\x1b[1m${text}\x1b[0m`;
  }
  dim(text) {
    return `\x1b[90m${text}\x1b[0m`;
  }
  cyan(text) {
    return `\x1b[36m${text}\x1b[0m`;
  }
  green(text) {
    return `\x1b[32m${text}\x1b[0m`;
  }
  yellow(text) {
    return `\x1b[33m${text}\x1b[0m`;
  }
}

/**
 * Create a tutorial runner
 */
export function createTutorialRunner(options = {}) {
  return new TutorialRunner(options);
}

/**
 * Check if this is first run and prompt for tutorial
 */
export async function checkFirstRun() {
  const fs = await import('node:fs');
  const path = await import('node:path');
  const os = await import('node:os');

  const markerPath = path.join(os.homedir(), '.stateset', '.tutorial-shown');

  if (fs.existsSync(markerPath)) {
    return false;
  }

  // Create marker
  const dir = path.dirname(markerPath);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
  fs.writeFileSync(markerPath, new Date().toISOString());

  return true;
}

/**
 * Show first-run welcome message
 */
export function showWelcome() {
  console.info(`
\x1b[36m╔══════════════════════════════════════════════════════════╗
║                                                          ║
║   Welcome to StateSet CLI! 🚀                            ║
║                                                          ║
║   AI-powered commerce operations at your fingertips.     ║
║                                                          ║
╚══════════════════════════════════════════════════════════╝\x1b[0m

\x1b[1mQuick Start:\x1b[0m
  $ stateset "list all customers"          \x1b[90m# Read operation\x1b[0m
  $ stateset --apply "create a customer"   \x1b[90m# Write operation\x1b[0m

\x1b[1mRun the tutorial:\x1b[0m
  $ stateset-tutorial quickstart

\x1b[1mCheck your setup:\x1b[0m
  $ stateset-doctor

\x1b[90mThis message won't appear again.\x1b[0m
`);
}

export default {
  TUTORIALS,
  TutorialRunner,
  createTutorialRunner,
  checkFirstRun,
  showWelcome,
};

#!/usr/bin/env node

/**
 * StateSet Subscriptions Agent - Subscription management specialist
 *
 * Handles subscription plans, customer subscriptions, billing cycles.
 *
 * Usage:
 *   stateset-subscriptions "show me active subscription plans"
 *   stateset-subscriptions --apply "create a monthly subscription plan"
 */

import { AGENTS } from '../src/claude-harness.js';
import { runMain } from '../src/graceful-shutdown.js';
import { createAgentCliMain } from '../src/utils/agent-cli.js';

const agentConfig = AGENTS['subscriptions'];

const HELP = `
StateSet Subscriptions Agent - Recurring Billing & Subscription Management
${agentConfig.description}

USAGE:
  stateset-subscriptions [options] "<request>"

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

BILLING INTERVALS:
  - weekly           Billed every week
  - biweekly         Billed every 2 weeks
  - monthly          Billed every month
  - quarterly        Billed every 3 months
  - annual           Billed yearly

SUBSCRIPTION LIFECYCLE:
  pending -> trial -> active -> (paused) -> cancelled -> expired

AVAILABLE TOOLS:
  Plans:
  • list_subscription_plans     - List all plans
  • get_subscription_plan       - Get plan details
  • create_subscription_plan    - Create plan (--apply)
  • activate_subscription_plan  - Make plan available (--apply)
  • archive_subscription_plan   - Retire a plan (--apply)

  Subscriptions:
  • list_subscriptions          - List subscriptions
  • get_subscription            - Get subscription details
  • create_subscription         - Subscribe customer (--apply)
  • pause_subscription          - Pause billing (--apply)
  • resume_subscription         - Resume subscription (--apply)
  • cancel_subscription         - Cancel subscription (--apply)
  • skip_billing_cycle          - Skip next billing (--apply)

  Billing:
  • list_billing_cycles         - View billing history
  • get_billing_cycle           - Get cycle details
  • get_subscription_events     - View audit log

EXAMPLES:
  # View plans
  stateset-subscriptions "show me all subscription plans"
  stateset-subscriptions "list active plans"
  stateset-subscriptions "get details for plan <id>"

  # Create plans
  stateset-subscriptions --apply "create a monthly plan called Coffee Club at $29.99 with 14 day trial"
  stateset-subscriptions --apply "create an annual plan called Pro Membership at $99.99"
  stateset-subscriptions --apply "activate plan <id>"

  # Manage subscriptions
  stateset-subscriptions "show me all active subscriptions"
  stateset-subscriptions "list subscriptions for customer <id>"
  stateset-subscriptions --apply "subscribe customer <id> to the Coffee Club plan"

  # Lifecycle operations
  stateset-subscriptions --apply "pause subscription <id>"
  stateset-subscriptions --apply "resume subscription <id>"
  stateset-subscriptions --apply "cancel subscription <id>"
  stateset-subscriptions --apply "skip next billing for subscription <id>"

  # View history
  stateset-subscriptions "show billing history for subscription <id>"
  stateset-subscriptions "get events for subscription <id>"

SAFETY:
  Write operations require --apply. Preview mode shows what would happen.
`;

const main = createAgentCliMain({
  agent: 'subscriptions',
  commandName: 'stateset-subscriptions',
  title: 'StateSet Subscriptions Agent',
  icon: '🔄',
  help: HELP,
});

runMain('stateset-subscriptions', main);

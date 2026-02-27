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

import { runAgentLoop, AGENTS } from '../src/claude-harness.js';
import { createConfirmHandler } from '../src/utils/confirm.js';
import {
  buildAgentOutputData,
  resolveOutputFormat,
  writeAgentOutputFile,
} from '../src/utils/agent-output.js';
import { resolveAgentRuntimeOptions, createStreamingHandler } from '../src/utils/agent-runtime.js';
import { DEFAULT_MODEL, CLI_VERSION } from '../src/config.js';
import { parseArgs } from 'node:util';

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

async function main() {
  const { values, positionals } = parseArgs({
    options: {
      db: { type: 'string', default: './store.db' },
      apply: { type: 'boolean', default: false },
      model: { type: 'string', default: DEFAULT_MODEL },
      provider: { type: 'string' },
      think: { type: 'string', default: 'off' },
      stream: { type: 'boolean', default: false },
      budget: { type: 'string' },
      memory: { type: 'boolean', default: false },
      'no-memory': { type: 'boolean', default: false },
      x402: { type: 'boolean', default: false },
      resume: { type: 'string' },
      json: { type: 'boolean', default: false },
      format: { type: 'string', default: 'table' },
      output: { type: 'string' },
      yes: { type: 'boolean', short: 'y', default: false },
      help: { type: 'boolean', short: 'h', default: false },
      version: { type: 'boolean', short: 'v', default: false },
    },
    allowPositionals: true,
  });

  if (values.help) {
    console.log(HELP);
    process.exit(0);
  }

  if (values.version) {
    console.log(`@stateset/cli subscriptions-agent v${CLI_VERSION}`);
    process.exit(0);
  }

  const request = positionals.join(' ').trim();
  if (!request) {
    console.error('Error: No request provided');
    console.error('Usage: stateset-subscriptions "<your request>"');
    console.error('Run stateset-subscriptions --help for more information');
    process.exit(1);
  }

  const outputFormat = resolveOutputFormat({
    format: values.format,
    json: values.json,
    argv: process.argv,
  });
  const isJsonOutput = outputFormat === 'json';

  if (values.stream && isJsonOutput) {
    console.error(
      'Error: --stream cannot be used with JSON output. Remove --stream or use a non-JSON format.',
    );
    process.exit(1);
  }

  let runtimeOptions;
  try {
    runtimeOptions = resolveAgentRuntimeOptions(values);
  } catch (error) {
    if (isJsonOutput) {
      console.log(JSON.stringify({ error: error.message }));
    } else {
      console.error(`\n❌ Error: ${error.message}`);
    }
    process.exit(1);
  }

  const { thinkLevel, providerName, streaming, maxBudgetUsd, memoryOverride, enableX402 } =
    runtimeOptions;

  if (!isJsonOutput) {
    console.log(`\n🔄 StateSet Subscriptions Agent`);
    console.log(`   Database: ${values.db}`);
    console.log(`   Mode: ${values.apply ? '✏️  Write enabled' : '👁️  Preview only'}`);
    if (values.resume) {
      console.log(`   Session: ${values.resume}`);
    }
    console.log();
  }

  try {
    const nonInteractive = !process.stdin.isTTY || isJsonOutput;
    const onConfirmRequired = createConfirmHandler({
      output: null,
      assumeYes: values.yes,
      nonInteractive,
    });

    const result = await runAgentLoop({
      request,
      dbPath: values.db,
      model: values.model,
      allowApply: values.apply,
      resumeSessionId: values.resume,
      agent: 'subscriptions',
      onConfirmRequired,
      thinkLevel,
      streaming,
      maxBudgetUsd,
      provider: providerName,
      enableMemory: memoryOverride === null ? null : memoryOverride,
      enableX402,
      onPartialMessage: createStreamingHandler(streaming),
      onToolCall: (toolCall) => {
        if (!isJsonOutput) {
          const toolName = toolCall.name.replace('mcp__stateset-commerce__', '');
          console.log(`🔧 ${toolName}(${JSON.stringify(toolCall.input)})`);
        }
      },
    });

    const outputData = buildAgentOutputData({
      agent: 'subscriptions',
      request,
      allowApply: values.apply,
      result,
    });

    if (values.output) {
      await writeAgentOutputFile(values.output, outputData, outputFormat);
      if (!isJsonOutput) {
        console.log(`Output written to ${values.output}`);
      }
    } else if (isJsonOutput) {
      console.log(JSON.stringify(outputData, null, 2));
    } else {
      if (streaming && result.response) {
        console.log();
      } else {
        console.log('\n' + result.response);
      }

      if (result.sessionId) {
        console.log(`\n💾 Session ID: ${result.sessionId}`);
        console.log(`   Use --resume ${result.sessionId} to continue this conversation`);
      }
    }

    process.exit(0);
  } catch (error) {
    if (isJsonOutput) {
      console.log(JSON.stringify({ error: error.message }));
    } else {
      console.error(`\n❌ Error: ${error.message}`);
    }
    process.exit(1);
  }
}

import { runMain } from '../src/graceful-shutdown.js';
runMain('stateset-subscriptions', main);

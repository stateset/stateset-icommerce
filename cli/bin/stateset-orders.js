#!/usr/bin/env node

/**
 * StateSet Orders Agent - Order lifecycle management specialist
 *
 * Handles order creation, status updates, shipping, and fulfillment.
 *
 * Usage:
 *   stateset-orders "show me pending orders"
 *   stateset-orders --apply "ship order #12345 with tracking FEDEX123"
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

const agentConfig = AGENTS['orders'];

const HELP = `
StateSet Orders Agent - Order Lifecycle Management
${agentConfig.description}

USAGE:
  stateset-orders [options] "<request>"

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

ORDER STATUS FLOW:
  pending → confirmed → processing → shipped → delivered
                    ↘ cancelled / refunded

AVAILABLE TOOLS:
  • list_orders                  - List all orders
  • get_order                    - Get order with items
  • create_order                 - Create new order (--apply)
  • update_order_status          - Change status (--apply)
  • ship_order                   - Ship with tracking (--apply)
  • cancel_order                 - Cancel order (--apply)

EXAMPLES:
  # View orders
  stateset-orders "show me all pending orders"
  stateset-orders "get order #12345"
  stateset-orders "list orders for customer alice@example.com"

  # Create order
  stateset-orders --apply "create order for customer X with 2 widgets at $29.99"

  # Fulfill order
  stateset-orders --apply "confirm order #12345"
  stateset-orders --apply "mark order #12345 as processing"
  stateset-orders --apply "ship order #12345 with tracking FEDEX123456"

  # Cancel order
  stateset-orders --apply "cancel order #12345 - customer requested"

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
      noMemory: { type: 'boolean', default: false },
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
    console.log(`@stateset/cli orders-agent v${CLI_VERSION}`);
    process.exit(0);
  }

  const request = positionals.join(' ').trim();
  if (!request) {
    console.error('Error: No request provided');
    console.error('Usage: stateset-orders "<your request>"');
    console.error('Run stateset-orders --help for more information');
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
    console.log(`\n📦 StateSet Orders Agent`);
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
      agent: 'orders',
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
      agent: 'orders',
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
runMain('stateset-orders', main);

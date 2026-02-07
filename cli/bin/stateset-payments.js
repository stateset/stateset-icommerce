#!/usr/bin/env node
/**
 * StateSet Payments Agent
 *
 * Payment processing and refund management specialist.
 *
 * Usage:
 *   stateset-payments "list all payments"
 *   stateset-payments "show payment PAY-123"
 *   stateset-payments --apply "create payment for order ORD-456"
 *   stateset-payments --apply "refund payment PAY-123"
 */

import { runAgentLoop } from '../src/claude-harness.js';
import { createConfirmHandler } from '../src/utils/confirm.js';
import {
  buildAgentOutputData,
  resolveOutputFormat,
  writeAgentOutputFile,
} from '../src/utils/agent-output.js';
import { resolveAgentRuntimeOptions, createStreamingHandler } from '../src/utils/agent-runtime.js';
import { parseArgs } from 'node:util';
import { installShutdownHandlers } from '../src/graceful-shutdown.js';
installShutdownHandlers('stateset-payments');

const options = {
  apply: { type: 'boolean', default: false },
  db: { type: 'string', default: './store.db' },
  json: { type: 'boolean', default: false },
  format: { type: 'string', default: 'table' },
  output: { type: 'string' },
  provider: { type: 'string' },
  think: { type: 'string', default: 'off' },
  stream: { type: 'boolean', default: false },
  budget: { type: 'string' },
  memory: { type: 'boolean', default: false },
  noMemory: { type: 'boolean', default: false },
  x402: { type: 'boolean', default: false },
  model: { type: 'string' },
  resume: { type: 'string' },
  yes: { type: 'boolean', short: 'y', default: false },
  help: { type: 'boolean', short: 'h', default: false },
};

const { values, positionals } = parseArgs({ options, allowPositionals: true });

if (values.help) {
  console.log(`
StateSet Payments Agent - Payment Processing

Usage:
  stateset-payments [options] "<request>"

Options:
  --apply          Enable write operations (default: preview only)
  --db <path>      Database path (default: ./store.db)
  --json           JSON output
  --format <fmt>   Output format: table, json, csv, yaml (default: table)
  --output <file>  Write output to file
  --provider <name>  Model provider (default: claude)
  --think <level>    Extended thinking: off, low, medium, high
  --stream           Stream partial responses
  --budget <usd>     Maximum spend per query in USD
  --memory           Enable memory
  --no-memory        Disable memory
  --x402             Enable x402 MCP tools (reads X402_* config/env)
  --model <name>   Claude model to use
  --resume <id>    Resume session
  --yes, -y        Skip confirmation prompts
  -h, --help       Show this help

Examples:
  stateset-payments "list all payments"
  stateset-payments "show payment PAY-123"
  stateset-payments "list pending payments"
  stateset-payments --apply "create payment for order ORD-456"
  stateset-payments --apply "complete payment PAY-123"
  stateset-payments --apply "refund payment PAY-123"
  stateset-payments --apply "refund $50 from payment PAY-123"

Payment Methods:
  credit_card    Credit/debit card payment
  ach            Bank transfer
  wallet         Digital wallet (Apple Pay, Google Pay, etc.)
  cash           Cash on delivery
  invoice        B2B invoicing (net terms)

Payment Status Flow:
  pending → processing → completed → refunded
         ↘ failed
`);
  process.exit(0);
}

const request = positionals.join(' ');

if (!request) {
  console.error('Usage: stateset-payments [options] "<request>"');
  console.error('Run with --help for more information');
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
    console.error(`Error: ${error.message}`);
  }
  process.exit(1);
}

const { thinkLevel, providerName, streaming, maxBudgetUsd, memoryOverride, enableX402 } =
  runtimeOptions;

try {
  const nonInteractive = !process.stdin.isTTY || isJsonOutput;
  const onConfirmRequired = createConfirmHandler({
    output: null,
    assumeYes: values.yes,
    nonInteractive,
  });

  if (!isJsonOutput) {
    console.log('\nStateSet Payments Agent');
    console.log(`   Database: ${values.db}`);
    console.log(`   Mode: ${values.apply ? 'Write enabled' : 'Preview only'}`);
    if (values.resume) {
      console.log(`   Session: ${values.resume}`);
    }
    console.log();
  }

  const result = await runAgentLoop({
    agent: 'payments',
    request,
    dbPath: values.db,
    model: values.model,
    allowApply: values.apply,
    resumeSessionId: values.resume,
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
        console.log(`Tool: ${toolName}(${JSON.stringify(toolCall.input)})`);
      }
    },
  });

  const outputData = buildAgentOutputData({
    agent: 'payments',
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
      console.log(`\nSession ID: ${result.sessionId}`);
      console.log(`   Use --resume ${result.sessionId} to continue this conversation`);
    }
  }
} catch (error) {
  if (isJsonOutput) {
    console.log(JSON.stringify({ error: error.message }));
  } else {
    console.error(`Error: ${error.message}`);
  }
  process.exit(1);
}

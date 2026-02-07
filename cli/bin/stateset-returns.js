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
    console.log(`@stateset/cli returns-agent v${CLI_VERSION}`);
    process.exit(0);
  }

  const request = positionals.join(' ').trim();
  if (!request) {
    console.error('Error: No request provided');
    console.error('Usage: stateset-returns "<your request>"');
    console.error('Run stateset-returns --help for more information');
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
    console.log(`\n🔄 StateSet Returns Agent`);
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
      agent: 'returns',
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
      agent: 'returns',
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
runMain('stateset-returns', main);

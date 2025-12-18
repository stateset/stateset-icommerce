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
  --model <model>    Claude model to use (default: claude-sonnet-4-20250514)
  --resume <id>      Resume a previous session
  --json             Output as JSON
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
    console.log('@stateset/cli returns-agent v0.1.2');
    process.exit(0);
  }

  const request = positionals.join(' ').trim();
  if (!request) {
    console.error('Error: No request provided');
    console.error('Usage: stateset-returns "<your request>"');
    console.error('Run stateset-returns --help for more information');
    process.exit(1);
  }

  if (!values.json) {
    console.log(`\n🔄 StateSet Returns Agent`);
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
      agent: 'returns',
      onToolCall: (toolCall) => {
        if (!values.json) {
          const toolName = toolCall.name.replace('mcp__stateset-commerce__', '');
          console.log(`🔧 ${toolName}(${JSON.stringify(toolCall.input)})`);
        }
      }
    });

    if (values.json) {
      console.log(JSON.stringify({
        agent: 'returns',
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

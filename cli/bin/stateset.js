#!/usr/bin/env node

/**
 * StateSet iCommerce CLI - AI-powered commerce operations
 *
 * Usage:
 *   stateset "show me all customers"
 *   stateset --apply "create a customer with email alice@example.com"
 *   stateset --db ./mystore.db "list all orders"
 *   stateset --resume <session-id> "now ship that order"
 *   stateset --verbose "debug agent routing"
 */

import { runAgentLoop, RichOutput, ICONS, AgentTelemetry } from '../src/claude-harness.js';
import { DEFAULT_MODEL, CLI_VERSION } from '../src/config.js';
import { parseArgs } from 'node:util';

const HELP = `
StateSet iCommerce CLI - AI-powered commerce operations

USAGE:
  stateset [options] "<request>"

OPTIONS:
  --db <path>        Path to SQLite database (default: ./store.db)
  --apply            Enable write operations (create, update, delete)
  --model <model>    Claude model to use (default: see config.js)
  --resume <id>      Resume a previous session
  --json             Output as JSON
  --verbose, -V      Enable verbose output with telemetry
  --stats            Show execution statistics after completion
  --help, -h         Show this help message
  --version, -v      Show version

SPECIALIZED AGENTS:
  stateset-checkout    Shopping cart & checkout flow (ACP)
  stateset-orders      Order lifecycle management
  stateset-inventory   Stock & reservation management
  stateset-returns     RMA & refund processing

  Use specialized agents for focused workflows with domain-specific tooling.
  The main 'stateset' command auto-routes to the best agent.

EXAMPLES:
  # List customers (read-only)
  stateset "show me all customers"

  # Check inventory
  stateset "how much stock do we have of SKU-001?"

  # Create a customer (requires --apply)
  stateset --apply "create a customer named Alice Smith with email alice@example.com"

  # Create and ship an order
  stateset --apply "create an order for customer X with 2 widgets at $29.99 each"
  stateset --apply --resume <session-id> "now ship that order with tracking ABC123"

  # Shopping cart checkout flow (ACP)
  stateset --apply "create a cart for alice@example.com"
  stateset --apply --resume <session-id> "add 2 widgets at $29.99"
  stateset --apply --resume <session-id> "set shipping to 123 Main St, Anytown, CA"
  stateset --apply --resume <session-id> "complete the checkout"

  # Cart recovery
  stateset "show me abandoned carts"

  # Use a different database
  stateset --db ./production.db "list recent orders"

SAFETY:
  By default, all write operations are blocked. Use --apply to enable them.
  The CLI will always show you what would happen before making changes.
`;

async function main() {
  // Parse arguments
  const { values, positionals } = parseArgs({
    options: {
      db: { type: 'string', default: './store.db' },
      apply: { type: 'boolean', default: false },
      model: { type: 'string', default: DEFAULT_MODEL },
      resume: { type: 'string' },
      json: { type: 'boolean', default: false },
      verbose: { type: 'boolean', short: 'V', default: false },
      stats: { type: 'boolean', default: false },
      help: { type: 'boolean', short: 'h', default: false },
      version: { type: 'boolean', short: 'v', default: false }
    },
    allowPositionals: true
  });

  // Initialize output formatter
  const output = new RichOutput({ color: !values.json });

  // Handle help
  if (values.help) {
    console.log(HELP);
    process.exit(0);
  }

  // Handle version
  if (values.version) {
    console.log(`@stateset/cli v${CLI_VERSION}`);
    process.exit(0);
  }

  // Get request from positionals
  const request = positionals.join(' ').trim();
  if (!request) {
    console.error('Error: No request provided');
    console.error('Usage: stateset "<your request>"');
    console.error('Run stateset --help for more information');
    process.exit(1);
  }

  // Show mode indicator
  if (!values.json) {
    console.log(`\n${ICONS.order} StateSet iCommerce CLI`);
    console.log(`   ${output.dim('Database:')} ${values.db}`);
    console.log(`   ${output.dim('Mode:')}     ${values.apply ? output.green('Write enabled') : output.yellow('Preview only')}`);
    if (values.verbose) {
      console.log(`   ${output.dim('Verbose:')}  ${output.cyan('Enabled')}`);
    }
    if (values.resume) {
      console.log(`   ${output.dim('Session:')}  ${values.resume}`);
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
      verbose: values.verbose,
      onToolCall: (toolCall) => {
        if (!values.json && !values.verbose) {
          // Standard tool call display (verbose mode handles its own output)
          console.log(output.toolCall(toolCall.name, toolCall.input));
        }
      }
    });

    if (values.json) {
      // JSON output with extended telemetry
      console.log(JSON.stringify({
        request,
        allowApply: values.apply,
        sessionId: result.sessionId,
        traceId: result.traceId,
        agent: result.agent,
        routing: result.routing ? {
          agent: result.routing.primary.agent,
          confidence: result.routing.primary.confidence,
          ambiguous: result.routing.ambiguous
        } : undefined,
        response: result.response,
        toolResults: result.toolResults.map(tr => ({
          tool: tr.toolCall.name,
          input: tr.toolCall.input,
          result: tr.result,
          duration: tr.duration
        })),
        telemetry: values.stats || values.verbose ? result.telemetry : undefined
      }, null, 2));
    } else {
      // Human-readable output
      console.log('\n' + result.response);

      // Show routing info in verbose mode
      if (values.verbose && result.routing) {
        console.log(`\n${output.dim('Agent:')} ${result.agent} (${Math.round(result.routing.primary.confidence * 100)}% confidence)`);
        if (result.routing.ambiguous) {
          console.log(output.yellow('  Note: Routing was ambiguous, consider using a specialized agent'));
        }
      }

      // Show stats if requested
      if ((values.stats || values.verbose) && result.telemetry) {
        const stats = result.telemetry;
        console.log(`\n${output.dim('─'.repeat(40))}`);
        console.log(`${ICONS.analytics} ${output.bold('Execution Stats')}`);
        console.log(`   ${output.dim('Trace ID:')}    ${result.traceId}`);
        console.log(`   ${output.dim('Duration:')}    ${stats.duration}ms`);
        console.log(`   ${output.dim('Tool Calls:')}  ${stats.toolCalls?.total || 0} (${stats.toolCalls?.successRate || 'N/A'} success)`);
        if (stats.avgToolDuration > 0) {
          console.log(`   ${output.dim('Avg Latency:')} ${stats.avgToolDuration}ms per tool`);
        }
      }

      if (result.sessionId) {
        console.log(`\n${ICONS.session} ${output.dim('Session ID:')} ${result.sessionId}`);
        console.log(`   ${output.dim('Use')} --resume ${result.sessionId} ${output.dim('to continue this conversation')}`);
      }
    }

    process.exit(0);
  } catch (error) {
    if (values.json) {
      console.log(JSON.stringify({ error: error.message }));
    } else {
      console.error(`\n${output.status('error', error.message)}`);
    }
    process.exit(1);
  }
}

main();

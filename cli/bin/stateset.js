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
 *   stateset --agent inventory "check stock levels"
 */

import { parseArgs } from 'node:util';
import * as readline from 'node:readline';

// IMPORTANT: Save and clean argv BEFORE importing SDK modules
// The Claude Agent SDK reads process.argv and passes it to the spawned process
const __savedArgv = [...process.argv];
process.argv = process.argv.slice(0, 2);

// Use dynamic imports after cleaning argv
const { runAgentLoop, RichOutput, ICONS, AgentTelemetry, AGENTS } = await import('../src/claude-harness.js');
const { DEFAULT_MODEL, CLI_VERSION } = await import('../src/config.js');
const { getProfileConfig } = await import('./stateset-config.js');

// Available agent names for validation
const AVAILABLE_AGENTS = Object.keys(AGENTS);

/**
 * Load configuration from profile and merge with CLI args
 */
function loadConfigWithProfile(profileName) {
  try {
    return getProfileConfig(profileName);
  } catch {
    // Return empty config if profile system not initialized
    return {};
  }
}

const HELP = `
StateSet iCommerce CLI - AI-powered commerce operations

USAGE:
  stateset [options] "<request>"

OPTIONS:
  --db <path>        Path to SQLite database (default: ./store.db)
  --apply            Enable write operations (create, update, delete)
  --agent <name>     Use specific agent (bypasses auto-routing)
  --profile <name>   Use configuration profile (from ~/.stateset/profiles/)
  --model <model>    Claude model to use (default: claude-sonnet-4)
  --resume <id>      Resume a previous session
  --json             Output as JSON
  --format <fmt>     Output format: table, json, csv, yaml (default: table)
  --output <file>    Write output to file instead of stdout
  --verbose, -V      Enable verbose output with telemetry
  --stats            Show execution statistics after completion
  --yes, -y          Skip confirmation prompts
  --quiet, -q        Minimal output (for scripting)
  --help, -h         Show this help message
  --version, -v      Show version

CONFIGURATION:
  Use 'stateset-config' to manage profiles:
    stateset-config create production
    stateset-config set db /var/data/production.db
    stateset-config use production

BATCH/PIPELINE MODE:
  --stdin              Read requests from stdin (one per line)
  --batch <file>       Read requests from file (one per line)
  --parallel <n>       Process requests in parallel (default: sequential)
                       Use with caution - parallel requests are independent

  Pipeline examples:
    # Sequential processing (default, maintains session context)
    echo "list customers" | stateset --stdin --json | jq '.response'

    # Parallel processing (faster, no shared context)
    stateset --batch requests.txt --parallel 4 --json

    # Parallel with write operations
    stateset --apply --batch orders.txt --parallel 3

AGENTS:
  customer-service   Full-service agent (default fallback)
  checkout           Shopping cart & checkout flow (ACP)
  orders             Order lifecycle management
  inventory          Stock & reservation management
  returns            RMA & refund processing
  analytics          Business intelligence & forecasting
  storefront         E-commerce website scaffolding

EXAMPLES:
  # List customers (read-only)
  stateset "show me all customers"

  # Use a specific agent
  stateset --agent inventory "check all stock levels"
  stateset --agent analytics "show me revenue trends"

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

  # Analytics and forecasting
  stateset "what's my total revenue this month?"
  stateset "forecast revenue for next quarter"

  # Cart recovery
  stateset "show me abandoned carts"

  # Use a different database
  stateset --db ./production.db "list recent orders"

  # Output to file
  stateset --format csv --output orders.csv "list all orders"

SAFETY:
  By default, all write operations are blocked. Use --apply to enable them.
  High-value operations (>$1000) will prompt for confirmation unless --yes is used.
  The CLI will always show you what would happen before making changes.
`;

/**
 * Prompt user for confirmation
 * @param {string} message - Confirmation message
 * @returns {Promise<boolean>} - True if confirmed
 */
async function confirm(message) {
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout
  });

  return new Promise((resolve) => {
    rl.question(`${message} [y/N] `, (answer) => {
      rl.close();
      resolve(answer.toLowerCase() === 'y' || answer.toLowerCase() === 'yes');
    });
  });
}

/**
 * Format output in specified format
 * @param {object} data - Data to format
 * @param {string} format - Output format (table, json, csv, yaml)
 * @returns {string} - Formatted output
 */
function formatOutput(data, format) {
  switch (format) {
    case 'json':
      return JSON.stringify(data, null, 2);
    case 'csv':
      if (Array.isArray(data) && data.length > 0) {
        const headers = Object.keys(data[0]);
        const rows = data.map(row => headers.map(h => JSON.stringify(row[h] ?? '')).join(','));
        return [headers.join(','), ...rows].join('\n');
      }
      return '';
    case 'yaml':
      // Simple YAML-like output
      return Object.entries(data)
        .map(([k, v]) => `${k}: ${typeof v === 'object' ? JSON.stringify(v) : v}`)
        .join('\n');
    default:
      return data;
  }
}

/**
 * Read lines from stdin
 */
async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(chunk);
  }
  return Buffer.concat(chunks).toString('utf-8').trim().split('\n').filter(line => line.trim());
}

/**
 * Process a single request in batch mode
 */
async function processBatchRequest(request, index, total, config, values, output) {
  const isQuiet = values.quiet || values.json;
  const startTime = Date.now();

  try {
    const result = await runAgentLoop({
      request,
      dbPath: config.db,
      model: config.model,
      allowApply: config.apply,
      agent: values.agent,
      verbose: false,
      onConfirmRequired: values.yes ? null : async () => true
    });

    const duration = Date.now() - startTime;

    return {
      index,
      request,
      success: true,
      response: result.response,
      agent: result.agent,
      sessionId: result.sessionId,
      duration
    };
  } catch (error) {
    return {
      index,
      request,
      success: false,
      error: error.message,
      duration: Date.now() - startTime
    };
  }
}

/**
 * Process requests sequentially (maintains session context)
 */
async function processSequential(requests, config, values, output) {
  const isQuiet = values.quiet || values.json;
  const results = [];
  let sessionId = values.resume;

  for (let i = 0; i < requests.length; i++) {
    const request = requests[i].trim();
    if (!request) continue;

    if (!isQuiet) {
      console.log(`${output.dim(`[${i + 1}/${requests.length}]`)} ${request}`);
    }

    const startTime = Date.now();

    try {
      const result = await runAgentLoop({
        request,
        dbPath: config.db,
        model: config.model,
        allowApply: config.apply,
        resumeSessionId: sessionId,
        agent: values.agent,
        verbose: false,
        onConfirmRequired: values.yes ? null : async () => true
      });

      // Chain session IDs for sequential operations
      sessionId = result.sessionId;

      const batchResult = {
        index: i,
        request,
        success: true,
        response: result.response,
        agent: result.agent,
        sessionId: result.sessionId,
        duration: Date.now() - startTime
      };

      results.push(batchResult);

      if (!isQuiet && !values.json) {
        console.log(`   ${output.green('✓')} ${result.response.slice(0, 100)}${result.response.length > 100 ? '...' : ''}`);
        console.log();
      }
    } catch (error) {
      results.push({
        index: i,
        request,
        success: false,
        error: error.message,
        duration: Date.now() - startTime
      });

      if (!isQuiet && !values.json) {
        console.log(`   ${output.red('✗')} ${error.message}`);
        console.log();
      }
    }
  }

  return results;
}

/**
 * Process requests in parallel with controlled concurrency
 */
async function processParallel(requests, concurrency, config, values, output) {
  const isQuiet = values.quiet || values.json;
  const results = [];
  let completed = 0;
  let inProgress = 0;

  // Create a queue of work
  const queue = requests.map((request, index) => ({ request: request.trim(), index }))
    .filter(item => item.request);

  const total = queue.length;

  if (!isQuiet) {
    console.log(`${output.dim('Processing...')}\n`);
  }

  // Process queue with controlled concurrency
  const processNext = async () => {
    while (queue.length > 0) {
      const item = queue.shift();
      if (!item) break;

      inProgress++;

      const result = await processBatchRequest(
        item.request,
        item.index,
        total,
        config,
        values,
        output
      );

      results.push(result);
      completed++;
      inProgress--;

      // Progress update for non-quiet mode
      if (!isQuiet && !values.json) {
        const pct = Math.round((completed / total) * 100);
        const status = result.success ? output.green('✓') : output.red('✗');
        process.stdout.write(`\r${output.dim(`Progress: ${completed}/${total} (${pct}%)`)}  `);
      }
    }
  };

  // Start concurrent workers
  const workers = [];
  for (let i = 0; i < Math.min(concurrency, queue.length); i++) {
    workers.push(processNext());
  }

  // Wait for all workers to complete
  await Promise.all(workers);

  if (!isQuiet && !values.json) {
    // Clear progress line
    process.stdout.write('\r' + ' '.repeat(50) + '\r');
    console.log(`${output.green('✓')} Completed ${completed} requests\n`);
  }

  return results;
}

/**
 * Handle batch mode - process multiple requests from stdin or file
 * Supports both sequential (default) and parallel processing
 */
async function handleBatchMode(values, config, output) {
  const fs = await import('node:fs/promises');
  const isQuiet = values.quiet || values.json;
  const parallelism = values.parallel ? parseInt(values.parallel, 10) : 0;

  // Read requests
  let requests = [];
  if (values.batch) {
    const content = await fs.readFile(values.batch, 'utf-8');
    requests = content.trim().split('\n').filter(line => line.trim() && !line.startsWith('#'));
  } else if (values.stdin) {
    requests = await readStdin();
  }

  if (requests.length === 0) {
    console.error('Error: No requests to process');
    process.exit(1);
  }

  const startTime = Date.now();

  if (!isQuiet) {
    console.log(`\n${ICONS.order} StateSet iCommerce CLI - Batch Mode`);
    console.log(`   ${output.dim('Requests:')}    ${requests.length}`);
    console.log(`   ${output.dim('Mode:')}        ${config.apply ? output.green('Write enabled') : output.yellow('Preview only')}`);
    console.log(`   ${output.dim('Processing:')}  ${parallelism > 0 ? output.cyan(`Parallel (${parallelism} concurrent)`) : 'Sequential'}`);
    console.log();
  }

  let results = [];

  if (parallelism > 0) {
    // Parallel processing mode
    results = await processParallel(requests, parallelism, config, values, output);
  } else {
    // Sequential processing mode (maintains session context)
    results = await processSequential(requests, config, values, output);
  }

  // Sort results by original index for consistent output
  results.sort((a, b) => a.index - b.index);

  // Output results
  for (const result of results) {
    if (values.json) {
      console.log(JSON.stringify({
        request: result.request,
        success: result.success,
        response: result.response,
        error: result.error,
        agent: result.agent,
        sessionId: result.sessionId,
        duration: result.duration
      }));
    } else if (!isQuiet && parallelism > 0) {
      // For parallel mode, output results after completion
      const status = result.success ? output.green('✓') : output.red('✗');
      const content = result.success
        ? result.response.slice(0, 100) + (result.response.length > 100 ? '...' : '')
        : result.error;
      console.log(`${output.dim(`[${result.index + 1}/${requests.length}]`)} ${result.request}`);
      console.log(`   ${status} ${content}`);
      console.log();
    }
  }

  const totalDuration = Date.now() - startTime;

  // Summary
  if (!isQuiet && !values.json) {
    const succeeded = results.filter(r => r.success).length;
    const failed = results.filter(r => !r.success).length;
    const avgDuration = results.length > 0
      ? Math.round(results.reduce((sum, r) => sum + (r.duration || 0), 0) / results.length)
      : 0;

    console.log(output.dim('─'.repeat(50)));
    console.log(`${output.bold('Summary:')}`);
    console.log(`   ${output.dim('Results:')}    ${output.green(succeeded + ' succeeded')}, ${failed > 0 ? output.red(failed + ' failed') : output.dim('0 failed')}`);
    console.log(`   ${output.dim('Total time:')} ${(totalDuration / 1000).toFixed(2)}s`);
    console.log(`   ${output.dim('Avg/request:')} ${(avgDuration / 1000).toFixed(2)}s`);

    if (parallelism > 0) {
      const speedup = (results.reduce((sum, r) => sum + (r.duration || 0), 0) / totalDuration).toFixed(1);
      console.log(`   ${output.dim('Speedup:')}    ${speedup}x (${parallelism} concurrent)`);
    }

    // Show last session for sequential mode
    const lastSession = results.filter(r => r.sessionId).pop()?.sessionId;
    if (lastSession && parallelism === 0) {
      console.log(`   ${output.dim('Session:')}    ${lastSession}`);
    }
  }

  process.exit(results.some(r => !r.success) ? 1 : 0);
}

async function main() {
  // Parse arguments using the saved argv (before we cleaned it for the SDK)
  const { values, positionals } = parseArgs({
    args: __savedArgv.slice(2), // Use saved argv, skip node and script path
    options: {
      db: { type: 'string' },
      apply: { type: 'boolean', default: false },
      agent: { type: 'string' },
      profile: { type: 'string', short: 'p' },
      model: { type: 'string' },
      resume: { type: 'string' },
      json: { type: 'boolean', default: false },
      format: { type: 'string', default: 'table' },
      output: { type: 'string' },
      verbose: { type: 'boolean', short: 'V', default: false },
      stats: { type: 'boolean', default: false },
      yes: { type: 'boolean', short: 'y', default: false },
      quiet: { type: 'boolean', short: 'q', default: false },
      stdin: { type: 'boolean', default: false },
      batch: { type: 'string' },
      parallel: { type: 'string' },
      help: { type: 'boolean', short: 'h', default: false },
      version: { type: 'boolean', short: 'v', default: false }
    },
    allowPositionals: true
  });

  // Load profile config and merge with CLI args (CLI args take precedence)
  const profileConfig = loadConfigWithProfile(values.profile);
  const config = {
    db: values.db || profileConfig.db || './store.db',
    model: values.model || profileConfig.model || DEFAULT_MODEL,
    apply: values.apply || profileConfig.apply || false,
    verbose: values.verbose || profileConfig.verbose || false
  };

  // Initialize output formatter
  const output = new RichOutput({ color: !values.json && !values.quiet });
  const isQuiet = values.quiet || values.json;

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

  // Validate agent name if provided
  if (values.agent && !AVAILABLE_AGENTS.includes(values.agent)) {
    console.error(`Error: Unknown agent '${values.agent}'`);
    console.error(`Available agents: ${AVAILABLE_AGENTS.join(', ')}`);
    process.exit(1);
  }

  // Handle batch/stdin modes
  if (values.stdin || values.batch) {
    await handleBatchMode(values, config, output);
    return;
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
  if (!isQuiet) {
    console.log(`\n${ICONS.order} StateSet iCommerce CLI`);
    if (values.profile) {
      console.log(`   ${output.dim('Profile:')}  ${output.cyan(values.profile)}`);
    }
    console.log(`   ${output.dim('Database:')} ${config.db}`);
    console.log(`   ${output.dim('Mode:')}     ${config.apply ? output.green('Write enabled') : output.yellow('Preview only')}`);
    if (values.agent) {
      console.log(`   ${output.dim('Agent:')}    ${output.cyan(values.agent)}`);
    }
    if (config.verbose) {
      console.log(`   ${output.dim('Verbose:')}  ${output.cyan('Enabled')}`);
    }
    if (values.resume) {
      console.log(`   ${output.dim('Session:')}  ${values.resume}`);
    }
    console.log();
  }

  try {
    // Confirmation callback for high-risk operations
    const onConfirmRequired = values.yes ? null : async ({ operation, details, amount }) => {
      if (isQuiet) return true; // Auto-confirm in quiet mode with --yes

      let message = `\n${output.yellow('⚠️  Confirmation required')}\n`;
      message += `   Operation: ${operation}\n`;
      if (details) message += `   Details: ${details}\n`;
      if (amount) message += `   Amount: ${output.bold('$' + amount.toFixed(2))}\n`;
      message += `\n   Proceed?`;

      console.log(message);
      return await confirm('');
    };

    const result = await runAgentLoop({
      request,
      dbPath: config.db,
      model: config.model,
      allowApply: config.apply,
      resumeSessionId: values.resume,
      agent: values.agent,
      verbose: config.verbose,
      onConfirmRequired,
      onToolCall: (toolCall) => {
        if (!isQuiet && !config.verbose) {
          // Standard tool call display (verbose mode handles its own output)
          console.log(output.toolCall(toolCall.name, toolCall.input));
        }
      }
    });

    // Prepare output data
    const outputData = {
      request,
      profile: values.profile,
      allowApply: config.apply,
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
    };

    // Handle file output
    if (values.output) {
      const fs = await import('node:fs/promises');
      const formattedOutput = values.format === 'json'
        ? JSON.stringify(outputData, null, 2)
        : formatOutput(outputData, values.format);
      await fs.writeFile(values.output, formattedOutput);
      if (!isQuiet) {
        console.log(`${output.green('✓')} Output written to ${values.output}`);
      }
    } else if (values.json || values.format === 'json') {
      // JSON output with extended telemetry
      console.log(JSON.stringify(outputData, null, 2));
    } else {
      // Human-readable output
      if (!isQuiet) {
        console.log('\n' + result.response);

        // Show routing info in verbose mode or when agent was auto-selected
        if ((values.verbose || !values.agent) && result.routing) {
          const conf = Math.round(result.routing.primary.confidence * 100);
          if (!values.agent) {
            console.log(`\n${output.dim('Agent:')} ${result.agent}${conf > 0 ? ` (${conf}% confidence)` : ''}`);
          }
          if (result.routing.ambiguous) {
            console.log(output.yellow('  💡 Tip: Use --agent <name> for more precise routing'));
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
      } else {
        // Quiet mode - just the response
        console.log(result.response);
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

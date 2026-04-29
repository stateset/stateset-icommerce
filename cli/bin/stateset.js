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
 *
 * Tip: `ss` is a shorthand alias for `stateset`.
 */

import { parseArgs } from 'node:util';
import { spawnSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import module from 'node:module';
import { getMainCliParseOptions, normalizeMainCliValues } from '../src/cli-schema.js';
import { buildRunAgentLoopOptions } from '../src/main-cli-options.js';

if (module.enableCompileCache && !process.env.NODE_DISABLE_COMPILE_CACHE) {
  try {
    module.enableCompileCache();
  } catch {
    // Ignore compile cache setup failures.
  }
}
// IMPORTANT: Save and clean argv BEFORE importing SDK modules
// The Claude Agent SDK reads process.argv and passes it to the spawned process
const __savedArgv = [...process.argv];
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const MAX_PARALLELISM = (() => {
  const raw = Number.parseInt(process.env.STATESET_MAX_PARALLEL || '16', 10);
  if (!Number.isFinite(raw) || raw < 1) return 16;
  return Math.min(raw, 64);
})();
const BACKPRESSURE_DELAY_MS = 1000;
const QUEUE_ADMIN_ENV = 'STATESET_QUEUE_ADMIN';
const SUBCOMMAND_SCRIPTS = new Map([
  ['doctor', 'stateset-doctor.js'],
  ['update', 'stateset-update.js'],
  ['simulate', 'stateset-simulate.js'],
  ['pay', 'stateset-pay.js'],
]);

function runLifecycleScript(scriptName, args) {
  const scriptPath = join(__dirname, scriptName);
  const result = spawnSync(process.execPath, [scriptPath, ...args], {
    stdio: 'inherit',
    env: process.env,
  });

  if (result.error && result.status === null) {
    console.error(`[stateset] Failed to run ${scriptName}: ${result.error.message}`);
    process.exit(1);
  }

  process.exit(typeof result.status === 'number' ? result.status : 1);
}

function routeLifecycleCommands(argv) {
  if (argv.includes('--update')) {
    const forwarded = [];
    let removed = false;
    for (const arg of argv) {
      if (!removed && arg === '--update') {
        removed = true;
        continue;
      }
      forwarded.push(arg);
    }
    runLifecycleScript('stateset-update.js', forwarded);
    return true;
  }

  const first = argv[0];
  const subcommandScript = SUBCOMMAND_SCRIPTS.get(first);
  if (subcommandScript) {
    runLifecycleScript(subcommandScript, argv.slice(1));
    return true;
  }

  return false;
}

routeLifecycleCommands(__savedArgv.slice(2));

// Fast path: --version without loading heavy modules
if (__savedArgv.includes('--version') || __savedArgv.includes('-v')) {
  const { CLI_VERSION } = await import('../src/config.js');
  console.log(`@stateset/cli v${CLI_VERSION}`);
  process.exit(0);
}

// Standalone mode: suppress sync-related output
const __standaloneMode = __savedArgv.includes('--standalone');
if (__standaloneMode) {
  process.env.STATESET_STANDALONE = '1';
}

let runAgentLoopMod = null;
let configMod = null;
let outputMod = null;
let confirmMod = null;
let stateConfigMod = null;
let validatorsMod = null;
let errorHintsMod = null;

try {
  process.argv = __savedArgv.slice(0, 2);

  // Use dynamic imports after cleaning argv
  runAgentLoopMod = await import('../src/claude-harness.js');
  configMod = await import('../src/config.js');
  outputMod = await import('../src/output.js');
  confirmMod = await import('../src/utils/confirm.js');
  stateConfigMod = await import('./stateset-config.js');
  validatorsMod = await import('../src/utils/validators.js');
  errorHintsMod = await import('../src/utils/error-hints.js');
} finally {
  process.argv = __savedArgv;
}

const { runAgentLoop, getQueueStats, removeQueueLane, clearQueueLanes, RichOutput, ICONS, AGENTS } =
  runAgentLoopMod;
const { DEFAULT_MODEL, CLI_VERSION } = configMod;
const { formatStructuredOutput } = outputMod;
const { createConfirmHandler } = confirmMod;
const { getProfileConfig } = stateConfigMod;
const { validateFormat, validateBudget, validateProvider, validateModel, validateThinkLevel } =
  validatorsMod;
const { getErrorHint } = errorHintsMod;

// Available agent names for validation
const AVAILABLE_AGENTS = Object.keys(AGENTS);

/**
 * Load configuration from profile and merge with CLI args
 */
function loadConfigWithProfile(profileName) {
  if (!profileName) {
    try {
      return getProfileConfig();
    } catch {
      // Return empty config if no default profile exists yet.
      return {};
    }
  }
  return getProfileConfig(profileName);
}

const HELP = `
StateSet iCommerce CLI v${CLI_VERSION} - AI-powered commerce operations

QUICK START:
  1. Set up your API key (required):
     stateset-config set-key anthropic

  2. Or set environment variable directly:
     export ANTHROPIC_API_KEY="sk-ant-api03-..."

  3. Get your API key from: https://console.anthropic.com/

  4. Run your first command:
     stateset "show me all customers"

USAGE:
  stateset [options] "<request>"
  ss [options] "<request>"

OPTIONS:
  --db <path>        Path to SQLite database (default: ./store.db)
  --apply            Enable write operations (create, update, delete)
  --agent <name>     Use specific agent (bypasses auto-routing)
  --profile <name>   Use configuration profile (from ~/.stateset/profiles/)
  --model <model>    AI model to use (default: claude-sonnet-4-5)
  --provider <name>  AI provider: claude, openai, gemini, ollama (default: claude)
  --think <level>    Extended thinking: off, low, medium, high (default: off)
  --stream           Enable streaming output (token-by-token)
  --budget <usd>     Maximum spend per query in USD (e.g., --budget 1.00)
  --memory           Enable conversation memory (overrides settings)
  --no-memory        Disable conversation memory (overrides settings)
  --x402             Enable x402 MCP tools (reads X402_* config/env)
  --treasury         Enable treasury billing (stablecoins)
  --treasury-chain <id>    Treasury chain id (e.g., base, solana)
  --treasury-token <sym>   Treasury token symbol (e.g., USDC)
  --treasury-agent <id>    Treasury agent id (default: default)
  --treasury-db <path>     Treasury DB path
  --treasury-erc8004-registry <uri>  ERC-8004 registry URI
  --treasury-erc8004-db <path>       ERC-8004 db path (defaults to --db)
  --resume <id>      Resume a previous session
  --queue-status       Show current agent queue state and exit
  --queue-clear        Clear queue lanes and exit
  --queue-lane <id>    Queue lane ID to inspect/remove
  --queue-force        Force clear/remove busy queue lanes (admin)
  --queue-admin        Required for queue admin commands (with ${QUEUE_ADMIN_ENV}=1)
  --json             Output as JSON
  --format <fmt>     Output format: table, json, csv, yaml (default: table)
  --output <file>    Write output to file instead of stdout
  --verbose, -V      Enable verbose output with telemetry
  --stats            Show execution statistics after completion
  --timeout <ms>     Abort requests that exceed this duration
  --update           Check for CLI updates and exit
  --yes, -y          Skip confirmation prompts
  --quiet, -q        Minimal output (for scripting)
  --no-color         Disable colored output (also respects NO_COLOR env var)
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
  Core Commerce:
    customer-service   Full-service agent (default fallback, 90+ tools)
    checkout           Shopping cart & checkout flow (ACP)
    orders             Order lifecycle management
    inventory          Stock & reservation management
    returns            RMA & refund processing
    analytics          Business intelligence & forecasting

  Marketing & Sales:
    promotions         Discounts, coupons & promotional campaigns
    subscriptions      Subscription plans & recurring billing

  Operations:
    manufacturing      BOMs, work orders & production management
    shipments          Shipment tracking & delivery
    suppliers          Supplier management & purchase orders
    invoices           B2B invoicing & accounts receivable
    warranties         Product warranties & claims

  Financial:
    currency           Multi-currency support & exchange rates
    tax                Tax calculation (US/EU/CA) & exemptions
    payments           Payment processing & refunds
    stablecoin         Native crypto payments (USDC, ssUSD, BTC, ZEC)

  Infrastructure:
    sync               Verifiable Event Sync (VES) with production
    storefront         E-commerce website scaffolding

SPECIALIZED COMMANDS:
  stateset-setup           Guided first-time setup wizard
  stateset-update          Check/install CLI updates
  stateset-chat            Interactive multi-turn REPL
  stateset-direct          Direct CLI (no AI, structured commands)
  stateset-pay             Native crypto payments
  stateset-autonomous      Autonomous business engine
  stateset-sync            VES sync management
  stateset-daemon          Daemon & service management
  stateset-channels        Messaging channel orchestration
  stateset-events          Legacy event streaming (DB webhooks + feed)
  stateset-mcp-events      MCP execution event stream gateway
  stateset-simulate        A2A simulation playground with snapshots
  stateset-skills          Skills marketplace
  stateset-x402            x402 config + key setup
  stateset-x402-mcp        x402 MCP server for paid API calls

  Messaging Channels (10+):
    stateset-slack         Slack integration
    stateset-discord       Discord bot
    stateset-telegram      Telegram bot
    stateset-whatsapp      WhatsApp Business
    stateset-signal        Signal messenger
    stateset-google-chat   Google Chat / Workspace

EXAMPLES:
  # Lifecycle shortcuts
  stateset doctor --checks api,db
  stateset update status
  stateset simulate --scenario supplier-goes-offline --agents inventory,procurement

  # List customers (read-only)
  stateset "show me all customers"
  ss "show me all customers"

  # Use a specific agent
  stateset --agent inventory "check all stock levels"
  stateset --agent analytics "show me revenue trends"
  stateset --agent promotions "show active promotions"
  stateset --agent subscriptions "list all subscription plans"

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

  # Promotions and coupons
  stateset --apply "create a 20% off promotion called Summer Sale"
  stateset "validate coupon SAVE20"

  # Subscriptions
  stateset --apply "create a monthly plan called Coffee Club at $29.99"
  stateset --apply "subscribe customer X to the Coffee Club plan"

  # Multi-currency
  stateset "convert $100 USD to EUR"
  stateset "what's the exchange rate from USD to GBP?"

  # Tax calculation
  stateset "calculate tax for an order shipping to California"

  # Stablecoin payments
  stateset pay --wallet --chain solana
  stateset pay --apply --to <address> --amount 50.00 --chain solana

  # Cart recovery
  stateset "show me abandoned carts"

  # Sync with production
  stateset-sync status
  stateset-sync push
  stateset-sync pull

  # Use a different database
  stateset --db ./production.db "list recent orders"

  # Output to file
  stateset --format csv --output orders.csv "list all orders"

  # Extended thinking for complex queries
  stateset --think high "analyze my business performance and suggest improvements"

SAFETY:
  By default, all write operations are blocked. Use --apply to enable them.
  High-value operations (>$1000) will prompt for confirmation unless --yes is used.
  The CLI will always show you what would happen before making changes.

MORE INFO:
  Documentation: https://docs.stateset.com/cli
  Issues: https://github.com/stateset/stateset-icommerce/issues
`;

/**
 * Format output in specified format
 * @param {object} data - Data to format
 * @param {string} format - Output format (table, json, csv, yaml)
 * @returns {string} - Formatted output
 */
function formatOutput(data, format) {
  return formatStructuredOutput(data, format);
}

function failCli(message, { json = false, code = 1, details = [], hint = null } = {}) {
  const normalizedMessage = String(message)
    .replace(/^Error:\s*/i, '')
    .trim();
  if (json) {
    const payload = { error: normalizedMessage };
    if (hint) {
      payload.hint = hint;
    }
    if (details.length > 0) {
      payload.details = details;
    }
    console.log(JSON.stringify(payload));
  } else {
    const rendered = String(message).startsWith('Error:') ? String(message) : `Error: ${message}`;
    console.error(rendered);
    for (const detail of details) {
      console.error(detail);
    }
  }
  process.exit(code);
}

/**
 * Read lines from stdin
 */
async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(chunk);
  }
  return Buffer.concat(chunks)
    .toString('utf-8')
    .trim()
    .split('\n')
    .filter((line) => line.trim());
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function shouldBackpressure(errorMessage) {
  if (!errorMessage) return false;
  return /429|rate limit|too many requests|resource exhausted|overload|timeout/i.test(errorMessage);
}

/**
 * Run agent loop with optional abort timeout.
 * @param {object} options
 * @param {number|null} timeoutMs
 */
async function runAgentLoopWithTimeout(options, timeoutMs) {
  if (!timeoutMs) {
    return runAgentLoop(options);
  }

  const abortController = new AbortController();
  const timer = setTimeout(() => {
    abortController.abort(new Error(`Request timeout exceeded: ${timeoutMs}ms`));
  }, timeoutMs);

  try {
    return await runAgentLoop({
      ...options,
      abortController,
      signal: abortController.signal,
    });
  } finally {
    clearTimeout(timer);
  }
}

/**
 * Process a single request in batch mode
 */
async function processBatchRequest(
  request,
  index,
  total,
  config,
  values,
  treasuryConfig,
  onConfirmRequired,
  thinkLevel,
  providerName,
  memoryOverride,
) {
  const startTime = Date.now();

  try {
    const result = await runAgentLoopWithTimeout(
      buildRunAgentLoopOptions({
        request,
        config,
        values,
        treasuryConfig,
        onConfirmRequired,
        thinkLevel,
        providerName,
        memoryOverride,
      }),
      config.timeoutMs,
    );

    const duration = Date.now() - startTime;

    return {
      index,
      request,
      success: true,
      response: result.response,
      agent: result.agent,
      sessionId: result.sessionId,
      treasury: result.treasury,
      duration,
    };
  } catch (error) {
    return {
      index,
      request,
      success: false,
      error: error.message,
      duration: Date.now() - startTime,
    };
  }
}

/**
 * Process requests sequentially (maintains session context)
 */
async function processSequential(
  requests,
  config,
  values,
  output,
  treasuryConfig,
  onConfirmRequired,
  thinkLevel,
  providerName,
  memoryOverride,
) {
  const isQuiet = values.quiet || values.json || values.format === 'json';
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
      const result = await runAgentLoopWithTimeout(
        buildRunAgentLoopOptions({
          request,
          config,
          values,
          treasuryConfig,
          onConfirmRequired,
          resumeSessionId: sessionId,
          thinkLevel,
          providerName,
          memoryOverride,
        }),
        config.timeoutMs,
      );

      // Chain session IDs for sequential operations
      sessionId = result.sessionId;

      const batchResult = {
        index: i,
        request,
        success: true,
        response: result.response,
        agent: result.agent,
        sessionId: result.sessionId,
        treasury: result.treasury,
        duration: Date.now() - startTime,
      };

      results.push(batchResult);

      if (!isQuiet && !values.json) {
        console.log(
          `   ${output.green('✓')} ${result.response.slice(0, 100)}${result.response.length > 100 ? '...' : ''}`,
        );
        console.log();
      }
    } catch (error) {
      results.push({
        index: i,
        request,
        success: false,
        error: error.message,
        duration: Date.now() - startTime,
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
async function processParallel(
  requests,
  concurrency,
  config,
  values,
  output,
  treasuryConfig,
  onConfirmRequired,
  thinkLevel,
  providerName,
  memoryOverride,
) {
  const isQuiet = values.quiet || values.json || values.format === 'json';
  const results = [];
  let completed = 0;
  let cooldownUntil = 0;

  // Create a queue of work
  const queue = requests
    .map((request, index) => ({ request: request.trim(), index }))
    .filter((item) => item.request);

  const total = queue.length;

  if (!isQuiet) {
    console.log(`${output.dim('Processing...')}\n`);
  }

  // Process queue with controlled concurrency
  const processNext = async () => {
    while (queue.length > 0) {
      const waitMs = cooldownUntil - Date.now();
      if (waitMs > 0) {
        await sleep(waitMs);
      }

      const item = queue.shift();
      if (!item) break;

      const result = await processBatchRequest(
        item.request,
        item.index,
        total,
        config,
        values,
        treasuryConfig,
        onConfirmRequired,
        thinkLevel,
        providerName,
        memoryOverride,
      );

      results.push(result);
      completed++;

      if (!result.success && shouldBackpressure(result.error)) {
        cooldownUntil = Math.max(cooldownUntil, Date.now() + BACKPRESSURE_DELAY_MS);
      }

      // Progress update for non-quiet mode
      if (!isQuiet && !values.json) {
        const pct = Math.round((completed / total) * 100);
        const status = result.success ? output.green('✓') : output.red('✗');
        process.stdout.write(
          `\r${status} ${output.dim(`Progress: ${completed}/${total} (${pct}%)`)}  `,
        );
      }

      // Yield between queue items to avoid starving the event loop.
      await new Promise((resolve) => setImmediate(resolve));
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
async function handleBatchMode(
  values,
  config,
  output,
  treasuryConfig,
  thinkLevel,
  providerName,
  memoryOverride,
) {
  const fs = await import('node:fs/promises');
  const isJsonOutput = values.json || values.format === 'json';
  const isQuiet = values.quiet || isJsonOutput;
  let parallelism = values.parallel ? parseInt(values.parallel, 10) : 0;
  if (values.parallel && (!Number.isFinite(parallelism) || parallelism < 1)) {
    failCli('--parallel must be a positive integer', { json: isJsonOutput });
  }
  if (parallelism > MAX_PARALLELISM) {
    failCli(
      `--parallel cannot exceed ${MAX_PARALLELISM}. Set STATESET_MAX_PARALLEL=<n> (max 64) to adjust the cap.`,
      { json: isJsonOutput },
    );
  }
  if (!parallelism) parallelism = 0;
  if (values.resume && parallelism > 0) {
    failCli('--resume is not compatible with --parallel.', {
      json: isJsonOutput,
      details: ['Use --resume with sequential batch mode for session continuity.'],
    });
  }
  if (values.stream) {
    failCli('--stream is not supported with --batch or --stdin.', {
      json: isJsonOutput,
      details: ['Use sequential non-batch mode for token streaming.'],
    });
  }
  const onConfirmRequired = createConfirmHandler({
    output,
    assumeYes: values.yes,
    nonInteractive: true,
  });

  // Read requests
  let requests = [];
  if (values.batch) {
    const content = await fs.readFile(values.batch, 'utf-8');
    requests = content
      .trim()
      .split('\n')
      .filter((line) => line.trim() && !line.startsWith('#'));
  } else if (values.stdin) {
    requests = await readStdin();
  }

  if (requests.length === 0) {
    failCli('No requests to process', { json: isJsonOutput });
  }

  const startTime = Date.now();

  if (!isQuiet) {
    console.log(`\n${ICONS.order} StateSet iCommerce CLI - Batch Mode`);
    console.log(`   ${output.dim('Requests:')}    ${requests.length}`);
    console.log(
      `   ${output.dim('Mode:')}        ${config.apply ? output.green('Write enabled') : output.yellow('Preview only')}`,
    );
    console.log(
      `   ${output.dim('Processing:')}  ${parallelism > 0 ? output.cyan(`Parallel (${parallelism} concurrent)`) : 'Sequential'}`,
    );
    console.log();
  }

  let results = [];

  if (parallelism > 0) {
    // Parallel processing mode
    results = await processParallel(
      requests,
      parallelism,
      config,
      values,
      output,
      treasuryConfig,
      onConfirmRequired,
      thinkLevel,
      providerName,
      memoryOverride,
    );
  } else {
    // Sequential processing mode (maintains session context)
    results = await processSequential(
      requests,
      config,
      values,
      output,
      treasuryConfig,
      onConfirmRequired,
      thinkLevel,
      providerName,
      memoryOverride,
    );
  }

  // Sort results by original index for consistent output
  results.sort((a, b) => a.index - b.index);

  // Output results
  for (const result of results) {
    if (isJsonOutput) {
      console.log(
        JSON.stringify({
          request: result.request,
          success: result.success,
          response: result.response,
          error: result.error,
          agent: result.agent,
          sessionId: result.sessionId,
          treasury: result.treasury,
          duration: result.duration,
        }),
      );
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
  if (!isQuiet && !isJsonOutput) {
    const succeeded = results.filter((r) => r.success).length;
    const failed = results.filter((r) => !r.success).length;
    const avgDuration =
      results.length > 0
        ? Math.round(results.reduce((sum, r) => sum + (r.duration || 0), 0) / results.length)
        : 0;

    console.log(output.dim('─'.repeat(50)));
    console.log(`${output.bold('Summary:')}`);
    console.log(
      `   ${output.dim('Results:')}    ${output.green(succeeded + ' succeeded')}, ${failed > 0 ? output.red(failed + ' failed') : output.dim('0 failed')}`,
    );
    console.log(`   ${output.dim('Total time:')} ${(totalDuration / 1000).toFixed(2)}s`);
    console.log(`   ${output.dim('Avg/request:')} ${(avgDuration / 1000).toFixed(2)}s`);

    if (parallelism > 0) {
      const speedup = (
        results.reduce((sum, r) => sum + (r.duration || 0), 0) / totalDuration
      ).toFixed(1);
      console.log(`   ${output.dim('Speedup:')}    ${speedup}x (${parallelism} concurrent)`);
    }

    // Show last session for sequential mode
    const lastSession = results.filter((r) => r.sessionId).pop()?.sessionId;
    if (lastSession && parallelism === 0) {
      console.log(`   ${output.dim('Session:')}    ${lastSession}`);
    }
  }

  process.exit(results.some((r) => !r.success) ? 1 : 0);
}

async function main() {
  // Parse arguments using the saved argv (before we cleaned it for the SDK)
  const parsed = parseArgs({
    args: __savedArgv.slice(2), // Use saved argv, skip node and script path
    options: getMainCliParseOptions(),
    allowPositionals: true,
  });
  const values = normalizeMainCliValues(parsed.values);
  const { positionals } = parsed;
  const jsonRequested = values.json || values.format === 'json';

  // Load profile config and merge with CLI args (CLI args take precedence)
  let profileConfig = {};
  try {
    profileConfig = loadConfigWithProfile(values.profile);
  } catch (error) {
    failCli(error.message, { json: jsonRequested });
  }
  const timeoutMs = values.timeout ? Number(values.timeout) : null;
  if (timeoutMs !== null && (!Number.isFinite(timeoutMs) || timeoutMs <= 0)) {
    failCli('--timeout must be a positive integer', { json: jsonRequested });
  }
  const config = {
    db: values.db || profileConfig.db || './store.db',
    model: values.model || profileConfig.model || DEFAULT_MODEL,
    apply: values.apply || profileConfig.apply || false,
    verbose: values.verbose || profileConfig.verbose || false,
    timeoutMs,
  };

  const isJsonOutput = jsonRequested;
  const isQuiet = values.quiet || isJsonOutput;
  const memoryOverride = values.noMemory ? false : values.memory ? true : null;
  const treasuryEnabled = Boolean(
    values.treasury ||
    values.treasuryChain ||
    values.treasuryToken ||
    values.treasuryAgent ||
    values.treasuryDb ||
    values.treasuryErc8004Registry ||
    values.treasuryErc8004Db,
  );
  const treasuryConfig = treasuryEnabled
    ? {
        enabled: true,
        chainId: values.treasuryChain,
        tokenSymbol: values.treasuryToken,
        agentId: values.treasuryAgent,
        dbPath: values.treasuryDb,
        erc8004Registry: values.treasuryErc8004Registry,
        erc8004DbPath: values.treasuryErc8004Db,
      }
    : null;

  // Initialize output formatter
  const output = new RichOutput({ color: !isQuiet && values.color });

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

  if (values.update) {
    runLifecycleScript('stateset-update.js', []);
    return;
  }

  if (values.queueStatus || values.queueClear) {
    const outputQueueError = (message, code = 1) => {
      failCli(message, { json: isJsonOutput, code });
    };

    const queueAdminAuthorized =
      values.queueAdmin && String(process.env[QUEUE_ADMIN_ENV] || '').trim() === '1';
    if (!queueAdminAuthorized) {
      outputQueueError(
        `Error: Queue admin commands require --queue-admin and ${QUEUE_ADMIN_ENV}=1.`,
      );
    }

    if (values.queueClear) {
      if (values.queueLane) {
        const result = removeQueueLane(values.queueLane, { force: values.queueForce });
        if (!result.found) {
          outputQueueError(`Queue lane not found: ${values.queueLane}`);
        }
        if (!result.removed) {
          outputQueueError(
            `Queue lane is busy: ${values.queueLane}. Use --queue-force to remove it.`,
          );
        }
        if (!isQuiet && !isJsonOutput) {
          console.log(formatOutput({ lane: result }, values.format));
        } else {
          console.log(JSON.stringify({ lane: result }, null, 2));
        }
        return;
      }

      const result = clearQueueLanes({ force: values.queueForce });
      if (!isQuiet && !isJsonOutput) {
        console.log(formatOutput(result, values.format));
      } else {
        console.log(JSON.stringify(result, null, 2));
      }
      return;
    }

    const stats = values.queueLane ? getQueueStats(values.queueLane) : getQueueStats();
    if (values.queueLane && !stats) {
      outputQueueError(`Queue lane not found: ${values.queueLane}`);
    }
    if (!isQuiet && !isJsonOutput) {
      console.log(formatOutput(stats, values.format));
    } else {
      console.log(JSON.stringify(stats, null, 2));
    }
    return;
  }

  // ── Input validation (fail fast with helpful messages) ──────────────
  if (values.format && values.format !== 'table') {
    const fmtResult = validateFormat(values.format);
    if (!fmtResult.valid) {
      failCli(fmtResult.error, { json: isJsonOutput });
    }
  }
  if (values.budget !== undefined) {
    const budgetResult = validateBudget(values.budget);
    if (!budgetResult.valid) {
      failCli(budgetResult.error, { json: isJsonOutput });
    }
  }
  if (values.provider) {
    const provResult = validateProvider(values.provider);
    if (!provResult.valid) {
      failCli(provResult.error, { json: isJsonOutput });
    }
  }
  const thinkLevel = values.think || 'off';
  const thinkResult = validateThinkLevel(thinkLevel);
  if (!thinkResult.valid) {
    failCli(thinkResult.error, { json: isJsonOutput });
  }
  const providerName = values.provider || 'claude';
  if (values.model) {
    const modelResult = validateModel(values.model);
    if (modelResult.warning && !isQuiet) {
      console.warn(`Warning: ${modelResult.warning}`);
    }
  }

  // Validate agent name if provided
  if (values.agent && !AVAILABLE_AGENTS.includes(values.agent)) {
    failCli(`Unknown agent '${values.agent}'`, {
      json: isJsonOutput,
      details: [`Available agents: ${AVAILABLE_AGENTS.join(', ')}`],
    });
  }

  // Handle batch/stdin modes
  if (values.stdin || values.batch) {
    await handleBatchMode(
      values,
      config,
      output,
      treasuryConfig,
      thinkLevel,
      providerName,
      memoryOverride,
    );
    return;
  }

  if (values.stream && isJsonOutput) {
    failCli('--stream cannot be used with JSON output. Remove --stream or use a non-JSON format.', {
      json: isJsonOutput,
    });
  }

  // Get request from positionals
  const request = positionals.join(' ').trim();
  if (!request) {
    if (isJsonOutput) {
      failCli('No request provided', {
        json: true,
        details: ['Usage: stateset "<your request>"', 'Run stateset --help for more information'],
      });
    }

    // Detect first-run: no request and no API key
    if (!process.env.ANTHROPIC_API_KEY) {
      console.log('\nWelcome to StateSet iCommerce CLI!');
      console.log('Get started by setting up your API key:\n');
      console.log('  stateset-config set-key anthropic');
      console.log('  stateset-setup                     (guided setup)\n');
      console.log('Usage: stateset "<your request>"');
      console.log('Run stateset --help for all options');
    } else {
      failCli('No request provided', {
        details: ['Usage: stateset "<your request>"', 'Run stateset --help for more information'],
      });
    }
    process.exit(1);
  }

  // Show mode indicator
  if (!isQuiet) {
    console.log(`\n${ICONS.order} StateSet iCommerce CLI`);
    if (values.profile) {
      console.log(`   ${output.dim('Profile:')}  ${output.cyan(values.profile)}`);
    }
    console.log(`   ${output.dim('Database:')} ${config.db}`);
    console.log(
      `   ${output.dim('Mode:')}     ${config.apply ? output.green('Write enabled') : output.yellow('Preview only')}`,
    );
    if (providerName !== 'claude') {
      console.log(`   ${output.dim('Provider:')} ${output.cyan(providerName)}`);
    }
    if (values.agent) {
      console.log(`   ${output.dim('Agent:')}    ${output.cyan(values.agent)}`);
    }
    if (thinkLevel !== 'off') {
      console.log(`   ${output.dim('Thinking:')} ${output.cyan(thinkLevel)}`);
    }
    if (values.stream) {
      console.log(`   ${output.dim('Stream:')}   ${output.cyan('Enabled')}`);
    }
    if (values.budget) {
      console.log(`   ${output.dim('Budget:')}   ${output.cyan('$' + values.budget)}`);
    }
    if (config.timeoutMs) {
      console.log(`   ${output.dim('Timeout:')}  ${output.cyan(config.timeoutMs + 'ms')}`);
    }
    if (memoryOverride !== null) {
      console.log(
        `   ${output.dim('Memory:')}   ${memoryOverride ? output.cyan('Enabled') : output.yellow('Disabled')}`,
      );
    }
    if (config.verbose) {
      console.log(`   ${output.dim('Verbose:')}  ${output.cyan('Enabled')}`);
    }
    if (values.resume) {
      console.log(`   ${output.dim('Session:')}  ${values.resume}`);
    }
    console.log();
  }

  const nonInteractive = !process.stdin.isTTY || values.quiet || isJsonOutput;
  const onConfirmRequired = createConfirmHandler({
    output,
    assumeYes: values.yes,
    nonInteractive,
  });

  try {
    const result = await runAgentLoopWithTimeout(
      buildRunAgentLoopOptions({
        request,
        config,
        values,
        treasuryConfig,
        onConfirmRequired,
        resumeSessionId: values.resume,
        thinkLevel,
        providerName,
        memoryOverride,
        onPartialMessage: values.stream
          ? (event) => {
              if (event?.content) {
                process.stdout.write(event.content);
              } else if (event?.delta?.text) {
                process.stdout.write(event.delta.text);
              } else if (typeof event?.text === 'string') {
                process.stdout.write(event.text);
              }
            }
          : null,
        onThinkingBlock:
          thinkLevel !== 'off'
            ? (block) => {
                if (!isQuiet && config.verbose) {
                  const preview = (block.thinking || block.text || '').slice(0, 200);
                  const message = output.dim(
                    `\n[Thinking] ${preview}${preview.length >= 200 ? '...' : ''}\n`,
                  );
                  if (values.stream) {
                    process.stderr.write(`${message}\n`);
                  } else {
                    console.log(message);
                  }
                }
              }
            : null,
        onToolCall: (toolCall) => {
          if (!isQuiet && !config.verbose) {
            const message = output.toolCall(toolCall.name, toolCall.input);
            if (values.stream) {
              process.stderr.write(`${message}\n`);
            } else {
              console.log(message);
            }
          }
        },
      }),
      config.timeoutMs,
    );

    // Prepare output data
    const outputData = {
      request,
      profile: values.profile,
      allowApply: config.apply,
      sessionId: result.sessionId,
      traceId: result.traceId,
      agent: result.agent,
      treasury: result.treasury,
      routing: result.routing
        ? {
            agent: result.routing.primary.agent,
            confidence: result.routing.primary.confidence,
            ambiguous: result.routing.ambiguous,
          }
        : undefined,
      response: result.response,
      toolResults: result.toolResults.map((tr) => ({
        tool: tr.toolCall.name,
        input: tr.toolCall.input,
        result: tr.result,
        duration: tr.duration,
      })),
      telemetry: values.stats || values.verbose ? result.telemetry : undefined,
      promptReport: values.stats || values.verbose ? result.promptReport : undefined,
    };

    // Handle file output
    if (values.output) {
      const fs = await import('node:fs/promises');
      const formattedOutput =
        values.format === 'json'
          ? JSON.stringify(outputData, null, 2)
          : formatOutput(outputData, values.format);
      await fs.writeFile(values.output, formattedOutput);
      if (!isQuiet) {
        console.log(`${output.green('✓')} Output written to ${values.output}`);
      }
    } else if (isJsonOutput) {
      // JSON output with extended telemetry
      console.log(JSON.stringify(outputData, null, 2));
    } else {
      // Human-readable output
      if (!isQuiet) {
        // If streaming was used, response was already written to stdout
        if (values.stream && result.response) {
          console.log(); // newline after streamed output
        } else {
          console.log('\n' + result.response);
        }

        // Show routing info in verbose mode or when agent was auto-selected
        if ((values.verbose || !values.agent) && result.routing) {
          const conf = Math.round(result.routing.primary.confidence * 100);
          if (!values.agent) {
            console.log(
              `\n${output.dim('Agent:')} ${result.agent}${conf > 0 ? ` (${conf}% confidence)` : ''}`,
            );
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
          console.log(
            `   ${output.dim('Tool Calls:')}  ${stats.toolCalls?.total || 0} (${stats.toolCalls?.successRate || 'N/A'} success)`,
          );
          if (stats.avgToolDuration > 0) {
            console.log(`   ${output.dim('Avg Latency:')} ${stats.avgToolDuration}ms per tool`);
          }
          if (result.provider) {
            console.log(`   ${output.dim('Provider:')}    ${result.provider}`);
          }
          if (result.cost !== null && result.cost !== undefined) {
            console.log(`   ${output.dim('Cost:')}        $${result.cost.toFixed(4)}`);
          }
          if (result.budgetExceeded) {
            console.log(
              `   ${output.yellow('Budget exceeded')}${values.budget ? ` (limit: $${values.budget})` : ''}`,
            );
          }
          if (result.thinkLevel && result.thinkLevel !== 'off') {
            console.log(`   ${output.dim('Thinking:')}    ${result.thinkLevel}`);
          }
          if (result.promptReport) {
            console.log(`\n${output.promptReport(result.promptReport)}`);
          }
        }

        if (result.sessionId) {
          console.log(`\n${ICONS.session} ${output.dim('Session ID:')} ${result.sessionId}`);
          console.log(
            `   ${output.dim('Use')} --resume ${result.sessionId} ${output.dim('to continue this conversation')}`,
          );
        }
      } else {
        // Quiet mode - just the response
        console.log(result.response);
      }
    }

    process.exit(0);
  } catch (error) {
    const hint = getErrorHint(error);
    if (values.json) {
      console.log(JSON.stringify({ error: error.message, hint: hint || undefined }));
    } else {
      console.error(`\n${output.status('error', error.message)}`);
      if (hint) {
        console.error(`\n${output.dim('Suggestion:')}`);
        for (const line of hint.split('\n')) {
          console.error(`  ${output.dim(line)}`);
        }
      }
    }
    process.exit(1);
  }
}

process.on('unhandledRejection', (reason) => {
  console.error(
    '[stateset] Unhandled rejection:',
    reason instanceof Error ? reason.message : reason,
  );
  process.exit(1);
});

main().catch((err) => {
  console.error('[stateset] Fatal error:', err instanceof Error ? err.message : err);
  process.exit(1);
});

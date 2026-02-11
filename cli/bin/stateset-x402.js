#!/usr/bin/env node

/**
 * StateSet x402 CLI
 *
 * Configure and execute x402 agentic payment operations.
 */

import { Command } from 'commander';
import { createRequire } from 'node:module';
import chalk from 'chalk';
import ora from 'ora';
import fs from 'node:fs';
import { createConfirmHandler } from '../src/utils/confirm.js';
import { getKeyManager } from '../src/sync/keys.js';
import { getWalletAddress } from '../src/chains/index.js';
import { CLI_VERSION } from '../src/config.js';
import {
  resolveX402ConfigPath,
  saveX402Config,
  loadX402Config,
  pickConfigValue,
} from '../src/x402/config.js';
import { x402Tools } from '../src/tools/x402.js';
import { installShutdownHandlers } from '../src/graceful-shutdown.js';
installShutdownHandlers('stateset-x402');

const require = createRequire(import.meta.url);

function getCommerceCtor() {
  let mod;
  try {
    mod = require('@stateset/embedded');
  } catch (error) {
    throw new Error(`Failed to load @stateset/embedded: ${error.message}`);
  }
  const Commerce = mod.Commerce || mod.default?.Commerce || mod.default;
  if (!Commerce) {
    throw new Error('Failed to resolve Commerce export from @stateset/embedded');
  }
  return Commerce;
}

function createCommerce(dbPath) {
  const Commerce = getCommerceCtor();
  return new Commerce(dbPath);
}

function resolveOutputOptions(options = {}) {
  return {
    jsonOutput: Boolean(options.json || options.output),
    outputPath: options.output || null,
  };
}

function writeJson(outputPath, data) {
  const payload = JSON.stringify(data, null, 2);
  if (outputPath) {
    fs.writeFileSync(outputPath, payload);
    return;
  }
  console.log(payload);
}

function emitResult(options, result) {
  const { jsonOutput, outputPath } = resolveOutputOptions(options);
  if (jsonOutput) {
    writeJson(outputPath, result);
    return;
  }
  console.log(JSON.stringify(result, null, 2));
}

function parseIntegerOption(value, fieldName) {
  if (value === undefined || value === null || value === '') return undefined;
  const parsed = Number.parseInt(String(value), 10);
  if (!Number.isFinite(parsed)) {
    throw new Error(`${fieldName} must be an integer`);
  }
  return parsed;
}

function parseNumberOption(value, fieldName) {
  if (value === undefined || value === null || value === '') return undefined;
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) {
    throw new Error(`${fieldName} must be a number`);
  }
  return parsed;
}

function parseListValue(value) {
  if (!value) return [];
  if (Array.isArray(value)) {
    return value.map((entry) => String(entry).trim()).filter(Boolean);
  }
  return String(value)
    .split(',')
    .map((entry) => entry.trim())
    .filter(Boolean);
}

function readX402Config(options = {}) {
  const configPath = resolveX402ConfigPath({
    env: process.env,
    configDir: options.configDir || '.stateset',
    configFile: options.configFile,
  });
  if (!fs.existsSync(configPath)) {
    return { configPath, fileConfig: {} };
  }
  return { configPath, fileConfig: loadX402Config(configPath) };
}

function resolveDefaultAgentId(options = {}, fileConfig = {}) {
  return (
    options.agentId ||
    options.payerAgent ||
    pickConfigValue(process.env, fileConfig, 'X402_AGENT_ID', 'agentId', 'agent_id') ||
    'default'
  );
}

function resolveDefaultNetwork(options = {}, fileConfig = {}) {
  if (options.network) return options.network;
  const preferred = pickConfigValue(
    process.env,
    fileConfig,
    'X402_PREFERRED_NETWORKS',
    'preferredNetworks',
    'preferred_networks',
  );
  const values = parseListValue(preferred);
  return values[0] || 'set_chain';
}

function getX402Tool(name) {
  const tool = x402Tools.find((entry) => entry.name === name);
  if (!tool) {
    throw new Error(`x402 tool not found: ${name}`);
  }
  return tool;
}

async function runX402Tool({ toolName, params, options, defaultAgentId }) {
  const tool = getX402Tool(toolName);
  const requestId = `stateset-x402:${toolName}:${Date.now()}`;
  const isWritePreview = !options.apply && tool.permission !== 'read';
  const commerce = isWritePreview
    ? new Proxy(
        {},
        {
          get() {
            throw new Error(
              `Tool ${toolName} attempted to access live commerce data during preview mode`,
            );
          },
        },
      )
    : createCommerce(options.db || './store.db');
  if (!isWritePreview && typeof commerce.x402 !== 'function') {
    throw new Error(
      'x402 APIs are unavailable in the current @stateset/embedded build. Rebuild or upgrade embedded bindings before running x402 commands.',
    );
  }
  return tool.handler({
    commerce,
    params,
    allowApply: Boolean(options.apply),
    resolveTreasuryAgentId: async () => defaultAgentId || 'default',
    treasuryContextOptions: options.treasuryDb ? { dbPath: options.treasuryDb } : {},
    buildAuditContext: (_extra, calledToolName) => ({
      taskId: null,
      requestId,
      sessionId: null,
      toolName: calledToolName,
    }),
    buildTreasuryIdentityMetadata: async () => ({}),
    extra: { requestId, sessionId: null },
  });
}

async function confirmWriteOperation(options, details) {
  if (!options.apply) return true;
  const { jsonOutput } = resolveOutputOptions(options);
  const nonInteractive = !process.stdin.isTTY || jsonOutput;
  const confirm = createConfirmHandler({
    output: {
      yellow: chalk.yellow,
      bold: chalk.bold,
    },
    assumeYes: Boolean(options.yes),
    nonInteractive,
  });
  return confirm(details);
}

async function executeToolCommand({
  spinnerText,
  toolName,
  params,
  options,
  defaultAgentId,
  successMessage,
}) {
  const { jsonOutput } = resolveOutputOptions(options);
  const spinner = jsonOutput ? null : ora({ text: spinnerText }).start();
  try {
    const result = await runX402Tool({ toolName, params, options, defaultAgentId });
    if (!jsonOutput && spinner) {
      spinner.succeed(successMessage);
    }
    emitResult(options, result);
    const isApplyPreview =
      !options.apply &&
      typeof result?.error === 'string' &&
      result.error.toLowerCase().includes('--apply');
    if ((result?.error || result?.success === false) && !isApplyPreview) {
      process.exitCode = 1;
    }
  } catch (error) {
    if (!jsonOutput && spinner) {
      spinner.fail(error.message);
    } else {
      writeJson(options.output || null, { error: error.message });
    }
    process.exitCode = 1;
  }
}

function withRuntimeOptions(command) {
  return command
    .option('--db <path>', 'Commerce SQLite DB path', './store.db')
    .option('--treasury-db <path>', 'Treasury SQLite DB path override')
    .option('--config-dir <path>', 'Config directory (default: .stateset)', '.stateset')
    .option('--config-file <path>', 'Override x402 config file path')
    .option('--apply', 'Execute write operations', false)
    .option('--json', 'Output as JSON')
    .option('--output <file>', 'Write JSON output to file (implies --json)')
    .option('--yes, -y', 'Skip confirmation prompts', false);
}

const program = new Command();

program
  .name('stateset-x402')
  .description('x402 configuration and payment execution for StateSet iCommerce')
  .version(CLI_VERSION);

program
  .command('init')
  .description('Initialize x402 configuration and signing keys')
  .requiredOption('--sequencer-url <url>', 'Sequencer URL (https://...)')
  .requiredOption('--tenant-id <uuid>', 'Tenant UUID')
  .requiredOption('--store-id <uuid>', 'Store UUID')
  .requiredOption('--agent-id <id>', 'Agent ID (UUID or slug)')
  .option('--network <network>', 'Preferred payment network', 'set_chain')
  .option('--payer-address <address>', 'Payer wallet address (optional, derived if omitted)')
  .option('--config-dir <path>', 'Config directory (default: .stateset)', '.stateset')
  .option('--config-file <path>', 'Override x402 config file path')
  .option('--agent-key-id <id>', 'Signing key ID to pin')
  .option('--budget-per-call <amount>', 'Max amount per call (smallest unit)')
  .option('--budget-daily <amount>', 'Daily budget (smallest unit)')
  .option('--starting-balance <amount>', 'Initial local balance for tracking')
  .option('--max-amount <amount>', 'Absolute max amount allowed per payment')
  .option('--api-key <key>', 'Sequencer API key')
  .option('--jwt <token>', 'Sequencer JWT token')
  .option('--json', 'Output as JSON')
  .option('--output <file>', 'Write JSON output to file (implies --json)')
  .option('--force', 'Overwrite existing config', false)
  .action(async (options) => {
    const { jsonOutput, outputPath } = resolveOutputOptions(options);

    const spinner = ora({
      text: 'Initializing x402 configuration...',
      isEnabled: !jsonOutput,
    }).start();

    try {
      const configDir = options.configDir;
      const configPath = resolveX402ConfigPath({
        env: {},
        configDir,
        configFile: options.configFile,
      });

      if (fs.existsSync(configPath) && !options.force) {
        if (jsonOutput) {
          writeJson(outputPath, {
            error: 'x402 config already exists. Use --force to overwrite.',
            configPath,
          });
        } else {
          spinner.fail('x402 config already exists. Use --force to overwrite.');
        }
        process.exit(1);
      }

      const keyManager = getKeyManager(configDir);
      let signingKey = await keyManager.getCurrentSigningKey(options.agentId);
      if (!signingKey) {
        spinner.text = 'Generating signing key...';
        signingKey = await keyManager.generateSigningKey(options.agentId);
      }

      let payerAddress = options.payerAddress;
      if (!payerAddress) {
        spinner.text = 'Deriving wallet address...';
        payerAddress = await getWalletAddress(options.agentId, options.network, { configDir });
      }

      const config = {
        sequencerUrl: options.sequencerUrl,
        tenantId: options.tenantId,
        storeId: options.storeId,
        agentId: options.agentId,
        agentKeyId: options.agentKeyId ? Number(options.agentKeyId) : signingKey.keyId,
        payerAddress,
        preferredNetworks: [options.network],
        maxAmount: options.maxAmount ? Number(options.maxAmount) : undefined,
        maxAmountPerCall: options.budgetPerCall ? Number(options.budgetPerCall) : undefined,
        dailyBudget: options.budgetDaily ? Number(options.budgetDaily) : undefined,
        startingBalance: options.startingBalance ? Number(options.startingBalance) : undefined,
        apiKey: options.apiKey,
        jwt: options.jwt,
      };

      saveX402Config(configPath, config);

      if (jsonOutput) {
        writeJson(outputPath, {
          success: true,
          configPath,
          agentKeyId: config.agentKeyId,
          payerAddress,
          network: options.network,
          sequencerUrl: options.sequencerUrl,
          tenantId: options.tenantId,
          storeId: options.storeId,
          agentId: options.agentId,
        });
        return;
      }

      spinner.succeed('x402 configuration saved.');
      console.log();
      console.log(chalk.green(`Config file: ${configPath}`));
      console.log(chalk.green(`Signing key ID: ${config.agentKeyId}`));
      console.log(chalk.green(`Payer address: ${payerAddress}`));
      console.log();
      console.log(chalk.bold('Next steps:'));
      console.log(chalk.dim('1) Fund the payer address with your preferred stablecoin.'));
      console.log(chalk.dim('2) Run the MCP server:'));
      console.log(chalk.cyan(`   stateset-x402-mcp --config-dir ${configDir}`));
      console.log();
      console.log(chalk.dim('Environment override (optional):'));
      console.log(chalk.cyan(`   export X402_CONFIG_FILE=${configPath}`));
    } catch (error) {
      if (jsonOutput) {
        writeJson(outputPath, { error: `Initialization failed: ${error.message}` });
      } else {
        spinner.fail(`Initialization failed: ${error.message}`);
      }
      process.exit(1);
    }
  });

withRuntimeOptions(
  program
    .command('pay')
    .description('Create, sign, settle, and optionally record incoming settlement in one step')
    .requiredOption('--amount <smallest>', 'Amount in smallest unit (e.g., 1000000 = 1 USDC)')
    .option('--payer-agent <id>', 'Payer local agent ID (default: x402 config agentId)')
    .option('--payee-agent <id>', 'Payee local agent ID')
    .option('--payer-address <address>', 'Explicit payer wallet address')
    .option('--payee-address <address>', 'Explicit payee wallet address')
    .option('--asset <asset>', 'Asset symbol (default: usdc)')
    .option('--network <network>', 'x402 network override')
    .option('--chain <chain>', 'Settlement chain override')
    .option('--token <symbol>', 'Settlement token override')
    .option('--key-id <id>', 'Payer signing key ID')
    .option('--cart-id <id>', 'Cart ID')
    .option('--order-id <id>', 'Order ID')
    .option('--description <text>', 'Payment description')
    .option('--validity-seconds <seconds>', 'Intent validity window in seconds')
    .option('--record-incoming', 'Record incoming payee treasury settlement', true)
    .option('--no-record-incoming', 'Do not record incoming payee treasury settlement')
    .action(async (options) => {
      const amount = parseNumberOption(options.amount, '--amount');
      const keyId = parseIntegerOption(options.keyId, '--key-id');
      const validitySeconds = parseIntegerOption(options.validitySeconds, '--validity-seconds');
      const { fileConfig } = readX402Config(options);
      const defaultAgentId = resolveDefaultAgentId(options, fileConfig);
      const defaultNetwork = resolveDefaultNetwork(options, fileConfig);

      if (!options.payeeAgent && !options.payeeAddress) {
        throw new Error('pay requires either --payee-agent or --payee-address');
      }

      const confirmed = await confirmWriteOperation(options, {
        operation: 'x402 payment',
        details: `${amount} ${options.asset || 'usdc'} from ${options.payerAgent || defaultAgentId} to ${options.payeeAgent || options.payeeAddress}`,
      });
      if (!confirmed) {
        emitResult(options, { success: false, cancelled: true });
        return;
      }

      await executeToolCommand({
        spinnerText: 'Executing end-to-end x402 payment...',
        toolName: 'x402_execute_agent_payment',
        params: {
          amount,
          payerAgentId: options.payerAgent || undefined,
          payeeAgentId: options.payeeAgent || undefined,
          payerAddress: options.payerAddress || undefined,
          payeeAddress: options.payeeAddress || undefined,
          asset: options.asset || undefined,
          network: options.network || defaultNetwork,
          chain: options.chain || undefined,
          token: options.token || undefined,
          keyId,
          cartId: options.cartId || undefined,
          orderId: options.orderId || undefined,
          description: options.description || undefined,
          validitySeconds,
          recordIncoming: options.recordIncoming,
        },
        options,
        defaultAgentId,
        successMessage: 'x402 payment flow completed.',
      });
    }),
);

withRuntimeOptions(
  program
    .command('sign')
    .description('Sign an existing x402 intent (manual signature or local signer)')
    .requiredOption('--intent-id <id>', 'x402 intent ID')
    .option('--signature <hexOrBase64>', 'Manual payer signature')
    .option('--public-key <hexOrBase64>', 'Manual payer public key')
    .option('--agent-id <id>', 'Local signing agent ID (defaults from x402 config)')
    .option('--key-id <id>', 'Local signing key ID')
    .option('--chain <chain>', 'Chain override for wallet verification')
    .action(async (options) => {
      const keyId = parseIntegerOption(options.keyId, '--key-id');
      const { fileConfig } = readX402Config(options);
      const defaultAgentId = resolveDefaultAgentId(options, fileConfig);

      const confirmed = await confirmWriteOperation(options, {
        operation: 'x402 sign intent',
        details: options.intentId,
      });
      if (!confirmed) {
        emitResult(options, { success: false, cancelled: true });
        return;
      }

      await executeToolCommand({
        spinnerText: 'Signing x402 intent...',
        toolName: 'x402_sign_intent',
        params: {
          intentId: options.intentId,
          signature: options.signature || undefined,
          publicKey: options.publicKey || undefined,
          agentId: options.agentId || undefined,
          keyId,
          chain: options.chain || undefined,
        },
        options,
        defaultAgentId,
        successMessage: 'x402 intent signing completed.',
      });
    }),
);

withRuntimeOptions(
  program
    .command('settle')
    .description('Settle a signed x402 intent on-chain')
    .requiredOption('--intent-id <id>', 'x402 intent ID')
    .option('--agent-id <id>', 'Payer settlement agent ID (defaults from x402 config)')
    .option('--payee-agent <id>', 'Payee agent ID to record incoming settlement')
    .option('--chain <chain>', 'Settlement chain override')
    .option('--token <symbol>', 'Settlement token override')
    .action(async (options) => {
      const { fileConfig } = readX402Config(options);
      const defaultAgentId = resolveDefaultAgentId(options, fileConfig);

      const confirmed = await confirmWriteOperation(options, {
        operation: 'x402 settle intent',
        details: options.intentId,
      });
      if (!confirmed) {
        emitResult(options, { success: false, cancelled: true });
        return;
      }

      await executeToolCommand({
        spinnerText: 'Settling x402 intent on-chain...',
        toolName: 'x402_settle_intent_onchain',
        params: {
          intentId: options.intentId,
          agentId: options.agentId || undefined,
          payeeAgentId: options.payeeAgent || undefined,
          chain: options.chain || undefined,
          token: options.token || undefined,
        },
        options,
        defaultAgentId,
        successMessage: 'x402 settlement finished.',
      });
    }),
);

withRuntimeOptions(
  program
    .command('record-incoming')
    .description('Record settled x402 payment as incoming payee treasury credit')
    .requiredOption('--intent-id <id>', 'x402 intent ID')
    .requiredOption('--payee-agent <id>', 'Payee agent ID to credit')
    .option('--chain <chain>', 'Chain override')
    .option('--token <symbol>', 'Token override')
    .action(async (options) => {
      const { fileConfig } = readX402Config(options);
      const defaultAgentId = resolveDefaultAgentId(options, fileConfig);

      const confirmed = await confirmWriteOperation(options, {
        operation: 'x402 record incoming settlement',
        details: `${options.intentId} -> ${options.payeeAgent}`,
      });
      if (!confirmed) {
        emitResult(options, { success: false, cancelled: true });
        return;
      }

      await executeToolCommand({
        spinnerText: 'Recording incoming settlement...',
        toolName: 'x402_record_incoming_settlement',
        params: {
          intentId: options.intentId,
          payeeAgentId: options.payeeAgent,
          chain: options.chain || undefined,
          token: options.token || undefined,
        },
        options,
        defaultAgentId,
        successMessage: 'Incoming settlement recording completed.',
      });
    }),
);

const intent = program.command('intent').description('Inspect x402 intents');

withRuntimeOptions(
  intent
    .command('get')
    .description('Get an x402 payment intent by ID')
    .requiredOption('--intent-id <id>', 'x402 intent ID')
    .action(async (options) => {
      const { fileConfig } = readX402Config(options);
      const defaultAgentId = resolveDefaultAgentId(options, fileConfig);
      await executeToolCommand({
        spinnerText: 'Loading x402 intent...',
        toolName: 'x402_get_intent',
        params: { intentId: options.intentId },
        options,
        defaultAgentId,
        successMessage: 'x402 intent loaded.',
      });
    }),
);

withRuntimeOptions(
  intent
    .command('list')
    .description('List x402 payment intents')
    .option('--payer-address <address>', 'Filter by payer wallet')
    .option('--payee-address <address>', 'Filter by payee wallet')
    .option('--status <status>', 'Filter by intent status')
    .option('--network <network>', 'Filter by network')
    .option('--limit <count>', 'Max results (default: 50)')
    .action(async (options) => {
      const { fileConfig } = readX402Config(options);
      const defaultAgentId = resolveDefaultAgentId(options, fileConfig);
      const limit = parseIntegerOption(options.limit, '--limit');
      await executeToolCommand({
        spinnerText: 'Listing x402 intents...',
        toolName: 'x402_list_intents',
        params: {
          payerAddress: options.payerAddress || undefined,
          payeeAddress: options.payeeAddress || undefined,
          status: options.status || undefined,
          network: options.network || undefined,
          limit: limit || undefined,
        },
        options,
        defaultAgentId,
        successMessage: 'x402 intents loaded.',
      });
    }),
);

withRuntimeOptions(
  program
    .command('next-nonce')
    .description('Get the next nonce for a payer wallet address')
    .requiredOption('--payer-address <address>', 'Payer wallet address')
    .action(async (options) => {
      const { fileConfig } = readX402Config(options);
      const defaultAgentId = resolveDefaultAgentId(options, fileConfig);
      await executeToolCommand({
        spinnerText: 'Fetching next nonce...',
        toolName: 'x402_get_next_nonce',
        params: { payerAddress: options.payerAddress },
        options,
        defaultAgentId,
        successMessage: 'Nonce loaded.',
      });
    }),
);

program.parse(process.argv);

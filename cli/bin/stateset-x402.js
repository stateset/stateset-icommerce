#!/usr/bin/env node

/**
 * StateSet x402 CLI
 *
 * Initialize x402 configuration and keys for agent-to-agent payments.
 */

import { Command } from 'commander';
import chalk from 'chalk';
import ora from 'ora';
import fs from 'node:fs';
import { getKeyManager } from '../src/sync/keys.js';
import { getWalletAddress } from '../src/chains/index.js';
import { CLI_VERSION } from '../src/config.js';
import {
  resolveX402ConfigPath,
  saveX402Config,
} from '../src/x402/config.js';

const program = new Command();

program
  .name('stateset-x402')
  .description('x402 configuration and key management for StateSet iCommerce')
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
  .option('--force', 'Overwrite existing config', false)
  .action(async (options) => {
    const spinner = ora('Initializing x402 configuration...').start();

    try {
      const configDir = options.configDir;
      const configPath = resolveX402ConfigPath({
        env: {},
        configDir,
        configFile: options.configFile,
      });

      if (fs.existsSync(configPath) && !options.force) {
        spinner.fail('x402 config already exists. Use --force to overwrite.');
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
      spinner.fail(`Initialization failed: ${error.message}`);
      process.exit(1);
    }
  });

program.parse(process.argv);

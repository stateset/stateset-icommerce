#!/usr/bin/env node

/**
 * StateSet Pay - Native Stablecoin Payments for AI Agents
 *
 * Send stablecoin payments using agent-controlled wallets derived from VES keys.
 *
 * Usage:
 *   stateset pay --to <address> --amount <amount> [--chain <chain>] [--token <token>]
 *   stateset pay --wallet                     # Show agent wallet addresses
 *   stateset pay --balance                    # Check balances
 *
 * Examples:
 *   stateset pay --to 9WzD...WWWM --amount 50.00 --chain solana
 *   stateset pay --to 0x1234...5678 --amount 100 --chain set_chain --token ssUSD
 *   stateset pay --wallet --chain solana
 *   stateset pay --balance --chain solana
 */

import { parseArgs } from 'node:util';
import chalk from 'chalk';
import ora from 'ora';
import {
  executePayment,
  getBalance,
  hasSufficientBalance,
  getWalletAddress,
  listWalletAddresses,
  listChains,
  getChain,
  getToken,
  getDefaultStablecoin,
  getExplorerTxUrl,
  getExplorerAddressUrl,
  formatAmount,
} from '../src/chains/index.js';
import { getKeyManager } from '../src/sync/keys.js';
import { CLI_VERSION } from '../src/config.js';

// =============================================================================
// CLI OPTIONS
// =============================================================================

const options = {
  // Payment options
  to: { type: 'string', short: 't', description: 'Recipient wallet address' },
  amount: { type: 'string', short: 'a', description: 'Amount to send (e.g., 100.00)' },
  chain: { type: 'string', short: 'c', default: 'solana', description: 'Blockchain network' },
  token: { type: 'string', description: 'Token symbol (default: chain stablecoin)' },

  // Agent options
  agent: { type: 'string', default: 'default', description: 'Agent ID' },

  // Metadata
  order: { type: 'string', description: 'Order ID to associate with payment' },
  customer: { type: 'string', description: 'Customer ID to associate' },
  memo: { type: 'string', description: 'Payment memo/description' },

  // Query options
  wallet: { type: 'boolean', short: 'w', default: false, description: 'Show wallet address' },
  balance: { type: 'boolean', short: 'b', default: false, description: 'Check balance' },
  chains: { type: 'boolean', default: false, description: 'List supported chains' },

  // Execution options
  apply: { type: 'boolean', default: false, description: 'Actually execute (default: simulate)' },
  json: { type: 'boolean', default: false, description: 'JSON output' },

  // Help
  help: { type: 'boolean', short: 'h', default: false },
  version: { type: 'boolean', short: 'v', default: false },
};

const HELP = `
${chalk.bold.cyan('StateSet Pay')} - Native Stablecoin Payments for AI Agents

${chalk.bold('USAGE:')}
  stateset pay --to <address> --amount <amount> [options]
  stateset pay --wallet [--chain <chain>]
  stateset pay --balance [--chain <chain>]
  stateset pay --chains

${chalk.bold('PAYMENT OPTIONS:')}
  -t, --to <address>      Recipient wallet address
  -a, --amount <amount>   Amount to send (e.g., 50.00)
  -c, --chain <chain>     Blockchain network (default: solana)
      --token <symbol>    Token symbol (default: chain's stablecoin)
      --order <id>        Order ID for audit trail
      --customer <id>     Customer ID for audit trail
      --memo <text>       Payment memo

${chalk.bold('QUERY OPTIONS:')}
  -w, --wallet            Show agent wallet address(es)
  -b, --balance           Check wallet balance
      --chains            List supported blockchains

${chalk.bold('EXECUTION:')}
      --apply             Actually execute payment (default: simulate)
      --json              Output as JSON
      --agent <id>        Agent ID (default: 'default')

${chalk.bold('SUPPORTED CHAINS:')}
  ${chalk.green('solana')}         Solana mainnet (USDC) - Fast, cheap, proven
  ${chalk.green('solana_devnet')}  Solana devnet (USDC) - Testing
  ${chalk.green('set_chain')}      SET Chain L2 (ssUSD) - StateSet native, yield-bearing
  ${chalk.green('base')}           Base L2 (USDC) - Coinbase, low fees
  ${chalk.green('ethereum')}       Ethereum mainnet (USDC) - Maximum security
  ${chalk.green('arbitrum')}       Arbitrum L2 (USDC) - Fast, cheap

${chalk.bold('EXAMPLES:')}
  ${chalk.dim('# Send 50 USDC on Solana (simulated)')}
  stateset pay --to 9WzD...WWWM --amount 50.00 --chain solana

  ${chalk.dim('# Actually execute payment')}
  stateset pay --apply --to 9WzD...WWWM --amount 50.00

  ${chalk.dim('# Send ssUSD on SET Chain')}
  stateset pay --apply --to 0x1234...5678 --amount 100 --chain set_chain

  ${chalk.dim('# Check balance')}
  stateset pay --balance --chain solana

  ${chalk.dim('# Show wallet addresses for all chains')}
  stateset pay --wallet

${chalk.bold('SECURITY:')}
  • Wallets are derived from agent VES keys (Ed25519)
  • Private keys never leave the local system
  • All payments are recorded in VES audit trail
  • Use --apply to actually execute (safe by default)
`;

// =============================================================================
// MAIN
// =============================================================================

async function main() {
  const { values } = parseArgs({ options, allowPositionals: true });

  if (values.help) {
    console.log(HELP);
    process.exit(0);
  }

  if (values.version) {
    console.log(`@stateset/cli pay v${CLI_VERSION}`);
    process.exit(0);
  }

  // Handle query commands
  if (values.chains) {
    await showChains(values);
    return;
  }

  if (values.wallet) {
    await showWallet(values);
    return;
  }

  if (values.balance) {
    await showBalance(values);
    return;
  }

  // Payment execution requires --to and --amount
  if (!values.to || !values.amount) {
    console.error(chalk.red('Error: --to and --amount are required for payments'));
    console.error('Run stateset pay --help for usage');
    process.exit(1);
  }

  await executePaymentCommand(values);
}

// =============================================================================
// COMMANDS
// =============================================================================

/**
 * List supported chains
 */
async function showChains(values) {
  const chains = listChains();

  if (values.json) {
    const chainData = chains.map(id => {
      const chain = getChain(id);
      const stablecoin = getDefaultStablecoin(id);
      return {
        id,
        name: chain.name,
        network: chain.network,
        stablecoin: stablecoin?.symbol || null,
        explorerUrl: chain.explorerUrl,
      };
    });
    console.log(JSON.stringify(chainData, null, 2));
    return;
  }

  console.log(chalk.bold('\n📡 Supported Blockchains\n'));

  for (const chainId of chains) {
    const chain = getChain(chainId);
    const stablecoin = getDefaultStablecoin(chainId);

    console.log(`  ${chalk.green(chainId.padEnd(18))} ${chain.name.padEnd(20)} ${chalk.cyan(stablecoin?.symbol || '-')}`);
  }

  console.log();
}

/**
 * Show agent wallet addresses
 */
async function showWallet(values) {
  const { agent, chain } = values;
  const configDir = '.stateset';

  // Ensure agent has keys
  const keyManager = getKeyManager(configDir);
  await keyManager.ensureKeys(agent);

  if (chain && chain !== 'all') {
    // Single chain
    const address = await getWalletAddress(agent, chain, { configDir });
    const chainConfig = getChain(chain);

    if (values.json) {
      console.log(JSON.stringify({
        agent,
        chain,
        address,
        explorerUrl: getExplorerAddressUrl(chain, address),
      }, null, 2));
      return;
    }

    console.log(chalk.bold(`\n💰 Agent Wallet (${chainConfig.name})\n`));
    console.log(`  Agent:   ${chalk.cyan(agent)}`);
    console.log(`  Chain:   ${chalk.green(chain)}`);
    console.log(`  Address: ${chalk.yellow(address)}`);
    console.log(`  Explorer: ${getExplorerAddressUrl(chain, address)}`);
    console.log();
  } else {
    // All chains
    const addresses = await listWalletAddresses(agent, { configDir });

    if (values.json) {
      console.log(JSON.stringify({ agent, wallets: addresses }, null, 2));
      return;
    }

    console.log(chalk.bold(`\n💰 Agent Wallets\n`));
    console.log(`  Agent: ${chalk.cyan(agent)}\n`);

    for (const [chainId, address] of Object.entries(addresses)) {
      const chainConfig = getChain(chainId);
      console.log(`  ${chalk.green(chainId.padEnd(18))} ${chalk.yellow(address)}`);
    }

    console.log();
  }
}

/**
 * Show wallet balance
 */
async function showBalance(values) {
  const { agent, chain, token } = values;
  const configDir = '.stateset';

  const spinner = ora('Checking balance...').start();

  try {
    const address = await getWalletAddress(agent, chain, { configDir });
    const balanceInfo = await getBalance(address, chain, token);

    spinner.stop();

    if (values.json) {
      console.log(JSON.stringify({
        agent,
        chain,
        address,
        balance: balanceInfo.balance,
        symbol: balanceInfo.symbol,
      }, null, 2));
      return;
    }

    console.log(chalk.bold(`\n💵 Wallet Balance\n`));
    console.log(`  Agent:   ${chalk.cyan(agent)}`);
    console.log(`  Chain:   ${chalk.green(chain)}`);
    console.log(`  Address: ${chalk.yellow(address)}`);
    console.log(`  Balance: ${chalk.bold.green(balanceInfo.balance)} ${balanceInfo.symbol}`);
    console.log();

  } catch (error) {
    spinner.fail(`Failed to check balance: ${error.message}`);
    process.exit(1);
  }
}

/**
 * Execute a stablecoin payment
 */
async function executePaymentCommand(values) {
  const {
    to,
    amount,
    chain,
    token,
    agent,
    order,
    customer,
    memo,
    apply,
    json,
  } = values;

  const configDir = '.stateset';
  const simulate = !apply;

  // Build metadata
  const metadata = {};
  if (order) metadata.order_id = order;
  if (customer) metadata.customer_id = customer;
  if (memo) metadata.memo = memo;

  // Get chain and token info for display
  const chainConfig = getChain(chain);
  const tokenConfig = token ? getToken(chain, token) : getDefaultStablecoin(chain);

  if (!chainConfig) {
    console.error(chalk.red(`Unknown chain: ${chain}`));
    console.error(`Run 'stateset pay --chains' to see supported chains`);
    process.exit(1);
  }

  if (!tokenConfig) {
    console.error(chalk.red(`No stablecoin found for chain ${chain}`));
    process.exit(1);
  }

  // Show header
  if (!json) {
    console.log(chalk.bold(`\n${simulate ? '🔍 Payment Preview' : '💸 Executing Payment'}\n`));
    console.log(`  Chain:     ${chalk.green(chainConfig.name)}`);
    console.log(`  Token:     ${chalk.cyan(tokenConfig.symbol)}`);
    console.log(`  Amount:    ${chalk.bold.yellow(amount)} ${tokenConfig.symbol}`);
    console.log(`  To:        ${chalk.yellow(to)}`);
    console.log(`  Agent:     ${chalk.cyan(agent)}`);
    if (simulate) {
      console.log(`  Mode:      ${chalk.yellow('SIMULATION (use --apply to execute)')}`);
    }
    console.log();
  }

  // Check balance first
  const spinner = ora('Checking balance...').start();

  try {
    const agentAddress = await getWalletAddress(agent, chain, { configDir });
    const balanceCheck = await hasSufficientBalance(agentAddress, chain, parseFloat(amount), token);

    if (!balanceCheck.sufficient) {
      spinner.fail(chalk.red(`Insufficient balance`));
      console.log(`  Balance:  ${balanceCheck.balance} ${balanceCheck.symbol}`);
      console.log(`  Required: ${balanceCheck.required} ${balanceCheck.symbol}`);
      process.exit(1);
    }

    spinner.succeed(`Balance: ${balanceCheck.balance} ${balanceCheck.symbol}`);

  } catch (error) {
    spinner.warn(`Could not verify balance: ${error.message}`);
  }

  // Execute payment
  const paymentSpinner = ora(simulate ? 'Simulating payment...' : 'Executing payment...').start();

  const result = await executePayment({
    agentId: agent,
    chainId: chain,
    toAddress: to,
    amount: parseFloat(amount),
    tokenSymbol: token,
    metadata,
  }, {
    configDir,
    simulate,
    onProgress: (progress) => {
      paymentSpinner.text = progress.message;
    },
  });

  if (result.success) {
    if (simulate) {
      paymentSpinner.succeed(chalk.green('Simulation successful'));

      if (!json) {
        console.log(chalk.dim('\n  Run with --apply to execute this payment\n'));
      }
    } else {
      paymentSpinner.succeed(chalk.green('Payment confirmed!'));

      if (!json) {
        console.log(`\n  Transaction: ${chalk.cyan(result.txHash)}`);
        console.log(`  Explorer:    ${result.explorerUrl}`);
        console.log(`  Block:       ${result.blockNumber}`);
        console.log(`  Confirms:    ${result.confirmations}`);
        console.log();
      }
    }

    if (json) {
      console.log(JSON.stringify(result, (key, value) =>
        typeof value === 'bigint' ? value.toString() : value
      , null, 2));
    }

  } else {
    paymentSpinner.fail(chalk.red(`Payment failed: ${result.error}`));

    if (json) {
      console.log(JSON.stringify(result, null, 2));
    }

    process.exit(1);
  }
}

// =============================================================================
// RUN
// =============================================================================

main().catch(error => {
  console.error(chalk.red(`Error: ${error.message}`));
  process.exit(1);
});

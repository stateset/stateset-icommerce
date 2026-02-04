#!/usr/bin/env node

/**
 * StateSet Treasury CLI
 *
 * Manage agent funding, balances, and token purchases.
 */

import { Command } from 'commander';
import chalk from 'chalk';
import fs from 'node:fs';

import { CLI_VERSION } from '../src/config.js';
import { createConfirmHandler } from '../src/utils/confirm.js';
import {
  loadTreasuryContext,
  listTokens,
  addRegistryToken,
  removeRegistryToken,
  addPricingRule,
  removePricingRuleEntry,
  resolveToken,
  computeBalanceDisplay,
  recordDeposit,
  buyTokens,
  syncOnChainBalance,
  ensureAgentWallet
} from '../src/treasury/index.js';
import { listWalletAddresses, getWalletAddress } from '../src/chains/wallet.js';
import { getDefaultStablecoin } from '../src/chains/config.js';
import {
  registerIdentity,
  setAgentWallet,
  getIdentity,
  getIdentityByWallet,
  listIdentities
} from '../src/erc8004/index.js';

const program = new Command();

function resolveOutputOptions(commandOptions = {}) {
  const globalOptions = program.opts();
  const merged = { ...globalOptions, ...commandOptions };
  const jsonOutput = Boolean(merged.json || merged.output);
  const outputPath = merged.output || null;
  return { jsonOutput, outputPath };
}

function writeJsonOutput(outputPath, data) {
  const payload = JSON.stringify(data, null, 2);
  if (outputPath) {
    fs.writeFileSync(outputPath, payload);
    return;
  }
  console.log(payload);
}

function printSection(title) {
  console.log(`\n${chalk.bold(title)}`);
}

function formatTokenEntry(entry) {
  return `${entry.symbol} (${entry.chainId})${entry.priceUsd ? ` @ $${entry.priceUsd}` : ''}`;
}

program
  .name('stateset-treasury')
  .description('Agent treasury management for funding and token purchases')
  .version(CLI_VERSION)
  .option('--db <path>', 'Treasury database path', null)
  .option('--registry <path>', 'Token registry path', null)
  .option('--commerce-db <path>', 'Commerce database path for ERC-8004', './store.db')
  .option('--json', 'JSON output')
  .option('--output <file>', 'Write JSON output to file')
  .option('--apply', 'Apply changes (required for writes)', false)
  .option('-y, --yes', 'Skip confirmation prompts', false);

program
  .command('init')
  .description('Initialize treasury storage and registry')
  .action(async () => {
    const { jsonOutput, outputPath } = resolveOutputOptions();
    const { db: dbPath, registry: registryPath } = program.opts();
    const ctx = await loadTreasuryContext({ dbPath, registryPath });

    if (jsonOutput) {
      writeJsonOutput(outputPath, {
        ok: true,
        dbPath: ctx.dbPath,
        registryPath: ctx.registryPath,
        tokenCount: ctx.registry.tokens.length
      });
      return;
    }

    printSection('Treasury Initialized');
    console.log(`   DB: ${ctx.dbPath}`);
    console.log(`   Registry: ${ctx.registryPath}`);
    console.log(`   Tokens: ${ctx.registry.tokens.length}`);
  });

program
  .command('wallet')
  .description('Show agent wallet address')
  .option('--agent <id>', 'Agent ID', 'default')
  .option('--chain <chain>', 'Chain ID')
  .option('--all', 'List addresses for all chains', false)
  .action(async (options) => {
    const { jsonOutput, outputPath } = resolveOutputOptions(options);

    if (options.all) {
      const addresses = await listWalletAddresses(options.agent, { configDir: '.stateset' });
      if (jsonOutput) {
        writeJsonOutput(outputPath, { agentId: options.agent, addresses });
        return;
      }
      printSection(`Wallets for ${options.agent}`);
      for (const [chainId, address] of Object.entries(addresses)) {
        console.log(`   ${chainId}: ${address}`);
      }
      return;
    }

    if (!options.chain) {
      throw new Error('wallet requires --chain or --all');
    }

    await ensureAgentWallet(options.agent, options.chain, '.stateset');
    const address = await getWalletAddress(options.agent, options.chain, { configDir: '.stateset' });

    if (jsonOutput) {
      writeJsonOutput(outputPath, { agentId: options.agent, chainId: options.chain, address });
      return;
    }

    printSection('Wallet Address');
    console.log(`   Agent: ${options.agent}`);
    console.log(`   Chain: ${options.chain}`);
    console.log(`   Address: ${address}`);
  });

program
  .command('balance')
  .description('Show treasury balances for an agent')
  .option('--agent <id>', 'Agent ID', 'default')
  .option('--chain <chain>', 'Chain ID')
  .option('--token <symbol>', 'Token symbol')
  .action(async (options) => {
    const { jsonOutput, outputPath } = resolveOutputOptions(options);
    const { db: dbPath, registry: registryPath } = program.opts();
    const ctx = await loadTreasuryContext({ dbPath, registryPath });

    if (options.token) {
      if (!options.chain) {
        throw new Error('balance with --token requires --chain');
      }
      const token = resolveToken(options.chain, options.token, ctx.registry);
      if (!token) {
        throw new Error(`Unknown token ${options.token} on ${options.chain}`);
      }
      const balance = ctx.store.getBalance({
        agentId: options.agent,
        chainId: options.chain,
        tokenSymbol: token.symbol,
        tokenDecimals: token.decimals
      });
      const display = computeBalanceDisplay(balance.balanceSmallest, token.decimals);

      if (jsonOutput) {
        writeJsonOutput(outputPath, {
          agentId: options.agent,
          chainId: options.chain,
          token: token.symbol,
          balance: display,
          balanceSmallest: balance.balanceSmallest.toString()
        });
        return;
      }

      printSection('Treasury Balance');
      console.log(`   Agent: ${options.agent}`);
      console.log(`   Chain: ${options.chain}`);
      console.log(`   Token: ${token.symbol}`);
      console.log(`   Balance: ${display}`);
      return;
    }

    const balances = ctx.store.getBalances({ agentId: options.agent, chainId: options.chain || null });

    if (jsonOutput) {
      writeJsonOutput(outputPath, {
        agentId: options.agent,
        chainId: options.chain || null,
        balances: balances.map(b => ({
          chainId: b.chainId,
          token: b.tokenSymbol,
          balance: computeBalanceDisplay(b.balanceSmallest, b.tokenDecimals || 0),
          balanceSmallest: b.balanceSmallest.toString()
        }))
      });
      return;
    }

    printSection('Treasury Balances');
    if (balances.length === 0) {
      console.log('   No balances recorded yet.');
      return;
    }

    for (const balance of balances) {
      const display = computeBalanceDisplay(balance.balanceSmallest, balance.tokenDecimals || 0);
      console.log(`   ${balance.chainId} ${balance.tokenSymbol}: ${display}`);
    }
  });

program
  .command('deposit')
  .description('Record a treasury deposit (funds sent to agent wallet)')
  .requiredOption('--agent <id>', 'Agent ID')
  .requiredOption('--chain <chain>', 'Chain ID')
  .requiredOption('--token <symbol>', 'Token symbol')
  .requiredOption('--amount <amount>', 'Amount to deposit')
  .option('--tx <hash>', 'Transaction hash')
  .option('--from <address>', 'Sender wallet address')
  .option('--task <id>', 'Task or tool call id for audit')
  .action(async (options) => {
    const { jsonOutput, outputPath } = resolveOutputOptions(options);
    const globalOptions = program.opts();
    const { db: dbPath, registry: registryPath, apply, yes } = globalOptions;
    const ctx = await loadTreasuryContext({ dbPath, registryPath });

    const preview = {
      agentId: options.agent,
      chainId: options.chain,
      token: options.token,
      amount: options.amount,
      txId: options.tx || null,
      from: options.from || null,
      taskId: options.task || null
    };

    if (!apply) {
      if (jsonOutput) {
        writeJsonOutput(outputPath, { preview, apply: false });
        return;
      }
      printSection('Deposit Preview');
      console.log(`   Agent: ${options.agent}`);
      console.log(`   Chain: ${options.chain}`);
      console.log(`   Token: ${options.token}`);
      console.log(`   Amount: ${options.amount}`);
      console.log('   (Use --apply to record this deposit)');
      return;
    }

    const nonInteractive = !process.stdin.isTTY || jsonOutput;
    const confirm = createConfirmHandler({
      output: { yellow: chalk.yellow, bold: chalk.bold },
      assumeYes: yes,
      nonInteractive
    });

    const ok = await confirm({
      operation: 'treasury deposit',
      details: `${options.amount} ${options.token} on ${options.chain}`,
      amount: Number(options.amount)
    });

    if (!ok) {
      return;
    }

    const entry = await recordDeposit({
      agentId: options.agent,
      chainId: options.chain,
      tokenSymbol: options.token,
      amount: options.amount,
      txId: options.tx || null,
      fromAddress: options.from || null,
      source: 'manual',
      taskId: options.task || null
    }, ctx);

    if (jsonOutput) {
      writeJsonOutput(outputPath, { success: true, entry });
      return;
    }

    printSection('Deposit Recorded');
    console.log(`   Agent: ${options.agent}`);
    console.log(`   Chain: ${options.chain}`);
    console.log(`   Token: ${options.token}`);
    console.log(`   Amount: ${entry.amount_display}`);
    if (entry.tx_id) console.log(`   Tx: ${entry.tx_id}`);
  });

program
  .command('buy')
  .description('Purchase tokens using treasury funds')
  .requiredOption('--agent <id>', 'Agent ID')
  .requiredOption('--chain <chain>', 'Chain ID')
  .requiredOption('--to <symbol>', 'Target token symbol')
  .requiredOption('--amount <amount>', 'Amount of stablecoin to spend')
  .option('--from <symbol>', 'Funding token symbol (default: chain stablecoin)')
  .option('--price <usd>', 'Override price in USD for target token')
  .option('--slippage <pct>', 'Slippage percentage', '1')
  .option('--task <id>', 'Task or tool call id for audit')
  .action(async (options) => {
    const { jsonOutput, outputPath } = resolveOutputOptions(options);
    const globalOptions = program.opts();
    const { db: dbPath, registry: registryPath, apply, yes } = globalOptions;
    const ctx = await loadTreasuryContext({ dbPath, registryPath });

    const fromSymbol = options.from || getDefaultStablecoin(options.chain)?.symbol;
    if (!fromSymbol) {
      throw new Error(`No default stablecoin configured for ${options.chain}`);
    }
    const preview = {
      agentId: options.agent,
      chainId: options.chain,
      from: fromSymbol,
      to: options.to,
      amount: options.amount,
      priceUsd: options.price || null,
      slippagePct: Number(options.slippage),
      taskId: options.task || null
    };

    if (!apply) {
      if (jsonOutput) {
        writeJsonOutput(outputPath, { preview, apply: false });
        return;
      }
      printSection('Purchase Preview');
      console.log(`   Agent: ${options.agent}`);
      console.log(`   Chain: ${options.chain}`);
      console.log(`   Spend: ${options.amount} ${fromSymbol}`);
      console.log(`   Buy: ${options.to}`);
      console.log('   (Use --apply to execute)');
      return;
    }

    const nonInteractive = !process.stdin.isTTY || jsonOutput;
    const confirm = createConfirmHandler({
      output: { yellow: chalk.yellow, bold: chalk.bold },
      assumeYes: yes,
      nonInteractive
    });

    const ok = await confirm({
      operation: 'treasury buy',
      details: `${options.amount} ${fromSymbol} -> ${options.to}`,
      amount: Number(options.amount)
    });

    if (!ok) return;

    const result = await buyTokens({
      agentId: options.agent,
      chainId: options.chain,
      fromSymbol: fromSymbol,
      toSymbol: options.to,
      amount: options.amount,
      priceUsd: options.price ? Number(options.price) : null,
      slippagePct: Number(options.slippage),
      taskId: options.task || null
    }, ctx);

    if (jsonOutput) {
      writeJsonOutput(outputPath, { success: true, result });
      return;
    }

    printSection('Purchase Executed');
    console.log(`   Agent: ${options.agent}`);
    console.log(`   Chain: ${options.chain}`);
    console.log(`   Spend: ${result.from.amount} ${result.from.symbol}`);
    console.log(`   Receive: ${result.to.amount} ${result.to.symbol}`);
    console.log(`   Price: $${result.priceUsd}`);
    console.log(`   Slippage: ${result.slippagePct}%`);
  });

const token = program.command('token').description('Manage treasury token registry');

token
  .command('add')
  .description('Add or update a token in the registry')
  .requiredOption('--symbol <symbol>', 'Token symbol')
  .requiredOption('--chain <chain>', 'Chain ID')
  .option('--decimals <n>', 'Token decimals', '18')
  .option('--address <address>', 'Token contract address')
  .option('--name <name>', 'Token name')
  .option('--price <usd>', 'Token price in USD')
  .option('--issuer <agent>', 'Issuing agent ID')
  .action(async (options) => {
    const { jsonOutput, outputPath } = resolveOutputOptions(options);
    const globalOptions = program.opts();
    const { db: dbPath, registry: registryPath, apply } = globalOptions;
    const ctx = await loadTreasuryContext({ dbPath, registryPath });

    if (!apply) {
      if (jsonOutput) {
        writeJsonOutput(outputPath, { preview: options, apply: false });
        return;
      }
      printSection('Registry Preview');
      console.log(`   Symbol: ${options.symbol}`);
      console.log(`   Chain: ${options.chain}`);
      console.log(`   (Use --apply to save)`);
      return;
    }

    const updated = await addRegistryToken(ctx.registryPath, ctx.registry, {
      symbol: options.symbol,
      chainId: options.chain,
      decimals: Number(options.decimals),
      address: options.address || null,
      name: options.name || null,
      priceUsd: options.price ? Number(options.price) : null,
      issuerAgentId: options.issuer || null
    });

    if (jsonOutput) {
      writeJsonOutput(outputPath, { success: true, tokens: updated.tokens });
      return;
    }

    printSection('Token Saved');
    console.log(`   ${options.symbol} (${options.chain})`);
  });

token
  .command('list')
  .description('List tokens from chain config and registry')
  .option('--chain <chain>', 'Chain ID')
  .action(async (options) => {
    const { jsonOutput, outputPath } = resolveOutputOptions(options);
    const globalOptions = program.opts();
    const { db: dbPath, registry: registryPath } = globalOptions;
    const ctx = await loadTreasuryContext({ dbPath, registryPath });

    const tokens = listTokens(options.chain || null, ctx.registry);

    if (jsonOutput) {
      writeJsonOutput(outputPath, { tokens });
      return;
    }

    printSection('Tokens');
    if (tokens.length === 0) {
      console.log('   No tokens found.');
      return;
    }

    for (const entry of tokens) {
      console.log(`   ${formatTokenEntry(entry)} [${entry.source}]`);
    }
  });

token
  .command('remove')
  .description('Remove a token from the registry')
  .requiredOption('--symbol <symbol>', 'Token symbol')
  .requiredOption('--chain <chain>', 'Chain ID')
  .action(async (options) => {
    const { jsonOutput, outputPath } = resolveOutputOptions(options);
    const globalOptions = program.opts();
    const { db: dbPath, registry: registryPath, apply } = globalOptions;
    const ctx = await loadTreasuryContext({ dbPath, registryPath });

    if (!apply) {
      if (jsonOutput) {
        writeJsonOutput(outputPath, { preview: options, apply: false });
        return;
      }
      printSection('Registry Preview');
      console.log(`   Remove: ${options.symbol} (${options.chain})`);
      console.log('   (Use --apply to remove)');
      return;
    }

    const updated = await removeRegistryToken(ctx.registryPath, ctx.registry, options.symbol, options.chain);

    if (jsonOutput) {
      writeJsonOutput(outputPath, { success: true, tokens: updated.tokens });
      return;
    }

    printSection('Token Removed');
    console.log(`   ${options.symbol} (${options.chain})`);
  });

program
  .command('ledger')
  .description('List recent treasury transactions')
  .requiredOption('--agent <id>', 'Agent ID')
  .option('--chain <chain>', 'Chain ID')
  .option('--token <symbol>', 'Token symbol')
  .option('--task <id>', 'Task id filter')
  .option('--request <id>', 'Request id filter')
  .option('--limit <n>', 'Max entries', '25')
  .action(async (options) => {
    const { jsonOutput, outputPath } = resolveOutputOptions(options);
    const { db: dbPath, registry: registryPath } = program.opts();
    const ctx = await loadTreasuryContext({ dbPath, registryPath });

    const entries = ctx.store.list({
      agentId: options.agent,
      chainId: options.chain || null,
      tokenSymbol: options.token ? options.token.toUpperCase() : null,
      taskId: options.task || null,
      requestId: options.request || null,
      limit: Number(options.limit)
    });

    if (jsonOutput) {
      writeJsonOutput(outputPath, { entries });
      return;
    }

    printSection('Treasury Ledger');
    if (entries.length === 0) {
      console.log('   No entries recorded.');
      return;
    }

    for (const entry of entries) {
      const sign = entry.direction === 'deposit' || entry.direction === 'swap_in' ? '+' : '-';
      console.log(`   ${new Date(entry.created_at).toISOString()} ${entry.chain_id} ${entry.token_symbol} ${sign}${entry.amount_display} (${entry.direction})`);
    }
  });

program
  .command('sync')
  .description('Sync on-chain balances and record deltas')
  .requiredOption('--agent <id>', 'Agent ID')
  .requiredOption('--chain <chain>', 'Chain ID (EVM only)')
  .requiredOption('--token <symbol>', 'Token symbol')
  .action(async (options) => {
    const { jsonOutput, outputPath } = resolveOutputOptions(options);
    const globalOptions = program.opts();
    const { db: dbPath, registry: registryPath, apply, yes } = globalOptions;
    const ctx = await loadTreasuryContext({ dbPath, registryPath });

    if (!apply && !jsonOutput) {
      console.log(chalk.yellow('Running sync in preview mode. Use --apply to record deltas.'));
    }

    const nonInteractive = !process.stdin.isTTY || jsonOutput;
    const confirm = createConfirmHandler({
      output: { yellow: chalk.yellow, bold: chalk.bold },
      assumeYes: yes,
      nonInteractive
    });

    if (apply) {
      const ok = await confirm({
        operation: 'treasury sync',
        details: `${options.chain} ${options.token} for ${options.agent}`
      });
      if (!ok) return;
    }

    const result = await syncOnChainBalance({
      agentId: options.agent,
      chainId: options.chain,
      tokenSymbol: options.token,
      apply
    }, ctx);

    if (jsonOutput) {
      writeJsonOutput(outputPath, result);
      return;
    }

    if (!result.updated) {
      printSection('Sync Complete');
      console.log('   No balance changes detected.');
      return;
    }

    printSection('Sync Complete');
    console.log(`   Direction: ${result.direction}`);
    console.log(`   Delta: ${result.delta}`);
    console.log(`   On-chain: ${result.onChain}`);
    console.log(`   Ledger: ${result.ledger}`);
    console.log(`   Applied: ${result.applied ? 'yes' : 'no'}`);
  });

pricing
  .command('set')
  .description('Set pricing for a tool')
  .requiredOption('--tool <name>', 'Tool name (e.g., list_orders)')
  .requiredOption('--chain <chain>', 'Chain ID for billing')
  .requiredOption('--token <symbol>', 'Token symbol used for billing')
  .requiredOption('--amount <amount>', 'Amount per call')
  .option('--description <text>', 'Description')
  .action(async (options) => {
    const { jsonOutput, outputPath } = resolveOutputOptions(options);
    const globalOptions = program.opts();
    const { db: dbPath, registry: registryPath, apply } = globalOptions;
    const ctx = await loadTreasuryContext({ dbPath, registryPath });

    if (!apply) {
      if (jsonOutput) {
        writeJsonOutput(outputPath, { preview: options, apply: false });
        return;
      }
      printSection('Pricing Preview');
      console.log(`   Tool: ${options.tool}`);
      console.log(`   Charge: ${options.amount} ${options.token} on ${options.chain}`);
      console.log('   (Use --apply to save)');
      return;
    }

    const updated = await addPricingRule(ctx.pricingPath, ctx.pricing, {
      tool: options.tool,
      chainId: options.chain,
      tokenSymbol: options.token,
      amount: Number(options.amount),
      description: options.description || null
    });

    if (jsonOutput) {
      writeJsonOutput(outputPath, { success: true, rules: updated.rules });
      return;
    }

    printSection('Pricing Saved');
    console.log(`   ${options.tool}: ${options.amount} ${options.token} (${options.chain})`);
  });

pricing
  .command('list')
  .description('List tool pricing rules')
  .action(async (options) => {
    const { jsonOutput, outputPath } = resolveOutputOptions(options);
    const globalOptions = program.opts();
    const { db: dbPath, registry: registryPath } = globalOptions;
    const ctx = await loadTreasuryContext({ dbPath, registryPath });

    if (jsonOutput) {
      writeJsonOutput(outputPath, { rules: ctx.pricing.rules || [] });
      return;
    }

    printSection('Pricing Rules');
    if (!ctx.pricing.rules || ctx.pricing.rules.length === 0) {
      console.log('   No pricing rules configured.');
      return;
    }

    for (const rule of ctx.pricing.rules) {
      console.log(`   ${rule.tool}: ${rule.amount} ${rule.tokenSymbol} (${rule.chainId})`);
    }
  });

pricing
  .command('remove')
  .description('Remove a pricing rule')
  .requiredOption('--tool <name>', 'Tool name')
  .requiredOption('--chain <chain>', 'Chain ID')
  .action(async (options) => {
    const { jsonOutput, outputPath } = resolveOutputOptions(options);
    const globalOptions = program.opts();
    const { db: dbPath, registry: registryPath, apply } = globalOptions;
    const ctx = await loadTreasuryContext({ dbPath, registryPath });

    if (!apply) {
      if (jsonOutput) {
        writeJsonOutput(outputPath, { preview: options, apply: false });
        return;
      }
      printSection('Pricing Preview');
      console.log(`   Remove: ${options.tool} (${options.chain})`);
      console.log('   (Use --apply to remove)');
      return;
    }

    const updated = await removePricingRuleEntry(ctx.pricingPath, ctx.pricing, options.tool, options.chain);

    if (jsonOutput) {
      writeJsonOutput(outputPath, { success: true, rules: updated.rules });
      return;
    }

    printSection('Pricing Removed');
    console.log(`   ${options.tool} (${options.chain})`);
  });

const identity = program.command('identity').description('ERC-8004 identity registry helpers');

const pricing = program.command('pricing').description('Configure per-tool pricing');

identity
  .command('register')
  .description('Register or update an ERC-8004 identity')
  .requiredOption('--registry <uri>', 'Agent registry URI')
  .requiredOption('--agent-id <id>', 'ERC-8004 agent ID')
  .requiredOption('--uri <uri>', 'Agent URI')
  .option('--wallet <address>', 'Agent wallet address')
  .option('--owner <address>', 'Owner address')
  .option('--card <id>', 'Agent card ID')
  .option('--registration <data>', 'Registration payload')
  .option('--registration-hash <hash>', 'Registration hash')
  .option('--proof-type <type>', 'Wallet proof type: eip712|erc1271')
  .option('--proof <value>', 'Wallet proof signature')
  .option('--proof-chain <id>', 'Wallet proof chain id')
  .option('--proof-deadline <iso>', 'Wallet proof deadline (ISO)')
  .option('--active', 'Mark identity active', true)
  .action(async (options) => {
    const { jsonOutput, outputPath } = resolveOutputOptions(options);
    const globalOptions = program.opts();
    const { commerceDb, apply, yes } = globalOptions;

    if (!apply) {
      if (jsonOutput) {
        writeJsonOutput(outputPath, { preview: options, apply: false });
        return;
      }
      printSection('Identity Preview');
      console.log(`   Registry: ${options.registry}`);
      console.log(`   Agent ID: ${options.agentId}`);
      console.log(`   URI: ${options.uri}`);
      console.log('   (Use --apply to register)');
      return;
    }

    const nonInteractive = !process.stdin.isTTY || jsonOutput;
    const confirm = createConfirmHandler({
      output: { yellow: chalk.yellow, bold: chalk.bold },
      assumeYes: yes,
      nonInteractive
    });

    const ok = await confirm({
      operation: 'erc8004 register',
      details: `${options.registry}:${options.agentId}`
    });

    if (!ok) return;

    const identity = registerIdentity(commerceDb, {
      agentRegistry: options.registry,
      agentId: options.agentId,
      agentUri: options.uri,
      agentWallet: options.wallet || null,
      ownerAddress: options.owner || null,
      agentCardId: options.card || null,
      registration: options.registration || null,
      registrationHash: options.registrationHash || null,
      walletProofType: options.proofType || null,
      walletProof: options.proof || null,
      walletProofChainId: options.proofChain ? Number(options.proofChain) : null,
      walletProofDeadline: options.proofDeadline || null,
      active: options.active !== false
    });

    if (jsonOutput) {
      writeJsonOutput(outputPath, { success: true, identity });
      return;
    }

    printSection('Identity Registered');
    console.log(`   Registry: ${identity.agent_registry}`);
    console.log(`   Agent ID: ${identity.agent_id}`);
    if (identity.agent_wallet) console.log(`   Wallet: ${identity.agent_wallet}`);
  });

identity
  .command('link-wallet')
  .description('Link a derived agent wallet to an ERC-8004 identity')
  .requiredOption('--registry <uri>', 'Agent registry URI')
  .requiredOption('--agent-id <id>', 'ERC-8004 agent ID')
  .requiredOption('--chain <chain>', 'Chain ID to derive wallet')
  .option('--agent <id>', 'Local agent ID for wallet derivation', 'default')
  .option('--proof-type <type>', 'Wallet proof type: eip712|erc1271')
  .option('--proof <value>', 'Wallet proof signature')
  .option('--proof-chain <id>', 'Wallet proof chain id')
  .option('--proof-deadline <iso>', 'Wallet proof deadline (ISO)')
  .action(async (options) => {
    const { jsonOutput, outputPath } = resolveOutputOptions(options);
    const globalOptions = program.opts();
    const { commerceDb, apply, yes } = globalOptions;

    const wallet = await ensureAgentWallet(options.agent, options.chain, '.stateset');

    if (!apply) {
      if (jsonOutput) {
        writeJsonOutput(outputPath, {
          preview: {
            registry: options.registry,
            agentId: options.agentId,
            chain: options.chain,
            wallet: wallet.address
          },
          apply: false
        });
        return;
      }
      printSection('Link Wallet Preview');
      console.log(`   Registry: ${options.registry}`);
      console.log(`   Agent ID: ${options.agentId}`);
      console.log(`   Chain: ${options.chain}`);
      console.log(`   Wallet: ${wallet.address}`);
      console.log('   (Use --apply to link)');
      return;
    }

    const nonInteractive = !process.stdin.isTTY || jsonOutput;
    const confirm = createConfirmHandler({
      output: { yellow: chalk.yellow, bold: chalk.bold },
      assumeYes: yes,
      nonInteractive
    });

    const ok = await confirm({
      operation: 'erc8004 link wallet',
      details: `${options.registry}:${options.agentId}`
    });

    if (!ok) return;

    const identity = setAgentWallet(commerceDb, {
      agentRegistry: options.registry,
      agentId: options.agentId,
      agentWallet: wallet.address,
      walletProofType: options.proofType || null,
      walletProof: options.proof || null,
      walletProofChainId: options.proofChain ? Number(options.proofChain) : null,
      walletProofDeadline: options.proofDeadline || null
    });

    if (jsonOutput) {
      writeJsonOutput(outputPath, { success: true, identity });
      return;
    }

    printSection('Wallet Linked');
    console.log(`   Registry: ${identity.agent_registry}`);
    console.log(`   Agent ID: ${identity.agent_id}`);
    console.log(`   Wallet: ${identity.agent_wallet}`);
  });

identity
  .command('get')
  .description('Get an ERC-8004 identity by registry + agent id')
  .requiredOption('--registry <uri>', 'Agent registry URI')
  .requiredOption('--agent-id <id>', 'ERC-8004 agent ID')
  .action(async (options) => {
    const { jsonOutput, outputPath } = resolveOutputOptions(options);
    const { commerceDb } = program.opts();
    const identity = getIdentity(commerceDb, options.registry, options.agentId);

    if (jsonOutput) {
      writeJsonOutput(outputPath, { identity });
      return;
    }

    if (!identity) {
      console.log('Identity not found.');
      return;
    }

    printSection('Identity');
    console.log(`   Registry: ${identity.agent_registry}`);
    console.log(`   Agent ID: ${identity.agent_id}`);
    console.log(`   URI: ${identity.agent_uri}`);
    if (identity.agent_wallet) console.log(`   Wallet: ${identity.agent_wallet}`);
  });

identity
  .command('by-wallet')
  .description('Find an ERC-8004 identity by wallet address')
  .requiredOption('--wallet <address>', 'Wallet address')
  .action(async (options) => {
    const { jsonOutput, outputPath } = resolveOutputOptions(options);
    const { commerceDb } = program.opts();
    const identity = getIdentityByWallet(commerceDb, options.wallet);

    if (jsonOutput) {
      writeJsonOutput(outputPath, { identity });
      return;
    }

    if (!identity) {
      console.log('Identity not found.');
      return;
    }

    printSection('Identity');
    console.log(`   Registry: ${identity.agent_registry}`);
    console.log(`   Agent ID: ${identity.agent_id}`);
    console.log(`   URI: ${identity.agent_uri}`);
    console.log(`   Wallet: ${identity.agent_wallet}`);
  });

identity
  .command('list')
  .description('List ERC-8004 identities')
  .option('--registry <uri>', 'Agent registry URI')
  .option('--agent-id <id>', 'ERC-8004 agent ID')
  .option('--wallet <address>', 'Wallet address')
  .option('--active', 'Only active identities', false)
  .option('--limit <n>', 'Max results', '50')
  .action(async (options) => {
    const { jsonOutput, outputPath } = resolveOutputOptions(options);
    const { commerceDb } = program.opts();
    const identities = listIdentities(commerceDb, {
      agentRegistry: options.registry || null,
      agentId: options.agentId || null,
      agentWallet: options.wallet || null,
      active: options.active ? true : null,
      limit: Number(options.limit)
    });

    if (jsonOutput) {
      writeJsonOutput(outputPath, { identities });
      return;
    }

    printSection('Identities');
    if (identities.length === 0) {
      console.log('   No identities found.');
      return;
    }

    for (const identity of identities) {
      console.log(`   ${identity.agent_registry}:${identity.agent_id} -> ${identity.agent_wallet || 'no wallet'}`);
    }
  });

program.parseAsync().catch((err) => {
  console.error(chalk.red(`Error: ${err.message}`));
  process.exit(1);
});

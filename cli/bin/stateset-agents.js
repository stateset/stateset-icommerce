#!/usr/bin/env node

/**
 * StateSet Agents CLI
 *
 * Create, manage, and orchestrate autonomous AI agent runtimes.
 *
 * Usage:
 *   stateset-agents create <name> [options]
 *   stateset-agents list
 *   stateset-agents status <name>
 *   stateset-agents run <name> [options]
 *   stateset-agents stop <name>
 *   stateset-agents discover [options]
 *   stateset-agents demo <scenario>
 */

import { program } from 'commander';
import path from 'node:path';
import fs from 'node:fs';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { installShutdownHandlers } from '../src/graceful-shutdown.js';

installShutdownHandlers('stateset-agents');

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const packageJson = JSON.parse(
  fs.readFileSync(path.join(__dirname, '..', 'package.json'), 'utf-8'),
);

// ── Version fast bail-out ──
if (process.argv.includes('--version') || process.argv.includes('-V')) {
  console.log(packageJson.version);
  process.exit(0);
}

const DEFAULT_DB_PATH = './.stateset/a2a.db';
const STRATEGY_NAMES = [
  'always-accept',
  'budget-gated',
  'negotiator',
  'best-of-n',
  'reputation-aware',
];

// Session-scoped runtimes (mirrors tools/agent-runtime.js approach)
const runtimes = new Map();

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeWallet() {
  return '0x' + crypto.randomBytes(20).toString('hex');
}

function makeSigningKey() {
  return {
    privateKey: crypto.randomBytes(32).toString('hex'),
    publicKey: crypto.randomBytes(32).toString('hex'),
  };
}

function normalizeOptions(options = {}) {
  const resolved = options && typeof options.opts === 'function' ? options.opts() : options || {};
  const argv = Array.isArray(options?.rawArgs) ? options.rawArgs : process.argv;
  if (argv.includes('--json')) resolved.json = true;
  return resolved;
}

function createOutputHelpers(options = {}) {
  const resolved = normalizeOptions(options);
  const argv = Array.isArray(options?.rawArgs) ? options.rawArgs : process.argv;
  const jsonOutput = Boolean(resolved.json || argv.includes('--json'));
  const writeJson = async (data) => {
    console.log(JSON.stringify(data, null, 2));
  };
  return { jsonOutput, writeJson, options: resolved };
}

async function loadStore(dbPath) {
  const { A2AStore } = await import('../src/a2a/store.js');
  const store = new A2AStore({ dbPath });
  store.init();
  return store;
}

async function loadCommerceProxy(store) {
  const { makeCommerceProxy } = await import('../src/a2a/agent-runtime.js');
  return makeCommerceProxy(store);
}

async function resolveStrategy(name, opts = {}) {
  const mod = await import('../src/a2a/strategies.js');
  switch (name) {
    case 'always-accept':
      return mod.createAlwaysAcceptStrategy(opts);
    case 'budget-gated':
      return mod.createBudgetGatedStrategy(opts);
    case 'negotiator':
      return mod.createNegotiatorStrategy(opts);
    case 'best-of-n':
      return mod.createBestOfNStrategy(opts);
    case 'reputation-aware':
      return mod.createReputationAwareStrategy
        ? mod.createReputationAwareStrategy(opts)
        : mod.createBudgetGatedStrategy(opts);
    default:
      return mod.createAlwaysAcceptStrategy();
  }
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

program
  .name('stateset-agents')
  .description('StateSet Agents — Create and orchestrate autonomous AI agent runtimes')
  .version(packageJson.version);

// ============================================================================
// Create Command
// ============================================================================

program
  .command('create')
  .description('Create an autonomous agent runtime')
  .argument('<name>', 'Agent name (e.g., "DataForge AI")')
  .option('-d, --db <path>', 'A2A database path', DEFAULT_DB_PATH)
  .option(
    '-s, --strategy <name>',
    `Negotiation strategy (${STRATEGY_NAMES.join(', ')})`,
    'budget-gated',
  )
  .option('--budget-daily <amount>', 'Maximum daily spend in USDC', parseFloat)
  .option('--budget-monthly <amount>', 'Maximum monthly spend in USDC', parseFloat)
  .option('--budget-per-tx <amount>', 'Maximum per-transaction spend', parseFloat)
  .option('--starting-balance <amount>', 'Starting balance in USDC', parseFloat)
  .option('--auto-register-card', 'Auto-register agent card in marketplace')
  .option('--description <text>', 'Agent description')
  .option('--json', 'JSON output')
  .action(async (name, options) => {
    const { jsonOutput, writeJson } = createOutputHelpers(options);
    try {
      const store = await loadStore(options.db);
      const commerce = await loadCommerceProxy(store);
      const strategy = await resolveStrategy(options.strategy);
      const { createAgentRuntime } = await import('../src/a2a/agent-runtime.js');

      const runtime = createAgentRuntime({
        name,
        walletAddress: makeWallet(),
        signingKey: makeSigningKey(),
        commerce,
        strategy,
        budget: {
          daily: options.budgetDaily || Infinity,
          monthly: options.budgetMonthly || Infinity,
          perTransaction: options.budgetPerTx || Infinity,
          startingBalance: options.startingBalance || null,
        },
        autoRegisterCard: !!options.autoRegisterCard,
        agentDescription: options.description || '',
        logger: jsonOutput ? () => {} : console.debug,
      });

      runtimes.set(name, runtime);

      const info = {
        name: runtime.name,
        agentId: runtime.agentId,
        walletAddress: runtime.walletAddress,
        strategy: options.strategy,
        budget: runtime.getBudget(),
        card: runtime.getAgentCard(),
      };

      if (jsonOutput) {
        await writeJson({ success: true, agent: info });
      } else {
        console.log(`\nAgent created: ${name}`);
        console.log(`  ID:     ${info.agentId}`);
        console.log(`  Wallet: ${info.walletAddress}`);
        console.log(`  Strategy: ${options.strategy}`);
        console.log(
          `  Budget (daily): $${info.budget.daily === Infinity ? 'unlimited' : info.budget.daily}`,
        );
        if (info.card) {
          console.log(`  Card:   registered (trust: ${info.card.trust_level})`);
        }
        console.log('');
      }

      store.close();
    } catch (error) {
      if (jsonOutput) {
        await writeJson({ success: false, error: error.message });
      } else {
        console.error(`Error: ${error.message}`);
      }
      process.exit(1);
    }
  });

// ============================================================================
// List Command
// ============================================================================

program
  .command('list')
  .description('List active agent runtimes')
  .option('--json', 'JSON output')
  .action(async (options) => {
    const { jsonOutput, writeJson } = createOutputHelpers(options);
    const entries = Array.from(runtimes.entries()).map(([_key, rt]) => ({
      name: rt.name,
      agentId: rt.agentId,
      walletAddress: rt.walletAddress,
      running: rt.isRunning(),
      budget: rt.getBudget(),
    }));

    if (jsonOutput) {
      await writeJson({ agents: entries, count: entries.length });
    } else {
      if (entries.length === 0) {
        console.log('\nNo active agent runtimes. Use `stateset-agents create` to create one.\n');
        return;
      }
      console.log(`\nActive Agent Runtimes (${entries.length}):\n`);
      for (const a of entries) {
        const status = a.running ? 'running' : 'idle';
        console.log(`  ${a.name} [${status}]`);
        console.log(`    ID: ${a.agentId}`);
        console.log(`    Wallet: ${a.walletAddress}`);
        console.log(`    Spent today: $${a.budget.spentToday}`);
        console.log('');
      }
    }
  });

// ============================================================================
// Status Command
// ============================================================================

program
  .command('status')
  .description('Show detailed agent status')
  .argument('<name>', 'Agent name or ID')
  .option('--json', 'JSON output')
  .action(async (name, options) => {
    const { jsonOutput, writeJson } = createOutputHelpers(options);
    const rt = runtimes.get(name);
    if (!rt) {
      const msg = `Agent not found: ${name}`;
      if (jsonOutput) {
        await writeJson({ success: false, error: msg });
      } else {
        console.error(msg);
      }
      process.exit(1);
    }

    const status = {
      name: rt.name,
      agentId: rt.agentId,
      walletAddress: rt.walletAddress,
      running: rt.isRunning(),
      budget: rt.getBudget(),
      services: rt.listMyServices(),
      card: rt.getAgentCard(),
    };

    if (jsonOutput) {
      await writeJson({ success: true, status });
    } else {
      console.log(`\nAgent: ${status.name}`);
      console.log(`  ID:      ${status.agentId}`);
      console.log(`  Wallet:  ${status.walletAddress}`);
      console.log(`  Running: ${status.running ? 'Yes' : 'No'}`);
      console.log(`  Budget:`);
      console.log(
        `    Daily:   $${status.budget.spentToday} / $${status.budget.daily === Infinity ? 'unlimited' : status.budget.daily}`,
      );
      console.log(
        `    Monthly: $${status.budget.spentThisMonth} / $${status.budget.monthly === Infinity ? 'unlimited' : status.budget.monthly}`,
      );
      if (status.budget.balance !== null) {
        console.log(`    Balance: $${status.budget.balance}`);
      }
      if (status.services.length > 0) {
        console.log(`  Services (${status.services.length}):`);
        for (const svc of status.services) {
          console.log(`    - ${svc.name} (${svc.category})`);
        }
      }
      if (status.card) {
        console.log(`  Card: registered (trust: ${status.card.trust_level})`);
      }
      console.log('');
    }
  });

// ============================================================================
// Run Command
// ============================================================================

program
  .command('run')
  .description('Start agent service loop')
  .argument('<name>', 'Agent name')
  .option('--interval <ms>', 'Poll interval in milliseconds', parseInt, 5000)
  .option('--wire-events', 'Wire runtime events to A2A event stream')
  .option('-d, --db <path>', 'A2A database path', DEFAULT_DB_PATH)
  .option('--json', 'JSON output')
  .action(async (name, options) => {
    const { jsonOutput, writeJson } = createOutputHelpers(options);
    const rt = runtimes.get(name);
    if (!rt) {
      const msg = `Agent not found: ${name}. Create one first with \`stateset-agents create ${name}\`.`;
      if (jsonOutput) {
        await writeJson({ success: false, error: msg });
      } else {
        console.error(msg);
      }
      process.exit(1);
    }

    // Wire events if requested
    let unwire;
    if (options.wireEvents) {
      try {
        const { wireRuntimeEvents } = await import('../src/a2a/event-wiring.js');
        const store = await loadStore(options.db);
        const { createEventStreamService } = await import('../src/a2a/event-stream.js');
        const eventStream = createEventStreamService(store);
        const result = wireRuntimeEvents(rt, eventStream);
        unwire = result.unwire;
        if (!jsonOutput) {
          console.log(`  Events wired to A2A event stream`);
        }
      } catch (err) {
        if (!jsonOutput) {
          console.warn(`  Warning: Could not wire events: ${err.message}`);
        }
      }
    }

    rt.start();

    if (jsonOutput) {
      await writeJson({ success: true, action: 'started', name, interval: options.interval });
    } else {
      console.log(`\nAgent ${name} service loop started (${options.interval}ms interval)`);
      console.log('Press Ctrl+C to stop\n');
    }

    // Keep alive
    const shutdown = () => {
      rt.stop();
      if (unwire) unwire();
      if (!jsonOutput) {
        console.log(`\nAgent ${name} stopped.`);
      }
      process.exit(0);
    };
    process.on('SIGINT', shutdown);
    process.on('SIGTERM', shutdown);

    await new Promise(() => {});
  });

// ============================================================================
// Stop Command
// ============================================================================

program
  .command('stop')
  .description('Stop agent service loop')
  .argument('<name>', 'Agent name')
  .option('--json', 'JSON output')
  .action(async (name, options) => {
    const { jsonOutput, writeJson } = createOutputHelpers(options);
    const rt = runtimes.get(name);
    if (!rt) {
      const msg = `Agent not found: ${name}`;
      if (jsonOutput) {
        await writeJson({ success: false, error: msg });
      } else {
        console.error(msg);
      }
      process.exit(1);
    }

    rt.stop();

    if (jsonOutput) {
      await writeJson({ success: true, action: 'stopped', name });
    } else {
      console.log(`Agent ${name} stopped.`);
    }
  });

// ============================================================================
// Discover Command
// ============================================================================

program
  .command('discover')
  .description('Discover agents and services in the A2A marketplace')
  .option('-d, --db <path>', 'A2A database path', DEFAULT_DB_PATH)
  .option('--category <cat>', 'Filter by service category')
  .option('--network <net>', 'Filter by supported network')
  .option('--asset <asset>', 'Filter by supported asset')
  .option('--skill <skill>', 'Filter by A2A skill')
  .option('--json', 'JSON output')
  .action(async (options) => {
    const { jsonOutput, writeJson } = createOutputHelpers(options);
    try {
      const store = await loadStore(options.db);
      const commerce = await loadCommerceProxy(store);

      const _agents = commerce.x402().listAgents({ active: true }) || [];
      const services = commerce.a2a().listServices({ active: 1 }) || [];

      // Apply discovery filters on agents
      const filteredAgents =
        commerce.x402().discoverAgents({
          network: options.network,
          asset: options.asset,
          skill: options.skill,
          category: options.category,
        }) || [];

      if (jsonOutput) {
        await writeJson({
          agents: filteredAgents,
          services,
          agentCount: filteredAgents.length,
          serviceCount: services.length,
        });
      } else {
        console.log(`\nA2A Marketplace Discovery\n`);

        if (filteredAgents.length > 0) {
          console.log(`Agents (${filteredAgents.length}):`);
          for (const a of filteredAgents) {
            const trust = a.trust_level || 'sandbox';
            console.log(`  ${a.name} [${trust}]`);
            console.log(`    Wallet: ${a.wallet_address}`);
            console.log(`    Skills: ${a.a2a_skills || 'N/A'}`);
            console.log('');
          }
        } else {
          console.log('No agents found matching filters.\n');
        }

        if (services.length > 0) {
          console.log(`Services (${services.length}):`);
          for (const s of services) {
            console.log(`  ${s.name} (${s.category})`);
            console.log(`    Provider: ${s.agent_address}`);
            console.log(`    Pricing: ${s.pricing_model}`);
            console.log('');
          }
        } else {
          console.log('No services registered.\n');
        }
      }

      store.close();
    } catch (error) {
      if (jsonOutput) {
        await writeJson({ success: false, error: error.message });
      } else {
        console.error(`Error: ${error.message}`);
      }
      process.exit(1);
    }
  });

// ============================================================================
// Demo Command
// ============================================================================

program
  .command('demo')
  .description('Run a built-in demo scenario')
  .argument('<scenario>', 'Scenario name: basic-negotiation, marketplace, escrow-deal')
  .option('-d, --db <path>', 'A2A database path (default: in-memory)', ':memory:')
  .option('--live', 'Enable real on-chain settlement')
  .option('--chain <chainId>', 'Target blockchain for settlement (base, solana, set_chain)', 'base')
  .option('--simulate', 'Simulate settlement transactions without broadcasting')
  .option('--config-dir <path>', 'Wallet/key config directory', '.stateset')
  .option('--json', 'JSON output')
  .action(async (scenario, options) => {
    const { jsonOutput, writeJson } = createOutputHelpers(options);
    try {
      const { runDemoScenario, DEMO_SCENARIOS } = await import('../src/a2a/demo-scenarios.js');

      if (!DEMO_SCENARIOS.includes(scenario)) {
        const msg = `Unknown scenario: ${scenario}. Available: ${DEMO_SCENARIOS.join(', ')}`;
        if (jsonOutput) {
          await writeJson({ success: false, error: msg });
        } else {
          console.error(msg);
        }
        process.exit(1);
      }

      const store = await loadStore(options.db);
      const commerce = await loadCommerceProxy(store);

      const log = jsonOutput ? () => {} : console.log;

      if (!jsonOutput) {
        console.log(`\n══════════════════════════════════════════════════════════════`);
        console.log(`  StateSet Agents Demo: ${scenario}`);
        console.log(`══════════════════════════════════════════════════════════════\n`);
      }

      // Build settlement config if --live or --chain flags passed
      let settlement = null;
      if (options.live || options.simulate) {
        settlement = {
          chainId: options.chain || 'base',
          simulate: options.simulate || !options.live,
          configDir: options.configDir || '.stateset',
        };
        if (!jsonOutput) {
          log(
            `[settlement] Chain: ${settlement.chainId}, Mode: ${settlement.simulate ? 'simulate' : 'LIVE'}`,
          );
        }
      }

      const result = await runDemoScenario(scenario, commerce, { log, settlement });

      if (jsonOutput) {
        await writeJson({ success: true, scenario, result });
      } else {
        console.log(`\nDemo complete: ${scenario}`);
        console.log(JSON.stringify(result, null, 2));
        console.log('');
      }

      store.close();
    } catch (error) {
      if (jsonOutput) {
        await writeJson({ success: false, error: error.message });
      } else {
        console.error(`Error: ${error.message}`);
      }
      process.exit(1);
    }
  });

program.parse();

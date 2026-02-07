#!/usr/bin/env node

/**
 * StateSet Autonomous Business Engine CLI
 *
 * Starts the full autonomous commerce platform:
 * - Scheduled jobs (cron, intervals)
 * - State machine workflows
 * - Declarative policies
 * - Webhook event handling
 * - Approval escalation
 *
 * Usage:
 *   stateset-autonomous start [options]
 *   stateset-autonomous status
 *   stateset-autonomous init
 *   stateset-autonomous jobs list
 *   stateset-autonomous jobs enable <id>
 *   stateset-autonomous jobs disable <id>
 *   stateset-autonomous jobs run <id>
 */

import { program } from 'commander';
import path from 'path';
import fs from 'fs';
import { fileURLToPath } from 'url';

import pkg from '@stateset/embedded';
const { Commerce } = pkg;
import { AutonomousEngine } from '../src/autonomous/engine.js';
import { runAgentLoop } from '../src/claude-harness.js';
import { getNotifier } from '../src/channels/notifier.js';
import { EventBridge } from '../src/channels/event-bridge.js';
import { installShutdownHandlers } from '../src/graceful-shutdown.js';
installShutdownHandlers('stateset-autonomous');

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Package info
const packageJson = JSON.parse(
  fs.readFileSync(path.join(__dirname, '..', 'package.json'), 'utf-8'),
);

const DEFAULT_DB_PATH = './.stateset/commerce.db';
const DEFAULT_STORE_PATH = './.stateset/autonomous';
const JOB_STATUSES = ['pending', 'running', 'completed', 'failed', 'paused', 'cancelled'];

function normalizeOptions(options = {}) {
  const resolved = options && typeof options.opts === 'function' ? options.opts() : options || {};

  const argv = Array.isArray(options?.rawArgs) ? options.rawArgs : process.argv;

  const getFlagValue = (flags) => {
    for (const flag of flags) {
      const eqMatch = argv.find((arg) => arg.startsWith(`${flag}=`));
      if (eqMatch) {
        return eqMatch.slice(flag.length + 1);
      }
      const idx = argv.indexOf(flag);
      if (idx !== -1 && argv[idx + 1] && !argv[idx + 1].startsWith('-')) {
        return argv[idx + 1];
      }
    }
    return null;
  };

  const db = getFlagValue(['--db', '-d']);
  const store = getFlagValue(['--store', '-s']);
  const output = getFlagValue(['--output']);

  if (db) resolved.db = db;
  if (store) resolved.store = store;
  if (output) resolved.output = output;
  if (argv.includes('--json')) resolved.json = true;

  return resolved;
}

function createOutputHelpers(options = {}) {
  const resolved = normalizeOptions(options);
  const argv = Array.isArray(options?.rawArgs) ? options.rawArgs : process.argv;
  const hasJsonFlag = argv.includes('--json');
  const hasOutputFlag = argv.includes('--output');
  const outputPath = resolved.output || null;
  const jsonOutput = Boolean(resolved.json || outputPath || hasJsonFlag || hasOutputFlag);
  const writeJson = async (data) => {
    const payload = JSON.stringify(data, null, 2);
    if (outputPath) {
      await fs.promises.writeFile(outputPath, payload);
      return;
    }
    console.log(payload);
  };

  return { jsonOutput, writeJson, options: resolved };
}

function serializeJob(job) {
  if (!job) return job;
  if (typeof job.toJSON === 'function') return job.toJSON();
  return { ...job };
}

function collectInitData(engine) {
  const jobs = engine.scheduler ? engine.scheduler.listJobs().map(serializeJob) : [];
  const workflows = engine.workflows ? engine.workflows.listWorkflows() : [];
  const policies = engine.policies ? engine.policies.listPolicySets() : [];
  const approvals = engine.approvals ? engine.approvals.getStatus() : null;

  return {
    jobs,
    workflows,
    policies,
    approvals,
    counts: {
      jobs: jobs.length,
      workflows: workflows.length,
      policies: policies.length,
      approvalChains: approvals?.chainCount ?? 0,
    },
  };
}

function hasExistingData(summary) {
  if (!summary) return false;
  return (
    summary.counts.jobs > 0 ||
    summary.counts.workflows > 0 ||
    summary.counts.policies > 0 ||
    summary.counts.approvalChains > 0
  );
}

async function withEngine(options, handler) {
  const commerce = new Commerce(options.db);

  const engine = new AutonomousEngine({
    storePath: options.store,
    commerce,
  });

  await engine.load();
  return handler(engine);
}

async function handleJobsList(options, jsonOutput, writeJson) {
  return withEngine(options, async (engine) => {
    if (options.enabled && options.disabled) {
      const message = 'Only one of --enabled or --disabled may be specified.';
      if (jsonOutput) {
        await writeJson({ error: message });
      } else {
        console.error(`Error: ${message}`);
      }
      process.exit(1);
    }

    const status = options.status ? String(options.status).toLowerCase() : null;
    if (status && !JOB_STATUSES.includes(status)) {
      const message = `Invalid status: ${options.status}. Expected one of ${JOB_STATUSES.join(', ')}.`;
      if (jsonOutput) {
        await writeJson({ error: message });
      } else {
        console.error(`Error: ${message}`);
      }
      process.exit(1);
    }

    const enabled = options.enabled ? true : options.disabled ? false : null;

    const jobs =
      engine.scheduler?.listJobs({
        status: status || null,
        enabled,
      }) || [];

    if (jsonOutput) {
      await writeJson({
        ok: true,
        timestamp: new Date().toISOString(),
        storePath: options.store,
        dbPath: options.db,
        filters: {
          status: status || null,
          enabled,
        },
        jobs: jobs.map(serializeJob),
        total: jobs.length,
      });
      return;
    }

    console.log('\n📅 Scheduled Jobs\n');

    if (jobs.length === 0) {
      console.log('   No jobs configured');
      return;
    }

    for (const job of jobs) {
      const status = job.enabled ? '✅' : '⏸️';
      console.log(`${status} ${job.name}`);
      console.log(`   ID: ${job.id}`);
      console.log(`   Schedule: ${job.type} - ${job.schedule}`);
      console.log(`   Next Run: ${job.nextRunAt || 'N/A'}`);
      console.log(`   Runs: ${job.runCount} (${job.failCount} failed)`);
      console.log('');
    }
  });
}

async function handleJobsEnable(jobId, options, jsonOutput, writeJson) {
  return withEngine(options, async (engine) => {
    const job = engine.scheduler?.resumeJob(jobId);
    if (!job) {
      const message = `Job not found: ${jobId}`;
      if (jsonOutput) {
        await writeJson({ error: message });
      } else {
        console.error(`Error: ${message}`);
      }
      process.exit(1);
    }

    if (jsonOutput) {
      await writeJson({ success: true, action: 'enable', job: serializeJob(job) });
    } else {
      console.log(`✅ Job enabled: ${jobId}`);
    }
  });
}

async function handleJobsDisable(jobId, options, jsonOutput, writeJson) {
  return withEngine(options, async (engine) => {
    const job = engine.scheduler?.pauseJob(jobId);
    if (!job) {
      const message = `Job not found: ${jobId}`;
      if (jsonOutput) {
        await writeJson({ error: message });
      } else {
        console.error(`Error: ${message}`);
      }
      process.exit(1);
    }

    if (jsonOutput) {
      await writeJson({ success: true, action: 'disable', job: serializeJob(job) });
    } else {
      console.log(`✅ Job disabled: ${jobId}`);
    }
  });
}

async function handleJobsRun(jobId, options, jsonOutput, writeJson) {
  return withEngine(options, async (engine) => {
    if (!jsonOutput) {
      console.log(`⏰ Running job: ${jobId}`);
    }
    const result = await engine.scheduler.runNow(jobId);
    if (jsonOutput) {
      await writeJson({ success: result.status === 'completed', action: 'run', jobId, result });
    } else {
      console.log(`   Status: ${result.status}`);
      console.log(`   Duration: ${result.duration}ms`);
    }
  });
}

program
  .name('stateset-autonomous')
  .description('StateSet Autonomous Business Engine - AI agents running your commerce operations')
  .version(packageJson.version);

// ============================================================================
// Start Command
// ============================================================================

program
  .command('start')
  .description('Start the autonomous business engine')
  .option('-d, --db <path>', 'Path to SQLite database', DEFAULT_DB_PATH)
  .option('-s, --store <path>', 'Path to store autonomous engine data', DEFAULT_STORE_PATH)
  .option('-p, --port <port>', 'Webhook server port', '3000')
  .option('--no-webhooks', 'Disable webhook server')
  .option('--no-scheduler', 'Disable job scheduler')
  .option('--no-workflows', 'Disable workflow engine')
  .option('--no-policies', 'Disable policy engine')
  .option('--no-approvals', 'Disable approval queue')
  .option('--init-defaults', 'Initialize with default templates')
  .option('--notify-config <path>', 'Path to JSON notification routing config')
  .option('-v, --verbose', 'Verbose output')
  .action(async (options) => {
    console.log('');
    console.log('╔══════════════════════════════════════════════════════════════╗');
    console.log('║     StateSet Autonomous Business Engine                      ║');
    console.log('║     AI-Powered Commerce Operations                           ║');
    console.log('╚══════════════════════════════════════════════════════════════╝');
    console.log('');

    try {
      // Initialize commerce instance
      console.log(`📦 Initializing commerce engine...`);
      const commerce = new Commerce(options.db);
      console.log(`   Database: ${options.db}`);

      // Create agent executor
      const agentExecutor = async (agent, request, _context) => {
        if (options.verbose) {
          console.log(`\n🤖 Agent Request: [${agent}] ${request}`);
        }

        const result = await runAgentLoop({
          commerce,
          request,
          agent,
          allowApply: true, // Autonomous mode has write access
          model: 'claude-sonnet-4-20250514',
          maxTurns: 10,
        });

        if (options.verbose) {
          console.log(`   Result: ${result.finalResponse?.substring(0, 100)}...`);
        }

        return result;
      };

      // Create autonomous engine
      console.log(`\n🚀 Starting autonomous engine...`);
      const engine = new AutonomousEngine({
        storePath: options.store,
        commerce,
        agentExecutor,
        webhookPort: parseInt(options.port, 10),
        enableWebhooks: options.webhooks,
        enableScheduler: options.scheduler,
        enableWorkflows: options.workflows,
        enablePolicies: options.policies,
        enableApprovals: options.approvals,
      });

      // Wire up channel notifier
      const notifier = getNotifier();
      if (options.notifyConfig) {
        try {
          const configRaw = fs.readFileSync(options.notifyConfig, 'utf-8');
          const config = JSON.parse(configRaw);
          if (config.routes) {
            notifier.loadRoutes(config.routes);
            console.log(`   Notification routes loaded from ${options.notifyConfig}`);
          }
        } catch (err) {
          console.error(`   Warning: Failed to load notify config: ${err.message}`);
        }
      }
      engine.setNotifier(notifier);

      // Wire event bridge to forward engine events to channel notifications
      const eventBridge = new EventBridge({
        engine,
        notifier,
        verbose: options.verbose,
      });
      eventBridge.start();

      // Set up event logging
      if (options.verbose) {
        engine.on('scheduler:job:started', ({ job }) => {
          console.log(`\n⏰ Job Started: ${job.name}`);
        });

        engine.on('scheduler:job:completed', ({ job, result }) => {
          console.log(`✅ Job Completed: ${job.name} (${result.duration}ms)`);
        });

        engine.on('scheduler:job:failed', ({ job, result }) => {
          console.log(`❌ Job Failed: ${job.name} - ${result.error}`);
        });

        engine.on('workflows:instance:started', ({ instance }) => {
          console.log(`\n🔄 Workflow Started: ${instance.workflowName} (${instance.id})`);
        });

        engine.on('workflows:instance:transitioned', ({ instanceId, from, to }) => {
          console.log(`   [${instanceId}] State: ${from} → ${to}`);
        });

        engine.on('workflows:instance:completed', ({ instance }) => {
          console.log(`✅ Workflow Completed: ${instance.workflowName}`);
        });

        engine.on('webhooks:event:received', ({ event }) => {
          console.log(`\n📥 Webhook Received: ${event.sourceName} - ${event.eventType}`);
        });

        engine.on('approvals:request:created', ({ request }) => {
          console.log(`\n📋 Approval Required: ${request.title} ($${request.amount || 'N/A'})`);
        });

        engine.on('approvals:request:approved', ({ request }) => {
          console.log(`✅ Approved: ${request.title}`);
        });

        engine.on('agent:executing', ({ agent, request }) => {
          console.log(`\n🤖 Executing: [${agent}] ${request.substring(0, 80)}...`);
        });
      }

      // Initialize defaults if requested
      if (options.initDefaults) {
        console.log(`   Initializing default templates...`);
        await engine.initializeDefaults();
      }

      // Start the engine
      await engine.start();

      // Print status
      const status = engine.getStatus();
      console.log(`\n📊 Engine Status:`);
      console.log(
        `   Scheduler: ${status.features.scheduler ? `✅ Running (${status.scheduler?.totalJobs || 0} jobs)` : '❌ Disabled'}`,
      );
      console.log(
        `   Workflows: ${status.features.workflows ? `✅ Ready (${status.workflows?.totalWorkflows || 0} definitions)` : '❌ Disabled'}`,
      );
      console.log(
        `   Policies:  ${status.features.policies ? `✅ Loaded (${status.policies?.totalPolicySets || 0} policy sets)` : '❌ Disabled'}`,
      );
      console.log(
        `   Webhooks:  ${status.features.webhooks ? `✅ Listening on port ${options.port}` : '❌ Disabled'}`,
      );
      console.log(
        `   Approvals: ${status.features.approvals ? `✅ Ready (${status.approvals?.pendingCount || 0} pending)` : '❌ Disabled'}`,
      );

      console.log(`\n✨ Autonomous engine is running!`);
      console.log(`   Press Ctrl+C to stop\n`);

      // Handle shutdown
      const shutdown = async () => {
        console.log('\n\n🛑 Shutting down...');
        await engine.stop();
        console.log('   Engine stopped');
        process.exit(0);
      };

      process.on('SIGINT', shutdown);
      process.on('SIGTERM', shutdown);

      // Keep process alive
      await new Promise(() => {});
    } catch (error) {
      console.error(`\n❌ Error: ${error.message}`);
      if (options.verbose) {
        console.error(error.stack);
      }
      process.exit(1);
    }
  });

// ============================================================================
// Status Command
// ============================================================================

program
  .command('status')
  .description('Show status of the autonomous engine')
  .option('-d, --db <path>', 'Path to SQLite database', DEFAULT_DB_PATH)
  .option('-s, --store <path>', 'Path to autonomous engine data', DEFAULT_STORE_PATH)
  .option('--json', 'Output status as JSON')
  .option('--output <file>', 'Write JSON output to file (implies --json)')
  .action(async (options) => {
    const { jsonOutput, writeJson, options: opts } = createOutputHelpers(options);
    try {
      const commerce = new Commerce(opts.db);

      const engine = new AutonomousEngine({
        storePath: opts.store,
        commerce,
      });

      await engine.load();
      const status = engine.getStatus();

      if (jsonOutput) {
        await writeJson({
          ok: true,
          timestamp: new Date().toISOString(),
          storePath: opts.store,
          dbPath: opts.db,
          status,
        });
        return;
      }

      const features = status.features || {};
      const scheduler = status.scheduler || {};
      const workflows = status.workflows || {};
      const policies = status.policies || {};
      const webhooks = status.webhooks || {};
      const approvals = status.approvals || {};
      const heartbeat = status.heartbeat || {};

      console.log('\n📊 Autonomous Engine Status\n');
      console.log(`Store: ${opts.store}`);
      console.log(`Database: ${opts.db}`);
      console.log(`Running: ${status.isRunning ? '✅ Yes' : '⏸️ No'}`);
      console.log('');

      console.log(
        `Scheduler: ${features.scheduler ? `✅ ${scheduler.enabledJobs || 0}/${scheduler.totalJobs || 0} jobs enabled` : '❌ Disabled'}`,
      );
      console.log(
        `Workflows: ${features.workflows ? `✅ ${workflows.totalWorkflows || 0} workflows (${workflows.totalInstances || 0} instances)` : '❌ Disabled'}`,
      );
      console.log(
        `Policies:  ${features.policies ? `✅ ${policies.totalPolicySets || 0} policy sets (${policies.totalRules || 0} rules)` : '❌ Disabled'}`,
      );
      console.log(
        `Webhooks:  ${features.webhooks ? `✅ ${webhooks.isRunning ? 'Listening' : 'Configured'} on ${webhooks.host || '0.0.0.0'}:${webhooks.port || 'N/A'}` : '❌ Disabled'}`,
      );
      console.log(
        `Approvals: ${features.approvals ? `✅ ${approvals.chainCount || 0} chains (${approvals.pendingCount || 0} pending)` : '❌ Disabled'}`,
      );

      if (features.heartbeat) {
        console.log(
          `Heartbeat: ✅ ${heartbeat.running ? 'Running' : 'Configured'} (${heartbeat.checkCount || 0} checks)`,
        );
      }
    } catch (error) {
      if (jsonOutput) {
        await writeJson({ error: error.message });
      } else {
        console.error(`Error: ${error.message}`);
      }
      process.exit(1);
    }
  });

// ============================================================================
// Init Command
// ============================================================================

program
  .command('init')
  .description('Initialize autonomous engine with default templates')
  .option('-d, --db <path>', 'Path to SQLite database', DEFAULT_DB_PATH)
  .option('-s, --store <path>', 'Path to store autonomous engine data', DEFAULT_STORE_PATH)
  .option('--force', 'Overwrite existing autonomous data')
  .option('--json', 'Output status as JSON')
  .option('--output <file>', 'Write JSON output to file (implies --json)')
  .action(async (options) => {
    const { jsonOutput, writeJson, options: opts } = createOutputHelpers(options);
    try {
      if (!jsonOutput) {
        console.log('\n🔧 Initializing Autonomous Business Engine...\n');
      }

      if (opts.force && fs.existsSync(opts.store)) {
        fs.rmSync(opts.store, { recursive: true, force: true });
      }

      const commerce = new Commerce(opts.db);

      const engine = new AutonomousEngine({
        storePath: opts.store,
        commerce,
        enableWebhooks: false, // Don't start server during init
      });

      await engine.load();

      if (!opts.force) {
        const existing = collectInitData(engine);
        if (hasExistingData(existing)) {
          const message = `Autonomous data already exists at ${opts.store}. Use --force to overwrite.`;
          if (jsonOutput) {
            await writeJson({ error: message, existing: existing.counts });
          } else {
            console.error(`❌ ${message}`);
          }
          process.exit(1);
        }
      }

      await engine.initializeDefaults();
      await engine.save();

      const initData = collectInitData(engine);

      if (jsonOutput) {
        await writeJson({
          success: true,
          timestamp: new Date().toISOString(),
          storePath: opts.store,
          dbPath: opts.db,
          ...initData,
        });
        return;
      }

      console.log('✅ Default templates initialized:\n');

      // List what was created
      if (engine.scheduler) {
        const jobs = engine.scheduler.listJobs();
        console.log(`📅 Scheduled Jobs (${jobs.length}):`);
        for (const job of jobs) {
          console.log(`   - ${job.name} (${job.type}: ${job.schedule}) [disabled]`);
        }
      }

      if (engine.workflows) {
        const workflows = engine.workflows.listWorkflows();
        console.log(`\n🔄 Workflows (${workflows.length}):`);
        for (const wf of workflows) {
          console.log(`   - ${wf.name} (${wf.states.length} states)`);
        }
      }

      if (engine.policies) {
        const policies = engine.policies.listPolicySets();
        console.log(`\n📜 Policies (${policies.length}):`);
        for (const p of policies) {
          console.log(`   - ${p.name} [${p.domain}] (${p.rules.length} rules)`);
        }
      }

      if (engine.approvals) {
        const status = engine.approvals.getStatus();
        console.log(`\n📋 Approval Chains (${status.chainCount}):`);
        console.log(`   - Order approval (4 tiers)`);
        console.log(`   - Return approval (3 tiers)`);
        console.log(`   - Purchase order approval (4 tiers)`);
        console.log(`   - Refund approval (3 tiers)`);
      }

      console.log('\n✨ Initialization complete!');
      console.log(`\nRun 'stateset-autonomous start' to begin autonomous operations.\n`);
    } catch (error) {
      if (jsonOutput) {
        await writeJson({ error: error.message });
      } else {
        console.error(`Error: ${error.message}`);
      }
      process.exit(1);
    }
  });

// ============================================================================
// Jobs Commands
// ============================================================================

const jobsCommand = program
  .command('jobs')
  .description('Manage scheduled jobs')
  .option('-d, --db <path>', 'Path to SQLite database', DEFAULT_DB_PATH)
  .option('-s, --store <path>', 'Path to autonomous engine data', DEFAULT_STORE_PATH)
  .option('--enable <id>', 'Enable a job')
  .option('--disable <id>', 'Disable a job')
  .option('--run <id>', 'Run a job immediately')
  .option('--status <status>', 'Filter jobs by status')
  .option('--enabled', 'Only enabled jobs')
  .option('--disabled', 'Only disabled jobs')
  .option('--json', 'Output status as JSON')
  .option('--output <file>', 'Write JSON output to file (implies --json)')
  .action(async (options) => {
    const { jsonOutput, writeJson, options: opts } = createOutputHelpers(options);
    try {
      const actionFlags = ['enable', 'disable', 'run'].filter((flag) => Boolean(opts[flag]));
      if (actionFlags.length > 1) {
        const message = 'Only one of --enable, --disable, or --run may be specified.';
        if (jsonOutput) {
          await writeJson({ error: message });
        } else {
          console.error(`Error: ${message}`);
        }
        process.exit(1);
      }

      if (opts.enable) {
        await handleJobsEnable(opts.enable, opts, jsonOutput, writeJson);
        return;
      }

      if (opts.disable) {
        await handleJobsDisable(opts.disable, opts, jsonOutput, writeJson);
        return;
      }

      if (opts.run) {
        await handleJobsRun(opts.run, opts, jsonOutput, writeJson);
        return;
      }

      await handleJobsList(opts, jsonOutput, writeJson);
    } catch (error) {
      if (jsonOutput) {
        await writeJson({ error: error.message });
      } else {
        console.error(`Error: ${error.message}`);
      }
      process.exit(1);
    }
  });

const applyJobsOptions = (command) =>
  command
    .option('-d, --db <path>', 'Path to SQLite database', DEFAULT_DB_PATH)
    .option('-s, --store <path>', 'Path to autonomous engine data', DEFAULT_STORE_PATH)
    .option('--json', 'Output status as JSON')
    .option('--output <file>', 'Write JSON output to file (implies --json)');

const applyJobsListOptions = (command) =>
  applyJobsOptions(command)
    .option('--status <status>', 'Filter jobs by status')
    .option('--enabled', 'Only enabled jobs')
    .option('--disabled', 'Only disabled jobs');

applyJobsListOptions(jobsCommand.command('list').description('List scheduled jobs')).action(
  async (options) => {
    const { jsonOutput, writeJson, options: opts } = createOutputHelpers(options);
    try {
      await handleJobsList(opts, jsonOutput, writeJson);
    } catch (error) {
      if (jsonOutput) {
        await writeJson({ error: error.message });
      } else {
        console.error(`Error: ${error.message}`);
      }
      process.exit(1);
    }
  },
);

applyJobsOptions(
  jobsCommand.command('enable').description('Enable a scheduled job').argument('<id>', 'Job ID'),
).action(async (jobId, options) => {
  const { jsonOutput, writeJson, options: opts } = createOutputHelpers(options);
  try {
    await handleJobsEnable(jobId, opts, jsonOutput, writeJson);
  } catch (error) {
    if (jsonOutput) {
      await writeJson({ error: error.message });
    } else {
      console.error(`Error: ${error.message}`);
    }
    process.exit(1);
  }
});

applyJobsOptions(
  jobsCommand.command('disable').description('Disable a scheduled job').argument('<id>', 'Job ID'),
).action(async (jobId, options) => {
  const { jsonOutput, writeJson, options: opts } = createOutputHelpers(options);
  try {
    await handleJobsDisable(jobId, opts, jsonOutput, writeJson);
  } catch (error) {
    if (jsonOutput) {
      await writeJson({ error: error.message });
    } else {
      console.error(`Error: ${error.message}`);
    }
    process.exit(1);
  }
});

applyJobsOptions(
  jobsCommand.command('run').description('Run a job immediately').argument('<id>', 'Job ID'),
).action(async (jobId, options) => {
  const { jsonOutput, writeJson, options: opts } = createOutputHelpers(options);
  try {
    await handleJobsRun(jobId, opts, jsonOutput, writeJson);
  } catch (error) {
    if (jsonOutput) {
      await writeJson({ error: error.message });
    } else {
      console.error(`Error: ${error.message}`);
    }
    process.exit(1);
  }
});

program.parse();

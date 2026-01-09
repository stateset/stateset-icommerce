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
 */

import { program } from 'commander';
import path from 'path';
import fs from 'fs';
import { fileURLToPath } from 'url';

import pkg from '@stateset/embedded';
const { Commerce } = pkg;
import { AutonomousEngine } from '../src/autonomous/engine.js';
import { runAgentLoop } from '../src/claude-harness.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Package info
const packageJson = JSON.parse(
  fs.readFileSync(path.join(__dirname, '..', 'package.json'), 'utf-8')
);

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
  .option('-d, --db <path>', 'Path to SQLite database', './.stateset/commerce.db')
  .option('-s, --store <path>', 'Path to store autonomous engine data', './.stateset/autonomous')
  .option('-p, --port <port>', 'Webhook server port', '3000')
  .option('--no-webhooks', 'Disable webhook server')
  .option('--no-scheduler', 'Disable job scheduler')
  .option('--no-workflows', 'Disable workflow engine')
  .option('--no-policies', 'Disable policy engine')
  .option('--no-approvals', 'Disable approval queue')
  .option('--init-defaults', 'Initialize with default templates')
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
      const agentExecutor = async (agent, request, context) => {
        if (options.verbose) {
          console.log(`\n🤖 Agent Request: [${agent}] ${request}`);
        }

        const result = await runAgentLoop({
          commerce,
          request,
          agent,
          allowApply: true, // Autonomous mode has write access
          model: 'claude-sonnet-4-20250514',
          maxTurns: 10
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
        enableApprovals: options.approvals
      });

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
          console.log(`   State: ${from} → ${to}`);
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
      console.log(`   Scheduler: ${status.features.scheduler ? `✅ Running (${status.scheduler?.totalJobs || 0} jobs)` : '❌ Disabled'}`);
      console.log(`   Workflows: ${status.features.workflows ? `✅ Ready (${status.workflows?.totalWorkflows || 0} definitions)` : '❌ Disabled'}`);
      console.log(`   Policies:  ${status.features.policies ? `✅ Loaded (${status.policies?.totalPolicySets || 0} policy sets)` : '❌ Disabled'}`);
      console.log(`   Webhooks:  ${status.features.webhooks ? `✅ Listening on port ${options.port}` : '❌ Disabled'}`);
      console.log(`   Approvals: ${status.features.approvals ? `✅ Ready (${status.approvals?.pendingCount || 0} pending)` : '❌ Disabled'}`);

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
  .option('-s, --store <path>', 'Path to autonomous engine data', './.stateset/autonomous')
  .action(async (options) => {
    try {
      const commerce = new Commerce('./.stateset/commerce.db');
      
      const engine = new AutonomousEngine({
        storePath: options.store,
        commerce
      });

      await engine.load();
      const status = engine.getStatus();

      console.log('\n📊 Autonomous Engine Status\n');
      console.log(JSON.stringify(status, null, 2));

    } catch (error) {
      console.error(`Error: ${error.message}`);
      process.exit(1);
    }
  });

// ============================================================================
// Init Command
// ============================================================================

program
  .command('init')
  .description('Initialize autonomous engine with default templates')
  .option('-d, --db <path>', 'Path to SQLite database', './.stateset/commerce.db')
  .option('-s, --store <path>', 'Path to store autonomous engine data', './.stateset/autonomous')
  .action(async (options) => {
    try {
      console.log('\n🔧 Initializing Autonomous Business Engine...\n');

      const commerce = new Commerce(options.db);
      
      const engine = new AutonomousEngine({
        storePath: options.store,
        commerce,
        enableWebhooks: false // Don't start server during init
      });

      await engine.initializeDefaults();

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
      console.error(`Error: ${error.message}`);
      process.exit(1);
    }
  });

// ============================================================================
// Jobs Commands
// ============================================================================

program
  .command('jobs')
  .description('Manage scheduled jobs')
  .option('-s, --store <path>', 'Path to autonomous engine data', './.stateset/autonomous')
  .option('--enable <id>', 'Enable a job')
  .option('--disable <id>', 'Disable a job')
  .option('--run <id>', 'Run a job immediately')
  .action(async (options) => {
    try {
      const commerce = new Commerce('./.stateset/commerce.db');
      
      const engine = new AutonomousEngine({
        storePath: options.store,
        commerce
      });

      await engine.load();

      if (options.enable) {
        engine.scheduler.resumeJob(options.enable);
        await engine.save();
        console.log(`✅ Job enabled: ${options.enable}`);
        return;
      }

      if (options.disable) {
        engine.scheduler.pauseJob(options.disable);
        await engine.save();
        console.log(`✅ Job disabled: ${options.disable}`);
        return;
      }

      if (options.run) {
        console.log(`⏰ Running job: ${options.run}`);
        const result = await engine.scheduler.runNow(options.run);
        console.log(`   Status: ${result.status}`);
        console.log(`   Duration: ${result.duration}ms`);
        return;
      }

      // List jobs
      const jobs = engine.scheduler.listJobs();
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

    } catch (error) {
      console.error(`Error: ${error.message}`);
      process.exit(1);
    }
  });

program.parse();

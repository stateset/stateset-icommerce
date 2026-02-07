/**
 * Autonomous Engine Commands for StateSet Channel Gateways
 *
 * Registers channel commands that bridge to the AutonomousEngine,
 * allowing users to manage workflows, jobs, and approvals directly
 * from messaging channels.
 *
 * Commands:
 *   /workflow <name> [ctx] - Start a workflow
 *   /job <id>              - Trigger a job
 *   /approve <id>          - Approve a pending request
 *   /reject <id> [reason]  - Reject a pending request
 *   /pending               - List pending approvals
 *   /jobs                  - List scheduled jobs
 *   /workflows             - List available workflows
 */

import { getCommandRegistry } from './command-registry.js';

// ============================================================================
// Command Definitions
// ============================================================================

const AUTONOMOUS_COMMANDS = [
  {
    name: 'workflow',
    description: 'Start a workflow: /workflow <name> [JSON context]',
    acceptsArgs: true,
    handler: createWorkflowHandler,
  },
  {
    name: 'job',
    description: 'Trigger a scheduled job: /job <id>',
    acceptsArgs: true,
    handler: createJobHandler,
  },
  {
    name: 'approve',
    description: 'Approve a pending request: /approve <id>',
    acceptsArgs: true,
    handler: createApproveHandler,
  },
  {
    name: 'reject',
    description: 'Reject a pending request: /reject <id> [reason]',
    acceptsArgs: true,
    handler: createRejectHandler,
  },
  {
    name: 'pending',
    description: 'List pending approval requests',
    acceptsArgs: false,
    handler: createPendingHandler,
  },
  {
    name: 'jobs',
    description: 'List scheduled jobs',
    acceptsArgs: false,
    handler: createJobsHandler,
  },
  {
    name: 'workflows',
    description: 'List available workflows',
    acceptsArgs: false,
    handler: createWorkflowsHandler,
  },
];

// ============================================================================
// Handler Factories
// ============================================================================

function createWorkflowHandler() {
  return async (argText, ctx) => {
    const { autonomousEngine, senderId } = ctx;

    if (!autonomousEngine) {
      return { response: 'Autonomous engine is not configured.' };
    }

    if (!autonomousEngine.workflows) {
      return { response: 'Workflows subsystem is not enabled.' };
    }

    const parts = argText.trim().split(/\s+/);
    const name = parts[0];

    if (!name) {
      return {
        response:
          'Usage: /workflow <name> [JSON context]\nExample: /workflow order-fulfillment {"orderId": "ORD-123"}',
      };
    }

    let context = { triggeredBy: senderId };
    if (parts.length > 1) {
      const contextStr = parts.slice(1).join(' ');
      try {
        context = { ...JSON.parse(contextStr), triggeredBy: senderId };
      } catch {
        return { response: 'Invalid JSON context. Example: {"orderId": "ORD-123"}' };
      }
    }

    try {
      const instance = await autonomousEngine.startWorkflow(name, context);
      return {
        response: `Workflow started: ${name}\nInstance ID: ${instance.id}\nStatus: ${instance.status}`,
      };
    } catch (err) {
      return { response: `Failed to start workflow: ${err.message}` };
    }
  };
}

function createJobHandler() {
  return async (argText, ctx) => {
    const { autonomousEngine } = ctx;

    if (!autonomousEngine) {
      return { response: 'Autonomous engine is not configured.' };
    }

    if (!autonomousEngine.scheduler) {
      return { response: 'Scheduler subsystem is not enabled.' };
    }

    const jobId = argText.trim();
    if (!jobId) {
      return { response: 'Usage: /job <id>' };
    }

    try {
      const result = await autonomousEngine.triggerJob(jobId);
      if (result?.success) {
        return { response: `Job triggered: ${jobId}\nDuration: ${result.duration || 0}ms` };
      }
      return { response: `Job triggered: ${jobId}` };
    } catch (err) {
      return { response: `Failed to trigger job: ${err.message}` };
    }
  };
}

function createApproveHandler() {
  return async (argText, ctx) => {
    const { autonomousEngine, senderId } = ctx;

    if (!autonomousEngine) {
      return { response: 'Autonomous engine is not configured.' };
    }

    if (!autonomousEngine.approvals) {
      return { response: 'Approvals subsystem is not enabled.' };
    }

    const requestId = argText.trim();
    if (!requestId) {
      return { response: 'Usage: /approve <id>' };
    }

    try {
      const result = await autonomousEngine.approvals.approve(requestId, senderId);
      if (result) {
        return { response: `Approved: ${result.title || requestId}` };
      }
      return { response: `Request ${requestId} not found or already processed.` };
    } catch (err) {
      return { response: `Failed to approve: ${err.message}` };
    }
  };
}

function createRejectHandler() {
  return async (argText, ctx) => {
    const { autonomousEngine, senderId } = ctx;

    if (!autonomousEngine) {
      return { response: 'Autonomous engine is not configured.' };
    }

    if (!autonomousEngine.approvals) {
      return { response: 'Approvals subsystem is not enabled.' };
    }

    const parts = argText.trim().split(/\s+/);
    const requestId = parts[0];
    const reason = parts.slice(1).join(' ') || undefined;

    if (!requestId) {
      return { response: 'Usage: /reject <id> [reason]' };
    }

    try {
      const result = await autonomousEngine.approvals.reject(requestId, senderId, { reason });
      if (result) {
        const reasonMsg = reason ? ` (reason: ${reason})` : '';
        return { response: `Rejected: ${result.title || requestId}${reasonMsg}` };
      }
      return { response: `Request ${requestId} not found or already processed.` };
    } catch (err) {
      return { response: `Failed to reject: ${err.message}` };
    }
  };
}

function createPendingHandler() {
  return async (argText, ctx) => {
    const { autonomousEngine } = ctx;

    if (!autonomousEngine) {
      return { response: 'Autonomous engine is not configured.' };
    }

    if (!autonomousEngine.approvals) {
      return { response: 'Approvals subsystem is not enabled.' };
    }

    try {
      const pending = await autonomousEngine.approvals.listPending();

      if (!pending || pending.length === 0) {
        return { response: 'No pending approval requests.' };
      }

      const lines = ['Pending Approvals:', ''];
      for (const req of pending) {
        const amount = req.amount ? ` ($${req.amount})` : '';
        const created = req.createdAt ? new Date(req.createdAt).toLocaleString() : 'unknown';
        lines.push(`${req.id}: ${req.title || 'Untitled'}${amount}`);
        lines.push(`  Created: ${created}`);
        lines.push(`  /approve ${req.id}  |  /reject ${req.id}`);
        lines.push('');
      }

      return { response: lines.join('\n').trim() };
    } catch (err) {
      return { response: `Failed to list pending: ${err.message}` };
    }
  };
}

function createJobsHandler() {
  return async (argText, ctx) => {
    const { autonomousEngine } = ctx;

    if (!autonomousEngine) {
      return { response: 'Autonomous engine is not configured.' };
    }

    if (!autonomousEngine.scheduler) {
      return { response: 'Scheduler subsystem is not enabled.' };
    }

    try {
      const jobs = await autonomousEngine.scheduler.listJobs();

      if (!jobs || jobs.length === 0) {
        return { response: 'No scheduled jobs configured.' };
      }

      const lines = ['Scheduled Jobs:', ''];
      for (const job of jobs) {
        const status = job.enabled ? 'enabled' : 'disabled';
        const schedule = job.cron || job.interval || 'manual';
        const lastRun = job.lastRunAt ? new Date(job.lastRunAt).toLocaleString() : 'never';
        lines.push(`${job.id}: ${job.name || job.id}`);
        lines.push(`  Schedule: ${schedule} (${status})`);
        lines.push(`  Last run: ${lastRun}`);
        lines.push(`  /job ${job.id}`);
        lines.push('');
      }

      return { response: lines.join('\n').trim() };
    } catch (err) {
      return { response: `Failed to list jobs: ${err.message}` };
    }
  };
}

function createWorkflowsHandler() {
  return async (argText, ctx) => {
    const { autonomousEngine } = ctx;

    if (!autonomousEngine) {
      return { response: 'Autonomous engine is not configured.' };
    }

    if (!autonomousEngine.workflows) {
      return { response: 'Workflows subsystem is not enabled.' };
    }

    try {
      const workflows = await autonomousEngine.workflows.listWorkflows();

      if (!workflows || workflows.length === 0) {
        return { response: 'No workflows registered.' };
      }

      const lines = ['Available Workflows:', ''];
      for (const wf of workflows) {
        const states = wf.states ? Object.keys(wf.states).length : 0;
        lines.push(`${wf.id}: ${wf.name || wf.id}`);
        if (wf.description) lines.push(`  ${wf.description}`);
        lines.push(`  States: ${states}`);
        lines.push(`  /workflow ${wf.id}`);
        lines.push('');
      }

      return { response: lines.join('\n').trim() };
    } catch (err) {
      return { response: `Failed to list workflows: ${err.message}` };
    }
  };
}

// ============================================================================
// Registration API
// ============================================================================

/** Registered command names (for cleanup) */
const _registeredCommands = [];

/**
 * Register all autonomous engine commands into the CommandRegistry.
 *
 * @param {import('../autonomous/engine.js').AutonomousEngine} engine
 */
export function registerAutonomousCommands(_engine) {
  const registry = getCommandRegistry();

  for (const cmd of AUTONOMOUS_COMMANDS) {
    try {
      registry.register({
        name: cmd.name,
        description: cmd.description,
        acceptsArgs: cmd.acceptsArgs,
        handler: cmd.handler(),
        source: 'autonomous',
      });
      _registeredCommands.push(cmd.name);
    } catch (err) {
      console.warn(`[AutonomousCommands] Failed to register /${cmd.name}: ${err.message}`);
    }
  }

  console.log(`[AutonomousCommands] Registered ${_registeredCommands.length} commands`);
}

/**
 * Unregister all autonomous engine commands.
 */
export function unregisterAutonomousCommands() {
  const registry = getCommandRegistry();

  for (const name of _registeredCommands) {
    registry.unregister(name);
  }

  const count = _registeredCommands.length;
  _registeredCommands.length = 0;

  console.log(`[AutonomousCommands] Unregistered ${count} commands`);
}

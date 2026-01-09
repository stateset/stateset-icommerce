/**
 * MCP Tools for Autonomous Business Engine
 *
 * Exposes scheduler, workflows, policies, webhooks, and approvals to AI agents.
 */

import { tool } from '@anthropic-ai/claude-agent-sdk';
import { z } from 'zod';

/**
 * Create MCP tools for the autonomous engine
 */
export function createAutonomousTools(engine) {
  const tools = [];

  // ============================================================================
  // Scheduler Tools
  // ============================================================================

  if (engine.scheduler) {
    tools.push(
      tool(
        'list_scheduled_jobs',
        'List all scheduled jobs. Returns job details including schedule, next run time, and status.',
        {
          status: z.enum(['all', 'enabled', 'disabled']).optional().describe('Filter by status'),
        },
        async ({ status }) => {
          try {
            const enabled = status === 'enabled' ? true : status === 'disabled' ? false : null;
            const jobs = engine.scheduler.listJobs({ enabled });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  count: jobs.length,
                  jobs: jobs.map(j => ({
                    id: j.id,
                    name: j.name,
                    type: j.type,
                    schedule: j.schedule,
                    enabled: j.enabled,
                    status: j.status,
                    nextRunAt: j.nextRunAt,
                    lastRunAt: j.lastRunAt,
                    runCount: j.runCount
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_scheduled_job',
        'Create a new scheduled job. Supports cron expressions, intervals, and one-time execution.',
        {
          name: z.string().describe('Job name'),
          description: z.string().optional().describe('Job description'),
          type: z.enum(['cron', 'interval', 'once']).describe('Schedule type'),
          schedule: z.string().describe('Cron expression (e.g., "0 * * * *"), interval in ms, or ISO date for once'),
          agent: z.string().describe('Agent to execute (e.g., "inventory", "analytics")'),
          request: z.string().describe('Request to send to the agent'),
          enabled: z.boolean().optional().default(true).describe('Whether job is enabled')
        },
        async ({ name, description, type, schedule, agent, request, enabled }) => {
          try {
            const scheduleValue = type === 'interval' ? parseInt(schedule, 10) : schedule;
            const job = engine.scheduler.addJob({
              name,
              description,
              type,
              schedule: scheduleValue,
              action: { agent, request },
              enabled
            });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: `Job "${name}" created`,
                  job: {
                    id: job.id,
                    name: job.name,
                    nextRunAt: job.nextRunAt
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'run_job_now',
        'Manually trigger a scheduled job to run immediately.',
        {
          jobId: z.string().describe('Job ID to run')
        },
        async ({ jobId }) => {
          try {
            const result = await engine.scheduler.runNow(jobId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Job executed',
                  result: {
                    runId: result.runId,
                    status: result.status,
                    duration: result.duration,
                    output: result.output
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'toggle_job',
        'Enable or disable a scheduled job.',
        {
          jobId: z.string().describe('Job ID'),
          enabled: z.boolean().describe('Whether to enable the job')
        },
        async ({ jobId, enabled }) => {
          try {
            const job = enabled
              ? engine.scheduler.resumeJob(jobId)
              : engine.scheduler.pauseJob(jobId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: `Job ${enabled ? 'enabled' : 'disabled'}`,
                  job: { id: job.id, name: job.name, enabled: job.enabled }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_scheduler_status',
        'Get scheduler status including running jobs and recent history.',
        {},
        async () => {
          try {
            const status = engine.scheduler.getStatus();
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({ success: true, status }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      )
    );
  }

  // ============================================================================
  // Workflow Tools
  // ============================================================================

  if (engine.workflows) {
    tools.push(
      tool(
        'list_workflows',
        'List all registered workflow definitions.',
        {},
        async () => {
          try {
            const workflows = engine.workflows.listWorkflows();
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  count: workflows.length,
                  workflows: workflows.map(w => ({
                    id: w.id,
                    name: w.name,
                    description: w.description,
                    initialState: w.initialState,
                    stateCount: w.states.length,
                    finalStates: w.finalStates
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'start_workflow',
        'Start a new workflow instance.',
        {
          workflowId: z.string().describe('Workflow definition ID'),
          context: z.record(z.any()).optional().describe('Initial context data')
        },
        async ({ workflowId, context }) => {
          try {
            const instance = await engine.workflows.startWorkflow(workflowId, { context });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Workflow started',
                  instance: {
                    id: instance.id,
                    workflowName: instance.workflowName,
                    currentState: instance.currentState,
                    status: instance.status
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'transition_workflow',
        'Transition a workflow instance to a new state.',
        {
          instanceId: z.string().describe('Workflow instance ID'),
          targetState: z.string().describe('Target state name'),
          context: z.record(z.any()).optional().describe('Additional context for transition')
        },
        async ({ instanceId, targetState, context }) => {
          try {
            const instance = await engine.workflows.transition(instanceId, targetState, context);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: `Transitioned to ${targetState}`,
                  instance: {
                    id: instance.id,
                    currentState: instance.currentState,
                    status: instance.status,
                    history: instance.history.slice(-3)
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'trigger_workflow_event',
        'Trigger an event on a workflow instance.',
        {
          instanceId: z.string().describe('Workflow instance ID'),
          event: z.string().describe('Event/transition name'),
          context: z.record(z.any()).optional().describe('Event context')
        },
        async ({ instanceId, event, context }) => {
          try {
            const instance = await engine.workflows.trigger(instanceId, event, context);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: `Event "${event}" triggered`,
                  instance: {
                    id: instance.id,
                    currentState: instance.currentState,
                    status: instance.status
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'list_workflow_instances',
        'List workflow instances.',
        {
          workflowId: z.string().optional().describe('Filter by workflow ID'),
          status: z.enum(['running', 'completed', 'failed', 'paused', 'cancelled']).optional().describe('Filter by status')
        },
        async ({ workflowId, status }) => {
          try {
            const instances = engine.workflows.listInstances({ workflowId, status });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  count: instances.length,
                  instances: instances.map(i => ({
                    id: i.id,
                    workflowName: i.workflowName,
                    currentState: i.currentState,
                    status: i.status,
                    updatedAt: i.updatedAt
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_workflow_status',
        'Get overall workflow engine status.',
        {},
        async () => {
          try {
            const status = engine.workflows.getStatus();
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({ success: true, status }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      )
    );
  }

  // ============================================================================
  // Policy Tools
  // ============================================================================

  if (engine.policies) {
    tools.push(
      tool(
        'list_policies',
        'List all registered policy sets.',
        {},
        async () => {
          try {
            const policies = engine.policies.listPolicySets();
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  count: policies.length,
                  policies: policies.map(p => ({
                    id: p.id,
                    name: p.name,
                    domain: p.domain,
                    ruleCount: p.rules.length,
                    version: p.version
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'evaluate_policy',
        'Evaluate policies for a domain with given context.',
        {
          domain: z.string().describe('Policy domain (e.g., "orders", "returns", "inventory")'),
          context: z.record(z.any()).describe('Context for evaluation')
        },
        async ({ domain, context }) => {
          try {
            const result = await engine.policies.evaluate(domain, context);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  domain,
                  shouldAllow: result.shouldAllow,
                  shouldDeny: result.shouldDeny,
                  matchedRules: result.results.flatMap(r => r.rules),
                  actions: result.actions.map(a => a.type)
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_policy_status',
        'Get policy engine status.',
        {},
        async () => {
          try {
            const status = engine.policies.getStatus();
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({ success: true, status }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      )
    );
  }

  // ============================================================================
  // Approval Tools
  // ============================================================================

  if (engine.approvals) {
    tools.push(
      tool(
        'list_pending_approvals',
        'List pending approval requests.',
        {
          domain: z.string().optional().describe('Filter by domain')
        },
        async ({ domain }) => {
          try {
            const requests = engine.approvals.listPending({ domain });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  count: requests.length,
                  requests: requests.map(r => ({
                    id: r.id,
                    title: r.title,
                    domain: r.domain,
                    entityType: r.entityType,
                    entityId: r.entityId,
                    amount: r.amount,
                    currentTier: r.currentTier,
                    status: r.status,
                    createdAt: r.createdAt
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_approval_request',
        'Create a new approval request for an operation.',
        {
          domain: z.string().describe('Domain (e.g., "orders", "returns", "purchase_orders")'),
          entityType: z.string().describe('Entity type (e.g., "order", "return")'),
          entityId: z.string().describe('Entity ID'),
          title: z.string().describe('Request title'),
          description: z.string().optional().describe('Request description'),
          amount: z.number().optional().describe('Amount for threshold-based routing'),
          requestedBy: z.string().describe('Requester ID')
        },
        async ({ domain, entityType, entityId, title, description, amount, requestedBy }) => {
          try {
            const result = await engine.approvals.createRequest({
              domain,
              entityType,
              entityId,
              title,
              description,
              amount,
              requestedBy
            });

            if (!result.required) {
              return {
                content: [{
                  type: 'text',
                  text: JSON.stringify({
                    success: true,
                    approvalRequired: false,
                    message: 'No approval chain configured for this domain'
                  }, null, 2)
                }]
              };
            }

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  approvalRequired: true,
                  request: {
                    id: result.request.id,
                    title: result.request.title,
                    currentTier: result.request.currentTier,
                    status: result.request.status
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'approve_request',
        'Approve an approval request.',
        {
          requestId: z.string().describe('Approval request ID'),
          approverId: z.string().describe('Approver ID'),
          reason: z.string().optional().describe('Approval reason')
        },
        async ({ requestId, approverId, reason }) => {
          try {
            const request = await engine.approvals.approve(requestId, approverId, { reason });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: request.status === 'approved' ? 'Request approved' : 'Approval recorded',
                  request: {
                    id: request.id,
                    status: request.status,
                    currentTier: request.currentTier
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'reject_request',
        'Reject an approval request.',
        {
          requestId: z.string().describe('Approval request ID'),
          approverId: z.string().describe('Approver ID'),
          reason: z.string().optional().describe('Rejection reason')
        },
        async ({ requestId, approverId, reason }) => {
          try {
            const request = await engine.approvals.reject(requestId, approverId, { reason });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Request rejected',
                  request: {
                    id: request.id,
                    status: request.status
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_approval_status',
        'Get approval queue status.',
        {},
        async () => {
          try {
            const status = engine.approvals.getStatus();
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({ success: true, status }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      )
    );
  }

  // ============================================================================
  // Webhook Tools
  // ============================================================================

  if (engine.webhooks) {
    tools.push(
      tool(
        'list_webhook_sources',
        'List registered webhook sources.',
        {},
        async () => {
          try {
            const sources = engine.webhooks.listSources();
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  count: sources.length,
                  sources: sources.map(s => ({
                    id: s.id,
                    name: s.name,
                    path: s.path,
                    enabled: s.enabled
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'list_webhook_handlers',
        'List registered webhook handlers.',
        {},
        async () => {
          try {
            const handlers = engine.webhooks.listHandlers();
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  count: handlers.length,
                  handlers: handlers.map(h => ({
                    id: h.id,
                    name: h.name,
                    sourceId: h.sourceId,
                    eventTypes: h.eventTypes,
                    enabled: h.enabled
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_webhook_history',
        'Get recent webhook event history.',
        {
          limit: z.number().optional().default(20).describe('Number of events to return')
        },
        async ({ limit }) => {
          try {
            const history = engine.webhooks.getHistory({ limit });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  count: history.length,
                  events: history.map(e => ({
                    id: e.id,
                    sourceName: e.sourceName,
                    eventType: e.eventType,
                    status: e.status,
                    receivedAt: e.receivedAt
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_webhook_status',
        'Get webhook server status.',
        {},
        async () => {
          try {
            const status = engine.webhooks.getStatus();
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({ success: true, status }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      )
    );
  }

  // ============================================================================
  // Engine-Level Tools
  // ============================================================================

  tools.push(
    tool(
      'get_autonomous_status',
      'Get comprehensive status of the autonomous business engine.',
      {},
      async () => {
        try {
          const status = engine.getStatus();
          return {
            content: [{
              type: 'text',
              text: JSON.stringify({ success: true, status }, null, 2)
            }]
          };
        } catch (error) {
          return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
        }
      }
    ),

    tool(
      'pre_operation_check',
      'Run policy and approval check before an operation.',
      {
        domain: z.string().describe('Operation domain'),
        context: z.record(z.any()).describe('Operation context')
      },
      async ({ domain, context }) => {
        try {
          const result = await engine.preOperationCheck(domain, context);
          return {
            content: [{
              type: 'text',
              text: JSON.stringify({
                success: true,
                allowed: result.allowed,
                reason: result.reason,
                details: result
              }, null, 2)
            }]
          };
        } catch (error) {
          return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
        }
      }
    )
  );

  return tools;
}

export default createAutonomousTools;

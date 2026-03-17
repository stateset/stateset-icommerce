/**
 * A2A Intelligence Tools Module
 *
 * MCP tools for scheduled actions, agent memory, rules engine,
 * and multi-agent fan-out/join coordination.
 */

import { z } from 'zod';

export const a2aIntelligenceTools = [
  // ==========================================================================
  // Scheduled Actions
  // ==========================================================================
  {
    name: 'a2a_schedule_action',
    description:
      'Schedule a future action: "pay in 3 days", "check escrow every hour", "remind me to follow up".',
    inputSchema: {
      actionType: z
        .enum([
          'payment',
          'quote_request',
          'escrow_check',
          'status_check',
          'custom',
          'reminder',
          'billing',
          'sla_check',
        ])
        .describe('Type of action'),
      payload: z.record(z.any()).describe('Action payload (params for the action)'),
      executeAt: z.string().describe('ISO timestamp for when to execute'),
      repeatInterval: z
        .number()
        .optional()
        .describe('Repeat interval in ms (e.g., 3600000 for hourly)'),
      maxExecutions: z.number().optional().describe('Max times to repeat (default unlimited)'),
      description: z.string().optional().describe('Human-readable description'),
    },
    permission: 'write',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!commerce._schedulerService) {
        return { success: false, error: 'Scheduler service not initialized' };
      }
      return commerce._schedulerService.scheduleAction({
        agentAddress: agentConfig?.walletAddress || 'unknown',
        ...params,
      });
    },
  },
  {
    name: 'a2a_cancel_scheduled',
    description: 'Cancel a scheduled action by ID.',
    inputSchema: {
      actionId: z.string().min(1).describe('Scheduled action ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params }) => {
      if (!commerce._schedulerService) {
        return { success: false, error: 'Scheduler service not initialized' };
      }
      return commerce._schedulerService.cancelAction(params.actionId);
    },
  },
  {
    name: 'a2a_list_scheduled',
    description: 'List scheduled actions. Filter by status or action type.',
    inputSchema: {
      status: z
        .string()
        .optional()
        .describe('Filter by status: pending, completed, failed, cancelled'),
      actionType: z.string().optional().describe('Filter by action type'),
    },
    permission: 'read',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!commerce._schedulerService) {
        return { success: false, error: 'Scheduler service not initialized' };
      }
      return commerce._schedulerService.listActions({
        agentAddress: agentConfig?.walletAddress,
        ...params,
      });
    },
  },
  {
    name: 'a2a_scheduler_metrics',
    description: 'Get scheduler metrics: total scheduled, executed, failed, pending, recurring.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      if (!commerce._schedulerService) {
        return { success: false, error: 'Scheduler service not initialized' };
      }
      return commerce._schedulerService.getMetrics();
    },
  },

  // ==========================================================================
  // Agent Memory
  // ==========================================================================
  {
    name: 'a2a_remember_interaction',
    description:
      'Record an interaction with a counterparty so the agent learns their patterns over time.',
    inputSchema: {
      counterpartyAddress: z.string().min(1).describe('Counterparty wallet address'),
      interactionType: z
        .enum([
          'quote_received',
          'quote_sent',
          'payment_sent',
          'payment_received',
          'negotiation',
          'dispute',
          'fulfillment',
          'rating',
        ])
        .describe('Type of interaction'),
      outcome: z
        .enum(['success', 'failure', 'timeout', 'rejected', 'accepted'])
        .describe('Outcome'),
      amount: z.number().optional().describe('Transaction amount'),
      responseTimeMs: z.number().optional().describe('Response time in ms'),
      metadata: z.record(z.any()).optional().describe('Additional context'),
    },
    permission: 'write',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!commerce._agentMemory) {
        return { success: false, error: 'Agent memory not initialized' };
      }
      return commerce._agentMemory.recordInteraction({
        agentAddress: agentConfig?.walletAddress || 'unknown',
        ...params,
      });
    },
  },
  {
    name: 'a2a_counterparty_profile',
    description:
      'Get learned profile of a counterparty: success rate, reliability, risk level, negotiation patterns.',
    inputSchema: {
      counterpartyAddress: z.string().min(1).describe('Counterparty wallet address'),
    },
    permission: 'read',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!commerce._agentMemory) {
        return { success: false, error: 'Agent memory not initialized' };
      }
      return commerce._agentMemory.getCounterpartyProfile(
        agentConfig?.walletAddress || 'unknown',
        params.counterpartyAddress,
      );
    },
  },
  {
    name: 'a2a_should_transact',
    description:
      'Get AI recommendation on whether to transact with a counterparty, based on learned history.',
    inputSchema: {
      counterpartyAddress: z.string().min(1).describe('Counterparty wallet address'),
      actionType: z.string().optional().describe('Type of action being considered'),
    },
    permission: 'read',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!commerce._agentMemory) {
        return { success: false, error: 'Agent memory not initialized' };
      }
      return commerce._agentMemory.getRecommendation(
        agentConfig?.walletAddress || 'unknown',
        params.counterpartyAddress,
        params.actionType,
      );
    },
  },
  {
    name: 'a2a_agent_insights',
    description:
      'Get aggregate insights: total counterparties, avg success rate, top performers, risk alerts.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce, agentConfig }) => {
      if (!commerce._agentMemory) {
        return { success: false, error: 'Agent memory not initialized' };
      }
      return commerce._agentMemory.getAgentInsights(agentConfig?.walletAddress || 'unknown');
    },
  },
  {
    name: 'a2a_top_counterparties',
    description: 'Get top counterparties ranked by volume, success rate, or reliability.',
    inputSchema: {
      sortBy: z.enum(['volume', 'success_rate', 'reliability']).optional().default('volume'),
      limit: z.number().optional().default(10),
    },
    permission: 'read',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!commerce._agentMemory) {
        return { success: false, error: 'Agent memory not initialized' };
      }
      return commerce._agentMemory.getTopCounterparties(
        agentConfig?.walletAddress || 'unknown',
        params,
      );
    },
  },

  // ==========================================================================
  // Rules Engine
  // ==========================================================================
  {
    name: 'a2a_add_rule',
    description:
      'Add a programmable guardrail rule. Example: "block transactions > $1000 without escrow".',
    inputSchema: {
      name: z.string().min(1).describe('Rule name'),
      description: z.string().optional().describe('Rule description'),
      condition: z
        .record(z.any())
        .describe('Condition object: { field, operator, value } or { all/any: [...] }'),
      action: z
        .object({
          type: z.enum([
            'block',
            'approve',
            'require_escrow',
            'notify',
            'adjust_price',
            'flag_review',
            'pause_agent',
            'custom',
          ]),
          params: z.record(z.any()).optional(),
        })
        .describe('Action to take when rule matches'),
      priority: z.number().optional().default(50).describe('Priority (1-100, higher first)'),
      tags: z.array(z.string()).optional().describe('Tags for filtering'),
    },
    permission: 'write',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!commerce._rulesEngine) {
        return { success: false, error: 'Rules engine not initialized' };
      }
      return commerce._rulesEngine.addRule({
        agentAddress: agentConfig?.walletAddress || 'unknown',
        enabled: true,
        ...params,
      });
    },
  },
  {
    name: 'a2a_evaluate_rules',
    description:
      'Evaluate all active rules against a transaction context. Returns: allowed, matched rules, explanation.',
    inputSchema: {
      context: z
        .record(z.any())
        .describe('Transaction context to evaluate (amount, counterparty, type, etc.)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._rulesEngine) {
        return { success: false, error: 'Rules engine not initialized' };
      }
      return commerce._rulesEngine.evaluate(params.context);
    },
  },
  {
    name: 'a2a_list_rules',
    description: 'List all registered rules. Filter by tags or enabled status.',
    inputSchema: {
      tags: z.array(z.string()).optional().describe('Filter by tags'),
      enabled: z.boolean().optional().describe('Filter by enabled status'),
    },
    permission: 'read',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!commerce._rulesEngine) {
        return { success: false, error: 'Rules engine not initialized' };
      }
      return commerce._rulesEngine.listRules({
        agentAddress: agentConfig?.walletAddress,
        ...params,
      });
    },
  },
  {
    name: 'a2a_rule_audit_log',
    description: 'Get recent rule evaluation audit log — see which rules fired and why.',
    inputSchema: {
      limit: z.number().optional().default(20).describe('Max entries'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._rulesEngine) {
        return { success: false, error: 'Rules engine not initialized' };
      }
      return commerce._rulesEngine.getAuditLog(params.limit);
    },
  },

  // ==========================================================================
  // Fan-Out / Join Coordination
  // ==========================================================================
  {
    name: 'a2a_scatter',
    description:
      'Broadcast a task to multiple agents in parallel (fan-out). Returns coordination ID for tracking.',
    inputSchema: {
      targets: z.array(z.string().min(1)).min(1).max(50).describe('Target agent addresses'),
      taskType: z.string().min(1).describe('Task type (e.g., "quote_request", "status_check")'),
      payload: z.record(z.any()).describe('Task payload sent to each target'),
      timeoutMs: z.number().optional().default(30000).describe('Timeout per target in ms'),
      joinStrategy: z
        .enum(['all', 'first', 'majority', 'best'])
        .optional()
        .default('all')
        .describe('When to aggregate results'),
    },
    permission: 'write',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!commerce._fanOutCoordinator) {
        return { success: false, error: 'Fan-out coordinator not initialized' };
      }
      return commerce._fanOutCoordinator.scatter({
        agentAddress: agentConfig?.walletAddress || 'unknown',
        ...params,
      });
    },
  },
  {
    name: 'a2a_coordination_status',
    description: 'Get status of a fan-out coordination: responses received, pending, timed out.',
    inputSchema: {
      coordinationId: z.string().min(1).describe('Coordination ID from scatter'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._fanOutCoordinator) {
        return { success: false, error: 'Fan-out coordinator not initialized' };
      }
      return commerce._fanOutCoordinator.getStatus(params.coordinationId);
    },
  },
  {
    name: 'a2a_submit_response',
    description: 'Submit a response to a fan-out coordination (as a target agent).',
    inputSchema: {
      coordinationId: z.string().min(1).describe('Coordination ID'),
      response: z.record(z.any()).describe('Your response payload'),
    },
    permission: 'write',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!commerce._fanOutCoordinator) {
        return { success: false, error: 'Fan-out coordinator not initialized' };
      }
      return commerce._fanOutCoordinator.registerResponse(
        params.coordinationId,
        agentConfig?.walletAddress || 'unknown',
        params.response,
      );
    },
  },
  {
    name: 'a2a_join_results',
    description: 'Wait for and aggregate fan-out results based on the join strategy.',
    inputSchema: {
      coordinationId: z.string().min(1).describe('Coordination ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._fanOutCoordinator) {
        return { success: false, error: 'Fan-out coordinator not initialized' };
      }
      return commerce._fanOutCoordinator.join(params.coordinationId);
    },
  },
];

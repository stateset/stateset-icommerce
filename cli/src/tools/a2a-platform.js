/**
 * A2A Platform Tools Module
 *
 * MCP tools for agent messaging, batch operations, state checkpointing,
 * data export, webhook verification, and tick optimization.
 */

import { z } from 'zod';

export const a2aPlatformTools = [
  // ==========================================================================
  // Agent-to-Agent Messaging
  // ==========================================================================
  {
    name: 'a2a_send_message',
    description:
      'Send a direct message to another agent. Supports text, task delegation, and status queries.',
    inputSchema: {
      to: z.string().min(1).describe('Recipient agent address'),
      type: z
        .enum(['text', 'task_delegation', 'status_query', 'data_request'])
        .describe('Message type'),
      payload: z.record(z.any()).describe('Message content'),
      parentMessageId: z.string().optional().describe('Parent message ID for threading'),
    },
    permission: 'write',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!commerce._messagingService) {
        return { success: false, error: 'Messaging service not initialized' };
      }
      return commerce._messagingService.sendMessage({
        from: agentConfig?.walletAddress || 'unknown',
        to: params.to,
        type: params.type,
        payload: params.payload,
        parentMessageId: params.parentMessageId,
      });
    },
  },
  {
    name: 'a2a_get_inbox',
    description: 'Get your message inbox. Filter by unread, type, or limit.',
    inputSchema: {
      unreadOnly: z.boolean().optional().default(false).describe('Only show unread messages'),
      type: z.string().optional().describe('Filter by message type'),
      limit: z.number().optional().default(20).describe('Max messages'),
    },
    permission: 'read',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!commerce._messagingService) {
        return { success: false, error: 'Messaging service not initialized' };
      }
      return commerce._messagingService.getInbox(agentConfig?.walletAddress || 'unknown', params);
    },
  },
  {
    name: 'a2a_delegate_task',
    description:
      'Delegate a task to another agent. Specify description, deadline, reward, and priority.',
    inputSchema: {
      to: z.string().min(1).describe('Agent to delegate to'),
      description: z.string().min(1).describe('Task description'),
      deadline: z.string().optional().describe('ISO deadline'),
      reward: z.number().optional().describe('Reward amount in USD'),
      priority: z.enum(['low', 'medium', 'high', 'critical']).optional().default('medium'),
    },
    permission: 'write',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!commerce._messagingService) {
        return { success: false, error: 'Messaging service not initialized' };
      }
      return commerce._messagingService.delegateTask({
        from: agentConfig?.walletAddress || 'unknown',
        ...params,
      });
    },
  },
  {
    name: 'a2a_respond_to_task',
    description: 'Respond to a delegated task: accept, reject, or mark complete.',
    inputSchema: {
      messageId: z.string().min(1).describe('Task message ID'),
      status: z.enum(['accepted', 'rejected', 'completed']).describe('Response status'),
      result: z.any().optional().describe('Task result (for completed status)'),
    },
    permission: 'write',
    handler: async ({ commerce, params }) => {
      if (!commerce._messagingService) {
        return { success: false, error: 'Messaging service not initialized' };
      }
      return commerce._messagingService.respondToTask(params.messageId, {
        status: params.status,
        result: params.result,
      });
    },
  },
  {
    name: 'a2a_get_thread',
    description: 'Get all messages in a conversation thread.',
    inputSchema: {
      parentMessageId: z.string().min(1).describe('Root message ID of the thread'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._messagingService) {
        return { success: false, error: 'Messaging service not initialized' };
      }
      return commerce._messagingService.getThread(params.parentMessageId);
    },
  },
  {
    name: 'a2a_messaging_metrics',
    description: 'Get messaging metrics: total messages, unread count, avg response time.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      if (!commerce._messagingService) {
        return { success: false, error: 'Messaging service not initialized' };
      }
      return commerce._messagingService.getMetrics();
    },
  },

  // ==========================================================================
  // Batch Operations
  // ==========================================================================
  {
    name: 'a2a_batch_pay',
    description:
      "Execute multiple payments in one call. Each payment is independent — one failure doesn't block others.",
    inputSchema: {
      payments: z
        .array(
          z.object({
            to: z.string().min(1),
            amount: z.number().positive(),
            asset: z.string().optional(),
            network: z.string().optional(),
            memo: z.string().optional(),
          }),
        )
        .min(1)
        .max(100)
        .describe('Array of payment params (max 100)'),
      concurrency: z.number().optional().default(5).describe('Max parallel payments'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Batch payments require --apply flag.',
          wouldPay: params.payments.length,
          totalAmount: params.payments.reduce((s, p) => s + p.amount, 0),
        };
      }
      if (!commerce._batchService) {
        return { success: false, error: 'Batch service not initialized' };
      }
      return commerce._batchService.batchPay(params.payments, {
        concurrency: params.concurrency,
      });
    },
  },
  {
    name: 'a2a_batch_request_quotes',
    description: 'Request quotes from multiple sellers simultaneously.',
    inputSchema: {
      requests: z
        .array(
          z.object({
            seller: z.string().min(1),
            items: z.array(z.any()).min(1),
          }),
        )
        .min(1)
        .max(50)
        .describe('Array of quote requests'),
    },
    permission: 'write',
    handler: async ({ commerce, params }) => {
      if (!commerce._batchService) {
        return { success: false, error: 'Batch service not initialized' };
      }
      return commerce._batchService.batchRequestQuotes(params.requests);
    },
  },

  // ==========================================================================
  // State Checkpoint
  // ==========================================================================
  {
    name: 'a2a_save_checkpoint',
    description: 'Save agent state checkpoint for recovery after restart.',
    inputSchema: {
      data: z.record(z.any()).optional().describe('Custom checkpoint data'),
    },
    permission: 'write',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!commerce._checkpointService) {
        return { success: false, error: 'Checkpoint service not initialized' };
      }
      const addr = agentConfig?.walletAddress || 'default';
      await commerce._checkpointService.saveCheckpoint(addr, {
        ...params.data,
        savedAt: new Date().toISOString(),
      });
      return { success: true, agentAddress: addr };
    },
  },
  {
    name: 'a2a_load_checkpoint',
    description: 'Load last saved agent state checkpoint.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce, agentConfig }) => {
      if (!commerce._checkpointService) {
        return { success: false, error: 'Checkpoint service not initialized' };
      }
      const addr = agentConfig?.walletAddress || 'default';
      const data = await commerce._checkpointService.loadCheckpoint(addr);
      return data || { exists: false };
    },
  },
  {
    name: 'a2a_list_checkpoints',
    description: 'List all saved agent checkpoints.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      if (!commerce._checkpointService) {
        return { success: false, error: 'Checkpoint service not initialized' };
      }
      return commerce._checkpointService.listCheckpoints();
    },
  },

  // ==========================================================================
  // Data Export
  // ==========================================================================
  {
    name: 'a2a_export_agent_data',
    description:
      'Export all commerce data for an agent: payments, quotes, escrows, disputes, subscriptions.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
      redact: z.boolean().optional().default(false).describe('Redact sensitive fields'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._dataExportService) {
        return { success: false, error: 'Data export service not initialized' };
      }
      return commerce._dataExportService.exportAgentData(params.agentAddress, {
        redact: params.redact,
      });
    },
  },
  {
    name: 'a2a_commerce_report',
    description:
      'Generate a commerce report for an agent: volume, transactions, dispute rate, top counterparties, margin.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
      since: z.string().optional().describe('ISO date — report start'),
      until: z.string().optional().describe('ISO date — report end'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._dataExportService) {
        return { success: false, error: 'Data export service not initialized' };
      }
      return commerce._dataExportService.generateReport(params.agentAddress, {
        since: params.since,
        until: params.until,
      });
    },
  },
  {
    name: 'a2a_data_stats',
    description: 'Get row counts for all A2A data tables.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      if (!commerce._dataExportService) {
        return { success: false, error: 'Data export service not initialized' };
      }
      return commerce._dataExportService.getDataStats();
    },
  },

  // ==========================================================================
  // Webhook Verification
  // ==========================================================================
  {
    name: 'a2a_verify_webhook',
    description:
      'Verify a received webhook signature. Use this to validate incoming StateSet webhooks.',
    inputSchema: {
      rawBody: z.string().min(1).describe('Raw JSON body string'),
      signatureHeader: z.string().min(1).describe('X-StateSet-Signature header value'),
      secret: z.string().min(1).describe('Your webhook secret'),
      timestampHeader: z
        .string()
        .optional()
        .describe('X-StateSet-Timestamp header for replay protection'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      let verify;
      try {
        const mod = await import('../a2a/webhook-verify.js');
        verify = mod;
      } catch {
        return { success: false, error: 'Webhook verify module not available' };
      }
      const sigResult = verify.verifyWebhookSignature(
        params.rawBody,
        params.signatureHeader,
        params.secret,
      );
      const result = { ...sigResult };
      if (params.timestampHeader) {
        const tsResult = verify.verifyWebhookTimestamp(params.timestampHeader);
        result.timestampValid = tsResult.valid;
        result.timestampError = tsResult.error;
        result.ageMs = tsResult.ageMs;
      }
      return result;
    },
  },

  // ==========================================================================
  // Tick Optimization
  // ==========================================================================
  {
    name: 'a2a_tick_metrics',
    description:
      'Get tick loop performance metrics: p50/p95/p99 duration, ticks/min, idle streaks, adaptive interval.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      if (!commerce._tickOptimizer) {
        return { success: false, error: 'Tick optimizer not initialized' };
      }
      return commerce._tickOptimizer.getMetrics();
    },
  },
];

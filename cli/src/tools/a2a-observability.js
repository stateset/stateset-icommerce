/**
 * A2A Observability & Protocol Tools Module
 *
 * MCP tools for distributed tracing, agent introspection, settlement finality,
 * protocol handshake, and operational visibility.
 */

import { z } from 'zod';

export const a2aObservabilityTools = [
  // ==========================================================================
  // Distributed Tracing
  // ==========================================================================
  {
    name: 'a2a_get_trace',
    description:
      'Retrieve all spans for a distributed trace ID. Shows the full journey of a transaction across agents.',
    inputSchema: {
      traceId: z.string().min(1).describe('Trace ID (32-char hex)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._tracingService) {
        return { success: false, error: 'Tracing service not initialized' };
      }
      return commerce._tracingService.getTrace(params.traceId);
    },
  },
  {
    name: 'a2a_tracing_metrics',
    description: 'Get tracing metrics: p50/p95/p99 latency, error rate, throughput, span count.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      if (!commerce._tracingService) {
        return { success: false, error: 'Tracing service not initialized' };
      }
      return commerce._tracingService.getMetrics();
    },
  },
  {
    name: 'a2a_recent_spans',
    description: 'Get the most recent trace spans for debugging.',
    inputSchema: {
      limit: z.number().optional().default(20).describe('Max spans to return'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._tracingService) {
        return { success: false, error: 'Tracing service not initialized' };
      }
      return commerce._tracingService.getRecentSpans(params.limit);
    },
  },
  {
    name: 'a2a_export_traces',
    description: 'Export all buffered spans in OpenTelemetry-compatible OTLP JSON format.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      if (!commerce._tracingService) {
        return { success: false, error: 'Tracing service not initialized' };
      }
      return commerce._tracingService.exportOTLP();
    },
  },

  // ==========================================================================
  // Agent Introspection
  // ==========================================================================
  {
    name: 'a2a_agent_dashboard',
    description:
      'Get a full operational dashboard for an agent: runtime status, tick metrics, decisions, budget, sagas, escrows, reputation.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._introspectionService) {
        return { success: false, error: 'Introspection service not initialized' };
      }
      return commerce._introspectionService.getAgentDashboard(params.agentAddress);
    },
  },
  {
    name: 'a2a_agent_decisions',
    description: 'Get recent strategy decisions for an agent: what was accepted/rejected and why.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
      limit: z.number().optional().default(20).describe('Max decisions'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._introspectionService) {
        return { success: false, error: 'Introspection service not initialized' };
      }
      return commerce._introspectionService.getDecisionHistory(params.agentAddress, params.limit);
    },
  },
  {
    name: 'a2a_agent_performance',
    description:
      'Get performance report: quote accept rate, avg response time, settlement success rate, dispute rate.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._introspectionService) {
        return { success: false, error: 'Introspection service not initialized' };
      }
      return commerce._introspectionService.getPerformanceReport(params.agentAddress);
    },
  },
  {
    name: 'a2a_agent_tick_metrics',
    description:
      'Get tick loop metrics: avg duration, ticks/min, quotes evaluated, payments executed, errors.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._introspectionService) {
        return { success: false, error: 'Introspection service not initialized' };
      }
      return commerce._introspectionService.getTickMetrics(params.agentAddress);
    },
  },
  {
    name: 'a2a_agent_lifecycle',
    description:
      'Get agent lifecycle history: start/stop/pause/resume events with timestamps and reasons.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._introspectionService) {
        return { success: false, error: 'Introspection service not initialized' };
      }
      return commerce._introspectionService.getLifecycleHistory(params.agentAddress);
    },
  },

  // ==========================================================================
  // Settlement Finality
  // ==========================================================================
  {
    name: 'a2a_settlement_status',
    description:
      'Get settlement finality status: broadcast → unconfirmed → confirming → final. Shows confirmation count vs chain requirement.',
    inputSchema: {
      intentId: z.string().min(1).describe('Payment intent ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._finalityTracker) {
        return { success: false, error: 'Finality tracker not initialized' };
      }
      return commerce._finalityTracker.getSettlementStatus(params.intentId);
    },
  },
  {
    name: 'a2a_settlement_pending',
    description: 'List all settlements not yet final — awaiting blockchain confirmations.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      if (!commerce._finalityTracker) {
        return { success: false, error: 'Finality tracker not initialized' };
      }
      return commerce._finalityTracker.listPending();
    },
  },
  {
    name: 'a2a_settlement_finality_metrics',
    description: 'Get settlement metrics: avg confirmation time, finality rate, reorg count.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      if (!commerce._finalityTracker) {
        return { success: false, error: 'Finality tracker not initialized' };
      }
      return commerce._finalityTracker.getMetrics();
    },
  },

  // ==========================================================================
  // Protocol Handshake
  // ==========================================================================
  {
    name: 'a2a_handshake',
    description:
      'Initiate capability handshake with another agent. Returns compatibility report: shared networks/assets, feature mismatches, recommended network/asset.',
    inputSchema: {
      targetCapabilities: z
        .object({
          protocolVersion: z.string().optional(),
          supportedNetworks: z.array(z.string()).optional(),
          supportedAssets: z.array(z.string()).optional(),
          features: z.record(z.boolean()).optional(),
          maxTransactionAmount: z.number().optional(),
        })
        .describe('Target agent capabilities (from their agent card or handshake response)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      if (!commerce._handshakeService) {
        return { success: false, error: 'Handshake service not initialized' };
      }
      return commerce._handshakeService.initiateHandshake(params.targetCapabilities);
    },
  },
  {
    name: 'a2a_my_capabilities',
    description: "Get this agent's capability manifest for protocol handshake.",
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      if (!commerce._handshakeService) {
        return { success: false, error: 'Handshake service not initialized' };
      }
      return commerce._handshakeService.getMyCapabilities();
    },
  },
];

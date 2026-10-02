/**
 * Circuit Breaker MCP Tools
 *
 * 8 tools for managing agent circuit breakers — spending limits, failure
 * detection, manual trip/reset, and global kill switch.
 *
 * The circuit breaker service is created once per database (the server's
 * `--db` store) and reused across calls and servers on that database.
 */

import { z } from 'zod';

import { scopedService } from './scoped-store.js';

// ---------------------------------------------------------------------------
// Scoped service — one circuit breaker per server database
// ---------------------------------------------------------------------------

/**
 * @param {object} ctx - tool handler context
 */
async function getCB(ctx) {
  return scopedService(ctx, 'circuit-breaker', async (store) => {
    const { createCircuitBreaker } = await import('../a2a/circuit-breaker.js');
    return createCircuitBreaker(store);
  });
}

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

export const circuitBreakerTools = [
  // ==========================================================================
  // Read operations
  // ==========================================================================
  {
    name: 'agent_get_breaker_state',
    description:
      'Get the circuit breaker state for a specific agent, including trip reason and config.',
    inputSchema: {
      agentName: z.string().min(1).describe('Agent name'),
    },
    permission: 'read',
    handler: async ({ params, ...ctx }) => {
      try {
        const cb = await getCB(ctx);
        const state = cb.getState(params.agentName);
        return { success: true, ...state };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  {
    name: 'agent_get_spending_summary',
    description:
      "Get the spending summary for an agent: today's spend, monthly spend, and remaining limits.",
    inputSchema: {
      agentName: z.string().min(1).describe('Agent name'),
    },
    permission: 'read',
    handler: async ({ params, ...ctx }) => {
      try {
        const cb = await getCB(ctx);
        const summary = cb.getSpendingSummary(params.agentName);
        return { success: true, agentName: params.agentName, ...summary };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  {
    name: 'agent_get_all_breaker_states',
    description: 'Get the circuit breaker states for all known agents.',
    inputSchema: {},
    permission: 'read',
    handler: async (ctx) => {
      try {
        const cb = await getCB(ctx);
        const states = cb.getAllStates();
        return { success: true, agents: states, count: states.length };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  // ==========================================================================
  // Admin operations
  // ==========================================================================
  {
    name: 'agent_trip_breaker',
    description:
      'Manually trip the circuit breaker for a specific agent. Blocks all transactions until reset.',
    inputSchema: {
      agentName: z.string().min(1).describe('Agent name'),
      reason: z.string().min(1).max(500).describe('Reason for tripping the circuit breaker'),
    },
    permission: 'admin',
    handler: async ({ params, ...ctx }) => {
      try {
        const cb = await getCB(ctx);
        cb.trip(params.agentName, params.reason);
        const state = cb.getState(params.agentName);
        return {
          success: true,
          message: `Circuit breaker tripped for agent "${params.agentName}"`,
          ...state,
        };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  {
    name: 'agent_trip_all_breakers',
    description: 'Activate the global kill switch — blocks ALL agent transactions immediately.',
    inputSchema: {
      reason: z.string().min(1).max(500).describe('Reason for global kill switch activation'),
    },
    permission: 'admin',
    handler: async ({ params, ...ctx }) => {
      try {
        const cb = await getCB(ctx);
        cb.tripAll(params.reason);
        const states = cb.getAllStates();
        return {
          success: true,
          message: 'Global kill switch activated — all agents blocked',
          agents: states,
          count: states.length,
        };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  {
    name: 'agent_reset_breaker',
    description: 'Reset the circuit breaker for a specific agent, allowing transactions again.',
    inputSchema: {
      agentName: z.string().min(1).describe('Agent name'),
    },
    permission: 'admin',
    handler: async ({ params, ...ctx }) => {
      try {
        const cb = await getCB(ctx);
        cb.reset(params.agentName);
        const state = cb.getState(params.agentName);
        return {
          success: true,
          message: `Circuit breaker reset for agent "${params.agentName}"`,
          ...state,
        };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  {
    name: 'agent_reset_all_breakers',
    description: 'Reset ALL circuit breakers and deactivate the global kill switch.',
    inputSchema: {},
    permission: 'admin',
    handler: async (ctx) => {
      try {
        const cb = await getCB(ctx);
        cb.resetAll();
        const states = cb.getAllStates();
        return {
          success: true,
          message: 'All circuit breakers reset and kill switch deactivated',
          agents: states,
          count: states.length,
        };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  {
    name: 'agent_set_spending_limits',
    description:
      'Update the spending limits for agent circuit breakers: per-transaction, daily, and monthly caps.',
    inputSchema: {
      maxSpendPerTx: z.number().positive().optional().describe('Maximum spend per transaction'),
      dailySpendLimit: z
        .number()
        .positive()
        .optional()
        .describe('Maximum daily spend across all transactions'),
      monthlySpendLimit: z
        .number()
        .positive()
        .optional()
        .describe('Maximum monthly spend across all transactions'),
    },
    permission: 'admin',
    handler: async ({ params, ...ctx }) => {
      try {
        const cb = await getCB(ctx);
        const overrides = {};
        if (params.maxSpendPerTx !== undefined) overrides.maxSpendPerTx = params.maxSpendPerTx;
        if (params.dailySpendLimit !== undefined)
          overrides.dailySpendLimit = params.dailySpendLimit;
        if (params.monthlySpendLimit !== undefined)
          overrides.monthlySpendLimit = params.monthlySpendLimit;
        if (Object.keys(overrides).length === 0) {
          return { success: false, error: 'At least one limit must be provided' };
        }
        cb.updateConfig(overrides);
        // Return the current state of an arbitrary agent to show updated config
        const state = cb.getState('__config_check__');
        return {
          success: true,
          message: 'Spending limits updated',
          config: {
            maxSpendPerTx: state.config.maxSpendPerTx,
            dailySpendLimit: state.config.dailySpendLimit,
            monthlySpendLimit: state.config.monthlySpendLimit,
          },
        };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },
];

export default circuitBreakerTools;

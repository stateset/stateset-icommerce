/**
 * Circuit Breaker MCP Tools
 *
 * 8 tools for managing agent circuit breakers — spending limits, failure
 * detection, manual trip/reset, and global kill switch.
 *
 * Uses a lazy singleton pattern: the circuit breaker service is created once
 * per process and reuses the same A2A database.
 */

import { z } from 'zod';

// ---------------------------------------------------------------------------
// Lazy singleton — creates the circuit breaker on first use
// ---------------------------------------------------------------------------

let _cbSingleton = null;

async function getCB() {
  if (_cbSingleton) return _cbSingleton;
  const { A2AStore } = await import('../a2a/store.js');
  const { createCircuitBreaker } = await import('../a2a/circuit-breaker.js');
  const store = new A2AStore();
  store.init();
  _cbSingleton = createCircuitBreaker(store);
  return _cbSingleton;
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
    handler: async ({ params }) => {
      try {
        const cb = await getCB();
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
    handler: async ({ params }) => {
      try {
        const cb = await getCB();
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
    handler: async () => {
      try {
        const cb = await getCB();
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
    handler: async ({ params }) => {
      try {
        const cb = await getCB();
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
    handler: async ({ params }) => {
      try {
        const cb = await getCB();
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
    handler: async ({ params }) => {
      try {
        const cb = await getCB();
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
    handler: async () => {
      try {
        const cb = await getCB();
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
    handler: async ({ params }) => {
      try {
        const cb = await getCB();
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

/**
 * Reset the singleton (for testing).
 */
export function _resetCBSingleton() {
  _cbSingleton = null;
}

export default circuitBreakerTools;

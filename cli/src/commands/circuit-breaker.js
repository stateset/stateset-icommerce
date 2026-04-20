/**
 * Circuit Breaker Commands Module
 */

let cbPromise = null;

async function getCB() {
  if (!cbPromise) {
    cbPromise = (async () => {
      const { A2AStore } = await import('../a2a/store.js');
      const { createCircuitBreaker } = await import('../a2a/circuit-breaker.js');
      const store = new A2AStore();
      store.init();
      return createCircuitBreaker(store);
    })();
  }
  return cbPromise;
}

export async function execute(action, args, { output, jsonOutput }) {
  const cb = await getCB();

  switch (action) {
    case 'state': {
      const agentName = args[0];
      if (!agentName) throw new Error('Usage: circuit-breaker state <agentName>');
      const result = cb.getState(agentName);
      return jsonOutput
        ? result
        : {
            result,
            formatted: `Breaker state for ${agentName}: ${result.tripped ? 'tripped' : 'healthy'}`,
          };
    }

    case 'spending': {
      const agentName = args[0];
      if (!agentName) throw new Error('Usage: circuit-breaker spending <agentName>');
      const result = cb.getSpendingSummary(agentName);
      return jsonOutput ? result : { result, formatted: `Spending summary for ${agentName}` };
    }

    case 'states': {
      const agents = cb.getAllStates();
      return formatRows(agents, { output, jsonOutput, empty: 'No breaker states found.' });
    }

    case 'trip': {
      const [agentName, ...reasonParts] = args;
      if (!agentName || reasonParts.length === 0) {
        throw new Error('Usage: circuit-breaker trip <agentName> <reason>');
      }
      cb.trip(agentName, reasonParts.join(' '));
      return { formatted: `Tripped breaker for ${agentName}` };
    }

    case 'trip-all': {
      const reason = args.join(' ');
      if (!reason) throw new Error('Usage: circuit-breaker trip-all <reason>');
      cb.tripAll(reason);
      return { formatted: 'Activated global breaker kill switch' };
    }

    case 'reset': {
      const agentName = args[0];
      if (!agentName) throw new Error('Usage: circuit-breaker reset <agentName>');
      cb.reset(agentName);
      return { formatted: `Reset breaker for ${agentName}` };
    }

    case 'reset-all': {
      cb.resetAll();
      return { formatted: 'Reset all circuit breakers' };
    }

    case 'limits': {
      const [maxSpendPerTxRaw, dailySpendLimitRaw, monthlySpendLimitRaw] = args;
      const overrides = {};
      if (maxSpendPerTxRaw !== undefined)
        overrides.maxSpendPerTx = Number.parseFloat(maxSpendPerTxRaw);
      if (dailySpendLimitRaw !== undefined)
        overrides.dailySpendLimit = Number.parseFloat(dailySpendLimitRaw);
      if (monthlySpendLimitRaw !== undefined)
        overrides.monthlySpendLimit = Number.parseFloat(monthlySpendLimitRaw);
      if (Object.keys(overrides).length === 0) {
        throw new Error(
          'Usage: circuit-breaker limits [maxSpendPerTx] [dailySpendLimit] [monthlySpendLimit]',
        );
      }
      cb.updateConfig(overrides);
      return { config: overrides, formatted: 'Updated circuit breaker spending limits' };
    }

    default:
      throw new Error(
        `Unknown action: circuit-breaker ${action}\n\n` +
          'Available actions:\n' +
          '  state <agentName>                                   Get breaker state\n' +
          '  spending <agentName>                                Get spending summary\n' +
          '  states                                              List all breaker states\n' +
          '  trip <agentName> <reason>                           Trip breaker\n' +
          '  trip-all <reason>                                   Trip all breakers\n' +
          '  reset <agentName>                                   Reset breaker\n' +
          '  reset-all                                           Reset all breakers\n' +
          '  limits [maxSpendPerTx] [dailySpendLimit] [monthlySpendLimit]  Update limits',
      );
  }
}

function formatRows(rows, { output, jsonOutput, empty }) {
  if (jsonOutput) return rows;
  if (!rows || rows.length === 0) return { formatted: empty };
  const columns = Object.keys(rows[0])
    .slice(0, 6)
    .map((key) => ({ key, header: key }));
  return { rows, formatted: output.table(rows, columns) };
}

export const metadata = {
  name: 'circuit-breaker',
  aliases: ['cb', 'breaker'],
  description: 'Agent circuit-breaker and spending-limit commands',
  actions: {
    state: { description: 'Get breaker state', args: ['<agentName>'] },
    spending: { description: 'Get spending summary', args: ['<agentName>'] },
    states: { description: 'List breaker states', args: [] },
    trip: { description: 'Trip breaker', args: ['<agentName>', '<reason>'] },
    'trip-all': { description: 'Trip all breakers', args: ['<reason>'] },
    reset: { description: 'Reset breaker', args: ['<agentName>'] },
    'reset-all': { description: 'Reset all breakers', args: [] },
    limits: {
      description: 'Update spending limits',
      args: ['[maxSpendPerTx]', '[dailySpendLimit]', '[monthlySpendLimit]'],
    },
  },
};

export default { execute, metadata };

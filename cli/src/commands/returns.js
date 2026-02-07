/**
 * Returns Commands Module
 *
 * Handles all return-related CLI operations for stateset-direct
 */

/**
 * Execute return commands
 * @param {string} action - The action to perform
 * @param {Array} args - Command arguments
 * @param {Object} options - Command options
 * @returns {Promise<any>} Command result
 */
export async function execute(action, args, { commerce, output, jsonOutput, resolveId }) {
  switch (action) {
    case 'list': {
      const returns = await commerce.returns.list();
      return formatReturnList(returns, { output, jsonOutput });
    }

    case 'get': {
      const idArg = args[0];
      if (!idArg) {
        throw new Error('Usage: returns get <id>\n\nProvide a return ID.');
      }

      const id = await resolveId(idArg, 'returns');
      const ret = await commerce.returns.get(id);

      if (!ret) {
        throw new Error(
          `Return not found: ${idArg}\n\nTry 'stateset-direct returns list' to see all returns.`,
        );
      }

      return formatReturnDetail(ret, { output, jsonOutput });
    }

    case 'approve': {
      const idArg = args[0];
      if (!idArg) {
        throw new Error('Usage: returns approve <id>\n\nApprove a pending return request.');
      }

      const id = await resolveId(idArg, 'returns');
      const ret = await commerce.returns.approve(id);

      return formatReturnApproved(ret, { output, jsonOutput });
    }

    case 'reject': {
      const [idArg, ...reasonParts] = args;
      const reason = reasonParts.join(' ');

      if (!idArg || !reason) {
        throw new Error(
          'Usage: returns reject <id> <reason>\n\n' +
            'Example: stateset-direct returns reject abc123 "Outside return window"',
        );
      }

      const id = await resolveId(idArg, 'returns');
      const ret = await commerce.returns.reject(id, reason);

      return formatReturnRejected(ret, reason, { output, jsonOutput });
    }

    case 'count': {
      const count = await commerce.returns.count();
      return { count, formatted: `Return count: ${count}` };
    }

    case 'pending': {
      const returns = await commerce.returns.list();
      const pending = returns.filter((r) => r.status === 'pending' || r.status === 'requested');
      return formatReturnList(pending, { output, jsonOutput });
    }

    case 'create': {
      const [orderId, reason] = args;
      if (!orderId || !reason) {
        throw new Error(
          'Usage: returns create <orderId> <reason>\n\n' +
            'Example: stateset-direct returns create abc123 "Defective product"',
        );
      }

      const resolvedOrderId = await resolveId(orderId, 'orders');
      const ret = await commerce.returns.create({
        orderId: resolvedOrderId,
        reason,
      });

      return formatReturnCreated(ret, { output, jsonOutput });
    }

    case 'stats': {
      const returns = await commerce.returns.list();
      const stats = {
        total: returns.length,
        pending: returns.filter((r) => r.status === 'pending' || r.status === 'requested').length,
        approved: returns.filter((r) => r.status === 'approved').length,
        rejected: returns.filter((r) => r.status === 'rejected').length,
        completed: returns.filter((r) => r.status === 'completed').length,
      };

      return formatReturnStats(stats, { output, jsonOutput });
    }

    default:
      throw new Error(
        `Unknown action: returns ${action}\n\n` +
          'Available actions:\n' +
          '  list              List all returns\n' +
          '  get <id>          Get return details\n' +
          '  create <orderId> <reason>  Create a return\n' +
          '  approve <id>      Approve a return\n' +
          '  reject <id> <reason>  Reject a return\n' +
          '  count             Count returns\n' +
          '  pending           List pending returns\n' +
          '  stats             Show return statistics',
      );
  }
}

/**
 * Format return list for output
 */
function formatReturnList(returns, { output, jsonOutput }) {
  if (jsonOutput) {
    return returns;
  }

  if (returns.length === 0) {
    return { formatted: 'No returns found.' };
  }

  const formatted = output.table(
    returns.map((r) => ({
      id: r.id.slice(0, 8) + '...',
      order: r.orderId.slice(0, 8) + '...',
      status: r.status,
      reason: r.reason?.length > 25 ? r.reason.slice(0, 22) + '...' : r.reason,
      created: r.createdAt?.slice(0, 10) || 'N/A',
    })),
    [
      { key: 'id', header: 'ID' },
      { key: 'order', header: 'Order' },
      { key: 'status', header: 'Status' },
      { key: 'reason', header: 'Reason' },
      { key: 'created', header: 'Created' },
    ],
  );

  return { returns, formatted };
}

/**
 * Format single return detail
 */
function formatReturnDetail(ret, { output: _output, jsonOutput }) {
  if (jsonOutput) {
    return ret;
  }

  const itemLines =
    ret.items?.map((i) => `  - ${i.name || i.sku} x${i.quantity}`).join('\n') || '  (all items)';

  const formatted = `
Return: ${ret.id}
${'-'.repeat(40)}
Order:     ${ret.orderId}
Status:    ${ret.status}
Reason:    ${ret.reason}
Created:   ${ret.createdAt}
Updated:   ${ret.updatedAt || 'N/A'}

Items:
${itemLines}
`;

  return { return: ret, formatted };
}

/**
 * Format return approved response
 */
function formatReturnApproved(ret, { output: _output, jsonOutput }) {
  if (jsonOutput) {
    return { success: true, return: ret };
  }

  return {
    return: ret,
    formatted: `Return ${ret.id.slice(0, 8)}... approved`,
  };
}

/**
 * Format return rejected response
 */
function formatReturnRejected(ret, reason, { output: _output, jsonOutput }) {
  if (jsonOutput) {
    return { success: true, return: ret };
  }

  return {
    return: ret,
    formatted: `Return ${ret.id.slice(0, 8)}... rejected\n  Reason: ${reason}`,
  };
}

/**
 * Format return created response
 */
function formatReturnCreated(ret, { output: _output, jsonOutput }) {
  if (jsonOutput) {
    return { success: true, return: ret };
  }

  return {
    return: ret,
    formatted: `Return created: ${ret.id}\n  Order: ${ret.orderId}\n  Status: ${ret.status}`,
  };
}

/**
 * Format return statistics
 */
function formatReturnStats(stats, { output: _output, jsonOutput }) {
  if (jsonOutput) {
    return stats;
  }

  const formatted = `
Return Statistics
${'-'.repeat(30)}
Total:     ${stats.total}
Pending:   ${stats.pending}
Approved:  ${stats.approved}
Rejected:  ${stats.rejected}
Completed: ${stats.completed}
`;

  return { stats, formatted };
}

/**
 * Command metadata for help/completion
 */
export const metadata = {
  name: 'returns',
  aliases: ['r', 'ret'],
  description: 'Return management commands',
  actions: {
    list: { description: 'List all returns', args: [] },
    get: { description: 'Get return by ID', args: ['<id>'] },
    create: { description: 'Create a return', args: ['<orderId>', '<reason>'] },
    approve: { description: 'Approve a return', args: ['<id>'] },
    reject: { description: 'Reject a return', args: ['<id>', '<reason>'] },
    count: { description: 'Count returns', args: [] },
    pending: { description: 'List pending returns', args: [] },
    stats: { description: 'Show return statistics', args: [] },
  },
};

export default { execute, metadata };

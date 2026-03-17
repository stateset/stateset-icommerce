/**
 * A2A Data Export Service
 *
 * Enables backup, restore, and reporting of agent commerce data.
 * Supports JSON and CSV export with optional privacy controls.
 *
 * Features:
 *   - Export all data for a specific agent
 *   - Export entire A2A database
 *   - Generate human-readable commerce reports
 *   - CSV export for specific entity types
 *   - Table row counts / stats
 *   - Privacy redaction (mask addresses, round amounts)
 *   - Date-range filtering
 *
 * @example
 * ```javascript
 * const exporter = createDataExportService(store);
 *
 * // Full agent data export
 * const data = await exporter.exportAgentData('0xAlice');
 *
 * // Commerce report
 * const report = await exporter.generateReport('0xAlice', {
 *   since: '2025-01-01', until: '2025-12-31',
 * });
 *
 * // CSV
 * const csv = await exporter.exportCSV('0xAlice', 'payments');
 * ```
 */

/**
 * Mask a wallet address for privacy: keep first 6 and last 4 chars.
 * @param {string} address
 * @returns {string}
 */
function maskAddress(address) {
  if (!address || typeof address !== 'string') return address;
  if (address.length <= 10) return '****';
  return `${address.slice(0, 6)}...${address.slice(-4)}`;
}

/**
 * Round a number to 2 decimal places for privacy.
 * @param {number} amount
 * @returns {number}
 */
function roundAmount(amount) {
  if (typeof amount !== 'number') return amount;
  return Math.round(amount * 100) / 100;
}

/**
 * Apply privacy redaction to an array of records.
 * Masks addresses and rounds amounts.
 *
 * @param {Array<Object>} records
 * @param {Array<string>} addressFields - Field names containing addresses
 * @param {Array<string>} amountFields - Field names containing amounts
 * @returns {Array<Object>} Redacted copies
 */
function redactRecords(records, addressFields = [], amountFields = []) {
  return records.map((r) => {
    const copy = { ...r };
    for (const field of addressFields) {
      if (copy[field]) copy[field] = maskAddress(copy[field]);
    }
    for (const field of amountFields) {
      if (typeof copy[field] === 'number') copy[field] = roundAmount(copy[field]);
    }
    return copy;
  });
}

/**
 * Apply date-range filter to records.
 *
 * @param {Array<Object>} records
 * @param {Object} [dateRange]
 * @param {string} [dateRange.since] - ISO date string, inclusive lower bound
 * @param {string} [dateRange.until] - ISO date string, inclusive upper bound
 * @param {string} [dateField='created_at'] - Which field to filter on
 * @returns {Array<Object>}
 */
function filterByDateRange(records, dateRange, dateField = 'created_at') {
  if (!dateRange) return records;
  const { since, until } = dateRange;
  return records.filter((r) => {
    const d = r[dateField];
    if (!d) return true;
    if (since && d < since) return false;
    if (until && d > until) return false;
    return true;
  });
}

/**
 * Convert an array of objects to CSV string.
 *
 * @param {Array<Object>} records
 * @returns {string}
 */
function toCSV(records) {
  if (!records || records.length === 0) return '';

  const headers = Object.keys(records[0]);
  const lines = [headers.join(',')];

  for (const record of records) {
    const values = headers.map((h) => {
      const val = record[h];
      if (val === null || val === undefined) return '';
      const str = typeof val === 'object' ? JSON.stringify(val) : String(val);
      // Escape commas and quotes
      if (str.includes(',') || str.includes('"') || str.includes('\n')) {
        return `"${str.replace(/"/g, '""')}"`;
      }
      return str;
    });
    lines.push(values.join(','));
  }

  return lines.join('\n');
}

/** Address fields commonly found in A2A records */
const ADDR_FIELDS = [
  'sender_address',
  'recipient_address',
  'buyer_address',
  'seller_address',
  'requester_address',
  'payer_address',
  'subscriber_address',
  'provider_address',
  'agent_address',
  'filed_by',
  'filed_against',
  'platform_fee_address',
];

/** Amount fields commonly found in A2A records */
const AMOUNT_FIELDS = [
  'amount',
  'amount_decimal',
  'total',
  'total_decimal',
  'subtotal',
  'fees',
  'tax',
  'amount_paid',
  'resolution_amount',
  'share_amount',
  'share_amount_decimal',
];

/**
 * Supported entity types and their store method mappings.
 */
const ENTITY_TYPES = {
  payments: {
    listAll: 'listPayments',
    agentFilter: (addr) => ({ sender_address: addr }),
    agentFilterAlt: (addr) => ({ recipient_address: addr }),
  },
  quotes: {
    listAll: 'listQuotes',
    agentFilter: (addr) => ({ buyer_address: addr }),
    agentFilterAlt: (addr) => ({ seller_address: addr }),
  },
  escrows: {
    listAll: 'listEscrows',
    agentFilter: (addr) => ({ buyer_address: addr }),
    agentFilterAlt: (addr) => ({ seller_address: addr }),
  },
  disputes: {
    listAll: 'listDisputes',
    agentFilter: (addr) => ({ filed_by: addr }),
    agentFilterAlt: (addr) => ({ filed_against: addr }),
  },
  subscriptions: {
    listAll: 'listSubscriptions',
    agentFilter: (addr) => ({ subscriber_address: addr }),
    agentFilterAlt: (addr) => ({ provider_address: addr }),
  },
  services: {
    listAll: 'listServices',
    agentFilter: (addr) => ({ agent_address: addr }),
  },
  feedback: {
    listAll: 'listFeedback',
    agentFilter: (addr) => ({ reviewer_address: addr }),
    agentFilterAlt: (addr) => ({ agent_address: addr }),
  },
};

/**
 * Create a data export service.
 *
 * @param {Object} store - A2A store instance
 * @returns {Object} Data export API
 */
export function createDataExportService(store) {
  if (!store) {
    throw new Error('store is required');
  }

  /**
   * Safely call a store method — returns empty array if method doesn't exist.
   * @param {string} method
   * @param {Object} [filter]
   * @returns {Array}
   */
  function safeList(method, filter = {}) {
    if (typeof store[method] !== 'function') return [];
    try {
      const result = store[method](filter);
      // Handle both sync and async store implementations
      if (result && typeof result.then === 'function') {
        return result;
      }
      return result || [];
    } catch (_) {
      return [];
    }
  }

  /**
   * Export all data for a specific agent.
   *
   * @param {string} agentAddress - Agent wallet address
   * @param {Object} [options]
   * @param {boolean} [options.redact=false] - Redact sensitive fields
   * @param {Object} [options.dateRange] - { since, until } filter
   * @returns {Promise<Object>} All agent data
   */
  async function exportAgentData(agentAddress, options = {}) {
    if (!agentAddress) throw new Error('agentAddress is required');

    const { redact = false, dateRange } = options;

    const data = {};

    for (const [entityType, config] of Object.entries(ENTITY_TYPES)) {
      let records = await safeList(config.listAll, config.agentFilter(agentAddress));

      // Also include records where the agent is on the other side
      if (config.agentFilterAlt) {
        const altRecords = await safeList(config.listAll, config.agentFilterAlt(agentAddress));
        // Merge, dedup by id
        const seenIds = new Set(records.map((r) => r.id));
        for (const r of altRecords) {
          if (!seenIds.has(r.id)) {
            records.push(r);
            seenIds.add(r.id);
          }
        }
      }

      if (dateRange) {
        records = filterByDateRange(records, dateRange);
      }

      if (redact) {
        records = redactRecords(records, ADDR_FIELDS, AMOUNT_FIELDS);
      }

      data[entityType] = records;
    }

    // Reputation
    let reputation = null;
    if (typeof store.getReputationScore === 'function') {
      try {
        reputation = store.getReputationScore(agentAddress);
        if (reputation && typeof reputation.then === 'function') {
          reputation = await reputation;
        }
      } catch (_) {
        reputation = null;
      }
    }
    data.reputation = reputation;

    return {
      agentAddress: redact ? maskAddress(agentAddress) : agentAddress,
      exportedAt: new Date().toISOString(),
      data,
    };
  }

  /**
   * Export the entire A2A database as JSON.
   *
   * @param {Object} [options]
   * @param {boolean} [options.redact=false] - Redact sensitive fields
   * @param {Object} [options.dateRange] - { since, until } filter
   * @returns {Promise<Object>} All data
   */
  async function exportAllData(options = {}) {
    const { redact = false, dateRange } = options;
    const data = {};

    for (const [entityType, config] of Object.entries(ENTITY_TYPES)) {
      let records = await safeList(config.listAll, {});

      if (dateRange) {
        records = filterByDateRange(records, dateRange);
      }

      if (redact) {
        records = redactRecords(records, ADDR_FIELDS, AMOUNT_FIELDS);
      }

      data[entityType] = records;
    }

    return {
      exportedAt: new Date().toISOString(),
      data,
    };
  }

  /**
   * Generate a human-readable commerce report for an agent.
   *
   * @param {string} agentAddress
   * @param {Object} [dateRange] - { since, until }
   * @returns {Promise<Object>} Report with metrics
   */
  async function generateReport(agentAddress, dateRange) {
    if (!agentAddress) throw new Error('agentAddress is required');

    // Gather raw data
    let sentPayments = await safeList('listPayments', { sender_address: agentAddress });
    let receivedPayments = await safeList('listPayments', { recipient_address: agentAddress });
    let buyerQuotes = await safeList('listQuotes', { buyer_address: agentAddress });
    let sellerQuotes = await safeList('listQuotes', { seller_address: agentAddress });
    let disputes = await safeList('listDisputes', { filed_by: agentAddress });
    let disputesAgainst = await safeList('listDisputes', { filed_against: agentAddress });

    if (dateRange) {
      sentPayments = filterByDateRange(sentPayments, dateRange);
      receivedPayments = filterByDateRange(receivedPayments, dateRange);
      buyerQuotes = filterByDateRange(buyerQuotes, dateRange);
      sellerQuotes = filterByDateRange(sellerQuotes, dateRange);
      disputes = filterByDateRange(disputes, dateRange);
      disputesAgainst = filterByDateRange(disputesAgainst, dateRange);
    }

    // Compute metrics
    const totalSent = sentPayments.reduce((s, p) => s + (p.amount_decimal || 0), 0);
    const totalReceived = receivedPayments.reduce((s, p) => s + (p.amount_decimal || 0), 0);
    const totalTransactions = sentPayments.length + receivedPayments.length;

    // Dispute rate
    const allDisputes = [...disputes, ...disputesAgainst];
    const totalQuotes = buyerQuotes.length + sellerQuotes.length;
    const disputeRate = totalQuotes > 0 ? allDisputes.length / totalQuotes : 0;

    // Top counterparties (by volume)
    const counterpartyVolume = new Map();
    for (const p of sentPayments) {
      const addr = p.recipient_address;
      counterpartyVolume.set(addr, (counterpartyVolume.get(addr) || 0) + (p.amount_decimal || 0));
    }
    for (const p of receivedPayments) {
      const addr = p.sender_address;
      counterpartyVolume.set(addr, (counterpartyVolume.get(addr) || 0) + (p.amount_decimal || 0));
    }
    const topCounterparties = [...counterpartyVolume.entries()]
      .sort((a, b) => b[1] - a[1])
      .slice(0, 10)
      .map(([address, volume]) => ({ address, volume: roundAmount(volume) }));

    // Margin analysis
    const netFlow = totalReceived - totalSent;
    const margin = totalReceived > 0 ? (netFlow / totalReceived) * 100 : 0;

    return {
      agentAddress,
      dateRange: dateRange || null,
      generatedAt: new Date().toISOString(),
      summary: {
        totalVolume: roundAmount(totalSent + totalReceived),
        totalSent: roundAmount(totalSent),
        totalReceived: roundAmount(totalReceived),
        netFlow: roundAmount(netFlow),
        margin: roundAmount(margin),
        transactionCount: totalTransactions,
        sentCount: sentPayments.length,
        receivedCount: receivedPayments.length,
      },
      quotes: {
        asBuyer: buyerQuotes.length,
        asSeller: sellerQuotes.length,
        total: totalQuotes,
      },
      disputes: {
        filed: disputes.length,
        receivedAgainst: disputesAgainst.length,
        total: allDisputes.length,
        disputeRate: roundAmount(disputeRate * 100),
      },
      topCounterparties,
    };
  }

  /**
   * Export a specific entity type as CSV for an agent.
   *
   * @param {string} agentAddress
   * @param {string} entityType - One of: payments, quotes, escrows, disputes, subscriptions, services, feedback
   * @param {Object} [options]
   * @param {boolean} [options.redact=false] - Redact sensitive fields
   * @param {Object} [options.dateRange] - { since, until }
   * @returns {Promise<string>} CSV string
   */
  async function exportCSV(agentAddress, entityType, options = {}) {
    if (!agentAddress) throw new Error('agentAddress is required');
    if (!entityType) throw new Error('entityType is required');

    const config = ENTITY_TYPES[entityType];
    if (!config) {
      throw new Error(
        `Unknown entity type: ${entityType}. Valid types: ${Object.keys(ENTITY_TYPES).join(', ')}`,
      );
    }

    const { redact = false, dateRange } = options;

    let records = await safeList(config.listAll, config.agentFilter(agentAddress));

    if (config.agentFilterAlt) {
      const altRecords = await safeList(config.listAll, config.agentFilterAlt(agentAddress));
      const seenIds = new Set(records.map((r) => r.id));
      for (const r of altRecords) {
        if (!seenIds.has(r.id)) {
          records.push(r);
          seenIds.add(r.id);
        }
      }
    }

    if (dateRange) {
      records = filterByDateRange(records, dateRange);
    }

    if (redact) {
      records = redactRecords(records, ADDR_FIELDS, AMOUNT_FIELDS);
    }

    return toCSV(records);
  }

  /**
   * Get row counts for all A2A tables.
   *
   * @returns {Promise<Object>} { payments, quotes, escrows, disputes, subscriptions, services, feedback }
   */
  async function getDataStats() {
    const stats = {};

    for (const [entityType, config] of Object.entries(ENTITY_TYPES)) {
      const records = await safeList(config.listAll, {});
      stats[entityType] = records.length;
    }

    return {
      ...stats,
      total: Object.values(stats).reduce((s, n) => s + n, 0),
      generatedAt: new Date().toISOString(),
    };
  }

  return {
    exportAgentData,
    exportAllData,
    generateReport,
    exportCSV,
    getDataStats,
  };
}

export default { createDataExportService };

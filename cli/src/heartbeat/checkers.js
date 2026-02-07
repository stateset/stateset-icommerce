/**
 * Heartbeat Commerce Checkers
 *
 * Pure async functions that query commerce state and return:
 *   { triggered: boolean, data: Object, summary: string }
 *
 * Each checker receives (commerce, config) and is resilient to errors.
 * No external dependencies beyond the commerce object.
 */

// ============================================================================
// Individual Checkers
// ============================================================================

/**
 * Check for low-stock items.
 *
 * @param {Object} commerce - StateSet Commerce instance
 * @param {{ threshold?: number }} config
 * @returns {Promise<{ triggered: boolean, data: Object, summary: string }>}
 */
async function lowStock(commerce, config = {}) {
  const threshold = config.threshold ?? 10;
  try {
    const items = await commerce.analytics.lowStockItems(threshold);
    const list = Array.isArray(items) ? items : [];
    if (list.length > 0) {
      return {
        triggered: true,
        data: { items: list, threshold },
        summary: `${list.length} item(s) below ${threshold} units`,
      };
    }
    return { triggered: false, data: { threshold }, summary: 'All stock levels OK' };
  } catch (err) {
    return {
      triggered: false,
      data: { error: err.message },
      summary: `Low-stock check failed: ${err.message}`,
    };
  }
}

/**
 * Check for abandoned carts older than a given age.
 *
 * @param {Object} commerce
 * @param {{ minAgeHours?: number }} config
 */
async function abandonedCarts(commerce, config = {}) {
  const minAgeHours = config.minAgeHours ?? 24;
  try {
    const carts = await commerce.carts.getAbandoned();
    const list = Array.isArray(carts) ? carts : [];
    const cutoff = Date.now() - minAgeHours * 3600_000;
    const old = list.filter((c) => {
      const ts = c.updatedAt || c.updated_at || c.createdAt || c.created_at;
      return ts && new Date(ts).getTime() < cutoff;
    });
    if (old.length > 0) {
      return {
        triggered: true,
        data: { carts: old, minAgeHours },
        summary: `${old.length} abandoned cart(s) older than ${minAgeHours}h`,
      };
    }
    return { triggered: false, data: { minAgeHours }, summary: 'No stale abandoned carts' };
  } catch (err) {
    return {
      triggered: false,
      data: { error: err.message },
      summary: `Abandoned-cart check failed: ${err.message}`,
    };
  }
}

/**
 * Check if revenue has hit a milestone.
 *
 * @param {Object} commerce
 * @param {{ target?: number, period?: string }} config
 */
async function revenueMilestone(commerce, config = {}) {
  const target = config.target ?? 10000;
  const period = config.period ?? 'month';
  try {
    const summary = await commerce.analytics.salesSummary({ period });
    const revenue = summary?.totalRevenue ?? summary?.total_revenue ?? 0;
    if (revenue >= target) {
      return {
        triggered: true,
        data: { revenue, target, period },
        summary: `Revenue milestone reached: $${revenue.toLocaleString()} (target: $${target.toLocaleString()})`,
      };
    }
    return {
      triggered: false,
      data: { revenue, target, period },
      summary: `Revenue $${revenue.toLocaleString()} / $${target.toLocaleString()} (${period})`,
    };
  } catch (err) {
    return {
      triggered: false,
      data: { error: err.message },
      summary: `Revenue check failed: ${err.message}`,
    };
  }
}

/**
 * Check for pending returns older than a threshold.
 *
 * @param {Object} commerce
 * @param {{ maxAgeDays?: number }} config
 */
async function pendingReturns(commerce, config = {}) {
  const maxAgeDays = config.maxAgeDays ?? 7;
  try {
    const returns = await commerce.returns.list();
    const list = Array.isArray(returns) ? returns : [];
    const cutoff = Date.now() - maxAgeDays * 86400_000;
    const old = list.filter((r) => {
      const status = r.status || '';
      if (status !== 'pending' && status !== 'requested') return false;
      const ts = r.createdAt || r.created_at;
      return ts && new Date(ts).getTime() < cutoff;
    });
    if (old.length > 0) {
      return {
        triggered: true,
        data: { returns: old, maxAgeDays },
        summary: `${old.length} pending return(s) older than ${maxAgeDays} days`,
      };
    }
    return { triggered: false, data: { maxAgeDays }, summary: 'No overdue pending returns' };
  } catch (err) {
    return {
      triggered: false,
      data: { error: err.message },
      summary: `Pending-returns check failed: ${err.message}`,
    };
  }
}

/**
 * Check for overdue invoices.
 *
 * @param {Object} commerce
 * @param {Object} _config - reserved
 */
async function overdueInvoices(commerce, _config = {}) {
  try {
    const invoices = await commerce.invoices.getOverdue();
    const list = Array.isArray(invoices) ? invoices : [];
    if (list.length > 0) {
      const total = list.reduce((sum, inv) => sum + (inv.amountDue ?? inv.amount_due ?? 0), 0);
      return {
        triggered: true,
        data: { invoices: list, totalOverdue: total },
        summary: `${list.length} overdue invoice(s) totalling $${total.toLocaleString()}`,
      };
    }
    return { triggered: false, data: {}, summary: 'No overdue invoices' };
  } catch (err) {
    return {
      triggered: false,
      data: { error: err.message },
      summary: `Overdue-invoice check failed: ${err.message}`,
    };
  }
}

/**
 * Check for subscription churn (cancelled or past-due).
 *
 * @param {Object} commerce
 * @param {Object} _config - reserved
 */
async function subscriptionChurn(commerce, _config = {}) {
  try {
    const cancelled = await commerce.listSubscriptions({ status: 'cancelled' });
    const pastDue = await commerce.listSubscriptions({ status: 'past_due' });
    const cancelledList = Array.isArray(cancelled) ? cancelled : [];
    const pastDueList = Array.isArray(pastDue) ? pastDue : [];
    const total = cancelledList.length + pastDueList.length;
    if (total > 0) {
      return {
        triggered: true,
        data: { cancelled: cancelledList, pastDue: pastDueList },
        summary: `Subscription churn: ${cancelledList.length} cancelled, ${pastDueList.length} past-due`,
      };
    }
    return { triggered: false, data: {}, summary: 'No subscription churn detected' };
  } catch (err) {
    return {
      triggered: false,
      data: { error: err.message },
      summary: `Churn check failed: ${err.message}`,
    };
  }
}

// ============================================================================
// Registry
// ============================================================================

/**
 * Built-in checkers keyed by ID.
 * @type {Object<string, (commerce: Object, config: Object) => Promise<{ triggered: boolean, data: Object, summary: string }>>}
 */
export const BUILTIN_CHECKERS = {
  'low-stock': lowStock,
  'abandoned-carts': abandonedCarts,
  'revenue-milestone': revenueMilestone,
  'pending-returns': pendingReturns,
  'overdue-invoices': overdueInvoices,
  'subscription-churn': subscriptionChurn,
};

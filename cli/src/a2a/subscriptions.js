/**
 * A2A Subscription Service
 *
 * Manages recurring payment subscriptions between agents.
 * Supports trial periods, pause/resume, graceful cancellation,
 * and automated billing cycle processing.
 *
 * Status Flow:
 *   trial   -> active (trial expires or manually activated)
 *   active  -> paused  (subscriber request)
 *   active  -> cancelled (immediate or at period end)
 *   paused  -> active  (resume)
 *   paused  -> cancelled
 *
 * @example
 * ```javascript
 * const subs = createA2ASubscriptionService(store);
 *
 * // Create a monthly subscription with a 14-day trial
 * const result = await subs.createSubscription({
 *   subscriberAddress: '0xSubscriber',
 *   providerAddress: '0xProvider',
 *   planName: 'Pro Plan',
 *   amount: 49.99,
 *   billingInterval: 'monthly',
 *   trialDays: 14,
 * });
 *
 * // Pause / resume
 * await subs.pauseSubscription(result.subscription.id);
 * await subs.resumeSubscription(result.subscription.id);
 *
 * // Cancel at end of current period
 * await subs.cancelSubscription(result.subscription.id, { immediate: false });
 *
 * // Process all due billing
 * const billing = await subs.processBilling();
 * ```
 */

import { randomUUID } from 'node:crypto';
import {
  DEFAULT_NETWORK,
  getAssetDecimals,
  getDefaultAssetForNetwork,
  toSmallestUnit,
} from './assets.js';

// Allowed billing intervals
const VALID_INTERVALS = ['weekly', 'biweekly', 'monthly', 'quarterly', 'annual'];

// Default configuration
const DEFAULT_MAX_PAST_DUE_CYCLES = 3;

/**
 * Compute the next billing date from a given start date and interval.
 *
 * @param {Date|string} fromDate - Start date (Date object or ISO string)
 * @param {string} interval - One of: weekly, biweekly, monthly, quarterly, annual
 * @returns {string} ISO 8601 date string of the next billing date
 */
function computeNextBillingDate(fromDate, interval) {
  const date = new Date(fromDate);

  switch (interval) {
    case 'weekly':
      date.setDate(date.getDate() + 7);
      break;
    case 'biweekly':
      date.setDate(date.getDate() + 14);
      break;
    case 'monthly':
      date.setMonth(date.getMonth() + 1);
      break;
    case 'quarterly':
      date.setMonth(date.getMonth() + 3);
      break;
    case 'annual':
      date.setFullYear(date.getFullYear() + 1);
      break;
    default:
      throw new Error(`Invalid billing interval: ${interval}`);
  }

  return date.toISOString();
}

/**
 * Format a subscription row from snake_case store format to camelCase.
 *
 * @param {Object} row - Raw subscription record from the store
 * @returns {Object} Formatted subscription with camelCase keys
 */
function formatSubscription(row) {
  if (!row) return null;

  return {
    id: row.id,
    subscriberAddress: row.subscriber_address,
    providerAddress: row.provider_address,
    serviceId: row.service_id || null,
    planName: row.plan_name,
    status: row.status,
    amount: row.amount,
    amountDecimal: row.amount_decimal,
    asset: row.asset,
    network: row.network,
    billingInterval: row.billing_interval,
    trialEndDate: row.trial_end_date || null,
    currentPeriodStart: row.current_period_start,
    currentPeriodEnd: row.current_period_end,
    nextBillingDate: row.next_billing_date,
    cancelAtPeriodEnd: Boolean(row.cancel_at_period_end),
    cancelledAt: row.cancelled_at || null,
    pastDueSince: row.past_due_since || null,
    maxPastDueCycles: row.max_past_due_cycles,
    totalBilled: row.total_billed,
    totalBilledDecimal: row.total_billed_decimal,
    billingCount: row.billing_count,
    lastPaymentId: row.last_payment_id || null,
    metadata: row.metadata || null,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

/**
 * Create an A2A Subscription Service instance
 *
 * @param {Object} store - A2A store with subscription methods
 * @param {Function} store.createSubscription - Persist a new subscription record
 * @param {Function} store.getSubscription - Retrieve subscription by ID
 * @param {Function} store.updateSubscription - Update subscription fields by ID
 * @param {Function} store.listSubscriptions - List subscriptions with optional filter
 * @param {Function} store.getDueSubscriptions - Get subscriptions due for billing
 * @param {Function} store.getExpiredTrials - Get trial subscriptions past their end date
 * @returns {Object} Subscription service API
 */
export function createA2ASubscriptionService(store) {
  /**
   * Create a new recurring subscription between two agents
   *
   * @param {Object} params - Subscription parameters
   * @param {string} params.subscriberAddress - Subscriber wallet address
   * @param {string} params.providerAddress - Provider wallet address
   * @param {string} [params.serviceId] - Associated service ID
   * @param {string} params.planName - Human-readable plan name
   * @param {number} params.amount - Amount per billing cycle (decimal, e.g. 49.99)
   * @param {string} [params.asset] - Asset type (default: USDC)
   * @param {string} [params.network] - Settlement network (default: set_chain)
   * @param {string} [params.billingInterval] - Billing interval (default: monthly)
   * @param {number} [params.trialDays] - Trial period in days (0 = no trial)
   * @param {number} [params.maxPastDueCycles] - Max past-due cycles before cancellation
   * @param {Object} [params.metadata] - Additional metadata
   * @returns {Promise<Object>} Created subscription result
   */
  async function createSubscription(params) {
    const {
      subscriberAddress,
      providerAddress,
      serviceId,
      planName,
      amount,
      network = DEFAULT_NETWORK,
      asset: requestedAsset = null,
      billingInterval = 'monthly',
      trialDays = 0,
      maxPastDueCycles = DEFAULT_MAX_PAST_DUE_CYCLES,
      metadata,
    } = params;

    const asset = requestedAsset || getDefaultAssetForNetwork(network);

    // Validate required fields
    if (!subscriberAddress) {
      throw new Error('subscriberAddress is required');
    }
    if (!providerAddress) {
      throw new Error('providerAddress is required');
    }
    if (!planName) {
      throw new Error('planName is required');
    }
    if (amount === undefined || amount === null) {
      throw new Error('amount is required');
    }
    if (typeof amount !== 'number' || amount <= 0) {
      throw new Error('amount must be a positive number');
    }

    // Validate billing interval
    if (!VALID_INTERVALS.includes(billingInterval)) {
      throw new Error(
        `Invalid billingInterval: ${billingInterval}. ` +
          `Must be one of: ${VALID_INTERVALS.join(', ')}`,
      );
    }

    const now = new Date();
    const nowIso = now.toISOString();

    const amountSmallest = toSmallestUnit(amount, getAssetDecimals(asset));

    let status;
    let trialEndDate = null;
    let nextBillingDate;
    let currentPeriodStart;
    let currentPeriodEnd;

    if (trialDays > 0) {
      // Trial period: billing starts after trial
      status = 'trial';
      const trialEnd = new Date(now.getTime() + trialDays * 24 * 60 * 60 * 1000);
      trialEndDate = trialEnd.toISOString();
      nextBillingDate = trialEndDate;
      currentPeriodStart = nowIso;
      currentPeriodEnd = trialEndDate;
    } else {
      // No trial: immediately active
      status = 'active';
      nextBillingDate = computeNextBillingDate(now, billingInterval);
      currentPeriodStart = nowIso;
      currentPeriodEnd = nextBillingDate;
    }

    const subscriptionId = randomUUID();
    const record = {
      id: subscriptionId,
      subscriber_address: subscriberAddress,
      provider_address: providerAddress,
      service_id: serviceId || null,
      plan_name: planName,
      status,
      amount: amountSmallest,
      amount_decimal: amount,
      asset: asset.toUpperCase(),
      network,
      billing_interval: billingInterval,
      trial_end_date: trialEndDate,
      current_period_start: currentPeriodStart,
      current_period_end: currentPeriodEnd,
      next_billing_date: nextBillingDate,
      cancel_at_period_end: false,
      cancelled_at: null,
      past_due_since: null,
      max_past_due_cycles: maxPastDueCycles,
      total_billed: 0,
      total_billed_decimal: 0,
      billing_count: 0,
      last_payment_id: null,
      metadata: metadata ? JSON.stringify(metadata) : null,
      created_at: nowIso,
      updated_at: nowIso,
    };

    const stored = await store.createSubscription(record);

    return {
      success: true,
      subscription: formatSubscription(stored || record),
    };
  }

  /**
   * Pause an active subscription
   *
   * Billing is suspended until the subscription is resumed.
   *
   * @param {string} subscriptionId - Subscription ID
   * @returns {Promise<Object>} Updated subscription result
   */
  async function pauseSubscription(subscriptionId) {
    const sub = await store.getSubscription(subscriptionId);
    if (!sub) {
      throw new Error('Subscription not found');
    }

    if (sub.status !== 'active') {
      throw new Error(
        `Cannot pause subscription in status: ${sub.status}. Only active subscriptions can be paused.`,
      );
    }

    const updated = await store.updateSubscription(subscriptionId, {
      status: 'paused',
    });

    return {
      success: true,
      subscription: formatSubscription(updated),
    };
  }

  /**
   * Resume a paused subscription
   *
   * Recalculates billing dates from the current time.
   *
   * @param {string} subscriptionId - Subscription ID
   * @returns {Promise<Object>} Updated subscription result
   */
  async function resumeSubscription(subscriptionId) {
    const sub = await store.getSubscription(subscriptionId);
    if (!sub) {
      throw new Error('Subscription not found');
    }

    if (sub.status !== 'paused') {
      throw new Error(
        `Cannot resume subscription in status: ${sub.status}. Only paused subscriptions can be resumed.`,
      );
    }

    const now = new Date();
    const nowIso = now.toISOString();
    const nextBilling = computeNextBillingDate(now, sub.billing_interval);

    const updated = await store.updateSubscription(subscriptionId, {
      status: 'active',
      current_period_start: nowIso,
      current_period_end: nextBilling,
      next_billing_date: nextBilling,
    });

    return {
      success: true,
      subscription: formatSubscription(updated),
    };
  }

  /**
   * Cancel a subscription
   *
   * Supports immediate cancellation or cancellation at the end of the
   * current billing period (graceful).
   *
   * @param {string} subscriptionId - Subscription ID
   * @param {Object} [options] - Cancellation options
   * @param {boolean} [options.immediate=true] - Cancel immediately or at period end
   * @returns {Promise<Object>} Updated subscription result
   */
  async function cancelSubscription(subscriptionId, { immediate = true } = {}) {
    const sub = await store.getSubscription(subscriptionId);
    if (!sub) {
      throw new Error('Subscription not found');
    }

    if (sub.status === 'cancelled') {
      throw new Error('Subscription is already cancelled');
    }

    const now = new Date().toISOString();
    let updates;

    if (immediate) {
      updates = {
        status: 'cancelled',
        cancelled_at: now,
        cancel_at_period_end: false,
      };
    } else {
      // Remain active until the current period ends
      updates = {
        cancel_at_period_end: true,
      };
    }

    const updated = await store.updateSubscription(subscriptionId, updates);

    return {
      success: true,
      subscription: formatSubscription(updated),
    };
  }

  /**
   * Get a single subscription by ID
   *
   * @param {string} subscriptionId - Subscription ID
   * @returns {Promise<Object>} Formatted subscription
   * @throws {Error} If subscription is not found
   */
  async function getSubscription(subscriptionId) {
    const sub = await store.getSubscription(subscriptionId);
    if (!sub) {
      throw new Error('Subscription not found');
    }
    return formatSubscription(sub);
  }

  /**
   * List subscriptions with optional filtering
   *
   * Accepts camelCase filter keys and converts to snake_case for the store.
   *
   * @param {Object} [filter] - Filter options
   * @param {string} [filter.subscriberAddress] - Filter by subscriber
   * @param {string} [filter.providerAddress] - Filter by provider
   * @param {string} [filter.status] - Filter by status
   * @param {string} [filter.serviceId] - Filter by service
   * @param {number} [filter.limit] - Max results
   * @param {number} [filter.offset] - Pagination offset
   * @returns {Promise<Array>} Formatted subscription list
   */
  async function listSubscriptions(filter = {}) {
    // Convert camelCase filter keys to snake_case
    const storeFilter = {};

    if (filter.subscriberAddress) {
      storeFilter.subscriber_address = filter.subscriberAddress;
    }
    if (filter.providerAddress) {
      storeFilter.provider_address = filter.providerAddress;
    }
    if (filter.status) {
      storeFilter.status = filter.status;
    }
    if (filter.serviceId) {
      storeFilter.service_id = filter.serviceId;
    }
    if (filter.limit !== undefined) {
      storeFilter.limit = filter.limit;
    }
    if (filter.offset !== undefined) {
      storeFilter.offset = filter.offset;
    }

    // Pass through any snake_case keys directly
    if (filter.subscriber_address) {
      storeFilter.subscriber_address = filter.subscriber_address;
    }
    if (filter.provider_address) {
      storeFilter.provider_address = filter.provider_address;
    }
    if (filter.service_id) {
      storeFilter.service_id = filter.service_id;
    }

    const subs = await store.listSubscriptions(storeFilter);
    return subs.map(formatSubscription);
  }

  /**
   * Process all due subscription billing and expired trials
   *
   * Iterates over subscriptions whose next_billing_date has passed:
   *   1. If cancel_at_period_end is true and the period has ended, cancels the sub
   *   2. Otherwise, records a billing event and advances the billing window
   *
   * Also transitions expired trials to active status.
   *
   * @returns {Promise<Object>} Processing summary with counts
   */
  async function processBilling() {
    const now = new Date();
    const nowIso = now.toISOString();

    let processed = 0;
    let succeeded = 0;
    let failed = 0;
    let cancelled = 0;

    // 1. Process due active subscriptions
    const dueSubscriptions = await store.getDueSubscriptions(nowIso);

    for (const sub of dueSubscriptions) {
      processed++;

      try {
        // Check if this subscription should be cancelled at period end
        if (
          sub.cancel_at_period_end &&
          sub.current_period_end &&
          new Date(sub.current_period_end) <= now
        ) {
          await store.updateSubscription(sub.id, {
            status: 'cancelled',
            cancelled_at: nowIso,
            cancel_at_period_end: false,
          });
          cancelled++;
          continue;
        }

        // Record billing: advance totals and billing window
        const newTotalBilled = (sub.total_billed || 0) + sub.amount;
        const newTotalBilledDecimal = (sub.total_billed_decimal || 0) + (sub.amount_decimal || 0);
        const newBillingCount = (sub.billing_count || 0) + 1;
        const paymentId = randomUUID();
        const nextBilling = computeNextBillingDate(now, sub.billing_interval);

        await store.updateSubscription(sub.id, {
          total_billed: newTotalBilled,
          total_billed_decimal: newTotalBilledDecimal,
          billing_count: newBillingCount,
          last_payment_id: paymentId,
          current_period_start: nowIso,
          current_period_end: nextBilling,
          next_billing_date: nextBilling,
        });

        succeeded++;
      } catch (err) {
        console.warn(`Failed to process billing for subscription ${sub.id}:`, err.message);
        failed++;
      }
    }

    // 2. Transition expired trials to active
    const expiredTrials = await store.getExpiredTrials(nowIso);

    for (const trial of expiredTrials) {
      processed++;

      try {
        const nextBilling = computeNextBillingDate(now, trial.billing_interval);

        await store.updateSubscription(trial.id, {
          status: 'active',
          current_period_start: nowIso,
          current_period_end: nextBilling,
          next_billing_date: nextBilling,
        });

        succeeded++;
      } catch (err) {
        console.warn(`Failed to transition trial subscription ${trial.id}:`, err.message);
        failed++;
      }
    }

    return { processed, succeeded, failed, cancelled };
  }

  return {
    // Core subscription operations
    createSubscription,
    pauseSubscription,
    resumeSubscription,
    cancelSubscription,

    // Query operations
    getSubscription,
    listSubscriptions,

    // Billing processor
    processBilling,
  };
}

// Exported for testing
export { computeNextBillingDate, formatSubscription, VALID_INTERVALS };

export default { createA2ASubscriptionService };

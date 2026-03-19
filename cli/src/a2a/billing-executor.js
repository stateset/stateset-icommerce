/**
 * A2A Billing Executor — Autonomous Subscription Billing Engine
 *
 * Runs on a configurable interval to:
 *   1. Find subscriptions due for billing
 *   2. Execute actual A2A payments for each
 *   3. Track past-due cycles with dunning notifications
 *   4. Auto-cancel after maxPastDueCycles exceeded
 *   5. Transition expired trials to active
 *
 * @example
 * ```javascript
 * const executor = createBillingExecutor(store, a2aService, notifications, {
 *   intervalMs: 60_000,  // check every minute
 * });
 * executor.start();
 * // ... later
 * executor.stop();
 * ```
 */

import { randomUUID } from 'node:crypto';
import { EventEmitter } from 'node:events';
import { DEFAULT_NETWORK, getDefaultAssetForNetwork } from './assets.js';

const INTERVAL_DAYS = {
  weekly: 7,
  biweekly: 14,
  monthly: 30,
  quarterly: 90,
  annual: 365,
};

/**
 * Compute the next billing date from a given date and interval.
 * @param {Date} fromDate
 * @param {string} interval
 * @returns {string} ISO 8601 date string
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
 * Count how many billing cycles have elapsed since a date.
 * @param {string} sinceIso - ISO date when past_due started
 * @param {string} interval - billing interval
 * @returns {number}
 */
function countPastDueCycles(sinceIso, interval) {
  const since = new Date(sinceIso).getTime();
  const now = Date.now();
  const daysPassed = (now - since) / (24 * 60 * 60 * 1000);
  const intervalDays = INTERVAL_DAYS[interval] || 30;
  return Math.floor(daysPassed / intervalDays);
}

/**
 * Create a billing executor instance.
 *
 * @param {Object} store - A2A store
 * @param {Object} a2aService - A2A service for executing payments
 * @param {Object} [notificationService] - Notification service for dunning
 * @param {Object} [options]
 * @param {number} [options.intervalMs=60000] - Polling interval
 * @param {number} [options.maxRetries=3] - Max payment retries per cycle
 * @returns {Object} Billing executor API
 */
export function createBillingExecutor(store, a2aService, notificationService, options = {}) {
  const { intervalMs = 60_000, maxRetries = 3 } = options;
  const emitter = new EventEmitter();
  let _timer = null;
  let _running = false;
  let _tickInProgress = false;

  // Metrics
  const _metrics = {
    totalTicks: 0,
    totalBilled: 0,
    totalFailed: 0,
    totalCancelled: 0,
    totalTrialsActivated: 0,
    totalDunningsSent: 0,
    lastTickAt: null,
    lastTickDurationMs: 0,
  };

  /**
   * Execute one billing cycle.
   * @returns {Promise<Object>} Tick result summary
   */
  async function tick() {
    if (_tickInProgress) {
      return { skipped: true, reason: 'previous tick still running' };
    }

    _tickInProgress = true;
    const startTime = Date.now();
    const now = new Date();
    const nowIso = now.toISOString();

    let billed = 0;
    let failed = 0;
    let cancelled = 0;
    let trialsActivated = 0;
    let dunningsSent = 0;

    try {
      // 1. Process expired trials → active
      const expiredTrials = await store.getExpiredTrials(nowIso);
      for (const trial of expiredTrials) {
        try {
          const nextBilling = computeNextBillingDate(now, trial.billing_interval);
          await store.updateSubscription(trial.id, {
            status: 'active',
            current_period_start: nowIso,
            current_period_end: nextBilling,
            next_billing_date: nextBilling,
          });
          trialsActivated++;
          emitter.emit('trial_activated', { subscriptionId: trial.id });
        } catch (err) {
          console.warn(`[billing-executor] Failed to activate trial ${trial.id}:`, err.message);
        }
      }

      // 2. Process due subscriptions
      const dueSubscriptions = await store.getDueSubscriptions(nowIso);

      for (const sub of dueSubscriptions) {
        // 2a. Cancel if cancel_at_period_end and period has ended
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
          emitter.emit('subscription_cancelled', {
            subscriptionId: sub.id,
            reason: 'cancel_at_period_end',
          });
          continue;
        }

        // 2b. Check past-due cycle count — auto-cancel if exceeded
        if (sub.past_due_since) {
          const pastDueCycles = countPastDueCycles(sub.past_due_since, sub.billing_interval);
          const maxCycles = sub.max_past_due_cycles || 3;
          if (pastDueCycles >= maxCycles) {
            await store.updateSubscription(sub.id, {
              status: 'cancelled',
              cancelled_at: nowIso,
            });
            cancelled++;
            emitter.emit('subscription_cancelled', {
              subscriptionId: sub.id,
              reason: 'max_past_due_cycles_exceeded',
              pastDueCycles,
            });

            // Send final cancellation notification
            if (notificationService) {
              try {
                await notificationService.sendNotification({
                  recipientAddress: sub.subscriber_address,
                  eventType: 'subscription.cancelled',
                  payload: {
                    subscriptionId: sub.id,
                    planName: sub.plan_name,
                    reason: 'Payment failed after maximum retry attempts',
                    pastDueCycles,
                  },
                });
              } catch (_notifErr) {
                // Best effort
              }
            }
            continue;
          }
        }

        // 2c. Execute the actual payment
        let paymentSucceeded = false;
        let paymentId = null;
        let lastError = null;

        for (let attempt = 1; attempt <= maxRetries; attempt++) {
          try {
            const payResult = await a2aService.pay({
              to: sub.provider_address,
              amount: sub.amount_decimal,
              asset: sub.asset || getDefaultAssetForNetwork(sub.network || DEFAULT_NETWORK),
              network: sub.network || DEFAULT_NETWORK,
              memo: `Subscription billing: ${sub.plan_name} (${sub.billing_interval})`,
              idempotencyKey: `sub-${sub.id}-${nowIso}`,
            });

            if (payResult && payResult.success !== false) {
              paymentSucceeded = true;
              paymentId = payResult.payment?.id || randomUUID();
              break;
            } else {
              lastError = payResult?.error || 'Payment returned unsuccessful';
            }
          } catch (err) {
            lastError = err.message;
            // Wait before retry with exponential backoff
            if (attempt < maxRetries) {
              await new Promise((r) => setTimeout(r, Math.min(1000 * Math.pow(2, attempt), 10000)));
            }
          }
        }

        if (paymentSucceeded) {
          // Success — advance billing window
          const newTotalBilled = (sub.total_billed || 0) + sub.amount;
          const newTotalBilledDecimal = (sub.total_billed_decimal || 0) + (sub.amount_decimal || 0);
          const newBillingCount = (sub.billing_count || 0) + 1;
          const nextBilling = computeNextBillingDate(now, sub.billing_interval);

          await store.updateSubscription(sub.id, {
            total_billed: newTotalBilled,
            total_billed_decimal: newTotalBilledDecimal,
            billing_count: newBillingCount,
            last_payment_id: paymentId,
            current_period_start: nowIso,
            current_period_end: nextBilling,
            next_billing_date: nextBilling,
            past_due_since: null, // Clear past-due on success
          });

          billed++;
          emitter.emit('billing_succeeded', {
            subscriptionId: sub.id,
            paymentId,
            amount: sub.amount_decimal,
          });
        } else {
          // Failed — mark past_due and send dunning notification
          failed++;

          const updates = {};
          if (!sub.past_due_since) {
            updates.past_due_since = nowIso;
          }
          // Advance next_billing_date so we don't re-process immediately
          updates.next_billing_date = computeNextBillingDate(now, sub.billing_interval);

          if (Object.keys(updates).length > 0) {
            await store.updateSubscription(sub.id, updates);
          }

          emitter.emit('billing_failed', {
            subscriptionId: sub.id,
            error: lastError,
            pastDueSince: sub.past_due_since || nowIso,
          });

          // Send dunning notification
          if (notificationService) {
            try {
              await notificationService.sendNotification({
                recipientAddress: sub.subscriber_address,
                eventType: 'subscription.payment_failed',
                payload: {
                  subscriptionId: sub.id,
                  planName: sub.plan_name,
                  amount: sub.amount_decimal,
                  asset: sub.asset,
                  error: lastError,
                  pastDueSince: sub.past_due_since || nowIso,
                },
              });
              dunningsSent++;
            } catch (_notifErr) {
              // Best effort
            }
          }
        }
      }
    } finally {
      _tickInProgress = false;
    }

    const duration = Date.now() - startTime;
    _metrics.totalTicks++;
    _metrics.totalBilled += billed;
    _metrics.totalFailed += failed;
    _metrics.totalCancelled += cancelled;
    _metrics.totalTrialsActivated += trialsActivated;
    _metrics.totalDunningsSent += dunningsSent;
    _metrics.lastTickAt = nowIso;
    _metrics.lastTickDurationMs = duration;

    const result = {
      billed,
      failed,
      cancelled,
      trialsActivated,
      dunningsSent,
      durationMs: duration,
    };

    emitter.emit('tick_complete', result);
    return result;
  }

  /**
   * Start the billing executor loop.
   */
  function start() {
    if (_running) return;
    _running = true;
    _timer = setInterval(() => {
      tick().catch((err) => {
        console.error('[billing-executor] Tick failed:', err.message);
        emitter.emit('tick_error', err);
      });
    }, intervalMs);
    if (_timer.unref) _timer.unref();
    emitter.emit('started');
  }

  /**
   * Stop the billing executor loop.
   */
  function stop() {
    if (!_running) return;
    _running = false;
    if (_timer) {
      clearInterval(_timer);
      _timer = null;
    }
    emitter.emit('stopped');
  }

  /** Get executor metrics. */
  function getMetrics() {
    return { ..._metrics, running: _running, intervalMs };
  }

  return {
    tick,
    start,
    stop,
    getMetrics,
    on: emitter.on.bind(emitter),
    off: emitter.removeListener.bind(emitter),
  };
}

export default { createBillingExecutor };

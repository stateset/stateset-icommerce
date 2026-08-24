/**
 * A2A Store — recurring A2A subscriptions.
 *
 * Extracted verbatim from `cli/src/a2a/store.js` (Aug 2026 decomposition).
 */

import { randomUUID } from 'node:crypto';

/**
 * A2A Store — recurring A2A subscriptions.
 *
 * Methods are copied verbatim onto `A2AStore.prototype` by `applyStoreMixins()`
 * in `../store.js`; this class is never instantiated on its own.
 * Every method runs with `this` bound to the owning A2AStore (`this.db` is the
 * open better-sqlite3 handle).
 *
 * @this {import('../store.js').A2AStore}
 */
export class A2ASubscriptionsMethods {
  // ===========================================================================
  // A2A Subscriptions
  // ===========================================================================

  createSubscription(sub) {
    this.init();
    const id = sub.id || randomUUID();
    const now = new Date().toISOString();

    this.db
      .prepare(
        `INSERT INTO a2a_subscriptions (
        id, subscriber_address, provider_address, service_id, plan_name,
        status, amount, amount_decimal, asset, network, billing_interval,
        trial_end_date, current_period_start, current_period_end,
        next_billing_date, cancel_at_period_end, cancelled_at, past_due_since,
        max_past_due_cycles, total_billed, total_billed_decimal, billing_count,
        last_payment_id, metadata, created_at, updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        id,
        sub.subscriber_address,
        sub.provider_address,
        sub.service_id || null,
        sub.plan_name,
        sub.status || 'active',
        sub.amount,
        sub.amount_decimal,
        sub.asset || 'USDC',
        sub.network || 'set_chain',
        sub.billing_interval || 'monthly',
        sub.trial_end_date || null,
        sub.current_period_start || now,
        sub.current_period_end,
        sub.next_billing_date,
        sub.cancel_at_period_end ? 1 : 0,
        sub.cancelled_at || null,
        sub.past_due_since || null,
        sub.max_past_due_cycles ?? 3,
        sub.total_billed || 0,
        sub.total_billed_decimal || 0,
        sub.billing_count || 0,
        sub.last_payment_id || null,
        sub.metadata || null,
        sub.created_at || now,
        sub.updated_at || now,
      );

    return this.getSubscription(id);
  }

  getSubscription(id) {
    this.init();
    const row = this.db.prepare('SELECT * FROM a2a_subscriptions WHERE id = ?').get(id);
    return row ? this._mapSubscription(row) : null;
  }

  updateSubscription(id, updates) {
    this.init();
    this._validateUpdateKeys('a2a_subscriptions', Object.keys(updates));
    const fields = [];
    const values = [];

    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        if (key === 'cancel_at_period_end') {
          fields.push(`${key} = ?`);
          values.push(value ? 1 : 0);
        } else {
          fields.push(`${key} = ?`);
          values.push(value);
        }
      }
    }

    if (fields.length === 0) return this.getSubscription(id);

    fields.push('updated_at = ?');
    values.push(new Date().toISOString());
    values.push(id);

    this.db
      .prepare(`UPDATE a2a_subscriptions SET ${fields.join(', ')} WHERE id = ?`)
      .run(...values);
    return this.getSubscription(id);
  }

  listSubscriptions(filter = {}) {
    this.init();
    const conditions = [];
    const params = [];

    if (filter.subscriber_address) {
      conditions.push('subscriber_address = ?');
      params.push(filter.subscriber_address);
    }
    if (filter.provider_address) {
      conditions.push('provider_address = ?');
      params.push(filter.provider_address);
    }
    if (filter.status) {
      conditions.push('status = ?');
      params.push(filter.status);
    }
    if (filter.service_id) {
      conditions.push('service_id = ?');
      params.push(filter.service_id);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const limit = filter.limit || 50;
    const offset = filter.offset || 0;

    const rows = this.db
      .prepare(`SELECT * FROM a2a_subscriptions ${where} ORDER BY created_at DESC LIMIT ? OFFSET ?`)
      .all(...params, limit, offset);

    return rows.map(this._mapSubscription);
  }

  getDueSubscriptions(now, limit = 50) {
    this.init();
    return this.db
      .prepare(
        `SELECT * FROM a2a_subscriptions
       WHERE status = 'active' AND next_billing_date <= ?
       ORDER BY next_billing_date ASC LIMIT ?`,
      )
      .all(now, limit)
      .map(this._mapSubscription);
  }

  getExpiredTrials(now) {
    this.init();
    return this.db
      .prepare(
        `SELECT * FROM a2a_subscriptions
       WHERE status = 'trial' AND trial_end_date IS NOT NULL AND trial_end_date <= ?`,
      )
      .all(now)
      .map(this._mapSubscription);
  }

  _mapSubscription(row) {
    return {
      ...row,
      cancel_at_period_end: Boolean(row.cancel_at_period_end),
    };
  }
}

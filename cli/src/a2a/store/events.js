/**
 * A2A Store — event subscriptions and event log.
 *
 * Extracted verbatim from `cli/src/a2a/store.js` (Aug 2026 decomposition).
 */

import { randomUUID } from 'node:crypto';

/**
 * A2A Store — event subscriptions and event log.
 *
 * Methods are copied verbatim onto `A2AStore.prototype` by `applyStoreMixins()`
 * in `../store.js`; this class is never instantiated on its own.
 * Every method runs with `this` bound to the owning A2AStore (`this.db` is the
 * open better-sqlite3 handle).
 *
 * @this {import('../store.js').A2AStore}
 */
export class A2AEventsMethods {
  // ===========================================================================
  // Event Subscriptions
  // ===========================================================================

  createEventSubscription(sub) {
    this.init();
    const id = sub.id || randomUUID();
    const now = new Date().toISOString();
    const eventTypes = Array.isArray(sub.event_types)
      ? JSON.stringify(sub.event_types)
      : sub.event_types || '["*"]';

    this.db
      .prepare(
        `INSERT INTO a2a_event_subscriptions (
        id, agent_address, event_types, active, last_event_id, created_at, updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        id,
        sub.agent_address,
        eventTypes,
        sub.active !== undefined ? (sub.active ? 1 : 0) : 1,
        sub.last_event_id || null,
        sub.created_at || now,
        sub.updated_at || now,
      );

    return this.getEventSubscription(id);
  }

  getEventSubscription(id) {
    this.init();
    const row = this.db.prepare('SELECT * FROM a2a_event_subscriptions WHERE id = ?').get(id);
    return row ? this._mapEventSubscription(row) : null;
  }

  updateEventSubscription(id, updates) {
    this.init();
    this._validateUpdateKeys('a2a_event_subscriptions', Object.keys(updates));
    const fields = [];
    const values = [];

    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        if (key === 'event_types' && Array.isArray(value)) {
          fields.push(`${key} = ?`);
          values.push(JSON.stringify(value));
        } else if (key === 'active') {
          fields.push(`${key} = ?`);
          values.push(value ? 1 : 0);
        } else {
          fields.push(`${key} = ?`);
          values.push(value);
        }
      }
    }

    if (fields.length === 0) return this.getEventSubscription(id);

    fields.push('updated_at = ?');
    values.push(new Date().toISOString());
    values.push(id);

    this.db
      .prepare(`UPDATE a2a_event_subscriptions SET ${fields.join(', ')} WHERE id = ?`)
      .run(...values);
    return this.getEventSubscription(id);
  }

  listEventSubscriptions(filter = {}) {
    this.init();
    const conditions = [];
    const params = [];

    if (filter.agent_address) {
      conditions.push('agent_address = ?');
      params.push(filter.agent_address);
    }
    if (filter.active !== undefined) {
      conditions.push('active = ?');
      params.push(filter.active ? 1 : 0);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    return this.db
      .prepare(`SELECT * FROM a2a_event_subscriptions ${where} ORDER BY created_at DESC`)
      .all(...params)
      .map(this._mapEventSubscription);
  }

  _mapEventSubscription(row) {
    return {
      ...row,
      event_types: JSON.parse(row.event_types || '["*"]'),
      active: Boolean(row.active),
    };
  }

  // ===========================================================================
  // Event Log
  // ===========================================================================

  createEventLog(event) {
    this.init();
    const id = event.id || randomUUID();
    const now = new Date().toISOString();

    this.db
      .prepare(
        `INSERT INTO a2a_event_log (id, event_type, agent_address, payload, created_at)
       VALUES (?, ?, ?, ?, ?)`,
      )
      .run(
        id,
        event.event_type,
        event.agent_address,
        typeof event.payload === 'object' ? JSON.stringify(event.payload) : event.payload,
        event.created_at || now,
      );

    return this.getEventLog(id);
  }

  getEventLog(id) {
    this.init();
    return this.db.prepare('SELECT * FROM a2a_event_log WHERE id = ?').get(id) || null;
  }

  listEventLog(filter = {}) {
    this.init();
    const conditions = [];
    const params = [];

    if (filter.agent_address) {
      conditions.push('agent_address = ?');
      params.push(filter.agent_address);
    }
    if (filter.event_type) {
      conditions.push('event_type = ?');
      params.push(filter.event_type);
    }
    if (filter.since) {
      conditions.push('created_at > ?');
      params.push(filter.since);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const limit = filter.limit || 100;
    const offset = filter.offset || 0;

    return this.db
      .prepare(`SELECT * FROM a2a_event_log ${where} ORDER BY created_at DESC LIMIT ? OFFSET ?`)
      .all(...params, limit, offset);
  }
}

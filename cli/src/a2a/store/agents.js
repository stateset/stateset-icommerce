/**
 * A2A Store — agent cards (registration, discovery, verification).
 *
 * Extracted verbatim from `cli/src/a2a/store.js` (Aug 2026 decomposition).
 */

import { randomUUID } from 'node:crypto';

/**
 * A2A Store — agent cards (registration, discovery, verification).
 *
 * Methods are copied verbatim onto `A2AStore.prototype` by `applyStoreMixins()`
 * in `../store.js`; this class is never instantiated on its own.
 * Every method runs with `this` bound to the owning A2AStore (`this.db` is the
 * open better-sqlite3 handle).
 *
 * @this {import('../store.js').A2AStore}
 */
export class A2AAgentsMethods {
  // ===========================================================================
  // Agent Cards
  // ===========================================================================

  registerAgent(card) {
    this.init();
    const id = card.id || randomUUID();
    const now = new Date().toISOString();

    this.db
      .prepare(
        `INSERT INTO a2a_runtime_agent_cards (
          id, name, wallet_address, public_key, supported_networks,
          supported_assets, a2a_skills, payment_addresses, endpoint_url, description,
          trust_level, active, suspended_at, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        id,
        card.name,
        card.wallet_address,
        card.public_key || null,
        typeof card.supported_networks === 'object'
          ? JSON.stringify(card.supported_networks)
          : card.supported_networks || '["set_chain"]',
        typeof card.supported_assets === 'object'
          ? JSON.stringify(card.supported_assets)
          : card.supported_assets || '["USDC"]',
        typeof card.a2a_skills === 'object'
          ? JSON.stringify(card.a2a_skills)
          : card.a2a_skills || '["buy","sell","quote"]',
        typeof card.payment_addresses === 'object'
          ? JSON.stringify(card.payment_addresses)
          : card.payment_addresses || null,
        card.endpoint_url || null,
        card.description || null,
        card.trust_level || 'sandbox',
        card.active !== undefined ? (card.active ? 1 : 0) : 1,
        card.suspended_at || null,
        card.created_at || now,
        card.updated_at || now,
      );

    return this.getAgent(id);
  }

  getAgent(id) {
    this.init();
    return this.db.prepare('SELECT * FROM a2a_runtime_agent_cards WHERE id = ?').get(id) || null;
  }

  getAgentByWallet(address) {
    this.init();
    return (
      this.db
        .prepare('SELECT * FROM a2a_runtime_agent_cards WHERE wallet_address = ?')
        .get(address) || null
    );
  }

  listAgents(filter = {}) {
    this.init();
    const conditions = [];
    const params = [];

    if (filter.active !== undefined) {
      conditions.push('active = ?');
      params.push(filter.active ? 1 : 0);
    }
    if (filter.trust_level) {
      conditions.push('trust_level = ?');
      params.push(filter.trust_level);
    }
    if (filter.name) {
      conditions.push('name LIKE ?');
      params.push(`%${filter.name}%`);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const limit = filter.limit || 100;
    const offset = filter.offset || 0;

    return this.db
      .prepare(
        `SELECT * FROM a2a_runtime_agent_cards ${where} ORDER BY created_at DESC LIMIT ? OFFSET ?`,
      )
      .all(...params, limit, offset);
  }

  discoverAgents(filter = {}, asset, skill, trustLevel) {
    this.init();
    if (!filter || typeof filter !== 'object' || Array.isArray(filter)) {
      filter = {
        network: filter || undefined,
        asset,
        skill,
        trust_level: trustLevel,
      };
    }
    const conditions = ['active = 1'];
    const params = [];

    if (filter.network) {
      conditions.push("supported_networks LIKE '%' || ? || '%'");
      params.push(filter.network);
    }
    if (filter.asset) {
      conditions.push("supported_assets LIKE '%' || ? || '%'");
      params.push(filter.asset);
    }
    if (filter.skill) {
      conditions.push("a2a_skills LIKE '%' || ? || '%'");
      params.push(filter.skill);
    }
    if (filter.trust_level) {
      conditions.push('trust_level = ?');
      params.push(filter.trust_level);
    }
    if (filter.category) {
      conditions.push('description LIKE ?');
      params.push(`%${filter.category}%`);
    }

    const where = `WHERE ${conditions.join(' AND ')}`;
    const limit = filter.limit || 50;

    return this.db
      .prepare(
        `SELECT * FROM a2a_runtime_agent_cards ${where} ORDER BY trust_level DESC, name ASC LIMIT ?`,
      )
      .all(...params, limit);
  }

  verifyAgent(id) {
    this.init();
    const now = new Date().toISOString();
    this.db
      .prepare('UPDATE a2a_runtime_agent_cards SET trust_level = ?, updated_at = ? WHERE id = ?')
      .run('verified', now, id);
    return this.getAgent(id);
  }

  updateAgent(id, updates) {
    this.init();
    this._validateUpdateKeys('a2a_runtime_agent_cards', Object.keys(updates));
    const fields = [];
    const values = [];

    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        fields.push(`${key} = ?`);
        values.push(typeof value === 'object' && value !== null ? JSON.stringify(value) : value);
      }
    }

    if (fields.length === 0) return this.getAgent(id);

    fields.push('updated_at = ?');
    values.push(new Date().toISOString());
    values.push(id);

    this.db
      .prepare(`UPDATE a2a_runtime_agent_cards SET ${fields.join(', ')} WHERE id = ?`)
      .run(...values);
    return this.getAgent(id);
  }
}

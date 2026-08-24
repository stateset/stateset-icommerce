/**
 * A2A Store — SLA definitions and violations.
 *
 * Extracted verbatim from `cli/src/a2a/store.js` (Aug 2026 decomposition).
 */

import { randomUUID } from 'node:crypto';

/**
 * A2A Store — SLA definitions and violations.
 *
 * Methods are copied verbatim onto `A2AStore.prototype` by `applyStoreMixins()`
 * in `../store.js`; this class is never instantiated on its own.
 * Every method runs with `this` bound to the owning A2AStore (`this.db` is the
 * open better-sqlite3 handle).
 *
 * @this {import('../store.js').A2AStore}
 */
export class A2ASLAMethods {
  // ===========================================================================
  // SLA Definitions
  // ===========================================================================

  createSLADefinition(sla) {
    this.init();
    const id = sla.id || randomUUID();
    const now = new Date().toISOString();
    this.db
      .prepare(
        `INSERT INTO a2a_sla_definitions (id, service_id, response_time_ms, uptime_percent, quality_min_score, throughput_rps, penalty_percent, penalty_type, active, metadata, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        id,
        sla.service_id,
        sla.response_time_ms ?? null,
        sla.uptime_percent ?? null,
        sla.quality_min_score ?? null,
        sla.throughput_rps ?? null,
        sla.penalty_percent ?? 5.0,
        sla.penalty_type || 'credit',
        sla.active ?? 1,
        typeof sla.metadata === 'string'
          ? sla.metadata
          : sla.metadata
            ? JSON.stringify(sla.metadata)
            : null,
        now,
        now,
      );
    return this.getSLADefinition(id);
  }

  getSLADefinition(id) {
    this.init();
    return this.db.prepare('SELECT * FROM a2a_sla_definitions WHERE id = ?').get(id) || null;
  }

  updateSLADefinition(id, updates) {
    this.init();
    this._validateUpdateKeys('a2a_sla_definitions', Object.keys(updates));
    const fields = [];
    const values = [];
    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        fields.push(`${key} = ?`);
        values.push(typeof value === 'object' && value !== null ? JSON.stringify(value) : value);
      }
    }
    if (fields.length === 0) return this.getSLADefinition(id);
    fields.push('updated_at = ?');
    values.push(new Date().toISOString());
    values.push(id);
    this.db
      .prepare(`UPDATE a2a_sla_definitions SET ${fields.join(', ')} WHERE id = ?`)
      .run(...values);
    return this.getSLADefinition(id);
  }

  listSLADefinitions(filter = {}) {
    this.init();
    const clauses = [];
    const params = [];
    if (filter.service_id) {
      clauses.push('service_id = ?');
      params.push(filter.service_id);
    }
    if (filter.active !== undefined) {
      clauses.push('active = ?');
      params.push(filter.active);
    }
    const where = clauses.length > 0 ? `WHERE ${clauses.join(' AND ')}` : '';
    return this.db
      .prepare(`SELECT * FROM a2a_sla_definitions ${where} ORDER BY created_at DESC`)
      .all(...params);
  }

  // ===========================================================================
  // SLA Violations
  // ===========================================================================

  createSLAViolation(v) {
    this.init();
    const id = v.id || randomUUID();
    const now = new Date().toISOString();
    this.db
      .prepare(
        `INSERT INTO a2a_sla_violations (id, sla_id, service_id, violation_type, expected_value, actual_value, severity, penalty_amount, resolved, metadata, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        id,
        v.sla_id,
        v.service_id,
        v.violation_type,
        v.expected_value,
        v.actual_value,
        v.severity || 'warning',
        v.penalty_amount ?? null,
        v.resolved ?? 0,
        typeof v.metadata === 'string'
          ? v.metadata
          : v.metadata
            ? JSON.stringify(v.metadata)
            : null,
        now,
      );
    return this.getSLAViolation(id);
  }

  getSLAViolation(id) {
    this.init();
    return this.db.prepare('SELECT * FROM a2a_sla_violations WHERE id = ?').get(id) || null;
  }

  updateSLAViolation(id, updates) {
    this.init();
    this._validateUpdateKeys('a2a_sla_violations', Object.keys(updates));
    const fields = [];
    const values = [];
    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        fields.push(`${key} = ?`);
        values.push(typeof value === 'object' && value !== null ? JSON.stringify(value) : value);
      }
    }
    if (fields.length === 0) return this.getSLAViolation(id);
    values.push(id);
    this.db
      .prepare(`UPDATE a2a_sla_violations SET ${fields.join(', ')} WHERE id = ?`)
      .run(...values);
    return this.getSLAViolation(id);
  }

  listSLAViolations(filter = {}) {
    this.init();
    const clauses = [];
    const params = [];
    if (filter.sla_id) {
      clauses.push('sla_id = ?');
      params.push(filter.sla_id);
    }
    if (filter.service_id) {
      clauses.push('service_id = ?');
      params.push(filter.service_id);
    }
    if (filter.resolved !== undefined) {
      clauses.push('resolved = ?');
      params.push(filter.resolved);
    }
    if (filter.severity) {
      clauses.push('severity = ?');
      params.push(filter.severity);
    }
    const where = clauses.length > 0 ? `WHERE ${clauses.join(' AND ')}` : '';
    return this.db
      .prepare(`SELECT * FROM a2a_sla_violations ${where} ORDER BY created_at DESC`)
      .all(...params);
  }
}

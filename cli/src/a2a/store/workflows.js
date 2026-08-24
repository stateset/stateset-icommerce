/**
 * A2A Store — multi-step workflows and workflow steps.
 *
 * Extracted verbatim from `cli/src/a2a/store.js` (Aug 2026 decomposition).
 */

import { randomUUID } from 'node:crypto';

/**
 * A2A Store — multi-step workflows and workflow steps.
 *
 * Methods are copied verbatim onto `A2AStore.prototype` by `applyStoreMixins()`
 * in `../store.js`; this class is never instantiated on its own.
 * Every method runs with `this` bound to the owning A2AStore (`this.db` is the
 * open better-sqlite3 handle).
 *
 * @this {import('../store.js').A2AStore}
 */
export class A2AWorkflowsMethods {
  // ===========================================================================
  // Workflows
  // ===========================================================================

  createWorkflow(wf) {
    this.init();
    const id = wf.id || randomUUID();
    const now = new Date().toISOString();
    this.db
      .prepare(
        `INSERT INTO a2a_workflows (id, name, definition, status, total_cost, current_step, error, metadata, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        id,
        wf.name,
        typeof wf.definition === 'string' ? wf.definition : JSON.stringify(wf.definition || {}),
        wf.status || 'pending',
        wf.total_cost ?? 0,
        wf.current_step || null,
        wf.error || null,
        typeof wf.metadata === 'string'
          ? wf.metadata
          : wf.metadata
            ? JSON.stringify(wf.metadata)
            : null,
        now,
        now,
      );
    return this.getWorkflow(id);
  }

  getWorkflow(id) {
    this.init();
    return this.db.prepare('SELECT * FROM a2a_workflows WHERE id = ?').get(id) || null;
  }

  updateWorkflow(id, updates) {
    this.init();
    this._validateUpdateKeys('a2a_workflows', Object.keys(updates));
    const fields = [];
    const values = [];
    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        fields.push(`${key} = ?`);
        values.push(typeof value === 'object' && value !== null ? JSON.stringify(value) : value);
      }
    }
    if (fields.length === 0) return this.getWorkflow(id);
    fields.push('updated_at = ?');
    values.push(new Date().toISOString());
    values.push(id);
    this.db.prepare(`UPDATE a2a_workflows SET ${fields.join(', ')} WHERE id = ?`).run(...values);
    return this.getWorkflow(id);
  }

  listWorkflows(filter = {}) {
    this.init();
    const clauses = [];
    const params = [];
    if (filter.status) {
      clauses.push('status = ?');
      params.push(filter.status);
    }
    const where = clauses.length > 0 ? `WHERE ${clauses.join(' AND ')}` : '';
    const limit = filter.limit ? `LIMIT ${Math.min(Number(filter.limit), 1000)}` : 'LIMIT 100';
    return this.db
      .prepare(`SELECT * FROM a2a_workflows ${where} ORDER BY created_at DESC ${limit}`)
      .all(...params);
  }

  // ===========================================================================
  // Workflow Steps
  // ===========================================================================

  createWorkflowStep(step) {
    this.init();
    const id = step.id || randomUUID();
    const now = new Date().toISOString();
    this.db
      .prepare(
        `INSERT INTO a2a_workflow_steps (id, workflow_id, step_name, step_type, agent_address, params, depends_on, status, result, cost, error, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        id,
        step.workflow_id,
        step.step_name,
        step.step_type || 'quote_request',
        step.agent_address || null,
        typeof step.params === 'string'
          ? step.params
          : step.params
            ? JSON.stringify(step.params)
            : null,
        typeof step.depends_on === 'string'
          ? step.depends_on
          : JSON.stringify(step.depends_on || []),
        step.status || 'pending',
        step.result || null,
        step.cost ?? 0,
        step.error || null,
        now,
      );
    return this.getWorkflowStep(id);
  }

  getWorkflowStep(id) {
    this.init();
    return this.db.prepare('SELECT * FROM a2a_workflow_steps WHERE id = ?').get(id) || null;
  }

  updateWorkflowStep(id, updates) {
    this.init();
    this._validateUpdateKeys('a2a_workflow_steps', Object.keys(updates));
    const fields = [];
    const values = [];
    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        fields.push(`${key} = ?`);
        values.push(typeof value === 'object' && value !== null ? JSON.stringify(value) : value);
      }
    }
    if (fields.length === 0) return this.getWorkflowStep(id);
    values.push(id);
    this.db
      .prepare(`UPDATE a2a_workflow_steps SET ${fields.join(', ')} WHERE id = ?`)
      .run(...values);
    return this.getWorkflowStep(id);
  }

  listWorkflowSteps(filter = {}) {
    this.init();
    const clauses = [];
    const params = [];
    if (filter.workflow_id) {
      clauses.push('workflow_id = ?');
      params.push(filter.workflow_id);
    }
    if (filter.status) {
      clauses.push('status = ?');
      params.push(filter.status);
    }
    const where = clauses.length > 0 ? `WHERE ${clauses.join(' AND ')}` : '';
    return this.db
      .prepare(`SELECT * FROM a2a_workflow_steps ${where} ORDER BY created_at ASC`)
      .all(...params);
  }
}

/**
 * A2A Store — split payments and split recipients.
 *
 * Extracted verbatim from `cli/src/a2a/store.js` (Aug 2026 decomposition).
 */

import { randomUUID } from 'node:crypto';

/**
 * A2A Store — split payments and split recipients.
 *
 * Methods are copied verbatim onto `A2AStore.prototype` by `applyStoreMixins()`
 * in `../store.js`; this class is never instantiated on its own.
 * Every method runs with `this` bound to the owning A2AStore (`this.db` is the
 * open better-sqlite3 handle).
 *
 * @this {import('../store.js').A2AStore}
 */
export class A2ASplitsMethods {
  // ===========================================================================
  // Split Payments
  // ===========================================================================

  createSplitPayment(split) {
    this.init();
    const id = split.id || randomUUID();
    const now = new Date().toISOString();

    this.db
      .prepare(
        `INSERT INTO a2a_split_payments (
        id, status, sender_address, total_amount, total_amount_decimal,
        asset, network, split_type, platform_fee_percent, platform_fee_amount,
        platform_fee_address, memo, reference_type, reference_id,
        metadata, created_at, updated_at, completed_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        id,
        split.status || 'pending',
        split.sender_address,
        split.total_amount,
        split.total_amount_decimal,
        split.asset || 'USDC',
        split.network || 'set_chain',
        split.split_type || 'percentage',
        split.platform_fee_percent ?? null,
        split.platform_fee_amount ?? null,
        split.platform_fee_address || null,
        split.memo || null,
        split.reference_type || null,
        split.reference_id || null,
        split.metadata || null,
        split.created_at || now,
        split.updated_at || now,
        split.completed_at || null,
      );

    return this.getSplitPayment(id);
  }

  getSplitPayment(id) {
    this.init();
    const row = this.db.prepare('SELECT * FROM a2a_split_payments WHERE id = ?').get(id);
    if (!row) return null;
    const recipients = this.listSplitRecipients(id);
    return { ...row, recipients };
  }

  updateSplitPayment(id, updates) {
    this.init();
    this._validateUpdateKeys('a2a_split_payments', Object.keys(updates));
    const fields = [];
    const values = [];

    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        fields.push(`${key} = ?`);
        values.push(value);
      }
    }

    if (fields.length === 0) return this.getSplitPayment(id);

    fields.push('updated_at = ?');
    values.push(new Date().toISOString());
    values.push(id);

    this.db
      .prepare(`UPDATE a2a_split_payments SET ${fields.join(', ')} WHERE id = ?`)
      .run(...values);
    return this.getSplitPayment(id);
  }

  listSplitPayments(filter = {}) {
    this.init();
    const conditions = [];
    const params = [];

    if (filter.sender_address) {
      conditions.push('sender_address = ?');
      params.push(filter.sender_address);
    }
    if (filter.status) {
      conditions.push('status = ?');
      params.push(filter.status);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const limit = filter.limit || 50;
    const offset = filter.offset || 0;

    return this.db
      .prepare(
        `SELECT * FROM a2a_split_payments ${where} ORDER BY created_at DESC LIMIT ? OFFSET ?`,
      )
      .all(...params, limit, offset);
  }

  // ===========================================================================
  // Split Recipients
  // ===========================================================================

  createSplitRecipient(recipient) {
    this.init();
    const id = recipient.id || randomUUID();
    const now = new Date().toISOString();

    this.db
      .prepare(
        `INSERT INTO a2a_split_recipients (
        id, split_payment_id, recipient_address, share_percent,
        share_amount, share_amount_decimal, payment_id, status,
        created_at, updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        id,
        recipient.split_payment_id,
        recipient.recipient_address,
        recipient.share_percent ?? null,
        recipient.share_amount ?? null,
        recipient.share_amount_decimal ?? null,
        recipient.payment_id || null,
        recipient.status || 'pending',
        recipient.created_at || now,
        recipient.updated_at || now,
      );

    return this.getSplitRecipient(id);
  }

  getSplitRecipient(id) {
    this.init();
    return this.db.prepare('SELECT * FROM a2a_split_recipients WHERE id = ?').get(id) || null;
  }

  updateSplitRecipient(id, updates) {
    this.init();
    this._validateUpdateKeys('a2a_split_recipients', Object.keys(updates));
    const fields = [];
    const values = [];

    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        fields.push(`${key} = ?`);
        values.push(value);
      }
    }

    if (fields.length === 0) return this.getSplitRecipient(id);

    fields.push('updated_at = ?');
    values.push(new Date().toISOString());
    values.push(id);

    this.db
      .prepare(`UPDATE a2a_split_recipients SET ${fields.join(', ')} WHERE id = ?`)
      .run(...values);
    return this.getSplitRecipient(id);
  }

  listSplitRecipients(splitPaymentId) {
    this.init();
    return this.db
      .prepare(
        'SELECT * FROM a2a_split_recipients WHERE split_payment_id = ? ORDER BY created_at ASC',
      )
      .all(splitPaymentId);
  }
}

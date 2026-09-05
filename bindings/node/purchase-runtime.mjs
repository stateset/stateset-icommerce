/** Durable purchase coordination. No CLI or model SDK dependency.
 * External adapters are operator-owned and MUST implement idempotency plus
 * authoritative lookup. A lost response is reconciled before resubmission.
 */
import { createHash, randomUUID } from 'node:crypto';

const SCALE = 10n ** 18n;
function units(value) {
  if (typeof value !== 'string' || !/^(0|[1-9]\d{0,19})(\.\d{1,18})?$/.test(value)) {
    throw new Error('amount must be an exact nonnegative decimal string (up to 18 places)');
  }
  const [whole, fraction = ''] = value.split('.');
  return BigInt(whole) * SCALE + BigInt(fraction.padEnd(18, '0'));
}
function decimal(value) {
  const fraction = (value % SCALE).toString().padStart(18, '0').replace(/0+$/, '');
  return `${value / SCALE}${fraction ? `.${fraction}` : ''}`;
}
function text(value, name) {
  if (typeof value !== 'string' || !value.trim() || value.length > 512) {
    throw new Error(`${name} must be a nonempty string of at most 512 characters`);
  }
  return value;
}
function canonical(value) {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
  if (value !== null && typeof value === 'object') {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`)
      .join(',')}}`;
  }
  const encoded = JSON.stringify(value);
  if (encoded === undefined) throw new Error('purchase data must be JSON serializable');
  return encoded;
}
const digest = (value) => createHash('sha256').update(canonical(value)).digest('hex');
const TERMINAL = new Set(['completed', 'cancelled']);
const STEPS = ['reserve_inventory', 'pay', 'create_order', 'confirm_inventory'];
const REQUIRED_EVIDENCE = {
  reserve_inventory: 'reservation_id',
  pay: 'transaction_id',
  create_order: 'order_id',
  confirm_inventory: 'reservation_id',
  release_inventory: 'reservation_id',
};

/** Caller owns the better-sqlite3-compatible handle and its backup lifecycle.
 * All balance/operation changes use one immediate transaction. Distinct agents
 * share a budget by using the same principal, tenant, store and budget ID.
 */
export class SqlitePurchaseStore {
  constructor(db) {
    if (typeof db?.transaction !== 'function' || typeof db?.prepare !== 'function') {
      throw new Error('a better-sqlite3-compatible database is required');
    }
    this.db = db;
    db.exec(`
      CREATE TABLE IF NOT EXISTS _stateset_purchase_budgets (
        scope TEXT NOT NULL, id TEXT NOT NULL, definition TEXT NOT NULL,
        reserved TEXT NOT NULL DEFAULT '0', spent TEXT NOT NULL DEFAULT '0',
        PRIMARY KEY(scope, id)
      );
      CREATE TABLE IF NOT EXISTS _stateset_purchases (
        id TEXT PRIMARY KEY, scope TEXT NOT NULL, agent TEXT NOT NULL,
        request_key TEXT NOT NULL, fingerprint TEXT NOT NULL, data TEXT NOT NULL,
        lease_owner TEXT, lease_until INTEGER NOT NULL DEFAULT 0,
        UNIQUE(scope, agent, request_key)
      );
      CREATE INDEX IF NOT EXISTS _stateset_purchase_recovery
        ON _stateset_purchases(scope, agent, id)
        WHERE json_extract(data, '$.status') NOT IN ('completed', 'cancelled');
    `);
  }

  provisionBudget(scope, definition) {
    text(scope, 'scope');
    const { id, asset, limit, expiresAt } = definition;
    text(id, 'budget id');
    text(asset, 'asset');
    units(limit);
    if (!Number.isFinite(Date.parse(expiresAt))) throw new Error('invalid budget expiry');
    const encoded = canonical({ id, asset, limit: decimal(units(limit)), expiresAt });
    this.db
      .transaction(() => {
        const old = this.db
          .prepare('SELECT definition FROM _stateset_purchase_budgets WHERE scope=? AND id=?')
          .get(scope, id);
        if (old && old.definition !== encoded) throw new Error('budget definition is immutable');
        this.db
          .prepare(
            'INSERT OR IGNORE INTO _stateset_purchase_budgets(scope,id,definition) VALUES(?,?,?)',
          )
          .run(scope, id, encoded);
      })
      .immediate();
    return this.budget(scope, id);
  }

  budget(scope, id) {
    const row = this.db
      .prepare('SELECT * FROM _stateset_purchase_budgets WHERE scope=? AND id=?')
      .get(scope, id);
    if (!row) throw new Error('budget not found');
    const definition = JSON.parse(row.definition);
    return {
      ...definition,
      reserved: row.reserved,
      spent: row.spent,
      available: decimal(units(definition.limit) - units(row.reserved) - units(row.spent)),
    };
  }

  get(id) {
    const row = this.db.prepare('SELECT data FROM _stateset_purchases WHERE id=?').get(id);
    return row ? JSON.parse(row.data) : null;
  }

  find(scope, agent, key) {
    const row = this.db
      .prepare('SELECT data FROM _stateset_purchases WHERE scope=? AND agent=? AND request_key=?')
      .get(scope, agent, key);
    return row ? JSON.parse(row.data) : null;
  }

  pending(scope, agent, { limit = 100, after = null } = {}) {
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 1000) {
      throw new Error('recovery limit must be an integer from 1 to 1000');
    }
    if (after !== null) text(after, 'recovery cursor');
    const rows = this.db
      .prepare(
        `SELECT id,data FROM _stateset_purchases
      WHERE scope=? AND agent=? AND id>?
        AND json_extract(data, '$.status') NOT IN ('completed', 'cancelled')
      ORDER BY id LIMIT ?`,
      )
      .all(scope, agent, after ?? '', limit + 1);
    const page = rows.slice(0, limit);
    return {
      operations: page.map((row) => JSON.parse(row.data)),
      nextCursor: rows.length > limit ? page.at(-1).id : null,
    };
  }

  create(operation, now) {
    return this.db
      .transaction(() => {
        const existing = this.find(
          operation.scope,
          operation.agent,
          operation.request.idempotencyKey,
        );
        if (existing) {
          if (existing.fingerprint !== operation.fingerprint)
            throw new Error('idempotency conflict');
          return existing;
        }
        const budget = this.budget(operation.scope, operation.request.budgetId);
        if (budget.asset !== operation.quote.asset || Date.parse(budget.expiresAt) <= now) {
          throw new Error('budget asset mismatch or expired budget');
        }
        if (units(budget.available) < units(operation.quote.amount))
          throw new Error('budget exceeded');
        this.db
          .prepare('UPDATE _stateset_purchase_budgets SET reserved=? WHERE scope=? AND id=?')
          .run(
            decimal(units(budget.reserved) + units(operation.quote.amount)),
            operation.scope,
            budget.id,
          );
        this.db
          .prepare(
            'INSERT INTO _stateset_purchases(id,scope,agent,request_key,fingerprint,data) VALUES(?,?,?,?,?,?)',
          )
          .run(
            operation.id,
            operation.scope,
            operation.agent,
            operation.request.idempotencyKey,
            operation.fingerprint,
            JSON.stringify(operation),
          );
        return operation;
      })
      .immediate();
  }

  claim(id, owner, now, leaseMs) {
    return (
      this.db
        .prepare(
          `UPDATE _stateset_purchases SET lease_owner=?, lease_until=?
      WHERE id=? AND lease_until<=?`,
        )
        .run(owner, now + leaseMs, id, now).changes === 1
    );
  }

  save(operation, owner, budgetAction = null) {
    this.db
      .transaction(() => {
        const row = this.db
          .prepare('SELECT lease_owner,data FROM _stateset_purchases WHERE id=?')
          .get(operation.id);
        if (row?.lease_owner !== owner) throw new Error('purchase lease lost');
        const prior = JSON.parse(row.data);
        if (budgetAction && prior.budgetState === 'reserved') {
          const budget = this.budget(operation.scope, operation.request.budgetId);
          const amount = units(operation.quote.amount);
          if (units(budget.reserved) < amount) throw new Error('corrupt purchase budget balance');
          this.db
            .prepare(
              'UPDATE _stateset_purchase_budgets SET reserved=?,spent=? WHERE scope=? AND id=?',
            )
            .run(
              decimal(units(budget.reserved) - amount),
              decimal(units(budget.spent) + (budgetAction === 'capture' ? amount : 0n)),
              operation.scope,
              budget.id,
            );
          operation.budgetState = budgetAction === 'capture' ? 'spent' : 'released';
        }
        this.db
          .prepare('UPDATE _stateset_purchases SET data=? WHERE id=? AND lease_owner=?')
          .run(JSON.stringify(operation), operation.id, owner);
      })
      .immediate();
  }

  release(id, owner) {
    this.db
      .prepare(
        'UPDATE _stateset_purchases SET lease_owner=NULL,lease_until=0 WHERE id=? AND lease_owner=?',
      )
      .run(id, owner);
  }
}

/** Host configuration is never supplied by the agent's buy() arguments.
 * authorize() runs on every resume. It may reject newly dispatched actions;
 * already dispatched effects are still reconciled to retain accountability.
 */
export class PurchaseRuntime {
  constructor({
    store,
    identity,
    policyVersion,
    resolveQuote,
    authorize,
    adapters,
    allowApply = false,
    clock = Date.now,
    leaseMs = 30_000,
  }) {
    this.identity = structuredClone(identity);
    for (const field of ['agentId', 'principalId', 'tenantId', 'storeId'])
      text(identity[field], field);
    this.scope = digest([identity.principalId, identity.tenantId, identity.storeId]);
    this.policyVersion = text(policyVersion, 'policyVersion');
    if (typeof resolveQuote !== 'function' || typeof authorize !== 'function') {
      throw new Error('trusted quote resolver and authorizer are required');
    }
    for (const step of [...STEPS, 'release_inventory']) {
      if (
        typeof adapters?.[step]?.execute !== 'function' ||
        typeof adapters[step]?.lookup !== 'function'
      ) {
        throw new Error(`${step} requires idempotent execute and authoritative lookup`);
      }
    }
    if (!Number.isSafeInteger(leaseMs) || leaseMs <= 0) throw new Error('invalid lease duration');
    Object.assign(this, {
      store,
      resolveQuote,
      authorize,
      adapters,
      allowApply: allowApply === true,
      clock,
      leaseMs,
    });
  }

  get(id) {
    const op = this.store.get(id);
    if (!op || op.scope !== this.scope || op.agent !== this.identity.agentId)
      throw new Error('purchase not found');
    return op;
  }

  async buy(input) {
    // Closed request shape prevents model arguments from supplying execution,
    // authority, quote prices, credentials, or settlement confirmations.
    const fields = ['idempotencyKey', 'quoteId', 'budgetId', 'maxAmount', 'asset'];
    if (!input || Object.keys(input).some((key) => !fields.includes(key)))
      throw new Error('unknown purchase argument');
    const request = Object.fromEntries(fields.map((key) => [key, text(input[key], key)]));
    request.maxAmount = decimal(units(request.maxAmount));
    const fingerprint = digest(request);
    const old = this.store.find(this.scope, this.identity.agentId, request.idempotencyKey);
    if (old) {
      if (old.fingerprint !== fingerprint) throw new Error('idempotency conflict');
      return this.allowApply ? this.resume(old.id) : old;
    }
    const quote = structuredClone(
      await this.resolveQuote(request.quoteId, structuredClone(this.identity)),
    );
    text(quote.id, 'quote id');
    text(quote.counterpartyId, 'counterparty');
    if (
      quote.id !== request.quoteId ||
      quote.asset !== request.asset ||
      units(quote.amount) <= 0n ||
      units(quote.amount) > units(request.maxAmount) ||
      !Number.isFinite(Date.parse(quote.expiresAt)) ||
      Date.parse(quote.expiresAt) <= this.clock()
    ) {
      throw new Error('quote is expired or violates purchase constraints');
    }
    const op = {
      id: randomUUID(),
      scope: this.scope,
      agent: this.identity.agentId,
      principal: this.identity.principalId,
      policyVersion: this.policyVersion,
      request,
      fingerprint,
      quote,
      status: 'pending',
      budgetState: 'reserved',
      steps: {},
      createdAt: new Date(this.clock()).toISOString(),
      error: null,
    };
    const decision = await this.authorize(structuredClone(op), structuredClone(this.identity));
    if (decision?.allowed !== true) throw new Error('purchase not authorized');
    op.authorization = structuredClone(decision);
    if (!this.allowApply) return { ...op, status: 'preview', budgetState: 'unreserved' };
    return this.resume(this.store.create(op, this.clock()).id);
  }

  async resume(id) {
    return this.#resume(id, false);
  }

  /** Read-only, identity-scoped discovery. Cursor pages are a live view, not a
   * snapshot: start a new scan periodically to revisit pending outcomes.
   */
  pending(options = {}) {
    return this.store.pending(this.scope, this.identity.agentId, options);
  }

  /** One bounded reconciliation pass. No background timers or blind retries.
   * Operator-attention cases are reported but never automatically retried.
   */
  async recover(options = {}) {
    if (!this.allowApply) throw new Error('apply is disabled');
    const page = this.pending(options);
    const results = [];
    for (const operation of page.operations) {
      try {
        results.push({ id: operation.id, operation: await this.#resume(operation.id, true) });
      } catch (error) {
        results.push({ id: operation.id, error: String(error.message || error) });
      }
    }
    return { results, nextCursor: page.nextCursor };
  }

  async #resume(id, automatic) {
    if (!this.allowApply) throw new Error('apply is disabled');
    let op = this.get(id);
    if (TERMINAL.has(op.status)) return op;
    const owner = randomUUID();
    if (!this.store.claim(id, owner, this.clock(), this.leaseMs)) {
      return { ...this.get(id), busy: true };
    }
    try {
      op = this.get(id);
      // Re-check under the lease: another worker may have changed status
      // after discovery (or completed the purchase before we claimed it).
      if (TERMINAL.has(op.status)) return op;
      if (automatic && op.status === 'needs_attention') {
        return { ...op, skipped: 'needs_attention' };
      }
      const decision = await this.authorize(structuredClone(op), structuredClone(this.identity));
      const allowed = decision?.allowed === true && this.policyVersion === op.policyVersion;
      for (;;) {
        let step = op.compensationRequested
          ? 'release_inventory'
          : STEPS.find((name) => op.steps[name]?.status !== 'succeeded');
        if (op.cancelRequested) {
          if (op.steps.pay?.status === 'succeeded') {
            op.status = 'needs_attention';
            op.error = 'payment settled; governed refund required';
            this.store.save(op, owner);
            return op;
          }
          if (op.steps.pay?.dispatched && op.steps.pay.status !== 'failed') step = 'pay';
          else if (op.steps.reserve_inventory?.status === 'succeeded') step = 'release_inventory';
          else if (
            op.steps.reserve_inventory?.dispatched &&
            op.steps.reserve_inventory.status !== 'failed'
          )
            step = 'reserve_inventory';
          else {
            op.status = 'cancelled';
            this.store.save(op, owner, 'release');
            return op;
          }
        }
        if (!step) {
          op.status = 'completed';
          op.error = null;
          op.receipt = {
            version: 1,
            operationId: op.id,
            agentId: op.agent,
            principalId: op.principal,
            policyVersion: op.policyVersion,
            authorization: op.authorization,
            requestFingerprint: op.fingerprint,
            quoteId: op.quote.id,
            counterpartyId: op.quote.counterpartyId,
            amount: op.quote.amount,
            asset: op.quote.asset,
            evidence: Object.fromEntries(STEPS.map((name) => [name, op.steps[name].evidence])),
            completedAt: new Date(this.clock()).toISOString(),
          };
          // Digest binds the local operation summary; this is NOT a settlement
          // signature or proof of delivery. Adapter evidence retains receipts.
          op.receipt.digest = digest(op.receipt);
          this.store.save(op, owner);
          return op;
        }
        let record = op.steps[step];
        const context = {
          operation: structuredClone(op),
          idempotencyKey: `purchase:${op.id}:${step}`,
        };
        let result;
        if (record?.dispatched) {
          // Never infer that a timed-out submission failed. Only authoritative
          // not_found permits sending the identical operation again.
          result = await this.adapters[step].lookup(context);
        }
        if (!record?.dispatched || result?.status === 'not_found') {
          if (op.cancelRequested && step !== 'release_inventory') {
            op.steps[step] = { dispatched: false, status: 'failed' };
            this.store.save(op, owner);
            continue;
          }
          if (!allowed || (step === 'pay' && Date.parse(op.quote.expiresAt) <= this.clock())) {
            op.status = 'needs_attention';
            op.error = 'authorization denied or quote expired';
            this.store.save(op, owner);
            return op;
          }
          record = { dispatched: true, status: 'pending' };
          op.steps[step] = record;
          this.store.save(op, owner);
          result = await this.adapters[step].execute(context);
        }
        if (result?.status === 'succeeded') {
          text(result.evidence?.[REQUIRED_EVIDENCE[step]], `${step} evidence`);
          if (
            ['confirm_inventory', 'release_inventory'].includes(step) &&
            result.evidence.reservation_id !== op.steps.reserve_inventory?.evidence?.reservation_id
          ) {
            throw new Error('inventory evidence does not match reservation');
          }
          if (
            step === 'pay' &&
            (result.evidence.asset !== op.quote.asset ||
              units(result.evidence.amount) !== units(op.quote.amount))
          ) {
            throw new Error('payment evidence does not match quote');
          }
          op.steps[step] = {
            dispatched: true,
            status: 'succeeded',
            evidence: structuredClone(result.evidence),
          };
          if (step === 'release_inventory') {
            op.status = 'cancelled';
            this.store.save(op, owner, 'release');
            return op;
          }
          op.status = 'running';
          op.error = null;
          this.store.save(op, owner, step === 'pay' ? 'capture' : null);
        } else if (result?.status === 'failed') {
          op.steps[step] = { dispatched: true, status: 'failed' };
          op.error = String(result.reason || `${step} rejected`);
          if (step === 'reserve_inventory') {
            op.status = 'cancelled';
            this.store.save(op, owner, 'release');
            return op;
          }
          if (step === 'pay') {
            op.compensationRequested = true;
            op.status = 'compensating';
            this.store.save(op, owner);
            continue;
          }
          op.status = 'needs_attention';
          this.store.save(op, owner);
          return op;
        } else {
          op.status = op.compensationRequested ? 'compensating' : 'reconciling';
          op.error = result?.status === 'pending' ? 'counterparty pending' : 'outcome unknown';
          this.store.save(op, owner);
          return op;
        }
      }
    } catch (error) {
      // Reload the persisted checkpoint: an exception can occur AFTER a remote
      // side effect but before its evidence was saved.
      op = this.get(id);
      op.status = op.compensationRequested ? 'compensating' : 'reconciling';
      op.error = String(error.message || error);
      this.store.save(op, owner);
      return op;
    } finally {
      this.store.release(id, owner);
    }
  }

  /** Cancellation reconciles any dispatched payment first. A settled payment
   * requires a separate governed refund and never restores spend implicitly.
   */
  async cancel(id) {
    if (!this.allowApply) throw new Error('apply is disabled');
    const operation = this.get(id);
    if (TERMINAL.has(operation.status)) return operation;
    const owner = randomUUID();
    if (!this.store.claim(id, owner, this.clock(), this.leaseMs))
      return { ...this.get(id), busy: true };
    try {
      const current = this.get(id);
      current.cancelRequested = true;
      this.store.save(current, owner);
    } finally {
      this.store.release(id, owner);
    }
    return this.resume(id);
  }
}

/** Adapt a local/remote governed kernel command to purchase coordination.
 * readReceipt must read durable receipts from the SAME store as commerce.
 * buildPayload and evidence are trusted host functions, never model arguments.
 */
export function createKernelPurchaseAdapter({
  commerce,
  policy,
  principal,
  storeId,
  commandType,
  buildPayload,
  readReceipt,
  evidence,
}) {
  const trustedPolicy = structuredClone(policy);
  const trustedPrincipal = structuredClone(principal);
  text(commandType, 'commandType');
  text(storeId, 'storeId');
  for (const fn of [buildPayload, readReceipt, evidence]) {
    if (typeof fn !== 'function') throw new Error('kernel adapter functions are required');
  }
  function project(receipt, context) {
    if (!receipt) return { status: 'not_found' };
    if (receipt.command_type !== commandType) throw new Error('kernel receipt command mismatch');
    if (receipt.idempotency_key !== context.idempotencyKey)
      throw new Error('kernel receipt idempotency mismatch');
    if (receipt.status === 'succeeded') {
      return { status: 'succeeded', evidence: { ...evidence(receipt), kernel_receipt: receipt } };
    }
    if (receipt.status === 'rejected')
      return { status: 'failed', reason: JSON.stringify(receipt.error ?? receipt) };
    return { status: 'unknown' };
  }
  return {
    async lookup(context) {
      return project(await readReceipt(context.idempotencyKey), context);
    },
    async execute(context) {
      const hex = digest(context.idempotencyKey);
      const commandId = `${hex.slice(0, 8)}-${hex.slice(8, 12)}-4${hex.slice(13, 16)}-8${hex.slice(17, 20)}-${hex.slice(20, 32)}`;
      return project(
        await commerce.executeKernelCommand(
          {
            command_id: commandId,
            command_type: commandType,
            contract_version: '1.0',
            idempotency_key: context.idempotencyKey,
            principal: trustedPrincipal,
            store_id: storeId,
            policy_version: trustedPolicy.version,
            mode: 'apply',
            issued_at: context.operation.createdAt,
            payload: buildPayload(context.operation),
          },
          trustedPolicy,
        ),
        context,
      );
    },
  };
}

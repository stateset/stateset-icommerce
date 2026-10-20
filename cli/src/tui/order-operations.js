import fs from 'node:fs/promises';
import { constants } from 'node:fs';
import path from 'node:path';
import { createHash, randomUUID } from 'node:crypto';

export const ORDER_ACTIONS = Object.freeze([
  { status: 'confirmed', label: 'Confirm order' },
  { status: 'processing', label: 'Start processing' },
  { status: 'cancelled', label: 'Cancel order' },
]);
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const digest = (value) => createHash('sha256').update(JSON.stringify(value)).digest('hex');
const configurationMessage =
  'Order actions need operator-owned --kernel-policy, --kernel-principal and --kernel-store-id. Live changes also require --db and --apply.';

export function orderAction(value) {
  if (
    !value ||
    typeof value !== 'object' ||
    Array.isArray(value) ||
    Object.keys(value).some((key) => !['orderId', 'status'].includes(key)) ||
    typeof value.orderId !== 'string' ||
    !uuid.test(value.orderId) ||
    !ORDER_ACTIONS.some((action) => action.status === value.status)
  )
    throw new Error('Choose a supported order status and a valid order ID.');
  return { orderId: value.orderId, status: value.status };
}

// Keep a returned native outcome, including a commit, but never present an
// inconsistent success as a normal completion or invite an automatic retry.
export function orderReceiptIssues(receipt, review) {
  if (receipt?.status !== 'succeeded') return [];
  const issues = [];
  if (receipt.version_before !== review.expectedVersion)
    issues.push(
      `Reviewed version ${review.expectedVersion}; native receipt started from ${receipt.version_before ?? 'unavailable'}.`,
    );
  if (
    receipt.version_after !== review.expectedVersion + 1 ||
    receipt.result?.version !== receipt.version_after
  )
    issues.push('Native result version does not match the reviewed transition.');
  if (
    receipt.aggregate_id !== review.params.orderId ||
    receipt.result?.id !== review.params.orderId
  )
    issues.push('Native result identifies a different or unavailable order.');
  if (receipt.result?.status !== review.params.status)
    issues.push('Native result status does not match the reviewed action.');
  if (
    receipt.economic_context?.principal?.id !== review.principalId ||
    receipt.economic_context?.store_id !== review.storeId
  )
    issues.push('Native receipt scope does not match the reviewed operator and store.');
  return issues;
}

function receiptEvidence(receipt) {
  if (!receipt) return null;
  return Object.fromEntries(
    [
      'command_id',
      'idempotency_key',
      'command_type',
      'status',
      'audit_hash',
      'error_code',
      'error_message',
      'retry',
      'aggregate_id',
      'version_before',
      'version_after',
      'event_ids',
      'completed_at',
    ].map((key) => [key, receipt[key]]),
  );
}

// Tickets are held by the host, not accepted from UI arguments or rehydrated
// from editable workspace JSON. Restart recovery only looks up a native receipt.
export class OrderOperations {
  constructor({ dbPath, allowApply, loadKernel, directory, invoke }) {
    this.dbPath = dbPath ? path.resolve(dbPath) : null;
    this.allowApply = allowApply === true && Boolean(this.dbPath);
    this.loadKernel = loadKernel || (() => null);
    this.directory = directory;
    this.invoke = invoke;
    this.tickets = new Map();
    this.pending = new Set();
    this.applying = false;
  }
  run(operation, payload) {
    const task = this[operation](payload);
    this.pending.add(task);
    void task.finally(() => this.pending.delete(task)).catch(() => {});
    return task;
  }
  async settle() {
    await Promise.allSettled([...this.pending]);
  }
  async configuration() {
    const kernel = structuredClone(await this.loadKernel());
    if (!kernel || kernel.strict !== true) throw new Error(configurationMessage);
    return kernel;
  }
  async preview(payload) {
    if (this.applying) throw new Error('Wait for the active order action.');
    if (!this.dbPath)
      throw new Error(
        'Order actions require --db with an existing store and trusted kernel configuration. The demo remains read-only.',
      );
    const params = orderAction(payload);
    const kernel = await this.configuration();
    const id = randomUUID();
    const idempotencyKey = `tui-order:${id}`;
    const data = await this.invoke('order-preview', { params, idempotencyKey }, kernel);
    const ticket = {
      id,
      idempotencyKey,
      params,
      order: data.order,
      expectedVersion: data.receipt?.version_before,
      configHash: digest(kernel),
      principalId: kernel.principal.id,
      storeId: kernel.storeId,
      source: this.dbPath,
      previewReceipt: data.receipt,
      createdAt: new Date().toISOString(),
    };
    if (data.receipt?.status === 'previewed' && Number.isSafeInteger(ticket.expectedVersion)) {
      this.tickets.set(id, ticket);
      if (this.tickets.size > 20) this.tickets.delete(this.tickets.keys().next().value);
    }
    return {
      ...data,
      ticketId: id,
      idempotencyKey,
      principalId: kernel.principal.id,
      storeId: kernel.storeId,
      applyAllowed: this.allowApply && this.tickets.has(id),
    };
  }
  async journal(record) {
    if (!this.directory)
      throw new Error('A durable workspace is required before applying an order action.');
    await fs.mkdir(this.directory, { recursive: true, mode: 0o700 });
    if ((await fs.lstat(this.directory)).isSymbolicLink())
      throw new Error('Operation workspace must not be a symlink.');
    const temporary = path.join(this.directory, `.order-action-${randomUUID()}.tmp`);
    try {
      const file = await fs.open(temporary, 'wx', 0o600);
      try {
        await file.writeFile(JSON.stringify(record, null, 2) + '\n');
        await file.sync();
      } finally {
        await file.close();
      }
      // Each intent keeps its own record even when another TUI session updates
      // the last-action pointer. Workspace JSON is never replay authority.
      await fs.rename(temporary, path.join(this.directory, `order-action-${record.id}.json`));
      const pointer = await fs.open(temporary, 'wx', 0o600);
      try {
        await pointer.writeFile(JSON.stringify(record, null, 2) + '\n');
        await pointer.sync();
      } finally {
        await pointer.close();
      }
      await fs.rename(temporary, path.join(this.directory, 'last-order-action.json'));
      if (process.platform !== 'win32') {
        const directory = await fs.open(this.directory, 'r');
        try {
          await directory.sync();
        } finally {
          await directory.close();
        }
      }
    } finally {
      await fs.rm(temporary, { force: true });
    }
  }
  async apply(payload) {
    if (!this.allowApply)
      throw new Error('Live order changes require --db and --apply. Preview only.');
    if (!payload || Object.keys(payload).length !== 1 || typeof payload.ticketId !== 'string')
      throw new Error('Apply accepts only a host-issued preview ticket.');
    if (this.applying) throw new Error('An order action is already running; wait for its receipt.');
    const ticket = this.tickets.get(payload.ticketId);
    if (!ticket) throw new Error('Preview ticket is unavailable. Review the order again.');
    this.applying = true;
    try {
      const kernel = await this.configuration();
      if (digest(kernel) !== ticket.configHash)
        throw new Error('Operator identity or policy changed. Review the order again.');
      if (ticket.receipt)
        return {
          receipt: structuredClone(ticket.receipt),
          reviewIssues: [...ticket.reviewIssues],
          replayed: true,
        };
      const record = {
        version: 1,
        createdAt: ticket.createdAt,
        updatedAt: new Date().toISOString(),
        source: this.dbPath,
        id: ticket.id,
        idempotencyKey: ticket.idempotencyKey,
        params: ticket.params,
        expectedVersion: ticket.expectedVersion,
        configHash: ticket.configHash,
        principalId: kernel.principal.id,
        storeId: kernel.storeId,
        state: 'pending',
        receipt: null,
      };
      await this.journal(record);
      let data;
      try {
        data = await this.invoke(
          'order-apply',
          {
            params: ticket.params,
            expectedVersion: ticket.expectedVersion,
            idempotencyKey: ticket.idempotencyKey,
          },
          kernel,
        );
        if (
          !data?.receipt ||
          data.receipt.idempotency_key !== ticket.idempotencyKey ||
          data.receipt.command_type !== 'orders.transition' ||
          !['succeeded', 'rejected', 'failed'].includes(data.receipt.status) ||
          (data.receipt.status !== 'succeeded' &&
            data.receipt.aggregate_id &&
            data.receipt.aggregate_id !== ticket.params.orderId)
        )
          throw new Error('The engine did not return a matching order receipt.');
      } catch (error) {
        error.outcomeUnknown = true;
        throw error;
      }
      ticket.receipt = structuredClone(data.receipt);
      ticket.reviewIssues = orderReceiptIssues(ticket.receipt, ticket);
      const outcome = { ...data, reviewIssues: [...ticket.reviewIssues] };
      // A journal failure after commit must retain the native outcome. The
      // pending entry remains sufficient to reconcile the same key on restart.
      try {
        await this.journal({
          ...record,
          state: 'finished',
          updatedAt: new Date().toISOString(),
          receipt: receiptEvidence(data.receipt),
          reviewIssues: ticket.reviewIssues,
        });
      } catch {
        return {
          ...outcome,
          journalWarning:
            'Native receipt returned; local history could not be updated. Reconcile this action after restart.',
        };
      }
      return outcome;
    } finally {
      this.applying = false;
    }
  }
  async historyContext() {
    if (this.applying) throw new Error('Wait for the active order action before reconciling.');
    if (!this.dbPath || !this.directory)
      throw new Error('Choose --db and the original workspace to reconcile an order action.');
    const kernel = await this.configuration();
    try {
      const directory = await fs.lstat(this.directory);
      if (!directory.isDirectory() || directory.isSymbolicLink())
        throw new Error('Operation workspace must be a directory, not a symlink.');
    } catch (error) {
      if (error.code !== 'ENOENT') throw error;
    }
    return kernel;
  }
  async readHistory(name, kernel, expectedId = null) {
    const file = await fs.open(
      path.join(this.directory, name),
      constants.O_RDONLY | (constants.O_NOFOLLOW || 0) | (constants.O_NONBLOCK || 0),
    );
    let record;
    try {
      const stat = await file.stat();
      if (!stat.isFile() || stat.size > 65536) throw new Error('Invalid order action history.');
      const buffer = Buffer.alloc(65537);
      let size = 0;
      while (size < buffer.length) {
        const { bytesRead } = await file.read(buffer, size, buffer.length - size, size);
        if (!bytesRead) break;
        size += bytesRead;
      }
      if (size > 65536) throw new Error('Invalid order action history.');
      record = JSON.parse(buffer.subarray(0, size).toString('utf8'));
    } finally {
      await file.close();
    }
    if (
      !record ||
      typeof record !== 'object' ||
      Array.isArray(record) ||
      record.version !== 1 ||
      !Number.isSafeInteger(record.expectedVersion) ||
      record.source !== this.dbPath ||
      record.principalId !== kernel.principal.id ||
      record.storeId !== kernel.storeId ||
      typeof record.id !== 'string' ||
      !uuid.test(record.id) ||
      (expectedId !== null && record.id !== expectedId) ||
      record.idempotencyKey !== `tui-order:${record.id}`
    )
      throw new Error('Order action history does not match this store and operator configuration.');
    orderAction(record.params);
    return record;
  }
  async history(payload = {}) {
    const invalid = () =>
      new Error(
        'Order history accepts only a valid page cursor and pending-only filter; no caller-supplied commands or paths.',
      );
    if (
      !payload ||
      typeof payload !== 'object' ||
      Array.isArray(payload) ||
      Object.keys(payload).some((key) => !['cursor', 'pendingOnly'].includes(key)) ||
      ('pendingOnly' in payload && typeof payload.pendingOnly !== 'boolean')
    )
      throw invalid();
    const cursor = payload.cursor ?? null,
      pendingOnly = payload.pendingOnly === true;
    let before = null;
    if (cursor !== null) {
      if (typeof cursor !== 'string' || cursor.length > 200 || !/^[A-Za-z0-9_-]+$/.test(cursor))
        throw invalid();
      try {
        before = JSON.parse(Buffer.from(cursor, 'base64url').toString('utf8'));
      } catch {
        throw invalid();
      }
      if (
        !Array.isArray(before) ||
        before.length !== 2 ||
        !Number.isFinite(before[0]) ||
        before[0] < -8640000000000000 ||
        before[0] > 8640000000000000 ||
        typeof before[1] !== 'string' ||
        !uuid.test(before[1])
      )
        throw invalid();
    }
    const page = { cursor, pendingOnly, nextCursor: null };
    const kernel = await this.historyContext();
    const candidates = [];
    let scanned = 0,
      skipped = 0,
      truncated = false;
    let directory;
    try {
      directory = await fs.opendir(this.directory);
    } catch (error) {
      if (error.code === 'ENOENT') return { ...page, entries: [], skipped: 0, truncated: false };
      throw error;
    }
    // Bound local filesystem work independently of the number of native receipts.
    // Never trust local receipt summaries as evidence of a committed outcome.
    for await (const item of directory) {
      if (++scanned > 2000) {
        truncated = true;
        break;
      }
      const match = /^order-action-([0-9a-f-]{36})\.json$/i.exec(item.name);
      if (!match || !uuid.test(match[1])) continue;
      if (!item.isFile()) {
        skipped++;
        continue;
      }
      try {
        const stat = await fs.lstat(path.join(this.directory, item.name));
        if (!stat.isFile() || stat.size > 65536) {
          skipped++;
          continue;
        }
        candidates.push({ name: item.name, id: match[1], modified: stat.mtimeMs });
      } catch {
        skipped++;
      }
    }
    candidates.sort((a, b) => b.modified - a.modified || a.id.localeCompare(b.id));
    const entries = [];
    let last = null;
    for (const candidate of candidates) {
      // Seek after the last displayed file rather than skipping a row count:
      // a newer intent must not shift or duplicate the next page.
      if (
        before &&
        !(
          candidate.modified < before[0] ||
          (candidate.modified === before[0] && candidate.id.localeCompare(before[1]) > 0)
        )
      )
        continue;
      try {
        const record = await this.readHistory(candidate.name, kernel, candidate.id);
        if (pendingOnly && record.state !== 'pending') continue;
        if (entries.length === 25) {
          page.nextCursor = Buffer.from(JSON.stringify([last.modified, last.id])).toString(
            'base64url',
          );
          break;
        }
        last = candidate;
        entries.push({
          id: record.id,
          orderId: record.params.orderId,
          status: record.params.status,
          localState: record.state === 'pending' ? 'pending' : 'recorded',
          recordedAt: new Date(candidate.modified).toISOString(),
          idempotencyKey: record.idempotencyKey,
        });
      } catch {
        skipped++;
      }
    }
    return { ...page, entries, skipped, truncated };
  }
  async reconcile(payload = {}) {
    if (
      !payload ||
      typeof payload !== 'object' ||
      Array.isArray(payload) ||
      Object.keys(payload).some((key) => key !== 'historyId') ||
      ('historyId' in payload &&
        (typeof payload.historyId !== 'string' || !uuid.test(payload.historyId)))
    )
      throw new Error(
        'Receipt reconciliation accepts no caller-supplied commands or paths; choose a valid history ID.',
      );
    const kernel = await this.historyContext();
    let record;
    try {
      record = await this.readHistory(
        payload.historyId ? `order-action-${payload.historyId}.json` : 'last-order-action.json',
        kernel,
        payload.historyId || null,
      );
    } catch (error) {
      if (error.code === 'ENOENT')
        throw new Error('No previous order action is recorded in this workspace.');
      throw error;
    }
    const data = await this.invoke(
      'order-reconcile',
      { idempotencyKey: record.idempotencyKey },
      kernel,
    );
    if (data.receipt?.aggregate_id && data.receipt.aggregate_id !== record.params.orderId)
      throw new Error('Native receipt does not match the recorded order action.');
    return {
      ...data,
      reviewIssues: orderReceiptIssues(data.receipt, record),
      idempotencyKey: record.idempotencyKey,
      params: record.params,
      message: data.receipt
        ? 'Native receipt recovered. No command was repeated.'
        : 'No native receipt found. No command was repeated; inspect the order before starting a new action.',
    };
  }
}

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
      source: this.dbPath,
      previewReceipt: data.receipt,
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
      if (ticket.receipt) return { receipt: structuredClone(ticket.receipt), replayed: true };
      const record = {
        version: 1,
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
          (data.receipt.aggregate_id && data.receipt.aggregate_id !== ticket.params.orderId)
        )
          throw new Error('The engine did not return a matching order receipt.');
      } catch (error) {
        error.outcomeUnknown = true;
        throw error;
      }
      ticket.receipt = structuredClone(data.receipt);
      // A journal failure after commit must retain the native outcome. The
      // pending entry remains sufficient to reconcile the same key on restart.
      try {
        await this.journal({
          ...record,
          state: 'finished',
          receipt: receiptEvidence(data.receipt),
        });
      } catch {
        return {
          ...data,
          journalWarning:
            'Native receipt returned; local history could not be updated. Reconcile this action after restart.',
        };
      }
      return data;
    } finally {
      this.applying = false;
    }
  }
  async reconcile(payload = {}) {
    if (Object.keys(payload).length)
      throw new Error('Receipt reconciliation accepts no caller-supplied commands.');
    if (this.applying) throw new Error('Wait for the active order action before reconciling.');
    if (!this.dbPath || !this.directory)
      throw new Error('Choose --db and the original workspace to reconcile an order action.');
    const kernel = await this.configuration();
    let file;
    try {
      file = await fs.open(
        path.join(this.directory, 'last-order-action.json'),
        constants.O_RDONLY | (constants.O_NOFOLLOW || 0),
      );
    } catch (error) {
      if (error.code === 'ENOENT')
        throw new Error('No previous order action is recorded in this workspace.');
      throw error;
    }
    let record;
    try {
      const stat = await file.stat();
      if (!stat.isFile() || stat.size > 65536) throw new Error('Invalid order action history.');
      record = JSON.parse(await file.readFile('utf8'));
    } finally {
      await file.close();
    }
    if (
      record.version !== 1 ||
      record.source !== this.dbPath ||
      record.principalId !== kernel.principal.id ||
      record.storeId !== kernel.storeId ||
      !uuid.test(record.id) ||
      record.idempotencyKey !== `tui-order:${record.id}`
    )
      throw new Error('Order action history does not match this store and operator configuration.');
    orderAction(record.params);
    const data = await this.invoke(
      'order-reconcile',
      { idempotencyKey: record.idempotencyKey },
      kernel,
    );
    if (data.receipt?.aggregate_id && data.receipt.aggregate_id !== record.params.orderId)
      throw new Error('Native receipt does not match the recorded order action.');
    return {
      ...data,
      idempotencyKey: record.idempotencyKey,
      params: record.params,
      message: data.receipt
        ? 'Native receipt recovered. No command was repeated.'
        : 'No native receipt found. No command was repeated; inspect the order before starting a new action.',
    };
  }
}

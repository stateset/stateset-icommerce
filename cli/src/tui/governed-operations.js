import fs from 'node:fs/promises';
import { constants } from 'node:fs';
import path from 'node:path';
import { createHash, randomUUID } from 'node:crypto';

const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const digest = (value) => createHash('sha256').update(JSON.stringify(value)).digest('hex');

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
export class GovernedOperations {
  constructor({ dbPath, allowApply, loadKernel, directory, invoke, gate }, workflow) {
    this.workflow = workflow;
    this.name = workflow.name;
    this.title = workflow.title;
    this.gate = gate || { active: false };
    this.dbPath = dbPath ? path.resolve(dbPath) : null;
    this.allowApply = allowApply === true && Boolean(this.dbPath);
    this.loadKernel = loadKernel || (() => null);
    this.directory = directory;
    this.invoke = invoke;
    this.tickets = new Map();
    this.pending = new Set();
  }
  get applying() {
    return this.gate.active;
  }
  set applying(value) {
    this.gate.active = value;
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
    if (!kernel || kernel.strict !== true)
      throw new Error(
        `${this.title} actions need operator-owned --kernel-policy, --kernel-principal and --kernel-store-id. Live changes also require --db and --apply.`,
      );
    return kernel;
  }
  async preview(payload) {
    if (this.applying) throw new Error(`Wait for the active ${this.name} action.`);
    if (!this.dbPath)
      throw new Error(
        `${this.title} actions require --db with an existing store and trusted kernel configuration. The demo remains read-only.`,
      );
    const params = this.workflow.validate(payload);
    const kernel = await this.configuration();
    const id = randomUUID();
    const idempotencyKey = `tui-${this.name}:${id}`;
    const data = await this.invoke(`${this.name}-preview`, { params, idempotencyKey }, kernel);
    const ticket = {
      id,
      idempotencyKey,
      params,
      context: this.workflow.context(data),
      expectedVersion: data.receipt?.version_before ?? null,
      configHash: digest(kernel),
      principalId: kernel.principal.id,
      storeId: kernel.storeId,
      source: this.dbPath,
      previewReceipt: data.receipt,
      createdAt: new Date().toISOString(),
    };
    if (
      data.receipt?.status === 'previewed' &&
      this.workflow.validVersion(ticket.expectedVersion)
    ) {
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
      throw new Error(`A durable workspace is required before applying a ${this.name} action.`);
    await fs.mkdir(this.directory, { recursive: true, mode: 0o700 });
    if ((await fs.lstat(this.directory)).isSymbolicLink())
      throw new Error('Operation workspace must not be a symlink.');
    const temporary = path.join(this.directory, `.${this.name}-action-${randomUUID()}.tmp`);
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
      await fs.rename(
        temporary,
        path.join(this.directory, `${this.name}-action-${record.id}.json`),
      );
      const pointer = await fs.open(temporary, 'wx', 0o600);
      try {
        await pointer.writeFile(JSON.stringify(record, null, 2) + '\n');
        await pointer.sync();
      } finally {
        await pointer.close();
      }
      await fs.rename(temporary, path.join(this.directory, `last-${this.name}-action.json`));
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
      throw new Error(`Live ${this.name} changes require --db and --apply. Preview only.`);
    if (!payload || Object.keys(payload).length !== 1 || typeof payload.ticketId !== 'string')
      throw new Error('Apply accepts only a host-issued preview ticket.');
    if (this.applying)
      throw new Error(`An active ${this.name} action is already running; wait for its receipt.`);
    const ticket = this.tickets.get(payload.ticketId);
    if (!ticket) throw new Error(`Preview ticket is unavailable. Review the ${this.name} again.`);
    this.applying = true;
    try {
      const kernel = await this.configuration();
      if (digest(kernel) !== ticket.configHash)
        throw new Error(`Operator identity or policy changed. Review the ${this.name} again.`);
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
        context: ticket.context,
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
          `${this.name}-apply`,
          {
            params: ticket.params,
            expectedVersion: ticket.expectedVersion,
            ...(this.workflow.contextInApply ? { context: ticket.context } : {}),
            idempotencyKey: ticket.idempotencyKey,
          },
          kernel,
        );
        if (
          !data?.receipt ||
          data.receipt.idempotency_key !== ticket.idempotencyKey ||
          data.receipt.command_type !== this.workflow.commandType ||
          !['succeeded', 'rejected', 'failed'].includes(data.receipt.status) ||
          (data.receipt.status !== 'succeeded' &&
            !this.workflow.matchesAggregate(data.receipt, ticket.params))
        )
          throw new Error(`The engine did not return a matching ${this.name} receipt.`);
      } catch (error) {
        error.outcomeUnknown = true;
        throw error;
      }
      ticket.receipt = structuredClone(data.receipt);
      ticket.reviewIssues = this.workflow.receiptIssues(ticket.receipt, ticket);
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
    if (this.applying)
      throw new Error(`Wait for the active ${this.name} action before reconciling.`);
    if (!this.dbPath || !this.directory)
      throw new Error(`Choose --db and the original workspace to reconcile a ${this.name} action.`);
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
      if (!stat.isFile() || stat.size > 65536)
        throw new Error(`Invalid ${this.name} action history.`);
      const buffer = Buffer.alloc(65537);
      let size = 0;
      while (size < buffer.length) {
        const { bytesRead } = await file.read(buffer, size, buffer.length - size, size);
        if (!bytesRead) break;
        size += bytesRead;
      }
      if (size > 65536) throw new Error(`Invalid ${this.name} action history.`);
      record = JSON.parse(buffer.subarray(0, size).toString('utf8'));
    } finally {
      await file.close();
    }
    if (
      !record ||
      typeof record !== 'object' ||
      Array.isArray(record) ||
      record.version !== 1 ||
      !this.workflow.validVersion(record.expectedVersion) ||
      !this.workflow.validContext(record.context) ||
      record.source !== this.dbPath ||
      record.principalId !== kernel.principal.id ||
      record.storeId !== kernel.storeId ||
      typeof record.id !== 'string' ||
      !uuid.test(record.id) ||
      (expectedId !== null && record.id !== expectedId) ||
      record.idempotencyKey !== `tui-${this.name}:${record.id}`
    )
      throw new Error(
        `${this.title} action history does not match this store and operator configuration.`,
      );
    this.workflow.validate(record.params);
    return record;
  }
  async history(payload = {}) {
    const invalid = () =>
      new Error(
        `${this.title} history accepts only a valid page cursor and pending-only filter; no caller-supplied commands or paths.`,
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
      const match = new RegExp(`^${this.name}-action-([0-9a-f-]{36})\\.json$`, 'i').exec(item.name);
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
          ...this.workflow.historySummary(record.params, record.context),
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
        payload.historyId
          ? `${this.name}-action-${payload.historyId}.json`
          : `last-${this.name}-action.json`,
        kernel,
        payload.historyId || null,
      );
    } catch (error) {
      if (error.code === 'ENOENT')
        throw new Error(`No previous ${this.name} action is recorded in this workspace.`);
      throw error;
    }
    const data = await this.invoke(
      `${this.name}-reconcile`,
      { idempotencyKey: record.idempotencyKey },
      kernel,
    );
    if (
      data.receipt &&
      (data.receipt.idempotency_key !== record.idempotencyKey ||
        data.receipt.command_type !== this.workflow.commandType ||
        !this.workflow.matchesAggregate(data.receipt, record.params))
    )
      throw new Error(`Native receipt does not match the recorded ${this.name} action.`);
    return {
      ...data,
      reviewIssues: this.workflow.receiptIssues(data.receipt, record),
      idempotencyKey: record.idempotencyKey,
      params: record.params,
      message: data.receipt
        ? 'Native receipt recovered. No command was repeated.'
        : `No native receipt found. No command was repeated; inspect the ${this.workflow.resource} before starting a new action.`,
    };
  }
}

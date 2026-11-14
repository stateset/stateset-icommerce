/**
 * Agent session report.
 *
 * Reads the agentic replay log (`agentic-tool-calls.jsonl`, written by
 * ./replay-log.js from ./tool-dispatch.js, ./plan-executor.js and
 * ./mutation-simulator.js) and answers the operator's questions: what did
 * each session do, what did it commit, what was refused and why, what did it
 * cost, how slow was it. Read-only: nothing here writes to the log or a store.
 *
 * Event shape, as written today (one JSON object per line):
 *
 *   eventId, tool, status, requestId, sessionId, policyDomain, occurredAt,
 *   elapsedMs, params, paramsHash, result, resultHash, policy, permission,
 *   charge, error, notes, source, agentic, eventSignature[, kernel]
 *
 * Writers (`source`):
 *   - `mcp_server`: one MCP tool call. `result.structuredContent` is
 *     `{ ok, preview, error: { code, message }, result: <tool result> }`, and
 *     `notes.mutationManifest` is an object exactly when the tool is a write.
 *   - `embedded_agent_toolkit`: one `server.executeTool` call; `result` is the
 *     tool result. Its `sessionId` defaults to its `requestId`.
 *   - `agentic_execute_plan`: a plan run, all events sharing the plan's
 *     `requestId`. One event per step (`notes.executedBy`, the step's tool in
 *     `notes.sourceStep` / `params.step`, the step outcome in `result`, so a
 *     governed step's receipt is at `result.result.receipt`), one per rollback
 *     compensation (`notes.phase: "rollback"`), then the plan's own record
 *     (`notes.final: true`, `status` = success | failed | dry_run).
 *   - `agentic_simulate_mutation` / `agentic_replay_mutation`: the record of
 *     one simulate/replay call (a replay's outcome is at
 *     `result.replayOutcome`).
 * A plan, simulate or replay call made through MCP or `executeTool` is logged
 * twice: the record above, then the ordinary wrapper event for the tool call,
 * whose tool result carries the record's `requestId`
 * (`result.structuredContent.result.requestId`, or `result.requestId` on the
 * toolkit path). The report counts such a call once.
 *
 * Kernel decisions:
 *   - A governed call's tool result carries the kernel receipt (`status`,
 *     `command_id`, `error_code`, `policy.reason_codes`,
 *     `economic_context.principal`). A kernel refusal is logged with
 *     `status: "error"`, not `policy_block`; only the receipt says it was a
 *     refusal.
 *   - The replay sanitizer bounds depth, so on `mcp_server` and plan-step
 *     events the receipt's nested `policy` and `economic_context` are the
 *     string "[truncated]": `error_code` survives, `reason_codes` and the
 *     principal do not. Such refusals are grouped by `error_code` and counted
 *     in `denials.reasonCodesUnavailable`.
 *   - Newer writers add a top-level `kernel` block to governed events, lifted
 *     from the receipt BEFORE sanitisation: `{ commandId, correlationId,
 *     receiptStatus, errorCode, reasonCodes, reasonCodesOmitted, principalId,
 *     auditHash }`, each present only when the receipt had it, absent on
 *     ungoverned calls. When present it is preferred to the receipt.
 *   - A retry that reuses an idempotency key gets the ORIGINAL receipt back,
 *     so its event repeats an earlier (commandId, receipt status). Such an
 *     event is counted as `replayed`, not as a new write or refusal.
 *
 * `charge` is `{ charged, rule: { tool, chainId, tokenSymbol, amount }, mpp }`.
 * `rule.amount` is the operator's pricing rule verbatim: a JSON number or a
 * decimal string. Charges are totalled as recorded, replayed calls included,
 * because the treasury recorded every one of them.
 */

import { createReadStream } from 'node:fs';
import { createInterface } from 'node:readline';

import { addDecimals } from '../utils/exact-decimal.js';

/**
 * The bucket for events without a real session: no `sessionId`, or a
 * `sessionId` that is just the event's own `requestId` (the toolkit and plan
 * default, which would otherwise make every call a session of its own).
 */
export const UNSESSIONED = '(unsessioned)';

/** Tools whose call writes its own record before the wrapper event. */
const COMPOSITE_TOOLS = new Set([
  'agentic_execute_plan',
  'agentic_simulate_mutation',
  'agentic_replay_mutation',
]);

/** Most composite records awaiting their wrapper event. */
const MAX_PENDING_COMPOSITES = 10_000;
/** Most (commandId, receipt status) pairs remembered to recognise a retry. */
export const MAX_TRACKED_RECEIPTS = 200_000;

/** Statuses a gate (hook, policy, permission, payment, treasury) refuses with. */
const GATE_DENIAL_STATUSES = new Set([
  'blocked',
  'policy_block',
  'permission_block',
  'payment_required',
  'treasury_block',
]);

const ERROR_STATUSES = new Set(['error', 'invalid', 'rollback_failed']);
const SUCCESS_STATUSES = new Set(['success', 'rollback_success']);
const DRY_RUN_STATUSES = new Set(['dry_run_success', 'dry_run']);

/** Every outcome an operation can have; they partition the operations. */
const OUTCOMES = Object.freeze([
  'executed',
  'previewed',
  'dryRun',
  'ok',
  'denied',
  'error',
  'replayed',
  'other',
]);

const isPlainObject = (value) =>
  value !== null && typeof value === 'object' && !Array.isArray(value);

const nonEmptyString = (value) => (typeof value === 'string' && value.trim() ? value : null);

/**
 * A count map. A `Map`, so a key such as `constructor` or `__proto__` (a tool,
 * status, reason or error code is whatever the log says) is just a key.
 */
const bump = (counts, key, by = 1) => {
  counts.set(key, (counts.get(key) || 0) + by);
};

/** Counts as a plain object, largest first, ties by key. */
const sortedCounts = (counts) =>
  Object.fromEntries(
    [...counts.entries()].sort(([a, x], [b, y]) => y - x || (a < b ? -1 : a > b ? 1 : 0)),
  );

/** A Set that forgets its oldest entries past `limit`. */
function createBoundedSet(limit) {
  const entries = new Set();
  let evicted = 0;
  return {
    has: (key) => entries.has(key),
    delete: (key) => entries.delete(key),
    add(key) {
      if (entries.has(key)) return;
      entries.add(key);
      if (entries.size > limit) {
        entries.delete(entries.values().next().value);
        evicted += 1;
      }
    },
    get evicted() {
      return evicted;
    },
  };
}

const looksLikeReceipt = (value) =>
  isPlainObject(value) &&
  typeof value.status === 'string' &&
  ('receipt_id' in value || 'error_code' in value || 'command_type' in value);

/**
 * The kernel receipt an event carries, wherever the writer put it.
 *
 * @param {object} event
 * @returns {object|null}
 */
export function findKernelReceipt(event) {
  const result = event?.result;
  if (!isPlainObject(result)) return null;
  const toolResult = isPlainObject(result.structuredContent)
    ? result.structuredContent.result
    : result;
  if (isPlainObject(toolResult)) {
    if (looksLikeReceipt(toolResult.receipt)) return toolResult.receipt;
    if (looksLikeReceipt(toolResult) && typeof toolResult.receipt_id === 'string') {
      return toolResult;
    }
  }
  // A plan step logs its step outcome, whose `result` is the tool result.
  if (isPlainObject(result.result) && looksLikeReceipt(result.result.receipt)) {
    return result.result.receipt;
  }
  // A replay logs `{ replayOutcome, deterministic }`; the outcome is a step.
  const replayed = result.replayOutcome?.result;
  if (isPlainObject(replayed) && looksLikeReceipt(replayed.receipt)) return replayed.receipt;
  return null;
}

/**
 * The top-level kernel block a newer writer attaches to a governed event, or
 * null for an ungoverned event or an older log.
 *
 * @param {object} event
 * @returns {object|null}
 */
export function findKernelBlock(event) {
  return isPlainObject(event?.kernel) ? event.kernel : null;
}

const stringsOf = (value) =>
  Array.isArray(value) ? value.map(nonEmptyString).filter(Boolean) : [];

/**
 * What kind of record an event is.
 *
 *   call    - one tool call (MCP or executeTool)
 *   step    - one step (or rollback compensation) of a plan
 *   plan    - a plan's own record
 *   record  - a simulate/replay call's own record
 *   wrapper - the MCP/executeTool event of a plan, simulate or replay call
 *
 * @param {object} event
 * @returns {'call'|'step'|'plan'|'record'|'wrapper'}
 */
export function eventKind(event) {
  const source = event?.source;
  if (source === 'agentic_execute_plan') return event.notes?.final === true ? 'plan' : 'step';
  if (source === 'agentic_simulate_mutation' || source === 'agentic_replay_mutation') {
    return 'record';
  }
  if (COMPOSITE_TOOLS.has(event?.tool)) return 'wrapper';
  return 'call';
}

/** The tool an event is about: a plan step's or compensation's own tool. */
export function attributedTool(event) {
  if (eventKind(event) === 'step') {
    const tool =
      event.notes?.phase === 'rollback'
        ? nonEmptyString(event.params?.compensationTool) || nonEmptyString(event.notes?.source)
        : nonEmptyString(event.notes?.sourceStep) || nonEmptyString(event.params?.step);
    if (tool) return tool;
  }
  return nonEmptyString(event?.tool) || '(unknown)';
}

/** The session bucket an event belongs to. */
export function sessionKeyOf(event) {
  const sessionId = nonEmptyString(event?.sessionId);
  if (!sessionId || sessionId === nonEmptyString(event?.requestId)) return UNSESSIONED;
  return sessionId;
}

const REQUEST_ID_IN_TEXT = /"requestId"\s*:\s*"([^"]{1,128})"/;

/**
 * The `requestId` of the composite record a wrapper event's tool result names.
 * Older writers kept only the text content (truncated, but the id comes
 * first), so that is read as a fallback.
 */
function linkedRequestIdOf(event) {
  const result = event?.result;
  if (!isPlainObject(result)) return null;
  const structured = result.structuredContent?.result;
  const direct =
    (isPlainObject(structured) && nonEmptyString(structured.requestId)) ||
    nonEmptyString(result.requestId);
  if (direct) return direct;
  const first = Array.isArray(result.content)
    ? result.content[0]
    : isPlainObject(result.content)
      ? result.content[0]
      : null;
  const text = isPlainObject(first) ? nonEmptyString(first.text) : null;
  return text ? (REQUEST_ID_IN_TEXT.exec(text)?.[1] ?? null) : null;
}

/** The (commandId, receipt status) pair a governed event carries, or null. */
function receiptKeyOf(event) {
  const kernel = findKernelBlock(event);
  const receipt = findKernelReceipt(event);
  const commandId = nonEmptyString(kernel?.commandId) || nonEmptyString(receipt?.command_id);
  if (!commandId) return null;
  const status = nonEmptyString(kernel?.receiptStatus) || nonEmptyString(receipt?.status) || '';
  return `${commandId}\u0000${status}`;
}

/** The structured error code an event carries, or null. Never parsed from prose. */
function errorCodeOf(event, receipt, kernel) {
  const result = isPlainObject(event?.result) ? event.result : null;
  const contract = isPlainObject(result?.structuredContent) ? result.structuredContent : null;
  const toolResult = contract ? contract.result : result;
  const candidates = [
    isPlainObject(contract?.error) ? contract.error.code : null,
    receipt?.error_code,
    kernel?.errorCode,
    isPlainObject(toolResult) ? toolResult.errorCode : null,
    isPlainObject(toolResult) ? toolResult.error_code : null,
    isPlainObject(toolResult) && isPlainObject(toolResult.error) ? toolResult.error.code : null,
    isPlainObject(event?.error) ? event.error.code : null,
  ];
  return candidates.map(nonEmptyString).find(Boolean) ?? null;
}

/**
 * Whether a successful operation is shown by the log to have committed a
 * write: a `succeeded` kernel receipt, a mutation manifest on a call or step
 * (written only for write tools), or a replay record (only write calls are
 * replayed). An ungoverned `executeTool` write carries neither and counts as
 * `ok`.
 */
function committedWrite(event, kind, receiptStatus) {
  if (receiptStatus === 'succeeded') return true;
  if (kind === 'record') return event.source === 'agentic_replay_mutation';
  if (kind === 'wrapper') return false; // the plan's steps carry its writes
  return isPlainObject(event.notes?.mutationManifest);
}

/**
 * Classify one operation (anything but a plan's own record, whose outcome is
 * its steps').
 *
 * @param {object} event
 * @returns {{ outcome: 'executed'|'previewed'|'dryRun'|'ok'|'denied'|'error'|'other',
 *             reasons?: string[], reasonCodesUnavailable?: boolean,
 *             reasonCodesOmitted?: number, code?: string }}
 */
export function classifyEvent(event) {
  const kind = eventKind(event);
  const status = nonEmptyString(event?.status) || '(unknown)';
  const receipt = findKernelReceipt(event);
  const kernel = findKernelBlock(event);
  const receiptStatus = nonEmptyString(kernel?.receiptStatus) || receipt?.status || null;

  // A kernel refusal: logged as `error` (or `preview`), named by the kernel
  // block when the writer added one, else by the receipt.
  if (receiptStatus === 'rejected') {
    const policy = receipt?.policy;
    const kernelCodes = stringsOf(kernel?.reasonCodes);
    const reasonCodes =
      kernelCodes.length > 0
        ? kernelCodes
        : isPlainObject(policy)
          ? stringsOf(policy.reason_codes)
          : [];
    const omitted = kernel?.reasonCodesOmitted;
    const extra = Number.isInteger(omitted) && omitted > 0 ? { reasonCodesOmitted: omitted } : {};
    if (reasonCodes.length > 0) return { outcome: 'denied', reasons: reasonCodes, ...extra };
    return {
      outcome: 'denied',
      reasons: [
        nonEmptyString(kernel?.errorCode) ||
          nonEmptyString(receipt?.error_code) ||
          'kernel.rejected (no error_code)',
      ],
      // Unavailable when the sanitizer cut the receipt's policy decision
      // (or its reason codes) and no kernel block was written. A block
      // without reason codes means the receipt had none.
      reasonCodesUnavailable:
        !kernel && (!isPlainObject(policy) || typeof policy.reason_codes === 'string'),
      ...extra,
    };
  }

  if (GATE_DENIAL_STATUSES.has(status)) return { outcome: 'denied', reasons: [status] };
  if (status === 'dry_run_blocked' && event?.permission?.preview !== true) {
    return { outcome: 'denied', reasons: [status] };
  }

  const contractFailed = event?.result?.structuredContent?.ok === false;
  if (ERROR_STATUSES.has(status) || receiptStatus === 'failed' || contractFailed) {
    return { outcome: 'error', code: errorCodeOf(event, receipt, kernel) || `(no code) ${status}` };
  }
  if (receiptStatus === 'previewed' || status === 'preview' || status === 'dry_run_blocked') {
    return { outcome: 'previewed' };
  }
  if (DRY_RUN_STATUSES.has(status)) return { outcome: 'dryRun' };
  if (SUCCESS_STATUSES.has(status)) {
    return { outcome: committedWrite(event, kind, receiptStatus) ? 'executed' : 'ok' };
  }
  return { outcome: 'other' };
}

/** Nearest-rank percentile of an ascending array; null when empty. */
function percentile(sorted, q) {
  if (sorted.length === 0) return null;
  return sorted[Math.max(0, Math.ceil(q * sorted.length) - 1)];
}

function createAccumulator() {
  return {
    calls: 0,
    steps: 0,
    collapsedWrappers: 0,
    byStatus: new Map(),
    byTool: new Map(),
    bySource: new Map(),
    outcomes: Object.fromEntries(OUTCOMES.map((outcome) => [outcome, 0])),
    plans: { count: 0, byFinalStatus: new Map() },
    denials: { total: 0, byReason: new Map(), reasonCodesUnavailable: 0, reasonCodesOmitted: 0 },
    errors: { total: 0, byCode: new Map() },
    charges: {
      count: 0,
      onReplayedReceipts: 0,
      numericAmounts: 0,
      unparseableAmounts: 0,
      byToken: new Map(),
    },
    principals: new Set(),
    firstAt: null,
    lastAt: null,
    firstMs: Infinity,
    lastMs: -Infinity,
    latencies: [],
  };
}

function accumulateCharge(acc, event, replayed) {
  const charge = event.charge;
  if (!isPlainObject(charge) || charge.charged !== true) return;
  acc.charges.count += 1;
  if (replayed) acc.charges.onReplayedReceipts += 1;
  const rule = isPlainObject(charge.rule) ? charge.rule : {};
  const tokenSymbol = nonEmptyString(rule.tokenSymbol) || '(unknown token)';
  const chainId = nonEmptyString(rule.chainId) || '(unknown chain)';
  const key = `${tokenSymbol}\u0000${chainId}`;
  const bucket = acc.charges.byToken.get(key) || { tokenSymbol, chainId, count: 0, total: '0' };
  try {
    // Exact: scaled BigInts. A JSON number enters via its shortest
    // round-trip string, which is what the pricing rule said.
    bucket.total = addDecimals(bucket.total, rule.amount);
    bucket.count += 1;
    if (typeof rule.amount === 'number') acc.charges.numericAmounts += 1;
    acc.charges.byToken.set(key, bucket);
  } catch {
    acc.charges.unparseableAmounts += 1;
  }
}

/**
 * Add one event to an accumulator.
 *
 * @param {ReturnType<typeof createAccumulator>} acc
 * @param {object} event
 * @param {{ kind: string, collapsed: boolean, replayed: boolean }} how
 */
function accumulate(acc, event, { kind, collapsed, replayed }) {
  if (collapsed) {
    // The composite record already counted this call; keep what only the
    // wrapper has: the charge for the tool call itself.
    acc.collapsedWrappers += 1;
    accumulateCharge(acc, event, false);
    return;
  }

  if (kind === 'step') acc.steps += 1;
  else acc.calls += 1;
  const status = nonEmptyString(event.status) || '(unknown)';
  bump(acc.byStatus, status);
  bump(acc.byTool, attributedTool(event));
  bump(acc.bySource, nonEmptyString(event.source) || '(unknown)');

  if (kind === 'plan') {
    acc.plans.count += 1;
    bump(acc.plans.byFinalStatus, status);
  } else if (replayed) {
    acc.outcomes.replayed += 1;
  } else {
    const verdict = classifyEvent(event);
    acc.outcomes[verdict.outcome] += 1;
    if (verdict.outcome === 'denied') {
      acc.denials.total += 1;
      for (const reason of verdict.reasons) bump(acc.denials.byReason, reason);
      if (verdict.reasonCodesUnavailable) acc.denials.reasonCodesUnavailable += 1;
      if (verdict.reasonCodesOmitted) acc.denials.reasonCodesOmitted += verdict.reasonCodesOmitted;
    } else if (verdict.outcome === 'error') {
      acc.errors.total += 1;
      bump(acc.errors.byCode, verdict.code);
    }
  }

  accumulateCharge(acc, event, replayed);

  const principalId =
    nonEmptyString(findKernelBlock(event)?.principalId) ||
    nonEmptyString(findKernelReceipt(event)?.economic_context?.principal?.id);
  if (principalId) acc.principals.add(principalId);

  const at = nonEmptyString(event.occurredAt);
  const ms = at ? Date.parse(at) : NaN;
  if (Number.isFinite(ms)) {
    if (ms < acc.firstMs) {
      acc.firstMs = ms;
      acc.firstAt = at;
    }
    if (ms > acc.lastMs) {
      acc.lastMs = ms;
      acc.lastAt = at;
    }
  }

  // One latency per call: a plan's record spans its steps.
  if (kind !== 'step' && typeof event.elapsedMs === 'number' && Number.isFinite(event.elapsedMs)) {
    acc.latencies.push(event.elapsedMs);
  }
}

function summarize(acc) {
  const sorted = [...acc.latencies].sort((a, b) => a - b);
  return {
    calls: acc.calls,
    steps: acc.steps,
    collapsedWrappers: acc.collapsedWrappers,
    outcomes: { ...acc.outcomes },
    plans: { count: acc.plans.count, byFinalStatus: sortedCounts(acc.plans.byFinalStatus) },
    byStatus: sortedCounts(acc.byStatus),
    byTool: sortedCounts(acc.byTool),
    bySource: sortedCounts(acc.bySource),
    denials: {
      total: acc.denials.total,
      byReason: sortedCounts(acc.denials.byReason),
      reasonCodesUnavailable: acc.denials.reasonCodesUnavailable,
      reasonCodesOmitted: acc.denials.reasonCodesOmitted,
    },
    errors: { total: acc.errors.total, byCode: sortedCounts(acc.errors.byCode) },
    charges: {
      count: acc.charges.count,
      byToken: [...acc.charges.byToken.values()]
        .map((bucket) => ({ ...bucket, total: addDecimals(bucket.total) }))
        .sort((a, b) =>
          `${a.tokenSymbol}@${a.chainId}` < `${b.tokenSymbol}@${b.chainId}` ? -1 : 1,
        ),
      onReplayedReceipts: acc.charges.onReplayedReceipts,
      numericAmounts: acc.charges.numericAmounts,
      unparseableAmounts: acc.charges.unparseableAmounts,
    },
    principals: [...acc.principals].sort(),
    firstAt: acc.firstAt,
    lastAt: acc.lastAt,
    latencyMs: {
      count: sorted.length,
      p50: percentile(sorted, 0.5),
      p95: percentile(sorted, 0.95),
      max: sorted.length ? sorted[sorted.length - 1] : null,
    },
  };
}

const isMalformed = (event) => !isPlainObject(event) || '_parseError' in event;

/**
 * Incremental report builder, for callers that stream. Memory grows with the
 * number of real sessions, not events: unsessioned events share one bucket,
 * and retry and wrapper matching remember bounded windows.
 *
 * @param {{ sessionId?: string|null }} [options] - report only this session
 *   (`"(unsessioned)"` selects the unsessioned bucket)
 */
export function createSessionReportBuilder({ sessionId = null } = {}) {
  const sessions = new Map();
  const totals = createAccumulator();
  const pendingComposites = createBoundedSet(MAX_PENDING_COMPOSITES);
  const seenReceipts = createBoundedSet(MAX_TRACKED_RECEIPTS);
  let malformedLines = 0;
  let filteredOut = 0;

  return {
    /** Add one parsed event. Anything that is not an event object is counted, not thrown. */
    add(event) {
      if (isMalformed(event)) {
        malformedLines += 1;
        return;
      }
      const kind = eventKind(event);

      // Matching happens before the session filter, so a filtered report
      // classifies each event exactly as the full report does.
      let collapsed = false;
      if (kind === 'wrapper') {
        const linked = linkedRequestIdOf(event);
        collapsed = Boolean(linked) && pendingComposites.delete(linked);
      } else if (kind === 'plan' || kind === 'record') {
        const requestId = nonEmptyString(event.requestId);
        if (requestId) pendingComposites.add(requestId);
      }
      let replayed = false;
      if (!collapsed && kind !== 'plan') {
        const receiptKey = receiptKeyOf(event);
        if (receiptKey) {
          replayed = seenReceipts.has(receiptKey);
          seenReceipts.add(receiptKey);
        }
      }

      const key = sessionKeyOf(event);
      if (sessionId && key !== sessionId) {
        filteredOut += 1;
        return;
      }
      let acc = sessions.get(key);
      if (!acc) {
        acc = createAccumulator();
        sessions.set(key, acc);
      }
      const how = { kind, collapsed, replayed };
      accumulate(acc, event, how);
      accumulate(totals, event, how);
    },
    finish() {
      return {
        filter: { sessionId: sessionId || null },
        malformedLines,
        filteredOut,
        sessionCount: sessions.size,
        retryWindowExceeded: seenReceipts.evicted > 0,
        totals: summarize(totals),
        // First-seen order.
        sessions: [...sessions.entries()].map(([key, acc]) => ({
          sessionId: key,
          unsessioned: key === UNSESSIONED,
          ...summarize(acc),
        })),
      };
    },
  };
}

/**
 * Build the report from parsed events. Pure: no I/O, no clock.
 *
 * Entries that are not event objects -- including the `{ _parseError, raw }`
 * placeholders `readReplayLogEvents` yields for lines that are not JSON --
 * are counted in `malformedLines` and skipped.
 *
 * @param {Iterable<unknown>} events
 * @param {{ sessionId?: string|null }} [options]
 */
export function buildSessionReport(events, options = {}) {
  const builder = createSessionReportBuilder(options);
  for (const event of events) builder.add(event);
  return builder.finish();
}

/**
 * Stream a JSONL replay log one line at a time. Blank lines are skipped; a
 * line that is not JSON is yielded as `{ _parseError, raw }` (the same
 * placeholder replay-log.js uses) so the consumer can count it.
 *
 * @param {string} logPath
 * @returns {AsyncGenerator<unknown>}
 */
export async function* readReplayLogEvents(logPath) {
  const input = createReadStream(logPath, { encoding: 'utf8' });
  const lines = createInterface({ input, crlfDelay: Infinity });
  try {
    for await (const line of lines) {
      if (!line.trim()) continue;
      try {
        yield JSON.parse(line);
      } catch (error) {
        yield { _parseError: error.message, raw: line.slice(0, 200) };
      }
    }
  } finally {
    lines.close();
    input.destroy();
  }
}

/**
 * Build the report for a replay log on disk without loading it whole.
 *
 * @param {string} logPath
 * @param {{ sessionId?: string|null }} [options]
 */
export async function buildSessionReportFromFile(logPath, options = {}) {
  const builder = createSessionReportBuilder(options);
  for await (const event of readReplayLogEvents(logPath)) builder.add(event);
  return builder.finish();
}

const countsLine = (counts) =>
  Object.entries(counts)
    .map(([key, n]) => `${key} ${n}`)
    .join(', ') || '-';

const chargesLine = (charges) =>
  charges.byToken.map((b) => `${b.total} ${b.tokenSymbol}@${b.chainId}`).join(', ') || '-';

const ms = (value) => (value === null ? '-' : String(value));

/**
 * Render the report as a compact table plus a block per session.
 *
 * @param {ReturnType<typeof buildSessionReport>} report
 * @param {{ logPath?: string }} [options]
 * @returns {string}
 */
export function formatSessionReport(report, { logPath = null } = {}) {
  const out = [];
  const t = report.totals;
  out.push(`Agent session report${logPath ? ` - ${logPath}` : ''}`);
  out.push(
    `Calls: ${t.calls}   Plan steps: ${t.steps}   Sessions: ${report.sessionCount}   Malformed lines skipped: ${report.malformedLines}` +
      (report.filter.sessionId ? `   Filter: session ${report.filter.sessionId}` : ''),
  );
  out.push(`Window: ${t.firstAt || '-'} -> ${t.lastAt || '-'}`);
  out.push('');

  const header = [
    'SESSION',
    'CALLS',
    'STEPS',
    'EXEC',
    'PREVIEW',
    'OK',
    'DENIED',
    'ERRORS',
    'REPLAYED',
    'P50ms',
    'P95ms',
    'MAXms',
    'CHARGES',
  ];
  const rows = [...report.sessions.map((s) => [s.sessionId, s]), ['TOTAL', t]].map(([label, s]) => [
    label,
    String(s.calls),
    String(s.steps),
    String(s.outcomes.executed),
    String(s.outcomes.previewed + s.outcomes.dryRun),
    String(s.outcomes.ok),
    String(s.outcomes.denied),
    String(s.outcomes.error),
    String(s.outcomes.replayed),
    ms(s.latencyMs.p50),
    ms(s.latencyMs.p95),
    ms(s.latencyMs.max),
    chargesLine(s.charges),
  ]);
  const last = header.length - 1;
  const widths = header.map((h, i) => Math.max(h.length, ...rows.map((r) => r[i].length)));
  const line = (cells) =>
    cells
      .map((cell, i) => (i === 0 || i === last ? cell.padEnd(widths[i]) : cell.padStart(widths[i])))
      .join('  ')
      .trimEnd();
  out.push(line(header));
  for (const row of rows) out.push(line(row));

  for (const s of report.sessions) {
    out.push('');
    out.push(`${s.sessionId}  (${s.firstAt || '-'} -> ${s.lastAt || '-'})`);
    out.push(`  status:   ${countsLine(s.byStatus)}`);
    out.push(`  tools:    ${countsLine(s.byTool)}`);
    if (s.plans.count > 0) {
      out.push(
        `  plans:    ${s.plans.count} (${countsLine(s.plans.byFinalStatus)}), ${s.steps} step(s)`,
      );
    }
    if (s.denials.total > 0) {
      out.push(`  denials:  ${countsLine(s.denials.byReason)}`);
      if (s.denials.reasonCodesUnavailable > 0) {
        out.push(
          `            (${s.denials.reasonCodesUnavailable} kernel refusal(s) logged without reason codes; grouped by error_code)`,
        );
      }
      if (s.denials.reasonCodesOmitted > 0) {
        out.push(
          `            (${s.denials.reasonCodesOmitted} further reason code(s) omitted by the log writer's per-event cap)`,
        );
      }
    }
    if (s.errors.total > 0) out.push(`  errors:   ${countsLine(s.errors.byCode)}`);
    if (s.charges.count > 0) {
      out.push(`  charges:  ${chargesLine(s.charges)}  (${s.charges.count} charged call(s))`);
    }
    if (s.principals.length > 0) out.push(`  principal: ${s.principals.join(', ')}`);
  }

  const notes = [
    'EXEC counts committed writes; PREVIEW counts previews and dry runs; OK counts other ' +
      'successes (reads, and writes the log cannot attribute). A plan counts as one call; ' +
      'its steps are counted under STEPS and in the outcome columns.',
  ];
  const unsessioned = report.sessions.find((s) => s.unsessioned);
  if (unsessioned) {
    notes.push(
      `"${UNSESSIONED}" groups the ${unsessioned.calls} call(s) and ${unsessioned.steps} step(s) ` +
        'logged without a session id, or whose session id is only their own request id.',
    );
  }
  if (t.outcomes.replayed > 0) {
    notes.push(
      `${t.outcomes.replayed} call(s) replayed an earlier kernel receipt (a retry with the same ` +
        'idempotency key) and are counted under REPLAYED, not as new writes or refusals.',
    );
  }
  if (t.charges.onReplayedReceipts > 0) {
    notes.push(
      `${t.charges.onReplayedReceipts} charge(s) were taken on replayed calls; the treasury ` +
        'recorded each one, so they are in the charge totals.',
    );
  }
  if (report.retryWindowExceeded) {
    notes.push(
      `More than ${MAX_TRACKED_RECEIPTS} governed commands: a retry of a command older than ` +
        'that window is counted as a new operation.',
    );
  }
  if (t.charges.numericAmounts > 0) {
    notes.push(
      `${t.charges.numericAmounts} charge amount(s) were logged as JSON numbers, not decimal strings; ` +
        'they were summed exactly from their shortest decimal form.',
    );
  }
  if (t.charges.unparseableAmounts > 0) {
    notes.push(
      `${t.charges.unparseableAmounts} charged call(s) had no readable amount and are not in the totals.`,
    );
  }
  out.push('');
  for (const note of notes) out.push(`Note: ${note}`);
  return out.join('\n');
}

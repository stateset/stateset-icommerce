/**
 * Agent session report.
 *
 * Reads the agentic replay log (`agentic-tool-calls.jsonl`, written by
 * ./replay-log.js from ./tool-dispatch.js) and answers the operator's
 * questions: what did each session do, what was refused and why, what did it
 * cost, how slow was it. Read-only: nothing here writes to the log or a store.
 *
 * Event shape, as written today (one JSON object per line):
 *
 *   eventId, tool, status, requestId, sessionId, policyDomain, occurredAt,
 *   elapsedMs, params, paramsHash, result, resultHash, policy, permission,
 *   charge, error, notes, source, agentic, eventSignature
 *
 *   - `source` is `mcp_server` (an MCP tool call) or `embedded_agent_toolkit`
 *     (`server.executeTool`). The two log `result` differently:
 *       mcp_server:             result.structuredContent = { ok, preview,
 *                               error: { code, message }, result: <tool result> }
 *       embedded_agent_toolkit: result = <tool result>
 *   - A governed call's tool result carries the kernel receipt at `.receipt`
 *     (`status`, `error_code`, `error_message`, `policy.reason_codes`,
 *     `economic_context.principal`). A kernel refusal is logged with
 *     `status: "error"`, not `policy_block`; only the receipt says it was a
 *     refusal.
 *   - The replay sanitizer bounds depth, so on `mcp_server` events the
 *     receipt's nested `policy` and `economic_context` are the string
 *     "[truncated]": `error_code` survives, `reason_codes` and the principal do
 *     not. Such refusals are grouped by `error_code` and counted in
 *     `denials.reasonCodesUnavailable`.
 *   - `charge` is `{ charged, rule: { tool, chainId, tokenSymbol, amount },
 *     mpp }`. `rule.amount` is the operator's pricing rule verbatim: a JSON
 *     number or a decimal string. The treasury ledger entry is not logged.
 */

import { createReadStream } from 'node:fs';
import { createInterface } from 'node:readline';

import { addDecimals } from '../utils/exact-decimal.js';

/** Key that groups events logged without a session id. */
export const NO_SESSION = '(no session)';

/** Statuses a gate (hook, policy, permission, payment, treasury) refuses with. */
const GATE_DENIAL_STATUSES = new Set([
  'blocked',
  'policy_block',
  'permission_block',
  'payment_required',
  'treasury_block',
]);

const ERROR_STATUSES = new Set(['error', 'invalid', 'rollback_failed']);
const OK_STATUSES = new Set(['success', 'preview', 'dry_run_success', 'rollback_success']);

const isPlainObject = (value) =>
  value !== null && typeof value === 'object' && !Array.isArray(value);

const nonEmptyString = (value) => (typeof value === 'string' && value.trim() ? value : null);

const bump = (counts, key, by = 1) => {
  counts[key] = (counts[key] || 0) + by;
};

const sortedCounts = (counts) =>
  Object.fromEntries(
    Object.entries(counts).sort(([a, x], [b, y]) => y - x || (a < b ? -1 : a > b ? 1 : 0)),
  );

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
  if (!isPlainObject(toolResult)) return null;
  if (looksLikeReceipt(toolResult.receipt)) return toolResult.receipt;
  if (looksLikeReceipt(toolResult) && typeof toolResult.receipt_id === 'string') return toolResult;
  return null;
}

/** The structured error code an event carries, or null. Never parsed from prose. */
function errorCodeOf(event, receipt) {
  const result = isPlainObject(event?.result) ? event.result : null;
  const contract = isPlainObject(result?.structuredContent) ? result.structuredContent : null;
  const toolResult = contract ? contract.result : result;
  const candidates = [
    isPlainObject(contract?.error) ? contract.error.code : null,
    receipt?.error_code,
    isPlainObject(toolResult) ? toolResult.errorCode : null,
    isPlainObject(toolResult) ? toolResult.error_code : null,
    isPlainObject(toolResult) && isPlainObject(toolResult.error) ? toolResult.error.code : null,
    isPlainObject(event?.error) ? event.error.code : null,
  ];
  return candidates.map(nonEmptyString).find(Boolean) ?? null;
}

/**
 * Classify one event.
 *
 * @param {object} event
 * @returns {{ outcome: 'ok'|'denied'|'error'|'other', reasons?: string[],
 *             reasonCodesUnavailable?: boolean, code?: string }}
 */
export function classifyEvent(event) {
  const status = nonEmptyString(event?.status) || '(unknown)';
  const receipt = findKernelReceipt(event);

  // A kernel refusal: logged as `error` (or `preview`), named by the receipt.
  if (receipt && receipt.status === 'rejected') {
    const policy = receipt.policy;
    const reasonCodes =
      isPlainObject(policy) && Array.isArray(policy.reason_codes)
        ? policy.reason_codes.map(nonEmptyString).filter(Boolean)
        : [];
    if (reasonCodes.length > 0) return { outcome: 'denied', reasons: reasonCodes };
    return {
      outcome: 'denied',
      reasons: [nonEmptyString(receipt.error_code) || 'kernel.rejected (no error_code)'],
      // The sanitizer replaced the nested policy decision with "[truncated]".
      reasonCodesUnavailable: !isPlainObject(policy),
    };
  }

  if (GATE_DENIAL_STATUSES.has(status)) return { outcome: 'denied', reasons: [status] };
  if (status === 'dry_run_blocked' && event?.permission?.preview !== true) {
    return { outcome: 'denied', reasons: [status] };
  }

  const contractFailed = event?.result?.structuredContent?.ok === false;
  if (ERROR_STATUSES.has(status) || (receipt && receipt.status === 'failed') || contractFailed) {
    return { outcome: 'error', code: errorCodeOf(event, receipt) || `(no code) ${status}` };
  }
  if (OK_STATUSES.has(status) || status === 'dry_run_blocked') return { outcome: 'ok' };
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
    byStatus: {},
    byTool: {},
    bySource: {},
    outcomes: { ok: 0, denied: 0, error: 0, other: 0 },
    denials: { total: 0, byReason: {}, reasonCodesUnavailable: 0 },
    errors: { total: 0, byCode: {} },
    charges: { count: 0, numericAmounts: 0, unparseableAmounts: 0, byToken: new Map() },
    principals: new Set(),
    firstAt: null,
    lastAt: null,
    firstMs: Infinity,
    lastMs: -Infinity,
    untimestamped: 0,
    latencies: [],
  };
}

function accumulate(acc, event) {
  acc.calls += 1;
  bump(acc.byStatus, nonEmptyString(event.status) || '(unknown)');
  bump(acc.byTool, nonEmptyString(event.tool) || '(unknown)');
  bump(acc.bySource, nonEmptyString(event.source) || '(unknown)');

  const verdict = classifyEvent(event);
  acc.outcomes[verdict.outcome] += 1;
  if (verdict.outcome === 'denied') {
    acc.denials.total += 1;
    for (const reason of verdict.reasons) bump(acc.denials.byReason, reason);
    if (verdict.reasonCodesUnavailable) acc.denials.reasonCodesUnavailable += 1;
  } else if (verdict.outcome === 'error') {
    acc.errors.total += 1;
    bump(acc.errors.byCode, verdict.code);
  }

  const charge = event.charge;
  if (isPlainObject(charge) && charge.charged === true) {
    acc.charges.count += 1;
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

  const principalId = findKernelReceipt(event)?.economic_context?.principal?.id;
  if (nonEmptyString(principalId)) acc.principals.add(principalId);

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
  } else {
    acc.untimestamped += 1;
  }

  if (typeof event.elapsedMs === 'number' && Number.isFinite(event.elapsedMs)) {
    acc.latencies.push(event.elapsedMs);
  }
}

function summarize(acc) {
  const sorted = [...acc.latencies].sort((a, b) => a - b);
  return {
    calls: acc.calls,
    outcomes: { ...acc.outcomes },
    byStatus: sortedCounts(acc.byStatus),
    byTool: sortedCounts(acc.byTool),
    bySource: sortedCounts(acc.bySource),
    denials: {
      total: acc.denials.total,
      byReason: sortedCounts(acc.denials.byReason),
      reasonCodesUnavailable: acc.denials.reasonCodesUnavailable,
    },
    errors: { total: acc.errors.total, byCode: sortedCounts(acc.errors.byCode) },
    charges: {
      count: acc.charges.count,
      byToken: [...acc.charges.byToken.values()]
        .map((bucket) => ({ ...bucket, total: addDecimals(bucket.total) }))
        .sort((a, b) =>
          `${a.tokenSymbol}@${a.chainId}` < `${b.tokenSymbol}@${b.chainId}` ? -1 : 1,
        ),
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
 * Incremental report builder, for callers that stream.
 *
 * @param {{ sessionId?: string|null }} [options] - report only this session
 *   (`"(no session)"` selects events logged without one)
 */
export function createSessionReportBuilder({ sessionId = null } = {}) {
  const sessions = new Map();
  const totals = createAccumulator();
  let malformedLines = 0;
  let filteredOut = 0;

  return {
    /** Add one parsed event. Anything that is not an event object is counted, not thrown. */
    add(event) {
      if (isMalformed(event)) {
        malformedLines += 1;
        return;
      }
      const key = nonEmptyString(event.sessionId) || NO_SESSION;
      if (sessionId && key !== sessionId) {
        filteredOut += 1;
        return;
      }
      let acc = sessions.get(key);
      if (!acc) {
        acc = createAccumulator();
        sessions.set(key, acc);
      }
      accumulate(acc, event);
      accumulate(totals, event);
    },
    finish() {
      const ordered = [...sessions.entries()].sort(
        ([a, x], [b, y]) => x.firstMs - y.firstMs || (a < b ? -1 : a > b ? 1 : 0),
      );
      return {
        filter: { sessionId: sessionId || null },
        malformedLines,
        filteredOut,
        sessionCount: ordered.length,
        totals: summarize(totals),
        sessions: Object.fromEntries(
          ordered.map(([key, acc]) => [key, { sessionId: key, ...summarize(acc) }]),
        ),
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
 * Build the report for a replay log on disk without loading it whole. Only
 * the per-event latency numbers are kept, for the percentiles.
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
    `Events: ${t.calls}   Sessions: ${report.sessionCount}   Malformed lines skipped: ${report.malformedLines}` +
      (report.filter.sessionId ? `   Filter: session ${report.filter.sessionId}` : ''),
  );
  out.push(`Window: ${t.firstAt || '-'} -> ${t.lastAt || '-'}`);
  out.push('');

  const header = [
    'SESSION',
    'CALLS',
    'OK',
    'DENIED',
    'ERRORS',
    'P50ms',
    'P95ms',
    'MAXms',
    'CHARGES',
  ];
  const rows = [...Object.values(report.sessions).map((s) => [s.sessionId, s]), ['TOTAL', t]].map(
    ([label, s]) => [
      label,
      String(s.calls),
      String(s.outcomes.ok),
      String(s.outcomes.denied),
      String(s.outcomes.error),
      ms(s.latencyMs.p50),
      ms(s.latencyMs.p95),
      ms(s.latencyMs.max),
      chargesLine(s.charges),
    ],
  );
  const widths = header.map((h, i) => Math.max(h.length, ...rows.map((r) => r[i].length)));
  const line = (cells) =>
    cells
      .map((cell, i) => (i === 0 || i === 8 ? cell.padEnd(widths[i]) : cell.padStart(widths[i])))
      .join('  ')
      .trimEnd();
  out.push(line(header));
  for (const row of rows) out.push(line(row));

  for (const s of Object.values(report.sessions)) {
    out.push('');
    out.push(`${s.sessionId}  (${s.firstAt || '-'} -> ${s.lastAt || '-'})`);
    out.push(`  status:   ${countsLine(s.byStatus)}`);
    out.push(`  tools:    ${countsLine(s.byTool)}`);
    if (s.denials.total > 0) {
      out.push(`  denials:  ${countsLine(s.denials.byReason)}`);
      if (s.denials.reasonCodesUnavailable > 0) {
        out.push(
          `            (${s.denials.reasonCodesUnavailable} kernel refusal(s) logged without reason codes; grouped by error_code)`,
        );
      }
    }
    if (s.errors.total > 0) out.push(`  errors:   ${countsLine(s.errors.byCode)}`);
    if (s.charges.count > 0) {
      out.push(`  charges:  ${chargesLine(s.charges)}  (${s.charges.count} charged call(s))`);
    }
    if (s.principals.length > 0) out.push(`  principal: ${s.principals.join(', ')}`);
  }

  const notes = [];
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
  if (report.sessions[NO_SESSION]) {
    notes.push(`"${NO_SESSION}" groups events logged without a sessionId.`);
  }
  if (notes.length > 0) {
    out.push('');
    for (const note of notes) out.push(`Note: ${note}`);
  }
  return out.join('\n');
}

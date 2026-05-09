// Cost budget tracking for the agentic plan executor.
//
// Plans submitted to the MCP server can declare a `costBudget` (a map from
// `chain:token` keys to per-bucket limits) plus a running `costSummary` that
// accumulates each step's actual or simulated spend. This module owns:
//
//   - Normalization: forgiving on input casing/whitespace, strict on output.
//   - Resolution: given (chain, token), pick the most-specific matching limit
//     in the priority order: `chain:token` → `token` → `chain:*` → `*`.
//   - Summary aggregation: per-step entries plus per-bucket totals with
//     numeric/text fallbacks.
//   - Empty-summary factory keyed by mode (e.g. "simulate" vs "execute").
//
// Extracted from `cli/src/mcp-server.js` to keep that file focused on
// orchestration. Shape preserved verbatim — callers in mcp-server.js continue
// to work without changes.

/**
 * Append one step's cost record to a running summary.
 *
 * Mutates `summary` in place; returns nothing.
 *
 * @param {Object} summary - Created via `createCostSummary(mode)`.
 * @param {Object} entry - Step record.
 * @param {string} [entry.chainId]
 * @param {string} [entry.tokenSymbol]
 * @param {number|string} [entry.amount]
 * @param {string} [entry.tool]
 * @param {string} [entry.status]
 * @param {number} [entry.stepIndex]
 * @param {boolean} [entry.charged]
 * @param {boolean} [entry.blocked]
 * @param {string} [entry.blockedReason]
 * @param {string} [entry.source]
 * @param {string} [entry.rule]
 */
export const addCostSummaryEntry = (summary, entry = {}) => {
  const chainId = entry.chainId || 'unknown';
  const tokenSymbol = entry.tokenSymbol || 'UNKNOWN';
  const key = `${chainId}:${tokenSymbol}`;
  const amount = entry.amount;
  const parsedAmount =
    typeof amount === 'number' || typeof amount === 'string' ? Number(amount) : NaN;
  if (!summary.totals[key]) {
    summary.totals[key] = {
      chainId,
      tokenSymbol,
      amount: 0,
      amountText: null,
      entries: 0,
    };
  }
  const bucket = summary.totals[key];
  bucket.entries += 1;
  if (Number.isFinite(parsedAmount)) {
    bucket.amount += parsedAmount;
  } else if (amount !== undefined && amount !== null) {
    bucket.amountText = amount;
  }

  summary.entries.push({
    step: entry.stepIndex ?? null,
    tool: entry.tool || null,
    status: entry.status || null,
    chainId,
    tokenSymbol,
    amount: amount ?? null,
    amountNumeric: Number.isFinite(parsedAmount) ? parsedAmount : null,
    charged: Boolean(entry.charged),
    blocked: Boolean(entry.blocked),
    blockedReason: entry.blockedReason || null,
    source: entry.source || null,
    rule: entry.rule || null,
  });

  summary.totalEntries = (summary.totalEntries || 0) + 1;
  if (entry.charged) summary.chargedEntries = (summary.chargedEntries || 0) + 1;
  if (entry.blocked) summary.blockedEntries = (summary.blockedEntries || 0) + 1;
};

/**
 * Coerce a budget limit value to a non-negative finite number, or null if
 * unparseable. Accepts numbers and string-of-numbers; rejects negatives and
 * non-finite values (NaN, ±Infinity).
 */
export const normalizeCostBudgetValue = (value) => {
  if (typeof value === 'number') return Number.isFinite(value) && value >= 0 ? value : null;
  if (typeof value === 'string' && value.trim()) {
    const parsed = Number(value.trim());
    return Number.isFinite(parsed) && parsed >= 0 ? parsed : null;
  }
  return null;
};

/**
 * Normalize a budget key. Accepts:
 *   - `*` (global wildcard)
 *   - `TOKEN` (token-only, any chain)
 *   - `CHAIN:TOKEN` (specific bucket; both halves required, case-insensitive)
 *   - `CHAIN:*` (chain-wide wildcard)
 *
 * Returns the canonical uppercase form, or `null` if unparseable.
 */
export const normalizeCostBudgetKey = (rawKey) => {
  if (typeof rawKey !== 'string') return null;
  const trimmed = rawKey.trim();
  if (!trimmed) return null;
  const upper = trimmed.toUpperCase();
  if (!upper.includes(':')) return upper;
  const [rawChain, rawToken] = upper.split(':').map((part) => part.trim());
  if (!rawChain || !rawToken) return null;
  return `${rawChain}:${rawToken}`;
};

/**
 * Normalize a full budget object: drops invalid keys/values silently, returns
 * a fresh object keyed canonically.
 */
export const normalizeCostBudget = (costBudget = null) => {
  if (!costBudget || typeof costBudget !== 'object' || Array.isArray(costBudget)) return {};
  const normalized = {};
  for (const [rawKey, rawLimit] of Object.entries(costBudget)) {
    const key = normalizeCostBudgetKey(rawKey);
    const limit = normalizeCostBudgetValue(rawLimit);
    if (!key || !Number.isFinite(limit)) continue;
    normalized[key] = limit;
  }
  return normalized;
};

/**
 * Resolve the applicable budget limit for a given (chainId, tokenSymbol),
 * using priority order: exact → token-only → chain-only → global.
 *
 * Returns the matched numeric limit, or `null` if no rule matched.
 */
export const resolveCostBudgetLimit = (costBudget = {}, chainId = null, tokenSymbol = null) => {
  const chain = String(chainId || '*').trim();
  const token = String(tokenSymbol || '*')
    .trim()
    .toUpperCase();
  const exact = costBudget[`${chain}:${token}`];
  if (Number.isFinite(exact)) return exact;
  const tokenOnly = costBudget[token];
  if (Number.isFinite(tokenOnly)) return tokenOnly;
  const chainOnly = costBudget[`${chain}:*`];
  if (Number.isFinite(chainOnly)) return chainOnly;
  const global = costBudget['*'];
  if (Number.isFinite(global)) return global;
  return null;
};

/**
 * Empty cost summary, ready to feed addCostSummaryEntry().
 *
 * @param {string} mode - "simulate" | "execute" (free-form, captured for telemetry).
 */
export const createCostSummary = (mode) => ({
  mode,
  totalEntries: 0,
  chargedEntries: 0,
  blockedEntries: 0,
  entries: [],
  totals: {},
});

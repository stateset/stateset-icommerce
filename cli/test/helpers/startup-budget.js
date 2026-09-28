/**
 * Time budgets for tests that spawn servers or measure wall-clock time.
 *
 * Booting an MCP server builds hundreds of tool schemas: about a second on an
 * idle machine, and 35-130s on a developer box running a parallel Rust build.
 * A fixed 30s budget therefore fails locally while passing in CI, which teaches
 * people to ignore red tests. These helpers keep the CI value as the baseline
 * and stretch it only when the machine is demonstrably oversubscribed:
 *
 *   load factor = clamp(max(loadavg 1m, 5m) / cpu count, 1, MAX_LOAD_FACTOR)
 *
 * On a machine whose load is at or below its core count the factor is exactly
 * 1, so CI behaviour is unchanged. Readiness is a boot cost, not a correctness
 * signal; the budgets only decide how long to wait, never what is asserted.
 *
 * Overrides:
 *   STATESET_TEST_STARTUP_TIMEOUT_MS  absolute startup budget (skips scaling)
 *   STATESET_TEST_LOAD_FACTOR         force the load factor (e.g. 1 to disable)
 */

import os from 'node:os';

/** Upper bound on the automatic stretch, so a hung server still fails. */
export const MAX_LOAD_FACTOR = 10;

function positiveNumber(raw) {
  if (raw === undefined || raw === '') return null;
  const value = Number(raw);
  return Number.isFinite(value) && value > 0 ? value : null;
}

/**
 * How oversubscribed the machine is right now: 1 when idle or at capacity,
 * up to MAX_LOAD_FACTOR. Always 1 where loadavg is unsupported (Windows).
 */
export function loadFactor() {
  const forced = positiveNumber(process.env.STATESET_TEST_LOAD_FACTOR);
  if (forced !== null) return forced;
  const cpus = os.cpus().length || 1;
  const [oneMinute = 0, fiveMinute = 0] = os.loadavg();
  const ratio = Math.max(oneMinute, fiveMinute) / cpus;
  return Math.min(MAX_LOAD_FACTOR, Math.max(1, ratio));
}

/** Scale a wall-clock budget (request timeout, upper timing bound) by load. */
export function scaleForLoad(ms) {
  return Math.ceil(ms * loadFactor());
}

/**
 * The budget for a spawned server to become ready. `defaultMs` is the value
 * CI has always used; STATESET_TEST_STARTUP_TIMEOUT_MS replaces it outright.
 */
export function startupBudgetMs(defaultMs = 30_000) {
  const override = positiveNumber(process.env.STATESET_TEST_STARTUP_TIMEOUT_MS);
  if (override !== null) return override;
  return scaleForLoad(defaultMs);
}

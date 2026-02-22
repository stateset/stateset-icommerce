/**
 * Progress Reporting for StateSet CLI
 *
 * Provides multi-backend progress indicators that degrade gracefully:
 *   TTY  → @clack/prompts animated spinner
 *   Pipe → console.error log lines
 *   Silent → noop
 */

import { theme } from './theme.js';

// ============================================================================
// Progress Reporter Interface
// ============================================================================

/**
 * @typedef {Object} ProgressReporter
 * @property {(label: string) => void} setLabel  - Update the progress label
 * @property {(pct: number) => void} setPercent   - Set 0-100 percent (for determinate tasks)
 * @property {() => void} done   - Mark as complete (stops animation)
 * @property {(msg?: string) => void} fail  - Mark as failed (stops animation)
 */

// ============================================================================
// Noop (disabled or unsupported)
// ============================================================================

function noopProgress() {
  return {
    setLabel() {},
    setPercent() {},
    done() {},
    fail() {},
  };
}

// ============================================================================
// Log-based (non-TTY / CI)
// ============================================================================

function logProgress(label) {
  let lastPct = -1;
  console.error(theme.muted(`  ${label}...`));

  return {
    setLabel(newLabel) {
      console.error(theme.muted(`  ${newLabel}...`));
    },
    setPercent(pct) {
      const rounded = Math.round(pct / 25) * 25; // only log at 0, 25, 50, 75, 100
      if (rounded > lastPct) {
        lastPct = rounded;
        console.error(theme.muted(`  ${label}... ${rounded}%`));
      }
    },
    done() {
      console.error(theme.success(`  ${label} — done`));
    },
    fail(msg) {
      console.error(theme.error(`  ${label} — ${msg || 'failed'}`));
    },
  };
}

// ============================================================================
// TTY Spinner (animated via @clack/prompts)
// ============================================================================

function ttyProgress(label) {
  // Lazy-load to avoid importing @clack in non-TTY environments
  let spinnerInstance = null;
  let started = false;

  function ensureStarted() {
    if (started) return;
    started = true;
    try {
      // Dynamic import would be ideal but @clack/prompts is ESM and already
      // in our dependency tree, so static reference is fine.
      const clack = /** @type {typeof import('@clack/prompts')} */ (globalThis.__clackPrompts);
      if (clack) {
        spinnerInstance = clack.spinner();
        spinnerInstance.start(label);
      }
    } catch {
      // Fallback — spinner unavailable
    }
  }

  // Pre-load @clack into globalThis for the lazy accessor
  import('@clack/prompts')
    .then((mod) => {
      globalThis.__clackPrompts = mod;
      // If ensureStarted was already called, start now
      if (started && !spinnerInstance) {
        spinnerInstance = mod.spinner();
        spinnerInstance.start(label);
      }
    })
    .catch(() => {
      // @clack not available — noop
    });

  return {
    setLabel(newLabel) {
      ensureStarted();
      if (spinnerInstance) {
        spinnerInstance.message(newLabel);
      }
    },
    setPercent(pct) {
      ensureStarted();
      if (spinnerInstance) {
        spinnerInstance.message(`${label} ${Math.round(pct)}%`);
      }
    },
    done() {
      ensureStarted();
      if (spinnerInstance) {
        spinnerInstance.stop(theme.success(`${label} — done`));
      }
    },
    fail(msg) {
      ensureStarted();
      if (spinnerInstance) {
        spinnerInstance.stop(theme.error(`${label} — ${msg || 'failed'}`));
      }
    },
  };
}

// ============================================================================
// Factory
// ============================================================================

/**
 * Create a progress reporter that auto-selects the best backend.
 *
 * @param {{ label?: string, enabled?: boolean, fallback?: 'log'|'none' }} [options]
 * @returns {ProgressReporter}
 */
export function createProgress(options = {}) {
  const { label = 'Working', enabled = true, fallback = 'log' } = options;

  if (!enabled) return noopProgress();

  if (process.stderr.isTTY) {
    return ttyProgress(label);
  }

  if (fallback === 'log') return logProgress(label);
  return noopProgress();
}

// ============================================================================
// Convenience Wrapper
// ============================================================================

/**
 * Run an async function with automatic progress reporting.
 *
 * @template T
 * @param {string} label
 * @param {(progress: ProgressReporter) => Promise<T>} fn
 * @param {{ enabled?: boolean, fallback?: 'log'|'none' }} [options]
 * @returns {Promise<T>}
 */
export async function withProgress(label, fn, options = {}) {
  const progress = createProgress({ label, ...options });
  try {
    const result = await fn(progress);
    progress.done();
    return result;
  } catch (err) {
    progress.fail(err instanceof Error ? err.message : String(err));
    throw err;
  }
}

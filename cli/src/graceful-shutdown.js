/**
 * Graceful Shutdown Utilities
 *
 * Provides unhandled rejection handlers, signal-based graceful shutdown,
 * themed error output, and actionable error hints for CLI entry points.
 */

import { theme } from './theme.js';

/** @type {((err: unknown) => string | null) | null} */
let _getErrorHint = null;

/**
 * Lazily load getErrorHint — avoids circular-dependency issues at module load
 * and keeps the import lightweight for commands that never hit an error.
 */
async function loadErrorHint() {
  if (_getErrorHint) return _getErrorHint;
  try {
    const mod = await import('./utils/error-hints.js');
    _getErrorHint = mod.getErrorHint;
  } catch {
    _getErrorHint = () => null;
  }
  return _getErrorHint;
}

/**
 * Format a fatal error with optional hint and optional stack trace.
 *
 * @param {string} name   - Command name
 * @param {unknown} err   - Error or string
 * @param {object} [opts]
 * @param {boolean} [opts.verbose] - Include stack trace
 * @param {((err: unknown) => string | null) | null} [opts.hintFn]
 */
function formatFatalError(name, err, opts = {}) {
  const message = err instanceof Error ? err.message : String(err);
  const hintFn = opts.hintFn ?? _getErrorHint;
  const hint = hintFn ? hintFn(err) : null;

  console.error(`\n${theme.error(`[${name}]`)} ${theme.bold('Fatal error:')} ${message}`);

  if (hint) {
    console.error(`\n${theme.muted('  Suggestion:')}`);
    hint.split('\n').forEach((line) => {
      console.error(`  ${theme.muted(line)}`);
    });
  }

  if ((opts.verbose || process.env.DEBUG) && err instanceof Error && err.stack) {
    console.error(`\n${theme.dim(err.stack)}`);
  }

  console.error(); // blank line for readability
}

/**
 * Install global error and signal handlers for a CLI entry point.
 *
 * @param {string} name - Command name (for log messages)
 * @param {object} [options]
 * @param {Function} [options.cleanup] - Async cleanup function called before exit
 */
export function installShutdownHandlers(name, options = {}) {
  const { cleanup } = options;

  // Pre-load error hints so they're ready when needed
  loadErrorHint().catch((err) => {
    console.debug('error-hint preload failed:', err.message);
  });

  process.on('unhandledRejection', (reason) => {
    formatFatalError(name, reason, { verbose: true });
    process.exit(1);
  });

  process.on('uncaughtException', (err) => {
    formatFatalError(name, err, { verbose: true });
    process.exit(1);
  });

  const gracefulShutdown = async (signal) => {
    console.error(`${theme.muted(`[${name}]`)} Received ${signal}, shutting down...`);
    if (cleanup) {
      try {
        await cleanup();
      } catch (err) {
        console.error(
          `${theme.warn(`[${name}]`)} Cleanup error:`,
          err instanceof Error ? err.message : err,
        );
      }
    }
    process.exit(0);
  };

  process.on('SIGINT', () => gracefulShutdown('SIGINT'));
  process.on('SIGTERM', () => gracefulShutdown('SIGTERM'));
}

/**
 * Run a main() function with error handling, shutdown hooks, and
 * actionable error hints.
 *
 * @param {string} name - Command name
 * @param {Function} mainFn - Async main function
 * @param {object} [options]
 * @param {Function} [options.cleanup] - Async cleanup function
 * @param {boolean} [options.verbose] - Include stack traces on error
 */
export function runMain(name, mainFn, options = {}) {
  installShutdownHandlers(name, options);

  Promise.resolve(mainFn()).catch(async (err) => {
    // Ensure hints are loaded before formatting
    await loadErrorHint().catch((hintErr) => {
      console.debug('error-hint load failed:', hintErr.message);
    });

    formatFatalError(name, err, {
      verbose: options.verbose || process.argv.includes('--verbose'),
    });

    const exitCode =
      err && typeof err === 'object' && typeof err.exitCode === 'number' ? err.exitCode : 1;
    process.exit(exitCode);
  });
}

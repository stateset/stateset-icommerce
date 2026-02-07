/**
 * Graceful Shutdown Utilities
 *
 * Provides unhandled rejection handlers and signal-based
 * graceful shutdown for CLI entry points.
 */

/**
 * Install global error and signal handlers for a CLI entry point.
 *
 * @param {string} name - Command name (for log messages)
 * @param {object} [options]
 * @param {Function} [options.cleanup] - Async cleanup function called before exit
 */
export function installShutdownHandlers(name, options = {}) {
  const { cleanup } = options;

  process.on('unhandledRejection', (reason) => {
    console.error(
      `[${name}] Unhandled rejection:`,
      reason instanceof Error ? reason.message : reason,
    );
    process.exit(1);
  });

  process.on('uncaughtException', (err) => {
    console.error(`[${name}] Uncaught exception:`, err.message);
    process.exit(1);
  });

  const gracefulShutdown = async (signal) => {
    console.error(`[${name}] Received ${signal}, shutting down...`);
    if (cleanup) {
      try {
        await cleanup();
      } catch (err) {
        console.error(`[${name}] Cleanup error:`, err.message);
      }
    }
    process.exit(0);
  };

  process.on('SIGINT', () => gracefulShutdown('SIGINT'));
  process.on('SIGTERM', () => gracefulShutdown('SIGTERM'));
}

/**
 * Run a main() function with error handling and shutdown hooks.
 *
 * @param {string} name - Command name
 * @param {Function} mainFn - Async main function
 * @param {object} [options]
 * @param {Function} [options.cleanup] - Async cleanup function
 */
export function runMain(name, mainFn, options = {}) {
  installShutdownHandlers(name, options);

  Promise.resolve(mainFn()).catch((err) => {
    console.error(`[${name}] Fatal error:`, err instanceof Error ? err.message : err);
    process.exit(1);
  });
}

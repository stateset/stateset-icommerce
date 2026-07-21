/**
 * Retry loop around a single agent query execution.
 *
 * Extracted from runAgentLoop in claude-harness.js. All collaborators are
 * dependency-injected; there is no module-scope state.
 *
 * Decision points preserved exactly:
 * - a result carrying errorType 'error_max_budget_usd' exits the loop
 *   without retrying (budget-exceeded is surfaced, not retried);
 * - other error-typed results are re-thrown as coded errors;
 * - watchdog timeouts and error_max* results are never retried;
 * - retryable errors back off per computeRetryDelay until maxRetries.
 */

import { isRetryableError, computeRetryDelay, sleep } from '../retry-helpers.js';
import { InactivityWatchdogError } from '../harness-utils.js';

export async function executeWithRetry({ executeOnce, retrySettings, telem }) {
  let queryResult;
  let attempt = 0;
  while (true) {
    attempt++;
    try {
      queryResult = await executeOnce();
      if (queryResult.error) {
        if (queryResult.errorType === 'error_max_budget_usd') {
          break;
        }
        const err = new Error(queryResult.error);
        err.code = queryResult.errorType;
        throw err;
      }
      break;
    } catch (err) {
      const errorType = queryResult?.errorType;
      const nonRetryable =
        (errorType && errorType.startsWith('error_max')) ||
        err?.code === 'WATCHDOG_TIMEOUT' ||
        err instanceof InactivityWatchdogError;
      const canRetry =
        retrySettings?.enabled &&
        attempt <= (retrySettings.maxRetries || 0) &&
        !nonRetryable &&
        isRetryableError(err, retrySettings);

      if (!canRetry) {
        throw err;
      }

      const delayMs = computeRetryDelay(attempt, retrySettings);
      telem.logCustomEvent('auto_retry', {
        attempt,
        delayMs,
        error: err?.message || String(err),
      });
      await sleep(delayMs);
    }
  }
  return queryResult;
}

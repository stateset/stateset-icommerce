/** File-backed compliance operations run outside the native engine process. */
import { execFile } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

export const COMPLIANCE_OPERATIONS = Object.freeze([
  'exportAuditTrail',
  'generate1099K',
  'generateGDPRExport',
  'deleteGDPRData',
  'generateComplianceSummary',
  'generateSOC2Evidence',
]);
const workerPath = fileURLToPath(new URL('./isolated-worker.js', import.meta.url));
const MAX_REQUEST_BYTES = 64 * 1024;

function filePath(value, label) {
  if (typeof value !== 'string' || !value || value === ':memory:' || value.startsWith('file:')) {
    throw new Error(
      `${label} must be a file path; in-memory stores cannot cross process boundaries`,
    );
  }
  return path.resolve(value);
}

/**
 * Create an async compliance service without opening SQLite in this process.
 * Both files must already exist. A null commerceDbPath selects A2A-only coverage.
 * Each call opens and closes its own worker, so engine close/reopen cannot leave
 * an auxiliary driver holding deleted WAL descriptors in the engine process.
 */
export function createIsolatedComplianceService(dbPath, { commerceDbPath = dbPath } = {}) {
  const storePath = filePath(dbPath, 'dbPath');
  const commercePath = commerceDbPath === null ? null : filePath(commerceDbPath, 'commerceDbPath');
  return Object.fromEntries(
    COMPLIANCE_OPERATIONS.map((operation) => [
      operation,
      (...args) => {
        const input = JSON.stringify({
          dbPath: storePath,
          commerceDbPath: commercePath,
          operation,
          args,
        });
        if (Buffer.byteLength(input) > MAX_REQUEST_BYTES) {
          return Promise.reject(new Error('Compliance request exceeds 64 KiB'));
        }
        return new Promise((resolve, reject) => {
          const child = execFile(
            process.execPath,
            [workerPath],
            {
              timeout: 60_000,
              maxBuffer: 32 * 1024 * 1024,
              windowsHide: true,
            },
            (error, stdout) => {
              if (error) {
                const failure = new Error(
                  operation === 'deleteGDPRData'
                    ? 'Compliance worker failed; erasure outcome is unknown. Reconcile the store before retrying.'
                    : 'Compliance worker failed before returning a complete result.',
                );
                failure.code =
                  operation === 'deleteGDPRData'
                    ? 'COMPLIANCE_OUTCOME_UNKNOWN'
                    : 'COMPLIANCE_WORKER_FAILED';
                failure.cause = error;
                reject(failure);
                return;
              }
              try {
                const reply = JSON.parse(stdout);
                if (!reply.ok) {
                  const failure = new Error(reply.error || 'Compliance operation failed');
                  failure.code = reply.code || 'COMPLIANCE_OPERATION_FAILED';
                  reject(failure);
                } else {
                  resolve(reply.result);
                }
              } catch (cause) {
                reject(new Error('Compliance worker returned an invalid response', { cause }));
              }
            },
          );
          // A startup failure can close stdin before the request is written. The
          // execFile callback owns failure reporting; never emit an unhandled EPIPE.
          child.stdin.on('error', () => {});
          child.stdin.end(input);
        });
      },
    ]),
  );
}

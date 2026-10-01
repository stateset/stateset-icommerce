/** Internal one-operation worker. Never import the native commerce binding here. */
import { readFileSync, realpathSync } from 'node:fs';
import { A2AStore } from '../a2a/store.js';
import { createComplianceService } from './exports.js';
import { COMPLIANCE_OPERATIONS } from './isolated.js';

let store;
let commerceDb;
let service;
let separateFileErasure = false;
try {
  const request = readFileSync(0, 'utf8');
  if (Buffer.byteLength(request) > 64 * 1024) throw new Error('Compliance request exceeds 64 KiB');
  const { dbPath, commerceDbPath, operation, args } = JSON.parse(request);
  if (!COMPLIANCE_OPERATIONS.includes(operation) || !Array.isArray(args)) {
    throw new Error('Invalid compliance operation');
  }
  // Resolve before init so a typo/missing configured database cannot silently
  // create an empty store and return an incomplete compliance result.
  const storePath = realpathSync(dbPath);
  const commercePath = commerceDbPath === null ? null : realpathSync(commerceDbPath);
  store = new A2AStore({ dbPath: storePath });
  store.init();
  if (commercePath) {
    commerceDb =
      commercePath === storePath
        ? store.db
        : new store.db.constructor(commercePath, { fileMustExist: true });
  }
  service = createComplianceService(store, { commerceDb });
  const invoke = () => service[operation](...args);
  // A shared store has one transaction for A2A and commerce: an error halfway
  // through erasure rolls everything back. Separate files retain independent
  // commit boundaries and require operator reconciliation on commit failure.
  separateFileErasure = operation === 'deleteGDPRData' && commerceDb && commerceDb !== store.db;
  const work =
    commerceDb && commerceDb !== store.db
      ? () => {
          const commerceTransaction = commerceDb.transaction(invoke);
          return operation === 'deleteGDPRData'
            ? commerceTransaction.immediate()
            : commerceTransaction();
        }
      : invoke;
  const transaction = store.db.transaction(work);
  const result = operation === 'deleteGDPRData' ? transaction.immediate() : transaction();
  process.stdout.write(JSON.stringify({ ok: true, result }));
} catch (error) {
  process.stdout.write(
    JSON.stringify({
      ok: false,
      error: separateFileErasure
        ? `Erasure failed across separate databases; reconcile both stores before retrying: ${error.message}`
        : error.message,
      code: separateFileErasure ? 'COMPLIANCE_OUTCOME_UNKNOWN' : error.code,
    }),
  );
} finally {
  service?.close();
  if (commerceDb && commerceDb !== store?.db) commerceDb.close();
  store?.close();
}

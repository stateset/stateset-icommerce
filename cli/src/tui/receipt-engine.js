import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);

// Read-only receipt retrieval. Neither local intent files nor UI text can
// select another database, principal, store, or executable command.
export function readNativeReceipt(database, kernel, idempotencyKey, commandType) {
  const Database = require('better-sqlite3');
  const db = new Database(database, { readonly: true, fileMustExist: true });
  try {
    const row = db
      .prepare('SELECT receipt FROM kernel_receipts WHERE idempotency_key = ? AND command_type = ?')
      .get(idempotencyKey, commandType);
    const receipt = row ? JSON.parse(row.receipt) : null;
    if (
      receipt &&
      (receipt.economic_context?.principal?.id !== kernel.principal.id ||
        receipt.economic_context?.store_id !== kernel.storeId)
    )
      throw new Error('Stored receipt does not match the current operator and store.');
    return { receipt };
  } finally {
    db.close();
  }
}

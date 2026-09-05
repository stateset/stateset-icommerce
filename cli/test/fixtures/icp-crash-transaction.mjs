import Database from 'better-sqlite3';
import { SqliteProtocolStore } from '../../../icp-handler/src/sqlite-store.mjs';
import * as state from '../../../icp-handler/src/state.mjs';
const db = new Database(process.argv[2]);
db.pragma('journal_mode = WAL');
db.pragma('synchronous = FULL');
state.configureStorage(new SqliteProtocolStore(db));
state.initializeInventory();
state.atomic(() => {
  state.reserveInventory('crash:escrow', [{ sku: 'SKU-100', quantity: 80 }]);
  state.createEscrow('crash:escrow', { state: 'pending', seq: 0 });
  state.appendEscrowEvent('crash:escrow', { seq: 0, to_state: 'pending' });
  process.kill(process.pid, 'SIGKILL');
});

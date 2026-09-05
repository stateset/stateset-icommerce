#!/usr/bin/env node
// Opt-in reference merchant: durable protocol state, SIMULATED economic rails.
import Database from 'better-sqlite3';
import { SqliteProtocolStore } from '../../icp-handler/src/sqlite-store.mjs';
import { configureStorage } from '../../icp-handler/src/state.mjs';

if (!process.argv.includes('--apply') || !process.argv.includes('--demo')) {
  console.log('No changes made. Durable reference mode requires --apply --demo --db PATH.');
  console.log(
    'Set ICP_MERCHANT_KEY_FILE (Ed25519 PEM) and ICP_MERCHANT_AID to operator-owned values.',
  );
} else {
  const index = process.argv.indexOf('--db');
  const path = index >= 0 ? process.argv[index + 1] : null;
  if (!path || path.startsWith('--') || path === ':memory:')
    throw new Error('a persistent --db path is required');
  if (!process.env.ICP_MERCHANT_KEY_FILE || !process.env.ICP_MERCHANT_AID) {
    throw new Error('operator-owned merchant key file and AID are required');
  }
  const db = new Database(path);
  db.pragma('journal_mode = WAL');
  db.pragma('synchronous = FULL');
  db.pragma('busy_timeout = 5000');
  try {
    configureStorage(new SqliteProtocolStore(db));
    const { server } = await import('../../icp-handler/src/server.mjs');
    console.log(
      'DURABLE REFERENCE DEMO: mock balances and settlement; not a production payment endpoint.',
    );
    server.on('close', () => db.close());
    for (const signal of ['SIGINT', 'SIGTERM']) process.once(signal, () => server.close());
  } catch (error) {
    db.close();
    throw error;
  }
}

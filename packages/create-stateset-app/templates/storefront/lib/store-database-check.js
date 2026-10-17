import fs from 'node:fs';
import path from 'node:path';

export function inspectStoreDatabase(environment, { cwd = process.cwd(), launch = false } = {}) {
  const database = environment.STATESET_DB_PATH || './store.db';
  if (database === ':memory:') {
    return {
      id: 'database',
      status: launch ? 'fail' : 'warn',
      message: 'The store uses an ephemeral database.',
      action: 'Set STATESET_DB_PATH to a persistent SQLite file before launch.',
    };
  }
  let descriptor;
  try {
    const filename = path.resolve(cwd, database);
    if (!fs.statSync(filename).isFile()) throw new Error();
    descriptor = fs.openSync(filename, 'r');
    const header = Buffer.alloc(16);
    if (
      fs.readSync(descriptor, header, 0, 16, 0) !== 16 ||
      header.toString('ascii') !== 'SQLite format 3\0'
    )
      throw new Error();
    fs.accessSync(filename, fs.constants.R_OK | fs.constants.W_OK);
    fs.accessSync(path.dirname(filename), fs.constants.R_OK | fs.constants.W_OK);
    return {
      id: 'database',
      status: 'pass',
      message:
        'An existing SQLite file and its directory are accessible; schema and integrity are not checked.',
      action: null,
    };
  } catch {
    return {
      id: 'database',
      status: 'fail',
      message: 'The configured SQLite store is missing, invalid or inaccessible.',
      action:
        'Set STATESET_DB_PATH to your existing store. For a new demo store, run npm run seed; verify file and directory permissions.',
    };
  } finally {
    if (descriptor !== undefined) fs.closeSync(descriptor);
  }
}

/**
 * Fingerprint persisted state from OUTSIDE the process under test.
 *
 * The MCP server holds the commerce store open through two SQLite libraries
 * at once: the native engine's and better-sqlite3 (A2A store, audit store).
 * POSIX advisory locks belong to a process, not to a file descriptor, so a
 * reader in the same process that opens and closes the database -- even
 * just to read its header -- silently drops every lock the server's
 * connections hold on it (https://www.sqlite.org/howtocorrupt.html, 2.2).
 * Another process can then take the database for its last user and delete
 * the WAL index out from under the server. Measuring in-process would
 * perturb exactly what it measures, so every read of the sandbox happens in
 * this child process; the parent only sends commands.
 *
 * The server can also lose its locks on its own: the erc8004 read tools open
 * and close the store file directly (src/erc8004/index.js ensureDbFile), after
 * which the next compliance tool's worker process deletes the WAL and -shm on
 * exit, and the server's later writes become invisible to every new
 * connection. The probe therefore keeps its connections open between
 * fingerprints (createConnectionCache in state-fingerprint.js), which keeps
 * the WAL in place; the negative control at the end of the sweep is what
 * shows that writes stayed visible.
 *
 *   const probe = await createStateProbe({ roots, appendOnly: [replayLog] });
 *   await probe.mark();                    // baseline
 *   const changes = await probe.diff(tool); // changes since the last mark/diff
 *   const pool = await probe.seedPool(dbPath);
 *   await probe.close();
 */

import { fork } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { createConnectionCache, diffFingerprints, fingerprintState } from './state-fingerprint.js';
import { readSeedPool } from './read-tool-sweep.js';

const IS_CHILD =
  process.env.STATESET_STATE_PROBE_CHILD === '1' && typeof process.send === 'function';

/**
 * @param {object} options
 * @param {string[]} options.roots - directories to fingerprint
 * @param {string[]} [options.appendOnly] - append-only logs (see state-fingerprint.js).
 *   A line appended to one of them is expected, not a change, when its JSON
 *   `tool` is the tool named in `diff(tool)`: that is how the MCP replay log
 *   records the call being measured.
 */
export async function createStateProbe({ roots, appendOnly = [] }) {
  const child = fork(fileURLToPath(import.meta.url), [], {
    env: { ...process.env, STATESET_STATE_PROBE_CHILD: '1' },
    execArgv: [],
    stdio: ['ignore', 'inherit', 'inherit', 'ipc'],
  });
  const pending = new Map();
  let nextId = 0;
  let exited = null;
  child.on('message', ({ id, ok, result, error }) => {
    const entry = pending.get(id);
    if (!entry) return;
    pending.delete(id);
    if (ok) entry.resolve(result);
    else entry.reject(new Error(`state probe: ${error}`));
  });
  child.on('exit', (code, signal) => {
    exited = new Error(`state probe exited (code ${code}, signal ${signal})`);
    for (const entry of pending.values()) entry.reject(exited);
    pending.clear();
  });
  const send = (cmd, args = {}) => {
    if (exited) return Promise.reject(exited);
    const id = nextId++;
    return new Promise((resolve, reject) => {
      pending.set(id, { resolve, reject });
      child.send({ id, cmd, ...args });
    });
  };
  await send('configure', { roots, appendOnly });
  return {
    mark: () => send('mark'),
    diff: (tool = null) => send('diff', { tool }),
    seedPool: async (dbPath) => {
      const { idsByTable, named } = await send('pool', { dbPath });
      return { idsByTable: new Map(idsByTable), named };
    },
    close: async () => {
      if (exited) return;
      child.disconnect();
      await new Promise((resolve) => child.once('exit', resolve));
    },
  };
}

if (IS_CHILD) {
  let config = { roots: [], appendOnly: new Set() };
  let previous = null;
  const connections = createConnectionCache();
  const snapshot = () =>
    fingerprintState(config.roots, {
      appendOnly: (file) => config.appendOnly.has(file),
      connections,
    });
  const handlers = {
    configure: ({ roots, appendOnly }) => {
      config = { roots, appendOnly: new Set(appendOnly) };
      return true;
    },
    mark: () => {
      previous = snapshot();
      return true;
    },
    diff: ({ tool }) => {
      const current = snapshot();
      const changes = diffFingerprints(previous ?? current, current, {
        expectedAppend: (line) => {
          if (!tool) return false;
          try {
            return JSON.parse(line).tool === tool;
          } catch {
            return false;
          }
        },
      });
      previous = current;
      return changes;
    },
    pool: ({ dbPath }) => {
      const { idsByTable, named } = readSeedPool(dbPath, { db: connections.open(dbPath) });
      return { idsByTable: [...idsByTable], named };
    },
  };
  process.on('message', (message) => {
    try {
      const result = handlers[message.cmd](message);
      process.send({ id: message.id, ok: true, result });
    } catch (error) {
      process.send({ id: message.id, ok: false, error: error?.stack ?? String(error) });
    }
  });
  process.on('disconnect', () => {
    connections.close();
    process.exit(0);
  });
}

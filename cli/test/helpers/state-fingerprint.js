/**
 * A fingerprint of everything a process could have persisted under a set of
 * directories, and a diff between two of them.
 *
 * - Every SQLite database (found by its file header, whatever its name) is
 *   fingerprinted by CONTENT, not bytes: its schema (sqlite_master) and, for
 *   each table, every row in rowid (or primary-key) order. Reading through
 *   SQLite means a WAL checkpoint, a page reorganisation or a growing -wal
 *   file is not a change, while a committed row is.
 * - The SQLite sidecars (-wal, -shm, -journal) are skipped as files: their
 *   committed content is already read through the database itself.
 * - Virtual tables (FTS5) are skipped as tables: their content lives in the
 *   shadow tables (`*_fts_data`, `*_fts_content`, ...), which are fingerprinted
 *   like any other table, and reading the virtual table itself would need the
 *   writer's tokenizer.
 * - Append-only logs named by the caller (`appendOnly`) are recorded by size
 *   and tail so a diff can return just the appended lines; the caller decides
 *   which appends are expected. Anything that rewrites such a log is a change.
 * - Every other file is hashed; directories are recorded so an empty directory
 *   created by a tool is a change too.
 *
 * Used by test/integration/mcp-read-tools-do-not-mutate.test.js.
 */

import { createHash } from 'node:crypto';
import { closeSync, openSync, readSync, readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';

import Database from 'better-sqlite3';

const SQLITE_HEADER = Buffer.from('SQLite format 3\u0000', 'latin1');
const SQLITE_SIDECAR = /-(?:wal|shm|journal)$/;
const TAIL_BYTES = 256;

const quoteIdent = (name) => `"${String(name).replaceAll('"', '""')}"`;
const sha = (value) => createHash('sha256').update(value).digest('hex');

function readRange(file, start, end) {
  const length = Math.max(0, end - start);
  const buffer = Buffer.alloc(length);
  if (length === 0) return buffer;
  const fd = openSync(file, 'r');
  try {
    let offset = 0;
    while (offset < length) {
      const read = readSync(fd, buffer, offset, length - offset, start + offset);
      if (read === 0) break;
      offset += read;
    }
    return buffer.subarray(0, offset);
  } finally {
    closeSync(fd);
  }
}

function isSqliteFile(file, size) {
  if (size < SQLITE_HEADER.length) return false;
  return readRange(file, 0, SQLITE_HEADER.length).equals(SQLITE_HEADER);
}

/** One row as a stable string: blobs become their digest, everything else JSON. */
function encodeRow(values) {
  return JSON.stringify(
    values.map((value) => {
      if (Buffer.isBuffer(value)) return `blob:${sha(value)}`;
      if (typeof value === 'bigint') return `bigint:${value}`;
      return value;
    }),
  );
}

function snapshotTable(db, name) {
  const table = quoteIdent(name);
  let statement;
  let keyColumns = null;
  try {
    statement = db.prepare(`SELECT rowid AS __fingerprint_key, * FROM ${table} ORDER BY rowid`);
  } catch {
    // WITHOUT ROWID: key and order by the primary key instead.
    const pk = db
      .prepare(`PRAGMA table_info(${table})`)
      .all()
      .filter((column) => column.pk > 0)
      .sort((a, b) => a.pk - b.pk)
      .map((column) => column.name);
    keyColumns = pk.length > 0 ? pk : null;
    const order = keyColumns ? ` ORDER BY ${keyColumns.map(quoteIdent).join(', ')}` : '';
    statement = db.prepare(`SELECT * FROM ${table}${order}`);
  }
  const columns = statement.columns().map((column) => column.name);
  const keyed = columns[0] === '__fingerprint_key';
  const dataColumns = keyed ? columns.slice(1) : columns;
  const keyIndexes = keyColumns ? keyColumns.map((column) => dataColumns.indexOf(column)) : null;
  statement.raw(true);

  const rows = new Map();
  const digest = createHash('sha256');
  let ordinal = 0;
  for (const raw of statement.iterate()) {
    const values = keyed ? raw.slice(1) : raw;
    const encoded = encodeRow(values);
    let key;
    if (keyed) key = String(raw[0]);
    else if (keyIndexes) key = encodeRow(keyIndexes.map((index) => values[index]));
    else key = `#${ordinal}:${encoded}`;
    ordinal += 1;
    rows.set(key, encoded);
    digest.update(key).update('\u0000').update(encoded).update('\u0001');
  }
  return { count: rows.size, digest: digest.digest('hex'), columns: dataColumns, rows };
}

/**
 * Read-only connections kept open across fingerprints, one per database.
 *
 * Keeping them open matters beyond speed. A connection holds a lock on its
 * database for as long as it is open, and a writer that closes its own
 * connection while it can take that database exclusively treats itself as
 * the last user: it checkpoints and deletes the -wal and -shm files, even if
 * another process still writes through them (because that process lost its
 * POSIX locks, e.g. to an unrelated open/close of the file in-process).
 * Writes that process makes afterwards land in an unlinked WAL that no new
 * connection sees, so a reader that opened fresh for every fingerprint would
 * silently stop seeing them. A held connection keeps the WAL in place.
 *
 * Within the process that holds them, nothing may open and close these files
 * directly (that would drop the held locks too): known databases are never
 * re-probed by header.
 */
export function createConnectionCache() {
  const connections = new Map();
  return {
    has: (file) => connections.has(file),
    open(file) {
      let db = connections.get(file);
      if (!db) {
        db = new Database(file, { readonly: true, fileMustExist: true });
        connections.set(file, db);
      }
      return db;
    },
    drop(file) {
      connections.get(file)?.close();
      connections.delete(file);
    },
    /** Close connections to databases no longer present. */
    prune(present) {
      for (const file of [...connections.keys()]) if (!present.has(file)) this.drop(file);
    },
    close() {
      for (const file of [...connections.keys()]) this.drop(file);
    },
  };
}

function snapshotSqlite(file, cache) {
  const db = cache ? cache.open(file) : new Database(file, { readonly: true, fileMustExist: true });
  try {
    const objects = db
      .prepare(
        "SELECT type, name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_autoindex_%' ORDER BY type, name",
      )
      .all();
    const schema = new Map(
      objects.map((object) => [`${object.type}:${object.name}`, object.sql ?? '']),
    );
    const tables = new Map();
    for (const object of objects) {
      if (object.type !== 'table') continue;
      if (/^\s*CREATE\s+VIRTUAL\s+TABLE/i.test(object.sql ?? '')) continue;
      tables.set(object.name, snapshotTable(db, object.name));
    }
    return { kind: 'sqlite', schema, tables };
  } catch (error) {
    cache?.drop(file); // reopen next time rather than reuse a broken handle
    throw error;
  } finally {
    if (!cache) db.close();
  }
}

/**
 * Fingerprint every file under `roots`.
 *
 * @param {string[]} roots - absolute directories (missing ones are skipped)
 * @param {object} [options]
 * @param {(absPath: string) => boolean} [options.appendOnly] - files recorded
 *   by size + tail, so `diffFingerprints` can hand back only their appends
 * @param {ReturnType<typeof createConnectionCache>} [options.connections] -
 *   keep database connections open between calls (see createConnectionCache)
 * @returns {Map<string, object>} absolute path -> entry
 */
export function fingerprintState(roots, { appendOnly = () => false, connections = null } = {}) {
  const entries = new Map();
  const visit = (dir) => {
    let children;
    try {
      children = readdirSync(dir, { withFileTypes: true });
    } catch (error) {
      if (error.code === 'ENOENT') return;
      throw error;
    }
    for (const child of children) {
      const abs = path.join(dir, child.name);
      if (child.isDirectory()) {
        entries.set(abs, { kind: 'dir' });
        visit(abs);
        continue;
      }
      if (child.isSymbolicLink()) {
        entries.set(abs, { kind: 'symlink' });
        continue;
      }
      if (!child.isFile() || SQLITE_SIDECAR.test(child.name)) continue;
      let size;
      try {
        size = statSync(abs).size;
      } catch (error) {
        if (error.code === 'ENOENT') continue; // vanished mid-walk
        throw error;
      }
      if (appendOnly(abs)) {
        entries.set(abs, {
          kind: 'append-only',
          size,
          tail: readRange(abs, Math.max(0, size - TAIL_BYTES), size).toString('base64'),
        });
      } else if (connections?.has(abs) || isSqliteFile(abs, size)) {
        try {
          entries.set(abs, snapshotSqlite(abs, connections));
        } catch (error) {
          // Never fall back to comparing bytes: a WAL database's committed
          // rows need not be in the main file, so a byte hash would be blind.
          entries.set(abs, {
            kind: 'sqlite-unreadable',
            note: `${error.code ?? ''} ${error.message}`,
          });
        }
      } else {
        entries.set(abs, { kind: 'file', digest: sha(readFileSync(abs)) });
      }
    }
  };
  for (const root of roots) visit(root);
  connections?.prune(new Set(entries.keys()));
  return entries;
}

function diffTable(dbPath, name, before, after) {
  if (before.digest === after.digest) return null;
  let added = 0;
  let removed = 0;
  let updated = 0;
  const columns = new Set();
  for (const [key, encoded] of after.rows) {
    const previous = before.rows.get(key);
    if (previous === undefined) added += 1;
    else if (previous !== encoded) {
      updated += 1;
      const was = JSON.parse(previous);
      const now = JSON.parse(encoded);
      after.columns.forEach((column, index) => {
        if (JSON.stringify(was[index]) !== JSON.stringify(now[index])) columns.add(column);
      });
    }
  }
  for (const key of before.rows.keys()) if (!after.rows.has(key)) removed += 1;
  if (added === 0 && removed === 0 && updated === 0) return null; // reordered only
  return {
    path: dbPath,
    kind: 'rows',
    table: name,
    added,
    removed,
    updated,
    ...(columns.size > 0 ? { columns: [...columns].sort() } : {}),
  };
}

function diffSqlite(dbPath, before, after) {
  const changes = [];
  const objects = new Set([...before.schema.keys(), ...after.schema.keys()]);
  for (const object of [...objects].sort()) {
    const was = before.schema.get(object);
    const now = after.schema.get(object);
    if (was === now) continue;
    changes.push({
      path: dbPath,
      kind: 'schema',
      object,
      change: was === undefined ? 'added' : now === undefined ? 'removed' : 'modified',
    });
  }
  const empty = { count: 0, digest: sha(''), columns: [], rows: new Map() };
  const tables = new Set([...before.tables.keys(), ...after.tables.keys()]);
  for (const name of [...tables].sort()) {
    const change = diffTable(
      dbPath,
      name,
      before.tables.get(name) ?? empty,
      after.tables.get(name) ?? empty,
    );
    if (change) changes.push(change);
  }
  return changes;
}

/**
 * Every difference between two fingerprints of the same roots.
 *
 * @param {Map<string, object>} before
 * @param {Map<string, object>} after
 * @param {object} [options]
 * @param {(line: string, absPath: string) => boolean} [options.expectedAppend] -
 *   an appended line of an append-only file that is not a change (e.g. the
 *   MCP replay log's own record of the call being measured)
 * @returns {object[]} change records; empty when nothing changed
 */
export function diffFingerprints(before, after, { expectedAppend = () => false } = {}) {
  const changes = [];
  const paths = new Set([...before.keys(), ...after.keys()]);
  for (const abs of [...paths].sort()) {
    const was = before.get(abs);
    const now = after.get(abs);
    if (now?.kind === 'sqlite-unreadable') {
      // A database that cannot be read cannot be shown unchanged.
      changes.push({ path: abs, kind: now.kind, change: 'unreadable', note: now.note });
      continue;
    }
    if (!was || !now || was.kind !== now.kind) {
      if (was && now)
        changes.push({
          path: abs,
          kind: now.kind,
          change: 'replaced',
          ...(now.note ? { note: now.note } : {}),
        });
      else {
        const entry = now ?? was;
        changes.push({
          path: abs,
          kind: entry.kind,
          change: now ? 'added' : 'removed',
          ...(entry.kind === 'sqlite'
            ? {
                tables: entry.tables.size,
                rows: [...entry.tables.values()].reduce((sum, table) => sum + table.count, 0),
              }
            : {}),
        });
      }
      continue;
    }
    if (now.kind === 'sqlite') changes.push(...diffSqlite(abs, was, now));
    else if (now.kind === 'file' && was.digest !== now.digest)
      changes.push({ path: abs, kind: 'file', change: 'modified' });
    else if (now.kind === 'append-only') {
      if (now.size === was.size && now.tail === was.tail) continue;
      const prefixIntact =
        now.size >= was.size &&
        readRange(abs, Math.max(0, was.size - TAIL_BYTES), was.size).toString('base64') ===
          was.tail;
      if (!prefixIntact) {
        changes.push({ path: abs, kind: 'append-only', change: 'rewritten' });
        continue;
      }
      const appended = readRange(abs, was.size, now.size)
        .toString('utf8')
        .split('\n')
        .filter((line) => line.trim() !== '');
      const unexpected = appended.filter((line) => !expectedAppend(line, abs));
      if (unexpected.length > 0) {
        changes.push({
          path: abs,
          kind: 'append-only',
          change: 'appended',
          lines: unexpected.length,
          sample: unexpected.slice(0, 3).map((line) => line.slice(0, 200)),
        });
      }
    }
  }
  return changes;
}

/** A change record as one short line, paths relative to `root`. */
export function describeChange(change, root = '') {
  const where = root ? path.relative(root, change.path) : change.path;
  switch (change.kind) {
    case 'rows': {
      const parts = [];
      if (change.added) parts.push(`+${change.added}`);
      if (change.removed) parts.push(`-${change.removed}`);
      if (change.updated) parts.push(`~${change.updated}`);
      const columns = change.columns ? ` (${change.columns.join(', ')})` : '';
      return `${where}:${change.table} ${parts.join(' ')} rows${columns}`;
    }
    case 'schema':
      return `${where} schema ${change.change} ${change.object}`;
    case 'append-only':
      return `${where} ${change.change}${change.lines ? ` ${change.lines} unexpected line(s)` : ''}`;
    case 'sqlite-unreadable':
      return `${where} unreadable database (${change.note.trim()})`;
    case 'sqlite':
      if (change.tables !== undefined)
        return `${where} database ${change.change} (${change.tables} tables, ${change.rows} rows)`;
      return `${where} database ${change.change}`;
    default:
      return `${where} ${change.kind} ${change.change}`;
  }
}

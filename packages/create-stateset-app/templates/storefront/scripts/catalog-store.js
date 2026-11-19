import { existsSync } from 'node:fs';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { Commerce } from '@stateset/embedded';

// Opening Commerce can migrate an old schema. Preview a consistent, private copy
// so inspecting another store cannot initialize tables or alter its catalog.
async function snapshot(path, target) {
  const sqlite = process.getBuiltinModule('node:sqlite');
  if (sqlite) {
    const source = new sqlite.DatabaseSync(path, { readOnly: true });
    try {
      if (sqlite.backup) await sqlite.backup(source, target);
      else source.prepare('VACUUM INTO ?').run(target);
    } finally {
      source.close();
    }
    return;
  }
  // Node 20 predates node:sqlite. Python's standard library exposes the same
  // SQLite online-backup operation, including any committed WAL contents.
  const script = [
    'from pathlib import Path',
    'import sqlite3, sys',
    'source = sqlite3.connect(Path(sys.argv[1]).resolve().as_uri() + "?mode=ro", uri=True)',
    'with sqlite3.connect(sys.argv[2]) as target: source.backup(target)',
    'source.close()',
  ].join('\n');
  try {
    await promisify(execFile)('python3', ['-c', script, path, target]);
  } catch (error) {
    if (error.code === 'ENOENT')
      throw new Error(
        'Preview existing stores in the merchant portal or use Node 22.13+ or Python 3 locally.',
      );
    throw error;
  }
}

export async function withCatalogStore(path, apply, run) {
  if (path === ':memory:') throw new Error('Set STATESET_DB_PATH to a persistent store file');
  if (!apply && !existsSync(path)) return run(null);
  let directory, commerce;
  try {
    let file = path;
    if (!apply) {
      directory = await mkdtemp(join(tmpdir(), 'stateset-catalog-preview-'));
      file = join(directory, 'preview.db');
      await snapshot(path, file);
    }
    commerce = new Commerce(file);
    return await run(commerce);
  } finally {
    try {
      if (commerce) await commerce.close();
    } finally {
      if (directory) await rm(directory, { recursive: true, force: true });
    }
  }
}

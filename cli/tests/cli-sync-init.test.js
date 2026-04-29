import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { runNodeScript } from './helpers/run-node-script.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const SYNC_BIN = join(__dirname, '..', 'bin', 'stateset-sync.js');

const tempDirs = new Set();

function createWorkspace() {
  const dir = mkdtempSync(join(tmpdir(), 'stateset-sync-cli-'));
  tempDirs.add(dir);
  mkdirSync(join(dir, '.stateset'), { recursive: true });
  writeFileSync(join(dir, '.stateset', 'sync.json'), JSON.stringify({ existing: true }, null, 2));
  return dir;
}

function runSync(args = [], cwd) {
  return runNodeScript(SYNC_BIN, args, { cwd });
}

afterEach(() => {
  for (const dir of tempDirs) {
    rmSync(dir, { recursive: true, force: true });
  }
  tempDirs.clear();
});

describe('stateset-sync init force reinitialize', () => {
  const initArgs = [
    'init',
    '--sequencer-url',
    'grpcs://127.0.0.1:1',
    '--tenant-id',
    '00000000-0000-0000-0000-000000000001',
    '--store-id',
    '00000000-0000-0000-0000-000000000002',
  ];

  it('requires --force when sync config already exists', () => {
    const cwd = createWorkspace();
    const result = runSync(initArgs, cwd);
    assert.equal(result.status, 1);
    assert.match(result.stdout, /Use --force to reinitialize/i);
  });

  it('reinitializes when --force is supplied', () => {
    const cwd = createWorkspace();
    const dbPath = join(cwd, 'store.db');
    const result = runSync([...initArgs, '--db', dbPath, '--force'], cwd);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /Configuration saved to \.stateset\/sync\.json/i);
  });
});

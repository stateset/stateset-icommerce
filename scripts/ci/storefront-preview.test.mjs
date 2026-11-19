import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const cli = fileURLToPath(new URL('../create-storefront-preview.mjs', import.meta.url));

test('refuses to overwrite a store, including an existing empty directory', (t) => {
  const directory = mkdtempSync(path.join(tmpdir(), 'preview-existing-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const result = spawnSync(process.execPath, [cli, directory], { encoding: 'utf8' });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /Choose a new project directory/);
  assert.equal(existsSync(path.join(directory, 'package.json')), false);
});

test('rejects an altered preview before creating or executing the storefront', {
  skip: process.platform !== 'linux' || process.arch !== 'x64' || !process.report.getReport().header.glibcVersionRuntime,
}, (t) => {
  const directory = mkdtempSync(path.join(tmpdir(), 'preview-invalid-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const file = path.join(directory, 'candidate.tar');
  const project = path.join(directory, 'store');
  writeFileSync(file, 'wrong bundle');
  const result = spawnSync(process.execPath, [cli, project, '--bundle', file], { encoding: 'utf8' });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /SHA-256/);
  assert.equal(existsSync(project), false);
  assert.equal(readFileSync(file, 'utf8'), 'wrong bundle');
});

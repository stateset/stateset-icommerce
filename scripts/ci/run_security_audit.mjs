#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const AUDITS = Object.freeze(['rust', 'deny', 'cli', 'node', 'storefront']);
export const STEP_COUNTS = Object.freeze({ rust: 1, deny: 1, cli: 2, node: 2, storefront: 4 });
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

export function auditCommands(id, root, work) {
  const command = (file, args, cwd = root) => ({ file, args, cwd });
  if (id === 'rust') return [command('bash', ['scripts/ci/check_rust_audits.sh'])];
  if (id === 'deny') return [command('cargo', ['deny', 'check', 'advisories'])];
  if (id === 'cli' || id === 'node') {
    const cwd = path.join(root, id === 'cli' ? 'cli' : 'bindings/node');
    return [command('npm', ['ci'], cwd), command('npm', ['audit', '--audit-level=high'], cwd)];
  }
  if (id === 'storefront') {
    const version = JSON.parse(
      fs.readFileSync(path.join(root, 'packages/create-stateset-app/package.json'), 'utf8'),
    ).version;
    const tarball = path.join(work, `create-stateset-app-${version}.tgz`);
    const project = path.join(work, 'audit-store');
    return [
      command('npm', ['pack', './packages/create-stateset-app', '--pack-destination', work]),
      command('npm', [
        'exec',
        '--yes',
        `--package=${tarball}`,
        '--',
        'create-stateset-app',
        project,
        '--yes',
        '--skip-install',
      ]),
      command('npm', ['install', '--no-fund', '--no-audit'], project),
      command(
        process.execPath,
        [
          path.join(root, 'scripts/ci/enforce_npm_audit_allowlist.mjs'),
          '--level',
          'high',
          '--allowlist',
          path.join(root, 'scripts/ci/npm-audit-allowlist.json'),
        ],
        project,
      ),
    ];
  }
  throw new Error(`Unknown security audit: ${id}`);
}

export function runAudit({ id, root = ROOT, output, sha, execute = spawnSync }) {
  if (!AUDITS.includes(id)) throw new Error(`Unknown security audit: ${id}`);
  if (!/^[a-f0-9]{40}$/.test(sha || '')) throw new Error('A full audited commit SHA is required');
  fs.mkdirSync(output, { recursive: true });
  const logPath = path.join(output, `${id}.log`);
  fs.writeFileSync(logPath, '');
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-security-audit-'));
  const steps = [];
  let success = false;
  const log = (text) => fs.appendFileSync(logPath, text);
  try {
    for (const { file, args, cwd } of auditCommands(id, root, work)) {
      log(`\n+ ${JSON.stringify([file, ...args])}\ncwd: ${cwd}\n`);
      const result = execute(file, args, {
        cwd,
        encoding: 'utf8',
        timeout: 600_000,
        maxBuffer: 16 * 1024 * 1024,
        env: { ...process.env, HUSKY: '0', NO_COLOR: '1', CARGO_TERM_COLOR: 'never' },
      });
      log(result.stdout || '');
      log(result.stderr || '');
      if (result.error) log(`\nInvocation error: ${result.error.message}\n`);
      steps.push({ file, args, cwd, exitCode: result.status, signal: result.signal || null });
      if (result.error || result.signal || result.status !== 0)
        throw new Error(`Audit command failed: ${file} (exit ${result.status})`);
    }
    success = true;
  } catch (error) {
    log(`\n${error.message}\n`);
  } finally {
    // Only this invocation's newly created scratch directory is removed.
    fs.rmSync(work, { recursive: true, force: true });
    fs.writeFileSync(
      path.join(output, `${id}.json`),
      `${JSON.stringify({ schemaVersion: 1, id, sha, success, steps }, null, 2)}\n`,
    );
  }
  process.stdout.write(fs.readFileSync(logPath, 'utf8'));
  return success;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const id = process.argv[2];
  process.exitCode = runAudit({
    id,
    output: path.resolve('artifacts/security-audit'),
    sha: process.env.GITHUB_SHA,
  })
    ? 0
    : 1;
}

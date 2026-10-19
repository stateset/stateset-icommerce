#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const TITLE = 'Embedded engine nightly stress failure';
const MARKER = '<!-- stateset-nightly-embedded-stress:v1 -->';
const BASE_ARGS = ['test', '--locked', '-p', 'stateset-embedded', '--test', 'stress_test', '--'];
export const MINIMUM_STRESS_TESTS = 12;

export function runStress({ output, sha, execute = spawnSync }) {
  if (!/^[a-f0-9]{40}$/.test(sha || '')) throw new Error('A full tested commit SHA is required');
  fs.mkdirSync(output, { recursive: true });
  const logPath = path.join(output, 'stress.log');
  fs.writeFileSync(logPath, '');
  const evidence = { schemaVersion: 1, sha, success: false, selected: [], passed: 0 };
  const run = (args, timeout) => {
    const heading = `\n+ cargo ${args.join(' ')}\n`;
    fs.appendFileSync(logPath, heading);
    process.stdout.write(heading);
    const result = execute('cargo', args, {
      encoding: 'utf8',
      timeout,
      maxBuffer: 16 * 1024 * 1024,
    });
    const text = `${result.stdout || ''}${result.stderr || ''}`;
    fs.appendFileSync(logPath, text);
    process.stdout.write(text);
    if (result.error || result.signal || result.status !== 0)
      throw new Error(
        result.error?.message || `cargo exited ${result.status} (${result.signal || 'no signal'})`,
      );
    return result.stdout || '';
  };
  try {
    const list = run([...BASE_ARGS, '--ignored', '--list'], 40 * 60_000);
    evidence.selected = [...list.matchAll(/^([^\s]+): test$/gm)].map((match) => match[1]);
    if (
      new Set(evidence.selected).size !== evidence.selected.length ||
      evidence.selected.length < MINIMUM_STRESS_TESTS
    )
      throw new Error(
        `Expected at least ${MINIMUM_STRESS_TESTS} distinct ignored stress tests; found ${evidence.selected.length}`,
      );
    // Serialize test cases for stable resource use. Each concurrency test still
    // runs its own actual competing engine callers and keeps every assertion.
    const log = run([...BASE_ARGS, '--ignored', '--nocapture', '--test-threads=1'], 20 * 60_000);
    const summary = log.match(
      /^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;/m,
    );
    if (
      !summary ||
      Number(summary[1]) !== evidence.selected.length ||
      summary.slice(2).some((count) => Number(count) !== 0)
    )
      throw new Error(
        'Incomplete stress acceptance: every discovered ignored test must pass without filtering or skipping',
      );
    evidence.passed = Number(summary[1]);
    evidence.success = true;
  } catch (error) {
    evidence.error = error.message;
    fs.appendFileSync(logPath, `\n${error.message}\n`);
    process.stderr.write(`${error.message}\n`);
  } finally {
    fs.writeFileSync(path.join(output, 'stress.json'), `${JSON.stringify(evidence, null, 2)}\n`);
  }
  return evidence;
}

export function readStressEvidence({ directory, sha, result }) {
  let success = false;
  let description;
  try {
    const evidence = JSON.parse(fs.readFileSync(path.join(directory, 'stress.json'), 'utf8'));
    success =
      result === 'success' &&
      evidence.schemaVersion === 1 &&
      evidence.sha === sha &&
      /^[a-f0-9]{40}$/.test(sha || '') &&
      evidence.success === true &&
      Array.isArray(evidence.selected) &&
      evidence.selected.length >= MINIMUM_STRESS_TESTS &&
      evidence.selected.every((name) => typeof name === 'string' && name.length > 0) &&
      new Set(evidence.selected).size === evidence.selected.length &&
      evidence.passed === evidence.selected.length;
    description = success
      ? `All ${evidence.passed} discovered ignored stress tests passed.`
      : 'The engine stress run failed or produced incomplete acceptance evidence.';
  } catch {
    description = 'The engine stress run produced no valid acceptance evidence.';
  }
  let log;
  try {
    log = fs.readFileSync(path.join(directory, 'stress.log'), 'utf8');
  } catch {
    log = 'No stress log was uploaded.';
    description = 'The stress log is missing; acceptance is incomplete.';
    success = false;
  }
  if (!log.trim()) {
    success = false;
    description = 'The stress log is empty.';
  }
  return { success, description, log };
}

export async function reportStress({ github, context, core, directory, sha, result, runUrl }) {
  if (
    !/^[a-f0-9]{40}$/.test(sha || '') ||
    !/^https:\/\/[a-z0-9.-]+\/[^/]+\/[^/]+\/actions\/runs\/\d+\/attempts\/\d+$/i.test(runUrl || '')
  )
    throw new Error('Invalid stress run identity');
  const evidence = readStressEvidence({ directory, sha, result });
  const escaped = evidence.log
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/@/g, '&#64;');
  const body = `${MARKER}\n\nNightly embedded-engine stress checks at \`${sha}\`.\n\n${evidence.description}\n\n[Run and complete logs](${runUrl}).\n\n<pre>${escaped.slice(-12000)}</pre>${escaped.length > 12000 ? '\n\nExcerpt truncated; complete logs are retained as run artifacts.' : ''}`;
  const scope = { owner: context.repo.owner, repo: context.repo.repo };
  const issues = await github.paginate(github.rest.issues.listForRepo, {
    ...scope,
    state: 'open',
    per_page: 100,
  });
  const matches = issues.filter(
    (issue) => !issue.pull_request && issue.user?.type === 'Bot' && issue.body?.includes(MARKER),
  );
  if (matches.length > 1) throw new Error('Multiple stress tracking issues found');
  const issue = matches[0];
  if (issue)
    await github.rest.issues.update({ ...scope, issue_number: issue.number, title: TITLE, body });
  else if (!evidence.success) await github.rest.issues.create({ ...scope, title: TITLE, body });
  if (evidence.success && issue)
    await github.rest.issues.update({
      ...scope,
      issue_number: issue.number,
      state: 'closed',
      state_reason: 'completed',
    });
  if (!evidence.success)
    core.setFailed('Nightly embedded engine stress failed; see the tracking issue.');
  return evidence;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = runStress({
    output: path.resolve('artifacts/nightly-stress'),
    sha: process.env.GITHUB_SHA,
  }).success
    ? 0
    : 1;
}

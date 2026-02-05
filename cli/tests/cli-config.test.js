import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const CLI_PATH = join(__dirname, '..', 'bin', 'stateset-config.js');

function runCli(args, env) {
  const result = spawnSync('node', [CLI_PATH, ...args], {
    encoding: 'utf-8',
    env: { ...process.env, ...env },
  });

  if (result.error) {
    throw result.error;
  }

  return result;
}

function parseJson(output) {
  try {
    return JSON.parse(output);
  } catch (error) {
    throw new Error(`Failed to parse JSON output: ${error.message}\nOutput:\n${output}`);
  }
}

describe('stateset-config CLI', () => {
  let homeDir;
  let env;

  before(() => {
    homeDir = mkdtempSync(join(tmpdir(), 'stateset-config-'));
    env = { HOME: homeDir };
  });

  after(() => {
    if (homeDir) {
      rmSync(homeDir, { recursive: true, force: true });
    }
  });

  it('path command emits JSON paths', () => {
    const result = runCli(['path', '--json'], env);
    assert.equal(result.status, 0, result.stderr);

    const payload = parseJson(result.stdout.trim());
    assert.ok(payload.configDir);
    assert.ok(payload.configFile);
    assert.ok(payload.profilesDir);
    assert.ok(payload.configDir.startsWith(homeDir));
  });

  it('create and list profiles via JSON', () => {
    const createResult = runCli(['create', 'dev', '--json'], env);
    assert.equal(createResult.status, 0, createResult.stderr);

    const created = parseJson(createResult.stdout.trim());
    assert.equal(created.profile, 'dev');
    assert.equal(created.created, true);

    const listResult = runCli(['list', '--json'], env);
    assert.equal(listResult.status, 0, listResult.stderr);

    const list = parseJson(listResult.stdout.trim());
    assert.ok(Array.isArray(list.profiles));
    assert.ok(list.profiles.includes('dev'));
    assert.equal(list.default, 'default');
  });

  it('set and get config values', () => {
    const setResult = runCli(['set', 'db', './test.db', '--profile', 'dev', '--json'], env);
    assert.equal(setResult.status, 0, setResult.stderr);

    const setPayload = parseJson(setResult.stdout.trim());
    assert.equal(setPayload.profile, 'dev');
    assert.equal(setPayload.key, 'db');
    assert.equal(setPayload.value, './test.db');

    const getResult = runCli(['get', 'db', '--profile', 'dev', '--json'], env);
    assert.equal(getResult.status, 0, getResult.stderr);

    const getPayload = parseJson(getResult.stdout.trim());
    assert.equal(getPayload.db, './test.db');

    const setBool = runCli(['set', 'apply', 'true', '--profile', 'dev', '--json'], env);
    assert.equal(setBool.status, 0, setBool.stderr);

    const boolPayload = parseJson(setBool.stdout.trim());
    assert.equal(boolPayload.value, true);
  });

  it('get returns missing key as error', () => {
    const result = runCli(['get', 'missing-key', '--json'], env);
    assert.equal(result.status, 1);

    const payload = parseJson(result.stdout.trim());
    assert.ok(payload.error);
  });

});

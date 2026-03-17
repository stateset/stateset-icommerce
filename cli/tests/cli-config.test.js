import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { runNodeScript } from './helpers/run-node-script.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const CLI_PATH = join(__dirname, '..', 'bin', 'stateset-config.js');

function runCli(args, env, input) {
  return runNodeScript(CLI_PATH, args, { env, input });
}

function parseJson(output) {
  const trimmed = output.trim();
  const firstBrace = trimmed.indexOf('{');
  const candidate = firstBrace >= 0 ? trimmed.slice(firstBrace) : trimmed;
  try {
    return JSON.parse(candidate);
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

  it('rejects invalid profile names', () => {
    const result = runCli(['create', '../prod', '--json'], env);
    assert.equal(result.status, 1);

    const payload = parseJson(result.stdout.trim());
    assert.match(payload.error, /invalid profile name/i);
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

  it('rejects unknown config keys unless --force is provided', () => {
    const strictResult = runCli(['set', 'modle', 'claude-opus', '--json'], env);
    assert.equal(strictResult.status, 1);
    const strictPayload = parseJson(strictResult.stdout.trim());
    assert.match(strictPayload.error, /Unknown config key/i);

    const forcedResult = runCli(['set', 'modle', 'claude-opus', '--force', '--json'], env);
    assert.equal(forcedResult.status, 0, forcedResult.stderr);
    const forcedPayload = parseJson(forcedResult.stdout.trim());
    assert.equal(forcedPayload.key, 'modle');
    assert.equal(forcedPayload.value, 'claude-opus');
  });

  it('recovers from corrupt config JSON by backing it up', () => {
    const configDir = join(homeDir, '.stateset');
    mkdirSync(configDir, { recursive: true });
    const configPath = join(configDir, 'config.json');
    writeFileSync(configPath, '{ invalid json');

    const result = runCli(['list', '--json'], env);
    assert.equal(result.status, 0, result.stderr);
    const payload = parseJson(result.stdout.trim());
    assert.ok(Array.isArray(payload.profiles));

    const files = readdirSync(configDir);
    assert.ok(files.some((name) => name.startsWith('config.json.corrupt-')));
  });

  it('preserves existing .env lines and enforces restrictive permissions when setting keys', () => {
    const configDir = join(homeDir, '.stateset');
    mkdirSync(configDir, { recursive: true });
    const envPath = join(configDir, '.env');
    writeFileSync(envPath, '# keep me\nCUSTOM_TOKEN="abc123"\n', { mode: 0o644 });

    const apiKey = 'sk-ant-test1234567890';
    const result = runCli(['set-key', 'anthropic'], env, `${apiKey}\n`);
    assert.equal(result.status, 0, result.stderr);

    const content = readFileSync(envPath, 'utf-8');
    assert.match(content, /# keep me/);
    assert.match(content, /CUSTOM_TOKEN="abc123"/);
    assert.match(content, /ANTHROPIC_API_KEY="sk-ant-test1234567890"/);

    const mode = statSync(envPath).mode & 0o777;
    assert.equal(mode, 0o600);
  });

  it('get returns missing key as error', () => {
    const result = runCli(['get', 'missing-key', '--json'], env);
    assert.equal(result.status, 1);

    const payload = parseJson(result.stdout.trim());
    assert.ok(payload.error);
  });

  it('show returns an error for an unknown explicit profile', () => {
    const result = runCli(['show', 'missing-profile', '--json'], env);
    assert.equal(result.status, 1);

    const payload = parseJson(result.stdout.trim());
    assert.match(payload.error, /profile 'missing-profile' not found/i);
  });

});

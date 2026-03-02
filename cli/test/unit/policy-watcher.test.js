import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'fs';
import path from 'path';
import os from 'os';

import { PolicyEngine } from '../../src/policies/engine.js';
import { watchPolicies } from '../../src/policies/watcher.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeTmpDir() {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'policy-watcher-test-'));
}

function writePolicyFile(dir, filename, policyData) {
  const ext = path.extname(filename);
  const content = ext === '.json'
    ? JSON.stringify(policyData, null, 2)
    : `name: ${policyData.name}\ndomain: ${policyData.domain}\nrules: []`;
  fs.writeFileSync(path.join(dir, filename), content, 'utf-8');
}

const SAMPLE_POLICY = {
  name: 'Test Policy',
  domain: 'returns',
  rules: [
    {
      name: 'test_rule',
      conditions: { logic: 'and', conditions: [{ field: 'amount', operator: 'lt', value: 100 }] },
      action: { type: 'allow' },
    },
  ],
};

// ---------------------------------------------------------------------------
// Constructor validation
// ---------------------------------------------------------------------------

describe('watchPolicies — validation', () => {
  it('throws when engine is null', () => {
    assert.throws(() => watchPolicies(null, '/tmp'), /engine is required/);
  });

  it('throws when policiesDir is null', () => {
    const engine = new PolicyEngine();
    assert.throws(() => watchPolicies(engine, null), /policiesDir is required/);
  });

  it('creates directory if it does not exist', () => {
    const engine = new PolicyEngine();
    const tmpDir = path.join(os.tmpdir(), `policy-watcher-nodir-${Date.now()}`);
    const handle = watchPolicies(engine, tmpDir);
    assert.ok(fs.existsSync(tmpDir));
    handle.stop();
    fs.rmdirSync(tmpDir);
  });
});

// ---------------------------------------------------------------------------
// Watcher lifecycle
// ---------------------------------------------------------------------------

describe('watchPolicies — lifecycle', () => {
  let tmpDir;
  let engine;

  beforeEach(() => {
    tmpDir = makeTmpDir();
    engine = new PolicyEngine();
  });

  afterEach(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('returns stop and isWatching methods', () => {
    const handle = watchPolicies(engine, tmpDir);
    assert.equal(typeof handle.stop, 'function');
    assert.equal(typeof handle.isWatching, 'function');
    assert.equal(handle.isWatching(), true);
    handle.stop();
    assert.equal(handle.isWatching(), false);
  });

  it('returns reload method', () => {
    const handle = watchPolicies(engine, tmpDir);
    assert.equal(typeof handle.reload, 'function');
    handle.stop();
  });

  it('can be stopped safely multiple times', () => {
    const handle = watchPolicies(engine, tmpDir);
    handle.stop();
    assert.doesNotThrow(() => handle.stop());
  });
});

// ---------------------------------------------------------------------------
// Manual reload
// ---------------------------------------------------------------------------

describe('watchPolicies — reload', () => {
  let tmpDir;
  let engine;

  beforeEach(() => {
    tmpDir = makeTmpDir();
    engine = new PolicyEngine();
  });

  afterEach(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('loads JSON policy files on reload', () => {
    writePolicyFile(tmpDir, 'test-policy.json', SAMPLE_POLICY);
    const handle = watchPolicies(engine, tmpDir);
    handle.reload();

    const policies = engine.listPolicySets();
    assert.ok(policies.length >= 1);
    assert.ok(policies.some((p) => p.name === 'Test Policy'));
    handle.stop();
  });

  it('loads YAML policy files on reload', () => {
    const yamlContent = `name: YAML Policy\ndomain: orders\nrules:\n  - name: yaml_rule\n    conditions:\n      logic: and\n      conditions:\n        - field: amount\n          operator: gt\n          value: 0\n    action:\n      type: allow\n`;
    fs.writeFileSync(path.join(tmpDir, 'test.yaml'), yamlContent, 'utf-8');

    const handle = watchPolicies(engine, tmpDir);
    handle.reload();

    const policies = engine.listPolicySets();
    assert.ok(policies.some((p) => p.name === 'YAML Policy'));
    handle.stop();
  });

  it('ignores non-policy files', () => {
    fs.writeFileSync(path.join(tmpDir, 'readme.txt'), 'not a policy', 'utf-8');
    writePolicyFile(tmpDir, 'real-policy.json', SAMPLE_POLICY);

    const handle = watchPolicies(engine, tmpDir);
    handle.reload();

    const policies = engine.listPolicySets();
    assert.equal(policies.length, 1);
    handle.stop();
  });

  it('calls onReload callback', () => {
    writePolicyFile(tmpDir, 'cb-policy.json', SAMPLE_POLICY);
    let reloadInfo = null;

    const handle = watchPolicies(engine, tmpDir, {
      onReload: (info) => { reloadInfo = info; },
    });
    handle.reload();

    assert.ok(reloadInfo);
    assert.equal(reloadInfo.fileCount, 1);
    assert.ok(reloadInfo.policyIds.length >= 1);
    handle.stop();
  });

  it('calls onError callback for malformed files', () => {
    fs.writeFileSync(path.join(tmpDir, 'bad.json'), '{ invalid json', 'utf-8');
    let errorCaught = null;

    const handle = watchPolicies(engine, tmpDir, {
      onError: (err) => { errorCaught = err; },
    });
    handle.reload();

    assert.ok(errorCaught);
    handle.stop();
  });

  it('emits reloaded event on engine', () => {
    writePolicyFile(tmpDir, 'emit-policy.json', SAMPLE_POLICY);
    let emitted = null;
    engine.on('reloaded', (info) => { emitted = info; });

    const handle = watchPolicies(engine, tmpDir);
    handle.reload();

    assert.ok(emitted);
    assert.equal(emitted.fileCount, 1);
    handle.stop();
  });
});

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'fs';
import path from 'path';
import os from 'os';

import { PolicyEngine, PolicyTemplates } from '../../src/policies/engine.js';
import { policyTools } from '../../src/tools/policies.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function findTool(name) {
  return policyTools.find((t) => t.name === name);
}

function callTool(name, params, policyEngine) {
  const tool = findTool(name);
  return tool.handler({ params, policyEngine });
}

function makeTmpDir() {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'policy-tools-test-'));
}

const SAMPLE_POLICY = {
  name: 'Test Policy',
  domain: 'returns',
  rules: [
    {
      name: 'auto_approve_small',
      conditions: {
        logic: 'and',
        conditions: [{ field: 'return.value', operator: 'lt', value: 100 }],
      },
      action: { type: 'allow' },
    },
    {
      name: 'deny_large',
      conditions: {
        logic: 'and',
        conditions: [{ field: 'return.value', operator: 'gte', value: 100 }],
      },
      action: {
        type: 'deny',
        reason: 'Amount too large',
        remediation: 'Requires manager approval',
      },
    },
  ],
};

// ---------------------------------------------------------------------------
// Tool registration
// ---------------------------------------------------------------------------

describe('policy tools — registration', () => {
  it('exports 5 tools', () => {
    assert.equal(policyTools.length, 5);
  });

  it('has evaluate_policy', () => {
    assert.ok(findTool('evaluate_policy'));
  });

  it('has list_policies', () => {
    assert.ok(findTool('list_policies'));
  });

  it('has register_policy_template', () => {
    assert.ok(findTool('register_policy_template'));
  });

  it('has load_policy_file', () => {
    assert.ok(findTool('load_policy_file'));
  });

  it('has explain_policy_denial', () => {
    assert.ok(findTool('explain_policy_denial'));
  });
});

// ---------------------------------------------------------------------------
// evaluate_policy
// ---------------------------------------------------------------------------

describe('policy tools — evaluate_policy', () => {
  let engine;

  beforeEach(() => {
    engine = new PolicyEngine();
    engine.registerPolicySet(SAMPLE_POLICY);
  });

  it('returns error when no policy engine', async () => {
    const result = await callTool('evaluate_policy', { domain: 'returns', context: {} }, null);
    assert.equal(result.success, false);
    assert.match(result.error, /not initialized/);
  });

  it('evaluates allow decision', async () => {
    const result = await callTool(
      'evaluate_policy',
      {
        domain: 'returns',
        context: { return: { value: 50 } },
      },
      engine,
    );
    assert.equal(result.success, true);
    assert.equal(result.decision, 'allow');
    assert.equal(result.shouldAllow, true);
  });

  it('evaluates deny decision', async () => {
    const result = await callTool(
      'evaluate_policy',
      {
        domain: 'returns',
        context: { return: { value: 200 } },
      },
      engine,
    );
    assert.equal(result.success, true);
    assert.equal(result.decision, 'deny');
    assert.equal(result.shouldDeny, true);
  });

  it('handles unknown domain in deny mode', async () => {
    const result = await callTool(
      'evaluate_policy',
      {
        domain: 'nonexistent',
        context: {},
      },
      engine,
    );
    assert.equal(result.success, true);
    assert.equal(result.unknownDomain, true);
    assert.equal(result.decision, 'deny');
  });

  it('handles unknown domain in allow mode', async () => {
    const allowEngine = new PolicyEngine({ unknownDomainMode: 'allow' });
    const result = await callTool(
      'evaluate_policy',
      {
        domain: 'nonexistent',
        context: {},
      },
      allowEngine,
    );
    assert.equal(result.success, true);
    assert.equal(result.unknownDomain, true);
    assert.equal(result.decision, 'allow');
  });

  it('supports dry-run', async () => {
    const result = await callTool(
      'evaluate_policy',
      {
        domain: 'returns',
        context: { 'return.value': 50 },
        dryRun: true,
      },
      engine,
    );
    assert.equal(result.success, true);
    assert.equal(result.dryRun, true);
  });
});

// ---------------------------------------------------------------------------
// list_policies
// ---------------------------------------------------------------------------

describe('policy tools — list_policies', () => {
  let engine;

  beforeEach(() => {
    engine = new PolicyEngine();
  });

  it('returns error when no policy engine', async () => {
    const result = await callTool('list_policies', {}, null);
    assert.equal(result.success, false);
  });

  it('lists empty when no policies', async () => {
    const result = await callTool('list_policies', {}, engine);
    assert.equal(result.success, true);
    assert.equal(result.count, 0);
    assert.deepEqual(result.policySets, []);
  });

  it('lists registered policies', async () => {
    engine.registerPolicySet(SAMPLE_POLICY);
    const result = await callTool('list_policies', {}, engine);
    assert.equal(result.success, true);
    assert.equal(result.count, 1);
    assert.equal(result.policySets[0].name, 'Test Policy');
    assert.equal(result.policySets[0].domain, 'returns');
  });

  it('filters by domain', async () => {
    engine.registerPolicySet(SAMPLE_POLICY);
    engine.registerPolicySet({ ...SAMPLE_POLICY, name: 'Other', domain: 'orders' });

    const result = await callTool('list_policies', { domain: 'returns' }, engine);
    assert.equal(result.count, 1);
    assert.equal(result.policySets[0].domain, 'returns');
  });

  it('includes unknownDomainMode', async () => {
    const result = await callTool('list_policies', {}, engine);
    assert.equal(result.unknownDomainMode, 'deny');
  });
});

// ---------------------------------------------------------------------------
// register_policy_template
// ---------------------------------------------------------------------------

describe('policy tools — register_policy_template', () => {
  let engine;

  beforeEach(() => {
    engine = new PolicyEngine();
  });

  it('returns error when no policy engine', async () => {
    const result = await callTool(
      'register_policy_template',
      { templateName: 'autoApproveReturns' },
      null,
    );
    assert.equal(result.success, false);
  });

  it('registers autoApproveReturns template', async () => {
    const result = await callTool(
      'register_policy_template',
      { templateName: 'autoApproveReturns' },
      engine,
    );
    assert.equal(result.success, true);
    assert.ok(result.policySet.id);
    assert.equal(result.policySet.domain, 'returns');
  });

  it('registers inventoryRestock template', async () => {
    const result = await callTool(
      'register_policy_template',
      { templateName: 'inventoryRestock' },
      engine,
    );
    assert.equal(result.success, true);
    assert.equal(result.policySet.domain, 'inventory');
  });

  it('registers orderFraudDetection template', async () => {
    const result = await callTool(
      'register_policy_template',
      { templateName: 'orderFraudDetection' },
      engine,
    );
    assert.equal(result.success, true);
  });

  it('registers promotionEligibility template', async () => {
    const result = await callTool(
      'register_policy_template',
      { templateName: 'promotionEligibility' },
      engine,
    );
    assert.equal(result.success, true);
  });

  it('registers subscriptionRules template', async () => {
    const result = await callTool(
      'register_policy_template',
      { templateName: 'subscriptionRules' },
      engine,
    );
    assert.equal(result.success, true);
  });
});

// ---------------------------------------------------------------------------
// load_policy_file
// ---------------------------------------------------------------------------

describe('policy tools — load_policy_file', () => {
  let engine;
  let tmpDir;

  beforeEach(() => {
    engine = new PolicyEngine();
    tmpDir = makeTmpDir();
  });

  it('returns error when no policy engine', async () => {
    const result = await callTool('load_policy_file', { filePath: '/nonexistent.json' }, null);
    assert.equal(result.success, false);
  });

  it('returns error for missing file', async () => {
    const result = await callTool('load_policy_file', { filePath: '/no/such/file.json' }, engine);
    assert.equal(result.success, false);
    assert.match(result.error, /not found/i);
  });

  it('returns error for unsupported extension', async () => {
    const filePath = path.join(tmpDir, 'policy.txt');
    fs.writeFileSync(filePath, 'text', 'utf-8');
    const result = await callTool('load_policy_file', { filePath }, engine);
    assert.equal(result.success, false);
    assert.match(result.error, /\.yaml.*\.yml.*\.json/);
  });

  it('loads a JSON policy file', async () => {
    const filePath = path.join(tmpDir, 'test.json');
    fs.writeFileSync(filePath, JSON.stringify(SAMPLE_POLICY), 'utf-8');
    const result = await callTool('load_policy_file', { filePath }, engine);
    assert.equal(result.success, true);
    assert.equal(result.policySet.name, 'Test Policy');
    assert.equal(result.policySet.ruleCount, 2);
  });

  it('loads a YAML policy file', async () => {
    const filePath = path.join(tmpDir, 'test.yaml');
    const yamlContent = `name: YAML Test\ndomain: orders\nrules:\n  - name: rule1\n    conditions:\n      logic: and\n      conditions:\n        - field: amount\n          operator: gt\n          value: 0\n    action:\n      type: allow\n`;
    fs.writeFileSync(filePath, yamlContent, 'utf-8');
    const result = await callTool('load_policy_file', { filePath }, engine);
    assert.equal(result.success, true);
    assert.equal(result.policySet.name, 'YAML Test');
  });

  it('returns error for malformed JSON', async () => {
    const filePath = path.join(tmpDir, 'bad.json');
    fs.writeFileSync(filePath, '{ invalid }', 'utf-8');
    const result = await callTool('load_policy_file', { filePath }, engine);
    assert.equal(result.success, false);
    assert.match(result.error, /Failed to load/);
  });

  // Cleanup
  it('cleanup tmp dir', () => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });
});

// ---------------------------------------------------------------------------
// explain_policy_denial
// ---------------------------------------------------------------------------

describe('policy tools — explain_policy_denial', () => {
  let engine;

  beforeEach(() => {
    engine = new PolicyEngine();
    engine.registerPolicySet(SAMPLE_POLICY);
  });

  it('returns error when no policy engine', async () => {
    const result = await callTool(
      'explain_policy_denial',
      { domain: 'returns', context: {} },
      null,
    );
    assert.equal(result.success, false);
  });

  it('explains an allowed evaluation', async () => {
    const result = await callTool(
      'explain_policy_denial',
      {
        domain: 'returns',
        context: { return: { value: 50 } },
      },
      engine,
    );
    assert.equal(result.success, true);
    assert.equal(result.decision, 'allow');
    assert.ok(Array.isArray(result.breakdown));
  });

  it('explains a denied evaluation', async () => {
    const result = await callTool(
      'explain_policy_denial',
      {
        domain: 'returns',
        context: { return: { value: 200 } },
      },
      engine,
    );
    assert.equal(result.success, true);
    assert.equal(result.decision, 'deny');
    assert.ok(Array.isArray(result.breakdown));
  });

  it('explains unknown domain in deny mode', async () => {
    const result = await callTool(
      'explain_policy_denial',
      {
        domain: 'nonexistent',
        context: {},
      },
      engine,
    );
    assert.equal(result.success, true);
    assert.equal(result.unknownDomain, true);
    assert.equal(result.decision, 'deny');
    assert.deepEqual(result.breakdown, []);
  });

  it('explains unknown domain in allow mode', async () => {
    const allowEngine = new PolicyEngine({ unknownDomainMode: 'allow' });
    const result = await callTool(
      'explain_policy_denial',
      {
        domain: 'nonexistent',
        context: {},
      },
      allowEngine,
    );
    assert.equal(result.success, true);
    assert.equal(result.unknownDomain, true);
    assert.equal(result.decision, 'allow');
  });

  it('does not record in evaluation history (uses dry-run)', async () => {
    await callTool(
      'explain_policy_denial',
      {
        domain: 'returns',
        context: { return: { value: 50 } },
      },
      engine,
    );
    // explain uses dry-run internally, so no history recorded for the explanation call
    const history = engine.getHistory();
    assert.equal(history.length, 0);
  });

  it('includes breakdown with conditions', async () => {
    const result = await callTool(
      'explain_policy_denial',
      {
        domain: 'returns',
        context: { return: { value: 200 } },
      },
      engine,
    );
    assert.ok(result.breakdown.length > 0);
    const deniedRule = result.breakdown.find((b) => b.ruleName === 'deny_large');
    assert.ok(deniedRule, 'deny_large rule should be in breakdown');
    assert.equal(deniedRule.matched, true);
  });
});

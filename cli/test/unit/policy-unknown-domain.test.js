import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import { PolicyEngine, PolicyTemplates } from '../../src/policies/engine.js';

// ---------------------------------------------------------------------------
// Constructor — unknownDomainMode
// ---------------------------------------------------------------------------

describe('PolicyEngine — unknownDomainMode constructor', () => {
  it('defaults to deny', () => {
    const engine = new PolicyEngine({});
    assert.equal(engine.unknownDomainMode, 'deny');
  });

  it('accepts allow', () => {
    const engine = new PolicyEngine({ unknownDomainMode: 'allow' });
    assert.equal(engine.unknownDomainMode, 'allow');
  });

  it('accepts deny', () => {
    const engine = new PolicyEngine({ unknownDomainMode: 'deny' });
    assert.equal(engine.unknownDomainMode, 'deny');
  });

  it('throws on invalid mode', () => {
    assert.throws(
      () => new PolicyEngine({ unknownDomainMode: 'maybe' }),
      /must be 'allow' or 'deny'/,
    );
  });

  it('throws on numeric mode', () => {
    assert.throws(
      () => new PolicyEngine({ unknownDomainMode: 1 }),
      /must be 'allow' or 'deny'/,
    );
  });

  it('works with no arguments (default constructor)', () => {
    const engine = new PolicyEngine();
    assert.equal(engine.unknownDomainMode, 'deny');
  });
});

// ---------------------------------------------------------------------------
// evaluate() — unknown domain with deny mode
// ---------------------------------------------------------------------------

describe('PolicyEngine — evaluate unknown domain (deny mode)', () => {
  let engine;

  beforeEach(() => {
    engine = new PolicyEngine({ unknownDomainMode: 'deny' });
  });

  it('denies when no policies exist for domain', async () => {
    const result = await engine.evaluate('nonexistent_domain', { foo: 'bar' });
    assert.equal(result.shouldDeny, true);
    assert.equal(result.shouldAllow, false);
  });

  it('includes unknownDomain flag', async () => {
    const result = await engine.evaluate('nonexistent_domain', {});
    assert.equal(result.unknownDomain, true);
  });

  it('includes unknownDomainMode in result', async () => {
    const result = await engine.evaluate('nonexistent_domain', {});
    assert.equal(result.unknownDomainMode, 'deny');
  });

  it('includes reason string', async () => {
    const result = await engine.evaluate('nonexistent_domain', {});
    assert.ok(result.reason);
    assert.match(result.reason, /deny/);
    assert.match(result.reason, /nonexistent_domain/);
  });

  it('returns empty arrays for results, actions, explanations', async () => {
    const result = await engine.evaluate('nonexistent_domain', {});
    assert.deepEqual(result.results, []);
    assert.deepEqual(result.actions, []);
    assert.deepEqual(result.explanations, []);
  });

  it('records in evaluation history', async () => {
    await engine.evaluate('nonexistent_domain', { key: 'val' });
    const history = engine.getHistory();
    assert.equal(history.length, 1);
    assert.equal(history[0].unknownDomain, true);
    assert.equal(history[0].mode, 'deny');
  });

  it('does not record in history for dry-run', async () => {
    await engine.evaluate('nonexistent_domain', {}, { dryRun: true });
    const history = engine.getHistory();
    assert.equal(history.length, 0);
  });

  it('emits evaluated event with unknownDomain', async () => {
    let emitted = null;
    engine.on('evaluated', (e) => { emitted = e; });
    await engine.evaluate('nonexistent_domain', {});
    assert.ok(emitted);
    assert.equal(emitted.unknownDomain, true);
    assert.equal(emitted.mode, 'deny');
  });

  it('still evaluates normally when policies exist', async () => {
    engine.registerPolicySet(PolicyTemplates.autoApproveReturns);
    const result = await engine.evaluate('returns', { return: { value: 50 }, customer: { lifetimeValue: 1000 } });
    assert.equal(result.unknownDomain, undefined);
    assert.equal(result.shouldAllow, true);
  });
});

// ---------------------------------------------------------------------------
// evaluate() — unknown domain with allow mode
// ---------------------------------------------------------------------------

describe('PolicyEngine — evaluate unknown domain (allow mode)', () => {
  let engine;

  beforeEach(() => {
    engine = new PolicyEngine({ unknownDomainMode: 'allow' });
  });

  it('allows when no policies exist for domain', async () => {
    const result = await engine.evaluate('nonexistent_domain', { foo: 'bar' });
    assert.equal(result.shouldAllow, true);
    assert.equal(result.shouldDeny, false);
  });

  it('includes unknownDomain flag', async () => {
    const result = await engine.evaluate('nonexistent_domain', {});
    assert.equal(result.unknownDomain, true);
  });

  it('includes unknownDomainMode = allow', async () => {
    const result = await engine.evaluate('nonexistent_domain', {});
    assert.equal(result.unknownDomainMode, 'allow');
  });

  it('includes reason string mentioning allow', async () => {
    const result = await engine.evaluate('nonexistent_domain', {});
    assert.match(result.reason, /allow/);
    assert.match(result.reason, /passing through/);
  });

  it('records in history with allow mode', async () => {
    await engine.evaluate('nonexistent_domain', {});
    const history = engine.getHistory();
    assert.equal(history.length, 1);
    assert.equal(history[0].mode, 'allow');
  });
});

// ---------------------------------------------------------------------------
// evaluateDryRun() — unknownDomainMode
// ---------------------------------------------------------------------------

describe('PolicyEngine — evaluateDryRun unknown domain', () => {
  it('returns deny for deny mode without recording history', async () => {
    const engine = new PolicyEngine({ unknownDomainMode: 'deny' });
    const result = await engine.evaluateDryRun('unknown', {});
    assert.equal(result.shouldDeny, true);
    assert.equal(result.dryRun, true);
    assert.equal(result.unknownDomain, true);
    assert.equal(engine.getHistory().length, 0);
  });

  it('returns allow for allow mode without recording history', async () => {
    const engine = new PolicyEngine({ unknownDomainMode: 'allow' });
    const result = await engine.evaluateDryRun('unknown', {});
    assert.equal(result.shouldAllow, true);
    assert.equal(result.dryRun, true);
    assert.equal(result.unknownDomain, true);
    assert.equal(engine.getHistory().length, 0);
  });

  it('includes context in dry-run result', async () => {
    const engine = new PolicyEngine({ unknownDomainMode: 'deny' });
    const ctx = { foo: 'bar' };
    const result = await engine.evaluateDryRun('unknown', ctx);
    assert.deepEqual(result.context, ctx);
  });
});

// ---------------------------------------------------------------------------
// evaluateAndExecute() — unknownDomainMode interaction
// ---------------------------------------------------------------------------

describe('PolicyEngine — evaluateAndExecute with unknownDomainMode', () => {
  it('blocks on unknown domain in deny mode', async () => {
    const engine = new PolicyEngine({ unknownDomainMode: 'deny' });
    const result = await engine.evaluateAndExecute('unknown_domain', {});
    assert.equal(result.allowed, false);
  });

  it('allows on unknown domain in allow mode', async () => {
    const engine = new PolicyEngine({ unknownDomainMode: 'allow' });
    const result = await engine.evaluateAndExecute('unknown_domain', {});
    assert.equal(result.allowed, true);
    assert.deepEqual(result.executed, []);
  });
});

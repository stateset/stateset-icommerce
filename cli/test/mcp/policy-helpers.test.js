// Unit tests for cli/src/mcp/policy-helpers.js
//
// Covers:
//  - normalizeToolName (mcp__server__tool prefix stripping + non-string input)
//  - applyPolicyTransform (no-op cases, replace, deep-shallow merge,
//    audit-entry shape, mutation semantics)

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  applyPolicyTransform,
  normalizeToolName,
} from '../../src/mcp/policy-helpers.js';

describe('normalizeToolName', () => {
  it('returns "" for non-string / null / undefined / empty', () => {
    assert.equal(normalizeToolName(undefined), '');
    assert.equal(normalizeToolName(null), '');
    assert.equal(normalizeToolName(''), '');
    assert.equal(normalizeToolName(42), '');
    assert.equal(normalizeToolName({}), '');
  });

  it('strips a single mcp__<server>__ prefix', () => {
    assert.equal(
      normalizeToolName('mcp__stateset-commerce__create_order'),
      'create_order',
    );
    assert.equal(
      normalizeToolName('mcp__a__list_customers'),
      'list_customers',
    );
  });

  it('does not strip a non-mcp prefix', () => {
    assert.equal(
      normalizeToolName('not_mcp_prefix__create_order'),
      'not_mcp_prefix__create_order',
    );
    assert.equal(normalizeToolName('create_order'), 'create_order');
  });

  it('greedily strips the longest mcp__...__ leading segment', () => {
    // The regex [a-z0-9_-]+ accepts underscores inside the server segment,
    // so a chained `mcp__a__mcp__b__tool` collapses entirely. This is
    // intentional — server names can contain underscores, so we trust the
    // last `__` boundary as the prefix terminator.
    assert.equal(
      normalizeToolName('mcp__server-1__mcp__nested__create_order'),
      'create_order',
    );
  });

  it('trims surrounding whitespace before stripping', () => {
    assert.equal(
      normalizeToolName('  mcp__stateset__create_order  '),
      'create_order',
    );
  });

  it('accepts hyphens and digits in the server segment', () => {
    assert.equal(
      normalizeToolName('mcp__stateset-commerce-2__create_order'),
      'create_order',
    );
    assert.equal(
      normalizeToolName('mcp__a1b2c3__do_thing'),
      'do_thing',
    );
  });
});

describe('applyPolicyTransform', () => {
  it('returns input unchanged when transform is null / undefined / array / non-object', () => {
    const input = { a: 1 };
    assert.deepEqual(applyPolicyTransform(input, null), {
      output: input,
      auditEntries: [],
    });
    assert.deepEqual(applyPolicyTransform(input, undefined), {
      output: input,
      auditEntries: [],
    });
    assert.deepEqual(applyPolicyTransform(input, []), {
      output: input,
      auditEntries: [],
    });
    assert.deepEqual(applyPolicyTransform(input, 'string'), {
      output: input,
      auditEntries: [],
    });
  });

  it('returns a new object — does not mutate input', () => {
    const input = { a: 1 };
    const { output } = applyPolicyTransform(input, { b: 2 });
    assert.notEqual(output, input);
    assert.deepEqual(input, { a: 1 });
    assert.deepEqual(output, { a: 1, b: 2 });
  });

  it('replaces scalar fields and records before/after in audit entries', () => {
    const { output, auditEntries } = applyPolicyTransform(
      { customerId: 'old', amount: 100 },
      { customerId: 'new' },
    );
    assert.deepEqual(output, { customerId: 'new', amount: 100 });
    assert.equal(auditEntries.length, 1);
    assert.equal(auditEntries[0].field, 'customerId');
    assert.equal(auditEntries[0].before, 'old');
    assert.equal(auditEntries[0].after, 'new');
    assert.equal(typeof auditEntries[0].timestamp, 'string');
    // ISO timestamp.
    assert.match(auditEntries[0].timestamp, /^\d{4}-\d{2}-\d{2}T/);
  });

  it('deep-merges (shallow) when both existing and incoming are non-array objects', () => {
    const { output } = applyPolicyTransform(
      { metadata: { a: 1, b: 2 } },
      { metadata: { b: 3, c: 4 } },
    );
    // shallow merge: existing keys preserved, transform overrides on conflict
    assert.deepEqual(output, { metadata: { a: 1, b: 3, c: 4 } });
  });

  it('replaces (does not merge) when existing field is null/undefined', () => {
    const { output: out1 } = applyPolicyTransform(
      { metadata: null },
      { metadata: { a: 1 } },
    );
    assert.deepEqual(out1, { metadata: { a: 1 } });

    const { output: out2 } = applyPolicyTransform(
      {},
      { metadata: { a: 1 } },
    );
    assert.deepEqual(out2, { metadata: { a: 1 } });
  });

  it('replaces (does not merge) when either side is an array', () => {
    const { output: out1 } = applyPolicyTransform(
      { tags: ['old'] },
      { tags: ['new'] },
    );
    assert.deepEqual(out1, { tags: ['new'] });

    const { output: out2 } = applyPolicyTransform(
      { tags: { a: 1 } },
      { tags: ['from-array'] },
    );
    assert.deepEqual(out2, { tags: ['from-array'] });
  });

  it('handles undefined input as if it were {}', () => {
    const { output, auditEntries } = applyPolicyTransform(undefined, {
      injected: 'value',
    });
    assert.deepEqual(output, { injected: 'value' });
    assert.equal(auditEntries.length, 1);
    assert.equal(auditEntries[0].before, undefined);
    assert.equal(auditEntries[0].after, 'value');
  });

  it('appends to a caller-provided audit array (mutation)', () => {
    const audit = [{ field: 'pre-existing', before: 0, after: 0, timestamp: 'x' }];
    const { auditEntries } = applyPolicyTransform({}, { a: 1 }, audit);
    // Same array — mutated in place.
    assert.equal(auditEntries, audit);
    assert.equal(audit.length, 2);
    assert.equal(audit[1].field, 'a');
  });

  it('records one audit entry per transform key, in iteration order', () => {
    const { auditEntries } = applyPolicyTransform(
      { a: 1, b: 2 },
      { a: 'A', b: 'B', c: 'C' },
    );
    assert.equal(auditEntries.length, 3);
    assert.deepEqual(
      auditEntries.map((e) => e.field),
      ['a', 'b', 'c'],
    );
  });
});

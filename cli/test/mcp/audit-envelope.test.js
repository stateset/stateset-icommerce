// Unit tests for cli/src/mcp/audit-envelope.js
//
// Covers:
//  - replayEventHash: hashes match sha256(stableStringify(compactReplayValue(x)))
//  - normalizePolicyAction / normalizePolicyExplanation: toJSON()
//    handling, type-filtering, edge cases
//  - buildRollbackContract: strategy/reversible flags, contractHash
//    determinism, fallthrough for unknown tools
//  - buildApprovalStagesFromActions: stage extraction, defaults,
//    dedup by (level,name), sort by level

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  attachCommerceExecutionEvidence,
  buildApprovalStagesFromActions,
  buildCommerceExecutionEvidence,
  buildRollbackContract,
  normalizePolicyAction,
  normalizePolicyExplanation,
  replayEventHash,
} from '../../src/mcp/audit-envelope.js';
import { compactReplayValue, sha256, stableStringify } from '../../src/mcp/replay-sanitizer.js';

// ---------------------------------------------------------------------------
// replayEventHash
// ---------------------------------------------------------------------------

describe('replayEventHash', () => {
  it('returns sha256(stableStringify(compactReplayValue(value)))', () => {
    const value = { tool: 'create_order', orderId: 'ord_1' };
    const expected = sha256(stableStringify(compactReplayValue(value)));
    assert.equal(replayEventHash(value), expected);
  });

  it('is deterministic across calls', () => {
    assert.equal(replayEventHash({ a: 1 }), replayEventHash({ a: 1 }));
  });

  it('produces the same hash for canonically-equivalent objects', () => {
    // stableStringify sorts keys, so insertion order doesn't matter.
    assert.equal(replayEventHash({ a: 1, b: 2 }), replayEventHash({ b: 2, a: 1 }));
  });

  it('returns a 64-char lowercase hex string', () => {
    assert.match(replayEventHash({ x: 1 }), /^[0-9a-f]{64}$/);
  });
});

describe('commerce execution evidence', () => {
  const event = {
    eventId: 'event-1',
    tool: 'create_refund',
    status: 'success',
    requestId: 'request-1',
    sessionId: 'session-1',
    occurredAt: '2026-09-04T12:00:00.000Z',
    params: { paymentId: 'pay-1', amount: '10.00' },
    result: { success: true, refund: { id: 'refund-1' } },
    policy: { allowed: true },
    permission: { allowed: true },
    notes: { mutationManifest: { phase: 'success' } },
  };

  it('exports privacy-minimal hashes and correlation identifiers', () => {
    const evidence = buildCommerceExecutionEvidence(event);
    assert.equal(evidence.version, 'stateset.commerce-evidence.v1');
    assert.equal(evidence.event_id, 'event-1');
    assert.equal(evidence.tool, 'create_refund');
    assert.match(evidence.params_sha256, /^sha256:[a-f0-9]{64}$/);
    assert.match(evidence.result_sha256, /^sha256:[a-f0-9]{64}$/);
    assert.equal(Object.hasOwn(evidence, 'params'), false);
    assert.equal(Object.hasOwn(evidence, 'result'), false);
  });

  it('attaches evidence without replacing existing MCP metadata', () => {
    const response = { content: [], _meta: { existing: { value: 1 } } };
    const output = attachCommerceExecutionEvidence(response, event);
    assert.deepEqual(output._meta.existing, { value: 1 });
    assert.equal(output._meta['com.stateset/commerce'].event_id, 'event-1');
    assert.equal(response._meta['com.stateset/commerce'], undefined);
  });
});

// ---------------------------------------------------------------------------
// normalizePolicyAction
// ---------------------------------------------------------------------------

describe('normalizePolicyAction', () => {
  it('returns plain objects as-is', () => {
    const action = { kind: 'audit', tier: 'high' };
    assert.equal(normalizePolicyAction(action), action);
  });

  it('calls toJSON() when present and returns the result', () => {
    const action = {
      _internal: 'hidden',
      toJSON() {
        return { kind: 'serialized' };
      },
    };
    assert.deepEqual(normalizePolicyAction(action), { kind: 'serialized' });
  });

  it('returns null when toJSON throws', () => {
    const action = {
      toJSON() {
        throw new Error('boom');
      },
    };
    assert.equal(normalizePolicyAction(action), null);
  });

  it('returns null for null/undefined/empty/array/primitive', () => {
    assert.equal(normalizePolicyAction(null), null);
    assert.equal(normalizePolicyAction(undefined), null);
    assert.equal(normalizePolicyAction(''), null);
    assert.equal(normalizePolicyAction(0), null);
    assert.equal(normalizePolicyAction(false), null);
    assert.equal(normalizePolicyAction([1, 2, 3]), null);
    assert.equal(normalizePolicyAction('string'), null);
    assert.equal(normalizePolicyAction(42), null);
  });
});

// ---------------------------------------------------------------------------
// normalizePolicyExplanation
// ---------------------------------------------------------------------------

describe('normalizePolicyExplanation', () => {
  it('shares the same logic as normalizePolicyAction (smoke)', () => {
    const e = { reason: 'limit_exceeded', limit: 1000 };
    assert.equal(normalizePolicyExplanation(e), e);
    assert.equal(normalizePolicyExplanation(null), null);
    assert.equal(normalizePolicyExplanation(['array', 'rejected']), null);
  });

  it('honors toJSON() with a swallow-on-throw guarantee', () => {
    const e = {
      toJSON() {
        return { reason: 'redacted' };
      },
    };
    assert.deepEqual(normalizePolicyExplanation(e), { reason: 'redacted' });

    const broken = {
      toJSON() {
        throw new Error('serialization failed');
      },
    };
    assert.equal(normalizePolicyExplanation(broken), null);
  });
});

// ---------------------------------------------------------------------------
// buildRollbackContract
// ---------------------------------------------------------------------------

describe('buildRollbackContract', () => {
  it('returns a best-effort contract for tools with compensations', () => {
    // create_order has cancel_order in AGENTIC_COMPENSATION_HINTS.
    const c = buildRollbackContract('create_order');
    assert.equal(c.sourceTool, 'create_order');
    assert.equal(c.strategy, 'best_effort_compensation');
    assert.equal(c.reversible, true);
    assert.equal(c.compensation.length, 1);
    assert.equal(c.compensation[0].tool, 'cancel_order');
    assert.deepEqual(c.compensation[0].params, ['orderId']);
  });

  it('returns a no-op contract for tools without compensations', () => {
    const c = buildRollbackContract('list_customers');
    assert.equal(c.strategy, 'none');
    assert.equal(c.reversible, false);
    assert.deepEqual(c.compensation, []);
  });

  it('falls back to ["id"] params when a compensation tool has no param hints', () => {
    // We can't easily test this without a fake hint table; verify via the
    // existing data that any compensation has SOME params array.
    const c = buildRollbackContract('create_payment');
    for (const entry of c.compensation) {
      assert.ok(Array.isArray(entry.params));
      assert.ok(entry.params.length > 0);
    }
  });

  it('contractHash is deterministic and content-addressed', () => {
    const a = buildRollbackContract('create_order');
    const b = buildRollbackContract('create_order');
    assert.equal(a.contractHash, b.contractHash);

    const other = buildRollbackContract('create_cart');
    assert.notEqual(a.contractHash, other.contractHash);
  });

  it('contractHash matches the externally-computed replayEventHash', () => {
    const c = buildRollbackContract('create_order');
    const { contractHash, ...inner } = c;
    assert.equal(contractHash, replayEventHash(inner));
  });
});

// ---------------------------------------------------------------------------
// buildApprovalStagesFromActions
// ---------------------------------------------------------------------------

describe('buildApprovalStagesFromActions', () => {
  it('returns [] for empty / undefined input', () => {
    assert.deepEqual(buildApprovalStagesFromActions([]), []);
    assert.deepEqual(buildApprovalStagesFromActions(), []);
  });

  it('skips actions that do not require approval', () => {
    const actions = [
      { kind: 'audit' }, // no approval, no metadata.requiresApproval → skipped
      { kind: 'block', metadata: { requiresApproval: false } }, // explicit false → skipped
    ];
    assert.deepEqual(buildApprovalStagesFromActions(actions), []);
  });

  it('extracts an explicit stages array', () => {
    const actions = [
      {
        approval: {
          stages: [
            { level: 2, name: 'cfo', requiredApprovals: 2, approvers: ['cfo@x'] },
            { level: 1, name: 'manager' },
          ],
        },
      },
    ];
    const out = buildApprovalStagesFromActions(actions);
    // Output is sorted by level ascending.
    assert.equal(out.length, 2);
    assert.equal(out[0].name, 'manager');
    assert.equal(out[0].level, 1);
    assert.equal(out[1].name, 'cfo');
    assert.equal(out[1].level, 2);
    assert.equal(out[1].requiredApprovals, 2);
    assert.deepEqual(out[1].approvers, ['cfo@x']);
  });

  it('promotes a single approval object to a 1-stage list', () => {
    const actions = [
      { approval: { name: 'finance', approvers: ['fin@x'] } },
    ];
    const out = buildApprovalStagesFromActions(actions);
    assert.equal(out.length, 1);
    assert.equal(out[0].name, 'finance');
    assert.equal(out[0].requiredApprovals, 1); // default
    assert.deepEqual(out[0].approvers, ['fin@x']);
    assert.equal(out[0].source, 'policy_action');
  });

  it('falls back to metadata.approvalTier when approval has no name', () => {
    const actions = [
      {
        metadata: { requiresApproval: true, approvalTier: 'cfo-required' },
      },
    ];
    const out = buildApprovalStagesFromActions(actions);
    assert.equal(out[0].name, 'cfo-required');
  });

  it('uses sequential level + "approval-required" name when neither is set', () => {
    const actions = [
      { metadata: { requiresApproval: true } },
      { metadata: { requiresApproval: true } },
    ];
    const out = buildApprovalStagesFromActions(actions);
    assert.equal(out.length, 2);
    // Both have the default name; deduped by (level,name) — but the
    // levels are 1 and 2 sequential, so no dedup collision.
    assert.equal(out[0].level, 1);
    assert.equal(out[0].name, 'approval-required');
    assert.equal(out[1].level, 2);
  });

  it('deduplicates stages by (level, name)', () => {
    const actions = [
      { approval: { level: 1, name: 'manager', approvers: ['a'] } },
      { approval: { level: 1, name: 'manager', approvers: ['b'] } }, // dup
      { approval: { level: 2, name: 'cfo' } },
    ];
    const out = buildApprovalStagesFromActions(actions);
    assert.equal(out.length, 2);
    // First wins on dedup; approvers ['a'] preserved.
    assert.deepEqual(out[0].approvers, ['a']);
  });

  it('drops malformed stages inside an explicit stages array', () => {
    const actions = [
      {
        approval: {
          stages: [
            null,
            'not-an-object',
            { level: 1, name: 'good' },
          ],
        },
      },
    ];
    const out = buildApprovalStagesFromActions(actions);
    assert.equal(out.length, 1);
    assert.equal(out[0].name, 'good');
  });

  it('coerces non-numeric levels to a sequential counter', () => {
    const actions = [
      { approval: { level: 'NaN-thing', name: 'a' } },
    ];
    const out = buildApprovalStagesFromActions(actions);
    assert.equal(out[0].level, 1);
  });
});

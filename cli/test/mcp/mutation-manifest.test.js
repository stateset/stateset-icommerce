// Unit tests for cli/src/mcp/mutation-manifest.js
//
// Covers:
//  - extractIdempotencyKeyFromParams: every candidate key, ordering,
//    trimming, type-rejection, falsy-value handling
//  - buildDeterministicMutationManifest:
//    - read/unknown-permission/null-runtimeMeta → null
//    - core fields, hashes, deterministicSignature, rollback presence
//    - idempotency-key resolution (caller-provided, generated, null)
//    - same input → same deterministicSignature (content addressing)
//    - phase/policyDomain/compensations defaults

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  buildDeterministicMutationManifest,
  extractIdempotencyKeyFromParams,
} from '../../src/mcp/mutation-manifest.js';
import { replayEventHash } from '../../src/mcp/audit-envelope.js';

// ---------------------------------------------------------------------------
// extractIdempotencyKeyFromParams
// ---------------------------------------------------------------------------

describe('extractIdempotencyKeyFromParams', () => {
  it('returns null for non-object / array / missing input', () => {
    assert.equal(extractIdempotencyKeyFromParams(null), null);
    assert.equal(extractIdempotencyKeyFromParams(undefined), null);
    assert.equal(extractIdempotencyKeyFromParams('string'), null);
    assert.equal(extractIdempotencyKeyFromParams(42), null);
    assert.equal(extractIdempotencyKeyFromParams([1, 2, 3]), null);
  });

  it('returns null when no candidate key is present', () => {
    assert.equal(extractIdempotencyKeyFromParams({ unrelated: 'x' }), null);
    assert.equal(extractIdempotencyKeyFromParams({}), null);
  });

  it('finds idempotencyKey (camelCase) first in priority order', () => {
    const key = extractIdempotencyKeyFromParams({
      idempotencyKey: 'first',
      idempotency_key: 'second',
      requestId: 'third',
    });
    assert.equal(key, 'first');
  });

  it('falls through to idempotency_key when camelCase is missing', () => {
    const key = extractIdempotencyKeyFromParams({
      idempotency_key: 'snake-cased',
      requestId: 'ignored',
    });
    assert.equal(key, 'snake-cased');
  });

  it('accepts each candidate key name', () => {
    const candidates = [
      'idempotencyKey',
      'idempotency_key',
      'idempotencyToken',
      'requestId',
      'request_id',
      'externalId',
      'external_id',
    ];
    for (const key of candidates) {
      const result = extractIdempotencyKeyFromParams({ [key]: `val-${key}` });
      assert.equal(result, `val-${key}`, `expected key ${key} to be recognized`);
    }
  });

  it('trims whitespace from a recognized value', () => {
    assert.equal(
      extractIdempotencyKeyFromParams({ idempotencyKey: '  spaces  ' }),
      'spaces',
    );
  });

  it('rejects non-string values (number / object / boolean / null)', () => {
    assert.equal(extractIdempotencyKeyFromParams({ idempotencyKey: 42 }), null);
    assert.equal(extractIdempotencyKeyFromParams({ idempotencyKey: {} }), null);
    assert.equal(extractIdempotencyKeyFromParams({ idempotencyKey: true }), null);
    assert.equal(extractIdempotencyKeyFromParams({ idempotencyKey: null }), null);
  });

  it('rejects empty / whitespace-only string', () => {
    assert.equal(extractIdempotencyKeyFromParams({ idempotencyKey: '' }), null);
    assert.equal(extractIdempotencyKeyFromParams({ idempotencyKey: '   ' }), null);
    assert.equal(extractIdempotencyKeyFromParams({ idempotencyKey: '\t\n' }), null);
  });
});

// ---------------------------------------------------------------------------
// buildDeterministicMutationManifest
// ---------------------------------------------------------------------------

describe('buildDeterministicMutationManifest', () => {
  describe('null returns', () => {
    it('returns null when runtimeMeta is missing/null', () => {
      assert.equal(
        buildDeterministicMutationManifest({ toolName: 'create_order' }),
        null,
      );
      assert.equal(
        buildDeterministicMutationManifest({
          toolName: 'create_order',
          runtimeMeta: null,
        }),
        null,
      );
    });

    it('returns null when sideEffect === "read"', () => {
      assert.equal(
        buildDeterministicMutationManifest({
          toolName: 'list_orders',
          runtimeMeta: { sideEffect: 'read', permission: 'read' },
        }),
        null,
      );
    });

    it('returns null when permission === "unknown"', () => {
      assert.equal(
        buildDeterministicMutationManifest({
          toolName: 'mystery_tool',
          runtimeMeta: { sideEffect: 'write', permission: 'unknown' },
        }),
        null,
      );
    });

    it('returns null with no args at all', () => {
      assert.equal(buildDeterministicMutationManifest(), null);
    });
  });

  describe('happy path', () => {
    const fixture = {
      toolName: 'create_order',
      params: { customerId: 'cust_1', items: [{ sku: 'X' }] },
      policy: { domain: 'orders' },
      permission: { level: 'write' },
      runtimeMeta: {
        sideEffect: 'write',
        permission: 'write',
        policyDomain: 'orders',
        idempotent: true,
        compensations: ['cancel_order'],
      },
    };

    it('returns a manifest with all core fields populated', () => {
      const m = buildDeterministicMutationManifest(fixture);
      assert.ok(m);
      assert.equal(m.version, '1.0.0');
      assert.equal(m.tool, 'create_order');
      assert.equal(m.phase, 'execute'); // default
      assert.equal(m.sideEffect, 'write');
      assert.equal(m.policyDomain, 'orders');
      assert.equal(m.idempotent, true);
      assert.deepEqual(m.compensationTools, ['cancel_order']);
      assert.match(m.paramsHash, /^[0-9a-f]{64}$/);
      assert.match(m.policyHash, /^[0-9a-f]{64}$/);
      assert.match(m.permissionHash, /^[0-9a-f]{64}$/);
    });

    it('includes the rollback contract object', () => {
      const m = buildDeterministicMutationManifest(fixture);
      assert.ok(m.rollback);
      assert.equal(m.rollback.sourceTool, 'create_order');
      assert.equal(m.rollback.strategy, 'best_effort_compensation');
      assert.match(m.rollback.contractHash, /^[0-9a-f]{64}$/);
      assert.equal(m.rollbackContractHash, m.rollback.contractHash);
    });

    it('deterministicSignature matches replayEventHash of the core', () => {
      const m = buildDeterministicMutationManifest(fixture);
      // Reconstruct the core (everything except `deterministicSignature`
      // and `rollback`).
      const { deterministicSignature, rollback, ...core } = m;
      assert.equal(deterministicSignature, replayEventHash(core));
    });

    it('deterministicSignature is stable across calls (same input)', () => {
      const a = buildDeterministicMutationManifest(fixture);
      const b = buildDeterministicMutationManifest(fixture);
      assert.equal(a.deterministicSignature, b.deterministicSignature);
    });

    it('deterministicSignature differs when the tool name differs', () => {
      const a = buildDeterministicMutationManifest(fixture);
      const b = buildDeterministicMutationManifest({ ...fixture, toolName: 'create_cart' });
      assert.notEqual(a.deterministicSignature, b.deterministicSignature);
    });
  });

  describe('idempotency-key resolution', () => {
    const baseRuntime = {
      sideEffect: 'write',
      permission: 'write',
      idempotent: true,
    };

    it('uses caller-provided idempotency key when present', () => {
      const m = buildDeterministicMutationManifest({
        toolName: 'create_payment',
        params: { idempotencyKey: 'caller-key-1' },
        runtimeMeta: baseRuntime,
      });
      assert.equal(m.idempotencyKey, 'caller-key-1');
    });

    it('generates ik_<tool>_<paramsHash[:16]> when caller key missing AND idempotent', () => {
      const m = buildDeterministicMutationManifest({
        toolName: 'create_payment',
        params: { customerId: 'c1' },
        runtimeMeta: baseRuntime,
      });
      assert.ok(m.idempotencyKey);
      assert.match(m.idempotencyKey, /^ik_create_payment_[0-9a-f]{16}$/);
    });

    it('returns null idempotencyKey when neither caller-provided nor idempotent', () => {
      const m = buildDeterministicMutationManifest({
        toolName: 'ship_order',
        params: { orderId: 'ord_1' },
        runtimeMeta: { ...baseRuntime, idempotent: false },
      });
      assert.equal(m.idempotencyKey, null);
    });

    it('caller-provided key wins over auto-generation even when idempotent', () => {
      const m = buildDeterministicMutationManifest({
        toolName: 'create_payment',
        params: { idempotencyKey: 'explicit', customerId: 'c1' },
        runtimeMeta: baseRuntime,
      });
      assert.equal(m.idempotencyKey, 'explicit');
    });
  });

  describe('field defaults', () => {
    const minimalArgs = {
      toolName: 't',
      runtimeMeta: { sideEffect: 'write', permission: 'write' },
    };

    it('defaults phase to "execute"', () => {
      assert.equal(buildDeterministicMutationManifest(minimalArgs).phase, 'execute');
    });

    it('honors a custom phase', () => {
      const m = buildDeterministicMutationManifest({ ...minimalArgs, phase: 'preview' });
      assert.equal(m.phase, 'preview');
    });

    it('defaults policyDomain to null when runtimeMeta omits it', () => {
      assert.equal(
        buildDeterministicMutationManifest(minimalArgs).policyDomain,
        null,
      );
    });

    it('defaults compensationTools to [] when runtimeMeta omits it', () => {
      assert.deepEqual(
        buildDeterministicMutationManifest(minimalArgs).compensationTools,
        [],
      );
    });

    it('idempotent boolean coerces non-boolean truthy/falsy correctly', () => {
      const truthy = buildDeterministicMutationManifest({
        ...minimalArgs,
        runtimeMeta: { ...minimalArgs.runtimeMeta, idempotent: 'truthy' },
      });
      assert.equal(truthy.idempotent, true);

      const falsy = buildDeterministicMutationManifest({
        ...minimalArgs,
        runtimeMeta: { ...minimalArgs.runtimeMeta, idempotent: 0 },
      });
      assert.equal(falsy.idempotent, false);
    });
  });
});

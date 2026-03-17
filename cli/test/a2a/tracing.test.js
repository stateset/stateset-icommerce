/**
 * Tests for cli/src/a2a/tracing.js
 *
 * Covers: createTracingService — startSpan, child spans, inject/extract,
 * getTrace, Span lifecycle (end, addEvent, setAttribute, setStatus),
 * getMetrics (p50/p95/p99, error rate, throughput), ring buffer eviction,
 * withSpan convenience, exportOTLP, A2A-specific attributes, getRecentSpans.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import { createTracingService } from '../../src/a2a/tracing.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Tiny sleep for when we need measurable duration. */
function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Hex pattern matchers */
const HEX32 = /^[0-9a-f]{32}$/;
const HEX16 = /^[0-9a-f]{16}$/;

// ---------------------------------------------------------------------------
// 1. startSpan creates span with correct traceId/spanId
// ---------------------------------------------------------------------------

describe('Tracing — startSpan creates span with correct IDs', () => {
  let tracing;

  beforeEach(() => {
    tracing = createTracingService();
  });

  it('generates a 32-hex-char traceId', () => {
    const span = tracing.startSpan('test-op');
    assert.match(span.traceId, HEX32);
  });

  it('generates a 16-hex-char spanId', () => {
    const span = tracing.startSpan('test-op');
    assert.match(span.spanId, HEX16);
  });

  it('sets the span name', () => {
    const span = tracing.startSpan('my-operation');
    assert.equal(span.name, 'my-operation');
  });

  it('defaults kind to internal', () => {
    const span = tracing.startSpan('test-op');
    assert.equal(span.kind, 'internal');
  });

  it('respects explicit kind', () => {
    const span = tracing.startSpan('test-op', { kind: 'server' });
    assert.equal(span.kind, 'server');
  });

  it('sets startTimeMs to a recent timestamp', () => {
    const before = Date.now();
    const span = tracing.startSpan('test-op');
    const after = Date.now();
    assert.ok(span.startTimeMs >= before);
    assert.ok(span.startTimeMs <= after);
  });

  it('initial status is ok', () => {
    const span = tracing.startSpan('test-op');
    assert.equal(span.status, 'ok');
  });

  it('initial endTimeMs and durationMs are null', () => {
    const span = tracing.startSpan('test-op');
    assert.equal(span.endTimeMs, null);
    assert.equal(span.durationMs, null);
  });

  it('parentSpanId is null for root spans', () => {
    const span = tracing.startSpan('root');
    assert.equal(span.parentSpanId, null);
  });

  it('generates unique IDs for each span', () => {
    const s1 = tracing.startSpan('op-1');
    const s2 = tracing.startSpan('op-2');
    assert.notEqual(s1.spanId, s2.spanId);
  });
});

// ---------------------------------------------------------------------------
// 2. Child span inherits parent's traceId
// ---------------------------------------------------------------------------

describe('Tracing — child span inherits parent traceId', () => {
  let tracing;

  beforeEach(() => {
    tracing = createTracingService();
  });

  it('child span has same traceId as parent', () => {
    const parent = tracing.startSpan('parent-op');
    const child = tracing.startSpan('child-op', {
      traceId: parent.traceId,
      parentSpanId: parent.spanId,
    });
    assert.equal(child.traceId, parent.traceId);
    assert.equal(child.parentSpanId, parent.spanId);
  });

  it('child span has different spanId from parent', () => {
    const parent = tracing.startSpan('parent-op');
    const child = tracing.startSpan('child-op', {
      traceId: parent.traceId,
      parentSpanId: parent.spanId,
    });
    assert.notEqual(child.spanId, parent.spanId);
  });

  it('grandchild inherits root traceId', () => {
    const root = tracing.startSpan('root');
    const mid = tracing.startSpan('mid', {
      traceId: root.traceId,
      parentSpanId: root.spanId,
    });
    const leaf = tracing.startSpan('leaf', {
      traceId: root.traceId,
      parentSpanId: mid.spanId,
    });
    assert.equal(leaf.traceId, root.traceId);
    assert.equal(leaf.parentSpanId, mid.spanId);
  });

  it('child span created after extract() inherits extracted traceId', () => {
    const injectedTraceId = 'a'.repeat(32);
    const injectedSpanId = 'b'.repeat(16);
    tracing.extract({
      traceparent: `00-${injectedTraceId}-${injectedSpanId}-01`,
    });
    const child = tracing.startSpan('child-after-extract');
    assert.equal(child.traceId, injectedTraceId);
  });
});

// ---------------------------------------------------------------------------
// 3. inject() adds traceparent header in W3C format
// ---------------------------------------------------------------------------

describe('Tracing — inject() adds traceparent header', () => {
  let tracing;

  beforeEach(() => {
    tracing = createTracingService();
  });

  it('sets traceparent on headers object', () => {
    tracing.startSpan('op');
    const headers = {};
    tracing.inject(headers);
    assert.ok(headers.traceparent);
  });

  it('follows 00-traceId-spanId-flags format', () => {
    const span = tracing.startSpan('op');
    const headers = {};
    tracing.inject(headers);
    const parts = headers.traceparent.split('-');
    assert.equal(parts.length, 4);
    assert.equal(parts[0], '00');
    assert.equal(parts[1], span.traceId);
    assert.equal(parts[2], span.spanId);
    assert.equal(parts[3], '01');
  });

  it('returns the same headers object for chaining', () => {
    tracing.startSpan('op');
    const headers = { existing: 'value' };
    const result = tracing.inject(headers);
    assert.equal(result, headers);
    assert.equal(result.existing, 'value');
  });

  it('returns headers unchanged when no context exists', () => {
    // Fresh tracing service with no spans
    const freshTracing = createTracingService();
    const headers = { foo: 'bar' };
    freshTracing.inject(headers);
    assert.equal(headers.foo, 'bar');
    assert.equal(headers.traceparent, undefined);
  });

  it('traceparent reflects the most recent span', () => {
    const s1 = tracing.startSpan('first');
    const s2 = tracing.startSpan('second');
    const headers = {};
    tracing.inject(headers);
    assert.ok(headers.traceparent.includes(s2.spanId));
  });
});

// ---------------------------------------------------------------------------
// 4. extract() parses traceparent header correctly
// ---------------------------------------------------------------------------

describe('Tracing — extract() parses traceparent', () => {
  let tracing;

  beforeEach(() => {
    tracing = createTracingService();
  });

  it('returns SpanContext with correct fields', () => {
    const traceId = 'a1b2c3d4e5f6a7b8a1b2c3d4e5f6a7b8';
    const spanId = '1234567890abcdef';
    const ctx = tracing.extract({
      traceparent: `00-${traceId}-${spanId}-01`,
    });
    assert.deepEqual(ctx, { traceId, spanId, traceFlags: '01' });
  });

  it('returns null for missing traceparent', () => {
    assert.equal(tracing.extract({}), null);
  });

  it('returns null for empty string', () => {
    assert.equal(tracing.extract({ traceparent: '' }), null);
  });

  it('returns null for malformed traceparent (wrong parts count)', () => {
    assert.equal(tracing.extract({ traceparent: '00-abc-def' }), null);
  });

  it('returns null for wrong traceId length', () => {
    assert.equal(
      tracing.extract({ traceparent: '00-short-1234567890abcdef-01' }),
      null,
    );
  });

  it('returns null for wrong spanId length', () => {
    const traceId = 'a'.repeat(32);
    assert.equal(
      tracing.extract({ traceparent: `00-${traceId}-short-01` }),
      null,
    );
  });

  it('returns null for non-hex characters in traceId', () => {
    const bad = 'g'.repeat(32);
    assert.equal(
      tracing.extract({ traceparent: `00-${bad}-${'a'.repeat(16)}-01` }),
      null,
    );
  });

  it('returns null for null headers', () => {
    assert.equal(tracing.extract(null), null);
  });

  it('returns null for undefined headers', () => {
    assert.equal(tracing.extract(undefined), null);
  });
});

// ---------------------------------------------------------------------------
// 5. getTrace() returns all spans for a traceId
// ---------------------------------------------------------------------------

describe('Tracing — getTrace()', () => {
  let tracing;

  beforeEach(() => {
    tracing = createTracingService();
  });

  it('returns all spans belonging to a trace', () => {
    const root = tracing.startSpan('root');
    const rootTraceId = root.traceId;
    const child = tracing.startSpan('child', {
      traceId: rootTraceId,
      parentSpanId: root.spanId,
    });

    // Use a separate tracing service so current context doesn't leak
    const tracing2 = createTracingService();
    const unrelated = tracing2.startSpan('unrelated');

    // The main service should only have root + child
    const trace = tracing.getTrace(rootTraceId);
    assert.equal(trace.length, 2);
    assert.ok(trace.some((s) => s.spanId === root.spanId));
    assert.ok(trace.some((s) => s.spanId === child.spanId));
  });

  it('returns empty array for unknown traceId', () => {
    tracing.startSpan('op');
    const trace = tracing.getTrace('nonexistent');
    assert.equal(trace.length, 0);
  });

  it('returns JSON objects, not Span instances', () => {
    const span = tracing.startSpan('op');
    const trace = tracing.getTrace(span.traceId);
    assert.equal(typeof trace[0], 'object');
    assert.ok(!trace[0].end); // no method
    assert.ok('attributes' in trace[0]); // plain object
  });

  it('returns spans across multiple agents (same traceId)', () => {
    const sharedTraceId = 'c'.repeat(32);
    const s1 = tracing.startSpan('agent-a', { traceId: sharedTraceId });
    const s2 = tracing.startSpan('agent-b', {
      traceId: sharedTraceId,
      parentSpanId: s1.spanId,
    });
    const s3 = tracing.startSpan('agent-c', {
      traceId: sharedTraceId,
      parentSpanId: s2.spanId,
    });

    const trace = tracing.getTrace(sharedTraceId);
    assert.equal(trace.length, 3);
  });
});

// ---------------------------------------------------------------------------
// 6. Span.end() sets endTimeMs and durationMs
// ---------------------------------------------------------------------------

describe('Tracing — Span.end()', () => {
  let tracing;

  beforeEach(() => {
    tracing = createTracingService();
  });

  it('sets endTimeMs', () => {
    const span = tracing.startSpan('op');
    span.end();
    assert.ok(span.endTimeMs !== null);
    assert.ok(span.endTimeMs >= span.startTimeMs);
  });

  it('computes durationMs', () => {
    const span = tracing.startSpan('op');
    span.end();
    assert.equal(typeof span.durationMs, 'number');
    assert.ok(span.durationMs >= 0);
  });

  it('durationMs = endTimeMs - startTimeMs', () => {
    const span = tracing.startSpan('op');
    span.end();
    assert.equal(span.durationMs, span.endTimeMs - span.startTimeMs);
  });

  it('calling end() twice is a no-op', () => {
    const span = tracing.startSpan('op');
    span.end();
    const firstEnd = span.endTimeMs;
    span.end();
    assert.equal(span.endTimeMs, firstEnd);
  });

  it('measurable duration with sleep', async () => {
    const span = tracing.startSpan('slow-op');
    await sleep(20);
    span.end();
    assert.ok(span.durationMs >= 15, `durationMs was ${span.durationMs}`);
  });
});

// ---------------------------------------------------------------------------
// 7. Span.addEvent() adds timestamped event
// ---------------------------------------------------------------------------

describe('Tracing — Span.addEvent()', () => {
  let tracing;

  beforeEach(() => {
    tracing = createTracingService();
  });

  it('appends an event with name and timestamp', () => {
    const span = tracing.startSpan('op');
    span.addEvent('checkpoint-1');
    assert.equal(span.events.length, 1);
    assert.equal(span.events[0].name, 'checkpoint-1');
    assert.ok(span.events[0].timeMs > 0);
  });

  it('stores event attributes', () => {
    const span = tracing.startSpan('op');
    span.addEvent('escrow_locked', { escrowId: 'esc-42' });
    assert.equal(span.events[0].attributes.escrowId, 'esc-42');
  });

  it('defaults attributes to empty object', () => {
    const span = tracing.startSpan('op');
    span.addEvent('simple');
    assert.deepEqual(span.events[0].attributes, {});
  });

  it('supports multiple events', () => {
    const span = tracing.startSpan('op');
    span.addEvent('step-1');
    span.addEvent('step-2');
    span.addEvent('step-3');
    assert.equal(span.events.length, 3);
    assert.equal(span.events[2].name, 'step-3');
  });

  it('is a no-op after span is ended', () => {
    const span = tracing.startSpan('op');
    span.end();
    span.addEvent('too-late');
    assert.equal(span.events.length, 0);
  });
});

// ---------------------------------------------------------------------------
// 8. Span.setAttribute() stores key-value pairs
// ---------------------------------------------------------------------------

describe('Tracing — Span.setAttribute()', () => {
  let tracing;

  beforeEach(() => {
    tracing = createTracingService();
  });

  it('stores a string attribute', () => {
    const span = tracing.startSpan('op');
    span.setAttribute('orderId', 'ORD-42');
    assert.equal(span.attributes.get('orderId'), 'ORD-42');
  });

  it('stores a numeric attribute', () => {
    const span = tracing.startSpan('op');
    span.setAttribute('amount', 99.50);
    assert.equal(span.attributes.get('amount'), 99.50);
  });

  it('overwrites existing attribute', () => {
    const span = tracing.startSpan('op');
    span.setAttribute('status', 'pending');
    span.setAttribute('status', 'completed');
    assert.equal(span.attributes.get('status'), 'completed');
  });

  it('is a no-op after span is ended', () => {
    const span = tracing.startSpan('op');
    span.end();
    span.setAttribute('late', 'value');
    assert.equal(span.attributes.has('late'), false);
  });

  it('initial attributes set via startSpan are accessible', () => {
    const span = tracing.startSpan('op', {
      attributes: { foo: 'bar', num: 7 },
    });
    assert.equal(span.attributes.get('foo'), 'bar');
    assert.equal(span.attributes.get('num'), 7);
  });
});

// ---------------------------------------------------------------------------
// 9. Span.setStatus('error') marks span as error
// ---------------------------------------------------------------------------

describe('Tracing — Span.setStatus()', () => {
  let tracing;

  beforeEach(() => {
    tracing = createTracingService();
  });

  it('sets status to error', () => {
    const span = tracing.startSpan('op');
    span.setStatus('error');
    assert.equal(span.status, 'error');
  });

  it('sets status back to ok', () => {
    const span = tracing.startSpan('op');
    span.setStatus('error');
    span.setStatus('ok');
    assert.equal(span.status, 'ok');
  });

  it('ignores invalid status values', () => {
    const span = tracing.startSpan('op');
    span.setStatus('warning');
    assert.equal(span.status, 'ok'); // unchanged
  });

  it('is a no-op after span is ended', () => {
    const span = tracing.startSpan('op');
    span.end();
    span.setStatus('error');
    assert.equal(span.status, 'ok');
  });
});

// ---------------------------------------------------------------------------
// 10. getMetrics() computes p50/p95/p99 correctly
// ---------------------------------------------------------------------------

describe('Tracing — getMetrics() percentiles', () => {
  it('computes p50/p95/p99 from ended spans', () => {
    const tracing = createTracingService();

    // Create 100 spans with known durations (1..100 ms)
    for (let i = 1; i <= 100; i++) {
      const span = tracing.startSpan(`op-${i}`);
      // Manually set durations for deterministic test
      span.startTimeMs = 1000;
      span.endTimeMs = 1000 + i;
      span.durationMs = i;
      span._ended = true;
    }

    const metrics = tracing.getMetrics();
    assert.equal(metrics.p50, 50);
    assert.equal(metrics.p95, 95);
    assert.equal(metrics.p99, 99);
  });

  it('returns zeros when no completed spans exist', () => {
    const tracing = createTracingService();
    tracing.startSpan('incomplete'); // not ended
    const metrics = tracing.getMetrics();
    assert.equal(metrics.p50, 0);
    assert.equal(metrics.p95, 0);
    assert.equal(metrics.p99, 0);
    assert.equal(metrics.spanCount, 0);
  });

  it('handles single span', () => {
    const tracing = createTracingService();
    const span = tracing.startSpan('single');
    span.startTimeMs = 1000;
    span.endTimeMs = 1042;
    span.durationMs = 42;
    span._ended = true;

    const metrics = tracing.getMetrics();
    assert.equal(metrics.p50, 42);
    assert.equal(metrics.p95, 42);
    assert.equal(metrics.p99, 42);
    assert.equal(metrics.spanCount, 1);
  });
});

// ---------------------------------------------------------------------------
// 11. getMetrics() computes error rate
// ---------------------------------------------------------------------------

describe('Tracing — getMetrics() error rate', () => {
  it('computes error rate from completed spans', () => {
    const tracing = createTracingService();

    // 3 ok + 2 error = 40% error rate
    for (let i = 0; i < 5; i++) {
      const span = tracing.startSpan(`op-${i}`);
      if (i >= 3) span.setStatus('error');
      span.end();
    }

    const metrics = tracing.getMetrics();
    assert.ok(Math.abs(metrics.errorRate - 0.4) < 0.001);
  });

  it('error rate is 0 when all spans succeed', () => {
    const tracing = createTracingService();
    for (let i = 0; i < 3; i++) {
      const span = tracing.startSpan(`op-${i}`);
      span.end();
    }
    assert.equal(tracing.getMetrics().errorRate, 0);
  });

  it('error rate is 1 when all spans fail', () => {
    const tracing = createTracingService();
    for (let i = 0; i < 3; i++) {
      const span = tracing.startSpan(`op-${i}`);
      span.setStatus('error');
      span.end();
    }
    assert.equal(tracing.getMetrics().errorRate, 1);
  });
});

// ---------------------------------------------------------------------------
// 12. Ring buffer evicts oldest spans when full
// ---------------------------------------------------------------------------

describe('Tracing — ring buffer eviction', () => {
  it('evicts oldest spans when maxSpans exceeded', () => {
    const tracing = createTracingService({ maxSpans: 5 });

    const spans = [];
    for (let i = 0; i < 8; i++) {
      spans.push(tracing.startSpan(`op-${i}`));
    }

    const recent = tracing.getRecentSpans(100);
    assert.equal(recent.length, 5);

    // First 3 should have been evicted; last 5 remain
    const names = recent.map((s) => s.name);
    assert.deepEqual(names, ['op-3', 'op-4', 'op-5', 'op-6', 'op-7']);
  });

  it('getTrace still works after evictions', () => {
    const tracing = createTracingService({ maxSpans: 3 });

    // Use explicit different traceIds so they don't share via current context
    const t1Id = 'a'.repeat(32);
    const t1 = tracing.startSpan('first', { traceId: t1Id });
    tracing.startSpan('second', { traceId: 'b'.repeat(32) });
    tracing.startSpan('third', { traceId: 'c'.repeat(32) });
    tracing.startSpan('fourth', { traceId: 'd'.repeat(32) }); // evicts "first"

    const trace = tracing.getTrace(t1Id);
    assert.equal(trace.length, 0); // evicted
  });

  it('respects maxSpans=1', () => {
    const tracing = createTracingService({ maxSpans: 1 });
    tracing.startSpan('a');
    tracing.startSpan('b');
    tracing.startSpan('c');

    const recent = tracing.getRecentSpans(100);
    assert.equal(recent.length, 1);
    assert.equal(recent[0].name, 'c');
  });
});

// ---------------------------------------------------------------------------
// 13. withSpan() convenience runs fn and ends span
// ---------------------------------------------------------------------------

describe('Tracing — withSpan() success path', () => {
  let tracing;

  beforeEach(() => {
    tracing = createTracingService();
  });

  it('runs the function and returns its result', async () => {
    const result = await tracing.withSpan('work', () => 42);
    assert.equal(result, 42);
  });

  it('ends the span after completion', async () => {
    let capturedSpan;
    await tracing.withSpan('work', (span) => {
      capturedSpan = span;
    });
    assert.ok(capturedSpan.endTimeMs !== null);
    assert.ok(capturedSpan._ended);
  });

  it('span status is ok on success', async () => {
    let capturedSpan;
    await tracing.withSpan('work', (span) => {
      capturedSpan = span;
    });
    assert.equal(capturedSpan.status, 'ok');
  });

  it('handles async functions', async () => {
    const result = await tracing.withSpan('async-work', async () => {
      await sleep(5);
      return 'done';
    });
    assert.equal(result, 'done');
  });

  it('provides span to the callback for attribute setting', async () => {
    let captured;
    await tracing.withSpan('work', (span) => {
      span.setAttribute('key', 'value');
      captured = span;
    });
    assert.equal(captured.attributes.get('key'), 'value');
  });
});

// ---------------------------------------------------------------------------
// 14. withSpan() marks error on throw and re-throws
// ---------------------------------------------------------------------------

describe('Tracing — withSpan() error path', () => {
  let tracing;

  beforeEach(() => {
    tracing = createTracingService();
  });

  it('re-throws the error', async () => {
    await assert.rejects(
      () => tracing.withSpan('fail', () => { throw new Error('boom'); }),
      { message: 'boom' },
    );
  });

  it('marks span as error on throw', async () => {
    let capturedSpan;
    try {
      await tracing.withSpan('fail', (span) => {
        capturedSpan = span;
        throw new Error('boom');
      });
    } catch { /* expected */ }
    assert.equal(capturedSpan.status, 'error');
  });

  it('sets error.message attribute on throw', async () => {
    let capturedSpan;
    try {
      await tracing.withSpan('fail', (span) => {
        capturedSpan = span;
        throw new Error('something broke');
      });
    } catch { /* expected */ }
    assert.equal(capturedSpan.attributes.get('error.message'), 'something broke');
  });

  it('still ends the span on error', async () => {
    let capturedSpan;
    try {
      await tracing.withSpan('fail', (span) => {
        capturedSpan = span;
        throw new Error('boom');
      });
    } catch { /* expected */ }
    assert.ok(capturedSpan._ended);
    assert.ok(capturedSpan.endTimeMs !== null);
  });

  it('handles async errors', async () => {
    await assert.rejects(
      () => tracing.withSpan('async-fail', async () => {
        await sleep(1);
        throw new Error('async boom');
      }),
      { message: 'async boom' },
    );
  });
});

// ---------------------------------------------------------------------------
// 15. exportOTLP() returns valid OTLP structure
// ---------------------------------------------------------------------------

describe('Tracing — exportOTLP()', () => {
  let tracing;

  beforeEach(() => {
    tracing = createTracingService({ serviceName: 'test-service' });
  });

  it('returns resourceSpans array', () => {
    tracing.startSpan('op').end();
    const otlp = tracing.exportOTLP();
    assert.ok(Array.isArray(otlp.resourceSpans));
    assert.equal(otlp.resourceSpans.length, 1);
  });

  it('includes resource with service.name', () => {
    tracing.startSpan('op').end();
    const otlp = tracing.exportOTLP();
    const resource = otlp.resourceSpans[0].resource;
    assert.ok(Array.isArray(resource.attributes));
    const svcAttr = resource.attributes.find((a) => a.key === 'service.name');
    assert.ok(svcAttr);
    assert.equal(svcAttr.value.stringValue, 'test-service');
  });

  it('includes scopeSpans with scope info', () => {
    tracing.startSpan('op').end();
    const otlp = tracing.exportOTLP();
    const scopeSpans = otlp.resourceSpans[0].scopeSpans;
    assert.equal(scopeSpans.length, 1);
    assert.equal(scopeSpans[0].scope.name, 'a2a-tracing');
    assert.equal(scopeSpans[0].scope.version, '1.0.0');
  });

  it('maps spans with traceId, spanId, name', () => {
    const span = tracing.startSpan('my-op');
    span.end();
    const otlp = tracing.exportOTLP();
    const otlpSpan = otlp.resourceSpans[0].scopeSpans[0].spans[0];
    assert.equal(otlpSpan.traceId, span.traceId);
    assert.equal(otlpSpan.spanId, span.spanId);
    assert.equal(otlpSpan.name, 'my-op');
  });

  it('converts startTimeMs to nanosecond string', () => {
    const span = tracing.startSpan('op');
    span.end();
    const otlp = tracing.exportOTLP();
    const otlpSpan = otlp.resourceSpans[0].scopeSpans[0].spans[0];
    const expected = String(span.startTimeMs * 1_000_000);
    assert.equal(otlpSpan.startTimeUnixNano, expected);
  });

  it('maps kind to OTLP enum', () => {
    const span = tracing.startSpan('op', { kind: 'server' });
    span.end();
    const otlp = tracing.exportOTLP();
    const otlpSpan = otlp.resourceSpans[0].scopeSpans[0].spans[0];
    assert.equal(otlpSpan.kind, 2); // SPAN_KIND_SERVER
  });

  it('maps client kind correctly', () => {
    const span = tracing.startSpan('op', { kind: 'client' });
    span.end();
    const otlp = tracing.exportOTLP();
    const otlpSpan = otlp.resourceSpans[0].scopeSpans[0].spans[0];
    assert.equal(otlpSpan.kind, 3); // SPAN_KIND_CLIENT
  });

  it('maps internal kind correctly', () => {
    const span = tracing.startSpan('op', { kind: 'internal' });
    span.end();
    const otlp = tracing.exportOTLP();
    const otlpSpan = otlp.resourceSpans[0].scopeSpans[0].spans[0];
    assert.equal(otlpSpan.kind, 1); // SPAN_KIND_INTERNAL
  });

  it('maps error status to code 2', () => {
    const span = tracing.startSpan('op');
    span.setStatus('error');
    span.end();
    const otlp = tracing.exportOTLP();
    const otlpSpan = otlp.resourceSpans[0].scopeSpans[0].spans[0];
    assert.equal(otlpSpan.status.code, 2);
    assert.equal(otlpSpan.status.message, 'ERROR');
  });

  it('maps ok status to code 1', () => {
    const span = tracing.startSpan('op');
    span.end();
    const otlp = tracing.exportOTLP();
    const otlpSpan = otlp.resourceSpans[0].scopeSpans[0].spans[0];
    assert.equal(otlpSpan.status.code, 1);
    assert.equal(otlpSpan.status.message, '');
  });

  it('converts attributes to OTLP KeyValue format', () => {
    const span = tracing.startSpan('op', {
      attributes: { agent: 'seller', amount: 100 },
    });
    span.end();
    const otlp = tracing.exportOTLP();
    const otlpSpan = otlp.resourceSpans[0].scopeSpans[0].spans[0];
    const agentAttr = otlpSpan.attributes.find((a) => a.key === 'agent');
    assert.ok(agentAttr);
    assert.equal(agentAttr.value.stringValue, 'seller');
    const amountAttr = otlpSpan.attributes.find((a) => a.key === 'amount');
    assert.ok(amountAttr);
    assert.equal(amountAttr.value.intValue, '100');
  });

  it('converts events to OTLP format', () => {
    const span = tracing.startSpan('op');
    span.addEvent('checkpoint', { step: 'done' });
    span.end();
    const otlp = tracing.exportOTLP();
    const otlpSpan = otlp.resourceSpans[0].scopeSpans[0].spans[0];
    assert.equal(otlpSpan.events.length, 1);
    assert.equal(otlpSpan.events[0].name, 'checkpoint');
    const stepAttr = otlpSpan.events[0].attributes.find((a) => a.key === 'step');
    assert.ok(stepAttr);
    assert.equal(stepAttr.value.stringValue, 'done');
  });

  it('parentSpanId is empty string for root spans', () => {
    const span = tracing.startSpan('root');
    span.end();
    const otlp = tracing.exportOTLP();
    const otlpSpan = otlp.resourceSpans[0].scopeSpans[0].spans[0];
    assert.equal(otlpSpan.parentSpanId, '');
  });

  it('exports multiple spans', () => {
    tracing.startSpan('a').end();
    tracing.startSpan('b').end();
    tracing.startSpan('c').end();
    const otlp = tracing.exportOTLP();
    assert.equal(otlp.resourceSpans[0].scopeSpans[0].spans.length, 3);
  });

  it('handles boolean attributes in OTLP', () => {
    const span = tracing.startSpan('op');
    span.setAttribute('verified', true);
    span.end();
    const otlp = tracing.exportOTLP();
    const otlpSpan = otlp.resourceSpans[0].scopeSpans[0].spans[0];
    const attr = otlpSpan.attributes.find((a) => a.key === 'verified');
    assert.ok(attr);
    assert.equal(attr.value.boolValue, true);
  });

  it('handles float attributes in OTLP', () => {
    const span = tracing.startSpan('op');
    span.setAttribute('price', 19.99);
    span.end();
    const otlp = tracing.exportOTLP();
    const otlpSpan = otlp.resourceSpans[0].scopeSpans[0].spans[0];
    const attr = otlpSpan.attributes.find((a) => a.key === 'price');
    assert.ok(attr);
    assert.equal(attr.value.doubleValue, 19.99);
  });
});

// ---------------------------------------------------------------------------
// 16. A2A attributes stored correctly
// ---------------------------------------------------------------------------

describe('Tracing — A2A-specific attributes', () => {
  let tracing;

  beforeEach(() => {
    tracing = createTracingService();
  });

  it('stores agentAddress', () => {
    const span = tracing.startSpan('a2a_pay', {
      attributes: { agentAddress: '0xSellerAgent' },
    });
    assert.equal(span.attributes.get('agentAddress'), '0xSellerAgent');
  });

  it('stores counterpartyAddress', () => {
    const span = tracing.startSpan('a2a_pay', {
      attributes: { counterpartyAddress: '0xBuyerAgent' },
    });
    assert.equal(span.attributes.get('counterpartyAddress'), '0xBuyerAgent');
  });

  it('stores operationType', () => {
    const span = tracing.startSpan('a2a_pay');
    span.setAttribute('operationType', 'escrow_release');
    assert.equal(span.attributes.get('operationType'), 'escrow_release');
  });

  it('stores amount and asset', () => {
    const span = tracing.startSpan('a2a_pay', {
      attributes: { amount: 250, asset: 'USDC' },
    });
    assert.equal(span.attributes.get('amount'), 250);
    assert.equal(span.attributes.get('asset'), 'USDC');
  });

  it('stores sagaId', () => {
    const span = tracing.startSpan('saga_step', {
      attributes: { sagaId: 'saga-abc-123' },
    });
    assert.equal(span.attributes.get('sagaId'), 'saga-abc-123');
  });

  it('stores escrowId', () => {
    const span = tracing.startSpan('escrow_lock', {
      attributes: { escrowId: 'esc-42' },
    });
    assert.equal(span.attributes.get('escrowId'), 'esc-42');
  });

  it('stores quoteId', () => {
    const span = tracing.startSpan('quote_accept', {
      attributes: { quoteId: 'q-789' },
    });
    assert.equal(span.attributes.get('quoteId'), 'q-789');
  });

  it('all A2A attributes survive toJSON()', () => {
    const span = tracing.startSpan('full-a2a-op', {
      attributes: {
        agentAddress: '0xA',
        counterpartyAddress: '0xB',
        operationType: 'payment',
        amount: 100,
        asset: 'ssUSD',
        sagaId: 'saga-1',
        escrowId: 'esc-1',
        quoteId: 'q-1',
      },
    });

    const json = span.toJSON();
    assert.equal(json.attributes.agentAddress, '0xA');
    assert.equal(json.attributes.counterpartyAddress, '0xB');
    assert.equal(json.attributes.operationType, 'payment');
    assert.equal(json.attributes.amount, 100);
    assert.equal(json.attributes.asset, 'ssUSD');
    assert.equal(json.attributes.sagaId, 'saga-1');
    assert.equal(json.attributes.escrowId, 'esc-1');
    assert.equal(json.attributes.quoteId, 'q-1');
  });

  it('A2A attributes export in OTLP format', () => {
    const span = tracing.startSpan('a2a-op', {
      attributes: { agentAddress: '0xSeller', amount: 500 },
    });
    span.end();
    const otlp = tracing.exportOTLP();
    const otlpSpan = otlp.resourceSpans[0].scopeSpans[0].spans[0];
    const agentAttr = otlpSpan.attributes.find((a) => a.key === 'agentAddress');
    assert.ok(agentAttr);
    assert.equal(agentAttr.value.stringValue, '0xSeller');
  });
});

// ---------------------------------------------------------------------------
// 17. getRecentSpans returns correct limit
// ---------------------------------------------------------------------------

describe('Tracing — getRecentSpans()', () => {
  let tracing;

  beforeEach(() => {
    tracing = createTracingService();
  });

  it('returns all spans when fewer than limit', () => {
    tracing.startSpan('a');
    tracing.startSpan('b');
    const recent = tracing.getRecentSpans(10);
    assert.equal(recent.length, 2);
  });

  it('returns exactly limit spans when more exist', () => {
    for (let i = 0; i < 10; i++) {
      tracing.startSpan(`op-${i}`);
    }
    const recent = tracing.getRecentSpans(3);
    assert.equal(recent.length, 3);
  });

  it('returns the most recent spans (newest last)', () => {
    for (let i = 0; i < 5; i++) {
      tracing.startSpan(`op-${i}`);
    }
    const recent = tracing.getRecentSpans(2);
    assert.equal(recent[0].name, 'op-3');
    assert.equal(recent[1].name, 'op-4');
  });

  it('defaults to 100 when no limit specified', () => {
    for (let i = 0; i < 150; i++) {
      tracing.startSpan(`op-${i}`);
    }
    const recent = tracing.getRecentSpans();
    assert.equal(recent.length, 100);
  });

  it('returns empty array when no spans exist', () => {
    const recent = tracing.getRecentSpans(5);
    assert.equal(recent.length, 0);
  });

  it('returns JSON objects, not Span instances', () => {
    tracing.startSpan('op');
    const recent = tracing.getRecentSpans(1);
    assert.equal(typeof recent[0], 'object');
    assert.ok(!recent[0].end); // no method
    assert.ok('traceId' in recent[0]);
    assert.ok('attributes' in recent[0]);
  });
});

// ---------------------------------------------------------------------------
// Additional edge cases
// ---------------------------------------------------------------------------

describe('Tracing — toJSON serialisation', () => {
  it('produces a clean plain object', () => {
    const tracing = createTracingService();
    const span = tracing.startSpan('op', {
      kind: 'client',
      attributes: { key: 'val' },
    });
    span.addEvent('evt', { data: 1 });
    span.end();

    const json = span.toJSON();
    assert.equal(json.name, 'op');
    assert.equal(json.kind, 'client');
    assert.equal(json.status, 'ok');
    assert.equal(typeof json.attributes, 'object');
    assert.ok(!json.attributes.get); // not a Map
    assert.equal(json.attributes.key, 'val');
    assert.equal(json.events.length, 1);
    assert.equal(json.events[0].name, 'evt');
    assert.ok(json.durationMs >= 0);
  });
});

describe('Tracing — getMetrics() throughput', () => {
  it('computes throughput as spans per second', () => {
    const tracing = createTracingService();

    // 10 spans spanning a 1-second window
    for (let i = 0; i < 10; i++) {
      const span = tracing.startSpan(`op-${i}`);
      span.startTimeMs = 1000 + i * 100; // 0ms..900ms
      span.endTimeMs = 1000 + i * 100 + 50;
      span.durationMs = 50;
      span._ended = true;
    }

    const metrics = tracing.getMetrics();
    // Window: earliest start = 1000, latest end = 1950, window = 0.95s
    // throughput = 10 / 0.95 ≈ 10.526
    assert.ok(metrics.throughput > 10);
    assert.ok(metrics.throughput < 12);
  });
});

describe('Tracing — context propagation round-trip', () => {
  it('inject → extract → child span preserves traceId', () => {
    const service1 = createTracingService({ serviceName: 'agent-a' });
    const service2 = createTracingService({ serviceName: 'agent-b' });

    // Agent A starts a span and injects context
    const spanA = service1.startSpan('request', { kind: 'client' });
    const headers = {};
    service1.inject(headers);

    // Agent B extracts context and creates child span
    const ctx = service2.extract(headers);
    assert.ok(ctx);
    const spanB = service2.startSpan('handle-request', {
      kind: 'server',
      traceId: ctx.traceId,
      parentSpanId: ctx.spanId,
    });

    assert.equal(spanB.traceId, spanA.traceId);
    assert.equal(spanB.parentSpanId, spanA.spanId);
  });
});

describe('Tracing — service name attribute', () => {
  it('adds service.name to every span', () => {
    const tracing = createTracingService({ serviceName: 'payment-agent' });
    const span = tracing.startSpan('op');
    assert.equal(span.attributes.get('service.name'), 'payment-agent');
  });

  it('defaults service.name to a2a-agent', () => {
    const tracing = createTracingService();
    const span = tracing.startSpan('op');
    assert.equal(span.attributes.get('service.name'), 'a2a-agent');
  });
});

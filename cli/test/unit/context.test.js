/**
 * Unit tests for context.js — RequestContext, Span, context management
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import {
  RequestContext,
  Span,
  runWithContext,
  getContext,
  getOrCreateContext,
  withContext,
  withChildContext,
  withSpan,
  ContextLogger,
  createContextLogger,
  createContextMiddleware,
} from '../../src/context.js';

// ===========================================================================
// RequestContext constructor & ID generation
// ===========================================================================

describe('RequestContext constructor', () => {
  it('generates unique requestId, traceId, spanId', () => {
    const ctx = new RequestContext();
    assert.ok(ctx.requestId.startsWith('req_'));
    assert.ok(ctx.traceId.length === 32); // 16 bytes hex
    assert.ok(ctx.spanId.length === 16); // 8 bytes hex
  });

  it('generates different IDs per instance', () => {
    const a = new RequestContext();
    const b = new RequestContext();
    assert.notStrictEqual(a.requestId, b.requestId);
    assert.notStrictEqual(a.traceId, b.traceId);
  });

  it('accepts custom options', () => {
    const ctx = new RequestContext({
      requestId: 'req_custom',
      traceId: 'trace_custom',
      agent: 'checkout',
      sessionId: 'sess-1',
      userId: 'user-1',
      dbPath: ':memory:',
      applyMode: true,
    });
    assert.strictEqual(ctx.requestId, 'req_custom');
    assert.strictEqual(ctx.traceId, 'trace_custom');
    assert.strictEqual(ctx.agent, 'checkout');
    assert.strictEqual(ctx.sessionId, 'sess-1');
    assert.strictEqual(ctx.userId, 'user-1');
    assert.strictEqual(ctx.dbPath, ':memory:');
    assert.strictEqual(ctx.applyMode, true);
  });

  it('defaults applyMode to false', () => {
    const ctx = new RequestContext();
    assert.strictEqual(ctx.applyMode, false);
  });
});

// ===========================================================================
// Tags and metadata
// ===========================================================================

describe('RequestContext tags and metadata', () => {
  it('addTag adds a single tag', () => {
    const ctx = new RequestContext();
    const result = ctx.addTag('urgent');
    assert.ok(ctx.tags.has('urgent'));
    assert.strictEqual(result, ctx); // chainable
  });

  it('addTags adds multiple tags', () => {
    const ctx = new RequestContext();
    ctx.addTags(['read', 'analytics']);
    assert.ok(ctx.tags.has('read'));
    assert.ok(ctx.tags.has('analytics'));
  });

  it('setMeta / getMeta round-trip', () => {
    const ctx = new RequestContext();
    ctx.setMeta('region', 'us-east');
    assert.strictEqual(ctx.getMeta('region'), 'us-east');
  });

  it('getMeta returns undefined for missing key', () => {
    const ctx = new RequestContext();
    assert.strictEqual(ctx.getMeta('nope'), undefined);
  });
});

// ===========================================================================
// Events and errors
// ===========================================================================

describe('RequestContext events and errors', () => {
  it('logEvent records an event', () => {
    const ctx = new RequestContext();
    const result = ctx.logEvent('request_start', { path: '/api' });
    assert.strictEqual(ctx.events.length, 1);
    assert.strictEqual(ctx.events[0].name, 'request_start');
    assert.deepStrictEqual(ctx.events[0].data, { path: '/api' });
    assert.ok(ctx.events[0].timestamp > 0);
    assert.strictEqual(result, ctx); // chainable
  });

  it('logError records an Error object', () => {
    const ctx = new RequestContext();
    const err = new Error('boom');
    ctx.logError(err, { op: 'test' });
    assert.strictEqual(ctx.errors.length, 1);
    assert.strictEqual(ctx.errors[0].error.name, 'Error');
    assert.strictEqual(ctx.errors[0].error.message, 'boom');
    assert.ok(ctx.errors[0].error.stack);
    assert.deepStrictEqual(ctx.errors[0].data, { op: 'test' });
  });

  it('logError records a string', () => {
    const ctx = new RequestContext();
    ctx.logError('something bad');
    assert.strictEqual(ctx.errors[0].error, 'something bad');
  });
});

// ===========================================================================
// elapsed
// ===========================================================================

describe('RequestContext.elapsed', () => {
  it('returns positive elapsed ms', async () => {
    const ctx = new RequestContext();
    await new Promise((r) => setTimeout(r, 10));
    assert.ok(ctx.elapsed >= 5); // allow some slack
  });
});

// ===========================================================================
// createChild
// ===========================================================================

describe('RequestContext.createChild', () => {
  it('inherits traceId', () => {
    const parent = new RequestContext({ traceId: 'parent-trace' });
    const child = parent.createChild();
    assert.strictEqual(child.traceId, 'parent-trace');
  });

  it('sets parentSpanId to parent spanId', () => {
    const parent = new RequestContext();
    const child = parent.createChild();
    assert.strictEqual(child.parentSpanId, parent.spanId);
  });

  it('gets its own spanId', () => {
    const parent = new RequestContext();
    const child = parent.createChild();
    assert.notStrictEqual(child.spanId, parent.spanId);
  });

  it('inherits sessionId, userId, dbPath, applyMode', () => {
    const parent = new RequestContext({
      sessionId: 'sess-1',
      userId: 'u1',
      dbPath: ':memory:',
      applyMode: true,
    });
    const child = parent.createChild();
    assert.strictEqual(child.sessionId, 'sess-1');
    assert.strictEqual(child.userId, 'u1');
    assert.strictEqual(child.dbPath, ':memory:');
    assert.strictEqual(child.applyMode, true);
  });

  it('inherits tags', () => {
    const parent = new RequestContext();
    parent.addTag('mytag');
    const child = parent.createChild();
    assert.ok(child.tags.has('mytag'));
  });

  it('allows overrides', () => {
    const parent = new RequestContext({ agent: 'orders' });
    const child = parent.createChild({ agent: 'checkout' });
    assert.strictEqual(child.agent, 'checkout');
  });
});

// ===========================================================================
// Serialization
// ===========================================================================

describe('RequestContext serialization', () => {
  it('serialize / deserialize round-trip', () => {
    const ctx = new RequestContext({
      agent: 'analytics',
      sessionId: 'sess-2',
      applyMode: true,
    });
    ctx.addTag('important');
    ctx.setMeta('foo', 'bar');
    ctx.logEvent('ev1');
    ctx.logError('e1');

    const json = ctx.serialize();
    const restored = RequestContext.deserialize(json);

    assert.strictEqual(restored.requestId, ctx.requestId);
    assert.strictEqual(restored.traceId, ctx.traceId);
    assert.strictEqual(restored.agent, 'analytics');
    assert.strictEqual(restored.sessionId, 'sess-2');
    assert.strictEqual(restored.applyMode, true);
    assert.strictEqual(restored.getMeta('foo'), 'bar');
    // events/errors are not restored by constructor (intentional — fresh start)
    assert.strictEqual(restored.events.length, 0);
    assert.strictEqual(restored.errors.length, 0);
  });

  it('deserialize from object', () => {
    const obj = { requestId: 'req_obj', traceId: 'tr', spanId: 'sp' };
    const ctx = RequestContext.deserialize(obj);
    assert.strictEqual(ctx.requestId, 'req_obj');
  });
});

// ===========================================================================
// toLogObject
// ===========================================================================

describe('RequestContext.toLogObject', () => {
  it('includes all expected fields', () => {
    const ctx = new RequestContext({ agent: 'test', applyMode: true });
    ctx.addTag('t1');
    ctx.logEvent('ev');
    ctx.logError('err');
    const log = ctx.toLogObject();

    assert.strictEqual(log.agent, 'test');
    assert.strictEqual(log.applyMode, true);
    assert.deepStrictEqual(log.tags, ['t1']);
    assert.strictEqual(log.eventCount, 1);
    assert.strictEqual(log.errorCount, 1);
    assert.ok(log.elapsed >= 0);
  });
});

// ===========================================================================
// getTraceHeaders
// ===========================================================================

describe('RequestContext.getTraceHeaders', () => {
  it('returns trace headers', () => {
    const ctx = new RequestContext({
      requestId: 'r1',
      traceId: 't1',
      spanId: 's1',
      parentSpanId: 'ps1',
    });
    const headers = ctx.getTraceHeaders();
    assert.strictEqual(headers['x-request-id'], 'r1');
    assert.strictEqual(headers['x-trace-id'], 't1');
    assert.strictEqual(headers['x-span-id'], 's1');
    assert.strictEqual(headers['x-parent-span-id'], 'ps1');
  });
});

// ===========================================================================
// Span
// ===========================================================================

describe('Span', () => {
  it('creates with name and context', () => {
    const ctx = new RequestContext();
    const span = new Span({ context: ctx, name: 'db-query' });
    assert.strictEqual(span.name, 'db-query');
    assert.strictEqual(span.context, ctx);
    assert.strictEqual(span.status, 'running');
    assert.strictEqual(span.endTime, null);
  });

  it('setAttribute / setAttributes', () => {
    const span = new Span({ name: 'test' });
    span.setAttribute('key', 'value');
    assert.strictEqual(span.attributes.key, 'value');

    span.setAttributes({ a: 1, b: 2 });
    assert.strictEqual(span.attributes.a, 1);
    assert.strictEqual(span.attributes.b, 2);
  });

  it('addEvent records event', () => {
    const span = new Span({ name: 'test' });
    span.addEvent('query_start', { sql: 'SELECT 1' });
    assert.strictEqual(span.events.length, 1);
    assert.strictEqual(span.events[0].name, 'query_start');
  });

  it('end sets status and endTime', () => {
    const span = new Span({ name: 'test' });
    span.end('ok');
    assert.strictEqual(span.status, 'ok');
    assert.ok(span.endTime > 0);
  });

  it('error sets error status and attributes', () => {
    const span = new Span({ name: 'test' });
    const err = new Error('fail');
    span.error(err);
    assert.strictEqual(span.status, 'error');
    assert.strictEqual(span.attributes.error, true);
    assert.strictEqual(span.attributes['error.message'], 'fail');
    assert.ok(span.attributes['error.stack']);
  });

  it('duration returns ms since start', () => {
    const span = new Span({ name: 'test' });
    assert.ok(span.duration >= 0);
    span.end();
    assert.ok(span.duration >= 0);
  });

  it('toLogObject returns structured data', () => {
    const span = new Span({ name: 'op', parentSpanId: 'parent' });
    span.setAttribute('x', 1);
    span.end('ok');
    const log = span.toLogObject();
    assert.strictEqual(log.name, 'op');
    assert.strictEqual(log.parentSpanId, 'parent');
    assert.strictEqual(log.status, 'ok');
    assert.ok(log.duration >= 0);
  });
});

// ===========================================================================
// createSpan from context
// ===========================================================================

describe('RequestContext.createSpan', () => {
  it('creates span with parentSpanId from context', () => {
    const ctx = new RequestContext();
    const span = ctx.createSpan('child-op');
    assert.strictEqual(span.parentSpanId, ctx.spanId);
    assert.strictEqual(span.name, 'child-op');
    assert.strictEqual(span.context, ctx);
  });
});

// ===========================================================================
// Context management functions
// ===========================================================================

describe('runWithContext / getContext', () => {
  it('provides context within callback', () => {
    const ctx = new RequestContext({ agent: 'test' });
    runWithContext(ctx, () => {
      const current = getContext();
      assert.strictEqual(current, ctx);
      assert.strictEqual(current.agent, 'test');
    });
  });

  it('getContext returns null outside of context', () => {
    const result = getContext();
    assert.strictEqual(result, null);
  });
});

describe('getOrCreateContext', () => {
  it('returns existing context when in context', () => {
    const ctx = new RequestContext({ agent: 'existing' });
    runWithContext(ctx, () => {
      const got = getOrCreateContext({ agent: 'fallback' });
      assert.strictEqual(got.agent, 'existing');
    });
  });

  it('creates new context when outside', () => {
    const got = getOrCreateContext({ agent: 'fallback' });
    assert.strictEqual(got.agent, 'fallback');
  });
});

describe('withContext', () => {
  it('creates context and passes to fn', async () => {
    const result = await withContext({ agent: 'analytics' }, (ctx) => {
      assert.strictEqual(ctx.agent, 'analytics');
      assert.strictEqual(getContext(), ctx);
      return 42;
    });
    assert.strictEqual(result, 42);
  });

  it('logs error and rethrows on failure', async () => {
    await assert.rejects(
      () =>
        withContext({}, () => {
          throw new Error('boom');
        }),
      { message: 'boom' },
    );
  });
});

describe('withChildContext', () => {
  it('creates child with parent traceId', async () => {
    const parent = new RequestContext({ traceId: 'parent-trace' });
    await runWithContext(parent, async () => {
      await withChildContext({ agent: 'child-agent' }, (child) => {
        assert.strictEqual(child.traceId, 'parent-trace');
        assert.strictEqual(child.parentSpanId, parent.spanId);
        assert.strictEqual(child.agent, 'child-agent');
      });
    });
  });

  it('creates fresh context if no parent', async () => {
    await withChildContext({ agent: 'solo' }, (ctx) => {
      assert.strictEqual(ctx.agent, 'solo');
      assert.strictEqual(ctx.parentSpanId, null);
    });
  });
});

describe('withSpan', () => {
  it('runs fn with span and ends ok', async () => {
    const ctx = new RequestContext();
    const result = await runWithContext(ctx, async () => {
      return withSpan('my-op', (span) => {
        assert.strictEqual(span.name, 'my-op');
        assert.strictEqual(span.parentSpanId, ctx.spanId);
        return 'done';
      });
    });
    assert.strictEqual(result, 'done');
  });

  it('ends span with error on throw', async () => {
    const ctx = new RequestContext();
    await assert.rejects(async () => {
      await runWithContext(ctx, () =>
        withSpan('fail-op', () => {
          throw new Error('fail');
        }),
      );
    });
  });

  it('calls fn with null when no context', async () => {
    const result = await withSpan('no-ctx', (span) => {
      assert.strictEqual(span, null);
      return 'ok';
    });
    assert.strictEqual(result, 'ok');
  });
});

// ===========================================================================
// ContextLogger
// ===========================================================================

describe('ContextLogger', () => {
  it('enriches log calls with context when present', () => {
    const logged = [];
    const base = { info: (msg, meta) => logged.push({ msg, meta }) };
    const logger = new ContextLogger(base);

    const ctx = new RequestContext({ agent: 'test-agent' });
    runWithContext(ctx, () => {
      logger.info('hello', { extra: true });
    });

    assert.strictEqual(logged.length, 1);
    assert.strictEqual(logged[0].msg, 'hello');
    assert.strictEqual(logged[0].meta.agent, 'test-agent');
    assert.strictEqual(logged[0].meta.extra, true);
  });

  it('passes meta as-is when no context', () => {
    const logged = [];
    const base = { warn: (msg, meta) => logged.push({ msg, meta }) };
    const logger = new ContextLogger(base);

    logger.warn('outside', { key: 'val' });
    assert.strictEqual(logged[0].meta.key, 'val');
    assert.strictEqual(logged[0].meta.agent, undefined);
  });

  it('supports all log levels', () => {
    const calls = [];
    const base = {};
    for (const lvl of ['error', 'warn', 'info', 'debug', 'trace']) {
      base[lvl] = (msg) => calls.push(lvl);
    }
    const logger = new ContextLogger(base);
    logger.error('e');
    logger.warn('w');
    logger.info('i');
    logger.debug('d');
    logger.trace('t');
    assert.deepStrictEqual(calls, ['error', 'warn', 'info', 'debug', 'trace']);
  });
});

describe('createContextLogger', () => {
  it('returns a ContextLogger', () => {
    const logger = createContextLogger({ info: () => {} });
    assert.ok(logger instanceof ContextLogger);
  });
});

// ===========================================================================
// createContextMiddleware
// ===========================================================================

describe('createContextMiddleware', () => {
  it('sets up context for the request handler', async () => {
    const middleware = createContextMiddleware();
    const request = { query: 'list orders', agent: 'orders', applyMode: true };

    const result = await middleware(request, async (req, ctx) => {
      assert.strictEqual(ctx.agent, 'orders');
      assert.strictEqual(ctx.applyMode, true);
      return { data: 'ok' };
    });

    assert.strictEqual(result.data, 'ok');
    assert.ok(result.context); // context appended
    assert.strictEqual(result.context.agent, 'orders');
  });

  it('logs error and rethrows on handler failure', async () => {
    const middleware = createContextMiddleware();
    await assert.rejects(
      () =>
        middleware({ query: 'fail' }, async () => {
          throw new Error('handler error');
        }),
      { message: 'handler error' },
    );
  });
});

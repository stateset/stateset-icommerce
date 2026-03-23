/**
 * A2A Distributed Tracing Service
 *
 * W3C Trace Context-compatible distributed tracing for multi-agent
 * commerce transactions across service boundaries. Generates trace/span IDs,
 * propagates context via `traceparent`/`tracestate` headers, computes
 * latency percentiles, and exports spans in OpenTelemetry-compatible JSON.
 *
 * @example
 * ```javascript
 * const tracing = createTracingService({ maxSpans: 5000 });
 *
 * // Manual span lifecycle
 * const span = tracing.startSpan('a2a_payment.create', {
 *   kind: 'server',
 *   attributes: { agentAddress: '0xSeller', amount: 100 },
 * });
 * span.addEvent('escrow_locked', { escrowId: 'esc-1' });
 * span.setStatus('ok');
 * span.end();
 *
 * // Convenience wrapper
 * const result = await tracing.withSpan('process_order', async (s) => {
 *   s.setAttribute('orderId', 'ORD-42');
 *   return doWork();
 * });
 *
 * // Context propagation
 * const outgoing = {};
 * tracing.inject(outgoing);
 * // outgoing['traceparent'] === '00-<traceId>-<spanId>-01'
 *
 * // Metrics
 * const { p50, p95, p99, errorRate, throughput } = tracing.getMetrics();
 * ```
 */

import { randomBytes } from 'node:crypto';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Default maximum number of spans retained in the ring buffer. */
const DEFAULT_MAX_SPANS = 10_000;

/** W3C Trace Context version byte. */
const TRACE_VERSION = '00';

/** Default trace flags (sampled). */
const DEFAULT_FLAGS = '01';

/** Valid span kinds. */
const VALID_KINDS = new Set(['client', 'server', 'internal']);

/** Valid span statuses. */
const VALID_STATUSES = new Set(['ok', 'error']);

// ---------------------------------------------------------------------------
// ID generation
// ---------------------------------------------------------------------------

/**
 * Generate a 32-hex-char trace ID (128-bit).
 * @returns {string}
 */
function generateTraceId() {
  return randomBytes(16).toString('hex');
}

/**
 * Generate a 16-hex-char span ID (64-bit).
 * @returns {string}
 */
function generateSpanId() {
  return randomBytes(8).toString('hex');
}

// ---------------------------------------------------------------------------
// Percentile helper
// ---------------------------------------------------------------------------

/**
 * Compute a percentile from a sorted array of numbers.
 * Uses the "nearest rank" method.
 *
 * @param {number[]} sorted - Pre-sorted ascending array
 * @param {number} p - Percentile (0–100)
 * @returns {number}
 */
function percentile(sorted, p) {
  if (sorted.length === 0) return 0;
  const idx = Math.max(0, Math.ceil((p / 100) * sorted.length) - 1);
  return sorted[idx];
}

// ---------------------------------------------------------------------------
// Span class
// ---------------------------------------------------------------------------

/**
 * @typedef {Object} SpanEvent
 * @property {string} name
 * @property {number} timeMs
 * @property {Record<string, unknown>} attributes
 */

/**
 * An individual trace span.
 *
 * Spans are created via `tracing.startSpan()` and finished with `span.end()`.
 * Once ended, mutation methods (`addEvent`, `setAttribute`, `setStatus`) are
 * no-ops to prevent accidental corruption.
 */
class Span {
  /**
   * @param {Object} opts
   * @param {string} opts.traceId
   * @param {string} opts.spanId
   * @param {string|null} opts.parentSpanId
   * @param {string} opts.name
   * @param {'client'|'server'|'internal'} opts.kind
   * @param {Record<string, unknown>} [opts.attributes]
   */
  constructor({ traceId, spanId, parentSpanId, name, kind, attributes }) {
    /** @type {string} 32-hex-char trace identifier */
    this.traceId = traceId;
    /** @type {string} 16-hex-char span identifier */
    this.spanId = spanId;
    /** @type {string|null} Parent span ID, or null for root spans */
    this.parentSpanId = parentSpanId;
    /** @type {string} Human-readable operation name */
    this.name = name;
    /** @type {'client'|'server'|'internal'} */
    this.kind = kind;
    /** @type {number} Unix epoch millis when the span was created */
    this.startTimeMs = Date.now();
    /** @type {number|null} Unix epoch millis when the span was ended */
    this.endTimeMs = null;
    /** @type {number|null} Duration in milliseconds */
    this.durationMs = null;
    /** @type {'ok'|'error'} */
    this.status = 'ok';
    /** @type {Map<string, unknown>} */
    this.attributes = new Map(Object.entries(attributes ?? {}));
    /** @type {SpanEvent[]} */
    this.events = [];
    /** @type {boolean} Whether end() has been called */
    this._ended = false;
  }

  /**
   * Mark the span as finished.
   * Sets `endTimeMs` and computes `durationMs`.
   * Calling `end()` more than once is a no-op.
   */
  end() {
    if (this._ended) return;
    this._ended = true;
    this.endTimeMs = Date.now();
    this.durationMs = this.endTimeMs - this.startTimeMs;
  }

  /**
   * Append a timestamped event to the span.
   *
   * @param {string} name - Event name
   * @param {Record<string, unknown>} [attrs] - Optional event attributes
   */
  addEvent(name, attrs) {
    if (this._ended) return;
    this.events.push({
      name,
      timeMs: Date.now(),
      attributes: attrs ?? {},
    });
  }

  /**
   * Set a single attribute on the span.
   *
   * @param {string} key
   * @param {unknown} value
   */
  setAttribute(key, value) {
    if (this._ended) return;
    this.attributes.set(key, value);
  }

  /**
   * Set the span status.
   *
   * @param {'ok'|'error'} status
   */
  setStatus(status) {
    if (this._ended) return;
    if (VALID_STATUSES.has(status)) {
      this.status = status;
    }
  }

  /**
   * Convert the span to a plain JSON-serialisable object.
   * @returns {Object}
   */
  toJSON() {
    return {
      traceId: this.traceId,
      spanId: this.spanId,
      parentSpanId: this.parentSpanId,
      name: this.name,
      kind: this.kind,
      startTimeMs: this.startTimeMs,
      endTimeMs: this.endTimeMs,
      durationMs: this.durationMs,
      status: this.status,
      attributes: Object.fromEntries(this.attributes),
      events: this.events.slice(),
    };
  }
}

// ---------------------------------------------------------------------------
// SpanContext (extracted from incoming headers)
// ---------------------------------------------------------------------------

/**
 * @typedef {Object} SpanContext
 * @property {string} traceId
 * @property {string} spanId
 * @property {string} traceFlags
 */

// ---------------------------------------------------------------------------
// Tracing Service
// ---------------------------------------------------------------------------

/**
 * Create a distributed tracing service.
 *
 * @param {Object} [options]
 * @param {number}  [options.maxSpans=10000] - Ring buffer capacity
 * @param {string}  [options.serviceName]    - Logical service/agent name
 * @returns {Object} Tracing service API
 */
export function createTracingService(options = {}) {
  const maxSpans = options.maxSpans ?? DEFAULT_MAX_SPANS;
  const serviceName = options.serviceName ?? 'a2a-agent';

  /**
   * Ring buffer storing completed (and in-progress) spans.
   * @type {Span[]}
   */
  const _spans = [];

  /**
   * Index: traceId -> Set of buffer indices.
   * Rebuilt lazily when needed.
   * @type {Map<string, Set<number>>}
   */
  const _traceIndex = new Map();

  /**
   * The "current" span context used by inject().
   * Updated whenever a new root span is started or context is extracted.
   * @type {SpanContext|null}
   */
  let _currentContext = null;

  // -----------------------------------------------------------------------
  // Internal helpers
  // -----------------------------------------------------------------------

  /**
   * Append a span to the ring buffer, evicting the oldest if full.
   * @param {Span} span
   */
  function _pushSpan(span) {
    if (_spans.length >= maxSpans) {
      // Evict oldest
      const evicted = _spans.shift();
      // Rebuild trace index for evicted span's trace
      if (evicted) {
        _rebuildTraceIndex(evicted.traceId);
      }
    }

    const idx = _spans.length;
    _spans.push(span);

    // Update trace index
    if (!_traceIndex.has(span.traceId)) {
      _traceIndex.set(span.traceId, new Set());
    }
    _traceIndex.get(span.traceId).add(idx);
  }

  /**
   * Rebuild the trace index for a specific traceId after eviction.
   * @param {string} traceId
   */
  function _rebuildTraceIndex(_traceId) {
    // After a shift, ALL indices change — full rebuild is safer
    _traceIndex.clear();
    for (let i = 0; i < _spans.length; i++) {
      const tid = _spans[i].traceId;
      if (!_traceIndex.has(tid)) {
        _traceIndex.set(tid, new Set());
      }
      _traceIndex.get(tid).add(i);
    }
  }

  // -----------------------------------------------------------------------
  // Public API
  // -----------------------------------------------------------------------

  /**
   * Start a new span.
   *
   * @param {string} name - Operation name
   * @param {Object} [opts]
   * @param {'client'|'server'|'internal'} [opts.kind='internal']
   * @param {string} [opts.parentSpanId]    - Explicit parent span ID
   * @param {string} [opts.traceId]         - Explicit trace ID (e.g. from extracted context)
   * @param {Record<string, unknown>} [opts.attributes] - Initial attributes
   * @returns {Span}
   */
  function startSpan(name, opts = {}) {
    const kind = VALID_KINDS.has(opts.kind) ? opts.kind : 'internal';
    const traceId = opts.traceId ?? _currentContext?.traceId ?? generateTraceId();
    const spanId = generateSpanId();
    const parentSpanId = opts.parentSpanId ?? null;

    const span = new Span({
      traceId,
      spanId,
      parentSpanId,
      name,
      kind,
      attributes: opts.attributes,
    });

    // Set service name attribute
    span.setAttribute('service.name', serviceName);

    // Update current context
    _currentContext = {
      traceId: span.traceId,
      spanId: span.spanId,
      traceFlags: DEFAULT_FLAGS,
    };

    _pushSpan(span);
    return span;
  }

  /**
   * Inject trace context into outgoing headers (W3C Trace Context format).
   *
   * Mutates the provided headers object by setting `traceparent` and
   * optionally `tracestate`.
   *
   * @param {Record<string, string>} headers - Mutable headers object
   * @returns {Record<string, string>} The same headers object (for chaining)
   */
  function inject(headers) {
    if (!_currentContext) return headers;
    const { traceId, spanId, traceFlags } = _currentContext;
    headers.traceparent = `${TRACE_VERSION}-${traceId}-${spanId}-${traceFlags}`;
    return headers;
  }

  /**
   * Extract trace context from incoming headers.
   *
   * Parses the `traceparent` header per the W3C Trace Context spec.
   * Returns `null` if the header is missing or malformed.
   *
   * @param {Record<string, string>} headers
   * @returns {SpanContext|null}
   */
  function extract(headers) {
    const raw = headers?.traceparent;
    if (!raw || typeof raw !== 'string') return null;

    const parts = raw.split('-');
    if (parts.length !== 4) return null;

    const [version, traceId, spanId, traceFlags] = parts;

    // Validate lengths per W3C spec
    if (version.length !== 2) return null;
    if (traceId.length !== 32) return null;
    if (spanId.length !== 16) return null;
    if (traceFlags.length !== 2) return null;

    // Validate hex characters
    const hexRe = /^[0-9a-f]+$/;
    if (!hexRe.test(traceId) || !hexRe.test(spanId)) return null;

    const ctx = { traceId, spanId, traceFlags };

    // Update the service's current context so child spans inherit this trace
    _currentContext = ctx;

    return ctx;
  }

  /**
   * Retrieve all spans belonging to a given trace.
   *
   * @param {string} traceId
   * @returns {Object[]} Array of span JSON objects
   */
  function getTrace(traceId) {
    const results = [];
    for (const span of _spans) {
      if (span.traceId === traceId) {
        results.push(span.toJSON());
      }
    }
    return results;
  }

  /**
   * Compute aggregate metrics from all completed spans in the buffer.
   *
   * @returns {{
   *   p50: number,
   *   p95: number,
   *   p99: number,
   *   errorRate: number,
   *   throughput: number,
   *   spanCount: number,
   * }}
   */
  function getMetrics() {
    const completed = _spans.filter((s) => s.durationMs !== null && s.durationMs !== undefined);
    const total = completed.length;

    if (total === 0) {
      return { p50: 0, p95: 0, p99: 0, errorRate: 0, throughput: 0, spanCount: 0 };
    }

    // Duration percentiles
    const durations = completed.map((s) => s.durationMs).sort((a, b) => a - b);
    const p50 = percentile(durations, 50);
    const p95 = percentile(durations, 95);
    const p99 = percentile(durations, 99);

    // Error rate
    const errors = completed.filter((s) => s.status === 'error').length;
    const errorRate = errors / total;

    // Throughput: spans per second across the observation window
    const earliest = Math.min(...completed.map((s) => s.startTimeMs));
    const latest = Math.max(...completed.map((s) => s.endTimeMs));
    const windowSec = (latest - earliest) / 1000;
    const throughput = windowSec > 0 ? total / windowSec : total;

    return { p50, p95, p99, errorRate, throughput, spanCount: total };
  }

  /**
   * Return the N most recent spans (newest last).
   *
   * @param {number} [limit=100]
   * @returns {Object[]} Array of span JSON objects
   */
  function getRecentSpans(limit = 100) {
    const start = Math.max(0, _spans.length - limit);
    return _spans.slice(start).map((s) => s.toJSON());
  }

  /**
   * Export all buffered spans in OpenTelemetry-compatible JSON (OTLP).
   *
   * Follows the OTLP/JSON Trace specification structure:
   * `{ resourceSpans: [{ resource, scopeSpans: [{ scope, spans }] }] }`
   *
   * @returns {Object} OTLP-compatible JSON object
   */
  function exportOTLP() {
    const otlpSpans = _spans.map((s) => {
      const json = s.toJSON();
      return {
        traceId: json.traceId,
        spanId: json.spanId,
        parentSpanId: json.parentSpanId ?? '',
        name: json.name,
        kind: _kindToOTLP(json.kind),
        startTimeUnixNano: String(json.startTimeMs * 1_000_000),
        endTimeUnixNano: json.endTimeMs ? String(json.endTimeMs * 1_000_000) : '',
        attributes: _attrsToOTLP(json.attributes),
        events: json.events.map((ev) => ({
          timeUnixNano: String(ev.timeMs * 1_000_000),
          name: ev.name,
          attributes: _attrsToOTLP(ev.attributes),
        })),
        status: {
          code: json.status === 'error' ? 2 : 1,
          message: json.status === 'error' ? 'ERROR' : '',
        },
      };
    });

    return {
      resourceSpans: [
        {
          resource: {
            attributes: _attrsToOTLP({ 'service.name': serviceName }),
          },
          scopeSpans: [
            {
              scope: { name: 'a2a-tracing', version: '1.0.0' },
              spans: otlpSpans,
            },
          ],
        },
      ],
    };
  }

  /**
   * Convenience: create a span, run an async/sync function, end the span.
   *
   * On success the span status remains 'ok'; on error it is set to 'error'
   * and the exception is re-thrown.
   *
   * @template T
   * @param {string} name - Span name
   * @param {(span: Span) => T|Promise<T>} fn - Function to execute within the span
   * @param {Object} [opts] - Same options as startSpan
   * @returns {Promise<T>}
   */
  async function withSpan(name, fn, opts) {
    const span = startSpan(name, opts);
    try {
      const result = await fn(span);
      span.end();
      return result;
    } catch (err) {
      span.setStatus('error');
      span.setAttribute('error.message', err?.message ?? String(err));
      span.end();
      throw err;
    }
  }

  // -----------------------------------------------------------------------
  // OTLP conversion helpers
  // -----------------------------------------------------------------------

  /**
   * Convert span kind string to OTLP SpanKind enum value.
   * @param {string} kind
   * @returns {number}
   */
  function _kindToOTLP(kind) {
    switch (kind) {
      case 'internal':
        return 1; // SPAN_KIND_INTERNAL
      case 'server':
        return 2; // SPAN_KIND_SERVER
      case 'client':
        return 3; // SPAN_KIND_CLIENT
      default:
        return 0; // SPAN_KIND_UNSPECIFIED
    }
  }

  /**
   * Convert a plain attributes object to OTLP KeyValue array.
   * @param {Record<string, unknown>} attrs
   * @returns {Array<{ key: string, value: Object }>}
   */
  function _attrsToOTLP(attrs) {
    return Object.entries(attrs ?? {}).map(([key, val]) => ({
      key,
      value: _toOTLPValue(val),
    }));
  }

  /**
   * Convert a JS value to an OTLP AnyValue.
   * @param {unknown} val
   * @returns {Object}
   */
  function _toOTLPValue(val) {
    if (typeof val === 'string') return { stringValue: val };
    if (typeof val === 'number')
      return Number.isInteger(val) ? { intValue: String(val) } : { doubleValue: val };
    if (typeof val === 'boolean') return { boolValue: val };
    return { stringValue: String(val) };
  }

  // -----------------------------------------------------------------------
  // Return public interface
  // -----------------------------------------------------------------------

  return {
    startSpan,
    inject,
    extract,
    getTrace,
    getMetrics,
    getRecentSpans,
    exportOTLP,
    withSpan,
  };
}

/**
 * Request Context Module for StateSet CLI
 *
 * Provides request-scoped context for tracing, logging, and debugging.
 * Implements async local storage for automatic context propagation.
 */

import { AsyncLocalStorage } from 'node:async_hooks';
import * as crypto from 'node:crypto';

/**
 * Async local storage for request context
 */
const asyncLocalStorage = new AsyncLocalStorage();

/**
 * Generate a unique request ID
 */
function generateRequestId() {
  const timestamp = Date.now().toString(36);
  const random = crypto.randomBytes(4).toString('hex');
  return `req_${timestamp}_${random}`;
}

/**
 * Generate a trace ID (longer, for distributed tracing)
 */
function generateTraceId() {
  return crypto.randomBytes(16).toString('hex');
}

/**
 * Generate a span ID
 */
function generateSpanId() {
  return crypto.randomBytes(8).toString('hex');
}

/**
 * RequestContext - Holds context for a single request
 */
export class RequestContext {
  constructor(options = {}) {
    this.requestId = options.requestId || generateRequestId();
    this.traceId = options.traceId || generateTraceId();
    this.spanId = options.spanId || generateSpanId();
    this.parentSpanId = options.parentSpanId || null;

    this.startTime = options.startTime || Date.now();
    this.agent = options.agent || null;
    this.sessionId = options.sessionId || null;
    this.userId = options.userId || null;
    this.dbPath = options.dbPath || null;

    this.request = options.request || null;
    this.applyMode = options.applyMode || false;

    this.metadata = options.metadata || {};
    this.tags = new Set(options.tags || []);
    this.events = [];
    this.errors = [];
  }

  /**
   * Get elapsed time in milliseconds
   */
  get elapsed() {
    return Date.now() - this.startTime;
  }

  /**
   * Add a tag
   */
  addTag(tag) {
    this.tags.add(tag);
    return this;
  }

  /**
   * Add multiple tags
   */
  addTags(tags) {
    for (const tag of tags) {
      this.tags.add(tag);
    }
    return this;
  }

  /**
   * Set metadata value
   */
  setMeta(key, value) {
    this.metadata[key] = value;
    return this;
  }

  /**
   * Get metadata value
   */
  getMeta(key) {
    return this.metadata[key];
  }

  /**
   * Log an event within this context
   */
  logEvent(name, data = {}) {
    this.events.push({
      timestamp: Date.now(),
      elapsed: this.elapsed,
      name,
      data,
    });
    return this;
  }

  /**
   * Log an error
   */
  logError(error, data = {}) {
    this.errors.push({
      timestamp: Date.now(),
      elapsed: this.elapsed,
      error:
        error instanceof Error
          ? {
              name: error.name,
              message: error.message,
              stack: error.stack,
            }
          : error,
      data,
    });
    return this;
  }

  /**
   * Create a child span
   */
  createSpan(name, options = {}) {
    return new Span({
      context: this,
      name,
      parentSpanId: this.spanId,
      ...options,
    });
  }

  /**
   * Convert to log-friendly object
   */
  toLogObject() {
    return {
      requestId: this.requestId,
      traceId: this.traceId,
      spanId: this.spanId,
      parentSpanId: this.parentSpanId,
      elapsed: this.elapsed,
      agent: this.agent,
      sessionId: this.sessionId,
      applyMode: this.applyMode,
      tags: Array.from(this.tags),
      metadata: this.metadata,
      eventCount: this.events.length,
      errorCount: this.errors.length,
    };
  }

  /**
   * Get trace headers for distributed tracing
   */
  getTraceHeaders() {
    return {
      'x-request-id': this.requestId,
      'x-trace-id': this.traceId,
      'x-span-id': this.spanId,
      'x-parent-span-id': this.parentSpanId,
    };
  }

  /**
   * Create child context (for nested operations)
   */
  createChild(options = {}) {
    return new RequestContext({
      traceId: this.traceId,
      parentSpanId: this.spanId,
      sessionId: this.sessionId,
      userId: this.userId,
      dbPath: this.dbPath,
      applyMode: this.applyMode,
      tags: Array.from(this.tags),
      ...options,
    });
  }

  /**
   * Serialize for storage/transmission
   */
  serialize() {
    return JSON.stringify({
      requestId: this.requestId,
      traceId: this.traceId,
      spanId: this.spanId,
      parentSpanId: this.parentSpanId,
      startTime: this.startTime,
      agent: this.agent,
      sessionId: this.sessionId,
      userId: this.userId,
      dbPath: this.dbPath,
      request: this.request,
      applyMode: this.applyMode,
      metadata: this.metadata,
      tags: Array.from(this.tags),
      events: this.events,
      errors: this.errors,
    });
  }

  /**
   * Deserialize from storage
   */
  static deserialize(json) {
    const data = typeof json === 'string' ? JSON.parse(json) : json;
    return new RequestContext(data);
  }
}

/**
 * Span - Represents a unit of work within a request
 */
export class Span {
  constructor(options = {}) {
    this.context = options.context;
    this.name = options.name;
    this.spanId = generateSpanId();
    this.parentSpanId = options.parentSpanId || null;
    this.startTime = Date.now();
    this.endTime = null;
    this.status = 'running';
    this.attributes = options.attributes || {};
    this.events = [];
  }

  /**
   * Set attribute
   */
  setAttribute(key, value) {
    this.attributes[key] = value;
    return this;
  }

  /**
   * Set multiple attributes
   */
  setAttributes(attrs) {
    for (const [key, value] of Object.entries(attrs || {})) {
      if (key === '__proto__' || key === 'constructor' || key === 'prototype') continue;
      this.attributes[key] = value;
    }
    return this;
  }

  /**
   * Add event
   */
  addEvent(name, attributes = {}) {
    this.events.push({
      timestamp: Date.now(),
      name,
      attributes,
    });
    return this;
  }

  /**
   * End the span successfully
   */
  end(status = 'ok') {
    this.endTime = Date.now();
    this.status = status;
    return this;
  }

  /**
   * End the span with error
   */
  error(err) {
    this.endTime = Date.now();
    this.status = 'error';
    this.setAttribute('error', true);
    this.setAttribute('error.message', err.message);
    if (err.stack) {
      this.setAttribute('error.stack', err.stack);
    }
    return this;
  }

  /**
   * Get duration in milliseconds
   */
  get duration() {
    if (!this.endTime) return Date.now() - this.startTime;
    return this.endTime - this.startTime;
  }

  /**
   * Convert to log object
   */
  toLogObject() {
    return {
      spanId: this.spanId,
      parentSpanId: this.parentSpanId,
      name: this.name,
      status: this.status,
      duration: this.duration,
      attributes: this.attributes,
      events: this.events,
    };
  }
}

// ============================================================================
// Context Management Functions
// ============================================================================

/**
 * Run a function with context
 */
export function runWithContext(context, fn) {
  return asyncLocalStorage.run(context, fn);
}

/**
 * Get current context (returns null if not in context)
 */
export function getContext() {
  return asyncLocalStorage.getStore() || null;
}

/**
 * Get current context or create a new one
 */
export function getOrCreateContext(options = {}) {
  const existing = getContext();
  if (existing) return existing;
  return new RequestContext(options);
}

/**
 * Create and run with new context
 */
export async function withContext(options, fn) {
  const context = new RequestContext(options);
  return runWithContext(context, async () => {
    try {
      const result = await fn(context);
      return result;
    } catch (error) {
      context.logError(error);
      throw error;
    }
  });
}

/**
 * Create a child context and run
 */
export async function withChildContext(options, fn) {
  const parent = getContext();
  const child = parent ? parent.createChild(options) : new RequestContext(options);

  return runWithContext(child, async () => {
    try {
      const result = await fn(child);
      return result;
    } catch (error) {
      child.logError(error);
      throw error;
    }
  });
}

/**
 * Create a span and run
 */
export async function withSpan(name, fn, options = {}) {
  const context = getContext();
  if (!context) {
    return fn(null);
  }

  const span = context.createSpan(name, options);

  try {
    const result = await fn(span);
    span.end('ok');
    return result;
  } catch (error) {
    span.error(error);
    throw error;
  }
}

// ============================================================================
// Context Logger Integration
// ============================================================================

/**
 * ContextLogger - Logger that automatically includes context
 */
export class ContextLogger {
  constructor(baseLogger) {
    this.baseLogger = baseLogger;
  }

  _log(level, message, meta = {}) {
    const context = getContext();
    const enrichedMeta = context ? { ...meta, ...context.toLogObject() } : meta;

    if (this.baseLogger[level]) {
      this.baseLogger[level](message, enrichedMeta);
    }
  }

  error(message, meta) {
    this._log('error', message, meta);
  }
  warn(message, meta) {
    this._log('warn', message, meta);
  }
  info(message, meta) {
    this._log('info', message, meta);
  }
  debug(message, meta) {
    this._log('debug', message, meta);
  }
  trace(message, meta) {
    this._log('trace', message, meta);
  }
}

/**
 * Create a context-aware logger
 */
export function createContextLogger(baseLogger) {
  return new ContextLogger(baseLogger);
}

// ============================================================================
// Request Middleware
// ============================================================================

/**
 * Create middleware that sets up request context
 */
export function createContextMiddleware(options = {}) {
  return async (request, next) => {
    const context = new RequestContext({
      request: typeof request === 'string' ? request : request.query,
      agent: request.agent,
      sessionId: request.sessionId,
      applyMode: request.applyMode,
      dbPath: request.dbPath,
      ...options,
    });

    return runWithContext(context, async () => {
      context.logEvent('request_start');

      try {
        const result = await next(request, context);
        context.logEvent('request_end', { success: true });
        return { ...result, context: context.toLogObject() };
      } catch (error) {
        context.logError(error);
        context.logEvent('request_end', { success: false });
        throw error;
      }
    });
  };
}

export default {
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
  generateRequestId,
  generateTraceId,
  generateSpanId,
};

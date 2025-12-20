/**
 * Telemetry & Observability for StateSet Agent Harness
 *
 * Provides structured logging, distributed tracing, and metrics collection
 * for debugging and monitoring agent operations.
 */

import { EventEmitter } from 'node:events';

// ============================================================================
// Trace & Span Management
// ============================================================================

/**
 * Generate a unique trace/span ID
 */
function generateId() {
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 9)}`;
}

/**
 * A span represents a single operation within a trace
 */
export class Span {
  constructor(name, traceId, parentSpanId = null, metadata = {}) {
    this.id = generateId();
    this.traceId = traceId;
    this.parentSpanId = parentSpanId;
    this.name = name;
    this.startTime = Date.now();
    this.endTime = null;
    this.duration = null;
    this.status = 'running';
    this.metadata = metadata;
    this.events = [];
    this.attributes = {};
  }

  /**
   * Add an event to this span
   */
  addEvent(name, data = {}) {
    this.events.push({
      name,
      timestamp: Date.now(),
      data
    });
  }

  /**
   * Set an attribute on this span
   */
  setAttribute(key, value) {
    this.attributes[key] = value;
  }

  /**
   * End this span
   */
  end(status = 'ok', result = null) {
    this.endTime = Date.now();
    this.duration = this.endTime - this.startTime;
    this.status = status;
    if (result !== null) {
      this.result = result;
    }
  }

  /**
   * Convert to JSON-serializable object
   */
  toJSON() {
    return {
      id: this.id,
      traceId: this.traceId,
      parentSpanId: this.parentSpanId,
      name: this.name,
      startTime: this.startTime,
      endTime: this.endTime,
      duration: this.duration,
      status: this.status,
      metadata: this.metadata,
      attributes: this.attributes,
      events: this.events,
      result: this.result
    };
  }
}

// ============================================================================
// Main Telemetry Class
// ============================================================================

/**
 * AgentTelemetry - Central telemetry collector for agent operations
 *
 * Usage:
 *   const telemetry = new AgentTelemetry({ verbose: true });
 *   const span = telemetry.startSpan('agent_run');
 *   // ... do work ...
 *   telemetry.logToolCall('list_orders', { limit: 10 }, result, 150);
 *   span.end('ok');
 */
export class AgentTelemetry extends EventEmitter {
  constructor(options = {}) {
    super();
    this.traceId = options.traceId || generateId();
    this.verbose = options.verbose || false;
    this.outputFormat = options.outputFormat || 'pretty'; // 'pretty' | 'json' | 'silent'

    // Storage
    this.spans = [];
    this.toolCalls = [];
    this.metrics = {
      totalToolCalls: 0,
      successfulToolCalls: 0,
      failedToolCalls: 0,
      totalDuration: 0,
      toolDurations: {}
    };

    // Current context
    this.currentSpan = null;
    this.spanStack = [];

    // Timing
    this.startTime = Date.now();
  }

  // --------------------------------------------------------------------------
  // Span Management
  // --------------------------------------------------------------------------

  /**
   * Start a new span
   */
  startSpan(name, metadata = {}) {
    const parentSpanId = this.currentSpan?.id || null;
    const span = new Span(name, this.traceId, parentSpanId, metadata);

    this.spans.push(span);
    this.spanStack.push(span);
    this.currentSpan = span;

    this.emit('span:start', span);

    if (this.verbose) {
      this._log('span', `Started: ${name}`, { spanId: span.id });
    }

    return span;
  }

  /**
   * End the current span
   */
  endSpan(status = 'ok', result = null) {
    const span = this.spanStack.pop();
    if (span) {
      span.end(status, result);
      this.currentSpan = this.spanStack[this.spanStack.length - 1] || null;

      this.emit('span:end', span);

      if (this.verbose) {
        this._log('span', `Ended: ${span.name} (${span.duration}ms, ${status})`, { spanId: span.id });
      }
    }
    return span;
  }

  /**
   * End a specific span by reference
   */
  endSpanRef(span, status = 'ok', result = null) {
    if (span && span.status === 'running') {
      span.end(status, result);
      this.emit('span:end', span);

      if (this.verbose) {
        this._log('span', `Ended: ${span.name} (${span.duration}ms, ${status})`, { spanId: span.id });
      }
    }
    return span;
  }

  // --------------------------------------------------------------------------
  // Tool Call Logging
  // --------------------------------------------------------------------------

  /**
   * Log a tool call with timing and result
   */
  logToolCall(toolName, input, output, duration = null) {
    const startTime = duration ? Date.now() - duration : Date.now();
    const endTime = Date.now();
    const actualDuration = duration || 0;

    const isSuccess = !output?.error;
    const record = {
      id: generateId(),
      traceId: this.traceId,
      spanId: this.currentSpan?.id,
      toolName,
      input,
      output,
      startTime,
      endTime,
      duration: actualDuration,
      success: isSuccess
    };

    this.toolCalls.push(record);

    // Update metrics
    this.metrics.totalToolCalls++;
    if (isSuccess) {
      this.metrics.successfulToolCalls++;
    } else {
      this.metrics.failedToolCalls++;
    }
    this.metrics.totalDuration += actualDuration;

    if (!this.metrics.toolDurations[toolName]) {
      this.metrics.toolDurations[toolName] = { count: 0, totalMs: 0, avgMs: 0 };
    }
    this.metrics.toolDurations[toolName].count++;
    this.metrics.toolDurations[toolName].totalMs += actualDuration;
    this.metrics.toolDurations[toolName].avgMs =
      this.metrics.toolDurations[toolName].totalMs / this.metrics.toolDurations[toolName].count;

    // Add event to current span
    if (this.currentSpan) {
      this.currentSpan.addEvent('tool_call', { toolName, duration: actualDuration, success: isSuccess });
    }

    this.emit('tool:call', record);

    if (this.verbose) {
      const status = isSuccess ? 'ok' : 'error';
      this._log('tool', `${toolName} (${actualDuration}ms) [${status}]`, {
        input: this._truncate(JSON.stringify(input), 100)
      });
    }

    return record;
  }

  /**
   * Start timing a tool call (returns a function to call when done)
   */
  startToolCall(toolName, input) {
    const startTime = Date.now();

    return (output) => {
      const duration = Date.now() - startTime;
      return this.logToolCall(toolName, input, output, duration);
    };
  }

  // --------------------------------------------------------------------------
  // Agent Events
  // --------------------------------------------------------------------------

  /**
   * Log agent routing decision
   */
  logAgentRouting(request, selectedAgent, confidence, alternatives = []) {
    const record = {
      timestamp: Date.now(),
      request: this._truncate(request, 200),
      selectedAgent,
      confidence,
      alternatives
    };

    if (this.currentSpan) {
      this.currentSpan.addEvent('agent_routing', record);
    }

    this.emit('agent:routing', record);

    if (this.verbose) {
      this._log('route', `Selected "${selectedAgent}" (${Math.round(confidence * 100)}% confidence)`, {
        alternatives: alternatives.slice(0, 2).map(a => a.agent).join(', ')
      });
    }
  }

  /**
   * Log assistant message
   */
  logAssistantMessage(message) {
    if (this.currentSpan) {
      this.currentSpan.addEvent('assistant_message', {
        length: message.length,
        preview: this._truncate(message, 100)
      });
    }

    this.emit('assistant:message', { message, timestamp: Date.now() });
  }

  /**
   * Log an error
   */
  logError(error, context = {}) {
    const record = {
      timestamp: Date.now(),
      error: error.message,
      stack: error.stack,
      context
    };

    if (this.currentSpan) {
      this.currentSpan.addEvent('error', record);
      this.currentSpan.setAttribute('error', true);
    }

    this.emit('error', record);

    if (this.verbose) {
      this._log('error', error.message, context);
    }
  }

  /**
   * Log a custom event
   */
  logCustomEvent(eventName, data = {}) {
    const record = {
      timestamp: Date.now(),
      eventName,
      ...data
    };

    if (this.currentSpan) {
      this.currentSpan.addEvent(eventName, data);
    }

    this.emit('custom:' + eventName, record);

    if (this.verbose) {
      this._log('info', eventName, data);
    }
  }

  // --------------------------------------------------------------------------
  // Reporting
  // --------------------------------------------------------------------------

  /**
   * Get complete trace data
   */
  getTrace() {
    return {
      traceId: this.traceId,
      startTime: this.startTime,
      endTime: Date.now(),
      totalDuration: Date.now() - this.startTime,
      spans: this.spans.map(s => s.toJSON()),
      toolCalls: this.toolCalls,
      metrics: this.metrics
    };
  }

  /**
   * Get summary statistics
   */
  getSummary() {
    return {
      traceId: this.traceId,
      duration: Date.now() - this.startTime,
      spanCount: this.spans.length,
      toolCalls: {
        total: this.metrics.totalToolCalls,
        successful: this.metrics.successfulToolCalls,
        failed: this.metrics.failedToolCalls,
        successRate: this.metrics.totalToolCalls > 0
          ? (this.metrics.successfulToolCalls / this.metrics.totalToolCalls * 100).toFixed(1) + '%'
          : 'N/A'
      },
      avgToolDuration: this.metrics.totalToolCalls > 0
        ? Math.round(this.metrics.totalDuration / this.metrics.totalToolCalls)
        : 0,
      topTools: Object.entries(this.metrics.toolDurations)
        .sort((a, b) => b[1].count - a[1].count)
        .slice(0, 5)
        .map(([name, stats]) => ({ name, ...stats }))
    };
  }

  /**
   * Print summary to console
   */
  printSummary() {
    const summary = this.getSummary();

    console.log('\n' + '─'.repeat(50));
    console.log('📊 Agent Execution Summary');
    console.log('─'.repeat(50));
    console.log(`Trace ID:     ${summary.traceId}`);
    console.log(`Duration:     ${summary.duration}ms`);
    console.log(`Tool Calls:   ${summary.toolCalls.total} (${summary.toolCalls.successRate} success)`);
    console.log(`Avg Latency:  ${summary.avgToolDuration}ms per tool`);

    if (summary.topTools.length > 0) {
      console.log('\nTop Tools:');
      for (const tool of summary.topTools) {
        console.log(`  • ${tool.name}: ${tool.count}x (avg ${Math.round(tool.avgMs)}ms)`);
      }
    }
    console.log('─'.repeat(50) + '\n');
  }

  // --------------------------------------------------------------------------
  // Internal Helpers
  // --------------------------------------------------------------------------

  _log(type, message, data = {}) {
    if (this.outputFormat === 'silent') return;

    const icons = {
      span: '📍',
      tool: '🔧',
      route: '🧭',
      error: '❌',
      info: 'ℹ️'
    };

    const timestamp = new Date().toISOString().split('T')[1].slice(0, 12);
    const icon = icons[type] || 'ℹ️';

    if (this.outputFormat === 'json') {
      console.log(JSON.stringify({ timestamp, type, message, ...data }));
    } else {
      const dataStr = Object.keys(data).length > 0
        ? ` ${JSON.stringify(data)}`
        : '';
      console.log(`[${timestamp}] ${icon} ${message}${dataStr}`);
    }
  }

  _truncate(str, maxLen) {
    if (str.length <= maxLen) return str;
    return str.slice(0, maxLen - 3) + '...';
  }
}

// ============================================================================
// Convenience Functions
// ============================================================================

/**
 * Create a telemetry instance with common presets
 */
export function createTelemetry(options = {}) {
  return new AgentTelemetry(options);
}

/**
 * No-op telemetry for when telemetry is disabled
 */
export class NoOpTelemetry {
  startSpan() { return { end: () => {}, addEvent: () => {}, setAttribute: () => {} }; }
  endSpan() { return null; }
  endSpanRef() { return null; }
  logToolCall() { return {}; }
  startToolCall() { return () => ({}); }
  logAgentRouting() {}
  logAssistantMessage() {}
  logError() {}
  logCustomEvent() {}
  getTrace() { return {}; }
  getSummary() { return {}; }
  printSummary() {}
  on() {}
  emit() {}
  get traceId() { return null; }
}

export const noOpTelemetry = new NoOpTelemetry();

export default AgentTelemetry;

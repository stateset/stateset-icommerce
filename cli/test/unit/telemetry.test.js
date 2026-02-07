/**
 * Tests for Telemetry & Secret Redaction
 */

import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { AgentTelemetry, Span } from '../../src/telemetry.js';

describe('AgentTelemetry', () => {
  describe('generateId (via Span)', () => {
    it('generates UUID-format IDs', () => {
      const telemetry = new AgentTelemetry();
      const span = telemetry.startSpan('test');
      // UUID v4 format: 8-4-4-4-12 hex chars
      assert.match(span.id, /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);
      telemetry.endSpan();
    });

    it('generates unique IDs', () => {
      const telemetry = new AgentTelemetry();
      const ids = new Set();
      for (let i = 0; i < 100; i++) {
        const span = telemetry.startSpan(`test-${i}`);
        ids.add(span.id);
        telemetry.endSpan();
      }
      assert.equal(ids.size, 100);
    });
  });

  describe('secret redaction in logToolCall', () => {
    it('redacts sensitive keys from verbose tool call logs', () => {
      const logs = [];
      const origLog = console.log;
      console.log = (...args) => logs.push(args.join(' '));

      const telemetry = new AgentTelemetry({ verbose: true, outputFormat: 'json' });
      telemetry.logToolCall('test_tool', {
        query: 'hello',
        apiKey: 'sk-secret-123',
        password: 'hunter2',
        token: 'tok-abc',
      }, { result: 'ok' }, 10);

      console.log = origLog;

      const loggedStr = logs.join('\n');
      assert.ok(!loggedStr.includes('sk-secret-123'), 'apiKey should be redacted');
      assert.ok(!loggedStr.includes('hunter2'), 'password should be redacted');
      assert.ok(!loggedStr.includes('tok-abc'), 'token should be redacted');
      assert.ok(loggedStr.includes('[REDACTED]'), 'should contain [REDACTED] placeholder');
    });

    it('preserves non-sensitive keys in logs', () => {
      const logs = [];
      const origLog = console.log;
      console.log = (...args) => logs.push(args.join(' '));

      const telemetry = new AgentTelemetry({ verbose: true, outputFormat: 'json' });
      telemetry.logToolCall('test_tool', {
        query: 'hello world',
        limit: 10,
      }, { result: 'ok' }, 5);

      console.log = origLog;

      const loggedStr = logs.join('\n');
      assert.ok(loggedStr.includes('hello world'), 'non-sensitive query should be present');
    });
  });

  describe('secret redaction in logError', () => {
    it('redacts sensitive context fields in error logs', () => {
      const telemetry = new AgentTelemetry({ verbose: false });
      // Must add an 'error' listener to prevent EventEmitter from throwing
      telemetry.on('error', () => {});
      const span = telemetry.startSpan('test');

      telemetry.logError(new Error('fail'), {
        authorization: 'Bearer sk-123',
        credential: 'cred-abc',
        normalField: 'visible',
      });

      // Check the recorded span event
      const errorEvent = span.events.find(e => e.name === 'error');
      assert.ok(errorEvent, 'should record error event');
      assert.equal(errorEvent.data.context.authorization, '[REDACTED]');
      assert.equal(errorEvent.data.context.credential, '[REDACTED]');
      assert.equal(errorEvent.data.context.normalField, 'visible');

      telemetry.endSpan();
    });
  });

  describe('span management', () => {
    it('tracks nested spans correctly', () => {
      const telemetry = new AgentTelemetry();
      const outer = telemetry.startSpan('outer');
      const inner = telemetry.startSpan('inner');

      assert.equal(inner.parentSpanId, outer.id);

      telemetry.endSpan('ok');
      telemetry.endSpan('ok');

      assert.equal(telemetry.spans.length, 2);
    });
  });

  describe('metrics', () => {
    it('tracks tool call success/failure counts', () => {
      const telemetry = new AgentTelemetry();

      telemetry.logToolCall('tool_a', {}, { data: 'ok' }, 10);
      telemetry.logToolCall('tool_b', {}, { error: 'fail' }, 5);
      telemetry.logToolCall('tool_a', {}, { data: 'ok' }, 8);

      assert.equal(telemetry.metrics.totalToolCalls, 3);
      assert.equal(telemetry.metrics.successfulToolCalls, 2);
      assert.equal(telemetry.metrics.failedToolCalls, 1);
    });
  });
});

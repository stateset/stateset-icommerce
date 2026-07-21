/**
 * Tests for Telemetry & Secret Redaction
 */

import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import {
  AgentTelemetry,
  Span,
  NoOpTelemetry,
  noOpTelemetry,
  redactSensitiveFields,
  SENSITIVE_KEY_PATTERN,
} from '../../src/telemetry.js';

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
      telemetry.logToolCall(
        'test_tool',
        {
          query: 'hello',
          apiKey: 'sk-secret-123',
          password: 'hunter2',
          token: 'tok-abc',
        },
        { result: 'ok' },
        10,
      );

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
      telemetry.logToolCall(
        'test_tool',
        {
          query: 'hello world',
          limit: 10,
        },
        { result: 'ok' },
        5,
      );

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
      const errorEvent = span.events.find((e) => e.name === 'error');
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

// ============================================================================
// SENSITIVE_KEY_PATTERN — crypto field coverage
// ============================================================================

describe('SENSITIVE_KEY_PATTERN — crypto fields', () => {
  const cryptoFields = [
    'signature',
    'x402_signature',
    'merkle_proof',
    'nonce',
    'agent_nonce',
    'receipt_hash',
    'wallet_address',
    'intent_id',
    'mnemonic',
    'seed_phrase',
    'private_key',
    'signing_key',
  ];

  for (const field of cryptoFields) {
    it(`matches crypto field: ${field}`, () => {
      assert.ok(SENSITIVE_KEY_PATTERN.test(field), `Expected "${field}" to match`);
    });
  }

  const safeFields = ['amount', 'status', 'orderId', 'email', 'currency', 'eventType', 'chain'];
  for (const field of safeFields) {
    it(`does not match safe field: ${field}`, () => {
      assert.ok(!SENSITIVE_KEY_PATTERN.test(field), `"${field}" should NOT match`);
    });
  }
});

// ============================================================================
// redactSensitiveFields — exported function tests
// ============================================================================

describe('redactSensitiveFields (exported)', () => {
  it('redacts all crypto-sensitive fields in a flat object', () => {
    const input = {
      orderId: 'ord-1',
      amount: 100,
      signature: '0xdeadbeef',
      merkle_proof: ['h1', 'h2'],
      nonce: 42,
      receipt_hash: 'sha256:abc',
      wallet_address: '0xSeller',
      intent_id: 'int-123',
      mnemonic: 'abandon ship',
      seed_phrase: 'word1 word2',
    };
    const result = redactSensitiveFields(input);
    assert.equal(result.orderId, 'ord-1');
    assert.equal(result.amount, 100);
    assert.equal(result.signature, '[REDACTED]');
    assert.equal(result.merkle_proof, '[REDACTED]');
    assert.equal(result.nonce, '[REDACTED]');
    assert.equal(result.receipt_hash, '[REDACTED]');
    assert.equal(result.wallet_address, '[REDACTED]');
    assert.equal(result.intent_id, '[REDACTED]');
    assert.equal(result.mnemonic, '[REDACTED]');
    assert.equal(result.seed_phrase, '[REDACTED]');
  });

  it('recursively redacts nested sensitive fields', () => {
    const result = redactSensitiveFields({
      payment: { amount: 50, wallet_address: '0xA', signature: '0xB' },
    });
    assert.equal(result.payment.amount, 50);
    assert.equal(result.payment.wallet_address, '[REDACTED]');
    assert.equal(result.payment.signature, '[REDACTED]');
  });

  it('handles arrays of objects with sensitive fields', () => {
    const result = redactSensitiveFields([
      { id: '1', nonce: 1 },
      { id: '2', nonce: 2 },
    ]);
    assert.ok(Array.isArray(result));
    assert.equal(result[0].id, '1');
    assert.equal(result[0].nonce, '[REDACTED]');
    assert.equal(result[1].nonce, '[REDACTED]');
  });

  it('returns null/undefined/primitives unchanged', () => {
    assert.equal(redactSensitiveFields(null), null);
    assert.equal(redactSensitiveFields(undefined), undefined);
    assert.equal(redactSensitiveFields('hello'), 'hello');
    assert.equal(redactSensitiveFields(42), 42);
  });

  it('stops recursion beyond depth 5', () => {
    const deep = { a: { b: { c: { d: { e: { f: { password: 'deep' } } } } } } };
    const result = redactSensitiveFields(deep);
    // At depth 6, the object is returned as-is
    assert.equal(result.a.b.c.d.e.f.password, 'deep');
  });
});

// ============================================================================
// NoOpTelemetry tests
// ============================================================================

describe('NoOpTelemetry', () => {
  it('all methods are callable without throwing', () => {
    const noop = new NoOpTelemetry();
    assert.doesNotThrow(() => {
      noop.startSpan('x');
      noop.endSpan();
      noop.endSpanRef(null);
      noop.logToolCall('t', {}, {}, 0);
      noop.startToolCall('t', {});
      noop.logAgentRouting('r', 'a', 0.9);
      noop.logAssistantMessage('msg');
      noop.logError(new Error('e'));
      noop.logCustomEvent('evt');
      noop.getTrace();
      noop.getSummary();
      noop.printSummary();
      noop.on('x', () => {});
      noop.emit('x');
    });
  });

  it('traceId returns null', () => {
    assert.equal(noOpTelemetry.traceId, null);
  });

  it('singleton is a NoOpTelemetry instance', () => {
    assert.ok(noOpTelemetry instanceof NoOpTelemetry);
  });
});

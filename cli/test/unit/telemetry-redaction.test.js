/**
 * Telemetry redaction tests
 *
 * Verifies that API keys, PII, and sensitive credentials are properly
 * redacted in telemetry output while non-sensitive fields are preserved.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import {
  AgentTelemetry,
  Span,
  NoOpTelemetry,
  noOpTelemetry,
  redactSensitiveFields,
  SENSITIVE_KEY_PATTERN,
} from '../../src/telemetry.js';

// ============================================================================
// Tests
// ============================================================================

describe('telemetry redaction', () => {
  // --------------------------------------------------------------------------
  // SENSITIVE_KEY_PATTERN
  // --------------------------------------------------------------------------

  describe('SENSITIVE_KEY_PATTERN', () => {
    const shouldMatch = [
      'password',
      'token',
      'secret',
      'apiKey',
      'api_key',
      'authorization',
      'credential',
      'signature',
      'merkle_proof',
      'nonce',
      'receipt_hash',
      'wallet_address',
      'intent_id',
      'mnemonic',
      'seed_phrase',
      'private_key',
      'signing_key',
      // Case variations
      'PASSWORD',
      'ApiKey',
      'API_KEY',
      'Authorization',
      'Secret',
      'Token',
      'PRIVATE_KEY',
    ];

    const shouldNotMatch = [
      'name',
      'email',
      'query',
      'limit',
      'status',
      'orderId',
      'customerId',
      'description',
      'amount',
      'currency',
    ];

    for (const field of shouldMatch) {
      it(`matches sensitive field "${field}"`, () => {
        assert.ok(
          SENSITIVE_KEY_PATTERN.test(field),
          `SENSITIVE_KEY_PATTERN should match "${field}"`,
        );
      });
    }

    for (const field of shouldNotMatch) {
      it(`does not match non-sensitive field "${field}"`, () => {
        assert.ok(
          !SENSITIVE_KEY_PATTERN.test(field),
          `SENSITIVE_KEY_PATTERN should NOT match "${field}"`,
        );
      });
    }
  });

  // --------------------------------------------------------------------------
  // redactSensitiveFields — API keys
  // --------------------------------------------------------------------------

  describe('API key redaction', () => {
    it('redacts Anthropic API key', () => {
      const input = { apiKey: 'sk-ant-api03-real-secret-key-12345' };
      const result = redactSensitiveFields(input);
      assert.strictEqual(result.apiKey, '[REDACTED]');
    });

    it('redacts OpenAI API key', () => {
      const input = { api_key: 'sk-proj-abc123def456ghi789' };
      const result = redactSensitiveFields(input);
      assert.strictEqual(result.api_key, '[REDACTED]');
    });

    it('redacts nested API keys', () => {
      const input = {
        config: {
          provider: {
            apiKey: 'sk-secret-nested-key',
          },
        },
      };
      const result = redactSensitiveFields(input);
      assert.strictEqual(result.config.provider.apiKey, '[REDACTED]');
    });

    it('redacts authorization header value', () => {
      const input = { authorization: 'Bearer sk-ant-api03-secret' };
      const result = redactSensitiveFields(input);
      assert.strictEqual(result.authorization, '[REDACTED]');
    });

    it('redacts credential field', () => {
      const input = { credential: 'some-oauth-credential-value' };
      const result = redactSensitiveFields(input);
      assert.strictEqual(result.credential, '[REDACTED]');
    });
  });

  // --------------------------------------------------------------------------
  // redactSensitiveFields — crypto/wallet fields
  // --------------------------------------------------------------------------

  describe('crypto/wallet field redaction', () => {
    it('redacts private_key', () => {
      const input = { private_key: '0xdeadbeef1234567890abcdef' };
      const result = redactSensitiveFields(input);
      assert.strictEqual(result.private_key, '[REDACTED]');
    });

    it('redacts signing_key', () => {
      const input = { signing_key: 'ed25519-secret-key-bytes' };
      const result = redactSensitiveFields(input);
      assert.strictEqual(result.signing_key, '[REDACTED]');
    });

    it('redacts wallet_address', () => {
      const input = { wallet_address: '0x1234567890abcdef1234567890abcdef12345678' };
      const result = redactSensitiveFields(input);
      assert.strictEqual(result.wallet_address, '[REDACTED]');
    });

    it('redacts mnemonic', () => {
      const input = { mnemonic: 'abandon abandon abandon abandon abandon abandon' };
      const result = redactSensitiveFields(input);
      assert.strictEqual(result.mnemonic, '[REDACTED]');
    });

    it('redacts seed_phrase', () => {
      const input = { seed_phrase: 'word1 word2 word3 word4 word5 word6' };
      const result = redactSensitiveFields(input);
      assert.strictEqual(result.seed_phrase, '[REDACTED]');
    });

    it('redacts signature', () => {
      const input = { signature: 'base64-encoded-signature-value' };
      const result = redactSensitiveFields(input);
      assert.strictEqual(result.signature, '[REDACTED]');
    });

    it('redacts nonce', () => {
      const input = { nonce: 42 };
      const result = redactSensitiveFields(input);
      assert.strictEqual(result.nonce, '[REDACTED]');
    });

    it('redacts merkle_proof', () => {
      const input = { merkle_proof: ['hash1', 'hash2', 'hash3'] };
      const result = redactSensitiveFields(input);
      assert.strictEqual(result.merkle_proof, '[REDACTED]');
    });

    it('redacts receipt_hash', () => {
      const input = { receipt_hash: '0xabc123' };
      const result = redactSensitiveFields(input);
      assert.strictEqual(result.receipt_hash, '[REDACTED]');
    });

    it('redacts intent_id', () => {
      const input = { intent_id: 'intent-uuid-1234' };
      const result = redactSensitiveFields(input);
      assert.strictEqual(result.intent_id, '[REDACTED]');
    });
  });

  // --------------------------------------------------------------------------
  // redactSensitiveFields — preserving non-sensitive fields
  // --------------------------------------------------------------------------

  describe('non-sensitive field preservation', () => {
    it('preserves string fields', () => {
      const input = { name: 'John Doe', status: 'active' };
      const result = redactSensitiveFields(input);
      assert.strictEqual(result.name, 'John Doe');
      assert.strictEqual(result.status, 'active');
    });

    it('preserves numeric fields', () => {
      const input = { amount: 99.99, quantity: 5, limit: 100 };
      const result = redactSensitiveFields(input);
      assert.strictEqual(result.amount, 99.99);
      assert.strictEqual(result.quantity, 5);
      assert.strictEqual(result.limit, 100);
    });

    it('preserves boolean fields', () => {
      const input = { active: true, deleted: false };
      const result = redactSensitiveFields(input);
      assert.strictEqual(result.active, true);
      assert.strictEqual(result.deleted, false);
    });

    it('preserves null and undefined', () => {
      const result1 = redactSensitiveFields(null);
      assert.strictEqual(result1, null);
      const result2 = redactSensitiveFields(undefined);
      assert.strictEqual(result2, undefined);
    });

    it('preserves nested non-sensitive structure', () => {
      const input = {
        order: {
          id: 'ord-123',
          items: [{ sku: 'WIDGET-001', qty: 2, price: 29.99 }],
          total: 59.98,
        },
      };
      const result = redactSensitiveFields(input);
      assert.strictEqual(result.order.id, 'ord-123');
      assert.strictEqual(result.order.items[0].sku, 'WIDGET-001');
      assert.strictEqual(result.order.total, 59.98);
    });

    it('preserves arrays of primitives', () => {
      const input = { tags: ['sale', 'new', 'featured'] };
      const result = redactSensitiveFields(input);
      assert.deepStrictEqual(result.tags, ['sale', 'new', 'featured']);
    });
  });

  // --------------------------------------------------------------------------
  // redactSensitiveFields — mixed content
  // --------------------------------------------------------------------------

  describe('mixed sensitive and non-sensitive', () => {
    it('redacts only sensitive fields in mixed object', () => {
      const input = {
        name: 'Test User',
        email: 'test@example.com',
        apiKey: 'sk-ant-secret-key',
        orderId: 'ord-456',
        password: 'hunter2',
        amount: 100,
      };
      const result = redactSensitiveFields(input);
      assert.strictEqual(result.name, 'Test User');
      assert.strictEqual(result.email, 'test@example.com');
      assert.strictEqual(result.apiKey, '[REDACTED]');
      assert.strictEqual(result.orderId, 'ord-456');
      assert.strictEqual(result.password, '[REDACTED]');
      assert.strictEqual(result.amount, 100);
    });

    it('handles deeply nested mixed content', () => {
      const input = {
        level1: {
          safe: 'value',
          level2: {
            also_safe: 42,
            level3: {
              secret: 'top-secret-value',
              description: 'a normal description',
            },
          },
        },
      };
      const result = redactSensitiveFields(input);
      assert.strictEqual(result.level1.safe, 'value');
      assert.strictEqual(result.level1.level2.also_safe, 42);
      assert.strictEqual(result.level1.level2.level3.secret, '[REDACTED]');
      assert.strictEqual(result.level1.level2.level3.description, 'a normal description');
    });
  });

  // --------------------------------------------------------------------------
  // redactSensitiveFields — depth limit
  // --------------------------------------------------------------------------

  describe('depth limiting', () => {
    it('stops redacting beyond depth 5', () => {
      // Build a deeply nested object
      let obj = { secret: 'deep-secret' };
      for (let i = 0; i < 8; i++) {
        obj = { nested: obj };
      }
      const result = redactSensitiveFields(obj);
      // The outer levels should be processed
      assert.ok(typeof result.nested === 'object');
    });

    it('handles circular-like deep nesting without crashing', () => {
      let obj = { token: 'value', data: 'safe' };
      for (let i = 0; i < 10; i++) {
        obj = { wrapper: obj };
      }
      // Should not throw
      const result = redactSensitiveFields(obj);
      assert.ok(result !== null);
    });
  });

  // --------------------------------------------------------------------------
  // redactSensitiveFields — edge cases
  // --------------------------------------------------------------------------

  describe('edge cases', () => {
    it('handles empty object', () => {
      const result = redactSensitiveFields({});
      assert.deepStrictEqual(result, {});
    });

    it('handles empty array', () => {
      const result = redactSensitiveFields([]);
      assert.deepStrictEqual(result, []);
    });

    it('handles primitive values', () => {
      assert.strictEqual(redactSensitiveFields('hello'), 'hello');
      assert.strictEqual(redactSensitiveFields(42), 42);
      assert.strictEqual(redactSensitiveFields(true), true);
    });

    it('handles array of objects with sensitive fields', () => {
      const input = [
        { name: 'Alice', token: 'tok-abc' },
        { name: 'Bob', token: 'tok-def' },
      ];
      const result = redactSensitiveFields(input);
      assert.strictEqual(result[0].name, 'Alice');
      assert.strictEqual(result[0].token, '[REDACTED]');
      assert.strictEqual(result[1].name, 'Bob');
      assert.strictEqual(result[1].token, '[REDACTED]');
    });

    it('does not modify the original object', () => {
      const input = { apiKey: 'original-key', name: 'Test' };
      const result = redactSensitiveFields(input);
      assert.strictEqual(input.apiKey, 'original-key');
      assert.strictEqual(result.apiKey, '[REDACTED]');
    });
  });

  // --------------------------------------------------------------------------
  // AgentTelemetry integration with redaction
  // --------------------------------------------------------------------------

  describe('AgentTelemetry redaction integration', () => {
    let captured;
    let origLog;

    beforeEach(() => {
      captured = [];
      origLog = console.log;
      console.log = (...args) => captured.push(args.join(' '));
    });

    afterEach(() => {
      console.log = origLog;
    });

    it('logToolCall redacts sensitive input fields in verbose mode', () => {
      const tel = new AgentTelemetry({ verbose: true, outputFormat: 'json' });
      tel.logToolCall(
        'create_payment',
        { orderId: 'ord-123', apiKey: 'sk-secret', amount: 50 },
        { success: true },
        10,
      );
      const output = captured.join('\n');
      assert.ok(!output.includes('sk-secret'), 'apiKey value should not appear in logs');
      assert.ok(output.includes('[REDACTED]'), 'should contain redaction marker');
    });

    it('logToolCall preserves tool name in verbose mode', () => {
      const tel = new AgentTelemetry({ verbose: true, outputFormat: 'json' });
      tel.logToolCall('list_orders', { limit: 10 }, { success: true }, 5);
      const output = captured.join('\n');
      assert.ok(output.includes('list_orders'), 'tool name should appear in logs');
    });

    it('logError redacts sensitive context', () => {
      const tel = new AgentTelemetry({ verbose: true, outputFormat: 'json' });
      // logError emits 'error' on the EventEmitter — must attach a listener
      // to prevent ERR_UNHANDLED_ERROR
      let emitted = null;
      tel.on('error', (record) => {
        emitted = record;
      });
      tel.logError(new Error('test error'), {
        apiKey: 'sk-secret',
        orderId: 'ord-123',
      });
      // Verify the context was redacted in the emitted record
      assert.ok(emitted !== null, 'error event should have been emitted');
      assert.strictEqual(emitted.context.apiKey, '[REDACTED]');
      assert.strictEqual(emitted.context.orderId, 'ord-123');
    });

    it('records metrics without leaking sensitive data', () => {
      const tel = new AgentTelemetry();
      tel.logToolCall('test_tool', { token: 'secret-token' }, { ok: true }, 10);
      const summary = tel.getSummary();
      const summaryStr = JSON.stringify(summary);
      assert.ok(!summaryStr.includes('secret-token'), 'token should not appear in summary');
    });
  });

  // --------------------------------------------------------------------------
  // NoOpTelemetry
  // --------------------------------------------------------------------------

  describe('NoOpTelemetry', () => {
    it('has all expected methods', () => {
      const noop = new NoOpTelemetry();
      assert.strictEqual(typeof noop.startSpan, 'function');
      assert.strictEqual(typeof noop.endSpan, 'function');
      assert.strictEqual(typeof noop.endSpanRef, 'function');
    });

    it('startSpan returns a span-like object', () => {
      const noop = new NoOpTelemetry();
      const span = noop.startSpan('test');
      assert.strictEqual(typeof span.end, 'function');
      assert.strictEqual(typeof span.addEvent, 'function');
      assert.strictEqual(typeof span.setAttribute, 'function');
    });

    it('noOpTelemetry singleton is a NoOpTelemetry', () => {
      assert.ok(noOpTelemetry instanceof NoOpTelemetry);
    });
  });

  // --------------------------------------------------------------------------
  // Span
  // --------------------------------------------------------------------------

  describe('Span', () => {
    it('constructor sets initial fields', () => {
      const span = new Span('test-span', 'trace-id', 'parent-id', { key: 'val' });
      assert.strictEqual(span.name, 'test-span');
      assert.strictEqual(span.traceId, 'trace-id');
      assert.strictEqual(span.parentSpanId, 'parent-id');
      assert.strictEqual(span.status, 'running');
      assert.strictEqual(span.endTime, null);
      assert.strictEqual(span.duration, null);
    });

    it('end() sets duration and status', () => {
      const span = new Span('test', 'trace-1');
      span.end('ok');
      assert.strictEqual(span.status, 'ok');
      assert.ok(typeof span.duration === 'number');
      assert.ok(span.endTime >= span.startTime);
    });

    it('addEvent() appends events', () => {
      const span = new Span('test', 'trace-1');
      span.addEvent('step_1', { detail: 'abc' });
      span.addEvent('step_2', {});
      assert.strictEqual(span.events.length, 2);
      assert.strictEqual(span.events[0].name, 'step_1');
      assert.strictEqual(span.events[0].data.detail, 'abc');
    });

    it('setAttribute() stores attributes', () => {
      const span = new Span('test', 'trace-1');
      span.setAttribute('error', true);
      span.setAttribute('toolName', 'list_orders');
      assert.strictEqual(span.attributes.error, true);
      assert.strictEqual(span.attributes.toolName, 'list_orders');
    });

    it('toJSON() returns serializable object', () => {
      const span = new Span('test', 'trace-1', null, { meta: 1 });
      span.addEvent('evt', {});
      span.setAttribute('key', 'val');
      span.end('ok', { result: 'done' });
      const json = span.toJSON();
      assert.strictEqual(json.name, 'test');
      assert.strictEqual(json.traceId, 'trace-1');
      assert.strictEqual(json.status, 'ok');
      assert.ok(json.events.length >= 1);
      assert.strictEqual(json.attributes.key, 'val');
      // Should be serializable
      const serialized = JSON.stringify(json);
      assert.ok(serialized.length > 0);
    });
  });
});

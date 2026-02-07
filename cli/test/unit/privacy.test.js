/**
 * Unit tests for privacy.js
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import { redactSensitive, redactObject } from '../../src/privacy.js';

// ===========================================================================
// redactSensitive
// ===========================================================================

describe('redactSensitive', () => {
  it('returns null/undefined/empty unchanged', () => {
    assert.strictEqual(redactSensitive(null), null);
    assert.strictEqual(redactSensitive(undefined), undefined);
    assert.strictEqual(redactSensitive(''), '');
  });

  it('returns non-string input unchanged', () => {
    assert.strictEqual(redactSensitive(42), 42);
  });

  it('redacts email addresses', () => {
    const input = 'Contact alice@example.com for details';
    const result = redactSensitive(input);
    assert.ok(!result.includes('alice@example.com'));
    assert.ok(result.includes('[email]'));
  });

  it('redacts multiple email addresses', () => {
    const input = 'alice@example.com and bob@test.org';
    const result = redactSensitive(input);
    assert.ok(!result.includes('alice@'));
    assert.ok(!result.includes('bob@'));
    assert.strictEqual((result.match(/\[email\]/g) || []).length, 2);
  });

  it('redacts phone numbers', () => {
    const input = 'Call me at 555-123-4567';
    const result = redactSensitive(input);
    assert.ok(!result.includes('555-123-4567'));
    assert.ok(result.includes('[phone]'));
  });

  it('redacts phone numbers with country code', () => {
    const input = 'International: +1-555-123-4567';
    const result = redactSensitive(input);
    assert.ok(!result.includes('555-123-4567'));
    assert.ok(result.includes('[phone]'));
  });

  it('redacts API keys (sk- pattern)', () => {
    const input = 'Use key sk-ant1234567890abcdef';
    const result = redactSensitive(input);
    assert.ok(!result.includes('sk-ant1234567890abcdef'));
    assert.ok(result.includes('[api_key]'));
  });

  it('redacts Slack tokens', () => {
    // Use letters-only token to avoid phone regex matching digits first
    const input = 'Token is xoxb-abcdefghijklmnop';
    const result = redactSensitive(input);
    assert.ok(!result.includes('xoxb-'), `Expected xoxb- to be redacted, got: ${result}`);
    assert.ok(result.includes('[token]'));
  });

  it('redacts GitHub tokens', () => {
    const input = 'ghp_1234567890abcdefghijklmn';
    const result = redactSensitive(input);
    assert.ok(!result.includes('ghp_'));
    assert.ok(result.includes('[token]'));
  });

  it('does not modify text without sensitive data', () => {
    const input = 'Hello world, order ORD-123 is ready.';
    assert.strictEqual(redactSensitive(input), input);
  });

  it('respects enabled=false option', () => {
    const input = 'Contact alice@example.com';
    const result = redactSensitive(input, { enabled: false });
    assert.strictEqual(result, input);
  });

  it('supports custom patterns', () => {
    const input = 'SSN: 123-45-6789';
    const customPatterns = [{ name: 'ssn', re: /\b\d{3}-\d{2}-\d{4}\b/g, replace: '[ssn]' }];
    const result = redactSensitive(input, { patterns: customPatterns });
    assert.ok(!result.includes('123-45-6789'));
    assert.ok(result.includes('[ssn]'));
  });
});

// ===========================================================================
// redactObject
// ===========================================================================

describe('redactObject', () => {
  it('returns null/undefined unchanged', () => {
    assert.strictEqual(redactObject(null), null);
    assert.strictEqual(redactObject(undefined), undefined);
  });

  it('redacts strings directly', () => {
    const result = redactObject('Contact alice@example.com');
    assert.ok(result.includes('[email]'));
  });

  it('redacts values within objects', () => {
    const input = { email: 'alice@example.com', name: 'Alice' };
    const result = redactObject(input);
    assert.ok(typeof result === 'object');
    assert.ok(!JSON.stringify(result).includes('alice@example.com'));
  });

  it('redacts values in nested objects', () => {
    const input = {
      user: {
        contact: { email: 'bob@test.org', phone: '555-123-4567' },
      },
    };
    const result = redactObject(input);
    const json = JSON.stringify(result);
    assert.ok(!json.includes('bob@test.org'));
    assert.ok(!json.includes('555-123-4567'));
  });

  it('redacts values in arrays', () => {
    const input = ['alice@example.com', 'no-sensitive-data', 'bob@test.org'];
    const result = redactObject(input);
    assert.ok(Array.isArray(result));
    assert.ok(!JSON.stringify(result).includes('alice@'));
  });

  it('returns original value on non-serializable input', () => {
    const circular = {};
    circular.self = circular;
    // Should not throw, returns original
    const result = redactObject(circular);
    assert.strictEqual(result, circular);
  });

  it('respects enabled=false option', () => {
    const input = { key: 'alice@example.com' };
    const result = redactObject(input, { enabled: false });
    assert.strictEqual(JSON.stringify(result), JSON.stringify(input));
  });
});

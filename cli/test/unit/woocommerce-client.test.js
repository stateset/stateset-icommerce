import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  WooCommerceClient,
  WooCommerceApiError,
  validateUrl,
  buildBasicAuth,
  RateLimiter,
} from '../../src/adapters/woocommerce/client.js';

// ---------------------------------------------------------------------------
// validateUrl — SSRF prevention
// ---------------------------------------------------------------------------

describe('woocommerce client — validateUrl', () => {
  it('accepts valid HTTPS URLs', () => {
    assert.deepEqual(validateUrl('https://mystore.example.com'), { valid: true });
    assert.deepEqual(validateUrl('https://shop.mysite.io'), { valid: true });
  });

  it('accepts valid HTTP URLs', () => {
    assert.deepEqual(validateUrl('http://mystore.example.com'), { valid: true });
  });

  it('blocks localhost', () => {
    const result = validateUrl('https://localhost/wp-json');
    assert.equal(result.valid, false);
    assert.match(result.error, /private/i);
  });

  it('blocks 127.0.0.1', () => {
    const result = validateUrl('https://127.0.0.1');
    assert.equal(result.valid, false);
  });

  it('blocks 0.0.0.0', () => {
    const result = validateUrl('https://0.0.0.0');
    assert.equal(result.valid, false);
  });

  it('blocks ::1', () => {
    const result = validateUrl('https://[::1]');
    assert.equal(result.valid, false);
  });

  it('blocks 10.x private IPs', () => {
    const result = validateUrl('https://10.0.0.1');
    assert.equal(result.valid, false);
    assert.match(result.error, /private/i);
  });

  it('blocks 172.16-31.x private IPs', () => {
    assert.equal(validateUrl('https://172.16.0.1').valid, false);
    assert.equal(validateUrl('https://172.31.255.255').valid, false);
  });

  it('allows 172.15.x (not private)', () => {
    assert.equal(validateUrl('https://172.15.0.1').valid, true);
  });

  it('allows 172.32.x (not private)', () => {
    assert.equal(validateUrl('https://172.32.0.1').valid, true);
  });

  it('blocks 192.168.x private IPs', () => {
    const result = validateUrl('https://192.168.1.1');
    assert.equal(result.valid, false);
    assert.match(result.error, /private/i);
  });

  it('blocks .local hostnames', () => {
    const result = validateUrl('https://mysite.local');
    assert.equal(result.valid, false);
    assert.match(result.error, /internal/i);
  });

  it('blocks .internal hostnames', () => {
    const result = validateUrl('https://api.internal');
    assert.equal(result.valid, false);
    assert.match(result.error, /internal/i);
  });

  it('rejects null/undefined/empty', () => {
    assert.equal(validateUrl(null).valid, false);
    assert.equal(validateUrl(undefined).valid, false);
    assert.equal(validateUrl('').valid, false);
  });

  it('rejects invalid URLs', () => {
    const result = validateUrl('not-a-url');
    assert.equal(result.valid, false);
    assert.match(result.error, /Invalid URL/i);
  });
});

// ---------------------------------------------------------------------------
// buildBasicAuth
// ---------------------------------------------------------------------------

describe('woocommerce client — buildBasicAuth', () => {
  it('encodes credentials as base64', () => {
    const result = buildBasicAuth('ck_test_key', 'cs_test_secret');
    const expected = `Basic ${Buffer.from('ck_test_key:cs_test_secret').toString('base64')}`;
    assert.equal(result, expected);
  });

  it('starts with "Basic "', () => {
    const result = buildBasicAuth('key', 'secret');
    assert.ok(result.startsWith('Basic '));
  });

  it('can be decoded back', () => {
    const result = buildBasicAuth('ck_abc', 'cs_xyz');
    const b64 = result.replace('Basic ', '');
    const decoded = Buffer.from(b64, 'base64').toString('utf-8');
    assert.equal(decoded, 'ck_abc:cs_xyz');
  });
});

// ---------------------------------------------------------------------------
// RateLimiter
// ---------------------------------------------------------------------------

describe('woocommerce client — RateLimiter', () => {
  it('creates with default 5 req/sec', () => {
    const rl = new RateLimiter();
    assert.equal(rl.minInterval, 200);
  });

  it('creates with custom rate', () => {
    const rl = new RateLimiter(10);
    assert.equal(rl.minInterval, 100);
  });

  it('first call resolves immediately', async () => {
    const rl = new RateLimiter(100);
    const start = Date.now();
    await rl.wait();
    const elapsed = Date.now() - start;
    assert.ok(elapsed < 50, `Expected <50ms, got ${elapsed}ms`);
  });
});

// ---------------------------------------------------------------------------
// WooCommerceClient constructor
// ---------------------------------------------------------------------------

describe('woocommerce client — constructor', () => {
  it('creates client with valid config', () => {
    const client = new WooCommerceClient({
      siteUrl: 'https://mystore.example.com',
      consumerKey: 'ck_test',
      consumerSecret: 'cs_test',
    });
    assert.equal(client.baseUrl, 'https://mystore.example.com/wp-json/wc/v3');
  });

  it('uses custom API version', () => {
    const client = new WooCommerceClient({
      siteUrl: 'https://mystore.example.com',
      consumerKey: 'ck_test',
      consumerSecret: 'cs_test',
      apiVersion: 'wc/v2',
    });
    assert.equal(client.baseUrl, 'https://mystore.example.com/wp-json/wc/v2');
  });

  it('strips trailing slash from siteUrl', () => {
    const client = new WooCommerceClient({
      siteUrl: 'https://mystore.example.com/',
      consumerKey: 'ck_test',
      consumerSecret: 'cs_test',
    });
    assert.equal(client.baseUrl, 'https://mystore.example.com/wp-json/wc/v3');
  });

  it('sets Authorization header', () => {
    const client = new WooCommerceClient({
      siteUrl: 'https://mystore.example.com',
      consumerKey: 'ck_test',
      consumerSecret: 'cs_test',
    });
    assert.ok(client.headers.Authorization.startsWith('Basic '));
  });

  it('throws on missing siteUrl', () => {
    assert.throws(
      () => new WooCommerceClient({ consumerKey: 'ck', consumerSecret: 'cs' }),
      /siteUrl is required/,
    );
  });

  it('throws on missing consumerKey', () => {
    assert.throws(
      () =>
        new WooCommerceClient({
          siteUrl: 'https://example.com',
          consumerSecret: 'cs',
        }),
      /consumerKey is required/,
    );
  });

  it('throws on missing consumerSecret', () => {
    assert.throws(
      () =>
        new WooCommerceClient({
          siteUrl: 'https://example.com',
          consumerKey: 'ck',
        }),
      /consumerSecret is required/,
    );
  });

  it('throws on SSRF-blocked URL (localhost)', () => {
    assert.throws(
      () =>
        new WooCommerceClient({
          siteUrl: 'https://localhost',
          consumerKey: 'ck',
          consumerSecret: 'cs',
        }),
      /Invalid site URL/,
    );
  });

  it('throws on SSRF-blocked URL (private IP)', () => {
    assert.throws(
      () =>
        new WooCommerceClient({
          siteUrl: 'https://192.168.1.1',
          consumerKey: 'ck',
          consumerSecret: 'cs',
        }),
      /Invalid site URL/,
    );
  });

  it('throws on SSRF-blocked URL (.internal)', () => {
    assert.throws(
      () =>
        new WooCommerceClient({
          siteUrl: 'https://api.internal',
          consumerKey: 'ck',
          consumerSecret: 'cs',
        }),
      /Invalid site URL/,
    );
  });
});

// ---------------------------------------------------------------------------
// WooCommerceApiError
// ---------------------------------------------------------------------------

describe('woocommerce client — WooCommerceApiError', () => {
  it('stores status, statusText, and body', () => {
    const err = new WooCommerceApiError(404, 'Not Found', '{"code":"not_found"}');
    assert.equal(err.status, 404);
    assert.equal(err.statusText, 'Not Found');
    assert.equal(err.body, '{"code":"not_found"}');
    assert.equal(err.name, 'WooCommerceApiError');
    assert.ok(err.message.includes('404'));
  });

  it('extends Error', () => {
    const err = new WooCommerceApiError(500, 'Internal Server Error');
    assert.ok(err instanceof Error);
  });

  it('defaults body to empty string', () => {
    const err = new WooCommerceApiError(400, 'Bad Request');
    assert.equal(err.body, '');
  });
});

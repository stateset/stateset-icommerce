import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  ShopifyClient,
  ShopifyApiError,
  isValidShopifyDomain,
  parseLinkHeader,
  RateLimiter,
} from '../../src/adapters/shopify/client.js';

describe('authenticated request boundaries', () => {
  const config = {
    shopDomain: 'store.myshopify.com',
    accessToken: 'operator-token',
    requestsPerSecond: 10000,
  };

  it('rejects other shops, origins, versions and credentials before sending a token', async (t) => {
    let calls = 0;
    t.mock.method(globalThis, 'fetch', async () => {
      calls++;
      throw new Error('unexpected network request');
    });
    const client = new ShopifyClient(config);
    for (const url of [
      'https://attacker.example/admin/api/2024-01/customers.json',
      'https://other.myshopify.com/admin/api/2024-01/customers.json',
      'http://store.myshopify.com/admin/api/2024-01/customers.json',
      'https://store.myshopify.com/admin/api/2025-01/customers.json',
      'https://user:pass@store.myshopify.com/admin/api/2024-01/customers.json',
      '/../../customers.json',
    ])
      await assert.rejects(client.get(url), /configured shop/);
    assert.equal(calls, 0);
  });

  it('blocks foreign pagination links after receiving the first page', async (t) => {
    const calls = [];
    t.mock.method(globalThis, 'fetch', async (url, options) => {
      calls.push({ url, options });
      return new Response(JSON.stringify({ customers: [{ id: '1' }] }), {
        headers: { link: '<https://attacker.example/steal>; rel="next"' },
      });
    });
    const client = new ShopifyClient(config);
    const pages = client.getCustomers();
    assert.deepEqual((await pages.next()).value, [{ id: '1' }]);
    await assert.rejects(pages.next(), /configured shop/);
    assert.equal(calls.length, 1);
    assert.equal(calls[0].options.redirect, 'error');
    assert.ok(calls[0].options.signal instanceof AbortSignal);
  });

  it('stops a repeated cursor instead of looping forever', async (t) => {
    let calls = 0;
    t.mock.method(globalThis, 'fetch', async () => {
      calls++;
      return new Response(JSON.stringify({ customers: [{ id: String(calls) }] }), {
        headers: {
          link: '<https://store.myshopify.com/admin/api/2024-01/customers.json?page_info=repeated>; rel="next"',
        },
      });
    });
    const client = new ShopifyClient(config);
    await assert.rejects(async () => {
      for await (const _page of client.getCustomers()) {
        /* consume */
      }
    }, /cursor repeated/);
    assert.equal(calls, 2);
  });

  it('refuses configuration that changes the API path or cannot rate limit', () => {
    assert.throws(() => new ShopifyClient({ ...config, apiVersion: '../other' }), /apiVersion/);
    for (const rate of [0, -1, Infinity, NaN])
      assert.throws(
        () => new ShopifyClient({ ...config, requestsPerSecond: rate }),
        /requestsPerSecond/,
      );
  });
});

// ---------------------------------------------------------------------------
// isValidShopifyDomain
// ---------------------------------------------------------------------------

describe('isValidShopifyDomain', () => {
  it('accepts valid .myshopify.com domains', () => {
    assert.equal(isValidShopifyDomain('my-store.myshopify.com'), true);
    assert.equal(isValidShopifyDomain('test123.myshopify.com'), true);
  });

  it('rejects non-myshopify domains', () => {
    assert.equal(isValidShopifyDomain('evil.com'), false);
    assert.equal(isValidShopifyDomain('shopify.com'), false);
  });

  it('rejects localhost and private IPs', () => {
    assert.equal(isValidShopifyDomain('localhost'), false);
    assert.equal(isValidShopifyDomain('127.0.0.1'), false);
  });

  it('rejects .internal and .local domains', () => {
    assert.equal(isValidShopifyDomain('store.internal'), false);
    assert.equal(isValidShopifyDomain('store.local'), false);
  });

  it('rejects null/undefined/empty', () => {
    assert.equal(isValidShopifyDomain(null), false);
    assert.equal(isValidShopifyDomain(undefined), false);
    assert.equal(isValidShopifyDomain(''), false);
  });

  it('rejects domains with path traversal', () => {
    assert.equal(isValidShopifyDomain('store.myshopify.com/admin'), false);
  });
});

// ---------------------------------------------------------------------------
// parseLinkHeader
// ---------------------------------------------------------------------------

describe('parseLinkHeader', () => {
  it('parses next link', () => {
    const header =
      '<https://store.myshopify.com/admin/api/2024-01/customers.json?page_info=abc>; rel="next"';
    const result = parseLinkHeader(header);
    assert.equal(
      result.next,
      'https://store.myshopify.com/admin/api/2024-01/customers.json?page_info=abc',
    );
    assert.equal(result.previous, null);
  });

  it('parses both next and previous', () => {
    const header =
      '<https://store.myshopify.com?page_info=abc>; rel="previous", <https://store.myshopify.com?page_info=def>; rel="next"';
    const result = parseLinkHeader(header);
    assert.ok(result.next);
    assert.ok(result.previous);
  });

  it('returns nulls for null header', () => {
    const result = parseLinkHeader(null);
    assert.equal(result.next, null);
    assert.equal(result.previous, null);
  });

  it('returns nulls for empty header', () => {
    const result = parseLinkHeader('');
    assert.equal(result.next, null);
    assert.equal(result.previous, null);
  });
});

// ---------------------------------------------------------------------------
// RateLimiter
// ---------------------------------------------------------------------------

describe('RateLimiter', () => {
  it('creates with default 2 req/sec', () => {
    const rl = new RateLimiter();
    assert.equal(rl.minInterval, 500);
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
// ShopifyClient constructor
// ---------------------------------------------------------------------------

describe('ShopifyClient constructor', () => {
  it('creates client with valid config', () => {
    const client = new ShopifyClient({
      shopDomain: 'test-store.myshopify.com',
      accessToken: 'shpat_test',
    });
    assert.equal(client.baseUrl, 'https://test-store.myshopify.com/admin/api/2024-01');
    assert.equal(client.headers['X-Shopify-Access-Token'], 'shpat_test');
  });

  it('uses custom API version', () => {
    const client = new ShopifyClient({
      shopDomain: 'test-store.myshopify.com',
      accessToken: 'shpat_test',
      apiVersion: '2025-01',
    });
    assert.equal(client.baseUrl, 'https://test-store.myshopify.com/admin/api/2025-01');
  });

  it('throws on missing shopDomain', () => {
    assert.throws(() => new ShopifyClient({ accessToken: 'shpat_test' }), /shopDomain is required/);
  });

  it('throws on missing accessToken', () => {
    assert.throws(
      () => new ShopifyClient({ shopDomain: 'test.myshopify.com' }),
      /accessToken is required/,
    );
  });

  it('throws on invalid domain', () => {
    assert.throws(
      () => new ShopifyClient({ shopDomain: 'evil.com', accessToken: 'shpat_test' }),
      /Invalid Shopify domain/,
    );
  });

  it('throws on localhost domain', () => {
    assert.throws(
      () => new ShopifyClient({ shopDomain: 'localhost', accessToken: 'shpat_test' }),
      /Invalid Shopify domain/,
    );
  });
});

// ---------------------------------------------------------------------------
// ShopifyApiError
// ---------------------------------------------------------------------------

describe('ShopifyApiError', () => {
  it('stores status and body', () => {
    const err = new ShopifyApiError(404, 'Not Found', '{"error":"not found"}');
    assert.equal(err.status, 404);
    assert.equal(err.statusText, 'Not Found');
    assert.equal(err.body, '{"error":"not found"}');
    assert.equal(err.name, 'ShopifyApiError');
    assert.ok(err.message.includes('404'));
  });

  it('extends Error', () => {
    const err = new ShopifyApiError(500, 'Internal Server Error');
    assert.ok(err instanceof Error);
  });
});

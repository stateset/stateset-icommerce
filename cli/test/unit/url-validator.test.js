/**
 * Unit tests for utils/url-validator.js
 *
 * Tests SSRF prevention (validateFetchUrl) and display-safe URL
 * checking (isSafeDisplayUrl).
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  fetchWithValidatedRedirects,
  validateFetchUrl,
  validateResolvedFetchUrl,
  createPinnedLookup,
  isBlockedIpAddress,
  isSafeDisplayUrl,
} from '../../src/utils/url-validator.js';

// ===========================================================================
// validateFetchUrl — allowed URLs
// ===========================================================================

describe('validateFetchUrl — allowed URLs', () => {
  it('allows a valid public HTTPS URL', () => {
    assert.doesNotThrow(() => validateFetchUrl('https://example.com/api'));
  });

  it('allows a valid public HTTP URL', () => {
    assert.doesNotThrow(() => validateFetchUrl('http://example.com/path'));
  });

  it('allows HTTPS URLs with ports', () => {
    assert.doesNotThrow(() => validateFetchUrl('https://api.example.com:8443/v1'));
  });

  it('allows HTTPS URLs with query strings', () => {
    assert.doesNotThrow(() => validateFetchUrl('https://example.com/search?q=hello&page=1'));
  });

  it('allows HTTPS URLs with fragments', () => {
    assert.doesNotThrow(() => validateFetchUrl('https://example.com/docs#section'));
  });

  it('allows public IP addresses', () => {
    assert.doesNotThrow(() => validateFetchUrl('https://8.8.8.8/dns'));
  });
});

// ===========================================================================
// validateFetchUrl — blocked: localhost / loopback
// ===========================================================================

describe('validateFetchUrl — blocks localhost', () => {
  it('blocks 127.0.0.1', () => {
    assert.throws(() => validateFetchUrl('http://127.0.0.1/admin'), {
      message: /SSRF blocked/,
    });
  });

  it('blocks localhost', () => {
    assert.throws(() => validateFetchUrl('http://localhost/admin'), {
      message: /SSRF blocked/,
    });
  });

  it('blocks ::1 (IPv6 loopback)', () => {
    assert.throws(() => validateFetchUrl('http://[::1]/admin'), {
      message: /SSRF blocked/,
    });
  });

  it('blocks 0.0.0.0', () => {
    assert.throws(() => validateFetchUrl('http://0.0.0.0/'), {
      message: /SSRF blocked/,
    });
  });

  it('blocks localhost with port', () => {
    assert.throws(() => validateFetchUrl('http://localhost:3000/api'), {
      message: /SSRF blocked/,
    });
  });

  it('blocks 127.0.0.1 with port', () => {
    assert.throws(() => validateFetchUrl('http://127.0.0.1:8080/'), {
      message: /SSRF blocked/,
    });
  });
});

// ===========================================================================
// validateFetchUrl — blocked: private IP ranges
// ===========================================================================

describe('validateFetchUrl — blocks private IPs', () => {
  it('blocks 10.0.0.1 (10.x.x.x range)', () => {
    assert.throws(() => validateFetchUrl('http://10.0.0.1/internal'), {
      message: /SSRF blocked/,
    });
  });

  it('blocks 10.255.255.255 (10.x upper bound)', () => {
    assert.throws(() => validateFetchUrl('http://10.255.255.255/'), {
      message: /SSRF blocked/,
    });
  });

  it('blocks 192.168.0.1 (192.168.x.x range)', () => {
    assert.throws(() => validateFetchUrl('http://192.168.0.1/'), {
      message: /SSRF blocked/,
    });
  });

  it('blocks 192.168.100.50', () => {
    assert.throws(() => validateFetchUrl('http://192.168.100.50/api'), {
      message: /SSRF blocked/,
    });
  });

  it('blocks 172.16.0.1 (172.16.x.x range start)', () => {
    assert.throws(() => validateFetchUrl('http://172.16.0.1/'), {
      message: /SSRF blocked/,
    });
  });

  it('blocks 172.31.255.255 (172.31.x.x range end)', () => {
    assert.throws(() => validateFetchUrl('http://172.31.255.255/'), {
      message: /SSRF blocked/,
    });
  });

  it('blocks 172.20.10.5 (mid-range 172.16-31)', () => {
    assert.throws(() => validateFetchUrl('http://172.20.10.5/'), {
      message: /SSRF blocked/,
    });
  });

  it('blocks link-local metadata addresses', () => {
    assert.throws(() => validateFetchUrl('http://169.254.169.254/latest/meta-data'), {
      message: /SSRF blocked/,
    });
  });

  it('blocks IPv6 unique-local addresses', () => {
    assert.throws(() => validateFetchUrl('http://[fd00::1]/'), {
      message: /SSRF blocked/,
    });
  });

  it('blocks IPv6 link-local addresses', () => {
    assert.throws(() => validateFetchUrl('http://[fe80::1]/'), {
      message: /SSRF blocked/,
    });
  });

  it('allows 172.32.0.1 (outside private range)', () => {
    assert.doesNotThrow(() => validateFetchUrl('http://172.32.0.1/'));
  });

  it('allows 172.15.0.1 (outside private range)', () => {
    assert.doesNotThrow(() => validateFetchUrl('http://172.15.0.1/'));
  });
});

// ===========================================================================
// validateResolvedFetchUrl — DNS rebinding protection
// ===========================================================================

describe('validateResolvedFetchUrl', () => {
  it('allows hostnames resolving only to public addresses', async () => {
    await assert.doesNotReject(() =>
      validateResolvedFetchUrl('https://merchant.test/pay', {
        cache: false,
        lookup: async () => [{ address: '8.8.8.8', family: 4 }],
      }),
    );
  });

  it('blocks hostnames resolving to private IPv4 addresses', async () => {
    await assert.rejects(
      () =>
        validateResolvedFetchUrl('https://merchant.test/pay', {
          cache: false,
          lookup: async () => [{ address: '10.0.0.5', family: 4 }],
        }),
      /resolves to internal address/,
    );
  });

  it('blocks hostnames resolving to private IPv6 addresses', async () => {
    await assert.rejects(
      () =>
        validateResolvedFetchUrl('https://merchant.test/pay', {
          cache: false,
          lookup: async () => [{ address: 'fd00::5', family: 6 }],
        }),
      /resolves to internal address/,
    );
  });

  it('reports DNS resolution failures before fetch', async () => {
    await assert.rejects(
      () =>
        validateResolvedFetchUrl('https://merchant.test/pay', {
          cache: false,
          lookup: async () => {
            throw new Error('no such host');
          },
        }),
      /Unable to resolve URL host/,
    );
  });

  it('returns the validated addresses so callers can pin the connection', async () => {
    const result = await validateResolvedFetchUrl('https://merchant.test/pay', {
      cache: false,
      lookup: async () => [
        { address: '8.8.8.8', family: 4 },
        { address: '2606:4700:4700::1111', family: 6 },
      ],
    });
    assert.ok(result);
    assert.equal(result.host, 'merchant.test');
    assert.deepEqual(result.addresses, [
      { address: '8.8.8.8', family: 4 },
      { address: '2606:4700:4700::1111', family: 6 },
    ]);
  });

  it('returns null for IP-literal hosts (nothing to pin)', async () => {
    const result = await validateResolvedFetchUrl('https://8.8.8.8/pay', { cache: false });
    assert.equal(result, null);
  });

  it('returns null for documentation hosts (nothing to pin)', async () => {
    const result = await validateResolvedFetchUrl('https://example.com/pay', { cache: false });
    assert.equal(result, null);
  });
});

// ===========================================================================
// createPinnedLookup — connection pinning closes the rebinding TOCTOU window
// ===========================================================================

describe('createPinnedLookup', () => {
  function callLookup(lookup, hostname, options) {
    return new Promise((resolve, reject) => {
      lookup(hostname, options, (err, address, family) => {
        if (err) reject(err);
        else resolve({ address, family });
      });
    });
  }

  it('resolves the pinned host to a validated address', async () => {
    const lookup = createPinnedLookup('merchant.test', [{ address: '8.8.8.8', family: 4 }]);
    const result = await callLookup(lookup, 'merchant.test', {});
    assert.deepEqual(result, { address: '8.8.8.8', family: 4 });
  });

  it('returns all validated addresses when options.all is set', async () => {
    const lookup = createPinnedLookup('merchant.test', [
      { address: '8.8.8.8', family: 4 },
      { address: '1.1.1.1', family: 4 },
    ]);
    const result = await callLookup(lookup, 'merchant.test', { all: true });
    assert.deepEqual(result.address, [
      { address: '8.8.8.8', family: 4 },
      { address: '1.1.1.1', family: 4 },
    ]);
  });

  it('honors a requested address family when available', async () => {
    const lookup = createPinnedLookup('merchant.test', [
      { address: '8.8.8.8', family: 4 },
      { address: '2606:4700:4700::1111', family: 6 },
    ]);
    const v6 = await callLookup(lookup, 'merchant.test', { family: 6 });
    assert.equal(v6.address, '2606:4700:4700::1111');
    assert.equal(v6.family, 6);
  });

  it('refuses a connect attempt to any host other than the pinned one', async () => {
    const lookup = createPinnedLookup('merchant.test', [{ address: '8.8.8.8', family: 4 }]);
    await assert.rejects(
      () => callLookup(lookup, 'evil.test', {}),
      /unexpected connect host evil\.test/,
    );
  });

  it('drops addresses that became blocked at connect time (rebinding defense)', async () => {
    // Simulate a validated set that, at connect time, re-checks as internal.
    const lookup = createPinnedLookup('merchant.test', [{ address: '169.254.169.254', family: 4 }]);
    await assert.rejects(
      () => callLookup(lookup, 'merchant.test', {}),
      /no validated public address/,
    );
  });

  it('matches the pinned host case-insensitively', async () => {
    const lookup = createPinnedLookup('merchant.test', [{ address: '8.8.8.8', family: 4 }]);
    const result = await callLookup(lookup, 'Merchant.TEST', {});
    assert.equal(result.address, '8.8.8.8');
  });
});

describe('fetchWithValidatedRedirects', () => {
  it('follows public redirects with redirect validation enabled', async () => {
    const calls = [];
    const fetch = async (url, options) => {
      calls.push({ url: String(url), redirect: options.redirect });
      if (String(url) === 'https://merchant.test/start') {
        return new Response('', {
          status: 302,
          headers: { location: 'https://checkout.test/final' },
        });
      }
      return new Response('ok', { status: 200 });
    };

    const response = await fetchWithValidatedRedirects(
      'https://merchant.test/start',
      {},
      {
        cache: false,
        fetch,
        lookup: async () => [{ address: '8.8.8.8', family: 4 }],
      },
    );

    assert.equal(response.status, 200);
    assert.deepEqual(calls, [
      { url: 'https://merchant.test/start', redirect: 'manual' },
      { url: 'https://checkout.test/final', redirect: 'manual' },
    ]);
  });

  it('blocks redirects to internal addresses before following them', async () => {
    const fetch = async () =>
      new Response('', {
        status: 302,
        headers: { location: 'http://169.254.169.254/latest/meta-data' },
      });

    await assert.rejects(
      () =>
        fetchWithValidatedRedirects(
          'https://merchant.test/start',
          {},
          {
            cache: false,
            fetch,
            lookup: async () => [{ address: '8.8.8.8', family: 4 }],
          },
        ),
      /SSRF blocked/,
    );
  });
});

describe('isBlockedIpAddress', () => {
  it('identifies non-public IPv4 ranges', () => {
    assert.equal(isBlockedIpAddress('127.0.0.1'), true);
    assert.equal(isBlockedIpAddress('169.254.169.254'), true);
    assert.equal(isBlockedIpAddress('8.8.8.8'), false);
  });

  it('identifies non-public IPv6 ranges', () => {
    assert.equal(isBlockedIpAddress('::1'), true);
    assert.equal(isBlockedIpAddress('fd00::1'), true);
    assert.equal(isBlockedIpAddress('2606:4700:4700::1111'), false);
  });
});

// ===========================================================================
// validateFetchUrl — blocked: internal/local hostnames
// ===========================================================================

describe('validateFetchUrl — blocks internal hostnames', () => {
  it('blocks .internal domains', () => {
    assert.throws(() => validateFetchUrl('http://api.internal/v1'), {
      message: /SSRF blocked/,
    });
  });

  it('blocks .local domains', () => {
    assert.throws(() => validateFetchUrl('http://printer.local/status'), {
      message: /SSRF blocked/,
    });
  });

  it('blocks deeply nested .internal domains', () => {
    assert.throws(() => validateFetchUrl('https://service.cluster.internal/health'), {
      message: /SSRF blocked/,
    });
  });

  it('blocks deeply nested .local domains', () => {
    assert.throws(() => validateFetchUrl('http://myhost.office.local/api'), {
      message: /SSRF blocked/,
    });
  });
});

// ===========================================================================
// validateFetchUrl — blocked: non-HTTP protocols
// ===========================================================================

describe('validateFetchUrl — blocks non-HTTP protocols', () => {
  it('blocks ftp:// protocol', () => {
    assert.throws(() => validateFetchUrl('ftp://files.example.com/data'), {
      message: /Unsupported protocol/,
    });
  });

  it('blocks file:// protocol', () => {
    assert.throws(() => validateFetchUrl('file:///etc/passwd'), {
      message: /Unsupported protocol/,
    });
  });

  it('blocks data: protocol', () => {
    assert.throws(() => validateFetchUrl('data:text/html,<h1>Hello</h1>'), {
      message: /Unsupported protocol/,
    });
  });

  it('blocks javascript: protocol', () => {
    assert.throws(
      () => validateFetchUrl('javascript:alert(1)'),
      // javascript: URL may fail at URL parse level or protocol check
    );
  });
});

// ===========================================================================
// validateFetchUrl — invalid URLs
// ===========================================================================

describe('validateFetchUrl — invalid URLs', () => {
  it('throws on empty string', () => {
    assert.throws(() => validateFetchUrl(''));
  });

  it('throws on garbage input', () => {
    assert.throws(() => validateFetchUrl('not-a-url'));
  });

  it('throws on missing protocol', () => {
    assert.throws(() => validateFetchUrl('example.com/path'));
  });

  it('throws on undefined', () => {
    assert.throws(() => validateFetchUrl(undefined));
  });

  it('throws on null', () => {
    assert.throws(() => validateFetchUrl(null));
  });
});

// ===========================================================================
// isSafeDisplayUrl — allowed
// ===========================================================================

describe('isSafeDisplayUrl — allowed URLs', () => {
  it('returns true for https:// URLs', () => {
    assert.strictEqual(isSafeDisplayUrl('https://example.com'), true);
  });

  it('returns true for http:// URLs', () => {
    assert.strictEqual(isSafeDisplayUrl('http://example.com'), true);
  });

  it('returns true for URLs with paths and queries', () => {
    assert.strictEqual(isSafeDisplayUrl('https://example.com/page?foo=bar'), true);
  });
});

// ===========================================================================
// isSafeDisplayUrl — blocked
// ===========================================================================

describe('isSafeDisplayUrl — blocked URLs', () => {
  it('returns false for javascript: URLs', () => {
    assert.strictEqual(isSafeDisplayUrl('javascript:alert(1)'), false);
  });

  it('returns false for data: URLs', () => {
    assert.strictEqual(isSafeDisplayUrl('data:text/html,<h1>Hello</h1>'), false);
  });

  it('returns false for file: URLs', () => {
    assert.strictEqual(isSafeDisplayUrl('file:///etc/passwd'), false);
  });

  it('returns false for ftp: URLs', () => {
    assert.strictEqual(isSafeDisplayUrl('ftp://files.example.com'), false);
  });
});

// ===========================================================================
// isSafeDisplayUrl — graceful handling of bad input
// ===========================================================================

describe('isSafeDisplayUrl — bad input', () => {
  it('returns false for empty string', () => {
    assert.strictEqual(isSafeDisplayUrl(''), false);
  });

  it('returns false for null', () => {
    assert.strictEqual(isSafeDisplayUrl(null), false);
  });

  it('returns false for undefined', () => {
    assert.strictEqual(isSafeDisplayUrl(undefined), false);
  });

  it('returns false for non-string input (number)', () => {
    assert.strictEqual(isSafeDisplayUrl(42), false);
  });

  it('returns false for non-string input (object)', () => {
    assert.strictEqual(isSafeDisplayUrl({}), false);
  });

  it('returns false for malformed URL string', () => {
    assert.strictEqual(isSafeDisplayUrl('not a url'), false);
  });

  it('returns false for protocol-less string', () => {
    assert.strictEqual(isSafeDisplayUrl('example.com'), false);
  });
});

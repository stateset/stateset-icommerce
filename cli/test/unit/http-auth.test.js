/**
 * Unit tests for http-auth.js
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import {
  LEVELS,
  ROUTE_PERMISSIONS,
  SANDBOX_BLOCKED_ROUTES,
  createApiKeyAuth,
  checkRoutePermission,
  checkSandbox,
} from '../../src/channels/http-auth.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Build a minimal mock IncomingMessage with optional auth header. */
function mockReq(headers = {}) {
  return { headers };
}

/** Build a URL with optional query params. */
function mockUrl(query = {}) {
  const url = new URL('http://localhost:8080/test');
  for (const [k, v] of Object.entries(query)) {
    url.searchParams.set(k, v);
  }
  return url;
}

// ===========================================================================
// LEVELS
// ===========================================================================

describe('LEVELS', () => {
  it('defines expected permission levels', () => {
    assert.strictEqual(LEVELS.none, 0);
    assert.strictEqual(LEVELS.read, 1);
    assert.strictEqual(LEVELS.preview, 2);
    assert.strictEqual(LEVELS.write, 3);
    assert.strictEqual(LEVELS.delete, 4);
    assert.strictEqual(LEVELS.admin, 5);
  });

  it('has strictly ascending order', () => {
    const vals = Object.values(LEVELS);
    for (let i = 1; i < vals.length; i++) {
      assert.ok(vals[i] > vals[i - 1], `${vals[i]} should be > ${vals[i - 1]}`);
    }
  });
});

// ===========================================================================
// ROUTE_PERMISSIONS
// ===========================================================================

describe('ROUTE_PERMISSIONS', () => {
  it('defines health as none-level (public)', () => {
    assert.strictEqual(ROUTE_PERMISSIONS['/health'].level, 'none');
  });

  it('defines metrics as read-level', () => {
    assert.strictEqual(ROUTE_PERMISSIONS['/metrics'].level, 'read');
  });

  it('defines daemon as admin-level', () => {
    assert.strictEqual(ROUTE_PERMISSIONS['/daemon'].level, 'admin');
  });

  it('defines method overrides for plugins', () => {
    assert.strictEqual(ROUTE_PERMISSIONS['/plugins'].level, 'read');
    assert.strictEqual(ROUTE_PERMISSIONS['/plugins'].methods.POST, 'admin');
  });
});

// ===========================================================================
// SANDBOX_BLOCKED_ROUTES
// ===========================================================================

describe('SANDBOX_BLOCKED_ROUTES', () => {
  it('blocks browser evaluate and navigate', () => {
    assert.ok(SANDBOX_BLOCKED_ROUTES.browser.includes('/browser/evaluate'));
    assert.ok(SANDBOX_BLOCKED_ROUTES.browser.includes('/browser/navigate'));
  });

  it('blocks shell daemon', () => {
    assert.ok(SANDBOX_BLOCKED_ROUTES.shell.includes('/daemon'));
  });
});

// ===========================================================================
// createApiKeyAuth
// ===========================================================================

describe('createApiKeyAuth', () => {
  describe('no keys configured (secure-by-default)', () => {
    it('rejects all requests by default', () => {
      const auth = createApiKeyAuth([]);
      const result = auth.authenticate(mockReq(), mockUrl());
      assert.strictEqual(result.authenticated, false);
    });

    it('works when called with no arguments', () => {
      const auth = createApiKeyAuth();
      const result = auth.authenticate(mockReq(), mockUrl());
      assert.strictEqual(result.authenticated, false);
    });

    it('supports explicit insecure open mode via allowAnonymous', () => {
      const auth = createApiKeyAuth([], {
        allowAnonymous: true,
        anonymousIdentity: { name: 'anonymous', level: 'admin' },
      });
      const result = auth.authenticate(mockReq(), mockUrl());
      assert.strictEqual(result.authenticated, true);
      assert.strictEqual(result.identity.name, 'anonymous');
      assert.strictEqual(result.identity.level, 'admin');
    });

    it('defaults anonymous identity to read-level', () => {
      const auth = createApiKeyAuth([], { allowAnonymous: true });
      const result = auth.authenticate(mockReq(), mockUrl());
      assert.strictEqual(result.authenticated, true);
      assert.strictEqual(result.identity.level, 'read');
    });
  });

  describe('with API keys', () => {
    const keys = [
      { key: 'sk-admin-secret', name: 'admin', level: 'admin' },
      { key: 'sk-read-only', name: 'dashboard', level: 'read' },
    ];

    it('authenticates valid bearer token', () => {
      const auth = createApiKeyAuth(keys);
      const req = mockReq({ authorization: 'Bearer sk-admin-secret' });
      const result = auth.authenticate(req, mockUrl());
      assert.strictEqual(result.authenticated, true);
      assert.strictEqual(result.identity.name, 'admin');
      assert.strictEqual(result.identity.level, 'admin');
    });

    it('authenticates valid query param token when explicitly enabled', () => {
      const auth = createApiKeyAuth(keys, { allowQueryParam: true });
      const req = mockReq();
      const url = mockUrl({ api_key: 'sk-read-only' });
      const result = auth.authenticate(req, url);
      assert.strictEqual(result.authenticated, true);
      assert.strictEqual(result.identity.name, 'dashboard');
      assert.strictEqual(result.identity.level, 'read');
    });

    it('rejects missing token', () => {
      const auth = createApiKeyAuth(keys);
      const result = auth.authenticate(mockReq(), mockUrl());
      assert.strictEqual(result.authenticated, false);
    });

    it('rejects invalid token', () => {
      const auth = createApiKeyAuth(keys);
      const req = mockReq({ authorization: 'Bearer wrong-key' });
      const result = auth.authenticate(req, mockUrl());
      assert.strictEqual(result.authenticated, false);
    });

    it('rejects partial token match (different length)', () => {
      const auth = createApiKeyAuth(keys);
      const req = mockReq({ authorization: 'Bearer sk-admin' });
      const result = auth.authenticate(req, mockUrl());
      assert.strictEqual(result.authenticated, false);
    });

    it('prefers bearer header over query param', () => {
      const auth = createApiKeyAuth(keys);
      const req = mockReq({ authorization: 'Bearer sk-admin-secret' });
      const url = mockUrl({ api_key: 'sk-read-only' });
      const result = auth.authenticate(req, url);
      assert.strictEqual(result.identity.name, 'admin');
    });

    it('defaults level to read when not specified', () => {
      const auth = createApiKeyAuth([{ key: 'sk-test', name: 'tester' }]);
      const req = mockReq({ authorization: 'Bearer sk-test' });
      const result = auth.authenticate(req, mockUrl());
      assert.strictEqual(result.identity.level, 'read');
    });

    it('defaults name to unnamed when not specified', () => {
      const auth = createApiKeyAuth([{ key: 'sk-anon' }]);
      const req = mockReq({ authorization: 'Bearer sk-anon' });
      const result = auth.authenticate(req, mockUrl());
      assert.strictEqual(result.identity.name, 'unnamed');
    });

    it('filters out entries with empty keys', () => {
      const auth = createApiKeyAuth([
        { key: '', name: 'empty' },
        { key: 'sk-valid', name: 'valid' },
      ]);
      const req = mockReq({ authorization: 'Bearer sk-valid' });
      const result = auth.authenticate(req, mockUrl());
      assert.strictEqual(result.authenticated, true);
      assert.strictEqual(result.identity.name, 'valid');
    });

    it('ignores non-Bearer authorization headers', () => {
      const auth = createApiKeyAuth(keys);
      const req = mockReq({ authorization: 'Basic dXNlcjpwYXNz' });
      const result = auth.authenticate(req, mockUrl());
      assert.strictEqual(result.authenticated, false);
    });
  });
});

// ===========================================================================
// checkRoutePermission
// ===========================================================================

describe('checkRoutePermission', () => {
  it('allows admin to access everything', () => {
    const identity = { name: 'admin', level: 'admin' };
    assert.strictEqual(checkRoutePermission(identity, '/health', 'GET').allowed, true);
    assert.strictEqual(checkRoutePermission(identity, '/daemon', 'GET').allowed, true);
    assert.strictEqual(checkRoutePermission(identity, '/memory/123', 'DELETE').allowed, true);
  });

  it('allows read-level access to /health (none required)', () => {
    const identity = { name: 'reader', level: 'read' };
    assert.strictEqual(checkRoutePermission(identity, '/health', 'GET').allowed, true);
  });

  it('allows read-level access to /metrics', () => {
    const identity = { name: 'reader', level: 'read' };
    assert.strictEqual(checkRoutePermission(identity, '/metrics', 'GET').allowed, true);
  });

  it('denies read-level access to /daemon (admin required)', () => {
    const identity = { name: 'reader', level: 'read' };
    const result = checkRoutePermission(identity, '/daemon', 'GET');
    assert.strictEqual(result.allowed, false);
    assert.ok(result.reason.includes('admin'));
  });

  it('uses method-specific override for plugins POST', () => {
    const readIdentity = { name: 'reader', level: 'read' };
    assert.strictEqual(
      checkRoutePermission(readIdentity, '/plugins', 'GET').allowed,
      true,
      'read can GET plugins',
    );
    assert.strictEqual(
      checkRoutePermission(readIdentity, '/plugins/abc/enable', 'POST').allowed,
      false,
      'read cannot POST plugins',
    );
  });

  it('uses method-specific override for memory DELETE', () => {
    const writeIdentity = { name: 'writer', level: 'write' };
    assert.strictEqual(
      checkRoutePermission(writeIdentity, '/memory/save', 'POST').allowed,
      true,
      'write can POST memory',
    );
    assert.strictEqual(
      checkRoutePermission(writeIdentity, '/memory/123', 'DELETE').allowed,
      false,
      'write cannot DELETE memory (requires delete level)',
    );
  });

  it('uses longest prefix match', () => {
    // /voice is read for GET, write for POST
    const readIdentity = { name: 'reader', level: 'read' };
    assert.strictEqual(checkRoutePermission(readIdentity, '/voice/status', 'GET').allowed, true);
    assert.strictEqual(
      checkRoutePermission(readIdentity, '/voice/transcribe', 'POST').allowed,
      false,
    );
  });

  it('denies unknown routes by default', () => {
    const noneIdentity = { name: 'nobody', level: 'none' };
    const readIdentity = { name: 'reader', level: 'read' };
    assert.strictEqual(checkRoutePermission(noneIdentity, '/unknown', 'GET').allowed, false);
    assert.strictEqual(checkRoutePermission(readIdentity, '/unknown', 'GET').allowed, false);
  });

  it('handles unknown permission level gracefully', () => {
    const identity = { name: 'user', level: 'bogus' };
    const result = checkRoutePermission(identity, '/metrics', 'GET');
    assert.strictEqual(result.allowed, false);
  });

  it('provides descriptive reason on denial', () => {
    const identity = { name: 'reader', level: 'read' };
    const result = checkRoutePermission(identity, '/daemon', 'GET');
    assert.ok(result.reason.includes('/daemon'));
    assert.ok(result.reason.includes('admin'));
    assert.ok(result.reason.includes('read'));
  });
});

// ===========================================================================
// checkSandbox
// ===========================================================================

describe('checkSandbox', () => {
  it('returns not blocked when sandbox is null', () => {
    assert.strictEqual(checkSandbox(null, '/browser/evaluate').blocked, false);
  });

  it('returns not blocked when sandbox is empty', () => {
    assert.strictEqual(checkSandbox({}, '/browser/evaluate').blocked, false);
  });

  it('blocks browser evaluate when browser sandbox enabled', () => {
    const result = checkSandbox({ browser: true }, '/browser/evaluate');
    assert.strictEqual(result.blocked, true);
    assert.ok(result.reason.includes('browser'));
  });

  it('blocks browser navigate when browser sandbox enabled', () => {
    const result = checkSandbox({ browser: true }, '/browser/navigate');
    assert.strictEqual(result.blocked, true);
  });

  it('does not block browser status (read-only) when browser sandbox enabled', () => {
    assert.strictEqual(checkSandbox({ browser: true }, '/browser/status').blocked, false);
  });

  it('blocks /daemon when shell sandbox enabled', () => {
    const result = checkSandbox({ shell: true }, '/daemon');
    assert.strictEqual(result.blocked, true);
    assert.ok(result.reason.includes('shell'));
  });

  it('does not block /daemon when only browser sandbox enabled', () => {
    assert.strictEqual(checkSandbox({ browser: true }, '/daemon').blocked, false);
  });

  it('blocks sub-paths of blocked routes', () => {
    const result = checkSandbox({ browser: true }, '/browser/evaluate/something');
    assert.strictEqual(result.blocked, true);
  });

  it('does not block unrelated routes', () => {
    const sandbox = { browser: true, shell: true };
    assert.strictEqual(checkSandbox(sandbox, '/metrics').blocked, false);
    assert.strictEqual(checkSandbox(sandbox, '/health').blocked, false);
    assert.strictEqual(checkSandbox(sandbox, '/plugins').blocked, false);
  });
});

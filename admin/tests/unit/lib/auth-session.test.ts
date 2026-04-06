/**
 * Tests for Auth Session Helpers
 *
 * @module tests/unit/lib/auth-session
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { NextRequest, NextResponse } from 'next/server';
import {
  extractBearerToken,
  getRequestSessionToken,
  requireRequestSessionToken,
  setSessionCookie,
  clearSessionCookie,
  isAuthenticatedRequest,
  ADMIN_SESSION_COOKIE,
  getBypassAdminUser,
  getServiceSessionToken,
  isAdminAuthDisabled,
  validateSessionToken,
} from '@/lib/shared/auth-session';
import { AppError } from '@/lib/shared/errors';

afterEach(() => {
  vi.unstubAllEnvs();
});

// ============================================================================
// extractBearerToken
// ============================================================================

describe('extractBearerToken', () => {
  it('extracts token from valid Bearer header', () => {
    expect(extractBearerToken('Bearer my-jwt-token')).toBe('my-jwt-token');
  });

  it('extracts token from lowercase bearer prefix', () => {
    expect(extractBearerToken('bearer my-token')).toBe('my-token');
  });

  it('extracts token from mixed-case BEARER prefix', () => {
    expect(extractBearerToken('BEARER my-token')).toBe('my-token');
  });

  it('returns null for null input', () => {
    expect(extractBearerToken(null)).toBeNull();
  });

  it('returns null for non-string input', () => {
    expect(extractBearerToken(undefined as any)).toBeNull();
    expect(extractBearerToken(123 as any)).toBeNull();
  });

  it('returns null for empty string', () => {
    expect(extractBearerToken('')).toBeNull();
  });

  it('returns null for Basic auth header', () => {
    expect(extractBearerToken('Basic dXNlcjpwYXNz')).toBeNull();
  });

  it('returns null for "Bearer " with no token (whitespace only)', () => {
    expect(extractBearerToken('Bearer   ')).toBeNull();
  });

  it('returns null for just "Bearer" with no space', () => {
    expect(extractBearerToken('Bearer')).toBeNull();
  });

  it('trims whitespace from extracted token', () => {
    expect(extractBearerToken('Bearer  my-token  ')).toBe('my-token');
  });
});

// ============================================================================
// getRequestSessionToken
// ============================================================================

describe('getRequestSessionToken', () => {
  function makeRequest(headers: Record<string, string> = {}): NextRequest {
    return new NextRequest('http://localhost:3000/api/test', {
      headers: new Headers(headers),
    });
  }

  it('returns token from Authorization header', () => {
    const request = makeRequest({ Authorization: 'Bearer jwt-abc-123' });
    expect(getRequestSessionToken(request)).toBe('jwt-abc-123');
  });

  it('returns token from session cookie when no auth header', () => {
    const request = makeRequest({
      Cookie: `${ADMIN_SESSION_COOKIE}=cookie-token-xyz`,
    });
    expect(getRequestSessionToken(request)).toBe('cookie-token-xyz');
  });

  it('prefers Authorization header over cookie', () => {
    const request = makeRequest({
      Authorization: 'Bearer header-token',
      Cookie: `${ADMIN_SESSION_COOKIE}=cookie-token`,
    });
    expect(getRequestSessionToken(request)).toBe('header-token');
  });

  it('returns null when neither header nor cookie is present', () => {
    const request = makeRequest({});
    expect(getRequestSessionToken(request)).toBeNull();
  });

  it('returns null for invalid Authorization header without cookie', () => {
    const request = makeRequest({ Authorization: 'Basic abc' });
    expect(getRequestSessionToken(request)).toBeNull();
  });
});

// ============================================================================
// requireRequestSessionToken
// ============================================================================

describe('requireRequestSessionToken', () => {
  function makeRequest(headers: Record<string, string> = {}): NextRequest {
    return new NextRequest('http://localhost:3000/api/test', {
      headers: new Headers(headers),
    });
  }

  it('returns token when present', () => {
    const request = makeRequest({ Authorization: 'Bearer valid-token' });
    expect(requireRequestSessionToken(request)).toBe('valid-token');
  });

  it('throws AppError.unauthorized when token is missing', () => {
    const request = makeRequest({});
    expect(() => requireRequestSessionToken(request)).toThrow(AppError);
    try {
      requireRequestSessionToken(request);
    } catch (e) {
      expect((e as AppError).statusCode).toBe(401);
      expect((e as AppError).code).toBe('UNAUTHORIZED');
    }
  });

  it('throws with custom message when provided', () => {
    const request = makeRequest({});
    expect(() =>
      requireRequestSessionToken(request, 'Token expired')
    ).toThrow('Token expired');
  });

  it('throws with default message when no custom message', () => {
    const request = makeRequest({});
    expect(() => requireRequestSessionToken(request)).toThrow(
      'Authentication required'
    );
  });

  it('uses the service token when admin auth is disabled', () => {
    vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');
    vi.stubEnv('STATESET_API_TOKEN', 'service-token');
    const request = makeRequest({});

    expect(requireRequestSessionToken(request)).toBe('service-token');
  });
});

// ============================================================================
// setSessionCookie / clearSessionCookie
// ============================================================================

describe('setSessionCookie', () => {
  it('sets the session cookie on the response', () => {
    const response = NextResponse.json({ ok: true });
    setSessionCookie(response, 'my-session-token');

    const setCookie = response.headers.get('set-cookie');
    expect(setCookie).toContain(`${ADMIN_SESSION_COOKIE}=my-session-token`);
  });

  it('sets HttpOnly flag', () => {
    const response = NextResponse.json({ ok: true });
    setSessionCookie(response, 'token-123');

    const setCookie = response.headers.get('set-cookie');
    expect(setCookie?.toLowerCase()).toContain('httponly');
  });

  it('sets SameSite=Lax', () => {
    const response = NextResponse.json({ ok: true });
    setSessionCookie(response, 'token-123');

    const setCookie = response.headers.get('set-cookie');
    expect(setCookie?.toLowerCase()).toContain('samesite=lax');
  });

  it('sets path to /', () => {
    const response = NextResponse.json({ ok: true });
    setSessionCookie(response, 'token-123');

    const setCookie = response.headers.get('set-cookie');
    expect(setCookie).toContain('Path=/');
  });

  it('returns the same response object', () => {
    const response = NextResponse.json({ ok: true });
    const result = setSessionCookie(response, 'token');
    expect(result).toBe(response);
  });
});

describe('clearSessionCookie', () => {
  it('sets the session cookie with empty value', () => {
    const response = NextResponse.json({ ok: true });
    clearSessionCookie(response);

    const setCookie = response.headers.get('set-cookie');
    expect(setCookie).toContain(`${ADMIN_SESSION_COOKIE}=`);
  });

  it('sets Max-Age=0 to expire immediately', () => {
    const response = NextResponse.json({ ok: true });
    clearSessionCookie(response);

    const setCookie = response.headers.get('set-cookie');
    expect(setCookie).toContain('Max-Age=0');
  });

  it('returns the same response object', () => {
    const response = NextResponse.json({ ok: true });
    const result = clearSessionCookie(response);
    expect(result).toBe(response);
  });
});

// ============================================================================
// isAuthenticatedRequest
// ============================================================================

describe('isAuthenticatedRequest', () => {
  function makeRequest(headers: Record<string, string> = {}): NextRequest {
    return new NextRequest('http://localhost:3000/api/test', {
      headers: new Headers(headers),
    });
  }

  it('returns true when Authorization header has a valid bearer token', () => {
    const request = makeRequest({ Authorization: 'Bearer some-token' });
    expect(isAuthenticatedRequest(request)).toBe(true);
  });

  it('returns true when session cookie is set', () => {
    const request = makeRequest({
      Cookie: `${ADMIN_SESSION_COOKIE}=session-val`,
    });
    expect(isAuthenticatedRequest(request)).toBe(true);
  });

  it('returns false when no auth credentials provided', () => {
    const request = makeRequest({});
    expect(isAuthenticatedRequest(request)).toBe(false);
  });

  it('returns false for invalid Authorization scheme', () => {
    const request = makeRequest({ Authorization: 'Digest abc' });
    expect(isAuthenticatedRequest(request)).toBe(false);
  });

  it('returns true when admin auth is disabled and a service token is configured', () => {
    vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');
    vi.stubEnv('STATESET_API_TOKEN', 'service-token');
    const request = makeRequest({});

    expect(isAuthenticatedRequest(request)).toBe(true);
  });
});

describe('admin auth bypass helpers', () => {
  it('detects the auth-disable flag', () => {
    vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');
    expect(isAdminAuthDisabled()).toBe(true);
  });

  it('ignores the auth-disable flag in production', () => {
    vi.stubEnv('NODE_ENV', 'production');
    vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');
    expect(isAdminAuthDisabled()).toBe(false);
  });

  it('reads the service token from env', () => {
    vi.stubEnv('STATESET_API_TOKEN', ' service-token ');
    expect(getServiceSessionToken()).toBe('service-token');
  });

  it('returns the local bypass admin user profile', () => {
    expect(getBypassAdminUser()).toMatchObject({
      id: 'stateset-admin-local',
      email: 'local@stateset.dev',
      authMode: 'disabled',
    });
  });
});

// ============================================================================
// validateSessionToken
// ============================================================================

describe('validateSessionToken', () => {
  const mockFetch = vi.fn();

  beforeEach(() => {
    vi.stubGlobal('fetch', mockFetch);
    vi.stubEnv('STATESET_API_URL', 'https://api.sandbox.stateset.app');
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    vi.unstubAllEnvs();
  });

  it('returns true when upstream returns ok', async () => {
    mockFetch.mockResolvedValue({ ok: true });
    const result = await validateSessionToken('valid-token');
    expect(result).toBe(true);
  });

  it('returns false when upstream returns non-ok', async () => {
    mockFetch.mockResolvedValue({ ok: false, status: 401 });
    const result = await validateSessionToken('expired-token');
    expect(result).toBe(false);
  });

  it('returns false when fetch throws', async () => {
    mockFetch.mockRejectedValue(new Error('Network error'));
    const result = await validateSessionToken('some-token');
    expect(result).toBe(false);
  });

  it('returns false for empty token', async () => {
    const result = await validateSessionToken('');
    expect(result).toBe(false);
    expect(mockFetch).not.toHaveBeenCalled();
  });

  it('returns false for whitespace-only token', async () => {
    const result = await validateSessionToken('   ');
    expect(result).toBe(false);
    expect(mockFetch).not.toHaveBeenCalled();
  });

  it('calls the correct upstream URL', async () => {
    mockFetch.mockResolvedValue({ ok: true });
    await validateSessionToken('my-token');
    expect(mockFetch).toHaveBeenCalledWith(
      'https://api.sandbox.stateset.app/api/auth/me',
      expect.objectContaining({
        method: 'GET',
        headers: expect.objectContaining({
          Authorization: 'Bearer my-token',
        }),
      })
    );
  });

  it('short-circuits to true when admin auth is disabled', async () => {
    vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');
    const result = await validateSessionToken('anything');

    expect(result).toBe(true);
    expect(mockFetch).not.toHaveBeenCalled();
  });
});

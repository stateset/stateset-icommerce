/**
 * Tests for CSRF Protection
 *
 * @module tests/unit/lib/csrf
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { NextRequest, NextResponse } from 'next/server';

// Mock next/headers cookies() — must be before imports
vi.mock('next/headers', () => {
  const cookieStore = new Map<string, { value: string }>();
  return {
    cookies: vi.fn(() =>
      Promise.resolve({
        get: (name: string) => cookieStore.get(name),
        set: (name: string, value: string, _opts?: unknown) => {
          cookieStore.set(name, { value });
        },
        _store: cookieStore,
      })
    ),
    __cookieStore: cookieStore,
  };
});

import { validateCsrfToken, requireCsrf } from '@/lib/shared/csrf';

function createRequest(options: {
  method?: string;
  csrfHeader?: string | null;
  csrfCookie?: string | null;
} = {}): NextRequest {
  const { method = 'POST', csrfHeader, csrfCookie } = options;
  const headers = new Headers({ 'Content-Type': 'application/json' });
  if (csrfHeader !== null && csrfHeader !== undefined) {
    headers.set('x-csrf-token', csrfHeader);
  }
  if (csrfCookie !== null && csrfCookie !== undefined) {
    headers.set('Cookie', `__csrf=${csrfCookie}`);
  }
  return new NextRequest('http://localhost:3000/api/test', {
    method,
    headers,
  });
}

describe('validateCsrfToken', () => {
  it('returns true when header and cookie match', async () => {
    const request = createRequest({
      csrfHeader: 'abc123def456',
      csrfCookie: 'abc123def456',
    });
    const result = await validateCsrfToken(request);
    expect(result).toBe(true);
  });

  it('returns false when header and cookie differ', async () => {
    const request = createRequest({
      csrfHeader: 'token-a',
      csrfCookie: 'token-b',
    });
    const result = await validateCsrfToken(request);
    expect(result).toBe(false);
  });

  it('returns false when header is missing', async () => {
    const request = createRequest({
      csrfHeader: null,
      csrfCookie: 'some-token',
    });
    const result = await validateCsrfToken(request);
    expect(result).toBe(false);
  });

  it('returns false when cookie is missing', async () => {
    const request = createRequest({
      csrfHeader: 'some-token',
      csrfCookie: null,
    });
    const result = await validateCsrfToken(request);
    expect(result).toBe(false);
  });

  it('returns false when both header and cookie are missing', async () => {
    const request = createRequest({
      csrfHeader: null,
      csrfCookie: null,
    });
    const result = await validateCsrfToken(request);
    expect(result).toBe(false);
  });

  it('returns false when tokens have different lengths', async () => {
    const request = createRequest({
      csrfHeader: 'short',
      csrfCookie: 'much-longer-token-value',
    });
    const result = await validateCsrfToken(request);
    expect(result).toBe(false);
  });

  it('performs comparison even for empty strings', async () => {
    const request = createRequest({
      csrfHeader: '',
      csrfCookie: '',
    });
    // Both empty strings in headers => both are falsy => returns false
    const result = await validateCsrfToken(request);
    expect(result).toBe(false);
  });
});

describe('requireCsrf', () => {
  it('returns null for GET requests (safe method)', async () => {
    const request = createRequest({ method: 'GET' });
    const result = await requireCsrf(request);
    expect(result).toBeNull();
  });

  it('returns null for HEAD requests (safe method)', async () => {
    const request = createRequest({ method: 'HEAD' });
    const result = await requireCsrf(request);
    expect(result).toBeNull();
  });

  it('returns null for OPTIONS requests (safe method)', async () => {
    const request = createRequest({ method: 'OPTIONS' });
    const result = await requireCsrf(request);
    expect(result).toBeNull();
  });

  it('returns 403 response for POST without valid CSRF', async () => {
    const request = createRequest({
      method: 'POST',
      csrfHeader: null,
      csrfCookie: null,
    });
    const result = await requireCsrf(request);
    expect(result).not.toBeNull();
    expect(result!.status).toBe(403);
    const body = await result!.json();
    expect(body.error.code).toBe('CSRF_INVALID');
  });

  it('returns 403 response for PUT without valid CSRF', async () => {
    const request = createRequest({
      method: 'PUT',
      csrfHeader: null,
      csrfCookie: null,
    });
    const result = await requireCsrf(request);
    expect(result).not.toBeNull();
    expect(result!.status).toBe(403);
  });

  it('returns 403 response for DELETE without valid CSRF', async () => {
    const request = createRequest({
      method: 'DELETE',
      csrfHeader: null,
      csrfCookie: null,
    });
    const result = await requireCsrf(request);
    expect(result).not.toBeNull();
    expect(result!.status).toBe(403);
  });

  it('returns 403 response for PATCH without valid CSRF', async () => {
    const request = createRequest({
      method: 'PATCH',
      csrfHeader: null,
      csrfCookie: null,
    });
    const result = await requireCsrf(request);
    expect(result).not.toBeNull();
    expect(result!.status).toBe(403);
  });

  it('returns null for POST with matching tokens', async () => {
    const token = 'valid-csrf-token-abc';
    const request = createRequest({
      method: 'POST',
      csrfHeader: token,
      csrfCookie: token,
    });
    const result = await requireCsrf(request);
    expect(result).toBeNull();
  });

  it('returns 403 for POST with mismatched tokens', async () => {
    const request = createRequest({
      method: 'POST',
      csrfHeader: 'token-from-header',
      csrfCookie: 'different-cookie-val',
    });
    const result = await requireCsrf(request);
    expect(result).not.toBeNull();
    expect(result!.status).toBe(403);
  });

  it('error response includes success: false', async () => {
    const request = createRequest({
      method: 'POST',
      csrfHeader: null,
      csrfCookie: null,
    });
    const result = await requireCsrf(request);
    const body = await result!.json();
    expect(body.success).toBe(false);
  });
});

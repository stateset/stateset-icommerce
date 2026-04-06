/**
 * Integration tests for auth route handlers:
 * forgot-password, reset-password, verify-email, csrf-token
 *
 * @module tests/integration/auth-routes
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  createMockRequest,
  createAuthenticatedRequest,
  expectSuccess,
  expectError,
} from '../helpers/api-test-utils';

// Mock the request-context module
vi.mock('@/lib/shared/request-context', () => {
  const { AsyncLocalStorage } = require('async_hooks');
  const store = new AsyncLocalStorage();
  return {
    requestStore: store,
    getRequestContext: () =>
      store.getStore() ?? { requestId: 'req_test', startTime: Date.now() },
    getRequestId: () => store.getStore()?.requestId ?? 'req_test',
    generateRequestId: () => 'req_test_generated',
  };
});

// Mock next/headers cookies() for csrf-token route
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

// ============================================================================
// POST /api/auth/forgot-password
// ============================================================================

describe('POST /api/auth/forgot-password', () => {
  it('always returns success to prevent email enumeration', async () => {
    mockFetch.mockResolvedValue({ ok: true, json: () => Promise.resolve({}) });

    const { POST } = await import('@/app/api/auth/forgot-password/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/auth/forgot-password',
      body: { email: 'user@example.com' },
    });

    const response = await POST(request, undefined as any);
    const body = await expectSuccess(response);

    expect(body.data).toHaveProperty('message');
    expect((body.data as any).message).toContain('reset link');
  });

  it('returns success even when upstream fails (prevents enumeration)', async () => {
    mockFetch.mockResolvedValue({ ok: false, status: 404 });

    const { POST } = await import('@/app/api/auth/forgot-password/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/auth/forgot-password',
      body: { email: 'nonexistent@example.com' },
    });

    const response = await POST(request, undefined as any);
    const body = await expectSuccess(response);
    expect(body.data).toHaveProperty('message');
  });

  it('returns 422 for missing email', async () => {
    const { POST } = await import('@/app/api/auth/forgot-password/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/auth/forgot-password',
      body: {},
    });

    const response = await POST(request, undefined as any);
    await expectError(response, 422, 'VALIDATION_ERROR');
  });

  it('returns 422 for invalid email format', async () => {
    const { POST } = await import('@/app/api/auth/forgot-password/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/auth/forgot-password',
      body: { email: 'not-an-email' },
    });

    const response = await POST(request, undefined as any);
    await expectError(response, 422, 'VALIDATION_ERROR');
  });

  it('forwards email to upstream API', async () => {
    mockFetch.mockResolvedValue({ ok: true, json: () => Promise.resolve({}) });

    const { POST } = await import('@/app/api/auth/forgot-password/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/auth/forgot-password',
      body: { email: 'user@example.com' },
    });

    await POST(request, undefined as any);

    expect(mockFetch).toHaveBeenCalledWith(
      'https://api.sandbox.stateset.app/api/auth/forgot-password',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ email: 'user@example.com' }),
      })
    );
  });
});

// ============================================================================
// POST /api/auth/reset-password
// ============================================================================

describe('POST /api/auth/reset-password', () => {
  it('returns success on valid reset', async () => {
    mockFetch.mockResolvedValue({ ok: true, json: () => Promise.resolve({}) });

    const { POST } = await import('@/app/api/auth/reset-password/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/auth/reset-password',
      body: { token: 'valid-reset-token', password: 'NewSecure1' },
    });

    const response = await POST(request, undefined as any);
    const body = await expectSuccess(response);
    expect((body.data as any).message).toContain('reset successfully');
  });

  it('returns 422 for missing token', async () => {
    const { POST } = await import('@/app/api/auth/reset-password/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/auth/reset-password',
      body: { password: 'NewSecure1' },
    });

    const response = await POST(request, undefined as any);
    await expectError(response, 422, 'VALIDATION_ERROR');
  });

  it('returns 422 for weak password', async () => {
    const { POST } = await import('@/app/api/auth/reset-password/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/auth/reset-password',
      body: { token: 'some-token', password: 'weak' },
    });

    const response = await POST(request, undefined as any);
    await expectError(response, 422, 'VALIDATION_ERROR');
  });

  it('returns 400 when upstream says token is invalid/expired', async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 400,
      json: () => Promise.resolve({ error: 'Token expired' }),
    });

    const { POST } = await import('@/app/api/auth/reset-password/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/auth/reset-password',
      body: { token: 'expired-token', password: 'NewSecure1' },
    });

    const response = await POST(request, undefined as any);
    await expectError(response, 400, 'BAD_REQUEST');
  });

  it('returns upstream error status for other failures', async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 500,
      json: () => Promise.resolve({ error: 'Internal error' }),
    });

    const { POST } = await import('@/app/api/auth/reset-password/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/auth/reset-password',
      body: { token: 'valid-token', password: 'NewSecure1' },
    });

    const response = await POST(request, undefined as any);
    await expectError(response, 500, 'AUTH_ERROR');
  });
});

// ============================================================================
// POST /api/auth/verify-email
// ============================================================================

describe('POST /api/auth/verify-email', () => {
  it('returns success on valid verification', async () => {
    mockFetch.mockResolvedValue({ ok: true, json: () => Promise.resolve({}) });

    const { POST } = await import('@/app/api/auth/verify-email/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/auth/verify-email',
      body: { token: 'verification-token-abc' },
    });

    const response = await POST(request, undefined as any);
    const body = await expectSuccess(response);
    expect((body.data as any).message).toContain('verified');
  });

  it('returns 422 for missing token', async () => {
    const { POST } = await import('@/app/api/auth/verify-email/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/auth/verify-email',
      body: {},
    });

    const response = await POST(request, undefined as any);
    await expectError(response, 422, 'VALIDATION_ERROR');
  });

  it('returns 422 for empty token', async () => {
    const { POST } = await import('@/app/api/auth/verify-email/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/auth/verify-email',
      body: { token: '' },
    });

    const response = await POST(request, undefined as any);
    await expectError(response, 422, 'VALIDATION_ERROR');
  });

  it('returns 400 when upstream says token is invalid', async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 400,
      json: () => Promise.resolve({ error: 'Invalid token' }),
    });

    const { POST } = await import('@/app/api/auth/verify-email/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/auth/verify-email',
      body: { token: 'bad-token' },
    });

    const response = await POST(request, undefined as any);
    await expectError(response, 400, 'BAD_REQUEST');
  });

  it('returns upstream status for other failures', async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 503,
      json: () => Promise.resolve({ error: 'Service unavailable' }),
    });

    const { POST } = await import('@/app/api/auth/verify-email/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/auth/verify-email',
      body: { token: 'valid-token' },
    });

    const response = await POST(request, undefined as any);
    await expectError(response, 503, 'AUTH_ERROR');
  });
});

// ============================================================================
// GET /api/auth/csrf-token
// ============================================================================

describe('GET /api/auth/csrf-token', () => {
  it('returns a CSRF token in the response body', async () => {
    const { GET } = await import('@/app/api/auth/csrf-token/route');

    const request = createMockRequest({
      method: 'GET',
      url: 'http://localhost:3000/api/auth/csrf-token',
    });

    const response = await GET(request, undefined as any);
    const body = await expectSuccess(response);

    expect(body.data).toHaveProperty('csrfToken');
    expect(typeof (body.data as any).csrfToken).toBe('string');
    expect((body.data as any).csrfToken.length).toBeGreaterThan(0);
  });
});

// ============================================================================
// POST /api/auth/me — unauthenticated
// ============================================================================

describe('GET /api/auth/me — unauthenticated', () => {
  it('returns 401 when no token provided', async () => {
    const { GET } = await import('@/app/api/auth/me/route');

    const request = createMockRequest({
      method: 'GET',
      url: 'http://localhost:3000/api/auth/me',
    });

    const response = await GET(request, undefined as any);
    await expectError(response, 401, 'UNAUTHORIZED');
  });
});

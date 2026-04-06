/**
 * Tests for /api/auth/login and /api/auth/register endpoints
 *
 * @module tests/unit/api/auth
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  createMockRequest,
  createSessionCookieRequest,
  parseResponse,
  expectSuccess,
  expectError,
} from '../../helpers/api-test-utils';

// Mock the request-context module
vi.mock('@/lib/shared/request-context', () => {
  const { AsyncLocalStorage } = require('async_hooks');
  const store = new AsyncLocalStorage();
  return {
    requestStore: store,
    getRequestContext: () => store.getStore() ?? { requestId: 'req_test', startTime: Date.now() },
    getRequestId: () => store.getStore()?.requestId ?? 'req_test',
    generateRequestId: () => 'req_test_generated',
  };
});

// Mock fetch for upstream API calls
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
// POST /api/auth/login
// ============================================================================

describe('POST /api/auth/login', () => {
  describe('valid credentials', () => {
    it('returns success with token data', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        json: () =>
          Promise.resolve({
            token: 'jwt-token-123',
            user: { id: 'user-1', email: 'test@example.com' },
          }),
      });

      const { POST } = await import('@/app/api/auth/login/route');

      const request = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/login',
        body: { email: 'test@example.com', password: 'Password1x' },
      });

      const response = await POST(request, undefined as any);
      const body = await expectSuccess(response);

      expect(body.data).toHaveProperty('token');
      expect(body.data).toHaveProperty('user');
    });

    it('sets the admin session cookie on successful login', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        json: () =>
          Promise.resolve({
            token: 'jwt-token-123',
            user: { id: 'user-1', email: 'test@example.com' },
          }),
      });

      const { POST } = await import('@/app/api/auth/login/route');

      const request = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/login',
        body: { email: 'test@example.com', password: 'Password1x' },
      });

      const response = await POST(request, undefined as any);
      const sessionCookie = response.headers.get('set-cookie');

      expect(sessionCookie).toContain('stateset_admin_session=jwt-token-123');
    });

    it('forwards credentials to upstream API', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        json: () =>
          Promise.resolve({
            token: 'jwt-token-123',
            user: { id: 'user-1', email: 'test@example.com' },
          }),
      });

      const { POST } = await import('@/app/api/auth/login/route');

      const request = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/login',
        body: { email: 'test@example.com', password: 'Password1x' },
      });

      await POST(request, undefined as any);

      expect(mockFetch).toHaveBeenCalledWith(
        'https://api.sandbox.stateset.app/api/auth/login',
        expect.objectContaining({
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ email: 'test@example.com', password: 'Password1x' }),
        })
      );
    });
  });

  describe('validation', () => {
    it('returns 422 when email is missing', async () => {
      const { POST } = await import('@/app/api/auth/login/route');

      const request = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/login',
        body: { password: 'Password1x' },
      });

      const response = await POST(request, undefined as any);
      await expectError(response, 422, 'VALIDATION_ERROR');
    });

    it('returns 422 when password is missing', async () => {
      const { POST } = await import('@/app/api/auth/login/route');

      const request = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/login',
        body: { email: 'test@example.com' },
      });

      const response = await POST(request, undefined as any);
      await expectError(response, 422, 'VALIDATION_ERROR');
    });

    it('returns 422 when email is invalid format', async () => {
      const { POST } = await import('@/app/api/auth/login/route');

      const request = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/login',
        body: { email: 'not-an-email', password: 'Password1x' },
      });

      const response = await POST(request, undefined as any);
      await expectError(response, 422, 'VALIDATION_ERROR');
    });

    it('returns 422 when password is too short', async () => {
      const { POST } = await import('@/app/api/auth/login/route');

      const request = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/login',
        body: { email: 'test@example.com', password: 'short' },
      });

      const response = await POST(request, undefined as any);
      await expectError(response, 422, 'VALIDATION_ERROR');
    });
  });

  describe('upstream errors', () => {
    it('returns 401 when upstream returns 401 (wrong credentials)', async () => {
      mockFetch.mockResolvedValue({
        ok: false,
        status: 401,
        json: () => Promise.resolve({ error: 'Invalid credentials' }),
      });

      const { POST } = await import('@/app/api/auth/login/route');

      const request = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/login',
        body: { email: 'test@example.com', password: 'Wrongpass1' },
      });

      const response = await POST(request, undefined as any);
      await expectError(response, 401, 'UNAUTHORIZED');
    });

    it('returns upstream error status for other failures', async () => {
      mockFetch.mockResolvedValue({
        ok: false,
        status: 500,
        json: () => Promise.resolve({ error: 'Internal server error' }),
      });

      const { POST } = await import('@/app/api/auth/login/route');

      const request = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/login',
        body: { email: 'test@example.com', password: 'Password1x' },
      });

      const response = await POST(request, undefined as any);
      await expectError(response, 500, 'AUTH_ERROR');
    });
  });
});

describe('GET /api/auth/me', () => {
  it('accepts the admin session cookie', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          user: { id: 'user-1', email: 'test@example.com' },
        }),
    });

    const { GET } = await import('@/app/api/auth/me/route');

    const request = createSessionCookieRequest({
      url: 'http://localhost:3000/api/auth/me',
    });

    const response = await GET(request, undefined as any);
    const body = await expectSuccess(response);

    expect(body.data).toHaveProperty('user');
    expect(mockFetch).toHaveBeenCalledWith(
      'https://api.sandbox.stateset.app/api/auth/me',
      expect.objectContaining({
        headers: expect.objectContaining({
          Authorization: 'Bearer test-session-token',
        }),
      })
    );
  });

  it('returns the local bypass user when admin auth is disabled', async () => {
    vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');

    const { GET } = await import('@/app/api/auth/me/route');

    const request = createMockRequest({
      url: 'http://localhost:3000/api/auth/me',
    });

    const response = await GET(request, undefined as any);
    const body = await expectSuccess(response);

    expect(body.data).toMatchObject({
      user: {
        id: 'stateset-admin-local',
        email: 'local@stateset.dev',
        authMode: 'disabled',
      },
    });
    expect(mockFetch).not.toHaveBeenCalled();
  });
});

describe('POST /api/auth/logout', () => {
  it('clears the admin session cookie after logout', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ message: 'ok' }),
    });

    const { POST } = await import('@/app/api/auth/logout/route');

    const request = createSessionCookieRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/auth/logout',
      body: {},
    });

    const response = await POST(request, undefined as any);
    const sessionCookie = response.headers.get('set-cookie');

    expect(sessionCookie).toContain('stateset_admin_session=');
    expect(sessionCookie).toContain('Max-Age=0');
  });

  it('succeeds without an upstream call when admin auth is disabled', async () => {
    vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');

    const { POST } = await import('@/app/api/auth/logout/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/auth/logout',
      body: {},
    });

    const response = await POST(request, undefined as any);
    const body = await expectSuccess(response);

    expect(body.data).toMatchObject({ message: 'Logged out successfully' });
    expect(mockFetch).not.toHaveBeenCalled();
  });
});

// ============================================================================
// POST /api/auth/register
// ============================================================================

describe('POST /api/auth/register', () => {
  describe('valid registration', () => {
    it('returns 201 with user data on success', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        json: () =>
          Promise.resolve({
            user: {
              id: 'user-new',
              email: 'newuser@example.com',
              firstName: 'John',
              lastName: 'Doe',
            },
          }),
      });

      const { POST } = await import('@/app/api/auth/register/route');

      const request = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/register',
        body: {
          email: 'newuser@example.com',
          password: 'Securepass1',
          firstName: 'John',
          lastName: 'Doe',
        },
      });

      const response = await POST(request, undefined as any);
      const body = await expectSuccess(response, 201);

      expect(body.data).toHaveProperty('user');
    });

    it('accepts optional orgName field', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        json: () =>
          Promise.resolve({
            user: { id: 'user-new', email: 'newuser@example.com' },
            org: { id: 'org-new', name: 'My Org' },
          }),
      });

      const { POST } = await import('@/app/api/auth/register/route');

      const request = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/register',
        body: {
          email: 'newuser@example.com',
          password: 'Securepass1',
          firstName: 'John',
          lastName: 'Doe',
          orgName: 'My Org',
        },
      });

      const response = await POST(request, undefined as any);

      expect(response.status).toBe(201);
    });
  });

  describe('validation', () => {
    it('returns 422 when email is missing', async () => {
      const { POST } = await import('@/app/api/auth/register/route');

      const request = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/register',
        body: {
          password: 'Securepass1',
          firstName: 'John',
          lastName: 'Doe',
        },
      });

      const response = await POST(request, undefined as any);
      await expectError(response, 422, 'VALIDATION_ERROR');
    });

    it('returns 422 when password is missing', async () => {
      const { POST } = await import('@/app/api/auth/register/route');

      const request = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/register',
        body: {
          email: 'newuser@example.com',
          firstName: 'John',
          lastName: 'Doe',
        },
      });

      const response = await POST(request, undefined as any);
      await expectError(response, 422, 'VALIDATION_ERROR');
    });

    it('returns 422 when firstName is missing', async () => {
      const { POST } = await import('@/app/api/auth/register/route');

      const request = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/register',
        body: {
          email: 'newuser@example.com',
          password: 'Securepass1',
          lastName: 'Doe',
        },
      });

      const response = await POST(request, undefined as any);
      await expectError(response, 422, 'VALIDATION_ERROR');
    });

    it('returns 422 when lastName is missing', async () => {
      const { POST } = await import('@/app/api/auth/register/route');

      const request = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/register',
        body: {
          email: 'newuser@example.com',
          password: 'Securepass1',
          firstName: 'John',
        },
      });

      const response = await POST(request, undefined as any);
      await expectError(response, 422, 'VALIDATION_ERROR');
    });

    it('returns 422 when email format is invalid', async () => {
      const { POST } = await import('@/app/api/auth/register/route');

      const request = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/register',
        body: {
          email: 'not-an-email',
          password: 'Securepass1',
          firstName: 'John',
          lastName: 'Doe',
        },
      });

      const response = await POST(request, undefined as any);
      await expectError(response, 422, 'VALIDATION_ERROR');
    });
  });

  describe('upstream errors', () => {
    it('returns 409 when email already exists', async () => {
      mockFetch.mockResolvedValue({
        ok: false,
        status: 409,
        json: () => Promise.resolve({ error: 'Email already registered' }),
      });

      const { POST } = await import('@/app/api/auth/register/route');

      const request = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/register',
        body: {
          email: 'existing@example.com',
          password: 'Securepass1',
          firstName: 'John',
          lastName: 'Doe',
        },
      });

      const response = await POST(request, undefined as any);
      await expectError(response, 409, 'CONFLICT');
    });

    it('returns upstream error status for other failures', async () => {
      mockFetch.mockResolvedValue({
        ok: false,
        status: 503,
        json: () => Promise.resolve({ error: 'Service unavailable' }),
      });

      const { POST } = await import('@/app/api/auth/register/route');

      const request = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/register',
        body: {
          email: 'newuser@example.com',
          password: 'Securepass1',
          firstName: 'John',
          lastName: 'Doe',
        },
      });

      const response = await POST(request, undefined as any);
      await expectError(response, 503, 'AUTH_ERROR');
    });
  });
});

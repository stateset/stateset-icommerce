/**
 * Tests for GET /api/sessions endpoint logic
 *
 * @module tests/unit/api/sessions
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  createMockRequest,
  createAuthenticatedRequest,
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
  vi.stubEnv('STATESET_API_TOKEN', '');
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.unstubAllEnvs();
});

describe('GET /api/sessions', () => {
  describe('authentication', () => {
    it('returns 401 when no auth token is present', async () => {
      // No Authorization header, no env token
      vi.stubEnv('STATESET_API_TOKEN', '');

      const { GET } = await import('@/app/api/sessions/route');

      const request = createMockRequest({
        url: 'http://localhost:3000/api/sessions',
      });

      const response = await GET(request, undefined as any);
      const body = await parseResponse(response);

      expect(response.status).toBe(401);
      expect((body as any).success).toBe(false);
      expect((body as any).error).toBeDefined();
    });

    it('accepts Authorization header token', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        json: () => Promise.resolve({ sessions: [], total: 0 }),
      });

      const { GET } = await import('@/app/api/sessions/route');

      const request = createAuthenticatedRequest({
        url: 'http://localhost:3000/api/sessions',
      });

      const response = await GET(request, undefined as any);

      expect(response.status).toBe(200);
    });

    it('accepts the admin session cookie', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        json: () => Promise.resolve({ sessions: [], total: 0 }),
      });

      const { GET } = await import('@/app/api/sessions/route');

      const request = createSessionCookieRequest({
        url: 'http://localhost:3000/api/sessions',
      });

      const response = await GET(request, undefined as any);

      expect(response.status).toBe(200);
      expect(mockFetch).toHaveBeenCalledWith(
        'https://api.sandbox.stateset.app/api/admin/agent-sessions?limit=20',
        expect.objectContaining({
          headers: expect.objectContaining({
            Authorization: 'Bearer test-session-token',
          }),
        })
      );
    });

    it('does not authenticate from STATESET_API_TOKEN alone', async () => {
      vi.stubEnv('STATESET_API_TOKEN', 'env-token');

      mockFetch.mockResolvedValue({
        ok: true,
        json: () => Promise.resolve({ sessions: [], total: 0 }),
      });

      const { GET } = await import('@/app/api/sessions/route');

      const request = createMockRequest({
        url: 'http://localhost:3000/api/sessions',
      });

      const response = await GET(request, undefined as any);
      await expectError(response, 401, 'UNAUTHORIZED');
    });

    it('returns an empty result when admin auth is disabled with no tokens', async () => {
      vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');

      const { GET } = await import('@/app/api/sessions/route');

      const request = createMockRequest({
        url: 'http://localhost:3000/api/sessions',
      });

      const response = await GET(request, undefined as any);
      const body = await expectSuccess<{ data: unknown[] }>(response);

      expect(body.data).toEqual([]);
      expect(mockFetch).not.toHaveBeenCalled();
    });

    it('uses STATESET_API_TOKEN when admin auth is disabled', async () => {
      vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');
      vi.stubEnv('STATESET_API_TOKEN', 'env-token');

      mockFetch.mockResolvedValue({
        ok: true,
        json: () => Promise.resolve({ sessions: [], total: 0 }),
      });

      const { GET } = await import('@/app/api/sessions/route');

      const request = createMockRequest({
        url: 'http://localhost:3000/api/sessions',
      });

      const response = await GET(request, undefined as any);

      expect(response.status).toBe(200);
      expect(mockFetch).toHaveBeenCalledWith(
        'https://api.sandbox.stateset.app/api/admin/agent-sessions?limit=20',
        expect.objectContaining({
          headers: expect.objectContaining({
            Authorization: 'Bearer env-token',
          }),
        })
      );
    });
  });

  describe('query parameter validation', () => {
    beforeEach(() => {
      mockFetch.mockResolvedValue({
        ok: true,
        json: () => Promise.resolve({ sessions: [], total: 0 }),
      });
    });

    it('accepts valid query params', async () => {
      const { GET } = await import('@/app/api/sessions/route');

      const request = createAuthenticatedRequest({
        url: 'http://localhost:3000/api/sessions',
        searchParams: {
          limit: '10',
          offset: '0',
          status: 'running',
        },
      });

      const response = await GET(request, undefined as any);

      expect(response.status).toBe(200);
    });

    it('rejects invalid status value', async () => {
      const { GET } = await import('@/app/api/sessions/route');

      const request = createAuthenticatedRequest({
        url: 'http://localhost:3000/api/sessions',
        searchParams: {
          status: 'invalid-status',
        },
      });

      const response = await GET(request, undefined as any);
      const body = await parseResponse(response);

      expect(response.status).toBe(422);
      expect((body as any).success).toBe(false);
    });

    it('rejects limit over 100', async () => {
      const { GET } = await import('@/app/api/sessions/route');

      const request = createAuthenticatedRequest({
        url: 'http://localhost:3000/api/sessions',
        searchParams: {
          limit: '200',
        },
      });

      const response = await GET(request, undefined as any);
      const body = await parseResponse(response);

      expect(response.status).toBe(422);
      expect((body as any).success).toBe(false);
    });

    it('rejects negative offset', async () => {
      const { GET } = await import('@/app/api/sessions/route');

      const request = createAuthenticatedRequest({
        url: 'http://localhost:3000/api/sessions',
        searchParams: {
          offset: '-1',
        },
      });

      const response = await GET(request, undefined as any);
      const body = await parseResponse(response);

      expect(response.status).toBe(422);
      expect((body as any).success).toBe(false);
    });

    it('uses default pagination when no params provided', async () => {
      const { GET } = await import('@/app/api/sessions/route');

      const request = createAuthenticatedRequest({
        url: 'http://localhost:3000/api/sessions',
      });

      const response = await GET(request, undefined as any);

      expect(response.status).toBe(200);
      // Verify the upstream fetch was called
      expect(mockFetch).toHaveBeenCalled();
    });
  });

  describe('response shape', () => {
    it('returns paginated response envelope', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        json: () =>
          Promise.resolve({
            sessions: [
              { id: 'session-1', name: 'Test', status: 'running' },
            ],
            total: 1,
          }),
      });

      const { GET } = await import('@/app/api/sessions/route');

      const request = createAuthenticatedRequest({
        url: 'http://localhost:3000/api/sessions',
      });

      const response = await GET(request, undefined as any);
      const body = await parseResponse<{
        success: boolean;
        data: unknown[];
        meta: { pagination: { total: number; limit: number; offset: number; hasMore: boolean } };
      }>(response);

      expect(body.success).toBe(true);
      expect(Array.isArray(body.data)).toBe(true);
      expect(body.meta).toBeDefined();
      expect(body.meta.pagination).toBeDefined();
      expect(body.meta.pagination.total).toBe(1);
    });

    it('handles upstream API errors', async () => {
      mockFetch.mockResolvedValue({
        ok: false,
        status: 502,
        json: () => Promise.resolve({ error: 'Bad gateway' }),
      });

      const { GET } = await import('@/app/api/sessions/route');

      const request = createAuthenticatedRequest({
        url: 'http://localhost:3000/api/sessions',
      });

      const response = await GET(request, undefined as any);
      const body = await parseResponse(response);

      expect(response.status).toBe(502);
      expect((body as any).success).toBe(false);
      expect((body as any).error.code).toBe('UPSTREAM_ERROR');
    });

    it('returns empty array when no sessions', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        json: () => Promise.resolve({ sessions: [], total: 0 }),
      });

      const { GET } = await import('@/app/api/sessions/route');

      const request = createAuthenticatedRequest({
        url: 'http://localhost:3000/api/sessions',
      });

      const response = await GET(request, undefined as any);
      const body = await parseResponse<{ data: unknown[] }>(response);

      expect(body.data).toEqual([]);
    });
  });
});

describe('GET /api/sessions/[id]', () => {
  it('returns a session with events for a valid ID', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          session: { id: 'session-1', status: 'running' },
          events: [{ id: 'event-1', type: 'started' }],
        }),
    });

    const { GET } = await import('@/app/api/sessions/[id]/route');
    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/sessions/session-1',
    });

    const response = await GET(request, { params: Promise.resolve({ id: 'session-1' }) });
    const body = await parseResponse<{ success: boolean; data: { session: { id: string } } }>(response);

    expect(response.status).toBe(200);
    expect(body.success).toBe(true);
    expect(body.data.session.id).toBe('session-1');
    expect(mockFetch).toHaveBeenCalledWith(
      'https://api.sandbox.stateset.app/api/admin/agent-sessions/session-1',
      expect.objectContaining({
        headers: expect.objectContaining({
          Authorization: 'Bearer test-token',
        }),
      })
    );
  });

  it('rejects invalid session IDs before proxying', async () => {
    const { GET } = await import('@/app/api/sessions/[id]/route');
    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/sessions/../bad',
    });

    const response = await GET(request, { params: Promise.resolve({ id: '../bad' }) });

    await expectError(response, 400, 'BAD_REQUEST');
    expect(mockFetch).not.toHaveBeenCalled();
  });

  it('returns an empty session payload when admin auth is disabled without a service token', async () => {
    vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');
    vi.stubEnv('STATESET_API_TOKEN', '');

    const { GET } = await import('@/app/api/sessions/[id]/route');
    const request = createMockRequest({
      url: 'http://localhost:3000/api/sessions/session-1',
    });

    const response = await GET(request, { params: Promise.resolve({ id: 'session-1' }) });
    const body = await parseResponse<{ success: boolean; data: { session: null; events: unknown[] } }>(response);

    expect(response.status).toBe(200);
    expect(body.success).toBe(true);
    expect(body.data.session).toBeNull();
    expect(body.data.events).toEqual([]);
    expect(mockFetch).not.toHaveBeenCalled();
  });

  it('maps upstream authorization failures to 401', async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 403,
      json: () => Promise.resolve({ error: 'Forbidden' }),
    });

    const { GET } = await import('@/app/api/sessions/[id]/route');
    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/sessions/session-1',
    });

    const response = await GET(request, { params: Promise.resolve({ id: 'session-1' }) });

    await expectError(response, 401, 'UNAUTHORIZED');
  });
});

describe('GET /api/sessions/summary', () => {
  it('returns upstream session summary data', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          total: 4,
          by_status: { running: 2, completed: 2 },
          active_now: 2,
          rotations_last_hour: 1,
          avg_duration_seconds: 42,
        }),
    });

    const { GET } = await import('@/app/api/sessions/summary/route');
    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/sessions/summary',
    });

    const response = await GET(request, undefined as any);
    const body = await parseResponse<{ success: boolean; data: { total: number } }>(response);

    expect(response.status).toBe(200);
    expect(body.success).toBe(true);
    expect(body.data.total).toBe(4);
    expect(mockFetch).toHaveBeenCalledWith(
      'https://api.sandbox.stateset.app/api/admin/agent-sessions/summary',
      expect.objectContaining({
        headers: expect.objectContaining({
          Authorization: 'Bearer test-token',
        }),
      })
    );
  });

  it('returns a zero summary when admin auth is disabled without a service token', async () => {
    vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');
    vi.stubEnv('STATESET_API_TOKEN', '');

    const { GET } = await import('@/app/api/sessions/summary/route');
    const request = createMockRequest({
      url: 'http://localhost:3000/api/sessions/summary',
    });

    const response = await GET(request, undefined as any);
    const body = await parseResponse<{ success: boolean; data: { total: number; active_now: number } }>(response);

    expect(response.status).toBe(200);
    expect(body.success).toBe(true);
    expect(body.data.total).toBe(0);
    expect(body.data.active_now).toBe(0);
    expect(mockFetch).not.toHaveBeenCalled();
  });

  it('maps upstream failures to an error envelope', async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 502,
      json: () => Promise.resolve({ error: 'Bad gateway' }),
    });

    const { GET } = await import('@/app/api/sessions/summary/route');
    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/sessions/summary',
    });

    const response = await GET(request, undefined as any);

    await expectError(response, 502, 'UPSTREAM_ERROR');
  });
});

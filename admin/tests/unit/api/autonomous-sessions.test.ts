/**
 * Tests for /api/autonomous/sessions and /api/autonomous/sessions/:id endpoints
 *
 * @module tests/unit/api/autonomous-sessions
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  createMockRequest,
  createAuthenticatedRequest,
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

// ============================================================================
// GET /api/autonomous/sessions
// ============================================================================

describe('GET /api/autonomous/sessions', () => {
  it('returns paginated list of sessions', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          sessions: [
            {
              id: 'asess-1',
              name: 'Autonomous Task 1',
              status: 'running',
              createdAt: '2026-01-28T00:00:00Z',
            },
            {
              id: 'asess-2',
              name: 'Autonomous Task 2',
              status: 'completed',
              createdAt: '2026-01-27T00:00:00Z',
            },
          ],
          total: 2,
        }),
    });

    const { GET } = await import('@/app/api/autonomous/sessions/route');

    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/autonomous/sessions',
    });

    const response = await GET(request, undefined as any);
    const body = await parseResponse<{
      success: boolean;
      data: unknown[];
      meta: { pagination: { total: number; limit: number; offset: number; hasMore: boolean } };
    }>(response);

    expect(response.status).toBe(200);
    expect(body.success).toBe(true);
    expect(Array.isArray(body.data)).toBe(true);
    expect(body.data).toHaveLength(2);
    expect(body.meta.pagination.total).toBe(2);
  });

  it('uses default pagination when no params provided', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          sessions: [],
          total: 0,
        }),
    });

    const { GET } = await import('@/app/api/autonomous/sessions/route');

    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/autonomous/sessions',
    });

    const response = await GET(request, undefined as any);
    const body = await parseResponse<{
      success: boolean;
      data: unknown[];
      meta: { pagination: { total: number; limit: number; offset: number; hasMore: boolean } };
    }>(response);

    expect(response.status).toBe(200);
    expect(body.meta.pagination.limit).toBe(20);
    expect(body.meta.pagination.offset).toBe(0);
  });

  it('respects custom pagination params', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          sessions: [{ id: 'asess-3', name: 'Task 3', status: 'paused' }],
          total: 10,
        }),
    });

    const { GET } = await import('@/app/api/autonomous/sessions/route');

    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/autonomous/sessions',
      searchParams: { limit: '5', offset: '5' },
    });

    const response = await GET(request, undefined as any);
    const body = await parseResponse<{
      success: boolean;
      data: unknown[];
      meta: { pagination: { total: number; limit: number; offset: number; hasMore: boolean } };
    }>(response);

    expect(response.status).toBe(200);
    expect(body.meta.pagination.limit).toBe(5);
    expect(body.meta.pagination.offset).toBe(5);
    expect(body.meta.pagination.hasMore).toBe(false);
  });

  it('returns 401 without auth', async () => {
    vi.stubEnv('STATESET_API_TOKEN', '');

    const { GET } = await import('@/app/api/autonomous/sessions/route');

    const request = createMockRequest({
      url: 'http://localhost:3000/api/autonomous/sessions',
    });

    const response = await GET(request, undefined as any);
    await expectError(response, 401, 'UNAUTHORIZED');
  });

  it('rejects invalid pagination params', async () => {
    const { GET } = await import('@/app/api/autonomous/sessions/route');

    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/autonomous/sessions',
      searchParams: { limit: '999' },
    });

    const response = await GET(request, undefined as any);
    await expectError(response, 422, 'VALIDATION_ERROR');
  });

  it('returns empty array when no sessions exist', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          sessions: [],
          total: 0,
        }),
    });

    const { GET } = await import('@/app/api/autonomous/sessions/route');

    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/autonomous/sessions',
    });

    const response = await GET(request, undefined as any);
    const body = await parseResponse<{ data: unknown[] }>(response);

    expect(body.data).toEqual([]);
  });

  it('handles upstream API errors', async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 502,
      json: () => Promise.resolve({ error: 'Bad gateway' }),
    });

    const { GET } = await import('@/app/api/autonomous/sessions/route');

    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/autonomous/sessions',
    });

    const response = await GET(request, undefined as any);
    await expectError(response, 502, 'AUTONOMOUS_ERROR');
  });
});

// ============================================================================
// POST /api/autonomous/sessions
// ============================================================================

describe('POST /api/autonomous/sessions', () => {
  it('creates session with valid body', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          id: 'asess-new',
          name: 'My New Task',
          status: 'pending',
          createdAt: '2026-01-28T00:00:00Z',
        }),
    });

    const { POST } = await import('@/app/api/autonomous/sessions/route');

    const request = createAuthenticatedRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/autonomous/sessions',
      body: {
        name: 'My New Task',
        description: 'Run automated data processing',
      },
    });

    const response = await POST(request, undefined as any);
    const body = await expectSuccess(response, 201);

    expect(body.data).toHaveProperty('id', 'asess-new');
    expect(body.data).toHaveProperty('name', 'My New Task');
  });

  it('creates session with budgetConfig', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          id: 'asess-budget',
          name: 'Budget Task',
          status: 'pending',
          budgetConfig: { costCapCents: 5000, iterationLimit: 100 },
        }),
    });

    const { POST } = await import('@/app/api/autonomous/sessions/route');

    const request = createAuthenticatedRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/autonomous/sessions',
      body: {
        name: 'Budget Task',
        budgetConfig: {
          costCapCents: 5000,
          iterationLimit: 100,
        },
      },
    });

    const response = await POST(request, undefined as any);
    const body = await expectSuccess(response, 201);

    expect(body.data).toHaveProperty('id', 'asess-budget');
  });

  it('creates session with only required name field', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          id: 'asess-minimal',
          name: 'Minimal Task',
          status: 'pending',
        }),
    });

    const { POST } = await import('@/app/api/autonomous/sessions/route');

    const request = createAuthenticatedRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/autonomous/sessions',
      body: {
        name: 'Minimal Task',
      },
    });

    const response = await POST(request, undefined as any);

    expect(response.status).toBe(201);
  });

  it('returns 422 when name is missing', async () => {
    const { POST } = await import('@/app/api/autonomous/sessions/route');

    const request = createAuthenticatedRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/autonomous/sessions',
      body: {
        description: 'Some description without a name',
      },
    });

    const response = await POST(request, undefined as any);
    await expectError(response, 422, 'VALIDATION_ERROR');
  });

  it('returns 422 with empty body', async () => {
    const { POST } = await import('@/app/api/autonomous/sessions/route');

    const request = createAuthenticatedRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/autonomous/sessions',
      body: {},
    });

    const response = await POST(request, undefined as any);
    await expectError(response, 422, 'VALIDATION_ERROR');
  });

  it('returns 401 without auth', async () => {
    vi.stubEnv('STATESET_API_TOKEN', '');

    const { POST } = await import('@/app/api/autonomous/sessions/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/autonomous/sessions',
      body: { name: 'Test Task' },
    });

    const response = await POST(request, undefined as any);
    await expectError(response, 401, 'UNAUTHORIZED');
  });
});

// ============================================================================
// GET /api/autonomous/sessions/:id
// ============================================================================

describe('GET /api/autonomous/sessions/:id', () => {
  it('returns session details for valid ID', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          id: 'asess-1',
          name: 'Autonomous Task 1',
          status: 'running',
          description: 'Processing data',
          budgetConfig: { costCapCents: 1000, iterationLimit: 50 },
          budgetConsumed: { costCents: 100, iterations: 5, durationSeconds: 120 },
          createdAt: '2026-01-28T00:00:00Z',
          startedAt: '2026-01-28T00:01:00Z',
        }),
    });

    const { GET } = await import('@/app/api/autonomous/sessions/[id]/route');

    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/autonomous/sessions/asess-1',
    });

    const response = await GET(request, { params: Promise.resolve({ id: 'asess-1' }) });
    const body = await expectSuccess(response);

    expect(body.data).toHaveProperty('id', 'asess-1');
    expect(body.data).toHaveProperty('status', 'running');
  });

  it('returns 404 for unknown session ID', async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 404,
      json: () => Promise.resolve({ error: 'Session not found' }),
    });

    const { GET } = await import('@/app/api/autonomous/sessions/[id]/route');

    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/autonomous/sessions/nonexistent',
    });

    const response = await GET(request, { params: Promise.resolve({ id: 'nonexistent' }) });
    await expectError(response, 404, 'NOT_FOUND');
  });

  it('returns 401 without auth', async () => {
    vi.stubEnv('STATESET_API_TOKEN', '');

    const { GET } = await import('@/app/api/autonomous/sessions/[id]/route');

    const request = createMockRequest({
      url: 'http://localhost:3000/api/autonomous/sessions/asess-1',
    });

    const response = await GET(request, { params: Promise.resolve({ id: 'asess-1' }) });
    await expectError(response, 401, 'UNAUTHORIZED');
  });

  it('calls upstream with correct session ID', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          id: 'asess-42',
          name: 'Task 42',
          status: 'completed',
        }),
    });

    const { GET } = await import('@/app/api/autonomous/sessions/[id]/route');

    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/autonomous/sessions/asess-42',
    });

    await GET(request, { params: Promise.resolve({ id: 'asess-42' }) });

    expect(mockFetch).toHaveBeenCalledWith(
      'https://api.sandbox.stateset.app/api/autonomous/sessions/asess-42',
      expect.objectContaining({
        method: 'GET',
      })
    );
  });
});

// ============================================================================
// POST /api/autonomous/sessions/:id (session actions)
// ============================================================================

describe('POST /api/autonomous/sessions/:id (actions)', () => {
  describe('start action', () => {
    it('starts a session successfully', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        json: () =>
          Promise.resolve({
            id: 'asess-1',
            status: 'running',
            startedAt: '2026-01-28T00:01:00Z',
          }),
      });

      const { POST } = await import('@/app/api/autonomous/sessions/[id]/route');

      const request = createAuthenticatedRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/autonomous/sessions/asess-1',
        body: { action: 'start' },
      });

      const response = await POST(request, { params: Promise.resolve({ id: 'asess-1' }) });
      const body = await expectSuccess(response);

      expect(body.data).toHaveProperty('status', 'running');
    });

    it('calls upstream start endpoint', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        json: () =>
          Promise.resolve({
            id: 'asess-1',
            status: 'running',
          }),
      });

      const { POST } = await import('@/app/api/autonomous/sessions/[id]/route');

      const request = createAuthenticatedRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/autonomous/sessions/asess-1',
        body: { action: 'start' },
      });

      await POST(request, { params: Promise.resolve({ id: 'asess-1' }) });

      expect(mockFetch).toHaveBeenCalledWith(
        'https://api.sandbox.stateset.app/api/autonomous/sessions/asess-1/start',
        expect.objectContaining({
          method: 'POST',
        })
      );
    });
  });

  describe('pause action', () => {
    it('pauses a session successfully', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        json: () =>
          Promise.resolve({
            id: 'asess-1',
            status: 'paused',
          }),
      });

      const { POST } = await import('@/app/api/autonomous/sessions/[id]/route');

      const request = createAuthenticatedRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/autonomous/sessions/asess-1',
        body: { action: 'pause' },
      });

      const response = await POST(request, { params: Promise.resolve({ id: 'asess-1' }) });
      const body = await expectSuccess(response);

      expect(body.data).toHaveProperty('status', 'paused');
    });
  });

  describe('cancel action', () => {
    it('cancels a session successfully', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        json: () =>
          Promise.resolve({
            id: 'asess-1',
            status: 'cancelled',
          }),
      });

      const { POST } = await import('@/app/api/autonomous/sessions/[id]/route');

      const request = createAuthenticatedRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/autonomous/sessions/asess-1',
        body: { action: 'cancel' },
      });

      const response = await POST(request, { params: Promise.resolve({ id: 'asess-1' }) });
      const body = await expectSuccess(response);

      expect(body.data).toHaveProperty('status', 'cancelled');
    });
  });

  describe('invalid state transitions', () => {
    it('returns 409 when action conflicts with current state', async () => {
      mockFetch.mockResolvedValue({
        ok: false,
        status: 409,
        json: () =>
          Promise.resolve({
            error: 'Cannot start session in completed state',
          }),
      });

      const { POST } = await import('@/app/api/autonomous/sessions/[id]/route');

      const request = createAuthenticatedRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/autonomous/sessions/asess-done',
        body: { action: 'start' },
      });

      const response = await POST(request, { params: Promise.resolve({ id: 'asess-done' }) });
      await expectError(response, 409, 'CONFLICT');
    });
  });

  describe('validation', () => {
    it('returns 422 for invalid action', async () => {
      const { POST } = await import('@/app/api/autonomous/sessions/[id]/route');

      const request = createAuthenticatedRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/autonomous/sessions/asess-1',
        body: { action: 'invalid-action' },
      });

      const response = await POST(request, { params: Promise.resolve({ id: 'asess-1' }) });
      await expectError(response, 422, 'VALIDATION_ERROR');
    });

    it('returns 422 when action is missing', async () => {
      const { POST } = await import('@/app/api/autonomous/sessions/[id]/route');

      const request = createAuthenticatedRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/autonomous/sessions/asess-1',
        body: {},
      });

      const response = await POST(request, { params: Promise.resolve({ id: 'asess-1' }) });
      await expectError(response, 422, 'VALIDATION_ERROR');
    });
  });

  describe('error handling', () => {
    it('returns 404 when session not found', async () => {
      mockFetch.mockResolvedValue({
        ok: false,
        status: 404,
        json: () => Promise.resolve({ error: 'Session not found' }),
      });

      const { POST } = await import('@/app/api/autonomous/sessions/[id]/route');

      const request = createAuthenticatedRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/autonomous/sessions/nonexistent',
        body: { action: 'start' },
      });

      const response = await POST(request, { params: Promise.resolve({ id: 'nonexistent' }) });
      await expectError(response, 404, 'NOT_FOUND');
    });

    it('returns 401 without auth', async () => {
      vi.stubEnv('STATESET_API_TOKEN', '');

      const { POST } = await import('@/app/api/autonomous/sessions/[id]/route');

      const request = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/autonomous/sessions/asess-1',
        body: { action: 'start' },
      });

      const response = await POST(request, { params: Promise.resolve({ id: 'asess-1' }) });
      await expectError(response, 401, 'UNAUTHORIZED');
    });
  });
});

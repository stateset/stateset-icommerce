/**
 * Tests for /api/integrations/credentials and /api/integrations/status endpoints
 *
 * @module tests/unit/api/integrations
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
// GET /api/integrations/credentials
// ============================================================================

describe('GET /api/integrations/credentials', () => {
  it('returns paginated list with auth', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          credentials: [
            { id: 'cred-1', provider: 'shopify', name: 'My Shopify Store' },
            { id: 'cred-2', provider: 'stripe', name: 'Stripe Production' },
          ],
          total: 2,
        }),
    });

    const { GET } = await import('@/app/api/integrations/credentials/route');

    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/integrations/credentials',
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
    expect(body.meta.pagination).toBeDefined();
    expect(body.meta.pagination.total).toBe(2);
  });

  it('uses default pagination when no params provided', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          credentials: [],
          total: 0,
        }),
    });

    const { GET } = await import('@/app/api/integrations/credentials/route');

    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/integrations/credentials',
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
          credentials: [{ id: 'cred-3', provider: 'zendesk', name: 'ZD' }],
          total: 5,
        }),
    });

    const { GET } = await import('@/app/api/integrations/credentials/route');

    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/integrations/credentials',
      searchParams: { limit: '1', offset: '2' },
    });

    const response = await GET(request, undefined as any);
    const body = await parseResponse<{
      success: boolean;
      data: unknown[];
      meta: { pagination: { total: number; limit: number; offset: number; hasMore: boolean } };
    }>(response);

    expect(response.status).toBe(200);
    expect(body.meta.pagination.limit).toBe(1);
    expect(body.meta.pagination.offset).toBe(2);
    expect(body.meta.pagination.hasMore).toBe(true);
  });

  it('returns 401 without auth', async () => {
    vi.stubEnv('STATESET_API_TOKEN', '');

    const { GET } = await import('@/app/api/integrations/credentials/route');

    const request = createMockRequest({
      url: 'http://localhost:3000/api/integrations/credentials',
    });

    const response = await GET(request, undefined as any);
    await expectError(response, 401, 'UNAUTHORIZED');
  });

  it('rejects invalid pagination params', async () => {
    const { GET } = await import('@/app/api/integrations/credentials/route');

    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/integrations/credentials',
      searchParams: { limit: '200' },
    });

    const response = await GET(request, undefined as any);
    await expectError(response, 422, 'VALIDATION_ERROR');
  });
});

// ============================================================================
// POST /api/integrations/credentials
// ============================================================================

describe('POST /api/integrations/credentials', () => {
  it('creates credential with valid body', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          id: 'cred-new',
          provider: 'shopify',
          name: 'New Store',
          createdAt: '2026-01-28T00:00:00Z',
        }),
    });

    const { POST } = await import('@/app/api/integrations/credentials/route');

    const request = createAuthenticatedRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/integrations/credentials',
      body: {
        provider: 'shopify',
        credentials: { apiKey: 'key-123', secret: 'secret-456' },
        name: 'New Store',
      },
    });

    const response = await POST(request, undefined as any);
    const body = await expectSuccess(response, 201);

    expect(body.data).toHaveProperty('id', 'cred-new');
    expect(body.data).toHaveProperty('provider', 'shopify');
  });

  it('creates credential without optional name', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          id: 'cred-new',
          provider: 'stripe',
          createdAt: '2026-01-28T00:00:00Z',
        }),
    });

    const { POST } = await import('@/app/api/integrations/credentials/route');

    const request = createAuthenticatedRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/integrations/credentials',
      body: {
        provider: 'stripe',
        credentials: { apiKey: 'sk_test_123' },
      },
    });

    const response = await POST(request, undefined as any);

    expect(response.status).toBe(201);
  });

  it('returns 422 when provider is missing', async () => {
    const { POST } = await import('@/app/api/integrations/credentials/route');

    const request = createAuthenticatedRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/integrations/credentials',
      body: {
        credentials: { apiKey: 'key-123' },
      },
    });

    const response = await POST(request, undefined as any);
    await expectError(response, 422, 'VALIDATION_ERROR');
  });

  it('returns 422 when credentials object is missing', async () => {
    const { POST } = await import('@/app/api/integrations/credentials/route');

    const request = createAuthenticatedRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/integrations/credentials',
      body: {
        provider: 'shopify',
      },
    });

    const response = await POST(request, undefined as any);
    await expectError(response, 422, 'VALIDATION_ERROR');
  });

  it('returns 401 without auth', async () => {
    vi.stubEnv('STATESET_API_TOKEN', '');

    const { POST } = await import('@/app/api/integrations/credentials/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/integrations/credentials',
      body: {
        provider: 'shopify',
        credentials: { apiKey: 'key-123' },
      },
    });

    const response = await POST(request, undefined as any);
    await expectError(response, 401, 'UNAUTHORIZED');
  });
});

// ============================================================================
// DELETE /api/integrations/credentials
// ============================================================================

describe('DELETE /api/integrations/credentials', () => {
  it('deletes credential by ID', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ deleted: true }),
    });

    const { DELETE } = await import('@/app/api/integrations/credentials/route');

    const request = createAuthenticatedRequest({
      method: 'DELETE',
      url: 'http://localhost:3000/api/integrations/credentials',
      searchParams: { id: 'cred-1' },
    });

    const response = await DELETE(request, undefined as any);
    const body = await expectSuccess(response);

    expect(body.data).toHaveProperty('message', 'Credential deleted successfully');
  });

  it('calls upstream with correct credential ID', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ deleted: true }),
    });

    const { DELETE } = await import('@/app/api/integrations/credentials/route');

    const request = createAuthenticatedRequest({
      method: 'DELETE',
      url: 'http://localhost:3000/api/integrations/credentials',
      searchParams: { id: 'cred-42' },
    });

    await DELETE(request, undefined as any);

    expect(mockFetch).toHaveBeenCalledWith(
      'https://api.sandbox.stateset.app/api/integrations/credentials/cred-42',
      expect.objectContaining({
        method: 'DELETE',
      })
    );
  });

  it('returns 400 when ID is missing', async () => {
    const { DELETE } = await import('@/app/api/integrations/credentials/route');

    const request = createAuthenticatedRequest({
      method: 'DELETE',
      url: 'http://localhost:3000/api/integrations/credentials',
    });

    const response = await DELETE(request, undefined as any);
    await expectError(response, 400, 'BAD_REQUEST');
  });

  it('returns 401 without auth', async () => {
    vi.stubEnv('STATESET_API_TOKEN', '');

    const { DELETE } = await import('@/app/api/integrations/credentials/route');

    const request = createMockRequest({
      method: 'DELETE',
      url: 'http://localhost:3000/api/integrations/credentials',
      searchParams: { id: 'cred-1' },
    });

    const response = await DELETE(request, undefined as any);
    await expectError(response, 401, 'UNAUTHORIZED');
  });

  it('handles upstream deletion error', async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 404,
      json: () => Promise.resolve({ error: 'Credential not found' }),
    });

    const { DELETE } = await import('@/app/api/integrations/credentials/route');

    const request = createAuthenticatedRequest({
      method: 'DELETE',
      url: 'http://localhost:3000/api/integrations/credentials',
      searchParams: { id: 'nonexistent' },
    });

    const response = await DELETE(request, undefined as any);
    await expectError(response, 404, 'INTEGRATION_ERROR');
  });
});

// ============================================================================
// GET /api/integrations/status
// ============================================================================

describe('GET /api/integrations/status', () => {
  it('returns integration statuses with auth', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          integrations: [
            { provider: 'shopify', status: 'connected', lastSync: '2026-01-28T00:00:00Z' },
            { provider: 'stripe', status: 'connected', lastSync: '2026-01-28T00:00:00Z' },
            { provider: 'zendesk', status: 'disconnected', lastSync: null },
          ],
        }),
    });

    const { GET } = await import('@/app/api/integrations/status/route');

    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/integrations/status',
    });

    const response = await GET(request, undefined as any);
    const body = await expectSuccess(response);

    expect(body.data).toHaveProperty('integrations');
  });

  it('returns 401 without auth', async () => {
    vi.stubEnv('STATESET_API_TOKEN', '');

    const { GET } = await import('@/app/api/integrations/status/route');

    const request = createMockRequest({
      url: 'http://localhost:3000/api/integrations/status',
    });

    const response = await GET(request, undefined as any);
    await expectError(response, 401, 'UNAUTHORIZED');
  });

  it('handles upstream API errors', async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 503,
      json: () => Promise.resolve({ error: 'Service unavailable' }),
    });

    const { GET } = await import('@/app/api/integrations/status/route');

    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/integrations/status',
    });

    const response = await GET(request, undefined as any);
    await expectError(response, 503, 'INTEGRATION_ERROR');
  });

  it('does not authenticate from env token alone', async () => {
    vi.stubEnv('STATESET_API_TOKEN', 'env-token');

    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          integrations: [],
        }),
    });

    const { GET } = await import('@/app/api/integrations/status/route');

    const request = createMockRequest({
      url: 'http://localhost:3000/api/integrations/status',
    });

    const response = await GET(request, undefined as any);
    await expectError(response, 401, 'UNAUTHORIZED');
    expect(mockFetch).not.toHaveBeenCalled();
  });
});

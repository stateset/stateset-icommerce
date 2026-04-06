import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  createMockRequest,
  createSessionCookieRequest,
  expectError,
  expectSuccess,
} from '../../helpers/api-test-utils';

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

const mockFetch = vi.fn();

beforeEach(() => {
  vi.stubGlobal('fetch', mockFetch);
  vi.stubEnv('STATESET_API_URL', 'https://api.sandbox.stateset.app');
  vi.stubEnv('GATEWAY_URL', 'http://127.0.0.1:8080');
  vi.stubEnv('GATEWAY_API_KEY', 'gateway-secret');
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.unstubAllEnvs();
});

describe('GET /api/gateway/[...path]', () => {
  it('rejects anonymous access', async () => {
    const { GET } = await import('@/app/api/gateway/[...path]/route');

    const request = createMockRequest({
      url: 'http://localhost:3000/api/gateway/health',
    });

    const response = await GET(request, {
      params: Promise.resolve({ path: ['health'] } as any),
    });

    await expectError(response, 401, 'UNAUTHORIZED');
    expect(mockFetch).not.toHaveBeenCalled();
  });

  it('requires a valid admin session before proxying to the gateway', async () => {
    mockFetch.mockImplementation(async (input: string, init?: RequestInit) => {
      if (input === 'https://api.sandbox.stateset.app/api/auth/me') {
        expect(init?.headers).toMatchObject({
          Authorization: 'Bearer test-session-token',
        });
        return {
          ok: true,
          status: 200,
          json: async () => ({ user: { id: 'user-1' } }),
        };
      }

      if (input === 'http://127.0.0.1:8080/health') {
        expect(init?.headers).toMatchObject({
          'Content-Type': 'application/json',
          Authorization: 'Bearer gateway-secret',
        });
        return {
          ok: true,
          status: 200,
          json: async () => ({ status: 'ok' }),
        };
      }

      throw new Error(`Unexpected fetch: ${input}`);
    });

    const { GET } = await import('@/app/api/gateway/[...path]/route');

    const request = createSessionCookieRequest({
      url: 'http://localhost:3000/api/gateway/health',
    });

    const response = await GET(request, {
      params: Promise.resolve({ path: ['health'] } as any),
    });
    const body = await expectSuccess<{ status: string }>(response);

    expect(body.data).toEqual({ status: 'ok' });
    expect(mockFetch).toHaveBeenCalledTimes(2);
  });
});

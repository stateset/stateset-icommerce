/**
 * Tests for GET /api/health endpoint
 *
 * @module tests/unit/api/health
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// We need to mock fetch globally since the health endpoint calls external services
const mockFetch = vi.fn();

beforeEach(() => {
  vi.stubGlobal('fetch', mockFetch);
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe('GET /api/health', () => {
  beforeEach(() => {
    // Clear env vars for deterministic tests
    vi.stubEnv('UPSTASH_REDIS_REST_URL', '');
    vi.stubEnv('UPSTASH_REDIS_REST_TOKEN', '');
    vi.stubEnv('STATESET_API_URL', 'https://api.sandbox.stateset.app');
    vi.stubEnv('npm_package_version', '9.9.9-test');
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('returns a JSON response', async () => {
    // Mock both dependency checks
    mockFetch.mockImplementation((url: string) => {
      if (typeof url === 'string' && url.includes('/health')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve({ status: 'healthy' }),
        });
      }
      return Promise.resolve({
        ok: true,
        json: () => Promise.resolve({ result: 'PONG' }),
      });
    });

    // Dynamic import to ensure mocks are applied
    const { GET } = await import('@/app/api/health/route');
    const response = await GET();
    const body = await response.json();

    expect(body).toBeDefined();
    expect(typeof body).toBe('object');
  });

  it('includes status field in response', async () => {
    mockFetch.mockImplementation((url: string) => {
      if (typeof url === 'string' && url.includes('/health')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve({ status: 'healthy' }),
        });
      }
      return Promise.resolve({
        ok: true,
        json: () => Promise.resolve({ result: 'PONG' }),
      });
    });

    const { GET } = await import('@/app/api/health/route');
    const response = await GET();
    const body = await response.json();

    expect(body.status).toBeDefined();
    expect(['healthy', 'degraded', 'unhealthy']).toContain(body.status);
  });

  it('includes version in response', async () => {
    mockFetch.mockImplementation(() =>
      Promise.resolve({
        ok: true,
        json: () => Promise.resolve({ result: 'PONG' }),
      })
    );

    const { GET } = await import('@/app/api/health/route');
    const response = await GET();
    const body = await response.json();

    expect(body.version).toBeDefined();
    expect(body.version).toBe('9.9.9-test');
  });

  it('includes uptime in response', async () => {
    mockFetch.mockImplementation(() =>
      Promise.resolve({
        ok: true,
        json: () => Promise.resolve({ result: 'PONG' }),
      })
    );

    const { GET } = await import('@/app/api/health/route');
    const response = await GET();
    const body = await response.json();

    expect(body.uptime).toBeDefined();
    expect(typeof body.uptime).toBe('number');
    expect(body.uptime).toBeGreaterThanOrEqual(0);
  });

  it('includes timestamp in response', async () => {
    mockFetch.mockImplementation(() =>
      Promise.resolve({
        ok: true,
        json: () => Promise.resolve({ result: 'PONG' }),
      })
    );

    const { GET } = await import('@/app/api/health/route');
    const response = await GET();
    const body = await response.json();

    expect(body.timestamp).toBeDefined();
    const date = new Date(body.timestamp);
    expect(date.toISOString()).toBe(body.timestamp);
  });

  it('includes dependencies array in response', async () => {
    mockFetch.mockImplementation((url: string) => {
      if (typeof url === 'string' && url.includes('/health')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve({ status: 'healthy' }),
        });
      }
      return Promise.resolve({
        ok: true,
        json: () => Promise.resolve({ result: 'PONG' }),
      });
    });

    const { GET } = await import('@/app/api/health/route');
    const response = await GET();
    const body = await response.json();

    expect(body.dependencies).toBeDefined();
    expect(Array.isArray(body.dependencies)).toBe(true);
    expect(body.dependencies.length).toBeGreaterThanOrEqual(1);
  });

  it('each dependency has name and status', async () => {
    mockFetch.mockImplementation((url: string) => {
      if (typeof url === 'string' && url.includes('/health')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve({ status: 'healthy' }),
        });
      }
      return Promise.resolve({
        ok: true,
        json: () => Promise.resolve({ result: 'PONG' }),
      });
    });

    const { GET } = await import('@/app/api/health/route');
    const response = await GET();
    const body = await response.json();

    for (const dep of body.dependencies) {
      expect(dep.name).toBeDefined();
      expect(dep.status).toBeDefined();
      expect(['healthy', 'unhealthy', 'degraded']).toContain(dep.status);
    }
  });

  it('returns HTTP 200 when dependencies are healthy', async () => {
    mockFetch.mockImplementation((url: string) => {
      if (typeof url === 'string' && url.includes('/health')) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve({ status: 'healthy' }),
        });
      }
      return Promise.resolve({
        ok: true,
        json: () => Promise.resolve({ result: 'PONG' }),
      });
    });

    // Set Redis env so it does not report as degraded
    vi.stubEnv('UPSTASH_REDIS_REST_URL', 'https://redis.upstash.io');
    vi.stubEnv('UPSTASH_REDIS_REST_TOKEN', 'test-token');

    const { GET } = await import('@/app/api/health/route');
    const response = await GET();

    expect(response.status).toBe(200);
  });

  it('returns HTTP 503 when a dependency is unhealthy', async () => {
    mockFetch.mockImplementation((url: string) => {
      if (typeof url === 'string' && url.includes('/health')) {
        return Promise.reject(new Error('Connection refused'));
      }
      return Promise.reject(new Error('Connection refused'));
    });

    // Set Redis env so it actually tries to connect (and fails)
    vi.stubEnv('UPSTASH_REDIS_REST_URL', 'https://redis.upstash.io');
    vi.stubEnv('UPSTASH_REDIS_REST_TOKEN', 'test-token');

    const { GET } = await import('@/app/api/health/route');
    const response = await GET();

    expect(response.status).toBe(503);
  });

  it('reports redis as degraded when not configured', async () => {
    vi.stubEnv('UPSTASH_REDIS_REST_URL', '');
    vi.stubEnv('UPSTASH_REDIS_REST_TOKEN', '');

    mockFetch.mockImplementation(() =>
      Promise.resolve({
        ok: true,
        json: () => Promise.resolve({ status: 'healthy' }),
      })
    );

    const { GET } = await import('@/app/api/health/route');
    const response = await GET();
    const body = await response.json();

    const redisDep = body.dependencies.find(
      (d: { name: string }) => d.name === 'redis'
    );
    expect(redisDep).toBeDefined();
    expect(redisDep.status).toBe('degraded');
  });

  it('has the expected response shape', async () => {
    mockFetch.mockImplementation(() =>
      Promise.resolve({
        ok: true,
        json: () => Promise.resolve({ result: 'PONG' }),
      })
    );

    const { GET } = await import('@/app/api/health/route');
    const response = await GET();
    const body = await response.json();

    expect(body).toHaveProperty('status');
    expect(body).toHaveProperty('version');
    expect(body).toHaveProperty('uptime');
    expect(body).toHaveProperty('timestamp');
    expect(body).toHaveProperty('dependencies');
  });
});

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { NextRequest } from 'next/server';
import { middleware } from '@/middleware';
import { apiRateLimiter, authRateLimiter } from '@/lib/shared/rate-limit';

describe('admin middleware', () => {
  beforeEach(() => {
    // Reset rate limiters between tests so they don't interfere
    apiRateLimiter.reset('127.0.0.1');
    authRateLimiter.reset('127.0.0.1');
    apiRateLimiter.reset('unknown');
    authRateLimiter.reset('unknown');
  });

  afterEach(() => {
    apiRateLimiter.reset('127.0.0.1');
    authRateLimiter.reset('127.0.0.1');
    apiRateLimiter.reset('unknown');
    authRateLimiter.reset('unknown');
    vi.unstubAllEnvs();
  });

  it('redirects anonymous page requests to the root gate', async () => {
    const request = new NextRequest('http://localhost:3000/orders');
    const response = await middleware(request);

    expect(response.status).toBe(307);
    expect(response.headers.get('location')).toBe('http://localhost:3000/');
  });

  it('rejects anonymous protected API requests', async () => {
    const request = new NextRequest('http://localhost:3000/api/sessions');
    const response = await middleware(request);
    const body = await response.json();

    expect(response.status).toBe(401);
    expect(body).toMatchObject({
      success: false,
      error: {
        code: 'UNAUTHORIZED',
      },
    });
  });

  it('allows protected API requests with a bearer token header', async () => {
    const request = new NextRequest('http://localhost:3000/api/sessions', {
      headers: {
        Authorization: 'Bearer test-header-token',
      },
    });
    const response = await middleware(request);

    expect(response.status).toBe(200);
  });

  it('allows public auth routes without a session cookie', async () => {
    const request = new NextRequest('http://localhost:3000/api/auth/login');
    const response = await middleware(request);

    expect(response.status).toBe(200);
  });

  it('allows anonymous page requests when admin auth is disabled', async () => {
    vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');
    const request = new NextRequest('http://localhost:3000/orders');
    const response = await middleware(request);

    expect(response.status).toBe(200);
  });

  it('allows anonymous protected API requests when admin auth is disabled', async () => {
    vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');
    const request = new NextRequest('http://localhost:3000/api/sessions');
    const response = await middleware(request);

    expect(response.status).toBe(200);
  });

  it('ignores auth-disable mode in production and still protects API routes', async () => {
    vi.stubEnv('NODE_ENV', 'production');
    vi.stubEnv('STATESET_ADMIN_DISABLE_AUTH', 'true');
    const request = new NextRequest('http://localhost:3000/api/sessions');
    const response = await middleware(request);
    const body = await response.json();

    expect(response.status).toBe(401);
    expect(body).toMatchObject({
      success: false,
      error: {
        code: 'UNAUTHORIZED',
      },
    });
  });

  describe('rate limiting', () => {
    it('returns 429 after exceeding the API rate limit', async () => {
      const ip = '127.0.0.1';

      // Exhaust the API limit (100 requests)
      for (let i = 0; i < 100; i++) {
        apiRateLimiter.consume(ip);
      }

      const request = new NextRequest('http://localhost:3000/api/sessions', {
        headers: { 'x-forwarded-for': ip },
      });
      const response = await middleware(request);
      const body = await response.json();

      expect(response.status).toBe(429);
      expect(body).toMatchObject({
        success: false,
        error: {
          message: 'Rate limit exceeded',
          code: 'RATE_LIMITED',
        },
      });
    });

    it('sets Retry-After header on 429 responses', async () => {
      const ip = '127.0.0.1';

      for (let i = 0; i < 100; i++) {
        apiRateLimiter.consume(ip);
      }

      const request = new NextRequest('http://localhost:3000/api/sessions', {
        headers: { 'x-forwarded-for': ip },
      });
      const response = await middleware(request);

      const retryAfter = response.headers.get('Retry-After');
      expect(retryAfter).toBeTruthy();
      expect(Number(retryAfter)).toBeGreaterThan(0);
      expect(Number(retryAfter)).toBeLessThanOrEqual(60);
    });

    it('sets X-RateLimit-Limit and X-RateLimit-Remaining headers on 429', async () => {
      const ip = '127.0.0.1';

      for (let i = 0; i < 100; i++) {
        apiRateLimiter.consume(ip);
      }

      const request = new NextRequest('http://localhost:3000/api/sessions', {
        headers: { 'x-forwarded-for': ip },
      });
      const response = await middleware(request);

      expect(response.headers.get('X-RateLimit-Limit')).toBe('100');
      expect(response.headers.get('X-RateLimit-Remaining')).toBe('0');
    });

    it('auth endpoints use the stricter 10/min limit', async () => {
      const ip = '127.0.0.1';

      // Exhaust the auth limit (10 requests)
      for (let i = 0; i < 10; i++) {
        authRateLimiter.consume(ip);
      }

      const request = new NextRequest('http://localhost:3000/api/auth/login', {
        headers: { 'x-forwarded-for': ip },
      });
      const response = await middleware(request);
      const body = await response.json();

      expect(response.status).toBe(429);
      expect(response.headers.get('X-RateLimit-Limit')).toBe('10');
      expect(body).toMatchObject({
        success: false,
        error: {
          code: 'RATE_LIMITED',
        },
      });
    });
  });
});

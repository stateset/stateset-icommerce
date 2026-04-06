/**
 * Tests for GET /api/metrics endpoint
 *
 * @module tests/unit/api/metrics
 */

import { describe, it, expect, vi } from 'vitest';

describe('GET /api/metrics', () => {
  it('returns Prometheus text format', async () => {
    const { GET } = await import('@/app/api/metrics/route');
    const response = await GET();

    expect(response.headers.get('Content-Type')).toContain('text/plain');
  });

  it('returns HTTP 200', async () => {
    const { GET } = await import('@/app/api/metrics/route');
    const response = await GET();

    expect(response.status).toBe(200);
  });

  it('sets no-cache headers', async () => {
    const { GET } = await import('@/app/api/metrics/route');
    const response = await GET();

    const cacheControl = response.headers.get('Cache-Control');
    expect(cacheControl).toContain('no-cache');
    expect(cacheControl).toContain('no-store');
  });

  it('body contains app_info metric', async () => {
    const { GET } = await import('@/app/api/metrics/route');
    const response = await GET();
    const body = await response.text();

    expect(body).toContain('app_info');
  });

  it('body contains http_requests_total metric family', async () => {
    const { GET } = await import('@/app/api/metrics/route');
    const response = await GET();
    const body = await response.text();

    expect(body).toContain('# TYPE http_requests_total counter');
  });

  it('body contains process_uptime_seconds', async () => {
    const { GET } = await import('@/app/api/metrics/route');
    const response = await GET();
    const body = await response.text();

    expect(body).toContain('process_uptime_seconds');
  });

  it('body contains active_loops_total', async () => {
    const { GET } = await import('@/app/api/metrics/route');
    const response = await GET();
    const body = await response.text();

    expect(body).toContain('active_loops_total');
  });
});

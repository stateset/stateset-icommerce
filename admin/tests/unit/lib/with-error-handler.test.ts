/**
 * Tests for withErrorHandler middleware
 *
 * @module tests/unit/lib/with-error-handler
 */

import { describe, it, expect, vi } from 'vitest';
import { NextRequest, NextResponse } from 'next/server';
import { withErrorHandler } from '@/lib/shared/with-error-handler';
import { AppError } from '@/lib/shared/errors';

function createMockRequest(options: {
  method?: string;
  path?: string;
  body?: string;
  contentLength?: string;
} = {}) {
  const { method = 'POST', path = '/api/test', body, contentLength } = options;
  const url = `http://localhost:3000${path}`;
  const headers = new Headers({ 'Content-Type': 'application/json' });
  if (contentLength) {
    headers.set('content-length', contentLength);
  }
  return new NextRequest(url, {
    method,
    headers,
    body: body ?? undefined,
  });
}

describe('withErrorHandler', () => {
  describe('body size limit', () => {
    it('rejects requests exceeding default 1MB limit', async () => {
      const handler = withErrorHandler(async () => {
        return NextResponse.json({ ok: true });
      });

      const request = createMockRequest({
        contentLength: '2000000', // 2MB
      });

      const response = await handler(request);
      const data = await response.json();

      expect(response.status).toBe(413);
      expect(data.error.code).toBe('PAYLOAD_TOO_LARGE');
    });

    it('rejects requests exceeding custom limit', async () => {
      const handler = withErrorHandler(
        async () => NextResponse.json({ ok: true }),
        { maxBodySize: 1024 } // 1KB
      );

      const request = createMockRequest({
        contentLength: '2048',
      });

      const response = await handler(request);
      expect(response.status).toBe(413);
    });

    it('allows requests within size limit', async () => {
      const handler = withErrorHandler(async () => {
        return NextResponse.json({ ok: true });
      });

      const request = createMockRequest({
        contentLength: '100',
        body: JSON.stringify({ test: true }),
      });

      const response = await handler(request);
      expect(response.status).toBe(200);
    });

    it('allows requests without content-length header', async () => {
      const handler = withErrorHandler(async () => {
        return NextResponse.json({ ok: true });
      });

      const request = createMockRequest({});

      const response = await handler(request);
      expect(response.status).toBe(200);
    });
  });

  describe('error handling', () => {
    it('catches AppError and returns structured response', async () => {
      const handler = withErrorHandler(async () => {
        throw AppError.notFound('Item not found');
      });

      const request = createMockRequest({ method: 'GET' });
      const response = await handler(request);
      const data = await response.json();

      expect(response.status).toBe(404);
      expect(data.success).toBe(false);
      expect(data.error.code).toBe('NOT_FOUND');
    });

    it('catches unknown errors and returns 500', async () => {
      const handler = withErrorHandler(async () => {
        throw new Error('unexpected');
      });

      const request = createMockRequest({ method: 'GET' });
      const response = await handler(request);
      const data = await response.json();

      expect(response.status).toBe(500);
      expect(data.error.code).toBe('INTERNAL_ERROR');
    });

    it('sets X-Request-Id header on success', async () => {
      const handler = withErrorHandler(async () => {
        return NextResponse.json({ ok: true });
      });

      const request = createMockRequest({ method: 'GET' });
      const response = await handler(request);

      expect(response.headers.get('X-Request-Id')).toMatch(/^req_/);
    });
  });
});

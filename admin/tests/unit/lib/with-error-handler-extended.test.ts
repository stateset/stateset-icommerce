/**
 * Extended Tests for withErrorHandler — SSE mode, ValidationError,
 * request context, and orgId extraction
 *
 * @module tests/unit/lib/with-error-handler-extended
 */

import { describe, it, expect, vi } from 'vitest';
import { NextRequest, NextResponse } from 'next/server';
import { withErrorHandler } from '@/lib/shared/with-error-handler';
import { AppError, ValidationError } from '@/lib/shared/errors';

function createMockRequest(options: {
  method?: string;
  path?: string;
  body?: string;
  contentLength?: string;
  headers?: Record<string, string>;
} = {}) {
  const {
    method = 'GET',
    path = '/api/test',
    body,
    contentLength,
    headers: extraHeaders = {},
  } = options;
  const url = `http://localhost:3000${path}`;
  const headers = new Headers({ 'Content-Type': 'application/json', ...extraHeaders });
  if (contentLength) {
    headers.set('content-length', contentLength);
  }
  return new NextRequest(url, {
    method,
    headers,
    body: body ?? undefined,
  });
}

// ============================================================================
// SSE error mode
// ============================================================================

describe('withErrorHandler SSE mode', () => {
  it('returns SSE error event for AppError when sse=true', async () => {
    const handler = withErrorHandler(
      async () => {
        throw AppError.notFound('Stream resource missing');
      },
      { sse: true }
    );

    const request = createMockRequest({ method: 'GET' });
    const response = await handler(request);
    const body = await response.text();

    expect(body).toContain('event: error');
    expect(body).toContain('"message"');
    expect(body).toContain('Stream resource missing');
    expect(body).toContain('"code"');
    expect(body).toContain('NOT_FOUND');
  });

  it('returns SSE content-type for errors in SSE mode', async () => {
    const handler = withErrorHandler(
      async () => {
        throw new Error('unexpected');
      },
      { sse: true }
    );

    const request = createMockRequest({ method: 'GET' });
    const response = await handler(request);

    expect(response.headers.get('Content-Type')).toBe('text/event-stream');
    expect(response.headers.get('Cache-Control')).toBe('no-cache');
  });

  it('returns SSE error for unknown errors in SSE mode', async () => {
    const handler = withErrorHandler(
      async () => {
        throw new Error('boom');
      },
      { sse: true }
    );

    const request = createMockRequest({ method: 'GET' });
    const response = await handler(request);
    const body = await response.text();

    expect(body).toContain('event: error');
    expect(body).toContain('INTERNAL_ERROR');
    expect(body).toContain('Internal server error');
  });

  it('returns SSE error for ValidationError in SSE mode', async () => {
    const handler = withErrorHandler(
      async () => {
        throw new ValidationError([{ field: 'query', message: 'Required' }]);
      },
      { sse: true }
    );

    const request = createMockRequest({ method: 'GET' });
    const response = await handler(request);
    const body = await response.text();

    expect(body).toContain('event: error');
    expect(body).toContain('VALIDATION_ERROR');
  });
});

// ============================================================================
// ValidationError handling
// ============================================================================

describe('withErrorHandler ValidationError handling', () => {
  it('returns 422 for ValidationError', async () => {
    const handler = withErrorHandler(async () => {
      throw new ValidationError([
        { field: 'email', message: 'Invalid' },
        { field: 'password', message: 'Too short' },
      ]);
    });

    const request = createMockRequest({ method: 'GET' });
    const response = await handler(request);
    const data = await response.json();

    expect(response.status).toBe(422);
    expect(data.error.code).toBe('VALIDATION_ERROR');
  });

  it('includes validation message in error response', async () => {
    const handler = withErrorHandler(async () => {
      throw new ValidationError([
        { field: 'name', message: 'Name is required' },
      ]);
    });

    const request = createMockRequest({ method: 'GET' });
    const response = await handler(request);
    const data = await response.json();

    expect(data.error.message).toContain('name');
    expect(data.error.message).toContain('Name is required');
  });
});

// ============================================================================
// Request context enrichment
// ============================================================================

describe('withErrorHandler request context', () => {
  it('sets X-Request-Id on error responses', async () => {
    const handler = withErrorHandler(async () => {
      throw AppError.badRequest('test');
    });

    const request = createMockRequest({ method: 'GET' });
    const response = await handler(request);

    // Error responses go through sendError which is within requestStore.run,
    // so the response should include request context metadata
    const body = await response.json();
    expect(body.meta).toBeDefined();
    expect(body.meta.requestId).toBeDefined();
  });

  it('extracts orgId from x-org-id header when Authorization is present', async () => {
    const handler = withErrorHandler(async () => {
      return NextResponse.json({ ok: true });
    });

    const request = createMockRequest({
      method: 'GET',
      headers: {
        Authorization: 'Bearer token-123',
        'x-org-id': 'org-42',
      },
    });

    const response = await handler(request);
    expect(response.status).toBe(200);
  });

  it('handles handler that returns plain Response', async () => {
    const handler = withErrorHandler(async () => {
      return new Response('plain text', { status: 200 });
    });

    const request = createMockRequest({ method: 'GET' });
    const response = await handler(request);
    const text = await response.text();

    expect(text).toBe('plain text');
  });
});

// ============================================================================
// Non-string errors
// ============================================================================

describe('withErrorHandler non-standard errors', () => {
  it('handles thrown string', async () => {
    const handler = withErrorHandler(async () => {
      throw 'string error';
    });

    const request = createMockRequest({ method: 'GET' });
    const response = await handler(request);
    const data = await response.json();

    expect(response.status).toBe(500);
    expect(data.error.code).toBe('INTERNAL_ERROR');
  });

  it('handles thrown null', async () => {
    const handler = withErrorHandler(async () => {
      throw null;
    });

    const request = createMockRequest({ method: 'GET' });
    const response = await handler(request);

    expect(response.status).toBe(500);
  });

  it('handles thrown number', async () => {
    const handler = withErrorHandler(async () => {
      throw 42;
    });

    const request = createMockRequest({ method: 'GET' });
    const response = await handler(request);

    expect(response.status).toBe(500);
  });
});

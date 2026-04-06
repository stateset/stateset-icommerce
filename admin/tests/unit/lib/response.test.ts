/**
 * Tests for Response Helpers
 *
 * @module tests/unit/lib/response
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { sendSuccess, sendError, sendPaginated } from '@/lib/shared/response';

// Mock the request-context module to return a stable requestId
vi.mock('@/lib/shared/request-context', () => ({
  getRequestId: () => 'req_test_12345',
  getRequestContext: () => ({
    requestId: 'req_test_12345',
    startTime: Date.now(),
  }),
  requestStore: {
    getStore: () => ({
      requestId: 'req_test_12345',
      startTime: Date.now(),
    }),
  },
}));

describe('sendSuccess', () => {
  it('returns a response with success true', async () => {
    const response = sendSuccess({ id: 1, name: 'Test' });
    const body = await response.json();

    expect(body.success).toBe(true);
  });

  it('includes data in the response', async () => {
    const data = { id: 1, name: 'Test Item' };
    const response = sendSuccess(data);
    const body = await response.json();

    expect(body.data).toEqual(data);
  });

  it('includes meta with requestId and timestamp', async () => {
    const response = sendSuccess({ value: 'test' });
    const body = await response.json();

    expect(body.meta).toBeDefined();
    expect(body.meta.requestId).toBe('req_test_12345');
    expect(body.meta.timestamp).toBeDefined();
    expect(typeof body.meta.timestamp).toBe('string');
  });

  it('defaults to HTTP status 200', () => {
    const response = sendSuccess({ ok: true });

    expect(response.status).toBe(200);
  });

  it('accepts a custom HTTP status', () => {
    const response = sendSuccess({ created: true }, 201);

    expect(response.status).toBe(201);
  });

  it('returns the correct envelope shape', async () => {
    const response = sendSuccess('hello');
    const body = await response.json();

    expect(body).toHaveProperty('success');
    expect(body).toHaveProperty('data');
    expect(body).toHaveProperty('meta');
    expect(Object.keys(body)).toEqual(
      expect.arrayContaining(['success', 'data', 'meta'])
    );
  });

  it('handles null data', async () => {
    const response = sendSuccess(null);
    const body = await response.json();

    expect(body.success).toBe(true);
    expect(body.data).toBeNull();
  });

  it('handles array data', async () => {
    const response = sendSuccess([1, 2, 3]);
    const body = await response.json();

    expect(body.data).toEqual([1, 2, 3]);
  });
});

describe('sendError', () => {
  it('returns a response with success false', async () => {
    const response = sendError(400, 'Bad request');
    const body = await response.json();

    expect(body.success).toBe(false);
  });

  it('includes error message and code', async () => {
    const response = sendError(404, 'Not found', 'NOT_FOUND');
    const body = await response.json();

    expect(body.error).toBeDefined();
    expect(body.error.message).toBe('Not found');
    expect(body.error.code).toBe('NOT_FOUND');
  });

  it('uses default error code when not specified', async () => {
    const response = sendError(500, 'Something broke');
    const body = await response.json();

    expect(body.error.code).toBe('ERROR');
  });

  it('sets the correct HTTP status code', () => {
    const response = sendError(422, 'Validation failed', 'VALIDATION_ERROR');

    expect(response.status).toBe(422);
  });

  it('includes meta with requestId and timestamp', async () => {
    const response = sendError(400, 'Bad');
    const body = await response.json();

    expect(body.meta).toBeDefined();
    expect(body.meta.requestId).toBe('req_test_12345');
    expect(body.meta.timestamp).toBeDefined();
  });

  it('returns the correct error envelope shape', async () => {
    const response = sendError(500, 'Internal error', 'INTERNAL');
    const body = await response.json();

    expect(body).toHaveProperty('success');
    expect(body).toHaveProperty('error');
    expect(body).toHaveProperty('meta');
    expect(body.error).toHaveProperty('message');
    expect(body.error).toHaveProperty('code');
  });
});

describe('sendPaginated', () => {
  it('returns a response with success true', async () => {
    const response = sendPaginated([], { total: 0, limit: 20, offset: 0 });
    const body = await response.json();

    expect(body.success).toBe(true);
  });

  it('includes data array', async () => {
    const items = [{ id: 1 }, { id: 2 }];
    const response = sendPaginated(items, { total: 2, limit: 20, offset: 0 });
    const body = await response.json();

    expect(body.data).toEqual(items);
    expect(body.data).toHaveLength(2);
  });

  it('includes pagination meta', async () => {
    const response = sendPaginated([{ id: 1 }], {
      total: 50,
      limit: 10,
      offset: 20,
    });
    const body = await response.json();

    expect(body.meta.pagination).toBeDefined();
    expect(body.meta.pagination.total).toBe(50);
    expect(body.meta.pagination.limit).toBe(10);
    expect(body.meta.pagination.offset).toBe(20);
  });

  it('calculates hasMore correctly when more results exist', async () => {
    const response = sendPaginated([{ id: 1 }], {
      total: 100,
      limit: 10,
      offset: 0,
    });
    const body = await response.json();

    expect(body.meta.pagination.hasMore).toBe(true);
  });

  it('calculates hasMore correctly when at the end', async () => {
    const response = sendPaginated([{ id: 1 }], {
      total: 10,
      limit: 10,
      offset: 5,
    });
    const body = await response.json();

    // offset(5) + limit(10) = 15 >= total(10), so hasMore = false
    expect(body.meta.pagination.hasMore).toBe(false);
  });

  it('calculates hasMore as false when offset + limit equals total', async () => {
    const response = sendPaginated([{ id: 1 }], {
      total: 20,
      limit: 10,
      offset: 10,
    });
    const body = await response.json();

    // offset(10) + limit(10) = 20, not < total(20), so hasMore = false
    expect(body.meta.pagination.hasMore).toBe(false);
  });

  it('includes requestId and timestamp in meta', async () => {
    const response = sendPaginated([], { total: 0, limit: 20, offset: 0 });
    const body = await response.json();

    expect(body.meta.requestId).toBe('req_test_12345');
    expect(body.meta.timestamp).toBeDefined();
  });

  it('handles empty data array', async () => {
    const response = sendPaginated([], { total: 0, limit: 20, offset: 0 });
    const body = await response.json();

    expect(body.data).toEqual([]);
    expect(body.meta.pagination.total).toBe(0);
    expect(body.meta.pagination.hasMore).toBe(false);
  });
});

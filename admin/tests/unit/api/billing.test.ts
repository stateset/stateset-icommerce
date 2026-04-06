/**
 * Tests for /api/billing/subscription and /api/billing/webhook endpoints
 *
 * @module tests/unit/api/billing
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

// Mock logger to suppress output and allow inspection
vi.mock('@/lib/shared/logger', () => ({
  logger: {
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  },
}));

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
// GET /api/billing/subscription
// ============================================================================

describe('GET /api/billing/subscription', () => {
  it('returns subscription data with valid auth', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          id: 'sub-1',
          planId: 'pro',
          status: 'active',
          currentPeriodEnd: '2026-02-28T00:00:00Z',
        }),
    });

    const { GET } = await import('@/app/api/billing/subscription/route');

    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/billing/subscription',
    });

    const response = await GET(request, undefined as any);
    const body = await expectSuccess(response);

    expect(body.data).toHaveProperty('id', 'sub-1');
    expect(body.data).toHaveProperty('status', 'active');
  });

  it('returns 401 when no auth token is provided', async () => {
    vi.stubEnv('STATESET_API_TOKEN', '');

    const { GET } = await import('@/app/api/billing/subscription/route');

    const request = createMockRequest({
      url: 'http://localhost:3000/api/billing/subscription',
    });

    const response = await GET(request, undefined as any);
    await expectError(response, 401, 'UNAUTHORIZED');
  });

  it('does not authenticate from STATESET_API_TOKEN alone', async () => {
    vi.stubEnv('STATESET_API_TOKEN', 'env-token');

    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          id: 'sub-1',
          planId: 'pro',
          status: 'active',
        }),
    });

    const { GET } = await import('@/app/api/billing/subscription/route');

    const request = createMockRequest({
      url: 'http://localhost:3000/api/billing/subscription',
    });

    const response = await GET(request, undefined as any);
    await expectError(response, 401, 'UNAUTHORIZED');
  });

  it('handles upstream API errors', async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 502,
      json: () => Promise.resolve({ error: 'Bad gateway' }),
    });

    const { GET } = await import('@/app/api/billing/subscription/route');

    const request = createAuthenticatedRequest({
      url: 'http://localhost:3000/api/billing/subscription',
    });

    const response = await GET(request, undefined as any);
    await expectError(response, 502, 'BILLING_ERROR');
  });
});

// ============================================================================
// POST /api/billing/subscription
// ============================================================================

describe('POST /api/billing/subscription', () => {
  it('returns 201 with valid subscription body', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          id: 'sub-new',
          planId: 'pro',
          status: 'active',
        }),
    });

    const { POST } = await import('@/app/api/billing/subscription/route');

    const request = createAuthenticatedRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/billing/subscription',
      body: {
        planId: 'pro',
        paymentMethodId: 'pm_test_123',
      },
    });

    const response = await POST(request, undefined as any);
    const body = await expectSuccess(response, 201);

    expect(body.data).toHaveProperty('id', 'sub-new');
  });

  it('returns 422 when planId is missing', async () => {
    const { POST } = await import('@/app/api/billing/subscription/route');

    const request = createAuthenticatedRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/billing/subscription',
      body: {
        paymentMethodId: 'pm_test_123',
      },
    });

    const response = await POST(request, undefined as any);
    await expectError(response, 422, 'VALIDATION_ERROR');
  });

  it('returns 422 when paymentMethodId is missing', async () => {
    const { POST } = await import('@/app/api/billing/subscription/route');

    const request = createAuthenticatedRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/billing/subscription',
      body: {
        planId: 'pro',
      },
    });

    const response = await POST(request, undefined as any);
    await expectError(response, 422, 'VALIDATION_ERROR');
  });

  it('returns 422 with empty body', async () => {
    const { POST } = await import('@/app/api/billing/subscription/route');

    const request = createAuthenticatedRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/billing/subscription',
      body: {},
    });

    const response = await POST(request, undefined as any);
    await expectError(response, 422, 'VALIDATION_ERROR');
  });

  it('returns 401 without auth', async () => {
    vi.stubEnv('STATESET_API_TOKEN', '');

    const { POST } = await import('@/app/api/billing/subscription/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/billing/subscription',
      body: {
        planId: 'pro',
        paymentMethodId: 'pm_test_123',
      },
    });

    const response = await POST(request, undefined as any);
    await expectError(response, 401, 'UNAUTHORIZED');
  });
});

// ============================================================================
// PATCH /api/billing/subscription
// ============================================================================

describe('PATCH /api/billing/subscription', () => {
  it('returns success when updating planId', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          id: 'sub-1',
          planId: 'enterprise',
          status: 'active',
        }),
    });

    const { PATCH } = await import('@/app/api/billing/subscription/route');

    const request = createAuthenticatedRequest({
      method: 'PATCH',
      url: 'http://localhost:3000/api/billing/subscription',
      body: { planId: 'enterprise' },
    });

    const response = await PATCH(request, undefined as any);
    const body = await expectSuccess(response);

    expect(body.data).toHaveProperty('planId', 'enterprise');
  });

  it('returns success when setting cancelAtPeriodEnd', async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          id: 'sub-1',
          planId: 'pro',
          status: 'active',
          cancelAtPeriodEnd: true,
        }),
    });

    const { PATCH } = await import('@/app/api/billing/subscription/route');

    const request = createAuthenticatedRequest({
      method: 'PATCH',
      url: 'http://localhost:3000/api/billing/subscription',
      body: { cancelAtPeriodEnd: true },
    });

    const response = await PATCH(request, undefined as any);
    const body = await expectSuccess(response);

    expect(body.data).toHaveProperty('cancelAtPeriodEnd', true);
  });

  it('returns 401 without auth', async () => {
    vi.stubEnv('STATESET_API_TOKEN', '');

    const { PATCH } = await import('@/app/api/billing/subscription/route');

    const request = createMockRequest({
      method: 'PATCH',
      url: 'http://localhost:3000/api/billing/subscription',
      body: { planId: 'enterprise' },
    });

    const response = await PATCH(request, undefined as any);
    await expectError(response, 401, 'UNAUTHORIZED');
  });

  it('handles upstream API errors', async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 400,
      json: () => Promise.resolve({ error: 'Invalid plan' }),
    });

    const { PATCH } = await import('@/app/api/billing/subscription/route');

    const request = createAuthenticatedRequest({
      method: 'PATCH',
      url: 'http://localhost:3000/api/billing/subscription',
      body: { planId: 'invalid-plan' },
    });

    const response = await PATCH(request, undefined as any);
    await expectError(response, 400, 'BILLING_ERROR');
  });
});

// ============================================================================
// POST /api/billing/webhook
// ============================================================================

describe('POST /api/billing/webhook', () => {
  beforeEach(() => {
    vi.stubEnv('STRIPE_WEBHOOK_SECRET', 'whsec_test_secret');
  });

  it('returns 400 when stripe-signature header is missing', async () => {
    const { POST } = await import('@/app/api/billing/webhook/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/billing/webhook',
      body: { type: 'customer.subscription.created', id: 'evt-1' },
    });

    const response = await POST(request);
    const body = await parseResponse<{
      success: boolean;
      error: { message: string; code: string };
    }>(response);

    expect(response.status).toBe(400);
    expect(body.success).toBe(false);
    expect(body.error.code).toBe('WEBHOOK_INVALID');
  });

  it('returns 400 when signature is malformed', async () => {
    const { POST } = await import('@/app/api/billing/webhook/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/billing/webhook',
      headers: {
        'stripe-signature': 'malformed-signature',
      },
      body: { type: 'customer.subscription.created', id: 'evt-1' },
    });

    const response = await POST(request);
    const body = await parseResponse<{
      success: boolean;
      error: { message: string; code: string };
    }>(response);

    expect(response.status).toBe(400);
    expect(body.success).toBe(false);
    expect(body.error.code).toBe('WEBHOOK_INVALID');
  });

  it('returns 500 when STRIPE_WEBHOOK_SECRET is not configured', async () => {
    vi.stubEnv('STRIPE_WEBHOOK_SECRET', '');

    // Need to re-import to pick up the new env value;
    // The constant is read at module load time, so we need to reset modules
    vi.resetModules();

    const { POST } = await import('@/app/api/billing/webhook/route');

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/billing/webhook',
      headers: {
        'stripe-signature': 't=123456789,v1=abc123',
      },
      body: { type: 'customer.subscription.created', id: 'evt-1' },
    });

    const response = await POST(request);
    const body = await parseResponse<{
      success: boolean;
      error: { message: string; code: string };
    }>(response);

    expect(response.status).toBe(500);
    expect(body.success).toBe(false);
    expect(body.error.code).toBe('WEBHOOK_CONFIG_ERROR');
  });

  it('returns 400 when timestamp is too old', async () => {
    // Reset modules to ensure STRIPE_WEBHOOK_SECRET is read fresh
    vi.resetModules();
    vi.stubEnv('STRIPE_WEBHOOK_SECRET', 'whsec_test_secret');

    const { POST } = await import('@/app/api/billing/webhook/route');

    // Use a timestamp that is more than 5 minutes old
    const oldTimestamp = Math.floor(Date.now() / 1000) - 600;

    const request = createMockRequest({
      method: 'POST',
      url: 'http://localhost:3000/api/billing/webhook',
      headers: {
        'stripe-signature': `t=${oldTimestamp},v1=somesignature`,
      },
      body: { type: 'customer.subscription.created', id: 'evt-1' },
    });

    const response = await POST(request);
    const body = await parseResponse<{
      success: boolean;
      error: { message: string; code: string };
    }>(response);

    expect(response.status).toBe(400);
    expect(body.success).toBe(false);
    expect(body.error.code).toBe('WEBHOOK_EXPIRED');
  });

  it('processes valid webhook event with correct signature', async () => {
    vi.resetModules();
    vi.stubEnv('STRIPE_WEBHOOK_SECRET', 'whsec_test_secret');

    const { POST } = await import('@/app/api/billing/webhook/route');

    const eventBody = JSON.stringify({
      type: 'customer.subscription.created',
      id: 'evt-1',
      data: { object: { id: 'sub-1' } },
    });

    const timestamp = Math.floor(Date.now() / 1000);
    const signedPayload = `${timestamp}.${eventBody}`;

    // Compute the expected HMAC-SHA256 signature
    const encoder = new TextEncoder();
    const key = await crypto.subtle.importKey(
      'raw',
      encoder.encode('whsec_test_secret'),
      { name: 'HMAC', hash: 'SHA-256' },
      false,
      ['sign']
    );
    const sigBuffer = await crypto.subtle.sign('HMAC', key, encoder.encode(signedPayload));
    const sigHex = Array.from(new Uint8Array(sigBuffer))
      .map((b) => b.toString(16).padStart(2, '0'))
      .join('');

    // Build a raw request with text body (webhook reads request.text())
    const url = new URL('http://localhost:3000/api/billing/webhook');
    const rawRequest = new Request(url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'stripe-signature': `t=${timestamp},v1=${sigHex}`,
      },
      body: eventBody,
    });

    // NextRequest wraps Request
    const { NextRequest } = await import('next/server');
    const nextRequest = new NextRequest(rawRequest);

    const response = await POST(nextRequest);
    const body = await parseResponse<{
      success: boolean;
      data: { received: boolean };
    }>(response);

    expect(response.status).toBe(200);
    expect(body.success).toBe(true);
    expect(body.data.received).toBe(true);
  });
});

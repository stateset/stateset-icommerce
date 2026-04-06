/**
 * API Test Utilities
 *
 * Mock request/response factories for testing Next.js App Router API routes.
 */

import { NextRequest } from 'next/server';

interface MockRequestOptions {
  method?: string;
  url?: string;
  headers?: Record<string, string>;
  body?: unknown;
  searchParams?: Record<string, string>;
}

/**
 * Create a mock NextRequest for testing API routes.
 */
export function createMockRequest(options: MockRequestOptions = {}): NextRequest {
  const {
    method = 'GET',
    url = 'http://localhost:3000/api/test',
    headers = {},
    body,
    searchParams = {},
  } = options;

  const urlObj = new URL(url);
  Object.entries(searchParams).forEach(([key, value]) => {
    urlObj.searchParams.set(key, value);
  });

  const upperMethod = method.toUpperCase();
  const isSafeMethod = upperMethod === 'GET' || upperMethod === 'HEAD' || upperMethod === 'OPTIONS';
  const normalizedHeaders: Record<string, string> = {
    'Content-Type': 'application/json',
    ...headers,
  };

  // State-changing route tests use a default CSRF token/cookie pair unless overridden.
  if (!isSafeMethod) {
    const csrfHeader = normalizedHeaders['x-csrf-token'] ?? normalizedHeaders['X-CSRF-Token'];
    if (!csrfHeader) {
      normalizedHeaders['x-csrf-token'] = 'test-csrf-token';
    }

    const existingCookie = normalizedHeaders['Cookie'] ?? normalizedHeaders['cookie'] ?? '';
    const csrfCookie = '__csrf=test-csrf-token';
    normalizedHeaders['Cookie'] = existingCookie
      ? `${existingCookie}; ${csrfCookie}`
      : csrfCookie;
    delete normalizedHeaders['cookie'];
  }

  const init: NonNullable<ConstructorParameters<typeof NextRequest>[1]> = {
    method,
    headers: normalizedHeaders,
  };

  if (body && upperMethod !== 'GET' && upperMethod !== 'HEAD') {
    init.body = JSON.stringify(body);
  }

  return new NextRequest(urlObj, init);
}

/**
 * Create a mock request with authorization header.
 */
export function createAuthenticatedRequest(
  options: MockRequestOptions = {}
): NextRequest {
  return createMockRequest({
    ...options,
    headers: {
      Authorization: 'Bearer test-token',
      ...options.headers,
    },
  });
}

/**
 * Create a mock request authenticated via the admin session cookie.
 */
export function createSessionCookieRequest(
  options: MockRequestOptions = {},
): NextRequest {
  const existingCookie = options.headers?.Cookie ?? options.headers?.cookie ?? '';
  const sessionCookie = 'stateset_admin_session=test-session-token';

  return createMockRequest({
    ...options,
    headers: {
      ...options.headers,
      Cookie: existingCookie ? `${existingCookie}; ${sessionCookie}` : sessionCookie,
    },
  });
}

/**
 * Parse a NextResponse JSON body.
 */
export async function parseResponse<T = unknown>(response: Response): Promise<T> {
  return response.json() as Promise<T>;
}

/**
 * Assert a response matches the standard success envelope.
 */
export async function expectSuccess<T>(response: Response, status: number = 200) {
  expect(response.status).toBe(status);
  const body = await parseResponse<{
    success: boolean;
    data: T;
    meta: { requestId: string; timestamp: string };
  }>(response);
  expect(body.success).toBe(true);
  expect(body.data).toBeDefined();
  expect(body.meta.requestId).toBeDefined();
  expect(body.meta.timestamp).toBeDefined();
  return body;
}

/**
 * Assert a response matches the standard error envelope.
 */
export async function expectError(
  response: Response,
  status: number,
  code?: string
) {
  expect(response.status).toBe(status);
  const body = await parseResponse<{
    success: boolean;
    error: { message: string; code: string };
    meta: { requestId: string; timestamp: string };
  }>(response);
  expect(body.success).toBe(false);
  expect(body.error).toBeDefined();
  expect(body.error.message).toBeTruthy();
  if (code) {
    expect(body.error.code).toBe(code);
  }
  return body;
}

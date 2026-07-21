/**
 * Catch-all proxy route: forwards /api/gateway/* to the CLI HTTP gateway.
 *
 * Uses GATEWAY_URL env var (default http://127.0.0.1:8080).
 * Path allowlist prevents use as an open proxy.
 */

import { NextRequest, NextResponse } from 'next/server';
import { withErrorHandler } from '@/lib/shared/with-error-handler';
import { AppError } from '@/lib/shared/errors';
import {
  isAdminAuthDisabled,
  requireRequestSessionToken,
  validateSessionToken,
} from '@/lib/shared/auth-session';
import { getRequestId } from '@/lib/shared/request-context';

const GATEWAY_URL = process.env.GATEWAY_URL || 'http://127.0.0.1:8080';
const GATEWAY_API_KEY = process.env.GATEWAY_API_KEY || '';

const ALLOWED_PREFIXES = [
  '/health',
  '/ready',
  '/metrics',
  '/plugins',
  '/commands',
  '/daemon',
  '/remote-access',
  '/heartbeat',
  '/voice/status',
  '/memory/stats',
];

function isAllowedPath(path: string): boolean {
  const normalized = path.replace(/\/+/g, '/').replace(/\/$/, '');
  if (normalized.includes('..')) return false;
  return ALLOWED_PREFIXES.some(
    (prefix) => normalized === prefix || normalized.startsWith(prefix + '/'),
  );
}

async function proxyToGateway(request: NextRequest, path: string): Promise<NextResponse> {
  if (!isAdminAuthDisabled()) {
    const sessionToken = requireRequestSessionToken(request);
    const isValidSession = await validateSessionToken(sessionToken);
    if (!isValidSession) {
      throw AppError.unauthorized('Session expired');
    }
  }

  if (!isAllowedPath(path)) {
    throw AppError.forbidden(`Gateway path not allowed: ${path}`);
  }

  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
  };
  if (GATEWAY_API_KEY) {
    headers['Authorization'] = `Bearer ${GATEWAY_API_KEY}`;
  }

  const url = new URL(path, GATEWAY_URL);
  url.search = request.nextUrl.search;

  try {
    const response = await fetch(url.toString(), {
      method: request.method,
      headers,
      signal: AbortSignal.timeout(10_000),
      ...(request.method !== 'GET' && request.method !== 'HEAD'
        ? { body: await request.text() }
        : {}),
    });

    const data = await response.json();
    return NextResponse.json(
      {
        success: response.ok,
        data,
        meta: {
          requestId: getRequestId(),
          timestamp: new Date().toISOString(),
        },
      },
      { status: response.status },
    );
  } catch (error) {
    if (error instanceof Error && error.name === 'TimeoutError') {
      throw new AppError('Gateway timeout', 504, 'GATEWAY_TIMEOUT');
    }
    throw new AppError(
      `Gateway unreachable: ${error instanceof Error ? error.message : 'Unknown error'}`,
      502,
      'GATEWAY_UNREACHABLE',
    );
  }
}

export const GET = withErrorHandler(async (request: NextRequest, context) => {
  const params = await context!.params;
  const path = '/' + (params.path as unknown as string[]).join('/');
  return proxyToGateway(request, path);
});

export const POST = withErrorHandler(
  async (request: NextRequest, context) => {
    const params = await context!.params;
    const path = '/' + (params.path as unknown as string[]).join('/');
    return proxyToGateway(request, path);
  },
  { requireCsrf: true },
);

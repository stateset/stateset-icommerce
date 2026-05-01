/**
 * Next.js Middleware
 *
 * Handles CSP nonce injection and security headers for all requests.
 */

import { NextRequest, NextResponse } from 'next/server';
import { isAdminAuthDisabled, isTruthyFlag } from '@/lib/shared/admin-auth-config';
import { getRequestSessionToken } from '@/lib/shared/auth-session';
import { getStateSetApiConnectSources } from '@/lib/stateset-api-url';
import { apiRateLimiter, authRateLimiter } from '@/lib/shared/rate-limit';

const ADMIN_SESSION_COOKIE = 'stateset_admin_session';
const TRUST_PROXY_HEADERS_FLAG = 'STATESET_ADMIN_TRUST_PROXY_HEADERS';

const PUBLIC_API_PREFIXES = [
  '/api/auth/csrf-token',
  '/api/auth/forgot-password',
  '/api/auth/login',
  '/api/auth/logout',
  '/api/auth/me',
  '/api/auth/register',
  '/api/auth/reset-password',
  '/api/auth/verify-email',
  '/api/billing/webhook',
  '/api/health',
];

function hasSessionCookie(request: NextRequest): string | null {
  return request.cookies.get(ADMIN_SESSION_COOKIE)?.value?.trim() || null;
}

function isPublicApiPath(pathname: string): boolean {
  return PUBLIC_API_PREFIXES.some(
    (prefix) => pathname === prefix || pathname.startsWith(`${prefix}/`)
  );
}

function normalizeRateLimitIdentifier(value: string | null | undefined): string | null {
  const normalized = value?.split(',')[0]?.trim();
  if (!normalized || normalized.length > 128 || /[\r\n]/.test(normalized)) {
    return null;
  }
  return normalized;
}

function shouldTrustProxyHeaders(): boolean {
  return isTruthyFlag(process.env[TRUST_PROXY_HEADERS_FLAG]);
}

export function resolveRateLimitClientKey(request: NextRequest): string {
  const runtimeIp = normalizeRateLimitIdentifier(
    (request as unknown as { ip?: string }).ip
  );
  if (runtimeIp) {
    return runtimeIp;
  }

  if (shouldTrustProxyHeaders()) {
    return normalizeRateLimitIdentifier(request.headers.get('x-forwarded-for'))
      || normalizeRateLimitIdentifier(request.headers.get('x-real-ip'))
      || 'unknown';
  }

  return 'unknown';
}

function applySecurityHeaders(request: NextRequest, response: NextResponse): NextResponse {
  const nonceSource = globalThis.crypto.randomUUID();
  const nonce =
    typeof btoa === 'function'
      ? btoa(nonceSource)
      : Buffer.from(nonceSource).toString('base64');

  const csp = [
    `default-src 'self'`,
    `script-src 'self' 'nonce-${nonce}'`,
    `style-src 'self' 'nonce-${nonce}' https://fonts.googleapis.com`,
    `font-src 'self' https://fonts.gstatic.com`,
    `img-src 'self' data: https:`,
    `connect-src 'self' ${getStateSetApiConnectSources().join(' ')}`,
    `frame-ancestors 'none'`,
    `base-uri 'self'`,
    `form-action 'self'`,
    `upgrade-insecure-requests`,
  ].join('; ');

  response.headers.set('Content-Security-Policy', csp);
  response.headers.set('x-nonce', nonce);

  if (request.nextUrl.pathname.startsWith('/api/') && request.nextUrl.pathname !== '/api/health') {
    response.headers.set('Cache-Control', 'no-store');
  }

  return response;
}

export async function middleware(request: NextRequest) {
  const { pathname } = request.nextUrl;
  const authDisabled = isAdminAuthDisabled();

  const limiter = pathname.startsWith('/api/auth/') ? authRateLimiter : apiRateLimiter;
  const rateLimitResult = await limiter.consumeAsync(resolveRateLimitClientKey(request));

  if (!rateLimitResult.allowed) {
    const response = NextResponse.json(
      {
        success: false,
        error: {
          message: 'Rate limit exceeded',
          code: 'RATE_LIMITED',
        },
      },
      { status: 429 }
    );
    response.headers.set('Retry-After', String(Math.ceil((rateLimitResult.resetAt - Date.now()) / 1000)));
    response.headers.set('X-RateLimit-Limit', String(rateLimitResult.limit));
    response.headers.set('X-RateLimit-Remaining', '0');
    return applySecurityHeaders(request, response);
  }

  const sessionCookie = hasSessionCookie(request);
  const authToken = getRequestSessionToken(request);
  const isApiPath = pathname.startsWith('/api/');

  if (!authDisabled && isApiPath && !isPublicApiPath(pathname) && !authToken) {
    return applySecurityHeaders(
      request,
      NextResponse.json(
        {
          success: false,
          error: {
            message: 'Authentication required',
            code: 'UNAUTHORIZED',
          },
        },
        { status: 401 }
      )
    );
  }

  if (!authDisabled && !isApiPath && pathname !== '/' && !sessionCookie) {
    return applySecurityHeaders(request, NextResponse.redirect(new URL('/', request.url)));
  }

  if (!authDisabled && isApiPath && authToken && !isPublicApiPath(pathname)) {
    const headers = new Headers(request.headers);
    if (!headers.get('Authorization')) {
      headers.set('Authorization', `Bearer ${authToken}`);
    }
    return applySecurityHeaders(
      request,
      NextResponse.next({
        request: {
          headers,
        },
      })
    );
  }

  return applySecurityHeaders(request, NextResponse.next());
}

export const config = {
  matcher: [
    /*
     * Match all request paths except:
     * - _next/static (static files)
     * - _next/image (image optimization)
     * - favicon.ico (favicon file)
     * - public files
     */
    {
      source: '/((?!_next/static|_next/image|favicon.ico|.*\\.(?:svg|png|jpg|jpeg|gif|webp)$).*)',
      missing: [
        { type: 'header', key: 'next-router-prefetch' },
        { type: 'header', key: 'purpose', value: 'prefetch' },
      ],
    },
  ],
};

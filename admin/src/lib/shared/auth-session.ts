import { cookies } from 'next/headers';
import type { NextRequest, NextResponse } from 'next/server';
import { AppError } from './errors';
import { isAdminAuthDisabled } from './admin-auth-config';
export {
  ADMIN_AUTH_DISABLE_FLAG,
  getBypassAdminUser,
  isAdminAuthDisabled,
} from './admin-auth-config';
import { getServerStateSetApiUrl } from '@/lib/stateset-api-url';

export const ADMIN_SESSION_COOKIE = 'stateset_admin_session';
const SESSION_MAX_AGE_SECONDS = 60 * 60 * 12;
const API_URL = getServerStateSetApiUrl();

function normalizeToken(token: string | null | undefined): string | null {
  if (typeof token !== 'string') return null;
  const trimmed = token.trim();
  return trimmed ? trimmed : null;
}

export function getServiceSessionToken(): string | null {
  return normalizeToken(process.env.STATESET_API_TOKEN);
}

export function extractBearerToken(authHeader: string | null): string | null {
  if (typeof authHeader !== 'string') return null;
  if (!authHeader.toLowerCase().startsWith('bearer ')) return null;
  return normalizeToken(authHeader.slice(7));
}

export function getRequestSessionToken(request: NextRequest): string | null {
  return (
    extractBearerToken(request.headers.get('Authorization')) ??
    normalizeToken(request.cookies.get(ADMIN_SESSION_COOKIE)?.value)
  );
}

export function requireRequestSessionToken(
  request: NextRequest,
  message: string = 'Authentication required',
): string {
  const token =
    getRequestSessionToken(request) ?? (isAdminAuthDisabled() ? getServiceSessionToken() : null);
  if (!token) {
    throw AppError.unauthorized(message);
  }
  return token;
}

export async function getServerSessionToken(): Promise<string | null> {
  const cookieStore = await cookies();
  return normalizeToken(cookieStore.get(ADMIN_SESSION_COOKIE)?.value);
}

export async function requireServerSessionToken(
  message: string = 'Authentication required',
): Promise<string> {
  const token = await getServerSessionToken();
  if (!token) {
    throw AppError.unauthorized(message);
  }
  return token;
}

/**
 * Auth guard for server actions. Requires the admin session cookie and
 * throws `AppError.unauthorized` when it is missing — except in the
 * auth-disabled dev mode, where it is skipped exactly like the middleware
 * bypass (`isAdminAuthDisabled()` is ignored in production).
 *
 * Returns the session token, or `null` when auth is disabled.
 */
export async function requireAdminSession(
  message: string = 'Authentication required',
): Promise<string | null> {
  if (isAdminAuthDisabled()) {
    return null;
  }
  return requireServerSessionToken(message);
}

export function setSessionCookie(response: NextResponse, token: string): NextResponse {
  response.cookies.set(ADMIN_SESSION_COOKIE, token, {
    httpOnly: true,
    secure: process.env.NODE_ENV === 'production',
    sameSite: 'lax',
    path: '/',
    maxAge: SESSION_MAX_AGE_SECONDS,
  });
  return response;
}

export function clearSessionCookie(response: NextResponse): NextResponse {
  response.cookies.set(ADMIN_SESSION_COOKIE, '', {
    httpOnly: true,
    secure: process.env.NODE_ENV === 'production',
    sameSite: 'lax',
    path: '/',
    maxAge: 0,
  });
  return response;
}

export function isAuthenticatedRequest(request: NextRequest): boolean {
  return Boolean(
    getRequestSessionToken(request) || (isAdminAuthDisabled() && getServiceSessionToken()),
  );
}

export async function validateSessionToken(token: string): Promise<boolean> {
  if (isAdminAuthDisabled()) {
    return true;
  }

  if (!normalizeToken(token)) {
    return false;
  }

  try {
    const response = await fetch(`${API_URL}/api/auth/me`, {
      method: 'GET',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${token}`,
      },
      cache: 'no-store',
    });
    return response.ok;
  } catch {
    return false;
  }
}

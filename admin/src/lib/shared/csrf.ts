/**
 * CSRF Protection
 *
 * Generates and validates CSRF tokens for state-changing requests.
 * Uses crypto.randomBytes for secure token generation.
 */

import { cookies } from 'next/headers';
import { NextRequest, NextResponse } from 'next/server';

const CSRF_COOKIE_NAME = '__csrf';
const CSRF_HEADER_NAME = 'x-csrf-token';
const CSRF_TOKEN_LENGTH = 32;

function generateToken(): string {
  const array = new Uint8Array(CSRF_TOKEN_LENGTH);
  crypto.getRandomValues(array);
  return Array.from(array, (b) => b.toString(16).padStart(2, '0')).join('');
}

/**
 * Get or create a CSRF token. Sets it as an HttpOnly cookie.
 */
export async function getOrCreateCsrfToken(): Promise<string> {
  const cookieStore = await cookies();
  const existing = cookieStore.get(CSRF_COOKIE_NAME);

  if (existing?.value) {
    return existing.value;
  }

  const token = generateToken();
  cookieStore.set(CSRF_COOKIE_NAME, token, {
    httpOnly: true,
    secure: process.env.NODE_ENV === 'production',
    sameSite: 'strict',
    path: '/',
    maxAge: 60 * 60 * 24, // 24 hours
  });

  return token;
}

/**
 * Validate CSRF token from request header against cookie.
 * Returns true if valid, false otherwise.
 */
export async function validateCsrfToken(request: NextRequest): Promise<boolean> {
  const headerToken = request.headers.get(CSRF_HEADER_NAME);
  const cookieToken = request.cookies.get(CSRF_COOKIE_NAME)?.value;

  if (!headerToken || !cookieToken) {
    return false;
  }

  // Constant-time comparison
  if (headerToken.length !== cookieToken.length) {
    return false;
  }

  const encoder = new TextEncoder();
  const a = encoder.encode(headerToken);
  const b = encoder.encode(cookieToken);

  if (a.length !== b.length) {
    return false;
  }

  let result = 0;
  for (let i = 0; i < a.length; i++) {
    result |= a[i] ^ b[i];
  }

  return result === 0;
}

/**
 * Middleware helper: require CSRF for state-changing methods.
 */
export async function requireCsrf(request: NextRequest): Promise<NextResponse | null> {
  const method = request.method.toUpperCase();
  const safeMethods = ['GET', 'HEAD', 'OPTIONS'];

  if (safeMethods.includes(method)) {
    return null; // No CSRF needed for safe methods
  }

  const valid = await validateCsrfToken(request);
  if (!valid) {
    return NextResponse.json(
      { success: false, error: { message: 'Invalid CSRF token', code: 'CSRF_INVALID' } },
      { status: 403 }
    );
  }

  return null; // Valid
}

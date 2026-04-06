import { NextRequest } from 'next/server';
import { withErrorHandler } from '@/lib/shared/with-error-handler';
import { sendSuccess } from '@/lib/shared/response';
import { AppError } from '@/lib/shared/errors';
import { validateBody, loginSchema } from '@/lib/shared/schemas';
import { setSessionCookie } from '@/lib/shared/auth-session';
import { getServerStateSetApiUrl } from '@/lib/stateset-api-url';

const API_URL = getServerStateSetApiUrl();

/**
 * POST /api/auth/login
 *
 * Authenticate a user and return a session token.
 */
export const POST = withErrorHandler(async (request: NextRequest) => {
  const body = await request.json();
  const validation = validateBody(body, loginSchema);
  if (!validation.success) {
    throw AppError.validationError(validation.errors.map(e => `${e.field}: ${e.message}`).join('; '));
  }

  const { email, password } = validation.data;

  const response = await fetch(`${API_URL}/api/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, password }),
  });

  if (!response.ok) {
    if (response.status === 401) {
      throw AppError.unauthorized('Invalid email or password');
    }
    const errorData = await response.json().catch(() => ({ error: 'Unknown error' }));
    throw new AppError(errorData.error || 'Login failed', response.status, 'AUTH_ERROR');
  }

  const data = await response.json();
  const loginResponse = sendSuccess(data);
  if (typeof data?.token === 'string' && data.token.trim()) {
    setSessionCookie(loginResponse, data.token);
  }
  return loginResponse;
}, { requireCsrf: true });

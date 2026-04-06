import { NextRequest } from 'next/server';
import { withErrorHandler } from '@/lib/shared/with-error-handler';
import { sendError, sendSuccess } from '@/lib/shared/response';
import { AppError } from '@/lib/shared/errors';
import {
  clearSessionCookie,
  getBypassAdminUser,
  isAdminAuthDisabled,
  requireRequestSessionToken,
} from '@/lib/shared/auth-session';
import { getServerStateSetApiUrl } from '@/lib/stateset-api-url';

const API_URL = getServerStateSetApiUrl();

/**
 * GET /api/auth/me
 *
 * Get the current authenticated user's profile.
 */
export const GET = withErrorHandler(async (request: NextRequest) => {
  if (isAdminAuthDisabled()) {
    return sendSuccess({ user: getBypassAdminUser() });
  }

  const token = requireRequestSessionToken(request);

  const response = await fetch(`${API_URL}/api/auth/me`, {
    method: 'GET',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
  });

  if (!response.ok) {
    if (response.status === 401) {
      const unauthorizedResponse = sendError(401, 'Session expired', 'UNAUTHORIZED');
      clearSessionCookie(unauthorizedResponse);
      return unauthorizedResponse;
    }
    const errorData = await response.json().catch(() => ({ error: 'Unknown error' }));
    throw new AppError(errorData.error || 'Failed to fetch user', response.status, 'AUTH_ERROR');
  }

  const data = await response.json();
  return sendSuccess(data);
});

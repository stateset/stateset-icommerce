import { NextRequest } from 'next/server';
import { withErrorHandler } from '@/lib/shared/with-error-handler';
import { sendSuccess } from '@/lib/shared/response';
import { AppError } from '@/lib/shared/errors';
import { clearSessionCookie, isAdminAuthDisabled, requireRequestSessionToken } from '@/lib/shared/auth-session';
import { getServerStateSetApiUrl } from '@/lib/stateset-api-url';

const API_URL = getServerStateSetApiUrl();

/**
 * POST /api/auth/logout
 *
 * Invalidate the current session.
 */
export const POST = withErrorHandler(async (request: NextRequest) => {
  if (isAdminAuthDisabled()) {
    const logoutResponse = sendSuccess({ message: 'Logged out successfully' });
    clearSessionCookie(logoutResponse);
    return logoutResponse;
  }

  const token = requireRequestSessionToken(request);

  const response = await fetch(`${API_URL}/api/auth/logout`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
  });

  if (!response.ok && response.status !== 401 && response.status !== 403) {
    const errorData = await response.json().catch(() => ({ error: 'Unknown error' }));
    throw new AppError(errorData.error || 'Logout failed', response.status, 'AUTH_ERROR');
  }

  const logoutResponse = sendSuccess({ message: 'Logged out successfully' });
  clearSessionCookie(logoutResponse);
  return logoutResponse;
}, { requireCsrf: true });

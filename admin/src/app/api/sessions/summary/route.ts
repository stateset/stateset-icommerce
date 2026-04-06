import { NextRequest } from 'next/server';
import { withErrorHandler } from '@/lib/shared/with-error-handler';
import { sendSuccess } from '@/lib/shared/response';
import { AppError } from '@/lib/shared/errors';
import { getRequestSessionToken, getServiceSessionToken, isAdminAuthDisabled } from '@/lib/shared/auth-session';
import { getServerStateSetApiUrl } from '@/lib/stateset-api-url';

const API_URL = getServerStateSetApiUrl();

/**
 * GET /api/sessions/summary
 *
 * Fetches summary statistics for agent sessions.
 */
export const GET = withErrorHandler(async (request: NextRequest) => {
  const token = getRequestSessionToken(request) ?? (isAdminAuthDisabled() ? getServiceSessionToken() : null);

  if (!token) {
    if (isAdminAuthDisabled()) {
      return sendSuccess({
        total: 0,
        by_status: {
          pending: 0,
          running: 0,
          rotating: 0,
          paused: 0,
          completed: 0,
          failed: 0,
          cancelled: 0,
        },
        active_now: 0,
        rotations_last_hour: 0,
        avg_duration_seconds: 0,
      });
    }
    throw AppError.unauthorized('Authentication required');
  }

  const response = await fetch(`${API_URL}/api/admin/agent-sessions/summary`, {
    method: 'GET',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
  });

  if (!response.ok) {
    if (response.status === 401 || response.status === 403) {
      throw AppError.unauthorized('Session expired');
    }
    const errorData = await response.json().catch(() => ({ error: 'Unknown error' }));
    throw new AppError(
      errorData.error || `API request failed: ${response.status}`,
      response.status,
      'UPSTREAM_ERROR'
    );
  }

  const data = await response.json();
  return sendSuccess(data);
});

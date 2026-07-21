import { NextRequest } from 'next/server';
import { withErrorHandler } from '@/lib/shared/with-error-handler';
import { sendSuccess } from '@/lib/shared/response';
import { AppError } from '@/lib/shared/errors';
import { requireRequestSessionToken } from '@/lib/shared/auth-session';
import { getServerStateSetApiUrl } from '@/lib/stateset-api-url';

const API_URL = getServerStateSetApiUrl();

/**
 * GET /api/integrations/status
 *
 * Get the status of all configured integrations.
 */
export const GET = withErrorHandler(async (request: NextRequest) => {
  const token = requireRequestSessionToken(request);

  const response = await fetch(`${API_URL}/api/integrations/status`, {
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
      errorData.error || 'Failed to fetch integration status',
      response.status,
      'INTEGRATION_ERROR',
    );
  }

  const data = await response.json();
  return sendSuccess(data);
});

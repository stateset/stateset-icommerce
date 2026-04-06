import { NextRequest } from 'next/server';
import { withErrorHandler } from '@/lib/shared/with-error-handler';
import { sendPaginated } from '@/lib/shared/response';
import { validateQuery, listSessionsSchema } from '@/lib/shared/schemas';
import { AppError } from '@/lib/shared/errors';
import { getRequestSessionToken, getServiceSessionToken, isAdminAuthDisabled } from '@/lib/shared/auth-session';
import { getServerStateSetApiUrl } from '@/lib/stateset-api-url';

const API_URL = getServerStateSetApiUrl();

/**
 * GET /api/sessions
 *
 * Proxies requests to the StateSet Sandbox API for agent sessions.
 * Query params: limit, offset, status, org_id, search
 */
export const GET = withErrorHandler(async (request: NextRequest) => {
  // Validate query params
  const validation = validateQuery(request.nextUrl.searchParams, listSessionsSchema);
  if (!validation.success) {
    throw AppError.validationError(validation.errors.map(e => `${e.field}: ${e.message}`).join('; '));
  }

  const { limit, offset, status, org_id, search } = validation.data;
  const token = getRequestSessionToken(request) ?? (isAdminAuthDisabled() ? getServiceSessionToken() : null);

  if (!token) {
    if (isAdminAuthDisabled()) {
      return sendPaginated([], {
        total: 0,
        limit: limit || 20,
        offset: offset || 0,
      });
    }
    throw AppError.unauthorized('Authentication required');
  }

  const searchParams = new URLSearchParams();
  if (limit) searchParams.set('limit', String(limit));
  if (offset) searchParams.set('offset', String(offset));
  if (status) searchParams.set('status', status);
  if (org_id) searchParams.set('org_id', org_id);
  if (search) searchParams.set('search', search);

  const queryString = searchParams.toString();
  const endpoint = `/api/admin/agent-sessions${queryString ? `?${queryString}` : ''}`;

  const response = await fetch(`${API_URL}${endpoint}`, {
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

  return sendPaginated(data.sessions || [], {
    total: data.total || 0,
    limit: limit || 20,
    offset: offset || 0,
  });
});

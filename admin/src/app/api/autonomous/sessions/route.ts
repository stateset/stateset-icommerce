import { NextRequest } from 'next/server';
import { withErrorHandler } from '@/lib/shared/with-error-handler';
import { sendSuccess, sendPaginated } from '@/lib/shared/response';
import { AppError } from '@/lib/shared/errors';
import {
  validateBody,
  validateQuery,
  createAutonomousSessionSchema,
  paginationQuerySchema,
} from '@/lib/shared/schemas';
import { requireRequestSessionToken } from '@/lib/shared/auth-session';
import { getServerStateSetApiUrl } from '@/lib/stateset-api-url';

const API_URL = getServerStateSetApiUrl();

/**
 * GET /api/autonomous/sessions
 *
 * List autonomous sessions.
 */
export const GET = withErrorHandler(async (request: NextRequest) => {
  const token = requireRequestSessionToken(request);
  const queryValidation = validateQuery(request.nextUrl.searchParams, paginationQuerySchema);
  if (!queryValidation.success) {
    throw AppError.validationError(
      queryValidation.errors.map((e) => `${e.field}: ${e.message}`).join('; '),
    );
  }

  const { limit = 20, offset = 0 } = queryValidation.data;
  const params = new URLSearchParams({ limit: String(limit), offset: String(offset) });

  const response = await fetch(`${API_URL}/api/autonomous/sessions?${params}`, {
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
      errorData.error || 'Failed to fetch sessions',
      response.status,
      'AUTONOMOUS_ERROR',
    );
  }

  const data = await response.json();
  return sendPaginated(data.sessions || [], {
    total: data.total || 0,
    limit,
    offset,
  });
});

/**
 * POST /api/autonomous/sessions
 *
 * Create a new autonomous session.
 */
export const POST = withErrorHandler(
  async (request: NextRequest) => {
    const token = requireRequestSessionToken(request);
    const body = await request.json();
    const validation = validateBody(body, createAutonomousSessionSchema);
    if (!validation.success) {
      throw AppError.validationError(
        validation.errors.map((e) => `${e.field}: ${e.message}`).join('; '),
      );
    }

    const response = await fetch(`${API_URL}/api/autonomous/sessions`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${token}`,
      },
      body: JSON.stringify(validation.data),
    });

    if (!response.ok) {
      if (response.status === 401 || response.status === 403) {
        throw AppError.unauthorized('Session expired');
      }
      const errorData = await response.json().catch(() => ({ error: 'Unknown error' }));
      throw new AppError(
        errorData.error || 'Failed to create session',
        response.status,
        'AUTONOMOUS_ERROR',
      );
    }

    const data = await response.json();
    return sendSuccess(data, 201);
  },
  { requireCsrf: true },
);

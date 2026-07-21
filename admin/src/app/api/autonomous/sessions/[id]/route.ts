import { NextRequest } from 'next/server';
import { withErrorHandler } from '@/lib/shared/with-error-handler';
import { sendSuccess } from '@/lib/shared/response';
import { AppError } from '@/lib/shared/errors';
import { validateBody, sessionActionSchema, safeIdSchema } from '@/lib/shared/schemas';
import { requireRequestSessionToken } from '@/lib/shared/auth-session';
import { getServerStateSetApiUrl } from '@/lib/stateset-api-url';

const API_URL = getServerStateSetApiUrl();

/**
 * GET /api/autonomous/sessions/:id
 *
 * Get details of a specific autonomous session.
 */
export const GET = withErrorHandler(
  async (request: NextRequest, context?: { params: Promise<Record<string, string>> }) => {
    const { id: rawId } = await context!.params;
    const idResult = safeIdSchema.safeParse(rawId);
    if (!idResult.success) throw AppError.badRequest('Invalid session ID');
    const id = idResult.data;
    const token = requireRequestSessionToken(request);

    const response = await fetch(`${API_URL}/api/autonomous/sessions/${id}`, {
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
      if (response.status === 404) throw AppError.notFound('Session not found');
      const errorData = await response.json().catch(() => ({ error: 'Unknown error' }));
      throw new AppError(
        errorData.error || 'Failed to fetch session',
        response.status,
        'AUTONOMOUS_ERROR',
      );
    }

    const data = await response.json();
    return sendSuccess(data);
  },
);

/**
 * POST /api/autonomous/sessions/:id
 *
 * Perform an action on a session (start, pause, cancel).
 */
export const POST = withErrorHandler(
  async (request: NextRequest, context?: { params: Promise<Record<string, string>> }) => {
    const { id: rawId } = await context!.params;
    const idResult = safeIdSchema.safeParse(rawId);
    if (!idResult.success) throw AppError.badRequest('Invalid session ID');
    const id = idResult.data;
    const token = requireRequestSessionToken(request);

    const body = await request.json();
    const validation = validateBody(body, sessionActionSchema);
    if (!validation.success) {
      throw AppError.validationError(
        validation.errors.map((e) => `${e.field}: ${e.message}`).join('; '),
      );
    }

    const { action } = validation.data;

    const response = await fetch(`${API_URL}/api/autonomous/sessions/${id}/${action}`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${token}`,
      },
    });

    if (!response.ok) {
      if (response.status === 401 || response.status === 403) {
        throw AppError.unauthorized('Session expired');
      }
      if (response.status === 404) throw AppError.notFound('Session not found');
      if (response.status === 409)
        throw AppError.conflict(`Cannot ${action} session in its current state`);
      const errorData = await response.json().catch(() => ({ error: 'Unknown error' }));
      throw new AppError(
        errorData.error || `Failed to ${action} session`,
        response.status,
        'AUTONOMOUS_ERROR',
      );
    }

    const data = await response.json();
    return sendSuccess(data);
  },
  { requireCsrf: true },
);

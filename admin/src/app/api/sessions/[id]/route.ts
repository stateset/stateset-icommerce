import { NextRequest } from 'next/server';
import { withErrorHandler } from '@/lib/shared/with-error-handler';
import { sendSuccess } from '@/lib/shared/response';
import { AppError } from '@/lib/shared/errors';
import { validateBody, cancelSessionSchema, safeIdSchema } from '@/lib/shared/schemas';
import {
  getRequestSessionToken,
  getServiceSessionToken,
  isAdminAuthDisabled,
} from '@/lib/shared/auth-session';
import { getServerStateSetApiUrl } from '@/lib/stateset-api-url';

const API_URL = getServerStateSetApiUrl();

/**
 * GET /api/sessions/:id
 *
 * Fetches a specific agent session with its events.
 */
export const GET = withErrorHandler(
  async (request: NextRequest, context?: { params: Promise<Record<string, string>> }) => {
    const { id: rawId } = await context!.params;
    const idResult = safeIdSchema.safeParse(rawId);
    if (!idResult.success) throw AppError.badRequest('Invalid session ID');
    const id = idResult.data;
    const token =
      getRequestSessionToken(request) ?? (isAdminAuthDisabled() ? getServiceSessionToken() : null);

    if (!token) {
      if (isAdminAuthDisabled()) {
        return sendSuccess({ session: null, events: [] });
      }
      throw AppError.unauthorized('Authentication required');
    }

    const response = await fetch(`${API_URL}/api/admin/agent-sessions/${id}`, {
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
        'UPSTREAM_ERROR',
      );
    }

    const data = await response.json();
    return sendSuccess(data);
  },
);

/**
 * POST /api/sessions/:id
 *
 * Cancel a session (action: cancel)
 */
export const POST = withErrorHandler(
  async (request: NextRequest, context?: { params: Promise<Record<string, string>> }) => {
    const { id: rawId } = await context!.params;
    const idResult = safeIdSchema.safeParse(rawId);
    if (!idResult.success) throw AppError.badRequest('Invalid session ID');
    const id = idResult.data;
    const token =
      getRequestSessionToken(request) ?? (isAdminAuthDisabled() ? getServiceSessionToken() : null);

    const body = await request.json().catch((err: unknown) => {
      console.warn('[sessions] Failed to parse request body:', (err as Error).message);
      return {};
    });
    const validation = validateBody(body, cancelSessionSchema);
    if (!validation.success) {
      throw AppError.validationError(
        validation.errors.map((e) => `${e.field}: ${e.message}`).join('; '),
      );
    }

    if (!token) {
      throw new AppError(
        'Session control requires STATESET_API_TOKEN when admin auth is disabled',
        503,
        'AUTH_DISABLED_NO_SERVICE_TOKEN',
      );
    }

    const response = await fetch(`${API_URL}/api/admin/agent-sessions/${id}/cancel`, {
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
      const errorData = await response.json().catch(() => ({ error: 'Unknown error' }));
      throw new AppError(
        errorData.error || `API request failed: ${response.status}`,
        response.status,
        'UPSTREAM_ERROR',
      );
    }

    const data = await response.json();
    return sendSuccess(data);
  },
  { requireCsrf: true },
);

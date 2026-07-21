import { NextRequest } from 'next/server';
import { withErrorHandler } from '@/lib/shared/with-error-handler';
import { sendSuccess, sendPaginated } from '@/lib/shared/response';
import { AppError } from '@/lib/shared/errors';
import {
  validateBody,
  validateQuery,
  integrationCredentialSchema,
  paginationQuerySchema,
  safeIdSchema,
} from '@/lib/shared/schemas';
import { requireRequestSessionToken } from '@/lib/shared/auth-session';
import { getServerStateSetApiUrl } from '@/lib/stateset-api-url';

const API_URL = getServerStateSetApiUrl();

/**
 * GET /api/integrations/credentials
 *
 * List all integration credentials for the organization.
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

  const response = await fetch(`${API_URL}/api/integrations/credentials?${params}`, {
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
      errorData.error || 'Failed to fetch credentials',
      response.status,
      'INTEGRATION_ERROR',
    );
  }

  const data = await response.json();
  return sendPaginated(data.credentials || [], {
    total: data.total || 0,
    limit,
    offset,
  });
});

/**
 * POST /api/integrations/credentials
 *
 * Create a new integration credential.
 */
export const POST = withErrorHandler(
  async (request: NextRequest) => {
    const token = requireRequestSessionToken(request);
    const body = await request.json();
    const validation = validateBody(body, integrationCredentialSchema);
    if (!validation.success) {
      throw AppError.validationError(
        validation.errors.map((e) => `${e.field}: ${e.message}`).join('; '),
      );
    }

    const response = await fetch(`${API_URL}/api/integrations/credentials`, {
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
        errorData.error || 'Failed to create credential',
        response.status,
        'INTEGRATION_ERROR',
      );
    }

    const data = await response.json();
    return sendSuccess(data, 201);
  },
  { requireCsrf: true },
);

/**
 * DELETE /api/integrations/credentials
 *
 * Delete an integration credential by ID (passed as query param).
 */
export const DELETE = withErrorHandler(
  async (request: NextRequest) => {
    const token = requireRequestSessionToken(request);
    const rawId = request.nextUrl.searchParams.get('id');
    const idResult = safeIdSchema.safeParse(rawId);
    if (!idResult.success) {
      throw AppError.badRequest('Invalid credential ID');
    }
    const id = idResult.data;

    const response = await fetch(`${API_URL}/api/integrations/credentials/${id}`, {
      method: 'DELETE',
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
        errorData.error || 'Failed to delete credential',
        response.status,
        'INTEGRATION_ERROR',
      );
    }

    return sendSuccess({ message: 'Credential deleted successfully' });
  },
  { requireCsrf: true },
);

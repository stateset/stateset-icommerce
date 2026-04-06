import { NextRequest } from 'next/server';
import { withErrorHandler } from '@/lib/shared/with-error-handler';
import { sendSuccess } from '@/lib/shared/response';
import { AppError } from '@/lib/shared/errors';
import { validateBody, registerSchema } from '@/lib/shared/schemas';
import { getServerStateSetApiUrl } from '@/lib/stateset-api-url';

const API_URL = getServerStateSetApiUrl();

/**
 * POST /api/auth/register
 *
 * Register a new user account.
 */
export const POST = withErrorHandler(async (request: NextRequest) => {
  const body = await request.json();
  const validation = validateBody(body, registerSchema);
  if (!validation.success) {
    throw AppError.validationError(validation.errors.map(e => `${e.field}: ${e.message}`).join('; '));
  }

  const response = await fetch(`${API_URL}/api/auth/register`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(validation.data),
  });

  if (!response.ok) {
    if (response.status === 409) {
      throw AppError.conflict('An account with this email already exists');
    }
    const errorData = await response.json().catch(() => ({ error: 'Unknown error' }));
    throw new AppError(errorData.error || 'Registration failed', response.status, 'AUTH_ERROR');
  }

  const data = await response.json();
  return sendSuccess(data, 201);
}, { requireCsrf: true });

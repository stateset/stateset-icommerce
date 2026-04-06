import { NextRequest } from 'next/server';
import { withErrorHandler } from '@/lib/shared/with-error-handler';
import { sendSuccess } from '@/lib/shared/response';
import { AppError } from '@/lib/shared/errors';
import { validateBody, verifyEmailSchema } from '@/lib/shared/schemas';
import { getServerStateSetApiUrl } from '@/lib/stateset-api-url';

const API_URL = getServerStateSetApiUrl();

/**
 * POST /api/auth/verify-email
 *
 * Verify a user's email address using a token.
 */
export const POST = withErrorHandler(async (request: NextRequest) => {
  const body = await request.json();
  const validation = validateBody(body, verifyEmailSchema);
  if (!validation.success) {
    throw AppError.validationError(validation.errors.map(e => `${e.field}: ${e.message}`).join('; '));
  }

  const response = await fetch(`${API_URL}/api/auth/verify-email`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(validation.data),
  });

  if (!response.ok) {
    if (response.status === 400) {
      throw AppError.badRequest('Invalid or expired verification token');
    }
    const errorData = await response.json().catch(() => ({ error: 'Unknown error' }));
    throw new AppError(errorData.error || 'Email verification failed', response.status, 'AUTH_ERROR');
  }

  return sendSuccess({ message: 'Email verified successfully.' });
}, { requireCsrf: true });

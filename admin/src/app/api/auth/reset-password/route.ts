import { NextRequest } from 'next/server';
import { withErrorHandler } from '@/lib/shared/with-error-handler';
import { sendSuccess } from '@/lib/shared/response';
import { AppError } from '@/lib/shared/errors';
import { validateBody, resetPasswordSchema } from '@/lib/shared/schemas';
import { getServerStateSetApiUrl } from '@/lib/stateset-api-url';

const API_URL = getServerStateSetApiUrl();

/**
 * POST /api/auth/reset-password
 *
 * Reset password using a token from email.
 */
export const POST = withErrorHandler(
  async (request: NextRequest) => {
    const body = await request.json();
    const validation = validateBody(body, resetPasswordSchema);
    if (!validation.success) {
      throw AppError.validationError(
        validation.errors.map((e) => `${e.field}: ${e.message}`).join('; '),
      );
    }

    const response = await fetch(`${API_URL}/api/auth/reset-password`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(validation.data),
    });

    if (!response.ok) {
      if (response.status === 400) {
        throw AppError.badRequest('Invalid or expired reset token');
      }
      const errorData = await response.json().catch(() => ({ error: 'Unknown error' }));
      throw new AppError(errorData.error || 'Password reset failed', response.status, 'AUTH_ERROR');
    }

    return sendSuccess({ message: 'Password has been reset successfully.' });
  },
  { requireCsrf: true },
);

import { NextRequest } from 'next/server';
import { withErrorHandler } from '@/lib/shared/with-error-handler';
import { sendSuccess } from '@/lib/shared/response';
import { AppError } from '@/lib/shared/errors';
import { validateBody, forgotPasswordSchema } from '@/lib/shared/schemas';
import { getServerStateSetApiUrl } from '@/lib/stateset-api-url';

const API_URL = getServerStateSetApiUrl();

/**
 * POST /api/auth/forgot-password
 *
 * Request a password reset email.
 */
export const POST = withErrorHandler(
  async (request: NextRequest) => {
    const body = await request.json();
    const validation = validateBody(body, forgotPasswordSchema);
    if (!validation.success) {
      throw AppError.validationError(
        validation.errors.map((e) => `${e.field}: ${e.message}`).join('; '),
      );
    }

    await fetch(`${API_URL}/api/auth/forgot-password`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(validation.data),
    });

    // Always return success to prevent email enumeration
    return sendSuccess({
      message: 'If an account exists with that email, a reset link has been sent.',
    });
  },
  { requireCsrf: true },
);

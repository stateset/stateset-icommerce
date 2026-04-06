/**
 * GET /api/auth/csrf-token
 *
 * Returns a CSRF token in the response body so the frontend can include it
 * in request headers for state-changing operations. The token is also set
 * as an HttpOnly cookie for server-side validation.
 */

import { withErrorHandler } from '@/lib/shared/with-error-handler';
import { sendSuccess } from '@/lib/shared/response';
import { getOrCreateCsrfToken } from '@/lib/shared/csrf';

export const GET = withErrorHandler(async () => {
  const token = await getOrCreateCsrfToken();
  return sendSuccess({ csrfToken: token });
});

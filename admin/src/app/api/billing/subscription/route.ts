import { NextRequest } from 'next/server';
import { withErrorHandler } from '@/lib/shared/with-error-handler';
import { sendSuccess } from '@/lib/shared/response';
import { AppError } from '@/lib/shared/errors';
import { validateBody, createSubscriptionSchema, updateSubscriptionSchema } from '@/lib/shared/schemas';
import { requireRequestSessionToken } from '@/lib/shared/auth-session';
import { getServerStateSetApiUrl } from '@/lib/stateset-api-url';

const API_URL = getServerStateSetApiUrl();

/**
 * GET /api/billing/subscription
 *
 * Get the current organization's subscription details.
 */
export const GET = withErrorHandler(async (request: NextRequest) => {
  const token = requireRequestSessionToken(request);

  const response = await fetch(`${API_URL}/api/billing/subscription`, {
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
    throw new AppError(errorData.error || 'Failed to fetch subscription', response.status, 'BILLING_ERROR');
  }

  const data = await response.json();
  return sendSuccess(data);
});

/**
 * POST /api/billing/subscription
 *
 * Create a new subscription.
 */
export const POST = withErrorHandler(async (request: NextRequest) => {
  const token = requireRequestSessionToken(request);
  const body = await request.json();
  const validation = validateBody(body, createSubscriptionSchema);
  if (!validation.success) {
    throw AppError.validationError(validation.errors.map(e => `${e.field}: ${e.message}`).join('; '));
  }

  const response = await fetch(`${API_URL}/api/billing/subscription`, {
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
    throw new AppError(errorData.error || 'Failed to create subscription', response.status, 'BILLING_ERROR');
  }

  const data = await response.json();
  return sendSuccess(data, 201);
}, { requireCsrf: true });

/**
 * PATCH /api/billing/subscription
 *
 * Update an existing subscription.
 */
export const PATCH = withErrorHandler(async (request: NextRequest) => {
  const token = requireRequestSessionToken(request);
  const body = await request.json();
  const validation = validateBody(body, updateSubscriptionSchema);
  if (!validation.success) {
    throw AppError.validationError(validation.errors.map(e => `${e.field}: ${e.message}`).join('; '));
  }

  const response = await fetch(`${API_URL}/api/billing/subscription`, {
    method: 'PATCH',
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
    throw new AppError(errorData.error || 'Failed to update subscription', response.status, 'BILLING_ERROR');
  }

  const data = await response.json();
  return sendSuccess(data);
}, { requireCsrf: true });

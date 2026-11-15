import 'server-only';
import { requireAdminSession, isAdminAuthDisabled, getBypassAdminUser } from './auth-session';
import { getServerStateSetApiUrl } from '@/lib/stateset-api-url';
import { AppError } from './errors';

/** Resolve privilege and audit identity from the identity service, never form data. */
export async function requireFinanceActor(): Promise<string> {
  const token = await requireAdminSession();
  if (isAdminAuthDisabled()) return getBypassAdminUser().id;
  const response = await fetch(`${getServerStateSetApiUrl()}/api/auth/me`, {
    headers: { Authorization: `Bearer ${token}` },
    cache: 'no-store',
  });
  if (!response.ok) throw AppError.unauthorized('Your session could not be verified');
  const body = await response.json();
  const user = body.user ?? body.data?.user ?? body.data ?? body;
  if (!['admin', 'owner'].includes(user.role)) {
    throw AppError.forbidden('Finance actions require an administrator');
  }
  if (typeof user.id !== 'string' || !user.id.trim()) {
    throw AppError.unauthorized('The authenticated user has no audit identity');
  }
  return user.id;
}

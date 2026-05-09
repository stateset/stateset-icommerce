// Active organization selection — cookie-backed, single source of truth
// for which org the operator is currently scoped to.
//
// The audit noted: "orgId extraction in auth-session and middleware suggests
// multi-tenancy plumbing, but dashboard shows no org picker". This module
// provides that picker layer. Server-side helpers read the cookie; the
// `<OrgSwitcher />` client component sets it via a server action.

import { cookies } from 'next/headers';

/**
 * Cookie name. Plain string (not HttpOnly) so the client can read it for
 * the switcher UI without an extra round-trip. The session cookie is the
 * security boundary; this is just operator preference state.
 */
export const ACTIVE_ORG_COOKIE = 'stateset_active_org';

/** Maximum length we'll accept for an org id, to bound cookie size. */
const MAX_ORG_ID_LEN = 128;

/** Validation: limit to URL-safe chars + bound length. */
export function isValidOrgId(value: unknown): value is string {
  if (typeof value !== 'string') return false;
  if (!value || value.length > MAX_ORG_ID_LEN) return false;
  return /^[A-Za-z0-9_.-]+$/.test(value);
}

/**
 * Server-side accessor: read the active org from the request cookie.
 * Returns `null` when unset or invalid.
 */
export async function getActiveOrgId(): Promise<string | null> {
  const store = await cookies();
  const value = store.get(ACTIVE_ORG_COOKIE)?.value;
  return isValidOrgId(value) ? value : null;
}

/**
 * Cookie spec used by the server action. Centralised so future tweaks
 * (SameSite, Secure, max-age) are made in one place.
 */
export const ACTIVE_ORG_COOKIE_OPTIONS = {
  name: ACTIVE_ORG_COOKIE,
  httpOnly: false,
  sameSite: 'lax' as const,
  secure: process.env.NODE_ENV === 'production',
  path: '/',
  // 30 days — operator preference, not a security boundary.
  maxAge: 60 * 60 * 24 * 30,
};

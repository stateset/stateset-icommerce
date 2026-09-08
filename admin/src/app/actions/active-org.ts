'use server';

// Server actions for the active-org switcher. The cookie write happens
// via Next.js's mutable cookie store, so this must be a Server Action
// (not a regular server function).

import { cookies } from 'next/headers';

import { ACTIVE_ORG_COOKIE, ACTIVE_ORG_COOKIE_OPTIONS } from '@/lib/shared/active-org';
import { requireAdminSession } from '@/lib/shared/auth-session';
import { setActiveOrgArgsSchema, validateArgs } from '@/lib/shared/schemas';

/**
 * Set the active org cookie. Called by `<OrgSwitcher />` when the
 * operator picks a different org from the dropdown.
 *
 * Gated by `requireAdminSession()` — server actions bypass the API
 * middleware, and `<OrgSwitcher />` only renders post-auth, so no
 * pre-auth flow depends on this action.
 *
 * Throws on invalid input — the switcher UI is responsible for offering
 * only valid options, so an invalid arg here means a programming bug. The
 * throw is the same typed `ValidationError` (422, per-field details) the rest
 * of the server-action surface raises, so a caller can branch on it instead
 * of string-matching a bare Error.
 */
export async function setActiveOrg(orgId: string): Promise<void> {
  await requireAdminSession();
  const args = validateArgs({ orgId }, setActiveOrgArgsSchema);
  const store = await cookies();
  store.set({
    ...ACTIVE_ORG_COOKIE_OPTIONS,
    value: args.orgId,
  });
}

/**
 * Clear the active org cookie. Switches the operator back to their
 * default org (whichever the API treats as default when no header is
 * present).
 */
export async function clearActiveOrg(): Promise<void> {
  await requireAdminSession();
  const store = await cookies();
  store.delete(ACTIVE_ORG_COOKIE);
}

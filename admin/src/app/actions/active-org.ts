'use server';

// Server actions for the active-org switcher. The cookie write happens
// via Next.js's mutable cookie store, so this must be a Server Action
// (not a regular server function).

import { cookies } from 'next/headers';

import {
  ACTIVE_ORG_COOKIE,
  ACTIVE_ORG_COOKIE_OPTIONS,
  isValidOrgId,
} from '@/lib/shared/active-org';

/**
 * Set the active org cookie. Called by `<OrgSwitcher />` when the
 * operator picks a different org from the dropdown.
 *
 * Throws on invalid input — the switcher UI is responsible for offering
 * only valid options, so an invalid arg here means a programming bug.
 */
export async function setActiveOrg(orgId: string): Promise<void> {
  if (!isValidOrgId(orgId)) {
    throw new Error(`Invalid orgId: must be 1-128 URL-safe chars, got ${JSON.stringify(orgId)}`);
  }
  const store = await cookies();
  store.set({
    ...ACTIVE_ORG_COOKIE_OPTIONS,
    value: orgId,
  });
}

/**
 * Clear the active org cookie. Switches the operator back to their
 * default org (whichever the API treats as default when no header is
 * present).
 */
export async function clearActiveOrg(): Promise<void> {
  const store = await cookies();
  store.delete(ACTIVE_ORG_COOKIE);
}

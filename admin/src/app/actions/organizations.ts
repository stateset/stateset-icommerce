'use server';

// Server-side actions for the operator's organization list.
//
// Wiring note: the StateSet HTTP service does not currently expose a
// "list my organizations" endpoint. Until it does, this module returns
// either:
//   1. An empty list (production default) — `<OrgSwitcher />` then
//      hides itself, matching the "renders nothing on ≤1 option" rule
//      tested in firing #30.
//   2. A small mock list when `NEXT_PUBLIC_ADMIN_DEV_ORGS` is set (for
//      local UI development against the multi-org switcher).
//
// When the backend lands, replace the mock branch with a `fetch` to
// `${STATESET_API_URL}/api/v1/organizations` carrying the session
// token, mirroring the pattern in `app/api/sessions/route.ts`.

import { requireAdminSession } from '@/lib/shared/auth-session';

interface OrgOption {
  id: string;
  name: string;
}

/**
 * Returns the org list visible to the current operator. Always serializable
 * — safe to pass directly to a Client Component.
 *
 * Gated by `requireAdminSession()` — server actions bypass the API
 * middleware. The only render-time caller (`<TopBar />`) sits in the
 * authenticated branch of the root layout, so the session cookie is
 * always present (or auth is disabled) when this runs during render.
 */
export async function listOrganizations(): Promise<OrgOption[]> {
  await requireAdminSession();
  const dev = process.env.NEXT_PUBLIC_ADMIN_DEV_ORGS?.trim();
  if (dev) {
    return dev
      .split(',')
      .map((entry) => entry.trim())
      .filter(Boolean)
      .map((entry) => {
        // Accept "id" or "id:Display Name" so devs can label orgs in the
        // env var without writing a JSON config.
        const [id, ...rest] = entry.split(':');
        const name = rest.join(':').trim() || id;
        return { id: id.trim(), name };
      });
  }
  return [];
}

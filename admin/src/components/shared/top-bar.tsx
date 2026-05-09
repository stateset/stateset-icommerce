// Top bar component rendered in the admin layout above the main content
// area. Currently houses the multi-org switcher; future: notifications,
// quick search, dark-mode toggle.
//
// Server Component — fetches the active org + org list at request time so
// server components reading `getActiveOrgId()` see a consistent view.

import { listOrganizations } from '@/app/actions/organizations';
import { getActiveOrgId } from '@/lib/shared/active-org';
import { OrgSwitcher } from '@/components/shared/org-switcher';

export async function TopBar() {
  const [activeOrgId, organizations] = await Promise.all([
    getActiveOrgId(),
    listOrganizations(),
  ]);

  // OrgSwitcher hides itself when there's ≤ 1 option; until the org-list
  // API exists, that's the production default. Devs can flip
  // NEXT_PUBLIC_ADMIN_DEV_ORGS=acme,globex to see the dropdown locally.
  const hasOrgSwitcher = organizations.length > 1;
  if (!hasOrgSwitcher) return null;

  return (
    <div className="border-b border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900/50">
      <div className="container mx-auto px-6 py-2 flex items-center justify-end gap-4">
        <OrgSwitcher options={organizations} activeOrgId={activeOrgId} />
      </div>
    </div>
  );
}

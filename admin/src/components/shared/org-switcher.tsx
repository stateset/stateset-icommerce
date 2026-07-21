'use client';

import { useCallback, useState, useTransition } from 'react';
import { useRouter } from 'next/navigation';

import { Badge, Button } from '@stateset/design';
import { clearActiveOrg, setActiveOrg } from '@/app/actions/active-org';

interface OrgOption {
  id: string;
  name: string;
}

interface OrgSwitcherProps {
  /** Available organizations the operator can scope to. */
  options: OrgOption[];
  /** Currently selected org id, or null when no override is set. */
  activeOrgId: string | null;
}

/**
 * Compact dropdown that scopes the admin to a specific org.
 *
 * Writes a cookie (`stateset_active_org`) read by `with-error-handler` and
 * threaded through to the upstream API as `x-org-id`. The dropdown also
 * carries a "Clear scope" option so operators can return to whatever the
 * server treats as default (typically their primary org).
 *
 * Renders nothing when there's only one option — there's no choice to make.
 */
export function OrgSwitcher({ options, activeOrgId }: OrgSwitcherProps) {
  const router = useRouter();
  const [pending, startTransition] = useTransition();
  const [error, setError] = useState<string | null>(null);

  const onSelect = useCallback(
    (next: string) => {
      setError(null);
      startTransition(async () => {
        try {
          if (next === '__clear__') {
            await clearActiveOrg();
          } else {
            await setActiveOrg(next);
          }
          // Force a server-component re-render so anything reading orgId
          // (request-scoped fetches, page metadata) sees the new value.
          router.refresh();
        } catch (err) {
          setError(err instanceof Error ? err.message : 'Switch failed');
        }
      });
    },
    [router],
  );

  if (options.length <= 1) return null;

  const activeName = activeOrgId
    ? (options.find((o) => o.id === activeOrgId)?.name ?? activeOrgId)
    : 'Default';

  return (
    <div className="flex items-center gap-2">
      <Badge variant="primary">org</Badge>
      <select
        className="ds-focus-ring rounded-md border border-ds-input bg-ds-background px-3 py-1.5 text-sm text-ds-foreground"
        value={activeOrgId ?? '__clear__'}
        onChange={(e) => onSelect(e.target.value)}
        disabled={pending}
        aria-label="Switch active organization"
      >
        <option value="__clear__">Default scope</option>
        {options.map((o) => (
          <option key={o.id} value={o.id}>
            {o.name}
          </option>
        ))}
      </select>
      {pending && <span className="text-xs text-ds-muted-foreground">switching…</span>}
      {!pending && activeOrgId && (
        <Button
          size="sm"
          variant="ghost"
          onClick={() => onSelect('__clear__')}
          aria-label="Clear active org override"
          title={`Currently scoped to ${activeName}`}
        >
          Clear
        </Button>
      )}
      {error && (
        <span className="text-xs text-ds-destructive" role="alert">
          {error}
        </span>
      )}
    </div>
  );
}

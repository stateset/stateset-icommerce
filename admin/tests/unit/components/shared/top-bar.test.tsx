// Server-component test for the admin TopBar. The component is async, so
// we resolve its promise to a React element and inspect the result.
//
// TopBar fetches the active org id + the org list at request time and
// hides itself when there is ≤ 1 option. We mock both server-action
// surfaces (`getActiveOrgId`, `listOrganizations`) and the OrgSwitcher
// so the assertions stay focused on TopBar's own logic (the gating rule
// and the wiring of props into OrgSwitcher).

import { render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const listOrganizationsMock = vi.fn();
const getActiveOrgIdMock = vi.fn();

vi.mock('@/app/actions/organizations', () => ({
  listOrganizations: () => listOrganizationsMock(),
}));

vi.mock('@/lib/shared/active-org', () => ({
  getActiveOrgId: () => getActiveOrgIdMock(),
}));

vi.mock('@/components/shared/org-switcher', () => ({
  OrgSwitcher: ({ options, activeOrgId }: { options: { id: string; name: string }[]; activeOrgId: string | null }) => (
    <div data-testid="org-switcher" data-active={activeOrgId ?? ''}>
      {options.map((o) => (
        <span key={o.id}>{o.name}</span>
      ))}
    </div>
  ),
}));

import { TopBar } from '@/components/shared/top-bar';

afterEach(() => {
  vi.clearAllMocks();
});

describe('TopBar', () => {
  it('renders nothing when only one organization is available', async () => {
    getActiveOrgIdMock.mockResolvedValue(null);
    listOrganizationsMock.mockResolvedValue([{ id: 'acme', name: 'Acme' }]);
    const tree = await TopBar();
    expect(tree).toBeNull();
  });

  it('renders nothing when the org list is empty', async () => {
    getActiveOrgIdMock.mockResolvedValue(null);
    listOrganizationsMock.mockResolvedValue([]);
    const tree = await TopBar();
    expect(tree).toBeNull();
  });

  it('renders the OrgSwitcher with all options when ≥ 2 orgs exist', async () => {
    getActiveOrgIdMock.mockResolvedValue('globex');
    listOrganizationsMock.mockResolvedValue([
      { id: 'acme', name: 'Acme' },
      { id: 'globex', name: 'Globex' },
    ]);
    const tree = await TopBar();
    render(tree as JSX.Element);
    const switcher = screen.getByTestId('org-switcher');
    expect(switcher).toBeInTheDocument();
    expect(switcher.dataset.active).toBe('globex');
    expect(screen.getByText('Acme')).toBeInTheDocument();
    expect(screen.getByText('Globex')).toBeInTheDocument();
  });

  it('passes a null activeOrgId through unchanged', async () => {
    getActiveOrgIdMock.mockResolvedValue(null);
    listOrganizationsMock.mockResolvedValue([
      { id: 'a', name: 'A' },
      { id: 'b', name: 'B' },
    ]);
    const tree = await TopBar();
    render(tree as JSX.Element);
    expect(screen.getByTestId('org-switcher').dataset.active).toBe('');
  });
});

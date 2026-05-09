// Component tests for the multi-org switcher dropdown.
//
// We mock the server actions so the test stays a pure unit test —
// integration tests can exercise the cookie write path end-to-end.

import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

// Mock next/navigation — we just need router.refresh() to be a no-op spy.
const refreshSpy = vi.fn();
vi.mock('next/navigation', () => ({
  useRouter: () => ({ refresh: refreshSpy }),
}));

// Mock the server actions. They're imported by the component; vitest
// hoists the mock above the import so the component picks it up.
const setActiveOrgSpy = vi.fn().mockResolvedValue(undefined);
const clearActiveOrgSpy = vi.fn().mockResolvedValue(undefined);
vi.mock('@/app/actions/active-org', () => ({
  setActiveOrg: (orgId: string) => setActiveOrgSpy(orgId),
  clearActiveOrg: () => clearActiveOrgSpy(),
}));

import { OrgSwitcher } from '@/components/shared/org-switcher';

const ORGS = [
  { id: 'acme', name: 'Acme Corp' },
  { id: 'globex', name: 'Globex' },
  { id: 'umbrella', name: 'Umbrella' },
];

describe('OrgSwitcher', () => {
  beforeEach(() => {
    refreshSpy.mockClear();
    setActiveOrgSpy.mockClear();
    clearActiveOrgSpy.mockClear();
  });

  it('renders nothing when there is only one option', () => {
    const { container } = render(<OrgSwitcher options={[ORGS[0]]} activeOrgId={null} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders nothing when there are zero options', () => {
    const { container } = render(<OrgSwitcher options={[]} activeOrgId={null} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders the dropdown with all options + the Default sentinel', () => {
    render(<OrgSwitcher options={ORGS} activeOrgId={null} />);
    const select = screen.getByRole('combobox', { name: /switch active organization/i });
    expect(select).toBeInTheDocument();
    // 3 orgs + 1 "Default scope" sentinel
    expect(screen.getAllByRole('option')).toHaveLength(4);
    expect(screen.getByRole('option', { name: 'Default scope' })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: 'Acme Corp' })).toBeInTheDocument();
  });

  it('shows the active org as selected when activeOrgId is set', () => {
    render(<OrgSwitcher options={ORGS} activeOrgId="globex" />);
    const select = screen.getByRole('combobox') as HTMLSelectElement;
    expect(select.value).toBe('globex');
  });

  it('shows __clear__ as selected when activeOrgId is null', () => {
    render(<OrgSwitcher options={ORGS} activeOrgId={null} />);
    const select = screen.getByRole('combobox') as HTMLSelectElement;
    expect(select.value).toBe('__clear__');
  });

  it('renders an inline Clear button only when activeOrgId is set', () => {
    const { rerender } = render(<OrgSwitcher options={ORGS} activeOrgId={null} />);
    expect(screen.queryByRole('button', { name: /clear active org override/i })).not.toBeInTheDocument();

    rerender(<OrgSwitcher options={ORGS} activeOrgId="acme" />);
    expect(screen.getByRole('button', { name: /clear active org override/i })).toBeInTheDocument();
  });

  it('calls setActiveOrg when a non-default option is picked', async () => {
    render(<OrgSwitcher options={ORGS} activeOrgId={null} />);
    const select = screen.getByRole('combobox') as HTMLSelectElement;
    fireEvent.change(select, { target: { value: 'globex' } });
    // useTransition resolves microtasks; flush them
    await Promise.resolve();
    await Promise.resolve();
    expect(setActiveOrgSpy).toHaveBeenCalledTimes(1);
    expect(setActiveOrgSpy).toHaveBeenCalledWith('globex');
  });

  it('calls clearActiveOrg when Default scope is picked', async () => {
    render(<OrgSwitcher options={ORGS} activeOrgId="globex" />);
    const select = screen.getByRole('combobox') as HTMLSelectElement;
    fireEvent.change(select, { target: { value: '__clear__' } });
    await Promise.resolve();
    await Promise.resolve();
    expect(clearActiveOrgSpy).toHaveBeenCalledTimes(1);
    expect(setActiveOrgSpy).not.toHaveBeenCalled();
  });

  it('calls router.refresh() after a successful switch', async () => {
    render(<OrgSwitcher options={ORGS} activeOrgId={null} />);
    const select = screen.getByRole('combobox') as HTMLSelectElement;
    fireEvent.change(select, { target: { value: 'acme' } });
    await Promise.resolve();
    await Promise.resolve();
    expect(refreshSpy).toHaveBeenCalled();
  });

  it('exposes the org Badge label', () => {
    render(<OrgSwitcher options={ORGS} activeOrgId={null} />);
    expect(screen.getByText('org')).toBeInTheDocument();
  });
});

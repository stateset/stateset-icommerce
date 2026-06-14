/**
 * Tests for the MobileNav disclosure (the only navigation below the lg
 * breakpoint, so its open/close + a11y wiring is load-bearing).
 * @module tests/unit/components/mobile-nav
 */

import React from 'react';
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/react';

// next/link as a plain anchor.
vi.mock('next/link', () => ({
  __esModule: true,
  default: ({
    children,
    href,
    className,
    ...rest
  }: {
    children: React.ReactNode;
    href: string;
    className?: string;
  }) => React.createElement('a', { href, className, ...rest }, children),
}));

// next/navigation — controllable pathname.
const mockUsePathname = vi.fn().mockReturnValue('/');
vi.mock('next/navigation', () => ({
  usePathname: () => mockUsePathname(),
}));

// Heroicons as identifiable stub svgs. Enumerated explicitly (rather than a
// Proxy) because vitest validates named imports against the mock's static
// exports — this must cover every icon used by mobile-nav AND by the sidebar
// module it imports `navigation` from.
vi.mock('@heroicons/react/24/outline', () => {
  const makeIcon =
    (name: string) =>
    (props: React.SVGProps<SVGSVGElement>) =>
      React.createElement('svg', { ...props, 'data-testid': `icon-${name}` });
  return {
    // mobile-nav
    Bars3Icon: makeIcon('bars3'),
    XMarkIcon: makeIcon('xmark'),
    SparklesIcon: makeIcon('sparkles'),
    // sidebar navigation
    HomeIcon: makeIcon('home'),
    ChatBubbleLeftRightIcon: makeIcon('chat'),
    ShoppingCartIcon: makeIcon('shopping-cart'),
    TagIcon: makeIcon('tag'),
    ArchiveBoxIcon: makeIcon('archive-box'),
    ArrowUturnLeftIcon: makeIcon('arrow-uturn-left'),
    UsersIcon: makeIcon('users'),
    CreditCardIcon: makeIcon('credit-card'),
    ChartBarIcon: makeIcon('chart-bar'),
    ServerStackIcon: makeIcon('server-stack'),
    ShieldCheckIcon: makeIcon('shield-check'),
    Cog6ToothIcon: makeIcon('cog'),
  };
});

// cn as passthrough so the open/hidden class toggle is assertable.
vi.mock('@/lib/utils', () => ({
  cn: (...args: unknown[]) => args.filter(Boolean).join(' '),
}));

import { MobileNav } from '@/components/mobile-nav';

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  mockUsePathname.mockReturnValue('/');
});

describe('MobileNav', () => {
  it('renders the brand and a collapsed menu by default', () => {
    render(<MobileNav />);
    expect(screen.getByText('StateSet')).toBeDefined();
    const toggle = screen.getByRole('button', { name: 'Open navigation menu' });
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    // Panel is present but hidden until opened.
    const nav = screen.getByRole('navigation', { name: 'Main navigation' });
    expect(nav.className).toContain('hidden');
    expect(nav.className).not.toContain('block');
  });

  it('opens and closes when the toggle is activated', () => {
    render(<MobileNav />);
    const toggle = screen.getByRole('button', { name: 'Open navigation menu' });
    fireEvent.click(toggle);

    const opened = screen.getByRole('button', { name: 'Close navigation menu' });
    expect(opened.getAttribute('aria-expanded')).toBe('true');
    expect(screen.getByRole('navigation', { name: 'Main navigation' }).className).toContain(
      'block'
    );

    fireEvent.click(opened);
    expect(
      screen.getByRole('button', { name: 'Open navigation menu' }).getAttribute('aria-expanded')
    ).toBe('false');
  });

  it('wires aria-controls on the toggle to the panel id', () => {
    render(<MobileNav />);
    const toggle = screen.getByRole('button', { name: 'Open navigation menu' });
    const nav = screen.getByRole('navigation', { name: 'Main navigation' });
    expect(toggle.getAttribute('aria-controls')).toBe(nav.getAttribute('id'));
  });

  it('closes the open menu on Escape', () => {
    render(<MobileNav />);
    fireEvent.click(screen.getByRole('button', { name: 'Open navigation menu' }));
    expect(
      screen.getByRole('button', { name: 'Close navigation menu' })
    ).toBeDefined();

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(
      screen.getByRole('button', { name: 'Open navigation menu' }).getAttribute('aria-expanded')
    ).toBe('false');
  });

  it('marks the active route with aria-current=page', () => {
    mockUsePathname.mockReturnValue('/orders');
    render(<MobileNav />);
    const ordersLink = screen.getByText('Orders').closest('a');
    expect(ordersLink?.getAttribute('aria-current')).toBe('page');
    const dashboardLink = screen.getByText('Dashboard').closest('a');
    expect(dashboardLink?.getAttribute('aria-current')).toBeNull();
  });

  it('treats sub-paths of a nav item as active', () => {
    mockUsePathname.mockReturnValue('/orders/123');
    render(<MobileNav />);
    expect(screen.getByText('Orders').closest('a')?.getAttribute('aria-current')).toBe('page');
  });
});

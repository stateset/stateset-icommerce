/**
 * Tests for Sidebar component
 * @module tests/unit/components/sidebar
 */

import React from 'react';
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { APP_VERSION } from '@/lib/version';

// Mock next/link as a simple anchor
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

// Mock next/navigation
const mockUsePathname = vi.fn().mockReturnValue('/');
vi.mock('next/navigation', () => ({
  usePathname: () => mockUsePathname(),
}));

// Mock heroicons
vi.mock('@heroicons/react/24/outline', () => {
  const makeIcon =
    (name: string) =>
    (props: React.SVGProps<SVGSVGElement>) =>
      React.createElement('svg', { ...props, 'data-testid': `icon-${name}` });

  return {
    HomeIcon: makeIcon('home'),
    ShoppingCartIcon: makeIcon('shopping-cart'),
    ArrowUturnLeftIcon: makeIcon('arrow-uturn-left'),
    UsersIcon: makeIcon('users'),
    ChartBarIcon: makeIcon('chart-bar'),
    Cog6ToothIcon: makeIcon('cog'),
    CreditCardIcon: makeIcon('credit-card'),
    SparklesIcon: makeIcon('sparkles'),
    ChatBubbleLeftRightIcon: makeIcon('chat'),
    TagIcon: makeIcon('tag'),
    ArchiveBoxIcon: makeIcon('archive-box'),
    ServerStackIcon: makeIcon('server-stack'),
  };
});

// Mock cn as passthrough
vi.mock('@/lib/utils', () => ({
  cn: (...args: unknown[]) => args.filter(Boolean).join(' '),
}));

import { Sidebar } from '@/components/sidebar';

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  mockUsePathname.mockReturnValue('/');
});

describe('Sidebar', () => {
  const navItems = [
    'Dashboard',
    'AI Chat',
    'Orders',
    'Products',
    'Inventory',
    'Returns',
    'Customers',
    'Subscriptions',
    'Analytics',
    'Gateway',
    'Settings',
  ];

  it('renders all 11 navigation items', () => {
    render(React.createElement(Sidebar));
    for (const item of navItems) {
      expect(screen.getByText(item)).toBeDefined();
    }
  });

  it('renders the StateSet brand text', () => {
    render(React.createElement(Sidebar));
    expect(screen.getByText('StateSet')).toBeDefined();
  });

  it('renders the Embedded Engine Active status badge', () => {
    render(React.createElement(Sidebar));
    expect(screen.getByText('Embedded Engine Active')).toBeDefined();
  });

  it('renders the SQLite latency info', () => {
    render(React.createElement(Sidebar));
    expect(screen.getByText('SQLite: 0ms latency')).toBeDefined();
  });

  it('renders the footer version string', () => {
    render(React.createElement(Sidebar));
    expect(screen.getByText(`StateSet iCommerce v${APP_VERSION}`)).toBeDefined();
  });

  it('renders Embedded Commerce Engine footer text', () => {
    render(React.createElement(Sidebar));
    expect(screen.getByText('Embedded Commerce Engine')).toBeDefined();
  });

  it('highlights Dashboard link when pathname is /', () => {
    mockUsePathname.mockReturnValue('/');
    render(React.createElement(Sidebar));
    const dashboardLink = screen.getByText('Dashboard').closest('a');
    expect(dashboardLink?.className).toContain('bg-indigo-50');
  });

  it('highlights Orders link when pathname is /orders', () => {
    mockUsePathname.mockReturnValue('/orders');
    render(React.createElement(Sidebar));
    const ordersLink = screen.getByText('Orders').closest('a');
    expect(ordersLink?.className).toContain('bg-indigo-50');
    const dashboardLink = screen.getByText('Dashboard').closest('a');
    expect(dashboardLink?.className).not.toContain('bg-indigo-50');
  });

  it('highlights a nav item for sub-paths (e.g. /orders/123)', () => {
    mockUsePathname.mockReturnValue('/orders/123');
    render(React.createElement(Sidebar));
    const ordersLink = screen.getByText('Orders').closest('a');
    expect(ordersLink?.className).toContain('bg-indigo-50');
  });

  it('renders correct href for each navigation item', () => {
    render(React.createElement(Sidebar));
    const expectedHrefs: Record<string, string> = {
      Dashboard: '/',
      'AI Chat': '/chat',
      Orders: '/orders',
      Products: '/products',
      Inventory: '/inventory',
      Returns: '/returns',
      Customers: '/customers',
      Subscriptions: '/subscriptions',
      Analytics: '/analytics',
      Gateway: '/gateway',
      Settings: '/settings',
    };
    for (const [name, href] of Object.entries(expectedHrefs)) {
      const link = screen.getByText(name).closest('a');
      expect(link?.getAttribute('href')).toBe(href);
    }
  });

  it('renders an icon for each navigation item plus logo', () => {
    render(React.createElement(Sidebar));
    const svgs = screen.getAllByTestId(/^icon-/);
    // 11 nav items + 1 logo sparkles icon = 12
    expect(svgs.length).toBe(12);
  });
});

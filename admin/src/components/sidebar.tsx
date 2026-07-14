'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import {
  HomeIcon,
  ShoppingCartIcon,
  ArrowUturnLeftIcon,
  UsersIcon,
  ChartBarIcon,
  Cog6ToothIcon,
  CreditCardIcon,
  SparklesIcon,
  ChatBubbleLeftRightIcon,
  TagIcon,
  ArchiveBoxIcon,
  ServerStackIcon,
  ShieldCheckIcon,
} from '@heroicons/react/24/outline';
import { DashboardSidebarSection } from '@stateset/design';
import { cn } from '@/lib/utils';
import { APP_VERSION } from '@/lib/version';

// Shared with the mobile disclosure nav (components/mobile-nav.tsx) so the
// two menus can never drift apart.
export const navigation = [
  { name: 'Dashboard', href: '/', icon: HomeIcon },
  { name: 'AI Chat', href: '/chat', icon: ChatBubbleLeftRightIcon },
  { name: 'Orders', href: '/orders', icon: ShoppingCartIcon },
  { name: 'Products', href: '/products', icon: TagIcon },
  { name: 'Inventory', href: '/inventory', icon: ArchiveBoxIcon },
  { name: 'Returns', href: '/returns', icon: ArrowUturnLeftIcon },
  { name: 'Customers', href: '/customers', icon: UsersIcon },
  { name: 'Subscriptions', href: '/subscriptions', icon: CreditCardIcon },
  { name: 'Analytics', href: '/analytics', icon: ChartBarIcon },
  { name: 'Gateway', href: '/gateway', icon: ServerStackIcon },
  { name: 'Build info', href: '/build-info', icon: ShieldCheckIcon },
  { name: 'Settings', href: '/settings', icon: Cog6ToothIcon },
];

// Mirror of @stateset/design's SidebarNavItem styling, applied to a Next <Link>
// so we keep the brand's navigation treatment AND client-side routing/prefetch.
export function navItemClass(active: boolean): string {
  return cn(
    'group flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-left text-sm font-medium transition-all duration-150',
    active
      ? 'bg-ds-sidebar-accent text-ds-sidebar-accent-foreground shadow-ds-card'
      : 'text-ds-sidebar-foreground/[0.78] hover:bg-ds-sidebar-foreground/[0.08] hover:text-ds-sidebar-foreground',
  );
}

export function isNavActive(pathname: string, href: string): boolean {
  return pathname === href || (href !== '/' && pathname.startsWith(href));
}

export function Sidebar() {
  const pathname = usePathname();

  return (
    <div className="hidden lg:flex lg:flex-shrink-0">
      <div className="flex w-ds-sidebar flex-col">
        <div className="flex min-h-0 flex-1 flex-col border-r border-ds-sidebar-border bg-ds-sidebar text-ds-sidebar-foreground">
          <div className="flex flex-1 flex-col overflow-y-auto pb-4 pt-5">
            {/* Logo */}
            <div className="flex flex-shrink-0 items-center px-5">
              <div className="flex items-center gap-2">
                <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-ds-primary">
                  <SparklesIcon className="h-5 w-5 text-ds-primary-foreground" />
                </div>
                <span className="font-ds-display text-xl font-semibold tracking-ds-tight text-ds-sidebar-foreground">
                  StateSet
                </span>
              </div>
            </div>

            {/* Engine status */}
            <div className="mx-4 mt-5 rounded-lg border border-ds-status-ok/25 bg-ds-status-ok/10 p-3">
              <div className="flex items-center gap-2">
                <span className="h-2 w-2 animate-ds-soft-pulse rounded-full bg-ds-status-ok" />
                <span className="text-xs font-semibold uppercase tracking-ds-kicker text-ds-status-ok">
                  Embedded Engine Active
                </span>
              </div>
              <p className="mt-1 text-xs text-ds-sidebar-foreground/60">SQLite · 0ms latency</p>
            </div>

            {/* Navigation */}
            <DashboardSidebarSection className="mt-3 flex-1">
              <nav aria-label="Main navigation" className="space-y-1">
                {navigation.map((item) => {
                  const active = isNavActive(pathname, item.href);
                  return (
                    <Link
                      key={item.name}
                      href={item.href}
                      aria-current={active ? 'page' : undefined}
                      className={navItemClass(active)}
                    >
                      <item.icon className="h-4 w-4 flex-shrink-0" aria-hidden="true" />
                      <span className="truncate">{item.name}</span>
                    </Link>
                  );
                })}
              </nav>
            </DashboardSidebarSection>
          </div>

          {/* Footer */}
          <div className="flex flex-shrink-0 border-t border-ds-sidebar-border p-4">
            <div className="w-full">
              <p className="text-xs text-ds-sidebar-foreground/70">StateSet iCommerce v{APP_VERSION}</p>
              <p className="text-xs text-ds-sidebar-foreground/50">Embedded Commerce Engine</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

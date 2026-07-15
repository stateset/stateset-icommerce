'use client';

// Accessible mobile navigation: the desktop sidebar is `hidden lg:flex`,
// so below the lg breakpoint this disclosure header is the only way to
// move between pages. Plain disclosure pattern (button with aria-expanded
// + aria-controls toggling an always-present panel); closes on route
// change and Escape.

import { useEffect, useId, useState } from 'react';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { Bars3Icon, SparklesIcon, XMarkIcon } from '@heroicons/react/24/outline';
import { cn } from '@/lib/utils';
import { navigation, navItemClass, isNavActive } from '@/components/sidebar';

export function MobileNav() {
  const pathname = usePathname();
  const [open, setOpen] = useState(false);
  const panelId = useId();

  // Close the menu after navigating.
  useEffect(() => {
    setOpen(false);
  }, [pathname]);

  // Close on Escape while open.
  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false);
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [open]);

  return (
    <div className="border-b border-ds-sidebar-border bg-ds-sidebar text-ds-sidebar-foreground lg:hidden">
      <div className="flex items-center justify-between px-4 py-3">
        <Link href="/" className="flex items-center gap-2">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-ds-primary">
            <SparklesIcon className="h-5 w-5 text-ds-primary-foreground" aria-hidden="true" />
          </div>
          <span className="font-ds-display text-xl font-semibold tracking-ds-tight text-ds-sidebar-foreground">
            StateSet
          </span>
        </Link>
        <button
          type="button"
          onClick={() => setOpen((value) => !value)}
          aria-expanded={open}
          aria-controls={panelId}
          aria-label={open ? 'Close navigation menu' : 'Open navigation menu'}
          className="ds-focus-ring inline-flex h-9 w-9 items-center justify-center rounded-lg text-ds-sidebar-foreground/[0.78] hover:bg-ds-sidebar-foreground/[0.08] hover:text-ds-sidebar-foreground"
        >
          {open ? (
            <XMarkIcon className="h-6 w-6" aria-hidden="true" />
          ) : (
            <Bars3Icon className="h-6 w-6" aria-hidden="true" />
          )}
        </button>
      </div>
      <nav
        id={panelId}
        aria-label="Main navigation"
        className={cn('space-y-1 px-2 pb-3', open ? 'block' : 'hidden')}
      >
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
    </div>
  );
}

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
import { navigation } from '@/components/sidebar';

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
    <div className="lg:hidden border-b border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900">
      <div className="flex items-center justify-between px-4 py-3">
        <Link href="/" className="flex items-center space-x-2">
          <div className="w-8 h-8 bg-indigo-600 rounded-lg flex items-center justify-center">
            <SparklesIcon className="w-5 h-5 text-white" aria-hidden="true" />
          </div>
          <span className="text-xl font-bold text-gray-900 dark:text-white">StateSet</span>
        </Link>
        <button
          type="button"
          onClick={() => setOpen((value) => !value)}
          aria-expanded={open}
          aria-controls={panelId}
          aria-label={open ? 'Close navigation menu' : 'Open navigation menu'}
          className="inline-flex h-9 w-9 items-center justify-center rounded-md text-gray-600 hover:bg-gray-100 hover:text-gray-900 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring dark:text-gray-400 dark:hover:bg-gray-800 dark:hover:text-gray-50"
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
          const isActive =
            pathname === item.href || (item.href !== '/' && pathname.startsWith(item.href));

          return (
            <Link
              key={item.name}
              href={item.href}
              aria-current={isActive ? 'page' : undefined}
              className={cn(
                isActive
                  ? 'bg-indigo-50 dark:bg-indigo-900/30 text-indigo-600 dark:text-indigo-400'
                  : 'text-gray-600 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-800 hover:text-gray-900 dark:hover:text-white',
                'group flex items-center px-3 py-2 text-sm font-medium rounded-md transition-colors'
              )}
            >
              <item.icon
                className={cn(
                  isActive
                    ? 'text-indigo-600 dark:text-indigo-400'
                    : 'text-gray-400 dark:text-gray-500 group-hover:text-gray-500 dark:group-hover:text-gray-400',
                  'mr-3 h-5 w-5 flex-shrink-0'
                )}
                aria-hidden="true"
              />
              {item.name}
            </Link>
          );
        })}
      </nav>
    </div>
  );
}

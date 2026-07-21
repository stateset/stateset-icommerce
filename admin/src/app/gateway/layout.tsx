'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { cn } from '@/lib/utils';

const tabs = [
  { name: 'Overview', href: '/gateway' },
  { name: 'Sessions', href: '/gateway/sessions' },
  { name: 'Logs', href: '/gateway/logs' },
  { name: 'Metrics', href: '/gateway/metrics' },
];

export default function GatewayLayout({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();

  return (
    <div>
      <nav className="flex space-x-1 border-b border-gray-200 dark:border-gray-800 mb-6">
        {tabs.map((tab) => {
          const isActive =
            tab.href === '/gateway' ? pathname === '/gateway' : pathname.startsWith(tab.href);

          return (
            <Link
              key={tab.name}
              href={tab.href}
              className={cn(
                'px-4 py-2 text-sm font-medium border-b-2 -mb-px transition-colors',
                isActive
                  ? 'border-indigo-500 text-indigo-600 dark:text-indigo-400'
                  : 'border-transparent text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-300',
              )}
            >
              {tab.name}
            </Link>
          );
        })}
      </nav>
      {children}
    </div>
  );
}

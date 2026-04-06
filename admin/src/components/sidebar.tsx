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
} from '@heroicons/react/24/outline';
import { cn } from '@/lib/utils';
import { APP_VERSION } from '@/lib/version';

const navigation = [
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
  { name: 'Settings', href: '/settings', icon: Cog6ToothIcon },
];

export function Sidebar() {
  const pathname = usePathname();

  return (
    <div className="hidden lg:flex lg:flex-shrink-0">
      <div className="flex w-64 flex-col">
        <div className="flex min-h-0 flex-1 flex-col border-r border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900">
          <div className="flex flex-1 flex-col overflow-y-auto pt-5 pb-4">
            {/* Logo */}
            <div className="flex flex-shrink-0 items-center px-4">
              <div className="flex items-center space-x-2">
                <div className="w-8 h-8 bg-indigo-600 rounded-lg flex items-center justify-center">
                  <SparklesIcon className="w-5 h-5 text-white" />
                </div>
                <span className="text-xl font-bold text-gray-900 dark:text-white">
                  StateSet
                </span>
              </div>
            </div>

            {/* Engine Status */}
            <div className="mx-4 mt-4 p-3 bg-emerald-50 dark:bg-emerald-900/20 rounded-lg">
              <div className="flex items-center space-x-2">
                <div className="w-2 h-2 bg-emerald-500 rounded-full animate-pulse" />
                <span className="text-xs font-medium text-emerald-700 dark:text-emerald-400">
                  Embedded Engine Active
                </span>
              </div>
              <p className="text-xs text-emerald-600 dark:text-emerald-500 mt-1">
                SQLite: 0ms latency
              </p>
            </div>

            {/* Navigation */}
            <nav className="mt-5 flex-1 space-y-1 px-2">
              {navigation.map((item) => {
                const isActive = pathname === item.href ||
                  (item.href !== '/' && pathname.startsWith(item.href));

                return (
                  <Link
                    key={item.name}
                    href={item.href}
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

          {/* Footer */}
          <div className="flex flex-shrink-0 border-t border-gray-200 dark:border-gray-800 p-4">
            <div className="w-full">
              <p className="text-xs text-gray-500 dark:text-gray-400">
                StateSet iCommerce v{APP_VERSION}
              </p>
              <p className="text-xs text-gray-400 dark:text-gray-500">
                Embedded Commerce Engine
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

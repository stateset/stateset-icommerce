'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { cn } from '@/lib/utils';

export const operationsLinks = [
  { name: 'Purchasing', href: '/operations/purchasing' },
  { name: 'Warehouse', href: '/operations/warehouse' },
  { name: 'Manufacturing', href: '/operations/manufacturing' },
  { name: 'EDI', href: '/operations/edi' },
];

export default function OperationsNav() {
  const pathname = usePathname();

  return (
    <nav aria-label="Operations sections" className="flex gap-1 border-b border-ds-border pb-2">
      {operationsLinks.map((link) => {
        const active = pathname === link.href || pathname.startsWith(`${link.href}/`);
        return (
          <Link
            key={link.href}
            href={link.href}
            aria-current={active ? 'page' : undefined}
            className={cn(
              'rounded-md px-3 py-1.5 text-sm font-medium transition-colors',
              active
                ? 'bg-ds-primary/10 text-ds-primary'
                : 'text-ds-muted-foreground hover:bg-ds-muted hover:text-ds-foreground'
            )}
          >
            {link.name}
          </Link>
        );
      })}
    </nav>
  );
}

import React from 'react';

import { cn } from '../utils/cn.js';

export function SidebarNavItem({
  label,
  icon: Icon,
  active = false,
  href = '',
  badge = '',
  className = '',
  ...props
}) {
  const Component = href ? 'a' : 'button';

  return (
    <Component
      href={href || undefined}
      className={cn(
        'group flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-left text-sm font-medium transition-all duration-150',
        active
          ? 'bg-ds-sidebar-accent text-ds-sidebar-accent-foreground shadow-ds-card'
          : 'text-ds-sidebar-foreground/78 hover:bg-ds-sidebar-foreground/8 hover:text-ds-sidebar-foreground',
        className,
      )}
      {...props}>
      {Icon ? <Icon className="h-4 w-4 flex-shrink-0" /> : null}
      <span className="truncate">{label}</span>
      {badge ? (
        <span
          className={cn(
            'ml-auto rounded-full px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.16em]',
            active
              ? 'bg-black/10 text-ds-sidebar-accent-foreground'
              : 'bg-ds-sidebar-foreground/8 text-ds-sidebar-foreground/70',
          )}>
          {badge}
        </span>
      ) : null}
    </Component>
  );
}

export default SidebarNavItem;

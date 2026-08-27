'use client';

import React from 'react';
import * as DropdownMenuPrimitive from '@radix-ui/react-dropdown-menu';

import { cn } from '../utils/cn.js';

export const DropdownMenu = DropdownMenuPrimitive.Root;
export const DropdownMenuTrigger = DropdownMenuPrimitive.Trigger;

export const DropdownMenuContent = React.forwardRef(function DropdownMenuContent(
  { className = '', sideOffset = 6, ...props },
  ref,
) {
  return (
    <DropdownMenuPrimitive.Portal>
      <DropdownMenuPrimitive.Content
        ref={ref}
        sideOffset={sideOffset}
        className={cn(
          'z-50 min-w-[10rem] rounded-xl border border-ds-border bg-ds-popover p-1 text-ds-popover-foreground shadow-ds-panel focus:outline-none',
          className,
        )}
        {...props}
      />
    </DropdownMenuPrimitive.Portal>
  );
});

export const DropdownMenuItem = React.forwardRef(function DropdownMenuItem(
  { className = '', ...props },
  ref,
) {
  return (
    <DropdownMenuPrimitive.Item
      ref={ref}
      className={cn(
        'flex cursor-default select-none items-center rounded-lg px-2.5 py-2 text-sm text-ds-foreground outline-none focus:bg-ds-muted data-[highlighted]:bg-ds-muted data-[disabled]:pointer-events-none data-[disabled]:opacity-50',
        className,
      )}
      {...props}
    />
  );
});

export const DropdownMenuLabel = React.forwardRef(function DropdownMenuLabel(
  { className = '', ...props },
  ref,
) {
  return (
    <DropdownMenuPrimitive.Label
      ref={ref}
      className={cn(
        'px-2.5 py-1.5 text-[11px] font-semibold uppercase tracking-[0.16em] text-ds-muted-foreground',
        className,
      )}
      {...props}
    />
  );
});

export const DropdownMenuSeparator = React.forwardRef(function DropdownMenuSeparator(
  { className = '', ...props },
  ref,
) {
  return (
    <DropdownMenuPrimitive.Separator
      ref={ref}
      className={cn('-mx-1 my-1 h-px bg-ds-border', className)}
      {...props}
    />
  );
});

export default DropdownMenu;

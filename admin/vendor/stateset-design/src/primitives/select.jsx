'use client';

import React from 'react';
import * as SelectPrimitive from '@radix-ui/react-select';
import { Check, ChevronDown } from 'lucide-react';

import { cn } from '../utils/cn.js';

export const Select = SelectPrimitive.Root;
export const SelectGroup = SelectPrimitive.Group;
export const SelectValue = SelectPrimitive.Value;

export const SelectTrigger = React.forwardRef(function SelectTrigger(
  { className = '', children, ...props },
  ref,
) {
  return (
    <SelectPrimitive.Trigger
      ref={ref}
      className={cn(
        'ds-focus-ring flex h-10 w-full items-center justify-between rounded-lg border border-ds-input bg-ds-card px-3 text-sm text-ds-foreground transition-all duration-150 disabled:cursor-not-allowed disabled:opacity-50 data-[placeholder]:text-ds-muted-foreground',
        className,
      )}
      {...props}>
      {children}
      <SelectPrimitive.Icon asChild>
        <ChevronDown className="h-4 w-4 text-ds-muted-foreground" />
      </SelectPrimitive.Icon>
    </SelectPrimitive.Trigger>
  );
});

export const SelectContent = React.forwardRef(function SelectContent(
  { className = '', children, position = 'popper', ...props },
  ref,
) {
  return (
    <SelectPrimitive.Portal>
      <SelectPrimitive.Content
        ref={ref}
        position={position}
        className={cn(
          'z-50 min-w-[8rem] overflow-hidden rounded-xl border border-ds-border bg-ds-popover p-1 text-ds-popover-foreground shadow-ds-panel',
          position === 'popper' && 'w-[var(--radix-select-trigger-width)]',
          className,
        )}
        {...props}>
        <SelectPrimitive.Viewport className="p-1">{children}</SelectPrimitive.Viewport>
      </SelectPrimitive.Content>
    </SelectPrimitive.Portal>
  );
});

export const SelectLabel = React.forwardRef(function SelectLabel(
  { className = '', ...props },
  ref,
) {
  return (
    <SelectPrimitive.Label
      ref={ref}
      className={cn(
        'px-2.5 py-1.5 text-[11px] font-semibold uppercase tracking-[0.16em] text-ds-muted-foreground',
        className,
      )}
      {...props}
    />
  );
});

export const SelectItem = React.forwardRef(function SelectItem(
  { className = '', children, ...props },
  ref,
) {
  return (
    <SelectPrimitive.Item
      ref={ref}
      className={cn(
        'relative flex cursor-default select-none items-center rounded-lg py-2 pl-8 pr-2.5 text-sm text-ds-foreground outline-none focus:bg-ds-muted data-[highlighted]:bg-ds-muted data-[disabled]:pointer-events-none data-[disabled]:opacity-50',
        className,
      )}
      {...props}>
      <span className="absolute left-2 flex h-4 w-4 items-center justify-center">
        <SelectPrimitive.ItemIndicator>
          <Check className="h-4 w-4 text-ds-primary" />
        </SelectPrimitive.ItemIndicator>
      </span>
      <SelectPrimitive.ItemText>{children}</SelectPrimitive.ItemText>
    </SelectPrimitive.Item>
  );
});

export default Select;

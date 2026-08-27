'use client';

import React from 'react';
import * as DialogPrimitive from '@radix-ui/react-dialog';

import { cn } from '../utils/cn.js';

export const Dialog = DialogPrimitive.Root;
export const DialogTrigger = DialogPrimitive.Trigger;
export const DialogClose = DialogPrimitive.Close;

export const DialogContent = React.forwardRef(function DialogContent(
  { className = '', children, ...props },
  ref,
) {
  return (
    <DialogPrimitive.Portal>
      <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-ds-foreground/40 backdrop-blur-sm data-[state=open]:animate-in data-[state=closed]:animate-out" />
      <DialogPrimitive.Content
        ref={ref}
        className={cn(
          'fixed left-1/2 top-1/2 z-50 w-full max-w-lg -translate-x-1/2 -translate-y-1/2 rounded-2xl border border-ds-border bg-ds-popover p-6 text-ds-popover-foreground shadow-ds-panel focus:outline-none',
          className,
        )}
        {...props}>
        {children}
      </DialogPrimitive.Content>
    </DialogPrimitive.Portal>
  );
});

export function DialogHeader({ className = '', ...props }) {
  return <div className={cn('flex flex-col gap-1.5', className)} {...props} />;
}

export function DialogFooter({ className = '', ...props }) {
  return (
    <div
      className={cn('mt-6 flex items-center justify-end gap-3', className)}
      {...props}
    />
  );
}

export const DialogTitle = React.forwardRef(function DialogTitle(
  { className = '', ...props },
  ref,
) {
  return (
    <DialogPrimitive.Title
      ref={ref}
      className={cn(
        'font-ds-display text-lg font-semibold leading-6 text-ds-foreground',
        className,
      )}
      {...props}
    />
  );
});

export const DialogDescription = React.forwardRef(function DialogDescription(
  { className = '', ...props },
  ref,
) {
  return (
    <DialogPrimitive.Description
      ref={ref}
      className={cn('text-sm leading-5 text-ds-muted-foreground', className)}
      {...props}
    />
  );
});

export default Dialog;

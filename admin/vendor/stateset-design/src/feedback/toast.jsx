'use client';

import React from 'react';
import * as ToastPrimitive from '@radix-ui/react-toast';

import { cn } from '../utils/cn.js';

export const ToastProvider = ToastPrimitive.Provider;

export const ToastViewport = React.forwardRef(function ToastViewport(
  { className = '', ...props },
  ref,
) {
  return (
    <ToastPrimitive.Viewport
      ref={ref}
      className={cn(
        'fixed bottom-0 right-0 z-50 flex max-h-screen w-full max-w-sm flex-col gap-3 p-4 outline-none',
        className,
      )}
      {...props}
    />
  );
});

export const Toast = React.forwardRef(function Toast(
  { className = '', ...props },
  ref,
) {
  return (
    <ToastPrimitive.Root
      ref={ref}
      className={cn(
        'flex w-full items-start gap-3 rounded-xl border border-ds-border bg-ds-popover p-4 text-ds-popover-foreground shadow-ds-panel',
        className,
      )}
      {...props}
    />
  );
});

export const ToastTitle = React.forwardRef(function ToastTitle(
  { className = '', ...props },
  ref,
) {
  return (
    <ToastPrimitive.Title
      ref={ref}
      className={cn('text-sm font-semibold text-ds-foreground', className)}
      {...props}
    />
  );
});

export const ToastDescription = React.forwardRef(function ToastDescription(
  { className = '', ...props },
  ref,
) {
  return (
    <ToastPrimitive.Description
      ref={ref}
      className={cn('text-sm text-ds-muted-foreground', className)}
      {...props}
    />
  );
});

export const ToastAction = React.forwardRef(function ToastAction(
  { className = '', ...props },
  ref,
) {
  return (
    <ToastPrimitive.Action
      ref={ref}
      className={cn(
        'ds-focus-ring inline-flex h-8 items-center rounded-lg px-3 text-sm font-medium text-ds-primary hover:bg-ds-muted',
        className,
      )}
      {...props}
    />
  );
});

export const ToastClose = React.forwardRef(function ToastClose(
  { className = '', ...props },
  ref,
) {
  return (
    <ToastPrimitive.Close
      ref={ref}
      className={cn(
        'ds-focus-ring ml-auto inline-flex h-7 w-7 items-center justify-center rounded-lg text-ds-muted-foreground transition-colors hover:bg-ds-muted hover:text-ds-foreground',
        className,
      )}
      {...props}
    />
  );
});

export default Toast;

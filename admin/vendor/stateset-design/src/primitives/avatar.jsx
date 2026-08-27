'use client';

import React from 'react';
import * as AvatarPrimitive from '@radix-ui/react-avatar';

import { cn } from '../utils/cn.js';

export const Avatar = React.forwardRef(function Avatar(
  { className = '', ...props },
  ref,
) {
  return (
    <AvatarPrimitive.Root
      ref={ref}
      className={cn(
        'relative flex h-10 w-10 shrink-0 overflow-hidden rounded-full bg-ds-muted',
        className,
      )}
      {...props}
    />
  );
});

export const AvatarImage = React.forwardRef(function AvatarImage(
  { className = '', ...props },
  ref,
) {
  return (
    <AvatarPrimitive.Image
      ref={ref}
      className={cn('aspect-square h-full w-full object-cover', className)}
      {...props}
    />
  );
});

export const AvatarFallback = React.forwardRef(function AvatarFallback(
  { className = '', ...props },
  ref,
) {
  return (
    <AvatarPrimitive.Fallback
      ref={ref}
      className={cn(
        'flex h-full w-full items-center justify-center text-sm font-medium text-ds-muted-foreground',
        className,
      )}
      {...props}
    />
  );
});

export default Avatar;

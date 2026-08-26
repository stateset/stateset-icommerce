'use client';

import React from 'react';
import * as TabsPrimitive from '@radix-ui/react-tabs';

import { cn } from '../utils/cn.js';

export const Tabs = TabsPrimitive.Root;

export const TabsList = React.forwardRef(function TabsList(
  { className = '', ...props },
  ref,
) {
  return (
    <TabsPrimitive.List
      ref={ref}
      className={cn('inline-flex gap-1 rounded-full bg-ds-muted p-1', className)}
      {...props}
    />
  );
});

export const TabsTrigger = React.forwardRef(function TabsTrigger(
  { className = '', ...props },
  ref,
) {
  return (
    <TabsPrimitive.Trigger
      ref={ref}
      className={cn(
        'ds-focus-ring rounded-full px-3.5 py-1.5 text-sm font-medium text-ds-muted-foreground transition-all duration-150 disabled:pointer-events-none disabled:opacity-50 data-[state=active]:bg-ds-card data-[state=active]:text-ds-foreground data-[state=active]:shadow-ds-card',
        className,
      )}
      {...props}
    />
  );
});

export const TabsContent = React.forwardRef(function TabsContent(
  { className = '', ...props },
  ref,
) {
  return (
    <TabsPrimitive.Content
      ref={ref}
      className={cn('mt-4 focus:outline-none', className)}
      {...props}
    />
  );
});

export default Tabs;

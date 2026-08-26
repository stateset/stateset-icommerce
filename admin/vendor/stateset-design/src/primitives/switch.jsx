'use client';

import React from 'react';
import * as SwitchPrimitive from '@radix-ui/react-switch';

import { cn } from '../utils/cn.js';

export const Switch = React.forwardRef(function Switch(
  { className = '', ...props },
  ref,
) {
  return (
    <SwitchPrimitive.Root
      ref={ref}
      className={cn(
        'ds-focus-ring inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full bg-ds-border transition-colors duration-150 disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:bg-ds-primary',
        className,
      )}
      {...props}>
      <SwitchPrimitive.Thumb className="pointer-events-none block h-5 w-5 translate-x-0.5 rounded-full bg-white shadow transition-transform duration-150 data-[state=checked]:translate-x-[1.375rem]" />
    </SwitchPrimitive.Root>
  );
});

export default Switch;

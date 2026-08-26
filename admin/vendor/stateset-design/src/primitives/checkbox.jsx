'use client';

import React from 'react';
import * as CheckboxPrimitive from '@radix-ui/react-checkbox';
import { Check } from 'lucide-react';

import { cn } from '../utils/cn.js';

export const Checkbox = React.forwardRef(function Checkbox(
  { className = '', ...props },
  ref,
) {
  return (
    <CheckboxPrimitive.Root
      ref={ref}
      className={cn(
        'ds-focus-ring flex h-5 w-5 shrink-0 items-center justify-center rounded-md border border-ds-input bg-ds-card transition-colors duration-150 disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:border-ds-primary data-[state=checked]:bg-ds-primary',
        className,
      )}
      {...props}>
      <CheckboxPrimitive.Indicator className="flex items-center justify-center text-white">
        <Check className="h-3.5 w-3.5" strokeWidth={3} />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
  );
});

export default Checkbox;

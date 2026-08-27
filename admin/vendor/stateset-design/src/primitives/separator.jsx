'use client';

import React from 'react';
import * as SeparatorPrimitive from '@radix-ui/react-separator';

import { cn } from '../utils/cn.js';

export const Separator = React.forwardRef(function Separator(
  { className = '', orientation = 'horizontal', decorative = true, ...props },
  ref,
) {
  return (
    <SeparatorPrimitive.Root
      ref={ref}
      orientation={orientation}
      decorative={decorative}
      className={cn(
        'shrink-0 bg-ds-border',
        orientation === 'vertical' ? 'h-full w-px' : 'h-px w-full',
        className,
      )}
      {...props}
    />
  );
});

export default Separator;

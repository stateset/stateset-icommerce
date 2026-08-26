import React from 'react';

import { cn } from '../utils/cn.js';

export function Skeleton({ className = '', ...props }) {
  return (
    <div
      className={cn('animate-pulse rounded-md bg-ds-muted', className)}
      {...props}
    />
  );
}

export default Skeleton;

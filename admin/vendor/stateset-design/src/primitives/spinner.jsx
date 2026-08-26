import React from 'react';

import { cn } from '../utils/cn.js';

const SIZE_STYLES = {
  sm: 'h-4 w-4',
  md: 'h-6 w-6',
  lg: 'h-8 w-8',
};

export const Spinner = React.forwardRef(function Spinner(
  { className = '', size = 'md', label = 'Loading', ...props },
  ref,
) {
  return (
    <svg
      ref={ref}
      role="status"
      aria-label={label}
      viewBox="0 0 24 24"
      fill="none"
      className={cn(
        'animate-spin text-ds-primary',
        SIZE_STYLES[size] || SIZE_STYLES.md,
        className,
      )}
      {...props}>
      <circle
        cx="12"
        cy="12"
        r="10"
        stroke="currentColor"
        strokeWidth="3"
        className="opacity-25"
      />
      <path
        d="M22 12a10 10 0 0 1-10 10"
        stroke="currentColor"
        strokeWidth="3"
        strokeLinecap="round"
      />
    </svg>
  );
});

export default Spinner;

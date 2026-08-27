import React from 'react';

import { cn } from '../utils/cn.js';

const VARIANT_STYLES = {
  default: 'border border-ds-border bg-ds-muted text-ds-muted-foreground',
  primary:
    'border border-ds-brand-200 bg-ds-brand-50 text-ds-brand-700 dark:border-ds-brand-700 dark:bg-ds-brand-950/30 dark:text-ds-brand-300',
  accent:
    'border border-ds-coral-200 bg-ds-coral-50 text-ds-coral-700 dark:border-ds-coral-700 dark:bg-ds-coral-950/30 dark:text-ds-coral-300',
  success: 'border border-ds-success/25 bg-ds-success/10 text-ds-success',
  warning: 'border border-ds-warning/25 bg-ds-warning/10 text-ds-warning',
  danger: 'border border-ds-destructive/25 bg-ds-destructive/10 text-ds-destructive',
  outline: 'border border-ds-border bg-transparent text-ds-foreground',
};

export function Badge({ children, className = '', variant = 'default', as = 'span', ...props }) {
  const Component = as;

  return (
    <Component
      className={cn(
        'inline-flex items-center rounded-full px-2.5 py-1 text-[11px] font-semibold uppercase tracking-[0.16em]',
        VARIANT_STYLES[variant] || VARIANT_STYLES.default,
        className,
      )}
      {...props}>
      {children}
    </Component>
  );
}

export default Badge;

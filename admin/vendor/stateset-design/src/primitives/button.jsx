import React from 'react';

import { cn } from '../utils/cn.js';

// Variants follow the authoritative Figma button spec: pill shape, electric-indigo
// fill, and an outline form in the same brand hue. See BRAND.md.
const VARIANT_STYLES = {
  primary:
    'bg-ds-primary text-ds-primary-foreground hover:bg-ds-brand-600 active:bg-ds-brand-600 disabled:bg-ds-brand-100 disabled:text-white',
  secondary:
    'border border-ds-primary bg-transparent text-ds-primary hover:bg-ds-brand-50 active:bg-ds-brand-50 disabled:border-ds-brand-200 disabled:text-ds-brand-200',
  ghost: 'text-ds-primary hover:bg-ds-brand-50',
  accent:
    'bg-ds-accent text-ds-accent-foreground hover:bg-ds-accent/90 disabled:bg-ds-brand-100 disabled:text-white',
  danger:
    'bg-ds-destructive text-ds-destructive-foreground hover:bg-ds-destructive/90 disabled:opacity-55',
};

// Normal (14px) and Big (20px) match the Figma sizes; sm/md/lg kept as aliases.
const SIZE_STYLES = {
  sm: 'h-9 px-4 text-sm',
  md: 'h-10 px-5 text-sm',
  normal: 'h-10 px-5 text-sm',
  lg: 'h-12 px-6 text-base',
  big: 'h-14 px-7 text-xl',
};

export function Button({
  children,
  className = '',
  variant = 'primary',
  size = 'md',
  href = '',
  type = 'button',
  ...props
}) {
  const classes = cn(
    'ds-focus-ring inline-flex items-center justify-center gap-2 rounded-full font-semibold transition-all duration-150 disabled:pointer-events-none',
    VARIANT_STYLES[variant] || VARIANT_STYLES.primary,
    SIZE_STYLES[size] || SIZE_STYLES.md,
    className,
  );

  if (href) {
    return (
      <a href={href} className={classes} {...props}>
        {children}
      </a>
    );
  }

  return (
    <button type={type} className={classes} {...props}>
      {children}
    </button>
  );
}

export default Button;

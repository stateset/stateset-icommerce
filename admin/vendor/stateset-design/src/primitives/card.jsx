import React from 'react';

import { cn } from '../utils/cn.js';

export function Card({ children, className = '', interactive = false, premium = false, ...props }) {
  return (
    <div
      className={cn(
        'ds-enterprise-panel rounded-lg text-ds-card-foreground',
        interactive &&
          'transition-all duration-200 hover:border-ds-enterprise-line-strong hover:bg-ds-enterprise-raised hover:shadow-ds-card-hover',
        premium && 'ds-premium-panel',
        className,
      )}
      {...props}>
      {children}
    </div>
  );
}

export function CardHeader({ children, className = '', ...props }) {
  return (
    <div
      className={cn(
        'flex flex-col gap-1.5 border-b border-ds-enterprise-line/70 px-5 py-4',
        className,
      )}
      {...props}>
      {children}
    </div>
  );
}

export function CardTitle({ children, className = '', as = 'h3', ...props }) {
  const Component = as;

  return (
    <Component
      className={cn(
        'font-ds-display text-base font-semibold leading-6 text-ds-foreground',
        className,
      )}
      {...props}>
      {children}
    </Component>
  );
}

export function CardDescription({ children, className = '', ...props }) {
  return (
    <p className={cn('text-sm leading-5 text-ds-muted-foreground', className)} {...props}>
      {children}
    </p>
  );
}

export function CardContent({ children, className = '', ...props }) {
  return (
    <div className={cn('p-5', className)} {...props}>
      {children}
    </div>
  );
}

export function CardFooter({ children, className = '', ...props }) {
  return (
    <div
      className={cn(
        'flex items-center gap-3 border-t border-ds-enterprise-line/70 px-5 py-4',
        className,
      )}
      {...props}>
      {children}
    </div>
  );
}

export default Card;

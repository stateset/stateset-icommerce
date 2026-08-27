import React from 'react';

import { cn } from '../utils/cn.js';
import { Button } from './button.jsx';

export function EmptyState({
  icon: Icon = null,
  eyebrow = '',
  title,
  description,
  action = null,
  secondaryAction = null,
  className = '',
}) {
  return (
    <section
      className={cn(
        'rounded-xl border border-ds-border/70 bg-ds-card px-6 py-10 text-center shadow-ds-card',
        className,
      )}>
      {Icon ? (
        <div className="mx-auto flex h-14 w-14 items-center justify-center rounded-2xl border border-ds-brand-200 bg-ds-brand-50 text-ds-brand-700 dark:border-ds-brand-700 dark:bg-ds-brand-950/30 dark:text-ds-brand-300">
          <Icon className="h-6 w-6" />
        </div>
      ) : null}
      {eyebrow ? (
        <p className="mt-5 text-[11px] font-semibold uppercase tracking-[0.18em] text-ds-accent">
          {eyebrow}
        </p>
      ) : null}
      <h3 className="mt-3 font-ds-display text-3xl tracking-tight text-ds-foreground">{title}</h3>
      <p className="mx-auto mt-3 max-w-xl text-sm leading-6 text-ds-muted-foreground">
        {description}
      </p>
      {action || secondaryAction ? (
        <div className="mt-6 flex flex-wrap items-center justify-center gap-3">
          {action ? <Button {...action}>{action.label}</Button> : null}
          {secondaryAction ? (
            <Button variant="secondary" {...secondaryAction}>
              {secondaryAction.label}
            </Button>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}

export default EmptyState;

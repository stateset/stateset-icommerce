import React from 'react';

import { cn } from '../utils/cn.js';

const TONE_STYLES = {
  primary: {
    ribbon: 'bg-ds-primary',
    icon: 'border-ds-brand-200 bg-ds-brand-50 text-ds-brand-700 dark:border-ds-brand-700 dark:bg-ds-brand-950/30 dark:text-ds-brand-300',
    trend: 'text-ds-primary',
  },
  accent: {
    ribbon: 'bg-ds-accent',
    icon: 'border-ds-coral-200 bg-ds-coral-50 text-ds-coral-700 dark:border-ds-coral-700 dark:bg-ds-coral-950/30 dark:text-ds-coral-300',
    trend: 'text-ds-accent',
  },
  success: {
    ribbon: 'bg-ds-success',
    icon: 'border-ds-success/25 bg-ds-success/10 text-ds-success',
    trend: 'text-ds-success',
  },
  warning: {
    ribbon: 'bg-ds-warning',
    icon: 'border-ds-warning/25 bg-ds-warning/10 text-ds-warning',
    trend: 'text-ds-warning',
  },
  danger: {
    ribbon: 'bg-ds-destructive',
    icon: 'border-ds-destructive/25 bg-ds-destructive/10 text-ds-destructive',
    trend: 'text-ds-destructive',
  },
};

function formatValue(value, format) {
  if (value === null || value === undefined || value === '') return '—';

  if (format === 'currency') {
    return new Intl.NumberFormat('en-US', {
      style: 'currency',
      currency: 'USD',
      minimumFractionDigits: 0,
      maximumFractionDigits: 0,
    }).format(Number(value));
  }

  if (format === 'number') {
    return new Intl.NumberFormat('en-US').format(Number(value));
  }

  return value;
}

export function MetricCard({
  label,
  value,
  subtitle = '',
  trend = '',
  icon: Icon = null,
  tone = 'primary',
  format = '',
  className = '',
}) {
  const styles = TONE_STYLES[tone] || TONE_STYLES.primary;

  return (
    <section
      className={cn(
        'ds-enterprise-panel group relative overflow-hidden rounded-lg p-5 text-ds-card-foreground transition-all duration-200 hover:border-ds-enterprise-line-strong hover:bg-ds-enterprise-raised hover:shadow-ds-card-hover',
        className,
      )}>
      <div className={cn('absolute inset-x-0 top-0 h-0.5 opacity-80', styles.ribbon)} />
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 space-y-2">
          <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-ds-muted-foreground">
            {label}
          </p>
          <p className="ds-instrument-number text-3xl text-ds-foreground">
            {formatValue(value, format)}
          </p>
          {subtitle ? <p className="text-sm text-ds-muted-foreground">{subtitle}</p> : null}
        </div>
        {Icon ? (
          <span
            className={cn(
              'inline-flex h-10 w-10 items-center justify-center rounded-2xl border',
              styles.icon,
            )}>
            <Icon className="h-5 w-5" />
          </span>
        ) : null}
      </div>
      {trend ? <p className={cn('mt-4 text-sm font-medium', styles.trend)}>{trend}</p> : null}
    </section>
  );
}

export default MetricCard;

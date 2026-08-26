import React from 'react';

import { cn } from '../utils/cn.js';

export function MetricStrip({ items = [], className = '' }) {
  return (
    <section
      className={cn(
        'grid overflow-hidden rounded-xl border border-ds-border/70 bg-ds-card shadow-ds-card md:grid-cols-2 xl:grid-cols-4',
        className,
      )}>
      {items.map((item, index) => (
        <div
          key={item.label || index}
          className={cn(
            'p-5',
            index !== items.length - 1 && 'border-b border-ds-border/70 xl:border-b-0 xl:border-r',
          )}>
          <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-ds-muted-foreground">
            {item.label}
          </p>
          <p className="ds-instrument-number mt-3 text-3xl text-ds-foreground">{item.value}</p>
          {item.detail ? (
            <p className="mt-2 text-sm text-ds-muted-foreground">{item.detail}</p>
          ) : null}
        </div>
      ))}
    </section>
  );
}

export default MetricStrip;

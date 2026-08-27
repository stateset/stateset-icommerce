import React from 'react';

import { cn } from '../utils/cn.js';

export function Table({ className = '', ...props }) {
  return (
    <div className="overflow-hidden rounded-xl border border-ds-border/70 bg-ds-card shadow-ds-card">
      <table className={cn('min-w-full divide-y divide-ds-border/70', className)} {...props} />
    </div>
  );
}

export function TableHeader({ className = '', ...props }) {
  return <thead className={cn('bg-ds-muted/70', className)} {...props} />;
}

export function TableBody({ className = '', ...props }) {
  return <tbody className={cn('divide-y divide-ds-border/60', className)} {...props} />;
}

export function TableRow({ className = '', ...props }) {
  return (
    <tr
      className={cn('transition-colors duration-150 hover:bg-ds-muted/40', className)}
      {...props}
    />
  );
}

export function TableHead({ className = '', ...props }) {
  return (
    <th
      className={cn(
        'px-4 py-3 text-left text-[11px] font-semibold uppercase tracking-[0.16em] text-ds-muted-foreground',
        className,
      )}
      {...props}
    />
  );
}

export function TableCell({ className = '', tone = 'default', ...props }) {
  const toneClass = tone === 'numeric' ? 'font-ds-mono tabular-nums' : 'font-medium';

  return (
    <td className={cn('px-4 py-3 text-sm text-ds-foreground', toneClass, className)} {...props} />
  );
}

import React from 'react';

import { cn } from '../utils/cn.js';

// The operational status vocabulary, carried over from the blueprint/console
// direction as portable data tokens. Each state is a tinted pill with an LED dot
// so it reads on any surface — calm marketing pages and dense ops tables alike.
const STATUS_STYLES = {
  ok: { dot: 'bg-ds-status-ok', text: 'text-ds-status-ok', ring: 'border-ds-status-ok/25 bg-ds-status-ok/10' },
  run: { dot: 'bg-ds-status-run', text: 'text-ds-status-run', ring: 'border-ds-status-run/25 bg-ds-status-run/10' },
  warn: { dot: 'bg-ds-status-warn', text: 'text-ds-status-warn', ring: 'border-ds-status-warn/25 bg-ds-status-warn/10' },
  fail: { dot: 'bg-ds-status-fail', text: 'text-ds-status-fail', ring: 'border-ds-status-fail/25 bg-ds-status-fail/10' },
  review: { dot: 'bg-ds-status-review', text: 'text-ds-status-review', ring: 'border-ds-status-review/25 bg-ds-status-review/10' },
  idle: { dot: 'bg-ds-status-idle', text: 'text-ds-status-idle', ring: 'border-ds-status-idle/25 bg-ds-status-idle/10' },
};

const DEFAULT_LABELS = {
  ok: 'Healthy',
  run: 'Running',
  warn: 'Attention',
  fail: 'Failed',
  review: 'Review',
  idle: 'Idle',
};

export function StatusPill({
  status = 'idle',
  children,
  pulse = false,
  className = '',
  ...props
}) {
  const tone = STATUS_STYLES[status] || STATUS_STYLES.idle;
  const label = children ?? DEFAULT_LABELS[status] ?? status;

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] font-semibold uppercase tracking-ds-kicker',
        tone.ring,
        tone.text,
        className,
      )}
      {...props}>
      <span
        className={cn(
          'h-1.5 w-1.5 rounded-full',
          tone.dot,
          pulse && 'animate-ds-soft-pulse',
        )}
      />
      {label}
    </span>
  );
}

export default StatusPill;

import { cn } from '@/lib/utils';

interface SimulatedDataBadgeProps {
  /** Visible badge text. */
  label?: string;
  /** Tooltip explaining why the data is simulated. */
  title?: string;
  className?: string;
}

/**
 * Honesty marker for widgets whose numbers are deterministic demo values
 * rather than live engine data (e.g. p95/p99 latency trends or payment
 * reconciliation deltas where no real telemetry/processor feed exists).
 *
 * Any chart or metric fed by a `deterministicRatio`-style generator in
 * `app/actions/commerce.ts` must carry this badge so simulated data is
 * never presented as real.
 */
export function SimulatedDataBadge({
  label = 'Simulated data',
  title = 'This widget shows deterministic demo values, not live engine data.',
  className,
}: SimulatedDataBadgeProps) {
  return (
    <span
      title={title}
      className={cn(
        'inline-flex items-center rounded-full border border-amber-300 bg-amber-50 px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide text-amber-700 dark:border-amber-700 dark:bg-amber-900/30 dark:text-amber-300',
        className,
      )}
    >
      {label}
    </span>
  );
}

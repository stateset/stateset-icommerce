import * as React from 'react';
import { cn } from '@/lib/utils';

interface ProgressBarProps {
  value: number;
  max?: number;
  color?: 'emerald' | 'amber' | 'red' | 'blue' | 'indigo' | 'purple' | 'gray';
  size?: 'sm' | 'md' | 'lg';
  showLabel?: boolean;
  className?: string;
}

const colorClasses: Record<string, string> = {
  emerald: 'bg-ds-status-ok',
  amber: 'bg-ds-status-warn',
  red: 'bg-ds-status-fail',
  blue: 'bg-ds-status-run',
  indigo: 'bg-ds-primary',
  purple: 'bg-ds-primary',
  gray: 'bg-ds-muted-foreground',
};

const sizeClasses: Record<string, string> = {
  sm: 'h-1.5',
  md: 'h-2',
  lg: 'h-3',
};

export function ProgressBar({
  value,
  max = 100,
  color = 'blue',
  size = 'md',
  showLabel = false,
  className,
}: ProgressBarProps) {
  const percentage = Math.min(100, Math.max(0, (value / max) * 100));

  return (
    <div className={cn('w-full', className)}>
      {showLabel && (
        <div className="flex justify-between text-xs text-ds-muted-foreground mb-1">
          <span>{percentage.toFixed(0)}%</span>
        </div>
      )}
      <div className={cn('w-full rounded-full bg-ds-muted', sizeClasses[size])}>
        <div
          className={cn(
            'rounded-full transition-all duration-300',
            sizeClasses[size],
            colorClasses[color],
          )}
          style={{ width: `${percentage}%` }}
        />
      </div>
    </div>
  );
}

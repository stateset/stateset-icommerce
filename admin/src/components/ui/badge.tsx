import * as React from 'react';
import { cva, type VariantProps } from 'class-variance-authority';
import { cn } from '@/lib/utils';

type DataAttributes = {
  [key in `data-${string}`]?: string | number | undefined;
};

const badgeVariants = cva(
  'inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-ds-ring focus:ring-offset-2',
  {
    variants: {
      variant: {
        default: 'border-transparent bg-ds-foreground text-ds-background hover:bg-ds-foreground/80',
        secondary: 'border-transparent bg-ds-muted text-ds-foreground hover:bg-ds-muted/80',
        destructive:
          'border-transparent bg-ds-destructive text-ds-destructive-foreground hover:bg-ds-destructive/80',
        outline: 'text-ds-foreground',
      },
      color: {
        gray: 'bg-ds-muted text-ds-muted-foreground',
        red: 'bg-ds-status-fail/10 text-ds-status-fail',
        orange: 'bg-ds-status-warn/10 text-ds-status-warn',
        amber: 'bg-ds-status-warn/10 text-ds-status-warn',
        yellow: 'bg-ds-status-warn/10 text-ds-status-warn',
        lime: 'bg-ds-status-ok/10 text-ds-status-ok',
        green: 'bg-ds-status-ok/10 text-ds-status-ok',
        emerald: 'bg-ds-status-ok/10 text-ds-status-ok',
        teal: 'bg-ds-status-ok/10 text-ds-status-ok',
        cyan: 'bg-ds-info/10 text-ds-info',
        sky: 'bg-ds-info/10 text-ds-info',
        blue: 'bg-ds-status-run/10 text-ds-status-run',
        indigo: 'bg-ds-brand-100 text-ds-brand-700',
        violet: 'bg-ds-brand-100 text-ds-brand-700',
        purple: 'bg-ds-brand-100 text-ds-brand-700',
        fuchsia: 'bg-ds-brand-100 text-ds-brand-700',
        pink: 'bg-ds-brand-100 text-ds-brand-700',
        rose: 'bg-ds-status-fail/10 text-ds-status-fail',
      },
      size: {
        xs: 'px-2 py-0.5 text-xs',
        sm: 'px-2.5 py-0.5 text-xs',
        md: 'px-3 py-1 text-sm',
        lg: 'px-4 py-1.5 text-sm',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'sm',
    },
  },
);

export type BadgeProps = Omit<React.HTMLAttributes<HTMLDivElement>, 'color'> &
  VariantProps<typeof badgeVariants> &
  DataAttributes & {
    icon?: React.ComponentType<{ className?: string }>;
  };

function Badge({ className, variant, color, size, icon: Icon, children, ...props }: BadgeProps) {
  return (
    <div className={cn(badgeVariants({ variant, color, size }), className)} {...props}>
      {Icon && <Icon className="mr-1 h-3 w-3" />}
      {children}
    </div>
  );
}

export { Badge, badgeVariants };

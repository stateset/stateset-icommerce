import * as React from 'react';
import { cva, type VariantProps } from 'class-variance-authority';
import { cn } from '@/lib/utils';

type DataAttributes = {
  [key in `data-${string}`]?: string | number | undefined;
};

const badgeVariants = cva(
  'inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2',
  {
    variants: {
      variant: {
        default:
          'border-transparent bg-gray-900 text-gray-50 hover:bg-gray-900/80 dark:bg-gray-50 dark:text-gray-900',
        secondary:
          'border-transparent bg-gray-100 text-gray-900 hover:bg-gray-100/80 dark:bg-gray-800 dark:text-gray-50',
        destructive:
          'border-transparent bg-red-500 text-gray-50 hover:bg-red-500/80',
        outline: 'text-gray-950 dark:text-gray-50',
      },
      color: {
        gray: 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-200',
        red: 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400',
        orange: 'bg-orange-100 text-orange-800 dark:bg-orange-900/30 dark:text-orange-400',
        amber: 'bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-400',
        yellow: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400',
        lime: 'bg-lime-100 text-lime-800 dark:bg-lime-900/30 dark:text-lime-400',
        green: 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400',
        emerald: 'bg-emerald-100 text-emerald-800 dark:bg-emerald-900/30 dark:text-emerald-400',
        teal: 'bg-teal-100 text-teal-800 dark:bg-teal-900/30 dark:text-teal-400',
        cyan: 'bg-cyan-100 text-cyan-800 dark:bg-cyan-900/30 dark:text-cyan-400',
        sky: 'bg-sky-100 text-sky-800 dark:bg-sky-900/30 dark:text-sky-400',
        blue: 'bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400',
        indigo: 'bg-indigo-100 text-indigo-800 dark:bg-indigo-900/30 dark:text-indigo-400',
        violet: 'bg-violet-100 text-violet-800 dark:bg-violet-900/30 dark:text-violet-400',
        purple: 'bg-purple-100 text-purple-800 dark:bg-purple-900/30 dark:text-purple-400',
        fuchsia: 'bg-fuchsia-100 text-fuchsia-800 dark:bg-fuchsia-900/30 dark:text-fuchsia-400',
        pink: 'bg-pink-100 text-pink-800 dark:bg-pink-900/30 dark:text-pink-400',
        rose: 'bg-rose-100 text-rose-800 dark:bg-rose-900/30 dark:text-rose-400',
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
  }
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

import * as React from 'react';
import { cn } from '@/lib/utils';

type DataAttributes = {
  [key in `data-${string}`]?: string | number | undefined;
};

type Decoration = 'top' | 'left' | 'bottom' | 'right';
type DecorationColor = 'gray' | 'red' | 'amber' | 'blue' | 'indigo' | 'purple' | 'emerald';
type DivProps = React.HTMLAttributes<HTMLDivElement> & DataAttributes;
type HeadingProps = React.HTMLAttributes<HTMLHeadingElement> & DataAttributes;
type ParagraphProps = React.HTMLAttributes<HTMLParagraphElement> & DataAttributes;

// Keep these as string literals so Tailwind can see every class at build time.
const CARD_DECORATION_CLASSES: Record<Decoration, Record<DecorationColor, string>> = {
  top: {
    gray: 'border-t-4 border-t-gray-500',
    red: 'border-t-4 border-t-red-500',
    amber: 'border-t-4 border-t-amber-500',
    blue: 'border-t-4 border-t-blue-500',
    indigo: 'border-t-4 border-t-indigo-500',
    purple: 'border-t-4 border-t-purple-500',
    emerald: 'border-t-4 border-t-emerald-500',
  },
  left: {
    gray: 'border-l-4 border-l-gray-500',
    red: 'border-l-4 border-l-red-500',
    amber: 'border-l-4 border-l-amber-500',
    blue: 'border-l-4 border-l-blue-500',
    indigo: 'border-l-4 border-l-indigo-500',
    purple: 'border-l-4 border-l-purple-500',
    emerald: 'border-l-4 border-l-emerald-500',
  },
  bottom: {
    gray: 'border-b-4 border-b-gray-500',
    red: 'border-b-4 border-b-red-500',
    amber: 'border-b-4 border-b-amber-500',
    blue: 'border-b-4 border-b-blue-500',
    indigo: 'border-b-4 border-b-indigo-500',
    purple: 'border-b-4 border-b-purple-500',
    emerald: 'border-b-4 border-b-emerald-500',
  },
  right: {
    gray: 'border-r-4 border-r-gray-500',
    red: 'border-r-4 border-r-red-500',
    amber: 'border-r-4 border-r-amber-500',
    blue: 'border-r-4 border-r-blue-500',
    indigo: 'border-r-4 border-r-indigo-500',
    purple: 'border-r-4 border-r-purple-500',
    emerald: 'border-r-4 border-r-emerald-500',
  },
};

const Card = React.forwardRef<
  HTMLDivElement,
  DivProps & {
    decoration?: Decoration;
    decorationColor?: string;
  }
>(({ className, decoration, decorationColor = 'indigo', ...props }, ref) => {
  const decorationClasses = decoration
    ? CARD_DECORATION_CLASSES[decoration][decorationColor as DecorationColor] ||
      CARD_DECORATION_CLASSES[decoration].indigo
    : '';

  return (
    <div
      ref={ref}
      className={cn(
        'rounded-lg border bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100 shadow-sm',
        decorationClasses,
        className
      )}
      {...props}
    />
  );
});
Card.displayName = 'Card';

const CardHeader = React.forwardRef<
  HTMLDivElement,
  DivProps
>(({ className, ...props }, ref) => (
  <div
    ref={ref}
    className={cn('flex flex-col space-y-1.5 p-6', className)}
    {...props}
  />
));
CardHeader.displayName = 'CardHeader';

const CardTitle = React.forwardRef<
  HTMLHeadingElement,
  HeadingProps
>(({ className, ...props }, ref) => (
  <h3
    ref={ref}
    className={cn(
      'text-2xl font-semibold leading-none tracking-tight',
      className
    )}
    {...props}
  />
));
CardTitle.displayName = 'CardTitle';

const CardDescription = React.forwardRef<
  HTMLParagraphElement,
  ParagraphProps
>(({ className, ...props }, ref) => (
  <p
    ref={ref}
    className={cn('text-sm text-gray-500 dark:text-gray-400', className)}
    {...props}
  />
));
CardDescription.displayName = 'CardDescription';

const CardContent = React.forwardRef<
  HTMLDivElement,
  DivProps
>(({ className, ...props }, ref) => (
  <div ref={ref} className={cn('p-6 pt-0', className)} {...props} />
));
CardContent.displayName = 'CardContent';

const CardFooter = React.forwardRef<
  HTMLDivElement,
  DivProps
>(({ className, ...props }, ref) => (
  <div
    ref={ref}
    className={cn('flex items-center p-6 pt-0', className)}
    {...props}
  />
));
CardFooter.displayName = 'CardFooter';

export { Card, CardHeader, CardFooter, CardTitle, CardDescription, CardContent };

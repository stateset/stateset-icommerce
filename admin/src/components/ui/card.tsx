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
    gray: 'border-t-4 border-t-ds-muted-foreground',
    red: 'border-t-4 border-t-ds-status-fail',
    amber: 'border-t-4 border-t-ds-status-warn',
    blue: 'border-t-4 border-t-ds-status-run',
    indigo: 'border-t-4 border-t-ds-primary',
    purple: 'border-t-4 border-t-ds-primary',
    emerald: 'border-t-4 border-t-ds-status-ok',
  },
  left: {
    gray: 'border-l-4 border-l-ds-muted-foreground',
    red: 'border-l-4 border-l-ds-status-fail',
    amber: 'border-l-4 border-l-ds-status-warn',
    blue: 'border-l-4 border-l-ds-status-run',
    indigo: 'border-l-4 border-l-ds-primary',
    purple: 'border-l-4 border-l-ds-primary',
    emerald: 'border-l-4 border-l-ds-status-ok',
  },
  bottom: {
    gray: 'border-b-4 border-b-ds-muted-foreground',
    red: 'border-b-4 border-b-ds-status-fail',
    amber: 'border-b-4 border-b-ds-status-warn',
    blue: 'border-b-4 border-b-ds-status-run',
    indigo: 'border-b-4 border-b-ds-primary',
    purple: 'border-b-4 border-b-ds-primary',
    emerald: 'border-b-4 border-b-ds-status-ok',
  },
  right: {
    gray: 'border-r-4 border-r-ds-muted-foreground',
    red: 'border-r-4 border-r-ds-status-fail',
    amber: 'border-r-4 border-r-ds-status-warn',
    blue: 'border-r-4 border-r-ds-status-run',
    indigo: 'border-r-4 border-r-ds-primary',
    purple: 'border-r-4 border-r-ds-primary',
    emerald: 'border-r-4 border-r-ds-status-ok',
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
        'rounded-lg border border-ds-enterprise-line bg-ds-card text-ds-foreground shadow-sm',
        decorationClasses,
        className,
      )}
      {...props}
    />
  );
});
Card.displayName = 'Card';

const CardHeader = React.forwardRef<HTMLDivElement, DivProps>(({ className, ...props }, ref) => (
  <div ref={ref} className={cn('flex flex-col space-y-1.5 p-6', className)} {...props} />
));
CardHeader.displayName = 'CardHeader';

const CardTitle = React.forwardRef<HTMLHeadingElement, HeadingProps>(
  ({ className, ...props }, ref) => (
    <h3
      ref={ref}
      className={cn('text-2xl font-semibold leading-none tracking-tight', className)}
      {...props}
    />
  ),
);
CardTitle.displayName = 'CardTitle';

const CardDescription = React.forwardRef<HTMLParagraphElement, ParagraphProps>(
  ({ className, ...props }, ref) => (
    <p ref={ref} className={cn('text-sm text-ds-muted-foreground', className)} {...props} />
  ),
);
CardDescription.displayName = 'CardDescription';

const CardContent = React.forwardRef<HTMLDivElement, DivProps>(({ className, ...props }, ref) => (
  <div ref={ref} className={cn('p-6 pt-0', className)} {...props} />
));
CardContent.displayName = 'CardContent';

const CardFooter = React.forwardRef<HTMLDivElement, DivProps>(({ className, ...props }, ref) => (
  <div ref={ref} className={cn('flex items-center p-6 pt-0', className)} {...props} />
));
CardFooter.displayName = 'CardFooter';

export { Card, CardHeader, CardFooter, CardTitle, CardDescription, CardContent };

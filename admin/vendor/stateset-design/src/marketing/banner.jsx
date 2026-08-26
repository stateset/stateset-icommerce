import React from 'react';

import { cn } from '../utils/cn.js';
import { Button } from '../primitives/button.jsx';
import { Input } from '../primitives/input.jsx';

// Marketing banner, per the Figma Banner components. Two tones:
//  - 'brand'  -> electric-indigo ground (ds-primary), white text (Banner True)
//  - 'subtle' -> cool near-white ground (ds-brand-50), ink text (Banner False)
// The action area is composable via `children`; convenience props cover the two
// canonical shapes (a CTA button, or an email-capture row).
const TONE_STYLES = {
  brand: {
    panel: 'bg-ds-primary text-ds-primary-foreground',
    kicker: 'text-ds-primary-foreground/70',
    description: 'text-ds-primary-foreground/85',
  },
  subtle: {
    panel: 'bg-ds-brand-50 text-ds-foreground',
    kicker: 'text-ds-primary',
    description: 'text-ds-muted-foreground',
  },
};

export function Banner({
  tone = 'brand',
  kicker = '',
  title,
  description = '',
  children,
  className = '',
  ...props
}) {
  const t = TONE_STYLES[tone] || TONE_STYLES.brand;

  return (
    <section
      className={cn(
        'flex flex-col gap-6 rounded-2xl px-8 py-10 sm:px-10 md:flex-row md:items-center md:justify-between',
        t.panel,
        className,
      )}
      {...props}>
      <div className="max-w-xl">
        {kicker ? (
          <p
            className={cn(
              'text-[11px] font-semibold uppercase tracking-ds-kicker',
              t.kicker,
            )}>
            {kicker}
          </p>
        ) : null}
        <h2 className="ds-headline mt-2 text-2xl sm:text-3xl">{title}</h2>
        {description ? (
          <p className={cn('mt-3 text-sm leading-6', t.description)}>{description}</p>
        ) : null}
      </div>
      {children ? <div className="shrink-0">{children}</div> : null}
    </section>
  );
}

// Email-capture action row (the "Banner Agent" webinar shape).
export function BannerSubscribe({
  placeholder = 'Email address',
  action = 'Subscribe',
  onSubmit,
  className = '',
}) {
  return (
    <form
      className={cn('flex w-full max-w-sm items-center gap-2', className)}
      onSubmit={(event) => {
        event.preventDefault();
        if (onSubmit) onSubmit(event);
      }}>
      <Input type="email" placeholder={placeholder} aria-label={placeholder} />
      <Button type="submit" size="sm">
        {action}
      </Button>
    </form>
  );
}

export default Banner;

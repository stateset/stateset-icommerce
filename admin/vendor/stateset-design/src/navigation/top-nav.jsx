import React from 'react';

import { cn } from '../utils/cn.js';
import { Button } from '../primitives/button.jsx';

// Top navigation, per the Figma TopNav component: a wordmark, a set of section
// links, and a single primary CTA (e.g. Get Started / Book Demo). Horizontal,
// generous padding, restrained — the brand color is reserved for the CTA.
export function TopNav({
  brand = 'Stateset',
  brandHref = '/',
  items = [],
  cta = null,
  sticky = false,
  className = '',
  ...props
}) {
  return (
    <header
      className={cn(
        'z-30 w-full border-b border-ds-border/80 bg-ds-background/80 backdrop-blur',
        sticky && 'sticky top-0',
        className,
      )}
      {...props}>
      <nav className="mx-auto flex h-16 max-w-ds-shell items-center gap-8 px-6 sm:px-8">
        <a
          href={brandHref}
          className="ds-focus-ring rounded-md font-ds-display text-lg font-bold tracking-ds-heading text-ds-foreground">
          {brand}
        </a>
        <ul className="hidden flex-1 items-center gap-7 md:flex">
          {items.map((item) => (
            <li key={item.label}>
              <a
                href={item.href || '#'}
                className="ds-focus-ring rounded-md text-sm font-medium text-ds-muted-foreground transition-colors hover:text-ds-foreground">
                {item.label}
              </a>
            </li>
          ))}
        </ul>
        {cta ? (
          <div className="ml-auto md:ml-0">
            <Button href={cta.href} size="sm">
              {cta.label}
            </Button>
          </div>
        ) : null}
      </nav>
    </header>
  );
}

export default TopNav;

import React from 'react';

import { cn } from '../utils/cn.js';

// Multi-column footer, per the Figma Footer component: a brand block with a
// tagline, several link columns with uppercase section labels, and a legal row.
export function Footer({
  brand = 'Stateset',
  tagline = '',
  columns = [],
  legal = '',
  className = '',
  ...props
}) {
  return (
    <footer
      className={cn('border-t border-ds-border bg-ds-card text-ds-foreground', className)}
      {...props}>
      <div className="mx-auto max-w-ds-shell px-6 py-16 sm:px-8">
        <div className="grid gap-12 lg:grid-cols-[1.4fr_repeat(auto-fit,minmax(8rem,1fr))]">
          <div className="max-w-xs">
            <div className="font-ds-display text-xl font-bold tracking-ds-heading">{brand}</div>
            {tagline ? (
              <p className="mt-3 text-sm leading-6 text-ds-muted-foreground">{tagline}</p>
            ) : null}
          </div>
          {columns.map((col) => (
            <div key={col.title}>
              <h3 className="text-[11px] font-semibold uppercase tracking-ds-kicker text-ds-muted-foreground">
                {col.title}
              </h3>
              <ul className="mt-4 space-y-3">
                {(col.links || []).map((link) => (
                  <li key={link.label}>
                    <a
                      href={link.href || '#'}
                      className="ds-focus-ring rounded-md text-sm text-ds-foreground/80 transition-colors hover:text-ds-primary">
                      {link.label}
                    </a>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>
        {legal ? (
          <div className="mt-14 border-t border-ds-border/70 pt-6 text-xs text-ds-muted-foreground">
            {legal}
          </div>
        ) : null}
      </div>
    </footer>
  );
}

export default Footer;

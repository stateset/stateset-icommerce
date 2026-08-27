import React from 'react';

import { cn } from '../utils/cn.js';

export function DashboardShell({
  sidebar = null,
  topbar = null,
  children,
  className = '',
  contentClassName = '',
}) {
  return (
    <div
      className={cn(
        'ds-app-frame min-h-screen bg-ds-enterprise-canvas text-ds-foreground',
        className,
      )}>
      <div className="mx-auto min-h-screen max-w-ds-shell lg:grid lg:grid-cols-[var(--ds-sidebar-width)_minmax(0,1fr)]">
        <aside className="hidden border-r border-ds-sidebar-border bg-ds-sidebar text-ds-sidebar-foreground lg:flex lg:flex-col">
          {sidebar}
        </aside>
        <div className="flex min-h-screen min-w-0 flex-col">
          {topbar ? (
            <header className="sticky top-0 z-20 border-b border-ds-enterprise-line bg-ds-enterprise-surface/92 backdrop-blur-xl">
              {topbar}
            </header>
          ) : null}
          <main className={cn('flex-1 px-4 py-6 sm:px-6 lg:px-8', contentClassName)}>
            {children}
          </main>
        </div>
      </div>
    </div>
  );
}

export function DashboardSidebarSection({ label, children, className = '' }) {
  return (
    <section className={cn('px-3 py-4', className)}>
      {label ? (
        <p className="px-3 pb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-ds-sidebar-foreground/55">
          {label}
        </p>
      ) : null}
      <div className="space-y-1">{children}</div>
    </section>
  );
}

export function DashboardSectionHeader({
  eyebrow = '',
  title,
  description = '',
  actions = null,
  className = '',
}) {
  return (
    <div
      className={cn('flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between', className)}>
      <div className="space-y-2">
        {eyebrow ? (
          <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-ds-accent">
            {eyebrow}
          </p>
        ) : null}
        <div className="space-y-2">
          <h1 className="font-ds-display text-2xl font-semibold tracking-normal text-ds-foreground sm:text-3xl">
            {title}
          </h1>
          {description ? (
            <p className="max-w-3xl text-sm leading-6 text-ds-muted-foreground">{description}</p>
          ) : null}
        </div>
      </div>
      {actions ? <div className="flex flex-wrap items-center gap-3">{actions}</div> : null}
    </div>
  );
}

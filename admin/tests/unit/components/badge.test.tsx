/**
 * Tests for Badge component
 * @module tests/unit/components/badge
 */

import React from 'react';
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';

// Mock cn as passthrough
vi.mock('@/lib/utils', () => ({
  cn: (...args: unknown[]) => args.filter(Boolean).join(' '),
}));

import { Badge, badgeVariants } from '@/components/ui/badge';

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe('Badge', () => {
  it('renders children text', () => {
    render(React.createElement(Badge, null, 'Active'));
    expect(screen.getByText('Active')).toBeDefined();
  });

  it('renders as a div element', () => {
    render(
      React.createElement(Badge, { 'data-testid': 'badge' }, 'Status')
    );
    const badge = screen.getByTestId('badge');
    expect(badge.tagName).toBe('DIV');
  });

  it('applies default variant classes', () => {
    render(
      React.createElement(Badge, { 'data-testid': 'badge' }, 'Default')
    );
    const badge = screen.getByTestId('badge');
    expect(badge.className).toContain('bg-ds-foreground');
  });

  it('applies destructive variant classes', () => {
    render(
      React.createElement(
        Badge,
        { variant: 'destructive', 'data-testid': 'badge' },
        'Error'
      )
    );
    const badge = screen.getByTestId('badge');
    expect(badge.className).toContain('bg-ds-destructive');
  });

  it('applies color variant classes', () => {
    render(
      React.createElement(
        Badge,
        { color: 'green', 'data-testid': 'badge' },
        'Success'
      )
    );
    const badge = screen.getByTestId('badge');
    expect(badge.className).toContain('bg-ds-status-ok/10');
  });

  it('applies size variant classes', () => {
    render(
      React.createElement(
        Badge,
        { size: 'lg', 'data-testid': 'badge' },
        'Large'
      )
    );
    const badge = screen.getByTestId('badge');
    expect(badge.className).toContain('px-4');
  });

  it('renders an icon when provided', () => {
    const MockIcon = (props: { className?: string }) =>
      React.createElement('svg', {
        ...props,
        'data-testid': 'badge-icon',
      });
    render(React.createElement(Badge, { icon: MockIcon }, 'With Icon'));
    expect(screen.getByTestId('badge-icon')).toBeDefined();
    expect(screen.getByText('With Icon')).toBeDefined();
  });

  it('does not render an icon when none is provided', () => {
    render(
      React.createElement(Badge, { 'data-testid': 'badge' }, 'No Icon')
    );
    expect(screen.queryByTestId('badge-icon')).toBeNull();
  });

  it('merges custom className', () => {
    render(
      React.createElement(
        Badge,
        { className: 'extra-class', 'data-testid': 'badge' },
        'Custom'
      )
    );
    const badge = screen.getByTestId('badge');
    expect(badge.className).toContain('extra-class');
  });
});

describe('badgeVariants', () => {
  it('is a callable function', () => {
    expect(typeof badgeVariants).toBe('function');
  });

  it('returns a string with base classes', () => {
    const result = badgeVariants();
    expect(typeof result).toBe('string');
    expect(result).toContain('inline-flex');
    expect(result).toContain('rounded-full');
  });
});

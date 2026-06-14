/**
 * Tests for the SimulatedDataBadge honesty marker.
 * @module tests/unit/components/shared/simulated-data-badge
 */

import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';

// cn as passthrough so we can assert the literal base classes are emitted.
vi.mock('@/lib/utils', () => ({
  cn: (...args: unknown[]) => args.filter(Boolean).join(' '),
}));

import { SimulatedDataBadge } from '@/components/shared/simulated-data-badge';

afterEach(() => {
  vi.restoreAllMocks();
});

describe('SimulatedDataBadge', () => {
  it('renders the default label when none is provided', () => {
    render(<SimulatedDataBadge />);
    expect(screen.getByText('Simulated data')).toBeDefined();
  });

  it('renders a custom label', () => {
    render(<SimulatedDataBadge label="Demo numbers" />);
    expect(screen.getByText('Demo numbers')).toBeDefined();
    expect(screen.queryByText('Simulated data')).toBeNull();
  });

  it('exposes the default explanatory tooltip via the title attribute', () => {
    render(<SimulatedDataBadge />);
    const badge = screen.getByText('Simulated data');
    expect(badge.getAttribute('title')).toBe(
      'This widget shows deterministic demo values, not live engine data.'
    );
  });

  it('accepts a custom title', () => {
    render(<SimulatedDataBadge title="No live processor feed" />);
    const badge = screen.getByText('Simulated data');
    expect(badge.getAttribute('title')).toBe('No live processor feed');
  });

  it('renders as a span and merges a custom className over the base styles', () => {
    render(<SimulatedDataBadge className="ml-2" />);
    const badge = screen.getByText('Simulated data');
    expect(badge.tagName).toBe('SPAN');
    // Base styling is preserved alongside the caller-supplied class.
    expect(badge.className).toContain('ml-2');
    expect(badge.className).toContain('rounded-full');
    expect(badge.className).toContain('text-amber-700');
  });
});

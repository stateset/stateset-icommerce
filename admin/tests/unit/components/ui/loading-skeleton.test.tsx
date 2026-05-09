// Component tests for the loading-skeleton primitives. Covers the bare
// Skeleton (animation + a11y), and the LoadingSkeleton dispatcher (renders
// the right shape per `type`, repeats `count` times).

import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/react';

import LoadingSkeleton, { Skeleton } from '@/components/ui/loading-skeleton';

describe('Skeleton', () => {
  it('renders an animated, aria-hidden div', () => {
    const { container } = render(<Skeleton />);
    const sk = container.firstChild as HTMLElement;
    expect(sk.tagName).toBe('DIV');
    expect(sk.getAttribute('aria-hidden')).toBe('true');
    expect(sk.className).toMatch(/animate-pulse/);
  });

  it('merges user className', () => {
    const { container } = render(<Skeleton className="custom-marker" />);
    expect((container.firstChild as HTMLElement).className).toContain('custom-marker');
  });
});

describe('LoadingSkeleton', () => {
  it('renders one card by default (no props)', () => {
    const { container } = render(<LoadingSkeleton />);
    // A "card" type renders an outer wrapper with multiple skeleton lines.
    expect(container.querySelectorAll('[aria-hidden="true"]').length).toBeGreaterThan(0);
  });

  it('repeats `count` skeletons when count > 1', () => {
    const { container: one } = render(<LoadingSkeleton type="metric" count={1} />);
    const { container: three } = render(<LoadingSkeleton type="metric" count={3} />);
    const oneCount = one.querySelectorAll('[aria-hidden="true"]').length;
    const threeCount = three.querySelectorAll('[aria-hidden="true"]').length;
    // Three should be roughly 3x one (per-shape skeleton count is fixed).
    expect(threeCount).toBe(oneCount * 3);
  });

  it.each(['metric', 'chart', 'table', 'list', 'card'] as const)(
    'renders the %s shape without throwing',
    (type) => {
      const { container } = render(<LoadingSkeleton type={type} />);
      expect(container.querySelectorAll('[aria-hidden="true"]').length).toBeGreaterThan(0);
    },
  );
});

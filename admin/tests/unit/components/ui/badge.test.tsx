// Component tests for the shared Badge. Locks down variant/color/size
// className composition, optional leading icon, and the forwarded
// data-* attributes used by analytics tooling.

import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { Badge } from '@/components/ui/badge';

describe('Badge', () => {
  it('renders its children', () => {
    render(<Badge>Active</Badge>);
    expect(screen.getByText('Active')).toBeInTheDocument();
  });

  it('applies the default variant + size classes', () => {
    const { container } = render(<Badge>x</Badge>);
    const badge = container.firstChild as HTMLElement;
    expect(badge.className).toMatch(/bg-gray-900/);
    expect(badge.className).toMatch(/px-2\.5/); // sm size default
  });

  it.each([
    ['secondary', /bg-gray-100/],
    ['destructive', /bg-red-500/],
    ['outline', /text-gray-950/],
  ] as const)('applies the %s variant className', (variant, pattern) => {
    const { container } = render(<Badge variant={variant}>v</Badge>);
    expect((container.firstChild as HTMLElement).className).toMatch(pattern);
  });

  it.each([
    ['red', /bg-red-100/],
    ['green', /bg-green-100/],
    ['blue', /bg-blue-100/],
    ['amber', /bg-amber-100/],
    ['indigo', /bg-indigo-100/],
  ] as const)('applies the %s color className', (color, pattern) => {
    const { container } = render(<Badge color={color}>c</Badge>);
    expect((container.firstChild as HTMLElement).className).toMatch(pattern);
  });

  it.each([
    ['xs', /px-2 py-0\.5/],
    ['md', /px-3 py-1/],
    ['lg', /px-4 py-1\.5/],
  ] as const)('applies the %s size className', (size, pattern) => {
    const { container } = render(<Badge size={size}>s</Badge>);
    expect((container.firstChild as HTMLElement).className).toMatch(pattern);
  });

  it('merges user-provided className', () => {
    const { container } = render(<Badge className="custom-marker">x</Badge>);
    expect((container.firstChild as HTMLElement).className).toContain('custom-marker');
  });

  it('renders the optional leading icon component', () => {
    const Icon = ({ className }: { className?: string }) => (
      <svg data-testid="leading-icon" className={className} />
    );
    render(<Badge icon={Icon}>x</Badge>);
    expect(screen.getByTestId('leading-icon')).toBeInTheDocument();
  });

  it('forwards data-* attributes for analytics tagging', () => {
    const { container } = render(
      <Badge data-status="active" data-id="42">
        x
      </Badge>,
    );
    const badge = container.firstChild as HTMLElement;
    expect(badge.getAttribute('data-status')).toBe('active');
    expect(badge.getAttribute('data-id')).toBe('42');
  });
});

// Component tests for the ProgressBar primitive. Locks down value clamping,
// the optional percentage label, color/size variant classes, and the inline
// width style derived from value/max.

import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { ProgressBar } from '@/components/ui/progress';

// Structure (showLabel=false):
//   wrapper (div) > track (div) > fill (div)
// We walk children directly rather than using CSS selectors, since
// querySelector('div > div') matches relative to the document root, not
// relative to the wrapper, and would pick up the wrong div in jsdom.

const trackFromBar = (container: HTMLElement) => {
  const wrapper = container.firstChild as HTMLElement;
  return wrapper.children[wrapper.children.length - 1] as HTMLElement;
};

const innerFill = (container: HTMLElement) => {
  const track = trackFromBar(container);
  return track.children[0] as HTMLElement;
};

describe('ProgressBar', () => {
  it('renders the fill at the correct percentage of value/max', () => {
    const { container } = render(<ProgressBar value={50} max={100} />);
    expect(innerFill(container).style.width).toBe('50%');
  });

  it('clamps values above max to 100%', () => {
    const { container } = render(<ProgressBar value={500} max={100} />);
    expect(innerFill(container).style.width).toBe('100%');
  });

  it('clamps negative values to 0%', () => {
    const { container } = render(<ProgressBar value={-10} max={100} />);
    expect(innerFill(container).style.width).toBe('0%');
  });

  it('honors a custom max', () => {
    const { container } = render(<ProgressBar value={5} max={20} />);
    // 5 / 20 = 25%
    expect(innerFill(container).style.width).toBe('25%');
  });

  it('does not render the percentage label by default', () => {
    render(<ProgressBar value={50} />);
    expect(screen.queryByText(/%$/)).not.toBeInTheDocument();
  });

  it('renders the percentage label when showLabel is true', () => {
    render(<ProgressBar value={42} showLabel />);
    expect(screen.getByText('42%')).toBeInTheDocument();
  });

  it.each([
    ['emerald', /bg-emerald-500/],
    ['amber', /bg-amber-500/],
    ['red', /bg-red-500/],
    ['blue', /bg-blue-500/],
    ['indigo', /bg-indigo-500/],
    ['purple', /bg-purple-500/],
    ['gray', /bg-gray-500/],
  ] as const)('applies the %s color class', (color, pattern) => {
    const { container } = render(<ProgressBar value={50} color={color} />);
    expect(innerFill(container).className).toMatch(pattern);
  });

  it.each([
    ['sm', /h-1\.5/],
    ['md', /h-2/],
    ['lg', /h-3/],
  ] as const)('applies the %s size class', (size, pattern) => {
    const { container } = render(<ProgressBar value={50} size={size} />);
    // The size class is applied to BOTH the track and the fill, so the
    // helper that returns the track is fine here.
    expect(trackFromBar(container).className).toMatch(pattern);
  });

  it('merges user className on the outer wrapper', () => {
    const { container } = render(<ProgressBar value={50} className="custom-marker" />);
    expect((container.firstChild as HTMLElement).className).toContain('custom-marker');
  });
});

// Component tests for the shared Button. These exercise the variant-driven
// className composition, the `asChild` Radix Slot escape hatch, and the
// forwardRef contract. They're cheap, deterministic, and catch regressions
// in the design-system layer that route-level tests don't surface.

import { describe, expect, it, vi } from 'vitest';
import { createRef } from 'react';
import { render, screen, fireEvent } from '@testing-library/react';

import { Button } from '@/components/ui/button';

describe('Button', () => {
  it('renders its children inside a <button> by default', () => {
    render(<Button>Click me</Button>);
    const button = screen.getByRole('button', { name: 'Click me' });
    expect(button).toBeInTheDocument();
    expect(button.tagName).toBe('BUTTON');
  });

  it('forwards refs to the underlying element', () => {
    const ref = createRef<HTMLButtonElement>();
    render(<Button ref={ref}>r</Button>);
    expect(ref.current).toBeInstanceOf(HTMLButtonElement);
  });

  it('applies the default variant + size classes', () => {
    render(<Button>x</Button>);
    const button = screen.getByRole('button');
    // Default variant has the dark "gray-900" bg, default size is h-9 px-4 py-2.
    expect(button.className).toMatch(/bg-ds-foreground/);
    expect(button.className).toMatch(/h-9/);
  });

  it.each([
    ['destructive', /bg-ds-destructive/],
    ['outline', /border/],
    ['secondary', /bg-ds-muted/],
    ['ghost', /hover:bg-ds-muted/],
    ['link', /underline-offset-4/],
    ['primary', /bg-ds-primary/],
  ] as const)('applies the %s variant className', (variant, pattern) => {
    render(<Button variant={variant}>v</Button>);
    expect(screen.getByRole('button').className).toMatch(pattern);
  });

  it.each([
    ['sm', /h-8/],
    ['lg', /h-10/],
    ['icon', /h-9 w-9/],
  ] as const)('applies the %s size className', (size, pattern) => {
    render(<Button size={size}>s</Button>);
    expect(screen.getByRole('button').className).toMatch(pattern);
  });

  it('merges user-provided className with variant classes', () => {
    render(<Button className="custom-marker">x</Button>);
    expect(screen.getByRole('button').className).toContain('custom-marker');
  });

  it('forwards onClick events', () => {
    const onClick = vi.fn();
    render(<Button onClick={onClick}>click</Button>);
    fireEvent.click(screen.getByRole('button'));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it('honors the disabled attribute', () => {
    const onClick = vi.fn();
    render(
      <Button disabled onClick={onClick}>
        x
      </Button>,
    );
    const button = screen.getByRole('button');
    expect(button).toBeDisabled();
    fireEvent.click(button);
    expect(onClick).not.toHaveBeenCalled();
  });

  it('renders as the child element when `asChild` is set', () => {
    render(
      <Button asChild>
        <a href="/dashboard">Go</a>
      </Button>,
    );
    const link = screen.getByRole('link', { name: 'Go' });
    expect(link).toBeInTheDocument();
    expect(link.tagName).toBe('A');
    expect(link).toHaveAttribute('href', '/dashboard');
    // Should still have the button-variant classes
    expect(link.className).toMatch(/inline-flex/);
  });
});

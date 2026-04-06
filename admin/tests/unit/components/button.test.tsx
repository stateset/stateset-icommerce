/**
 * Tests for Button component
 * @module tests/unit/components/button
 */

import React from 'react';
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

// Mock @radix-ui/react-slot
vi.mock('@radix-ui/react-slot', () => ({
  Slot: React.forwardRef(
    (
      { children, ...props }: { children?: React.ReactNode },
      ref: React.Ref<HTMLElement>
    ) => {
      if (React.isValidElement(children)) {
        return React.cloneElement(children as React.ReactElement<Record<string, unknown>>, {
          ...props,
          ref,
        });
      }
      return React.createElement('span', { ...props, ref }, children);
    }
  ),
}));

// Mock cn as passthrough
vi.mock('@/lib/utils', () => ({
  cn: (...args: unknown[]) => args.filter(Boolean).join(' '),
}));

import { Button, buttonVariants } from '@/components/ui/button';

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe('Button', () => {
  it('renders a button element by default', () => {
    render(React.createElement(Button, null, 'Click me'));
    const button = screen.getByRole('button', { name: 'Click me' });
    expect(button).toBeDefined();
    expect(button.tagName).toBe('BUTTON');
  });

  it('renders children text', () => {
    render(React.createElement(Button, null, 'Submit'));
    expect(screen.getByText('Submit')).toBeDefined();
  });

  it('forwards onClick handler', () => {
    const onClick = vi.fn();
    render(React.createElement(Button, { onClick }, 'Press'));
    fireEvent.click(screen.getByRole('button'));
    expect(onClick).toHaveBeenCalledOnce();
  });

  it('applies the default variant classes', () => {
    render(React.createElement(Button, null, 'Default'));
    const button = screen.getByRole('button');
    expect(button.className).toContain('bg-gray-900');
  });

  it('applies the destructive variant classes', () => {
    render(React.createElement(Button, { variant: 'destructive' }, 'Delete'));
    const button = screen.getByRole('button');
    expect(button.className).toContain('bg-red-500');
  });

  it('applies the outline variant classes', () => {
    render(React.createElement(Button, { variant: 'outline' }, 'Outline'));
    const button = screen.getByRole('button');
    expect(button.className).toContain('border');
  });

  it('applies the primary variant classes', () => {
    render(React.createElement(Button, { variant: 'primary' }, 'Primary'));
    const button = screen.getByRole('button');
    expect(button.className).toContain('bg-indigo-600');
  });

  it('applies the sm size classes', () => {
    render(React.createElement(Button, { size: 'sm' }, 'Small'));
    const button = screen.getByRole('button');
    expect(button.className).toContain('h-8');
  });

  it('applies the lg size classes', () => {
    render(React.createElement(Button, { size: 'lg' }, 'Large'));
    const button = screen.getByRole('button');
    expect(button.className).toContain('h-10');
  });

  it('applies the icon size classes', () => {
    render(React.createElement(Button, { size: 'icon' }, 'X'));
    const button = screen.getByRole('button');
    expect(button.className).toContain('w-9');
  });

  it('supports the disabled attribute', () => {
    render(React.createElement(Button, { disabled: true }, 'Disabled'));
    const button = screen.getByRole('button') as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });

  it('renders via Slot when asChild is true', () => {
    render(
      React.createElement(
        Button,
        { asChild: true },
        React.createElement('a', { href: '/test' }, 'Link Button')
      )
    );
    const link = screen.getByText('Link Button');
    expect(link.tagName).toBe('A');
    expect(link.getAttribute('href')).toBe('/test');
  });

  it('has displayName Button', () => {
    expect(Button.displayName).toBe('Button');
  });
});

describe('buttonVariants', () => {
  it('is a callable function', () => {
    expect(typeof buttonVariants).toBe('function');
  });

  it('returns a string with base classes', () => {
    const result = buttonVariants();
    expect(typeof result).toBe('string');
    expect(result).toContain('inline-flex');
  });
});

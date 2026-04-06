/**
 * Tests for Card components
 * @module tests/unit/components/card
 */

import React from 'react';
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';

// Mock cn as passthrough
vi.mock('@/lib/utils', () => ({
  cn: (...args: unknown[]) => args.filter(Boolean).join(' '),
}));

import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter,
} from '@/components/ui/card';

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe('Card', () => {
  it('renders a div with children', () => {
    render(
      React.createElement(Card, { 'data-testid': 'card' }, 'Card content')
    );
    const card = screen.getByTestId('card');
    expect(card).toBeDefined();
    expect(card.textContent).toBe('Card content');
  });

  it('applies base card classes', () => {
    render(React.createElement(Card, { 'data-testid': 'card' }));
    const card = screen.getByTestId('card');
    expect(card.className).toContain('rounded-lg');
    expect(card.className).toContain('shadow-sm');
  });

  it('applies top decoration classes', () => {
    render(
      React.createElement(Card, {
        'data-testid': 'card',
        decoration: 'top',
        decorationColor: 'red',
      })
    );
    const card = screen.getByTestId('card');
    expect(card.className).toContain('border-t-4');
    expect(card.className).toContain('border-t-red-500');
  });

  it('applies left decoration classes with default color', () => {
    render(
      React.createElement(Card, {
        'data-testid': 'card',
        decoration: 'left',
      })
    );
    const card = screen.getByTestId('card');
    expect(card.className).toContain('border-l-4');
    expect(card.className).toContain('border-l-indigo-500');
  });

  it('merges custom className', () => {
    render(
      React.createElement(Card, {
        'data-testid': 'card',
        className: 'my-custom-class',
      })
    );
    const card = screen.getByTestId('card');
    expect(card.className).toContain('my-custom-class');
  });

  it('has displayName Card', () => {
    expect(Card.displayName).toBe('Card');
  });
});

describe('CardHeader', () => {
  it('renders children and applies base classes', () => {
    render(
      React.createElement(
        CardHeader,
        { 'data-testid': 'header' },
        'Header content'
      )
    );
    const header = screen.getByTestId('header');
    expect(header.textContent).toBe('Header content');
    expect(header.className).toContain('p-6');
  });

  it('has displayName CardHeader', () => {
    expect(CardHeader.displayName).toBe('CardHeader');
  });
});

describe('CardTitle', () => {
  it('renders an h3 element with children', () => {
    render(React.createElement(CardTitle, null, 'My Title'));
    const title = screen.getByText('My Title');
    expect(title.tagName).toBe('H3');
    expect(title.className).toContain('font-semibold');
  });
});

describe('CardDescription', () => {
  it('renders a p element with children', () => {
    render(React.createElement(CardDescription, null, 'A description'));
    const desc = screen.getByText('A description');
    expect(desc.tagName).toBe('P');
    expect(desc.className).toContain('text-sm');
  });
});

describe('CardContent', () => {
  it('renders children with content padding', () => {
    render(
      React.createElement(
        CardContent,
        { 'data-testid': 'content' },
        'Body text'
      )
    );
    const content = screen.getByTestId('content');
    expect(content.textContent).toBe('Body text');
    expect(content.className).toContain('p-6');
  });
});

describe('CardFooter', () => {
  it('renders children with flex layout', () => {
    render(
      React.createElement(
        CardFooter,
        { 'data-testid': 'footer' },
        'Footer text'
      )
    );
    const footer = screen.getByTestId('footer');
    expect(footer.textContent).toBe('Footer text');
    expect(footer.className).toContain('flex');
    expect(footer.className).toContain('items-center');
  });

  it('has displayName CardFooter', () => {
    expect(CardFooter.displayName).toBe('CardFooter');
  });
});

describe('Card composition', () => {
  it('renders a full card with all sub-components', () => {
    render(
      React.createElement(
        Card,
        { 'data-testid': 'full-card' },
        React.createElement(
          CardHeader,
          null,
          React.createElement(CardTitle, null, 'Order Summary'),
          React.createElement(
            CardDescription,
            null,
            'Details about the order'
          )
        ),
        React.createElement(CardContent, null, 'Main content here'),
        React.createElement(CardFooter, null, 'Action buttons')
      )
    );

    expect(screen.getByText('Order Summary')).toBeDefined();
    expect(screen.getByText('Details about the order')).toBeDefined();
    expect(screen.getByText('Main content here')).toBeDefined();
    expect(screen.getByText('Action buttons')).toBeDefined();
  });
});

// Component tests for the Card primitives. Locks down the ref forwarding,
// className composition, the optional decoration border (top/left/bottom/right
// × 7 colors), and the standard sub-component layout classes.

import { describe, expect, it } from 'vitest';
import { createRef } from 'react';
import { render, screen } from '@testing-library/react';

import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';

describe('Card', () => {
  it('renders children inside a div with the base class set', () => {
    const { container } = render(<Card data-testid="card">body</Card>);
    expect(screen.getByTestId('card')).toHaveTextContent('body');
    expect((container.firstChild as HTMLElement).className).toMatch(/rounded-lg/);
  });

  it('forwards refs', () => {
    const ref = createRef<HTMLDivElement>();
    render(<Card ref={ref}>x</Card>);
    expect(ref.current).toBeInstanceOf(HTMLDivElement);
  });

  it('merges user className', () => {
    const { container } = render(<Card className="custom-marker">x</Card>);
    expect((container.firstChild as HTMLElement).className).toContain('custom-marker');
  });

  it.each([
    ['top', /border-t-4/],
    ['left', /border-l-4/],
    ['bottom', /border-b-4/],
    ['right', /border-r-4/],
  ] as const)('applies the %s decoration border', (decoration, pattern) => {
    const { container } = render(<Card decoration={decoration}>x</Card>);
    expect((container.firstChild as HTMLElement).className).toMatch(pattern);
  });

  it.each([
    ['red', /border-t-ds-status-fail/],
    ['amber', /border-t-ds-status-warn/],
    ['blue', /border-t-ds-status-run/],
    ['indigo', /border-t-ds-primary/],
    ['purple', /border-t-ds-primary/],
    ['emerald', /border-t-ds-status-ok/],
    ['gray', /border-t-ds-muted-foreground/],
  ] as const)('applies the %s decoration color on top', (color, pattern) => {
    const { container } = render(
      <Card decoration="top" decorationColor={color}>
        x
      </Card>,
    );
    expect((container.firstChild as HTMLElement).className).toMatch(pattern);
  });

  it('falls back to indigo when an unknown decorationColor is given', () => {
    const { container } = render(
      <Card decoration="top" decorationColor="not-a-color">
        x
      </Card>,
    );
    expect((container.firstChild as HTMLElement).className).toMatch(/border-t-ds-primary/);
  });

  it('omits the decoration class when decoration is unset', () => {
    const { container } = render(<Card>x</Card>);
    expect((container.firstChild as HTMLElement).className).not.toMatch(/border-t-4/);
  });
});

describe('Card sub-components', () => {
  it('CardHeader renders with header layout classes', () => {
    const { container } = render(<CardHeader>h</CardHeader>);
    expect((container.firstChild as HTMLElement).className).toMatch(/p-6/);
    expect((container.firstChild as HTMLElement).className).toMatch(/flex-col/);
  });

  it('CardTitle renders as <h3> with title classes', () => {
    render(<CardTitle>Title</CardTitle>);
    const heading = screen.getByRole('heading', { level: 3, name: 'Title' });
    expect(heading.className).toMatch(/font-semibold/);
  });

  it('CardDescription renders as <p>', () => {
    const { container } = render(<CardDescription>desc</CardDescription>);
    const desc = container.firstChild as HTMLElement;
    expect(desc.tagName).toBe('P');
    expect(desc).toHaveTextContent('desc');
  });

  it('CardContent renders with content padding', () => {
    const { container } = render(<CardContent>c</CardContent>);
    expect((container.firstChild as HTMLElement).className).toMatch(/p-6 pt-0/);
  });

  it('CardFooter renders with footer flex classes', () => {
    const { container } = render(<CardFooter>f</CardFooter>);
    expect((container.firstChild as HTMLElement).className).toMatch(/items-center/);
  });

  it('all sub-components forward refs', () => {
    const headerRef = createRef<HTMLDivElement>();
    const titleRef = createRef<HTMLHeadingElement>();
    const descRef = createRef<HTMLParagraphElement>();
    const contentRef = createRef<HTMLDivElement>();
    const footerRef = createRef<HTMLDivElement>();
    render(
      <Card>
        <CardHeader ref={headerRef}>
          <CardTitle ref={titleRef}>t</CardTitle>
          <CardDescription ref={descRef}>d</CardDescription>
        </CardHeader>
        <CardContent ref={contentRef}>c</CardContent>
        <CardFooter ref={footerRef}>f</CardFooter>
      </Card>,
    );
    expect(headerRef.current).toBeInstanceOf(HTMLDivElement);
    expect(titleRef.current).toBeInstanceOf(HTMLHeadingElement);
    expect(descRef.current).toBeInstanceOf(HTMLParagraphElement);
    expect(contentRef.current).toBeInstanceOf(HTMLDivElement);
    expect(footerRef.current).toBeInstanceOf(HTMLDivElement);
  });
});

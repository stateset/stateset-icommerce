// Component test for ExceptionManagement. The component is pure
// presentational with an optional `data` prop and a built-in demo
// fallback — test both branches.

import { render, screen } from '@testing-library/react';
import type { HTMLAttributes, ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

vi.mock('framer-motion', () => ({
  motion: {
    div: ({ children, ...props }: HTMLAttributes<HTMLDivElement>) => (
      <div {...props}>{children}</div>
    ),
    tr: ({ children, ...props }: HTMLAttributes<HTMLTableRowElement>) => (
      <tr {...props}>{children}</tr>
    ),
  },
}));

vi.mock('@tremor/react', () => {
  const Wrapper = ({ children }: { children?: ReactNode }) => <div>{children}</div>;
  return {
    Card: Wrapper,
    Title: Wrapper,
    Text: Wrapper,
    Badge: Wrapper,
    Grid: Wrapper,
    Metric: Wrapper,
    ProgressBar: Wrapper,
  };
});

import ExceptionManagement from '@/components/operations/exception-management';

describe('ExceptionManagement', () => {
  it('renders demo data when no payload is provided', () => {
    render(<ExceptionManagement />);
    // Stable headings always render
    expect(screen.getByText('Open Exceptions')).toBeInTheDocument();
    expect(screen.getByText('Auto-Resolved')).toBeInTheDocument();
    // Demo headings
    expect(screen.getByText('Exception Severity Distribution')).toBeInTheDocument();
  });

  it('uses the supplied summary counts over the demo defaults', () => {
    render(
      <ExceptionManagement
        data={{
          summary: {
            openCount: 7,
            criticalCount: 2,
            investigatingCount: 1,
            resolvedToday: 3,
            autoResolvedPercent: 80,
            bySeverity: { critical: 2, high: 3, medium: 1, low: 1 },
          },
          exceptions: [],
          recentResolutions: [],
        }}
      />,
    );
    expect(screen.getByText('7')).toBeInTheDocument();
    expect(screen.getByText('2 critical')).toBeInTheDocument();
    expect(screen.getByText('80%')).toBeInTheDocument();
  });
});

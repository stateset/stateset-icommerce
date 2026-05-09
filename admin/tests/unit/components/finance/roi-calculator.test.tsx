// Component test for ROICalculator. The component takes optional `data`
// and falls back to a built-in demo payload — both branches need to
// render without crashing. We don't mock `useEmbeddedData` because the
// component doesn't use it (pure presentational with optional input).

import { render, screen } from '@testing-library/react';
import type { HTMLAttributes, ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

vi.mock('framer-motion', () => ({
  motion: {
    div: ({ children, ...props }: HTMLAttributes<HTMLDivElement>) => (
      <div {...props}>{children}</div>
    ),
  },
}));

vi.mock('@tremor/react', () => {
  const Wrapper = ({ children }: { children?: ReactNode }) => <div>{children}</div>;
  const Chart = ({ data }: { data?: unknown[] }) => (
    <div data-testid="chart">{data?.length ?? 0}</div>
  );
  return {
    Card: Wrapper,
    Title: Wrapper,
    Text: Wrapper,
    Badge: Wrapper,
    Grid: Wrapper,
    Metric: Wrapper,
    BarChart: Chart,
    DonutChart: Chart,
    ProgressBar: Wrapper,
  };
});

import ROICalculator from '@/components/finance/roi-calculator';

describe('ROICalculator', () => {
  it('falls back to the built-in demo data when no payload is provided', () => {
    render(<ROICalculator />);
    // Headings stay regardless of payload
    expect(screen.getByText('Cost Breakdown')).toBeInTheDocument();
    expect(screen.getByText('Savings by Category')).toBeInTheDocument();
    expect(screen.getByText('Payback Analysis')).toBeInTheDocument();
    // Demo data — milestone names
    expect(screen.getByText('Break Even')).toBeInTheDocument();
    expect(screen.getByText('100% ROI')).toBeInTheDocument();
  });

  it('renders the supplied payload instead of the demo when one is provided', () => {
    const data = {
      summary: {
        annualSavings: 1,
        savingsGrowth: 0,
        roi: 100,
        paybackMonths: 1,
        hoursSaved: 1,
        costPerTransaction: 0.01,
        costReduction: 0,
        totalValueGenerated: 1,
      },
      costBreakdown: {
        categories: [{ name: 'Shipping', value: 500, trend: -5 }],
        savingsByCategory: [{ name: 'Picking', saved: 100, previous: 200 }],
      },
      savingsProjection: { monthly: [{ month: 'Jan', currentCost: 1, projectedCost: 1, savings: 0 }] },
      paybackAnalysis: {
        initialInvestment: 1,
        monthlyCost: 1,
        monthlySavings: 1,
        milestones: [{ name: 'Custom Milestone', timeline: 'Month X', achieved: false }],
      },
    };
    render(<ROICalculator data={data} />);
    expect(screen.getByText('Custom Milestone')).toBeInTheDocument();
    expect(screen.getByText('Shipping')).toBeInTheDocument();
    expect(screen.getByText('Picking')).toBeInTheDocument();
    expect(screen.queryByText('Break Even')).toBeNull();
  });
});

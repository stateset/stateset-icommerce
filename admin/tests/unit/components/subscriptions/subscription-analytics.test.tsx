// Component test for SubscriptionAnalytics. Every figure comes from the
// subscription records; figures nothing records (MRR history, plan changes,
// cancellation reasons) must say they are unavailable, never show demo data.

import { render, screen } from '@testing-library/react';
import type { HTMLAttributes, ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { SubscriptionAnalyticsData } from '@/lib/types/dashboard-data';

const useEmbeddedDataMock = vi.fn();

vi.mock('@/hooks/use-embedded-data', () => ({
  useEmbeddedData: (fetcher: () => Promise<unknown>, options?: unknown) =>
    useEmbeddedDataMock(fetcher, options),
}));

vi.mock('@/app/actions/commerce', () => ({
  getSubscriptionAnalyticsData: vi.fn(),
}));

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
  const Chart = ({ data }: { data?: unknown[] }) => (
    <div data-testid="chart">{data?.length ?? 0}</div>
  );
  return { AreaChart: Chart, DonutChart: Chart, ProgressBar: Wrapper };
});

import SubscriptionAnalytics from '@/components/subscriptions/subscription-analytics';

afterEach(() => {
  vi.clearAllMocks();
});

const derived: SubscriptionAnalyticsData = {
  summary: {
    mrrExact: '63.36',
    mrrGrowth: null,
    activeCount: 2,
    churnRate: 0.25,
    arpuExact: '31.68',
    statusBreakdown: { active: 2, paused: 1, cancelled: 1 },
    activityDays: 30,
    newSubscribers: 1,
    newMrrExact: '19.99',
    cancellations: 1,
    churnedMrrExact: '10.00',
    upgrades: null,
    expansionMrrExact: null,
    complete: true,
  },
  mrrTrend: null,
  churnAnalysis: null,
  planDistribution: [{ plan: 'Pro', count: 2, mrrExact: '63.36' }],
  upcomingRenewals: {
    days: 7,
    count: 3,
    amountExact: '59.97',
    next: [
      {
        id: 'sub_1',
        customerName: 'Ada Lovelace',
        email: 'ada@example.com',
        plan: 'Pro',
        renewalDate: '2026-11-16',
        amountExact: '19.99',
      },
      {
        id: 'sub_2',
        customerName: 'cus_gone',
        plan: 'Pro',
        renewalDate: '2026-11-18',
        amountExact: '19.99',
      },
    ],
  },
};

function serve(data: SubscriptionAnalyticsData) {
  useEmbeddedDataMock.mockReturnValue({ data, isLoading: false, error: null });
}

describe('SubscriptionAnalytics', () => {
  it('renders the figures derived from subscription records', () => {
    serve(derived);
    render(<SubscriptionAnalytics />);

    // Exact money, and the 0-1 churn ratio as a percentage.
    expect(screen.getAllByText('$63.36').length).toBeGreaterThan(0);
    expect(screen.getByText('$31.68')).toBeInTheDocument();
    expect(screen.getByText('25.0%')).toBeInTheDocument();
    expect(screen.getByText('$19.99', { selector: '.ds-instrument-number' })).toBeInTheDocument();
    expect(screen.getByText('From 1 new subscriptions in the last 30 days')).toBeInTheDocument();
    expect(screen.getByText('$10.00')).toBeInTheDocument();
    expect(screen.getByText('From 1 cancellations in the last 30 days')).toBeInTheDocument();
    expect(screen.getByText('Pro · 2 active')).toBeInTheDocument();

    // Renewals name real customers, with the exact expected total of all three.
    expect(screen.getByText('Ada Lovelace')).toBeInTheDocument();
    expect(screen.getByText('ada@example.com')).toBeInTheDocument();
    expect(screen.getByText('cus_gone')).toBeInTheDocument();
    expect(screen.getByText('$59.97 expected')).toBeInTheDocument();
    expect(screen.getByText('The next 2 of 3 renewals')).toBeInTheDocument();
    expect(screen.queryByText('Risk')).toBeNull();
  });

  it('says which figures are unavailable instead of inventing them', () => {
    serve(derived);
    render(<SubscriptionAnalytics />);

    expect(screen.getByText('No MRR history to compare')).toBeInTheDocument();
    expect(
      screen.getByText(
        'MRR history is not available: the engine keeps current subscriptions, not past MRR.',
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        'Cancellation reasons are not available: they are not on subscription records.',
      ),
    ).toBeInTheDocument();
    expect(screen.getByText('Plan changes are not recorded')).toBeInTheDocument();
    // Expansion MRR has no source.
    expect(screen.getByText('—')).toBeInTheDocument();

    // None of the old demo figures or names.
    for (const invented of [
      '$45,000.00',
      '1,250',
      '3.2%',
      'Acme Corp',
      'Too expensive',
      '$5,200.00',
    ]) {
      expect(screen.queryByText(invented)).toBeNull();
    }
  });

  it('shows growth when a source reports it and says when there is nothing to show', () => {
    serve({
      ...derived,
      summary: { ...derived.summary, mrrGrowth: 0.08, statusBreakdown: {} },
      planDistribution: [],
      upcomingRenewals: { days: 7, count: 0, amountExact: '0', next: [] },
    });
    render(<SubscriptionAnalytics />);

    expect(screen.getByText('+8.0% vs last month')).toBeInTheDocument();
    expect(screen.getByText('No active subscriptions')).toBeInTheDocument();
    expect(screen.getByText('No subscriptions')).toBeInTheDocument();
    expect(screen.getByText('No renewals in the next 7 days')).toBeInTheDocument();
  });

  it('flags figures that stop short of every subscription', () => {
    serve({ ...derived, summary: { ...derived.summary, complete: false } });
    render(<SubscriptionAnalytics />);

    expect(
      screen.getByText(
        'These figures stop short of every subscription: the scan reached its record limit.',
      ),
    ).toBeInTheDocument();
  });
});

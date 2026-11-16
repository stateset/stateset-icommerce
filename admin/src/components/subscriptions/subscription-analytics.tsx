'use client';

import { memo } from 'react';
import { AreaChart, DonutChart, ProgressBar } from '@tremor/react';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
  Badge,
  MetricCard,
  StatusPill,
} from '@stateset/design';
import {
  CreditCardIcon,
  ArrowTrendingUpIcon,
  UserMinusIcon,
  CalendarIcon,
} from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getSubscriptionAnalyticsData } from '@/app/actions/commerce';
import { formatCurrency, formatMeasured, formatNumber, formatRatio } from '@/lib/utils';
import { formatMoney } from '@/lib/finance/format';
import type {
  SubscriptionAnalyticsData,
  ChurnReason,
  UpcomingRenewal,
} from '@/lib/types/dashboard-data';
import type { SubscriptionStatus } from '@/lib/engine-lifecycles';

type DsStatus = 'ok' | 'run' | 'warn' | 'fail' | 'review' | 'idle';

interface SubscriptionAnalyticsProps {
  data?: SubscriptionAnalyticsData;
}

/** One pill per engine subscription status (`SubscriptionStatus`). */
const statusPills: Record<SubscriptionStatus, DsStatus> = {
  pending: 'review',
  trial: 'run',
  active: 'ok',
  paused: 'idle',
  past_due: 'warn',
  cancelled: 'fail',
  expired: 'idle',
};

const money = (amount: string) => formatMoney(amount);

/** A card body that explains why there is nothing to show. */
function Unavailable({ children, className = '' }: { children: string; className?: string }) {
  return (
    <div className={`flex items-center justify-center ${className}`}>
      <p className="text-sm text-ds-muted-foreground text-center">{children}</p>
    </div>
  );
}

function SubscriptionAnalyticsInner({ data: propData }: SubscriptionAnalyticsProps) {
  const { data, isLoading, error } = useEmbeddedData(() => getSubscriptionAnalyticsData(), {
    initialData: propData,
    refreshInterval: 60000,
  });

  if (isLoading && !data) {
    return (
      <Card>
        <CardContent>
          <div className="animate-pulse space-y-4">
            <div className="h-6 bg-ds-muted rounded w-48" />
            <div className="h-64 bg-ds-muted rounded" />
          </div>
        </CardContent>
      </Card>
    );
  }

  if (error || !data) {
    return (
      <Card className="border-ds-status-fail/30">
        <CardContent>
          <p className="text-sm text-ds-status-fail">Failed to load subscription analytics</p>
        </CardContent>
      </Card>
    );
  }

  // Every figure here is derived from the subscription records; a figure no
  // record supports is null and says it is unavailable, never a stand-in.
  const { summary, mrrTrend, churnAnalysis, planDistribution, upcomingRenewals } = data;
  const growth = summary?.mrrGrowth ?? null;
  const days = summary?.activityDays ?? null;

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className="space-y-6"
    >
      {summary?.complete === false && (
        <p className="text-sm text-ds-status-warn">
          These figures stop short of every subscription: the scan reached its record limit.
        </p>
      )}

      {/* Key Metrics */}
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
        <MetricCard
          label="Monthly Recurring Revenue"
          value={formatMeasured(summary?.mrrExact, money)}
          subtitle={
            growth === null
              ? 'No MRR history to compare'
              : `${growth > 0 ? '+' : growth < 0 ? '-' : ''}${formatRatio(Math.abs(growth))} vs last month`
          }
          tone="success"
        />
        <MetricCard
          label="Active Subscriptions"
          value={formatMeasured(summary?.activeCount, formatNumber)}
          tone="primary"
        />
        <MetricCard
          label="Churn Rate"
          value={formatMeasured(summary?.churnRate, (rate) => formatRatio(rate))}
          subtitle="Cancelled or expired, all time"
          tone="warning"
        />
        <MetricCard
          label="Avg Revenue/User"
          value={formatMeasured(summary?.arpuExact, money)}
          tone="accent"
        />
      </div>

      {/* MRR Trend */}
      <Card>
        <CardHeader>
          <CardTitle>MRR Growth Trend</CardTitle>
          <CardDescription>Monthly recurring revenue over time</CardDescription>
        </CardHeader>
        <CardContent>
          {mrrTrend && mrrTrend.length > 0 ? (
            <AreaChart
              className="h-72"
              data={mrrTrend}
              index="month"
              categories={['mrr', 'newMrr', 'churnedMrr']}
              colors={['emerald', 'indigo', 'rose']}
              showAnimation
              curveType="monotone"
              valueFormatter={(value) => formatCurrency(value)}
            />
          ) : (
            <Unavailable className="h-72">
              MRR history is not available: the engine keeps current subscriptions, not past MRR.
            </Unavailable>
          )}
        </CardContent>
      </Card>

      {/* Charts Row */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Plan Distribution */}
        <Card>
          <CardHeader>
            <CardTitle>Plan Distribution</CardTitle>
            <CardDescription>Active subscribers by plan</CardDescription>
          </CardHeader>
          <CardContent>
            {planDistribution && planDistribution.length > 0 ? (
              <>
                <DonutChart
                  className="h-64"
                  data={planDistribution}
                  category="count"
                  index="plan"
                  colors={['indigo', 'emerald', 'violet', 'amber']}
                  showAnimation
                  valueFormatter={(value) => `${value} subscribers`}
                />
                <div className="mt-4 space-y-2">
                  {planDistribution.map((entry) => (
                    <div key={entry.plan} className="flex justify-between text-sm">
                      <p className="text-ds-muted-foreground">
                        {entry.plan} · {formatNumber(entry.count)} active
                      </p>
                      <p className="font-medium text-ds-foreground">
                        {formatMoney(entry.mrrExact)} MRR
                      </p>
                    </div>
                  ))}
                </div>
              </>
            ) : (
              <Unavailable className="h-64">
                {planDistribution ? 'No active subscriptions' : 'Plan distribution unavailable'}
              </Unavailable>
            )}
          </CardContent>
        </Card>

        {/* Churn Analysis */}
        <Card>
          <CardHeader>
            <CardTitle>Churn Analysis</CardTitle>
            <CardDescription>Cancellation reasons</CardDescription>
          </CardHeader>
          <CardContent>
            {churnAnalysis && churnAnalysis.reasons.length > 0 ? (
              <div className="space-y-4">
                {churnAnalysis.reasons.map((reason: ChurnReason) => (
                  <div key={reason.name}>
                    <div className="flex justify-between mb-1">
                      <p className="text-sm font-medium text-ds-foreground">{reason.name}</p>
                      <div className="flex items-center space-x-2">
                        <Badge variant="default">{reason.count}</Badge>
                        <p className="text-sm text-ds-muted-foreground">
                          {formatRatio(reason.percentage)}
                        </p>
                      </div>
                    </div>
                    <ProgressBar value={reason.percentage * 100} color="rose" />
                  </div>
                ))}
              </div>
            ) : (
              <Unavailable className="h-64">
                Cancellation reasons are not available: they are not on subscription records.
              </Unavailable>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Subscription Status Breakdown */}
      <Card>
        <CardHeader>
          <CardTitle>Subscription Status</CardTitle>
          <CardDescription>Current status distribution</CardDescription>
        </CardHeader>
        <CardContent>
          {summary?.statusBreakdown && Object.keys(summary.statusBreakdown).length > 0 ? (
            <div className="grid grid-cols-2 sm:grid-cols-5 gap-4">
              {Object.entries(summary.statusBreakdown).map(([status, count]) => (
                <div
                  key={status}
                  className="text-center p-4 border border-ds-enterprise-line rounded-lg"
                >
                  <StatusPill status={statusPills[status as SubscriptionStatus] || 'idle'}>
                    {status.replace('_', ' ')}
                  </StatusPill>
                  <p className="ds-instrument-number text-3xl text-ds-foreground mt-2">{count}</p>
                </div>
              ))}
            </div>
          ) : (
            <Unavailable>
              {summary?.statusBreakdown ? 'No subscriptions' : 'Status breakdown unavailable'}
            </Unavailable>
          )}
        </CardContent>
      </Card>

      {/* Upcoming Renewals */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <div>
              <CardTitle>Upcoming Renewals</CardTitle>
              <CardDescription>
                {upcomingRenewals
                  ? `Active subscriptions renewing in the next ${upcomingRenewals.days} days`
                  : 'Active subscriptions renewing soon'}
              </CardDescription>
            </div>
            {upcomingRenewals && (
              <Badge variant="primary">{formatMoney(upcomingRenewals.amountExact)} expected</Badge>
            )}
          </div>
        </CardHeader>
        <CardContent>
          {!upcomingRenewals ? (
            <Unavailable>Renewal schedule unavailable</Unavailable>
          ) : upcomingRenewals.next.length === 0 ? (
            <Unavailable>{`No renewals in the next ${upcomingRenewals.days} days`}</Unavailable>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="border-b border-ds-enterprise-line">
                    <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">
                      Customer
                    </th>
                    <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">
                      Plan
                    </th>
                    <th className="text-left py-2 px-3 text-sm font-medium text-ds-muted-foreground">
                      Renewal Date
                    </th>
                    <th className="text-right py-2 px-3 text-sm font-medium text-ds-muted-foreground">
                      Amount
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {upcomingRenewals.next.map((renewal: UpcomingRenewal, index: number) => (
                    <motion.tr
                      key={renewal.id}
                      initial={{ opacity: 0, x: -20 }}
                      animate={{ opacity: 1, x: 0 }}
                      transition={{ delay: index * 0.05 }}
                      className="border-b border-ds-enterprise-line hover:bg-ds-muted"
                    >
                      <td className="py-3 px-3">
                        <div>
                          <p className="text-sm font-medium text-ds-foreground">
                            {renewal.customerName}
                          </p>
                          {renewal.email && (
                            <p className="text-xs text-ds-muted-foreground">{renewal.email}</p>
                          )}
                        </div>
                      </td>
                      <td className="py-3 px-3">
                        <Badge variant="primary">{renewal.plan}</Badge>
                      </td>
                      <td className="py-3 px-3">
                        <div className="flex items-center space-x-2">
                          <CalendarIcon className="w-4 h-4 text-ds-muted-foreground" />
                          <p className="text-sm text-ds-foreground">{renewal.renewalDate}</p>
                        </div>
                      </td>
                      <td className="py-3 px-3 text-right">
                        <p className="text-sm font-medium text-ds-foreground">
                          {formatMoney(renewal.amountExact)}
                        </p>
                      </td>
                    </motion.tr>
                  ))}
                </tbody>
              </table>
              {upcomingRenewals.count > upcomingRenewals.next.length && (
                <p className="mt-2 text-xs text-ds-muted-foreground">
                  The next {upcomingRenewals.next.length} of {upcomingRenewals.count} renewals
                </p>
              )}
            </div>
          )}
        </CardContent>
      </Card>

      {/* Revenue Metrics */}
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
        <Card>
          <CardContent className="p-5">
            <div className="flex items-center space-x-2">
              <ArrowTrendingUpIcon className="w-5 h-5 text-ds-status-ok" />
              <p className="text-sm font-medium text-ds-foreground">New MRR</p>
            </div>
            <p className="ds-instrument-number text-3xl text-ds-foreground mt-2">
              {formatMeasured(summary?.newMrrExact, money)}
            </p>
            <p className="text-xs text-ds-muted-foreground">
              {summary?.newSubscribers == null || days === null
                ? 'New subscriptions unavailable'
                : `From ${formatNumber(summary.newSubscribers)} new subscriptions in the last ${days} days`}
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-5">
            <div className="flex items-center space-x-2">
              <CreditCardIcon className="w-5 h-5 text-ds-primary" />
              <p className="text-sm font-medium text-ds-foreground">Expansion MRR</p>
            </div>
            <p className="ds-instrument-number text-3xl text-ds-foreground mt-2">
              {formatMeasured(summary?.expansionMrrExact, money)}
            </p>
            <p className="text-xs text-ds-muted-foreground">
              {summary?.upgrades == null
                ? 'Plan changes are not recorded'
                : `From ${formatNumber(summary.upgrades)} upgrades`}
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-5">
            <div className="flex items-center space-x-2">
              <UserMinusIcon className="w-5 h-5 text-ds-status-fail" />
              <p className="text-sm font-medium text-ds-foreground">Churned MRR</p>
            </div>
            <p className="ds-instrument-number text-3xl text-ds-foreground mt-2">
              {formatMeasured(summary?.churnedMrrExact, money)}
            </p>
            <p className="text-xs text-ds-muted-foreground">
              {summary?.cancellations == null || days === null
                ? 'Cancellation dates unavailable'
                : `From ${formatNumber(summary.cancellations)} cancellations in the last ${days} days`}
            </p>
          </CardContent>
        </Card>
      </div>
    </motion.div>
  );
}

const SubscriptionAnalytics = memo(SubscriptionAnalyticsInner);
export default SubscriptionAnalytics;

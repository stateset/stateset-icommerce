'use client';

import { memo } from 'react';
import { BarChart, DonutChart, ProgressBar } from '@tremor/react';
import {
  Badge,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
  Separator,
} from '@stateset/design';
import {
  CurrencyDollarIcon,
  ClockIcon,
  ArrowTrendingUpIcon,
  CheckCircleIcon,
} from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { formatCurrency, formatNumber, formatPercentage } from '@/lib/utils';
import type {
  ROICalculatorData,
  CostCategory,
  SavingsCategory,
  ROIMilestone,
} from '@/lib/types/dashboard-data';

interface ROICalculatorProps {
  data?: ROICalculatorData;
}

function ROICalculatorInner({ data: propData }: ROICalculatorProps) {
  // Demo data - in production this would come from embedded API
  const data = propData || generateDemoData();

  const { summary, costBreakdown, savingsProjection, paybackAnalysis } = data;

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className="space-y-6"
    >
      {/* Key ROI Metrics */}
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
        <Card className="border-t-2 border-t-ds-status-ok">
          <CardContent>
            <p className="text-sm text-ds-muted-foreground">Annual Savings</p>
            <p className="ds-instrument-number text-3xl text-ds-foreground">
              {formatCurrency(summary.annualSavings)}
            </p>
            <p className="text-xs text-ds-status-ok mt-1">
              +{formatPercentage(summary.savingsGrowth)} vs last year
            </p>
          </CardContent>
        </Card>
        <Card className="border-t-2 border-t-ds-status-run">
          <CardContent>
            <p className="text-sm text-ds-muted-foreground">ROI</p>
            <p className="ds-instrument-number text-3xl text-ds-foreground">{summary.roi}%</p>
            <p className="text-xs text-ds-status-run mt-1">
              Payback: {summary.paybackMonths} months
            </p>
          </CardContent>
        </Card>
        <Card className="border-t-2 border-t-ds-brand-500">
          <CardContent>
            <p className="text-sm text-ds-muted-foreground">Hours Saved</p>
            <p className="ds-instrument-number text-3xl text-ds-foreground">
              {formatNumber(summary.hoursSaved)}
            </p>
            <p className="text-xs text-ds-brand-600 mt-1">Per month</p>
          </CardContent>
        </Card>
        <Card className="border-t-2 border-t-ds-status-warn">
          <CardContent>
            <p className="text-sm text-ds-muted-foreground">Cost per Transaction</p>
            <p className="ds-instrument-number text-3xl text-ds-foreground">
              {formatCurrency(summary.costPerTransaction)}
            </p>
            <p className="text-xs text-ds-status-warn mt-1">
              -{formatPercentage(summary.costReduction)} reduction
            </p>
          </CardContent>
        </Card>
      </div>

      {/* Cost Breakdown */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <Card>
          <CardHeader>
            <CardTitle>Cost Breakdown</CardTitle>
            <CardDescription>Where your operational costs go</CardDescription>
          </CardHeader>
          <CardContent>
            <DonutChart
              className="h-64"
              data={costBreakdown.categories}
              category="value"
              index="name"
              colors={['indigo', 'emerald', 'amber', 'violet', 'cyan']}
              showAnimation
              valueFormatter={(value) => formatCurrency(value)}
            />
            <div className="mt-4 space-y-2">
              {costBreakdown.categories.map((category: CostCategory, index: number) => (
                <div key={index} className="flex justify-between items-center">
                  <p className="text-sm text-ds-muted-foreground">{category.name}</p>
                  <div className="flex items-center space-x-2">
                    <p className="text-sm font-medium text-ds-foreground">
                      {formatCurrency(category.value)}
                    </p>
                    {category.trend < 0 && <Badge variant="success">{category.trend}%</Badge>}
                  </div>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Savings by Category</CardTitle>
            <CardDescription>Where automation saves you money</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              {costBreakdown.savingsByCategory.map((category: SavingsCategory, index: number) => (
                <div key={index}>
                  <div className="flex justify-between mb-1">
                    <p className="text-sm font-medium text-ds-foreground">{category.name}</p>
                    <div className="flex items-center space-x-2">
                      <p className="text-sm font-medium text-ds-status-ok">
                        {formatCurrency(category.saved)}
                      </p>
                      <p className="text-sm text-ds-muted-foreground">
                        / {formatCurrency(category.previous)}
                      </p>
                    </div>
                  </div>
                  <ProgressBar value={(category.saved / category.previous) * 100} color="emerald" />
                  <p className="text-xs text-ds-muted-foreground mt-1">
                    {formatPercentage(category.saved / category.previous)} reduction
                  </p>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Savings Projection */}
      <Card>
        <CardHeader>
          <CardTitle>Savings Projection</CardTitle>
          <CardDescription>Projected savings over the next 12 months</CardDescription>
        </CardHeader>
        <CardContent>
          <BarChart
            className="h-72"
            data={savingsProjection.monthly}
            index="month"
            categories={['currentCost', 'projectedCost', 'savings']}
            colors={['red', 'indigo', 'emerald']}
            showAnimation
            valueFormatter={(value) => formatCurrency(value)}
          />
        </CardContent>
      </Card>

      {/* Payback Analysis */}
      <Card>
        <CardContent>
          <div className="flex items-center justify-between mb-4">
            <div>
              <h3 className="font-ds-display text-base font-semibold text-ds-foreground">
                Payback Analysis
              </h3>
              <p className="text-sm text-ds-muted-foreground">Investment recovery timeline</p>
            </div>
            <Badge variant="success">{summary.paybackMonths} month payback</Badge>
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-3 gap-4 mb-6">
            <div className="p-4 bg-ds-muted rounded-lg">
              <div className="flex items-center space-x-2 mb-2">
                <CurrencyDollarIcon className="w-5 h-5 text-ds-status-run" />
                <p className="text-sm font-medium text-ds-foreground">Initial Investment</p>
              </div>
              <p className="ds-instrument-number text-3xl text-ds-status-run">
                {formatCurrency(paybackAnalysis.initialInvestment)}
              </p>
            </div>
            <div className="p-4 bg-ds-muted rounded-lg">
              <div className="flex items-center space-x-2 mb-2">
                <ClockIcon className="w-5 h-5 text-ds-status-warn" />
                <p className="text-sm font-medium text-ds-foreground">Monthly Cost</p>
              </div>
              <p className="ds-instrument-number text-3xl text-ds-status-warn">
                {formatCurrency(paybackAnalysis.monthlyCost)}
              </p>
            </div>
            <div className="p-4 bg-ds-status-ok/10 rounded-lg">
              <div className="flex items-center space-x-2 mb-2">
                <ArrowTrendingUpIcon className="w-5 h-5 text-ds-status-ok" />
                <p className="text-sm font-medium text-ds-foreground">Monthly Savings</p>
              </div>
              <p className="ds-instrument-number text-3xl text-ds-status-ok">
                {formatCurrency(paybackAnalysis.monthlySavings)}
              </p>
            </div>
          </div>

          {/* ROI Milestones */}
          <Separator />
          <div className="pt-4">
            <p className="text-sm font-medium text-ds-foreground mb-3">ROI Milestones</p>
            <div className="flex items-center justify-between">
              {paybackAnalysis.milestones.map((milestone: ROIMilestone, index: number) => (
                <div key={index} className="flex items-center space-x-3">
                  <div
                    className={`w-8 h-8 rounded-full flex items-center justify-center ${
                      milestone.achieved ? 'bg-ds-status-ok/15' : 'bg-ds-muted'
                    }`}
                  >
                    {milestone.achieved ? (
                      <CheckCircleIcon className="w-5 h-5 text-ds-status-ok" />
                    ) : (
                      <p className="text-sm font-medium text-ds-muted-foreground">{index + 1}</p>
                    )}
                  </div>
                  <div>
                    <p className="text-sm font-medium text-ds-foreground">{milestone.name}</p>
                    <p className="text-xs text-ds-muted-foreground">{milestone.timeline}</p>
                  </div>
                  {index < paybackAnalysis.milestones.length - 1 && (
                    <div className="flex-1 h-0.5 bg-ds-enterprise-line mx-4" />
                  )}
                </div>
              ))}
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Value Summary */}
      <Card className="bg-gradient-to-r from-ds-status-ok/10 to-ds-status-run/10">
        <CardContent>
          <div className="flex items-center justify-between">
            <div>
              <h3 className="font-ds-display text-base font-semibold text-ds-foreground">
                Total Value Generated
              </h3>
              <p className="text-sm text-ds-muted-foreground">Since implementation</p>
            </div>
            <div className="text-right">
              <p className="ds-instrument-number text-3xl text-ds-status-ok">
                {formatCurrency(summary.totalValueGenerated)}
              </p>
              <p className="text-sm text-ds-muted-foreground">across all optimizations</p>
            </div>
          </div>
        </CardContent>
      </Card>
    </motion.div>
  );
}

function generateDemoData() {
  return {
    summary: {
      annualSavings: 145000,
      savingsGrowth: 0.23,
      roi: 380,
      paybackMonths: 4,
      hoursSaved: 320,
      costPerTransaction: 0.12,
      costReduction: 0.45,
      totalValueGenerated: 287500,
    },
    costBreakdown: {
      categories: [
        { name: 'Labor', value: 45000, trend: -15 },
        { name: 'Software', value: 12000, trend: 5 },
        { name: 'Infrastructure', value: 8500, trend: -8 },
        { name: 'Processing Fees', value: 28000, trend: -12 },
        { name: 'Customer Support', value: 15000, trend: -25 },
      ],
      savingsByCategory: [
        { name: 'Order Processing', saved: 32000, previous: 65000 },
        { name: 'Inventory Management', saved: 18000, previous: 35000 },
        { name: 'Customer Service', saved: 25000, previous: 50000 },
        { name: 'Returns Handling', saved: 12000, previous: 22000 },
        { name: 'Reporting & Analytics', saved: 8000, previous: 18000 },
      ],
    },
    savingsProjection: {
      monthly: [
        { month: 'Jan', currentCost: 25000, projectedCost: 18000, savings: 7000 },
        { month: 'Feb', currentCost: 26000, projectedCost: 17500, savings: 8500 },
        { month: 'Mar', currentCost: 24500, projectedCost: 16800, savings: 7700 },
        { month: 'Apr', currentCost: 27000, projectedCost: 17200, savings: 9800 },
        { month: 'May', currentCost: 25500, projectedCost: 16500, savings: 9000 },
        { month: 'Jun', currentCost: 28000, projectedCost: 17000, savings: 11000 },
        { month: 'Jul', currentCost: 26500, projectedCost: 15800, savings: 10700 },
        { month: 'Aug', currentCost: 27500, projectedCost: 15500, savings: 12000 },
        { month: 'Sep', currentCost: 29000, projectedCost: 16000, savings: 13000 },
        { month: 'Oct', currentCost: 28500, projectedCost: 15200, savings: 13300 },
        { month: 'Nov', currentCost: 31000, projectedCost: 16500, savings: 14500 },
        { month: 'Dec', currentCost: 35000, projectedCost: 18000, savings: 17000 },
      ],
    },
    paybackAnalysis: {
      initialInvestment: 35000,
      monthlyCost: 2500,
      monthlySavings: 12000,
      milestones: [
        { name: 'Break Even', timeline: 'Month 4', achieved: true },
        { name: '100% ROI', timeline: 'Month 7', achieved: true },
        { name: '200% ROI', timeline: 'Month 10', achieved: false },
        { name: '300% ROI', timeline: 'Month 12', achieved: false },
      ],
    },
  };
}

const ROICalculator = memo(ROICalculatorInner);
export default ROICalculator;

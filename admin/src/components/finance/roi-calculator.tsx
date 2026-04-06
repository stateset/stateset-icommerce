'use client';

import { memo } from 'react';
import { Card, Title, Text, Badge, Grid, Metric, BarChart, DonutChart, ProgressBar } from '@tremor/react';
import { CurrencyDollarIcon, ClockIcon, ArrowTrendingUpIcon, CheckCircleIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { formatCurrency, formatNumber, formatPercentage } from '@/lib/utils';
import type { ROICalculatorData, CostCategory, SavingsCategory, ROIMilestone } from '@/lib/types/dashboard-data';

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
      <Grid numItems={2} numItemsSm={4} className="gap-4">
        <Card decoration="top" decorationColor="emerald">
          <Text>Annual Savings</Text>
          <Metric>{formatCurrency(summary.annualSavings)}</Metric>
          <Text className="text-xs text-emerald-600 mt-1">
            +{formatPercentage(summary.savingsGrowth)} vs last year
          </Text>
        </Card>
        <Card decoration="top" decorationColor="blue">
          <Text>ROI</Text>
          <Metric>{summary.roi}%</Metric>
          <Text className="text-xs text-blue-600 mt-1">
            Payback: {summary.paybackMonths} months
          </Text>
        </Card>
        <Card decoration="top" decorationColor="purple">
          <Text>Hours Saved</Text>
          <Metric>{formatNumber(summary.hoursSaved)}</Metric>
          <Text className="text-xs text-purple-600 mt-1">
            Per month
          </Text>
        </Card>
        <Card decoration="top" decorationColor="amber">
          <Text>Cost per Transaction</Text>
          <Metric>{formatCurrency(summary.costPerTransaction)}</Metric>
          <Text className="text-xs text-amber-600 mt-1">
            -{formatPercentage(summary.costReduction)} reduction
          </Text>
        </Card>
      </Grid>

      {/* Cost Breakdown */}
      <Grid numItems={1} numItemsLg={2} className="gap-6">
        <Card>
          <Title>Cost Breakdown</Title>
          <Text className="text-gray-500 mb-4">Where your operational costs go</Text>
          <DonutChart
            className="h-64"
            data={costBreakdown.categories}
            category="value"
            index="name"
            colors={['blue', 'emerald', 'amber', 'purple', 'red']}
            showAnimation
            valueFormatter={(value) => formatCurrency(value)}
          />
          <div className="mt-4 space-y-2">
            {costBreakdown.categories.map((category: CostCategory, index: number) => (
              <div key={index} className="flex justify-between items-center">
                <Text className="text-sm">{category.name}</Text>
                <div className="flex items-center space-x-2">
                  <Text className="text-sm font-medium">{formatCurrency(category.value)}</Text>
                  {category.trend < 0 && (
                    <Badge color="emerald" size="xs">
                      {category.trend}%
                    </Badge>
                  )}
                </div>
              </div>
            ))}
          </div>
        </Card>

        <Card>
          <Title>Savings by Category</Title>
          <Text className="text-gray-500 mb-4">Where automation saves you money</Text>
          <div className="space-y-4">
            {costBreakdown.savingsByCategory.map((category: SavingsCategory, index: number) => (
              <div key={index}>
                <div className="flex justify-between mb-1">
                  <Text className="font-medium">{category.name}</Text>
                  <div className="flex items-center space-x-2">
                    <Text className="text-emerald-600 font-medium">
                      {formatCurrency(category.saved)}
                    </Text>
                    <Text className="text-sm text-gray-500">
                      / {formatCurrency(category.previous)}
                    </Text>
                  </div>
                </div>
                <ProgressBar
                  value={(category.saved / category.previous) * 100}
                  color="emerald"
                />
                <Text className="text-xs text-gray-500 mt-1">
                  {formatPercentage(category.saved / category.previous)} reduction
                </Text>
              </div>
            ))}
          </div>
        </Card>
      </Grid>

      {/* Savings Projection */}
      <Card>
        <Title>Savings Projection</Title>
        <Text className="text-gray-500 mb-4">Projected savings over the next 12 months</Text>
        <BarChart
          className="h-72"
          data={savingsProjection.monthly}
          index="month"
          categories={['currentCost', 'projectedCost', 'savings']}
          colors={['red', 'blue', 'emerald']}
          showAnimation
          valueFormatter={(value) => formatCurrency(value)}
        />
      </Card>

      {/* Payback Analysis */}
      <Card>
        <div className="flex items-center justify-between mb-4">
          <div>
            <Title>Payback Analysis</Title>
            <Text className="text-gray-500">Investment recovery timeline</Text>
          </div>
          <Badge color="emerald" size="lg">
            {summary.paybackMonths} month payback
          </Badge>
        </div>

        <Grid numItems={1} numItemsSm={3} className="gap-4 mb-6">
          <div className="p-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
            <div className="flex items-center space-x-2 mb-2">
              <CurrencyDollarIcon className="w-5 h-5 text-blue-600" />
              <Text className="font-medium">Initial Investment</Text>
            </div>
            <Metric className="text-blue-600">{formatCurrency(paybackAnalysis.initialInvestment)}</Metric>
          </div>
          <div className="p-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
            <div className="flex items-center space-x-2 mb-2">
              <ClockIcon className="w-5 h-5 text-amber-600" />
              <Text className="font-medium">Monthly Cost</Text>
            </div>
            <Metric className="text-amber-600">{formatCurrency(paybackAnalysis.monthlyCost)}</Metric>
          </div>
          <div className="p-4 bg-emerald-50 dark:bg-emerald-900/20 rounded-lg">
            <div className="flex items-center space-x-2 mb-2">
              <ArrowTrendingUpIcon className="w-5 h-5 text-emerald-600" />
              <Text className="font-medium">Monthly Savings</Text>
            </div>
            <Metric className="text-emerald-600">{formatCurrency(paybackAnalysis.monthlySavings)}</Metric>
          </div>
        </Grid>

        {/* ROI Milestones */}
        <div className="border-t dark:border-gray-700 pt-4">
          <Text className="font-medium mb-3">ROI Milestones</Text>
          <div className="flex items-center justify-between">
            {paybackAnalysis.milestones.map((milestone: ROIMilestone, index: number) => (
              <div key={index} className="flex items-center space-x-3">
                <div className={`w-8 h-8 rounded-full flex items-center justify-center ${
                  milestone.achieved ? 'bg-emerald-100 dark:bg-emerald-900/30' : 'bg-gray-100 dark:bg-gray-800'
                }`}>
                  {milestone.achieved ? (
                    <CheckCircleIcon className="w-5 h-5 text-emerald-600" />
                  ) : (
                    <Text className="text-sm font-medium text-gray-500">{index + 1}</Text>
                  )}
                </div>
                <div>
                  <Text className="text-sm font-medium">{milestone.name}</Text>
                  <Text className="text-xs text-gray-500">{milestone.timeline}</Text>
                </div>
                {index < paybackAnalysis.milestones.length - 1 && (
                  <div className="flex-1 h-0.5 bg-gray-200 dark:bg-gray-700 mx-4" />
                )}
              </div>
            ))}
          </div>
        </div>
      </Card>

      {/* Value Summary */}
      <Card className="bg-gradient-to-r from-emerald-50 to-blue-50 dark:from-emerald-900/20 dark:to-blue-900/20">
        <div className="flex items-center justify-between">
          <div>
            <Title>Total Value Generated</Title>
            <Text className="text-gray-600">Since implementation</Text>
          </div>
          <div className="text-right">
            <Metric className="text-emerald-600">{formatCurrency(summary.totalValueGenerated)}</Metric>
            <Text className="text-sm text-gray-500">across all optimizations</Text>
          </div>
        </div>
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

/**
 * Dashboard Component Data Interfaces
 *
 * Typed interfaces for all 13 dashboard components' data shapes.
 * These replace `data?: any` props with concrete types based on
 * what each component actually accesses in its JSX.
 */

import type { Order, InventoryItem, Return } from '@/lib/types';

// ============================================================================
// Shared / Reusable Types
// ============================================================================

/** Tremor color string used for Badge/ProgressBar color props */
export type TremorColor =
  | 'amber'
  | 'blue'
  | 'emerald'
  | 'red'
  | 'indigo'
  | 'purple'
  | 'pink'
  | 'gray'
  | 'slate'
  | 'zinc'
  | 'neutral'
  | 'stone'
  | 'orange'
  | 'yellow'
  | 'lime'
  | 'green'
  | 'teal'
  | 'cyan'
  | 'sky'
  | 'violet'
  | 'fuchsia'
  | 'rose';

// ============================================================================
// 1. Order Pipeline
// ============================================================================

export interface OrderPipelineSummary {
  totalOrders: number;
  totalValue: number;
  averageOrderValue: number;
  deliveredRate: number;
  inProgressCount: number;
  exceptionsCount: number;
}

export interface OrderPipelineStatusGroup {
  key: string;
  label: string;
  count: number;
  totalValue: number;
  orders?: Pick<Order, 'id' | 'totalAmount'>[];
}

export interface OrderPipelineTimelineEntry {
  date: string;
  count: number;
  revenue: number;
}

export interface OrderPipelineData {
  summary: OrderPipelineSummary;
  statusGroups: OrderPipelineStatusGroup[];
  timeline?: OrderPipelineTimelineEntry[];
}

// ============================================================================
// 2. Inventory Analytics
// ============================================================================

export interface InventoryCategory {
  name: string;
  units: number;
  value: number;
  items?: number;
}

export interface TopMovingItem {
  sku: string;
  name: string;
  velocity: number;
}

export interface SlowMovingItem {
  sku: string;
  name: string;
  daysSinceLastSale: number;
}

export interface InventoryAnalyticsData {
  totalSKUs: number;
  totalUnits: number;
  totalValue: number;
  lowStockItems: number;
  outOfStockItems: number;
  turnoverRate?: number;
  categories?: InventoryCategory[];
  topMovingItems?: TopMovingItem[];
  slowMovingItems?: SlowMovingItem[];
  criticalItems?: InventoryItem[];
}

// ============================================================================
// 3. Demand Forecasting
// ============================================================================

export interface ForecastTimelineEntry {
  date: string;
  predicted: number;
  actual: number | null;
  lowerBound: number;
  upperBound: number;
}

export interface CategoryDemandEntry {
  category: string;
  current: number;
  predicted: number;
}

export interface DemandForecast {
  predictedRevenue: number;
  trendScore: number;
  timeline: ForecastTimelineEntry[];
  categoryDemand: CategoryDemandEntry[];
}

export interface DemandHighProduct {
  id: string;
  name: string;
  sku: string;
  growthRate: number;
  predictedUnits: number;
}

export interface DemandAlert {
  productId: string;
  productName: string;
  reason: string;
  daysUntilStockout: number;
  recommendedQuantity: number;
}

export interface DemandForecastingData {
  forecast: DemandForecast;
  topProducts: {
    highDemand: DemandHighProduct[];
  };
  alerts: DemandAlert[];
  accuracy: {
    overall: number;
  };
}

// ============================================================================
// 4. Customer Health Score
// ============================================================================

export interface CustomerHealthMetric {
  name: string;
  score: number;
}

export interface AtRiskCustomer {
  id: string;
  name: string;
  email: string;
  healthScore: number;
  riskReason: string;
  lifetimeValue: number;
  daysSinceLastOrder: number;
}

export interface CustomerHealthTrendEntry {
  month: string;
  excellent: number;
  good: number;
  fair: number;
  atRisk: number;
}

export interface CustomerHealthSummary {
  overallScore: number;
  totalCustomers: number;
  atRiskCount: number;
  avgLifetimeValue: number;
  metrics?: CustomerHealthMetric[];
}

export interface CustomerSegmentDetail {
  count: number;
  avgLtv: number;
}

export interface CustomerHealthData {
  summary: CustomerHealthSummary;
  segments: Record<string, number | CustomerSegmentDetail>;
  atRiskCustomers?: AtRiskCustomer[];
  trends?: {
    timeline: CustomerHealthTrendEntry[];
  };
}

// ============================================================================
// 5. Returns Management
// ============================================================================

export interface ReturnsPipelineStage {
  stage: string;
  count: number;
}

export interface ReturnsAnalyticsSummary {
  totalReturns: number;
  returnRate: number;
  refundTotal: number;
  averageProcessingTime: number;
  returnsByReason: Record<string, number>;
  returnsByStatus: Record<string, number>;
  topReturnedProducts?: Array<{
    productId: string;
    name: string;
    count: number;
    rate: number;
  }>;
}

export interface ReturnsManagementData {
  returns: Return[];
  analytics: ReturnsAnalyticsSummary;
  pipeline: ReturnsPipelineStage[];
}

// ============================================================================
// 6. Subscription Analytics
// ============================================================================

export interface SubscriptionSummary {
  mrr: number;
  mrrGrowth: number;
  activeCount: number;
  churnRate: number;
  arpu: number;
  statusBreakdown?: Record<string, number>;
  newMrr?: number;
  expansionMrr?: number;
  churnedMrr?: number;
  newSubscribers?: number;
  upgrades?: number;
  cancelations?: number;
}

export interface MrrTrendEntry {
  month: string;
  mrr: number;
  newMrr: number;
  churnedMrr: number;
}

export interface ChurnReason {
  name: string;
  count: number;
  percentage: number;
}

export interface PlanDistributionEntry {
  plan: string;
  count: number;
  revenue: number;
}

export interface UpcomingRenewal {
  id: string;
  customerName: string;
  email: string;
  plan: string;
  renewalDate: string;
  amount: number;
  churnRisk: number;
}

export interface SubscriptionAnalyticsData {
  summary: SubscriptionSummary;
  mrrTrend?: MrrTrendEntry[];
  churnAnalysis?: {
    reasons: ChurnReason[];
  };
  planDistribution?: PlanDistributionEntry[];
  upcomingRenewals?: UpcomingRenewal[];
}

// ============================================================================
// 7. Exception Management
// ============================================================================

export interface ExceptionSummary {
  openCount: number;
  criticalCount: number;
  investigatingCount: number;
  resolvedToday: number;
  autoResolvedPercent: number;
  bySeverity?: Record<string, number>;
}

export interface ExceptionItem {
  id: string;
  title: string;
  description: string;
  severity: 'critical' | 'high' | 'medium' | 'low';
  status: 'open' | 'investigating' | 'resolved' | 'dismissed';
  category: string;
  timestamp: string;
  suggestedAction?: boolean;
}

export interface ExceptionResolution {
  id: string;
  title: string;
  resolution: string;
  method: 'auto' | 'manual';
  timeToResolve: string;
}

export interface ExceptionManagementData {
  summary: ExceptionSummary;
  exceptions: ExceptionItem[];
  recentResolutions: ExceptionResolution[];
}

// ============================================================================
// 8. Workflow Builder
// ============================================================================

export interface WorkflowExecution {
  workflow: string;
  trigger: string;
  status: 'success' | 'failed';
  duration: string;
  time: string;
}

export interface WorkflowSummary {
  activeCount: number;
  executionsToday: number;
  successRate: number;
  hoursSaved: number;
  recentExecutions?: WorkflowExecution[];
}

export interface Workflow {
  id: string;
  name: string;
  description: string;
  status: 'active' | 'paused' | 'draft' | 'error';
  trigger: string;
  steps: string[];
  executions: number;
  successRate: number;
  lastRun: string;
}

export interface WorkflowTemplate {
  id: string;
  name: string;
  description: string;
  category: string;
  usedBy: number;
}

export interface WorkflowBuilderData {
  summary: WorkflowSummary;
  workflows: Workflow[];
  templates: WorkflowTemplate[];
}

// ============================================================================
// 9. System Health
// ============================================================================

export interface SystemHealthSummary {
  overallStatus: 'healthy' | 'degraded' | 'critical';
  uptime: number;
  healthyServices: number;
  totalServices: number;
}

export interface SystemService {
  name: string;
  status: 'healthy' | 'degraded' | 'critical' | 'unknown';
  latency: number;
  successRate: number;
}

export interface PerformanceTimelineEntry {
  time: string;
  cpu: number;
  memory: number;
  latency: number;
}

export interface SystemPerformance {
  cpuUsage: number;
  memoryUsage: number;
  requestsPerSecond: number;
  timeline: PerformanceTimelineEntry[];
}

export interface DatabaseHealth {
  latency: number;
  connections: number;
  maxConnections: number;
  avgQueryTime: number;
  queriesPerSecond: number;
  size: string;
}

export interface VectorSearchHealth {
  model: string;
  dimensions: number;
  counts: {
    products: number;
    customers: number;
    orders: number;
    inventory: number;
  };
  total: number;
}

export interface SystemEvent {
  type: 'success' | 'warning' | 'error' | 'info';
  message: string;
  service: string;
  timestamp: string;
}

export interface SystemHealthData {
  summary: SystemHealthSummary;
  services: SystemService[];
  performance: SystemPerformance;
  database: DatabaseHealth;
  vectorSearch?: VectorSearchHealth;
  recentEvents: SystemEvent[];
}

// ============================================================================
// 10. Product Catalog
// ============================================================================

export interface ProductCatalogSummary {
  totalProducts: number;
  activeProducts: number;
  lowStockProducts: number;
  avgPrice: number;
}

export interface ProductCategoryDistribution {
  category: string;
  count: number;
  inventoryValue: number;
}

export interface TopProduct {
  id: string;
  name: string;
  sku: string;
  category: string;
  price: number;
  inventory?: number;
  unitsSold: number;
  revenue: number;
  status: string;
  compareAtPrice?: number;
  imageUrl?: string;
  categories?: string[];
}

export interface ProductCatalogData {
  summary: ProductCatalogSummary;
  categoryDistribution: ProductCategoryDistribution[];
  topProducts: TopProduct[];
  products?: TopProduct[];
}

// ============================================================================
// 11. Agent Performance
// ============================================================================

export interface AgentPerformanceSummary {
  activeAgents: number;
  onlinePercentage: number;
  tasksCompleted: number;
  avgResponseTime: number;
  successRate: number;
}

export interface Agent {
  id: string;
  name: string;
  status: 'online' | 'busy' | 'offline' | 'error';
  tasksCompleted: number;
  successRate: number;
  avgResponseTime: number;
  utilization: number;
}

export interface ResponseTimeTrendEntry {
  time: string;
  avgTime: number;
  p95Time: number;
  p99Time: number;
}

export interface TaskDistributionEntry {
  type: string;
  count: number;
}

export interface DailyOutcomeEntry {
  day: string;
  success: number;
  failed: number;
  timeout: number;
}

export interface RecentTask {
  id: string;
  agent: string;
  type: string;
  status: 'success' | 'failed' | string;
  duration: number;
  timestamp: string;
}

export interface TaskMetrics {
  distribution?: TaskDistributionEntry[];
  dailyOutcomes?: DailyOutcomeEntry[];
  recentTasks?: RecentTask[];
}

export interface AgentPerformanceData {
  summary: AgentPerformanceSummary;
  agents: Agent[];
  responseTimeTrend?: ResponseTimeTrendEntry[];
  taskMetrics?: TaskMetrics;
}

// ============================================================================
// 12. ROI Calculator
// ============================================================================

export interface ROISummary {
  annualSavings: number;
  savingsGrowth: number;
  roi: number;
  paybackMonths: number;
  hoursSaved: number;
  costPerTransaction: number;
  costReduction: number;
  totalValueGenerated: number;
}

export interface CostCategory {
  name: string;
  value: number;
  trend: number;
}

export interface SavingsCategory {
  name: string;
  saved: number;
  previous: number;
}

export interface CostBreakdown {
  categories: CostCategory[];
  savingsByCategory: SavingsCategory[];
}

export interface SavingsProjectionEntry {
  month: string;
  currentCost: number;
  projectedCost: number;
  savings: number;
}

export interface ROIMilestone {
  name: string;
  timeline: string;
  achieved: boolean;
}

export interface PaybackAnalysis {
  initialInvestment: number;
  monthlyCost: number;
  monthlySavings: number;
  milestones: ROIMilestone[];
}

export interface ROICalculatorData {
  summary: ROISummary;
  costBreakdown: CostBreakdown;
  savingsProjection: {
    monthly: SavingsProjectionEntry[];
  };
  paybackAnalysis: PaybackAnalysis;
}

// ============================================================================
// 13. Financial Reconciliation
// ============================================================================

export interface ReconciliationStatusEntry {
  status: string;
  value: number;
}

export interface ReconciliationSummary {
  totalReconciled: number;
  reconciledRate: number;
  pendingAmount: number;
  pendingCount: number;
  discrepancyAmount: number;
  discrepancyCount: number;
  netCash: number;
  statusDistribution?: ReconciliationStatusEntry[];
}

export interface CashFlowEntry {
  date: string;
  inflow: number;
  outflow: number;
  net: number;
}

export interface ReconciliationCategory {
  name: string;
  reconciled: number;
  total: number;
  rate: number;
}

export interface DiscrepancyType {
  type: string;
  count: number;
  amount: number;
}

export interface DiscrepancyItem {
  id: string;
  transactionId: string;
  description: string;
  source: string;
  expected: number;
  actual: number;
  difference: number;
  status: string;
}

export interface ReconciliationTransaction {
  id: string;
  type: 'inflow' | 'outflow';
  source: string;
  amount: number;
  status: string;
  date: string;
}

export interface FinancialReconciliationData {
  summary: ReconciliationSummary;
  cashFlow?: CashFlowEntry[];
  reconciliationRate?: {
    overall: number;
    byCategory?: ReconciliationCategory[];
  };
  discrepancies?: {
    byType?: DiscrepancyType[];
    items?: DiscrepancyItem[];
  };
  transactions?: ReconciliationTransaction[];
}

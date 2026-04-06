import { z } from 'zod';
import type React from 'react';

/**
 * Generative UI Component Registry for StateSet Admin Dashboard
 *
 * This registry enables AI-powered component selection based on user intent,
 * context, and data shape. Components are dynamically loaded and rendered
 * using the embedded commerce engine as the data source.
 */

// Schema definitions for component data shapes
export const orderStatusBoardSchema = z.object({
  summary: z.object({
    totalOrders: z.number(),
    totalValue: z.number(),
    averageOrderValue: z.number(),
    deliveredRate: z.number(),
    inProgressCount: z.number(),
    exceptionsCount: z.number(),
  }),
  statusGroups: z.array(z.object({
    key: z.string(),
    label: z.string(),
    count: z.number(),
    totalValue: z.number(),
  })),
});

export const inventoryAnalyticsSchema = z.object({
  totalSKUs: z.number(),
  totalUnits: z.number(),
  totalValue: z.number(),
  lowStockItems: z.number(),
  outOfStockItems: z.number(),
  categories: z.array(z.object({
    name: z.string(),
    units: z.number(),
    value: z.number(),
  })),
});

export const returnAnalyticsSchema = z.object({
  totalReturns: z.number(),
  returnRate: z.number(),
  refundTotal: z.number(),
  returnsByReason: z.record(z.number()),
  returnsByStatus: z.record(z.number()),
});

export const customerAnalyticsSchema = z.object({
  totalCustomers: z.number(),
  activeCustomers: z.number(),
  newCustomersThisMonth: z.number(),
  averageLifetimeValue: z.number(),
  retentionRate: z.number(),
  customersBySegment: z.record(z.number()),
});

export const dashboardMetricsSchema = z.object({
  gmvToday: z.number(),
  gmvChange: z.number(),
  ordersToday: z.number(),
  ordersChange: z.number(),
  averageOrderValue: z.number(),
  aovChange: z.number(),
  conversionRate: z.number(),
  inventoryHealth: z.number(),
});

// Component definition type
export interface GenerativeComponent {
  id: string;
  name: string;
  description: string;
  category: string;
  aiPrompts: string[];
  features: string[];
  dataShape?: z.ZodSchema;
  load: () => Promise<React.ComponentType<Record<string, unknown>>>;
  resolveData?: (context: ComponentContext) => Promise<Record<string, unknown>>;
  usageCount?: number;
  lastUsed?: Date;
}

export interface ComponentContext {
  intent?: string;
  category?: string;
  agentType?: string;
  sessionId?: string;
  priority?: 'high' | 'normal' | 'low';
  preferredComponents?: string[];
  excludeComponents?: string[];
  data?: Record<string, unknown>;
  componentId?: string;
}

// Component Registry Class
class ComponentRegistryClass {
  private components: Map<string, GenerativeComponent> = new Map();
  private usageStats: Map<string, { count: number; lastUsed: Date }> = new Map();
  private categoryIndex: Map<string, Set<string>> = new Map();
  private promptIndex: Map<string, Set<string>> = new Map();

  register(component: GenerativeComponent): void {
    this.components.set(component.id, component);

    // Index by category
    if (!this.categoryIndex.has(component.category)) {
      this.categoryIndex.set(component.category, new Set());
    }
    this.categoryIndex.get(component.category)!.add(component.id);

    // Index by AI prompts for semantic matching
    for (const prompt of component.aiPrompts) {
      const keywords = this.extractKeywords(prompt);
      for (const keyword of keywords) {
        if (!this.promptIndex.has(keyword)) {
          this.promptIndex.set(keyword, new Set());
        }
        this.promptIndex.get(keyword)!.add(component.id);
      }
    }
  }

  getComponent(id: string): GenerativeComponent | null {
    return this.components.get(id) || null;
  }

  getComponentsByCategory(category: string): GenerativeComponent[] {
    const ids = this.categoryIndex.get(category);
    if (!ids) return [];
    return Array.from(ids)
      .map(id => this.components.get(id)!)
      .filter(Boolean);
  }

  getAllComponents(): GenerativeComponent[] {
    return Array.from(this.components.values());
  }

  searchComponents(query: string, limit = 5): GenerativeComponent[] {
    const keywords = this.extractKeywords(query);
    const scores: Map<string, number> = new Map();

    for (const keyword of keywords) {
      const matchingIds = this.promptIndex.get(keyword);
      if (matchingIds) {
        Array.from(matchingIds).forEach(id => {
          scores.set(id, (scores.get(id) || 0) + 1);
        });
      }
    }

    // Sort by score and return top matches
    const sortedIds = Array.from(scores.entries())
      .sort((a, b) => b[1] - a[1])
      .slice(0, limit)
      .map(([id]) => id);

    return sortedIds
      .map(id => this.components.get(id)!)
      .filter(Boolean);
  }

  selectOptimalComponent(context: ComponentContext): GenerativeComponent | null {
    let candidates: GenerativeComponent[] = [];

    // If category specified, start with category matches
    if (context.category) {
      candidates = this.getComponentsByCategory(context.category);
    } else {
      candidates = Array.from(this.components.values());
    }

    // Filter by intent if provided
    if (context.intent) {
      const intentMatches = this.searchComponents(context.intent, 10);
      const intentIds = new Set(intentMatches.map(c => c.id));
      candidates = candidates.filter(c => intentIds.has(c.id));
    }

    // Score remaining candidates
    const scored = candidates.map(component => {
      let score = 0;

      // Category match
      if (context.category && component.category === context.category) {
        score += 30;
      }

      // Intent matching
      if (context.intent) {
        const intentLower = context.intent.toLowerCase();
        for (const prompt of component.aiPrompts) {
          if (prompt.toLowerCase().includes(intentLower) ||
              intentLower.includes(prompt.toLowerCase())) {
            score += 20;
            break;
          }
        }
      }

      // Usage bonus (popular components get a slight boost)
      const usage = this.usageStats.get(component.id);
      if (usage) {
        score += Math.min(usage.count, 10);
      }

      // Data shape compatibility
      if (context.data && component.dataShape) {
        try {
          component.dataShape.parse(context.data);
          score += 25; // Perfect data match
        } catch {
          // Partial or no match
        }
      }

      return { component, score };
    });

    // Sort by score and return best match
    scored.sort((a, b) => b.score - a.score);
    return scored[0]?.component || null;
  }

  trackUsage(componentId: string): void {
    const current = this.usageStats.get(componentId) || { count: 0, lastUsed: new Date() };
    this.usageStats.set(componentId, {
      count: current.count + 1,
      lastUsed: new Date(),
    });
  }

  getPopularComponents(limit = 10): GenerativeComponent[] {
    const sorted = Array.from(this.usageStats.entries())
      .sort((a, b) => b[1].count - a[1].count)
      .slice(0, limit)
      .map(([id]) => this.components.get(id)!)
      .filter(Boolean);

    return sorted;
  }

  private extractKeywords(text: string): string[] {
    const stopWords = new Set(['the', 'a', 'an', 'is', 'are', 'for', 'to', 'of', 'and', 'or', 'in', 'on', 'at', 'by', 'my', 'me', 'show', 'display', 'what', 'how', 'where', 'when']);
    return text
      .toLowerCase()
      .replace(/[^\w\s]/g, '')
      .split(/\s+/)
      .filter(word => word.length > 2 && !stopWords.has(word));
  }
}

// Singleton instance
export const componentRegistry = new ComponentRegistryClass();

// Register all generative UI components
// These components use the embedded API instead of REST calls

componentRegistry.register({
  id: 'unified-dashboard',
  name: 'Unified Dashboard',
  description: 'Executive-level operations dashboard with real-time KPIs and system health monitoring',
  category: 'operations',
  aiPrompts: [
    'Show unified dashboard',
    'Display executive overview',
    'How is the business performing today?',
    'Show me the main dashboard',
    'Display KPIs',
  ],
  features: ['Live metrics', 'Department health', 'AI insights', 'Critical alerts'],
  dataShape: dashboardMetricsSchema,
  load: () => import('@/components/operations/unified-dashboard').then(m => m.default),
});

componentRegistry.register({
  id: 'order-pipeline',
  name: 'Order Pipeline',
  description: 'Real-time order flow visualization showing orders progressing through fulfillment stages',
  category: 'orders',
  aiPrompts: [
    'Show order pipeline',
    'Display order flow',
    'How are orders moving through fulfillment?',
    'Show orders by status',
    'Display order stages',
  ],
  features: ['Stage-by-stage tracking', 'Order velocity metrics', 'Exception handling', 'Priority management'],
  dataShape: orderStatusBoardSchema,
  load: () => import('@/components/orders/order-pipeline').then(m => m.default),
});

componentRegistry.register({
  id: 'inventory-analytics',
  name: 'Inventory Analytics',
  description: 'Real-time inventory tracking with stock levels, low stock alerts, and trend visualization',
  category: 'inventory',
  aiPrompts: [
    'Show inventory analytics',
    'What products are low in stock?',
    'Display inventory trends',
    'Show stock levels',
    'Display inventory health',
  ],
  features: ['Stock level monitoring', 'Low stock alerts', 'Category distribution', 'Trend analysis'],
  dataShape: inventoryAnalyticsSchema,
  load: () => import('@/components/inventory/inventory-analytics').then(m => m.default),
});

componentRegistry.register({
  id: 'demand-forecasting',
  name: 'Demand Forecasting',
  description: 'AI-powered inventory predictions with stockout prevention and seasonal analysis',
  category: 'inventory',
  aiPrompts: [
    'Show demand forecasting',
    'What products need reordering?',
    'Display inventory predictions',
    'Predict stock needs',
    'Show forecast',
  ],
  features: ['8-week forecasts', 'Critical stock alerts', 'Seasonal factors', 'Category accuracy'],
  load: () => import('@/components/inventory/demand-forecasting').then(m => m.default),
});

componentRegistry.register({
  id: 'returns-management',
  name: 'Returns Management',
  description: 'AI-powered returns processing with automated approvals and smart routing',
  category: 'returns',
  aiPrompts: [
    'Show returns management',
    'Display return processing',
    'Track returns and RMAs',
    'Show return analytics',
    'Display refunds',
  ],
  features: ['Auto-approval AI', 'Processing pipeline', 'Return analytics', 'Fraud detection'],
  dataShape: returnAnalyticsSchema,
  load: () => import('@/components/returns/returns-management').then(m => m.default),
});

componentRegistry.register({
  id: 'customer-health-score',
  name: 'Customer Health Score',
  description: 'Churn prediction and intervention recommendations with proactive customer success',
  category: 'customers',
  aiPrompts: [
    'Show customer health scores',
    'Display churn risk',
    'Which customers need intervention?',
    'Show at-risk customers',
    'Display customer segments',
  ],
  features: ['Health scoring', 'Churn prediction', 'Intervention triggers', 'Success tracking'],
  dataShape: customerAnalyticsSchema,
  load: () => import('@/components/customers/customer-health-score').then(m => m.default),
});

componentRegistry.register({
  id: 'subscription-analytics',
  name: 'Subscription Analytics',
  description: 'Subscription revenue tracking with plan distribution and growth visualization',
  category: 'subscriptions',
  aiPrompts: [
    'Show subscription analytics',
    'Display recurring revenue',
    'What is our MRR?',
    'Show subscription metrics',
    'Display ARR',
  ],
  features: ['Revenue tracking', 'Plan distribution', 'Growth trends', 'MRR analysis'],
  load: () => import('@/components/subscriptions/subscription-analytics').then(m => m.default),
});

componentRegistry.register({
  id: 'agent-performance',
  name: 'Agent Performance',
  description: 'Real-time monitoring of AI agent workforce with learning metrics',
  category: 'ai',
  aiPrompts: [
    'Show agent performance',
    'How are my agents performing?',
    'Display AI workforce metrics',
    'Show autonomous operations',
  ],
  features: ['Task tracking', 'Success rates', 'Learning progress', 'System health'],
  load: () => import('@/components/agents/agent-performance').then(m => m.default),
});

componentRegistry.register({
  id: 'financial-reconciliation',
  name: 'Financial Reconciliation',
  description: 'Automated payment matching and financial operations with real-time cash flow',
  category: 'finance',
  aiPrompts: [
    'Show financial reconciliation',
    'Display payment matching',
    'What is our cash flow?',
    'Show financial overview',
  ],
  features: ['Payment matching', 'Discrepancy detection', 'Cash flow analysis', 'Fee tracking'],
  load: () => import('@/components/finance/financial-reconciliation').then(m => m.default),
});

componentRegistry.register({
  id: 'exception-management',
  name: 'Exception Management',
  description: 'Critical issues requiring human intervention with severity tracking',
  category: 'operations',
  aiPrompts: [
    'Show exceptions',
    'What needs my attention?',
    'Display critical issues',
    'Show alerts',
    'Display problems',
  ],
  features: ['Priority queue', 'Auto-resolve tracking', 'Suggested actions', 'Impact assessment'],
  load: () => import('@/components/operations/exception-management').then(m => m.default),
});

componentRegistry.register({
  id: 'workflow-builder',
  name: 'Workflow Builder',
  description: 'Create and manage automated workflows with drag-and-drop simplicity',
  category: 'operations',
  aiPrompts: [
    'Show workflow builder',
    'Create automated workflow',
    'Build automation',
    'Show workflows',
  ],
  features: ['Visual builder', 'Trigger conditions', 'Multi-step workflows', 'AI suggestions'],
  load: () => import('@/components/operations/workflow-builder').then(m => m.default),
});

componentRegistry.register({
  id: 'roi-calculator',
  name: 'ROI Calculator',
  description: 'Financial impact analysis showing cost savings and return on investment',
  category: 'finance',
  aiPrompts: [
    'Show ROI calculator',
    'Calculate my savings',
    'What is my return on investment?',
    'Show cost analysis',
  ],
  features: ['Cost breakdown', 'Savings projection', 'Outcome pricing', 'Payback period'],
  load: () => import('@/components/finance/roi-calculator').then(m => m.default),
});

componentRegistry.register({
  id: 'product-catalog',
  name: 'Product Catalog',
  description: 'Comprehensive product management with inventory integration',
  category: 'products',
  aiPrompts: [
    'Show product catalog',
    'Display products',
    'Show product list',
    'Manage products',
  ],
  features: ['Product listing', 'Inventory sync', 'Pricing management', 'Category organization'],
  load: () => import('@/components/products/product-catalog').then(m => m.default),
});

componentRegistry.register({
  id: 'system-health',
  name: 'System Health',
  description: 'Monitor embedded engine health and database performance',
  category: 'operations',
  aiPrompts: [
    'Show system health',
    'Display database status',
    'Is the system healthy?',
    'Show performance metrics',
  ],
  features: ['Database latency', 'Error rates', 'Connection pool', 'Query performance'],
  load: () => import('@/components/operations/system-health').then(m => m.default),
});


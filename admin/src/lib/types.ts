/**
 * TypeScript Types for StateSet Commerce
 *
 * These types are shared between client and server components.
 * Import these types in client components instead of from embedded.ts.
 *
 * This is the only declaration of the commerce record shapes: `embedded.ts`
 * re-exports them rather than keeping a second copy, so the two can no
 * longer drift apart.
 *
 * Status unions are not written out here: they come from the native binding
 * through `./engine-lifecycles` (type-only import, safe in client bundles).
 */

import type {
  OrderStatus,
  ReturnReason,
  ReturnStatus,
  SubscriptionStatus,
} from './engine-lifecycles';

// ============================================================================
// Orders Types
// ============================================================================

export interface Order {
  id: string;
  customerId: string;
  /** The engine's order lifecycle (`ORDER_LIFECYCLE` in `./engine-lifecycles`). */
  status: OrderStatus;
  items: OrderItem[];
  /** Display convenience; arithmetic uses `totalAmountExact` when the engine supplies it. */
  totalAmount: number;
  /** Exact decimal string (the binding's `OrderOutput.totalAmountExact`). */
  totalAmountExact?: string;
  currency: string;
  shippingAddress?: Address;
  billingAddress?: Address;
  createdAt: string;
  updatedAt: string;
}

export interface OrderItem {
  productId: string;
  sku: string;
  name: string;
  quantity: number;
  unitPrice: number;
  totalPrice: number;
}

export interface Address {
  line1: string;
  line2?: string;
  city: string;
  state: string;
  postalCode: string;
  country: string;
}

/**
 * How much of the store an aggregate the admin derives from records covers.
 * The engine pages every list, so a derivation that stops early must say so
 * rather than present a subset as the whole.
 */
export interface AnalyticsCoverage {
  /** False when the figures stop short of every matching record. */
  complete: boolean;
  /** How many records the figures were computed from. */
  records: number;
  /** The requested reporting window (inclusive RFC 3339 bounds); absent for all time. */
  window?: { start?: string; end?: string };
  /**
   * Figures the engine cannot report for this window. They read zero but
   * were not measured; show them as unavailable, not as zero.
   */
  unavailable?: string[];
}

/*
 * Money in the derived aggregates below: each `…Exact` field is the exact
 * decimal string, computed without floats; the number beside it is the same
 * value for display only (like the binding's deprecated float twins).
 */

export interface OrderAnalytics {
  totalOrders: number;
  totalRevenue: number;
  totalRevenueExact?: string;
  averageOrderValue: number;
  averageOrderValueExact?: string;
  ordersByStatus: Record<string, number>;
  ordersByDay: { date: string; count: number; revenue: number; revenueExact?: string }[];
  /** Present when the admin derived the figures from order records. */
  coverage?: AnalyticsCoverage;
}

// ============================================================================
// Inventory Types
// ============================================================================

export interface InventoryItem {
  id: string;
  sku: string;
  productId: string;
  productName: string;
  quantity: number;
  reservedQuantity: number;
  availableQuantity: number;
  reorderPoint: number;
  reorderQuantity: number;
  warehouseId?: string;
  location?: string;
  lastRestocked?: string;
  updatedAt: string;
}

export interface InventoryAnalytics {
  totalSKUs: number;
  totalUnits: number;
  totalValue: number;
  totalValueExact?: string;
  lowStockItems: number;
  outOfStockItems: number;
  turnoverRate: number;
  topMovingItems: { sku: string; name: string; velocity: number }[];
  slowMovingItems: { sku: string; name: string; daysSinceLastSale: number }[];
}

// ============================================================================
// Returns Types
// ============================================================================

export interface Return {
  id: string;
  orderId: string;
  customerId: string;
  /** The engine's return lifecycle (`RETURN_LIFECYCLE` in `./engine-lifecycles`). */
  status: ReturnStatus;
  items: ReturnItem[];
  /** Free-text detail; the engine's own reason code is `reasonCategory`. */
  reason: string;
  reasonCategory: ReturnReason;
  refundAmount?: number;
  refundMethod?: 'original' | 'store_credit' | 'exchange';
  trackingNumber?: string;
  notes?: string;
  createdAt: string;
  updatedAt: string;
}

export interface ReturnItem {
  productId: string;
  sku: string;
  name: string;
  quantity: number;
  condition?: 'new' | 'opened' | 'damaged' | 'used';
  returnReason?: string;
}

export interface ReturnAnalytics {
  totalReturns: number;
  returnRate: number;
  refundTotal: number;
  refundTotalExact?: string;
  returnsByReason: Record<string, number>;
  returnsByStatus: Record<string, number>;
  averageProcessingTime: number;
  topReturnedProducts: { productId: string; name: string; count: number; rate: number }[];
  /** Present when the admin derived the figures from return records. */
  coverage?: AnalyticsCoverage;
}

// ============================================================================
// Customers Types
// ============================================================================

export interface Customer {
  id: string;
  email: string;
  firstName?: string;
  lastName?: string;
  phone?: string;
  defaultAddress?: Address;
  addresses: Address[];
  tags: string[];
  totalOrders: number;
  totalSpent: number;
  averageOrderValue: number;
  lastOrderDate?: string;
  createdAt: string;
  updatedAt: string;
}

export interface CustomerAnalytics {
  totalCustomers: number;
  newCustomersThisMonth: number;
  activeCustomers: number;
  averageLifetimeValue: number;
  averageOrdersPerCustomer: number;
  customersBySegment: Record<string, number>;
  acquisitionTrend: { date: string; count: number }[];
  retentionRate: number;
  churnRate: number;
}

// ============================================================================
// Products Types
// ============================================================================

export interface Product {
  id: string;
  sku: string;
  name: string;
  description?: string;
  price: number;
  compareAtPrice?: number;
  costPrice?: number;
  currency: string;
  category?: string;
  tags: string[];
  status: 'active' | 'draft' | 'archived';
  images: string[];
  variants: ProductVariant[];
  createdAt: string;
  updatedAt: string;
}

export interface ProductVariant {
  id: string;
  sku: string;
  name: string;
  price: number;
  options: Record<string, string>;
  inventoryQuantity: number;
}

export interface ProductAnalytics {
  totalProducts: number;
  activeProducts: number;
  draftProducts: number;
  archivedProducts: number;
  productsByCategory: Record<string, number>;
  avgPrice: number;
  minPrice: number;
  maxPrice: number;
  totalInventoryValue: number;
}

// ============================================================================
// Subscriptions Types
// ============================================================================

export interface Subscription {
  id: string;
  customerId: string;
  /** The engine's subscription lifecycle (`SUBSCRIPTION_LIFECYCLE` in `./engine-lifecycles`). */
  status: SubscriptionStatus;
  plan: string;
  planId?: string;
  frequency: 'weekly' | 'biweekly' | 'monthly' | 'quarterly' | 'annually';
  nextBillingDate: string;
  currentPeriodEnd?: string;
  items: { productId: string; quantity: number }[];
  quantity: number;
  /** Display convenience; arithmetic uses `totalAmountExact` when the engine supplies it. */
  totalAmount: number;
  /** Exact decimal string (the binding's `SubscriptionOutput.priceExact`). */
  totalAmountExact?: string;
  createdAt: string;
  updatedAt: string;
}

export interface SubscriptionAnalytics {
  totalSubscriptions: number;
  activeSubscriptions: number;
  mrr: number;
  mrrExact?: string;
  mrrGrowth?: number;
  arr: number;
  arrExact?: string;
  churnRate: number;
  arpu?: number;
  arpuExact?: string;
  averageLifetime: number;
  subscriptionsByPlan: Record<string, number>;
  subscriptionsByFrequency: Record<string, number>;
  /** Present when the admin derived the figures from subscription records. */
  coverage?: AnalyticsCoverage;
}

// ============================================================================
// Analytics Types
// ============================================================================

export interface DashboardMetrics {
  gmvToday: number;
  gmvChange: number;
  ordersToday: number;
  ordersChange: number;
  averageOrderValue: number;
  aovChange: number;
  conversionRate: number;
  conversionChange: number;
  activeCustomers: number;
  newCustomers: number;
  returnRate: number;
  inventoryHealth: number;
}

export interface HourlyActivity {
  hour: string;
  orders: number;
  revenue: number;
}

export interface SystemHealth {
  databaseLatency: number;
  errorRate: number;
  activeConnections: number;
  queueDepth: number;
  processingSpeed: number;
}

// ============================================================================
// Sessions Types (from StateSet Sandbox API)
// ============================================================================

export type AgentSessionStatus =
  | 'pending'
  | 'running'
  | 'rotating'
  | 'paused'
  | 'completed'
  | 'failed'
  | 'cancelled';

export interface AgentSession {
  id: string;
  organization_id: string;
  org_name: string | null;
  org_slug: string | null;
  status: AgentSessionStatus;
  current_sandbox_id: string | null;
  name: string | null;
  description: string | null;
  budget_config: {
    cost_cap_cents?: number;
    iteration_limit?: number;
    duration_limit_seconds?: number;
  };
  budget_consumed: {
    cost_cents: number;
    iterations: number;
    duration_seconds: number;
  };
  rotation_count: number;
  total_exec_count: number;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
  last_activity_at: string;
  error_message: string | null;
  error_code: string | null;
}

export interface AgentSessionsResponse {
  total: number;
  sessions: AgentSession[];
}

export interface AgentEvent {
  id: string;
  session_id: string;
  sandbox_id: string | null;
  event_type: string;
  event_subtype: string | null;
  payload: Record<string, unknown>;
  sequence_number: number;
  duration_ms: number | null;
  success: boolean | null;
  error_message: string | null;
  created_at: string;
}

export interface AgentSessionDetail {
  session: AgentSession;
  events: AgentEvent[];
}

export interface AgentSessionSummary {
  total: number;
  by_status: Record<AgentSessionStatus, number>;
  active_now: number;
  rotations_last_hour: number;
  avg_duration_seconds: number;
}

/**
 * TypeScript Types for StateSet Commerce
 *
 * These types are shared between client and server components.
 * Import these types in client components instead of from embedded.ts
 */

// ============================================================================
// Orders Types
// ============================================================================

export interface Order {
  id: string;
  customerId: string;
  status: 'pending' | 'confirmed' | 'processing' | 'shipped' | 'delivered' | 'cancelled';
  items: OrderItem[];
  totalAmount: number;
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

export interface OrderAnalytics {
  totalOrders: number;
  totalRevenue: number;
  averageOrderValue: number;
  ordersByStatus: Record<string, number>;
  ordersByDay: { date: string; count: number; revenue: number }[];
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
  status: 'requested' | 'approved' | 'received' | 'inspected' | 'refunded' | 'rejected' | 'closed';
  items: ReturnItem[];
  reason: string;
  reasonCategory: 'defective' | 'wrong_item' | 'not_as_described' | 'changed_mind' | 'other';
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
  returnsByReason: Record<string, number>;
  returnsByStatus: Record<string, number>;
  averageProcessingTime: number;
  topReturnedProducts: { productId: string; name: string; count: number; rate: number }[];
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
  status: 'active' | 'paused' | 'cancelled' | 'expired';
  plan: string;
  planId?: string;
  frequency: 'weekly' | 'biweekly' | 'monthly' | 'quarterly' | 'annually';
  nextBillingDate: string;
  currentPeriodEnd?: string;
  items: { productId: string; quantity: number }[];
  quantity: number;
  totalAmount: number;
  createdAt: string;
  updatedAt: string;
}

export interface SubscriptionAnalytics {
  totalSubscriptions: number;
  activeSubscriptions: number;
  mrr: number;
  mrrGrowth?: number;
  arr: number;
  churnRate: number;
  arpu?: number;
  averageLifetime: number;
  subscriptionsByPlan: Record<string, number>;
  subscriptionsByFrequency: Record<string, number>;
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

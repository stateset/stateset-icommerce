/**
 * StateSet Embedded Commerce API Service
 *
 * This module provides direct access to the embedded Rust commerce engine
 * instead of making REST API calls. All operations run locally with SQLite
 * or connect to PostgreSQL for production deployments.
 *
 * NOTE: This module should ONLY be imported in server components or server actions.
 * For client components, use the server actions in app/actions/commerce.ts
 */

import 'server-only';

// Commerce engine interface matching the API surface
export interface CommerceEngine {
  orders: {
    list: (params?: { status?: string; limit?: number; offset?: number }) => Promise<Order[]>;
    get: (id: string) => Promise<Order | null>;
    create: (params: CreateOrderParams) => Promise<Order>;
    updateStatus: (id: string, status: string) => Promise<Order>;
    cancel: (id: string, reason?: string) => Promise<Order>;
    getAnalytics: (params?: { startDate?: string; endDate?: string }) => Promise<OrderAnalytics>;
  };
  inventory: {
    list: (params?: { warehouseId?: string; lowStock?: boolean }) => Promise<InventoryItem[]>;
    get: (sku: string) => Promise<InventoryItem | null>;
    adjust: (sku: string, quantity: number, reason?: string) => Promise<InventoryAdjustment>;
    reserve: (sku: string, quantity: number, orderId: string) => Promise<InventoryItem>;
    release: (sku: string, quantity: number, orderId: string) => Promise<InventoryItem>;
    getLowStock: (threshold?: number) => Promise<InventoryItem[]>;
    getAnalytics: () => Promise<InventoryAnalytics>;
    forecast: (sku: string, days: number) => Promise<DemandForecast>;
  };
  returns: {
    list: (params?: { status?: string; customerId?: string }) => Promise<Return[]>;
    get: (id: string) => Promise<Return | null>;
    create: (params: CreateReturnParams) => Promise<Return>;
    approve: (id: string) => Promise<Return>;
    reject: (id: string, reason?: string) => Promise<Return>;
    receive: (id: string, items?: { productId: string; condition: string }[]) => Promise<Return>;
    processRefund: (id: string, method?: Return['refundMethod']) => Promise<Return>;
    getAnalytics: (params?: { startDate?: string; endDate?: string }) => Promise<ReturnAnalytics>;
  };
  customers: {
    list: (params?: { segment?: string; limit?: number; offset?: number }) => Promise<Customer[]>;
    get: (id: string) => Promise<Customer | null>;
    getByEmail: (email: string) => Promise<Customer | null>;
    create: (params: Partial<Customer>) => Promise<Customer>;
    update: (id: string, params: Partial<Customer>) => Promise<Customer>;
    getOrders: (customerId: string) => Promise<Order[]>;
    getHealthScore: (customerId: string) => Promise<CustomerHealthScore>;
    getSegments: () => Promise<CustomerSegment[]>;
    getAnalytics: () => Promise<CustomerAnalytics>;
  };
  products: {
    list: (params?: { status?: string; category?: string }) => Promise<Product[]>;
    get: (id: string) => Promise<Product | null>;
    create: (params: Partial<Product>) => Promise<Product>;
    update: (id: string, params: Partial<Product>) => Promise<Product>;
    delete: (id: string) => Promise<void>;
  };
  subscriptions: {
    list: (params?: { status?: string; customerId?: string }) => Promise<Subscription[]>;
    get: (id: string) => Promise<Subscription | null>;
    create: (params: Partial<Subscription>) => Promise<Subscription>;
    pause: (id: string) => Promise<Subscription>;
    resume: (id: string) => Promise<Subscription>;
    cancel: (id: string, reason?: string) => Promise<Subscription>;
    getAnalytics: () => Promise<SubscriptionAnalytics>;
  };
  generalLedger: {
    listAccounts: () => Promise<GlAccount[]>;
    getTrialBalance: (asOfDate: string) => Promise<TrialBalance>;
    listJournalEntries: () => Promise<JournalEntry[]>;
    /** Optional: not every engine build exposes period listing yet. */
    listPeriods?: () => Promise<GlPeriod[]>;
    closeMonth: (periodId: string, options?: CloseMonthOptions) => Promise<CloseMonthReport>;
  };
  accountsPayable: {
    listBills: () => Promise<Bill[]>;
    getAgingSummary: () => Promise<ApAgingSummary>;
  };
  accountsReceivable: {
    getAgingSummary: () => Promise<ArAgingSummary>;
    /** Optional: not every engine build exposes DSO. */
    getDso?: (days: number) => Promise<number>;
  };
  invoices: {
    list: () => Promise<Invoice[]>;
  };
  fixedAssets: {
    list: (filter?: FixedAssetFilter) => Promise<FixedAsset[]>;
    getSchedule: (id: string) => Promise<DepreciationSchedule | null>;
  };
  revenueRecognition: {
    listContracts: (filter?: RevenueContractFilter) => Promise<RevenueContract[]>;
  };
  /** Optional: not every engine build exposes EDI documents yet. */
  ediDocuments?: {
    list: (filter?: EdiDocumentFilter) => Promise<EdiDocument[]>;
    get: (id: string) => Promise<EdiDocument | null>;
    /** Optional even on EDI-capable builds; the API layer computes a fallback. */
    summary?: () => Promise<EdiSummary>;
  };
  /** Optional: purchasing (purchase orders + suppliers). Read-only slice. */
  purchaseOrders?: {
    list: () => Promise<PurchaseOrder[]>;
    get: (id: string) => Promise<PurchaseOrder | null>;
    listSuppliers: () => Promise<Supplier[]>;
  };
  /** Optional: warehouses + storage locations. Read-only slice. */
  warehouse?: {
    listWarehouses: () => Promise<WarehouseRecord[]>;
    listLocations: (warehouseId?: number) => Promise<WarehouseLocation[]>;
  };
  /** Optional: cycle counts. Read-only slice. */
  cycleCounts?: {
    list: (filter?: CycleCountFilter) => Promise<CycleCount[]>;
  };
  /** Optional: manufacturing work orders. Read-only slice. */
  workOrders?: {
    list: () => Promise<WorkOrder[]>;
    get: (id: string) => Promise<WorkOrder | null>;
  };
  /** Optional: quality inspections + non-conformance reports. Read-only slice. */
  quality?: {
    listInspections: () => Promise<QualityInspection[]>;
    listNcrs: () => Promise<NonConformanceReport[]>;
  };
  /** Optional: fulfillment waves + pick tasks. Read-only slice. */
  fulfillment?: {
    listWaves: () => Promise<Wave[]>;
    listPicks: () => Promise<PickTask[]>;
  };
  /** Optional: lot genealogy. Read-only slice. */
  lots?: {
    list: () => Promise<Lot[]>;
  };
  /** Optional: serial numbers. Read-only slice. */
  serials?: {
    list: () => Promise<SerialNumber[]>;
  };
  /** Optional: inbound receipts. Read-only slice. */
  receiving?: {
    listReceipts: () => Promise<Receipt[]>;
  };
  analytics: {
    getDashboardMetrics: () => Promise<DashboardMetrics>;
    getHourlyActivity: (date?: string) => Promise<HourlyActivity[]>;
    getSystemHealth: () => Promise<SystemHealth>;
    getRevenueByPeriod: (params: {
      startDate: string;
      endDate: string;
      groupBy: 'day' | 'week' | 'month';
    }) => Promise<{ period: string; revenue: number; orders: number }[]>;
    getTopProducts: (
      limit?: number,
    ) => Promise<{ productId: string; name: string; revenue: number; units: number }[]>;
    getConversionFunnel: () => Promise<{ stage: string; count: number; rate: number }[]>;
  };
  initialize?: () => Promise<void>;
}

// Dynamic import to prevent bundling issues
let StateSetCommerce: (new (config: { databasePath: string }) => CommerceEngine) | null = null;

const DAY_MS = 24 * 60 * 60 * 1000;
const HOUR_MS = 60 * 60 * 1000;
const DEFAULT_DATABASE_PATH = process.env.DATABASE_PATH || './data/admin.db';
const MOCK_FLAG = 'STATESET_ADMIN_ALLOW_MOCK_DATA';
const MOCK_NOW = Date.parse('2026-03-01T12:00:00.000Z');

type MockCommerceData = {
  orders: Order[];
  orderAnalytics: OrderAnalytics;
  inventory: InventoryItem[];
  inventoryAnalytics: InventoryAnalytics;
  returns: Return[];
  returnAnalytics: ReturnAnalytics;
  customers: Customer[];
  customerAnalytics: CustomerAnalytics;
  products: Product[];
  subscriptions: Subscription[];
  subscriptionAnalytics: SubscriptionAnalytics;
  dashboardMetrics: DashboardMetrics;
  hourlyActivity: HourlyActivity[];
};

let commerceInstance: CommerceEngine | null = null;
let mockDataCache: MockCommerceData | null = null;
let mockSequence = 0;

function roundCurrency(value: number): number {
  return Number(value.toFixed(2));
}

function deterministicUnit(seed: number): number {
  const x = Math.sin(seed * 12.9898 + 78.233) * 43758.5453;
  return x - Math.floor(x);
}

function deterministicInt(seed: number, min: number, max: number): number {
  return Math.floor(min + deterministicUnit(seed) * (max - min + 1));
}

function deterministicNumber(seed: number, min: number, max: number): number {
  return min + deterministicUnit(seed) * (max - min);
}

function deterministicChoice<T>(seed: number, values: T[]): T {
  return values[deterministicInt(seed, 0, values.length - 1)];
}

function formatMockDate(daysOffset: number = 0, hoursOffset: number = 0): string {
  return new Date(MOCK_NOW + daysOffset * DAY_MS + hoursOffset * HOUR_MS).toISOString();
}

function toDateKey(iso: string): string {
  return iso.slice(0, 10);
}

function isTruthyFlag(value: string | undefined): boolean {
  return typeof value === 'string' && /^(1|true|yes|on)$/i.test(value.trim());
}

function isProductionRuntime(): boolean {
  return process.env.NODE_ENV === 'production';
}

function shouldAllowMockEngine(): boolean {
  const explicit = process.env[MOCK_FLAG];
  if (explicit === undefined) {
    return process.env.NODE_ENV === 'test';
  }
  return isTruthyFlag(explicit) && !isProductionRuntime();
}

function mockEngineError(reason: unknown): Error {
  const detail = reason instanceof Error ? reason.message : 'unknown error';
  const requestedMockMode = isTruthyFlag(process.env[MOCK_FLAG]);
  const guidance =
    requestedMockMode && isProductionRuntime()
      ? `${MOCK_FLAG}=true is rejected in production.`
      : `Set ${MOCK_FLAG}=true only for explicit demo mode.`;
  return new Error(`Unable to load @stateset/embedded (${detail}). ${guidance}`);
}

async function loadCommerceModule() {
  if (StateSetCommerce) {
    return StateSetCommerce;
  }

  try {
    const mod = await import('@stateset/embedded');
    const moduleExports = mod as Record<string, unknown>;
    const candidate = (moduleExports.default || moduleExports.Commerce || moduleExports) as unknown;
    if (typeof candidate !== 'function') {
      throw new Error('Embedded module did not export a constructor');
    }
    StateSetCommerce = candidate as new (config: { databasePath: string }) => CommerceEngine;
    return StateSetCommerce;
  } catch (error) {
    if (!shouldAllowMockEngine()) {
      throw mockEngineError(error);
    }

    console.warn(`[embedded] Falling back to deterministic mock data because ${MOCK_FLAG}=true.`);
    return null;
  }
}

/**
 * Initialize or get the embedded commerce engine instance.
 * Mock data is only allowed when explicitly enabled for demo or test mode.
 */
export async function getCommerceEngine(): Promise<CommerceEngine> {
  if (!commerceInstance) {
    const CommerceModule = await loadCommerceModule();
    if (CommerceModule) {
      commerceInstance = new CommerceModule({ databasePath: DEFAULT_DATABASE_PATH });
      await commerceInstance.initialize?.();
    } else {
      commerceInstance = createMockCommerceEngine();
    }
  }
  return commerceInstance;
}

function nextMockId(prefix: string): string {
  mockSequence += 1;
  return `${prefix}_${10_000 + mockSequence}`;
}

function buildMockCommerceData(): MockCommerceData {
  const categories = ['Electronics', 'Home', 'Office', 'Outdoor', 'Apparel'];
  const plans = [
    { plan: 'Basic', planId: 'basic', amount: 29, frequency: 'monthly' as const },
    { plan: 'Pro', planId: 'pro', amount: 79, frequency: 'monthly' as const },
    { plan: 'Enterprise', planId: 'enterprise', amount: 249, frequency: 'quarterly' as const },
  ];
  const statusPool: Order['status'][] = [
    'delivered',
    'delivered',
    'shipped',
    'processing',
    'confirmed',
    'pending',
    'cancelled',
  ];
  const returnStatuses: Return['status'][] = [
    'requested',
    'approved',
    'received',
    'inspected',
    'refunded',
    'rejected',
  ];
  const returnReasons: Return['reasonCategory'][] = [
    'defective',
    'wrong_item',
    'not_as_described',
    'changed_mind',
    'other',
  ];
  const subscriptionStatuses: Subscription['status'][] = [
    'active',
    'active',
    'active',
    'paused',
    'cancelled',
  ];

  const products = Array.from({ length: 50 }, (_, index) => {
    const price = roundCurrency(24.99 + deterministicNumber(index + 1, 0, 185));
    const createdAt = formatMockDate(-90 + index, index % 12);
    return {
      id: `prod_${index}`,
      sku: `SKU-${1000 + index}`,
      name: `${categories[index % categories.length]} Product ${index + 1}`,
      description: `Deterministic demo record for product ${index + 1}`,
      price,
      currency: 'USD',
      category: categories[index % categories.length],
      tags: [categories[index % categories.length].toLowerCase()],
      status: index % 13 === 0 ? 'draft' : index % 17 === 0 ? 'archived' : ('active' as const),
      images: [],
      variants: [],
      createdAt,
      updatedAt: createdAt,
    } satisfies Product;
  });

  const customers = Array.from({ length: 100 }, (_, index) => {
    const totalOrders = deterministicInt(index + 101, 1, 18);
    const averageOrderValue = roundCurrency(deterministicNumber(index + 201, 55, 165));
    const totalSpent = roundCurrency(totalOrders * averageOrderValue);
    const createdAt = formatMockDate(-360 + index * 3, index % 18);
    return {
      id: `cus_${100 + index}`,
      email: `customer${index}@example.com`,
      firstName: `First${index}`,
      lastName: `Last${index}`,
      addresses: [],
      tags: index % 6 === 0 ? ['vip'] : [],
      totalOrders,
      totalSpent,
      averageOrderValue,
      lastOrderDate: formatMockDate(-(index % 45), index % 12),
      createdAt,
      updatedAt: formatMockDate(-(index % 21), index % 9),
    } satisfies Customer;
  });

  const orders = Array.from({ length: 50 }, (_, index) => {
    const customer = customers[index % customers.length];
    const product = products[index % products.length];
    const quantity = deterministicInt(index + 301, 1, 3);
    const subtotal = roundCurrency(quantity * product.price);
    const fulfillment = roundCurrency(deterministicNumber(index + 401, 8, 42));
    const createdAt = formatMockDate(-(29 - (index % 30)), (index * 3) % 24);
    return {
      id: `ord_${1000 + index}`,
      customerId: customer.id,
      status: deterministicChoice(index + 501, statusPool),
      items: [
        {
          productId: product.id,
          sku: product.sku,
          name: product.name,
          quantity,
          unitPrice: product.price,
          totalPrice: subtotal,
        },
      ],
      totalAmount: roundCurrency(subtotal + fulfillment),
      currency: 'USD',
      createdAt,
      updatedAt: createdAt,
    } satisfies Order;
  });

  const productById = new Map(products.map((product) => [product.id, product]));

  const inventory = Array.from({ length: 100 }, (_, index) => {
    const product = products[index % products.length];
    const quantity = deterministicInt(index + 601, 40, 480);
    const reservedQuantity = deterministicInt(
      index + 701,
      0,
      Math.min(40, Math.floor(quantity / 3)),
    );
    const availableQuantity = Math.max(quantity - reservedQuantity, 0);
    return {
      id: `inv_${index}`,
      sku: product.sku,
      productId: product.id,
      productName: product.name,
      quantity,
      reservedQuantity,
      availableQuantity,
      reorderPoint: 20 + (index % 5) * 5,
      reorderQuantity: 80 + (index % 4) * 20,
      warehouseId: `wh_${1 + (index % 3)}`,
      location: `A-${1 + (index % 8)}`,
      lastRestocked: formatMockDate(-(index % 21) - 2, index % 10),
      updatedAt: formatMockDate(-(index % 14), index % 8),
    } satisfies InventoryItem;
  });

  const returns = Array.from({ length: 25 }, (_, index) => {
    const order = orders[(index * 2) % orders.length];
    const amount = roundCurrency(
      order.totalAmount * (0.35 + deterministicNumber(index + 801, 0, 0.45)),
    );
    const createdAt = formatMockDate(-(13 - (index % 14)), index % 12);
    return {
      id: `ret_${1000 + index}`,
      orderId: order.id,
      customerId: order.customerId,
      status: deterministicChoice(index + 901, returnStatuses),
      items: order.items.map((item) => ({
        productId: item.productId,
        sku: item.sku,
        name: item.name,
        quantity: 1,
      })),
      reason: 'Customer requested return',
      reasonCategory: deterministicChoice(index + 1001, returnReasons),
      refundAmount: amount,
      createdAt,
      updatedAt: createdAt,
    } satisfies Return;
  });

  const subscriptions: Subscription[] = Array.from({ length: 30 }, (_, index) => {
    const customer = customers[index % customers.length];
    const plan = plans[index % plans.length];
    const quantity = deterministicInt(index + 1101, 1, 4);
    const createdAt = formatMockDate(-120 + index * 2, index % 10);
    return {
      id: `sub_${1000 + index}`,
      customerId: customer.id,
      status: deterministicChoice(index + 1201, subscriptionStatuses),
      plan: plan.plan,
      planId: plan.planId,
      frequency: plan.frequency,
      nextBillingDate: formatMockDate(1 + (index % 28), index % 10),
      currentPeriodEnd: formatMockDate(7 + (index % 28), index % 10),
      items: [{ productId: products[index % products.length].id, quantity }],
      quantity,
      totalAmount: roundCurrency(plan.amount * quantity),
      createdAt,
      updatedAt: formatMockDate(-(index % 10), index % 6),
    } satisfies Subscription;
  });

  const ordersByStatus = orders.reduce<Record<string, number>>((acc, order) => {
    acc[order.status] = (acc[order.status] || 0) + 1;
    return acc;
  }, {});

  const ordersByDay = Array.from({ length: 30 }, (_, index) => {
    const day = toDateKey(formatMockDate(-(29 - index)));
    const dailyOrders = orders.filter((order) => toDateKey(order.createdAt) === day);
    return {
      date: day,
      count: dailyOrders.length,
      revenue: roundCurrency(dailyOrders.reduce((sum, order) => sum + order.totalAmount, 0)),
    };
  });

  const totalRevenue = roundCurrency(orders.reduce((sum, order) => sum + order.totalAmount, 0));
  const totalUnits = inventory.reduce((sum, item) => sum + item.quantity, 0);
  const inventoryValue = roundCurrency(
    inventory.reduce((sum, item) => {
      const product = productById.get(item.productId);
      return sum + item.quantity * (product?.price || 0);
    }, 0),
  );
  const lowStockItems = inventory.filter((item) => item.availableQuantity <= item.reorderPoint);
  const outOfStockItems = inventory.filter((item) => item.availableQuantity === 0);
  const customersBySegment = customers.reduce<Record<string, number>>((acc, customer) => {
    const segment =
      customer.totalSpent > 1200
        ? 'champion'
        : customer.totalOrders > 10
          ? 'loyal'
          : customer.totalOrders > 6
            ? 'potential'
            : customer.totalOrders > 3
              ? 'at_risk'
              : customer.totalOrders > 1
                ? 'hibernating'
                : 'lost';
    acc[segment] = (acc[segment] || 0) + 1;
    return acc;
  }, {});
  const subscriptionMrr = roundCurrency(
    subscriptions
      .filter((subscription) => subscription.status === 'active')
      .reduce((sum, subscription) => {
        const monthlyFactor =
          subscription.frequency === 'quarterly'
            ? 1 / 3
            : subscription.frequency === 'annually'
              ? 1 / 12
              : subscription.frequency === 'biweekly'
                ? 2
                : subscription.frequency === 'weekly'
                  ? 4
                  : 1;
        return sum + subscription.totalAmount * monthlyFactor;
      }, 0),
  );

  const dashboardMetrics = (() => {
    const today = toDateKey(formatMockDate(0));
    const yesterday = toDateKey(formatMockDate(-1));
    const todayOrders = orders.filter((order) => toDateKey(order.createdAt) === today);
    const yesterdayOrders = orders.filter((order) => toDateKey(order.createdAt) === yesterday);
    const gmvToday = roundCurrency(todayOrders.reduce((sum, order) => sum + order.totalAmount, 0));
    const gmvYesterday = roundCurrency(
      yesterdayOrders.reduce((sum, order) => sum + order.totalAmount, 0),
    );
    const averageOrderValue = orders.length ? roundCurrency(totalRevenue / orders.length) : 0;
    const gmvChange =
      gmvYesterday > 0 ? Number(((gmvToday - gmvYesterday) / gmvYesterday).toFixed(3)) : 0;
    const ordersChange =
      yesterdayOrders.length > 0
        ? Number(
            ((todayOrders.length - yesterdayOrders.length) / yesterdayOrders.length).toFixed(3),
          )
        : 0;

    return {
      gmvToday,
      gmvChange,
      ordersToday: todayOrders.length,
      ordersChange,
      averageOrderValue,
      aovChange: Number((averageOrderValue / 140).toFixed(3)),
      conversionRate: Number(Math.min(0.12, orders.length / 1200).toFixed(3)),
      conversionChange: 0.018,
      activeCustomers: customers.filter((customer) => customer.totalOrders > 2).length,
      newCustomers: customers.filter((customer) => customer.createdAt >= formatMockDate(-30))
        .length,
      returnRate: Number((returns.length / Math.max(orders.length, 1)).toFixed(3)),
      inventoryHealth: Number(
        (1 - lowStockItems.length / Math.max(inventory.length, 1)).toFixed(3),
      ),
    } satisfies DashboardMetrics;
  })();

  const hourlyActivity = Array.from({ length: 24 }, (_, hour) => {
    const ordersCount = deterministicInt(hour + 1301, 4, 18);
    const averageRevenue = deterministicNumber(hour + 1401, 550, 1950);
    return {
      hour: `${hour}:00`,
      orders: ordersCount,
      revenue: roundCurrency(ordersCount * averageRevenue * 0.18),
    } satisfies HourlyActivity;
  });

  return {
    orders,
    orderAnalytics: {
      totalOrders: orders.length,
      totalRevenue,
      averageOrderValue: orders.length ? roundCurrency(totalRevenue / orders.length) : 0,
      ordersByStatus,
      ordersByDay,
    },
    inventory,
    inventoryAnalytics: {
      totalSKUs: inventory.length,
      totalUnits,
      totalValue: inventoryValue,
      lowStockItems: lowStockItems.length,
      outOfStockItems: outOfStockItems.length,
      turnoverRate: Number((orders.length / Math.max(inventory.length, 1) + 3.2).toFixed(1)),
      topMovingItems: products.slice(0, 5).map((product, index) => ({
        sku: product.sku,
        name: product.name,
        velocity: deterministicInt(index + 1501, 24, 48),
      })),
      slowMovingItems: products.slice(-3).map((product, index) => ({
        sku: product.sku,
        name: product.name,
        daysSinceLastSale: 24 + index * 7,
      })),
    },
    returns,
    returnAnalytics: {
      totalReturns: returns.length,
      returnRate: Number((returns.length / Math.max(orders.length, 1)).toFixed(3)),
      refundTotal: roundCurrency(returns.reduce((sum, item) => sum + (item.refundAmount || 0), 0)),
      returnsByReason: returns.reduce<Record<string, number>>((acc, item) => {
        acc[item.reasonCategory] = (acc[item.reasonCategory] || 0) + 1;
        return acc;
      }, {}),
      returnsByStatus: returns.reduce<Record<string, number>>((acc, item) => {
        acc[item.status] = (acc[item.status] || 0) + 1;
        return acc;
      }, {}),
      averageProcessingTime: 48,
      topReturnedProducts: returns.slice(0, 3).map((item, index) => ({
        productId: item.items[0]?.productId || `prod_${index}`,
        name: item.items[0]?.name || `Product ${index + 1}`,
        count: 2 + index,
        rate: Number((0.02 + index * 0.01).toFixed(2)),
      })),
    },
    customers,
    customerAnalytics: {
      totalCustomers: customers.length,
      newCustomersThisMonth: customers.filter(
        (customer) => customer.createdAt >= formatMockDate(-30),
      ).length,
      activeCustomers: customers.filter((customer) => customer.totalOrders > 2).length,
      averageLifetimeValue: roundCurrency(
        customers.reduce((sum, customer) => sum + customer.totalSpent, 0) /
          Math.max(customers.length, 1),
      ),
      averageOrdersPerCustomer: Number(
        (
          customers.reduce((sum, customer) => sum + customer.totalOrders, 0) /
          Math.max(customers.length, 1)
        ).toFixed(1),
      ),
      customersBySegment,
      acquisitionTrend: Array.from({ length: 6 }, (_, index) => ({
        date: toDateKey(formatMockDate(-(150 - index * 30))),
        count: 8 + index * 3,
      })),
      retentionRate: 0.72,
      churnRate: 0.028,
    },
    products,
    subscriptions,
    subscriptionAnalytics: {
      totalSubscriptions: subscriptions.length,
      activeSubscriptions: subscriptions.filter((subscription) => subscription.status === 'active')
        .length,
      mrr: subscriptionMrr,
      mrrGrowth: 0.08,
      arr: roundCurrency(subscriptionMrr * 12),
      churnRate: Number(
        (
          subscriptions.filter((subscription) => subscription.status === 'cancelled').length /
          Math.max(subscriptions.length, 1)
        ).toFixed(3),
      ),
      arpu: roundCurrency(
        subscriptionMrr /
          Math.max(
            subscriptions.filter((subscription) => subscription.status === 'active').length,
            1,
          ),
      ),
      averageLifetime: 18,
      subscriptionsByPlan: subscriptions.reduce<Record<string, number>>((acc, subscription) => {
        acc[subscription.plan] = (acc[subscription.plan] || 0) + 1;
        return acc;
      }, {}),
      subscriptionsByFrequency: subscriptions.reduce<Record<string, number>>(
        (acc, subscription) => {
          acc[subscription.frequency] = (acc[subscription.frequency] || 0) + 1;
          return acc;
        },
        {},
      ),
    },
    dashboardMetrics,
    hourlyActivity,
  };
}

function getMockData(): MockCommerceData {
  if (!mockDataCache) {
    mockDataCache = buildMockCommerceData();
  }
  return mockDataCache;
}

function groupRevenueByPeriod(
  orders: Order[],
  params: { startDate: string; endDate: string; groupBy: 'day' | 'week' | 'month' },
): { period: string; revenue: number; orders: number }[] {
  const start = Date.parse(params.startDate);
  const end = Date.parse(params.endDate);
  const buckets = new Map<string, { revenue: number; orders: number }>();

  for (const order of orders) {
    const created = Date.parse(order.createdAt);
    if (Number.isNaN(created) || created < start || created > end) {
      continue;
    }

    const date = new Date(created);
    const period =
      params.groupBy === 'month'
        ? `${date.getUTCFullYear()}-${String(date.getUTCMonth() + 1).padStart(2, '0')}`
        : params.groupBy === 'week'
          ? (() => {
              const startOfWeek = new Date(
                Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate()),
              );
              startOfWeek.setUTCDate(
                startOfWeek.getUTCDate() - ((startOfWeek.getUTCDay() + 6) % 7),
              );
              return toDateKey(startOfWeek.toISOString());
            })()
          : toDateKey(order.createdAt);

    const current = buckets.get(period) || { revenue: 0, orders: 0 };
    current.revenue += order.totalAmount;
    current.orders += 1;
    buckets.set(period, current);
  }

  return Array.from(buckets.entries())
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([period, current]) => ({
      period,
      revenue: roundCurrency(current.revenue),
      orders: current.orders,
    }));
}

function getMockOrder(id: string): Order | null {
  return getMockData().orders.find((order) => order.id === id) || null;
}

function getMockInventoryItem(sku: string): InventoryItem | null {
  return getMockData().inventory.find((item) => item.sku === sku) || null;
}

function getMockReturn(id: string): Return | null {
  return getMockData().returns.find((entry) => entry.id === id) || null;
}

function getMockSubscription(id: string): Subscription | null {
  return getMockData().subscriptions.find((subscription) => subscription.id === id) || null;
}

function getMockCustomer(id: string): Customer | null {
  return getMockData().customers.find((customer) => customer.id === id) || null;
}

function buildMockOrderFromParams(params: CreateOrderParams): Order {
  const productsById = new Map(getMockData().products.map((product) => [product.id, product]));
  const items = params.items.map((item, index) => {
    const product = productsById.get(item.productId);
    const unitPrice = product?.price || roundCurrency(29 + index * 7);
    return {
      productId: item.productId,
      sku: product?.sku || `SKU-${item.productId}`,
      name: product?.name || `Product ${item.productId}`,
      quantity: item.quantity,
      unitPrice,
      totalPrice: roundCurrency(unitPrice * item.quantity),
    };
  });
  const totalAmount = roundCurrency(items.reduce((sum, item) => sum + item.totalPrice, 0));
  const createdAt = new Date().toISOString();

  return {
    id: nextMockId('ord'),
    customerId: params.customerId,
    status: 'pending',
    items,
    totalAmount,
    currency: 'USD',
    shippingAddress: params.shippingAddress,
    billingAddress: params.billingAddress,
    createdAt,
    updatedAt: createdAt,
  };
}

function buildMockReturnFromParams(params: CreateReturnParams): Return {
  const productsById = new Map(getMockData().products.map((product) => [product.id, product]));
  const createdAt = new Date().toISOString();
  return {
    id: nextMockId('ret'),
    orderId: params.orderId,
    customerId: getMockOrder(params.orderId)?.customerId || 'cus_100',
    status: 'requested',
    items: params.items.map((item) => {
      const product = productsById.get(item.productId);
      return {
        productId: item.productId,
        sku: product?.sku || `SKU-${item.productId}`,
        name: product?.name || `Product ${item.productId}`,
        quantity: item.quantity,
        returnReason: item.reason,
      };
    }),
    reason: params.reason,
    reasonCategory: params.reasonCategory,
    refundAmount: roundCurrency(
      params.items.reduce((sum, item) => {
        const product = productsById.get(item.productId);
        return sum + (product?.price || 25) * item.quantity;
      }, 0),
    ),
    createdAt,
    updatedAt: createdAt,
  };
}

function buildMockCustomer(params: Partial<Customer>, id: string): Customer {
  const createdAt = new Date().toISOString();
  return {
    id,
    email: params.email || '',
    firstName: params.firstName,
    lastName: params.lastName,
    phone: params.phone,
    defaultAddress: params.defaultAddress,
    addresses: params.addresses || [],
    tags: params.tags || [],
    totalOrders: params.totalOrders || 0,
    totalSpent: params.totalSpent || 0,
    averageOrderValue: params.averageOrderValue || 0,
    lastOrderDate: params.lastOrderDate,
    createdAt,
    updatedAt: createdAt,
  };
}

function buildMockProduct(
  params: Partial<Product>,
  id: string,
  status: Product['status'],
): Product {
  const createdAt = new Date().toISOString();
  return {
    id,
    sku: params.sku || `SKU-${id}`,
    name: params.name || `Product ${id}`,
    description: params.description,
    price: params.price || 0,
    compareAtPrice: params.compareAtPrice,
    costPrice: params.costPrice,
    currency: params.currency || 'USD',
    category: params.category,
    tags: params.tags || [],
    status,
    images: params.images || [],
    variants: params.variants || [],
    createdAt,
    updatedAt: createdAt,
  };
}

function buildMockSubscription(params: Partial<Subscription>, id: string): Subscription {
  const createdAt = new Date().toISOString();
  return {
    id,
    customerId: params.customerId || 'cus_100',
    status: params.status || 'active',
    plan: params.plan || 'Basic',
    planId: params.planId || 'basic',
    frequency: params.frequency || 'monthly',
    nextBillingDate: params.nextBillingDate || createdAt,
    currentPeriodEnd: params.currentPeriodEnd || createdAt,
    items: params.items || [],
    quantity: params.quantity || 1,
    totalAmount: params.totalAmount || 29,
    createdAt,
    updatedAt: createdAt,
  };
}

/**
 * Mock commerce engine for explicit demo and test mode.
 */
function createMockCommerceEngine(): CommerceEngine {
  return {
    orders: {
      list: async (params) => {
        let orders = getMockData().orders;
        if (params?.status) {
          orders = orders.filter((order) => order.status === params.status);
        }
        const offset = params?.offset || 0;
        const end = params?.limit ? offset + params.limit : undefined;
        return orders.slice(offset, end);
      },
      get: async (id: string) => getMockOrder(id),
      create: async (params: CreateOrderParams) => buildMockOrderFromParams(params),
      updateStatus: async (id: string, status: string) => {
        const current =
          getMockOrder(id) || buildMockOrderFromParams({ customerId: 'cus_100', items: [] });
        return {
          ...current,
          id,
          status: status as Order['status'],
          updatedAt: new Date().toISOString(),
        };
      },
      cancel: async (id: string) => {
        const current =
          getMockOrder(id) || buildMockOrderFromParams({ customerId: 'cus_100', items: [] });
        return { ...current, id, status: 'cancelled', updatedAt: new Date().toISOString() };
      },
      getAnalytics: async () => getMockData().orderAnalytics,
    },
    inventory: {
      list: async (params) => {
        let items = getMockData().inventory;
        if (params?.warehouseId) {
          items = items.filter((item) => item.warehouseId === params.warehouseId);
        }
        if (params?.lowStock) {
          items = items.filter((item) => item.availableQuantity <= item.reorderPoint);
        }
        return items;
      },
      get: async (sku: string) => getMockInventoryItem(sku),
      adjust: async (sku: string, quantity: number, reason?: string) => {
        const current =
          getMockInventoryItem(sku) ||
          ({
            id: nextMockId('inv'),
            sku,
            productId: `prod_${sku}`,
            productName: `Product ${sku}`,
            quantity: 0,
            reservedQuantity: 0,
            availableQuantity: 0,
            reorderPoint: 20,
            reorderQuantity: 100,
            updatedAt: new Date().toISOString(),
          } satisfies InventoryItem);
        const newQuantity = Math.max(current.quantity + quantity, 0);
        return {
          id: nextMockId('adj'),
          sku,
          adjustmentType: quantity >= 0 ? 'add' : 'remove',
          quantity,
          previousQuantity: current.quantity,
          newQuantity,
          reason,
          createdAt: new Date().toISOString(),
        };
      },
      reserve: async (sku: string, quantity: number) => {
        const current = getMockInventoryItem(sku);
        if (!current) {
          throw new Error(`Unknown SKU ${sku}`);
        }
        return {
          ...current,
          reservedQuantity: current.reservedQuantity + quantity,
          availableQuantity: Math.max(current.availableQuantity - quantity, 0),
          updatedAt: new Date().toISOString(),
        };
      },
      release: async (sku: string, quantity: number) => {
        const current = getMockInventoryItem(sku);
        if (!current) {
          throw new Error(`Unknown SKU ${sku}`);
        }
        return {
          ...current,
          reservedQuantity: Math.max(current.reservedQuantity - quantity, 0),
          availableQuantity: current.availableQuantity + quantity,
          updatedAt: new Date().toISOString(),
        };
      },
      getLowStock: async (threshold?: number) => {
        const limit = threshold ?? 20;
        return getMockData().inventory.filter((item) => item.availableQuantity <= limit);
      },
      getAnalytics: async () => getMockData().inventoryAnalytics,
      forecast: async (sku: string, days: number) => {
        const current = getMockInventoryItem(sku);
        const currentStock = current?.availableQuantity || 100;
        const forecastedDemand = Array.from({ length: Math.max(days, 1) }, (_, index) =>
          deterministicInt(index + currentStock, 8, 22),
        );
        return {
          sku,
          currentStock,
          forecastedDemand,
          recommendedReorder: Math.max(
            20,
            Math.round(forecastedDemand.reduce((sum, value) => sum + value, 0) * 0.4),
          ),
          confidence: 0.87,
        };
      },
    },
    returns: {
      list: async (params) => {
        let returns = getMockData().returns;
        if (params?.status) {
          returns = returns.filter((entry) => entry.status === params.status);
        }
        if (params?.customerId) {
          returns = returns.filter((entry) => entry.customerId === params.customerId);
        }
        return returns;
      },
      get: async (id: string) => getMockReturn(id),
      create: async (params: CreateReturnParams) => buildMockReturnFromParams(params),
      approve: async (id: string) => {
        const current =
          getMockReturn(id) ||
          buildMockReturnFromParams({
            orderId: 'ord_1000',
            items: [],
            reason: 'Manual review',
            reasonCategory: 'other',
          });
        return { ...current, id, status: 'approved', updatedAt: new Date().toISOString() };
      },
      reject: async (id: string, reason?: string) => {
        const current =
          getMockReturn(id) ||
          buildMockReturnFromParams({
            orderId: 'ord_1000',
            items: [],
            reason: reason || 'Rejected',
            reasonCategory: 'other',
          });
        return {
          ...current,
          id,
          status: 'rejected',
          notes: reason,
          updatedAt: new Date().toISOString(),
        };
      },
      receive: async (id: string) => {
        const current =
          getMockReturn(id) ||
          buildMockReturnFromParams({
            orderId: 'ord_1000',
            items: [],
            reason: 'Received',
            reasonCategory: 'other',
          });
        return { ...current, id, status: 'received', updatedAt: new Date().toISOString() };
      },
      processRefund: async (id: string, method?: Return['refundMethod']) => {
        const current =
          getMockReturn(id) ||
          buildMockReturnFromParams({
            orderId: 'ord_1000',
            items: [],
            reason: 'Refunded',
            reasonCategory: 'other',
          });
        return {
          ...current,
          id,
          status: 'refunded',
          refundMethod: method || 'original',
          updatedAt: new Date().toISOString(),
        };
      },
      getAnalytics: async () => getMockData().returnAnalytics,
    },
    customers: {
      list: async (params) => {
        let customers = getMockData().customers;
        if (params?.segment) {
          customers = customers.filter((customer) => customer.tags.includes(params.segment || ''));
        }
        const offset = params?.offset || 0;
        const end = params?.limit ? offset + params.limit : undefined;
        return customers.slice(offset, end);
      },
      get: async (id: string) => getMockCustomer(id),
      getByEmail: async (email: string) =>
        getMockData().customers.find((customer) => customer.email === email) || null,
      create: async (params: Partial<Customer>) => buildMockCustomer(params, nextMockId('cus')),
      update: async (id: string, params: Partial<Customer>) => ({
        ...(getMockCustomer(id) || buildMockCustomer({}, id)),
        ...params,
        id,
        updatedAt: new Date().toISOString(),
      }),
      getOrders: async (customerId: string) =>
        getMockData().orders.filter((order) => order.customerId === customerId),
      getHealthScore: async (customerId: string) => {
        const customer = getMockCustomer(customerId);
        const monetary = customer ? Math.min(100, Math.round(customer.totalSpent / 25)) : 40;
        const frequency = customer ? Math.min(100, customer.totalOrders * 6) : 30;
        const recency = customer?.lastOrderDate ? 80 : 45;
        const overallScore = Math.round((monetary + frequency + recency + 70 + 78) / 5);
        return {
          customerId,
          overallScore,
          factors: {
            recency,
            frequency,
            monetary,
            engagement: 70,
            satisfaction: 78,
          },
          segment: overallScore > 80 ? 'loyal' : overallScore > 65 ? 'potential' : 'at_risk',
          churnRisk: Number((1 - overallScore / 100).toFixed(2)),
        };
      },
      getSegments: async () =>
        Object.entries(getMockData().customerAnalytics.customersBySegment).map(([name, count]) => ({
          id: name,
          name,
          description: `${name} customers`,
          criteria: {},
          customerCount: count,
        })),
      getAnalytics: async () => getMockData().customerAnalytics,
    },
    products: {
      list: async (params) => {
        let products = getMockData().products;
        if (params?.status) {
          products = products.filter((product) => product.status === params.status);
        }
        if (params?.category) {
          products = products.filter((product) => product.category === params.category);
        }
        return products;
      },
      get: async (id: string) =>
        getMockData().products.find((product) => product.id === id) || null,
      create: async (params: Partial<Product>) =>
        buildMockProduct(params, nextMockId('prod'), params.status || 'draft'),
      update: async (id: string, params: Partial<Product>) => ({
        ...(getMockData().products.find((product) => product.id === id) ||
          buildMockProduct({}, id, 'active')),
        ...params,
        id,
        updatedAt: new Date().toISOString(),
      }),
      delete: async () => {},
    },
    subscriptions: {
      list: async (params) => {
        let subscriptions = getMockData().subscriptions;
        if (params?.status) {
          subscriptions = subscriptions.filter(
            (subscription) => subscription.status === params.status,
          );
        }
        if (params?.customerId) {
          subscriptions = subscriptions.filter(
            (subscription) => subscription.customerId === params.customerId,
          );
        }
        return subscriptions;
      },
      get: async (id: string) => getMockSubscription(id),
      create: async (params: Partial<Subscription>) =>
        buildMockSubscription(params, nextMockId('sub')),
      pause: async (id: string) => ({
        ...(getMockSubscription(id) || buildMockSubscription({}, id)),
        id,
        status: 'paused',
        updatedAt: new Date().toISOString(),
      }),
      resume: async (id: string) => ({
        ...(getMockSubscription(id) || buildMockSubscription({}, id)),
        id,
        status: 'active',
        updatedAt: new Date().toISOString(),
      }),
      cancel: async (id: string) => ({
        ...(getMockSubscription(id) || buildMockSubscription({}, id)),
        id,
        status: 'cancelled',
        updatedAt: new Date().toISOString(),
      }),
      getAnalytics: async () => getMockData().subscriptionAnalytics,
    },
    generalLedger: {
      listAccounts: async () => getMockFinanceData().glAccounts,
      getTrialBalance: async (asOfDate: string) => ({
        ...getMockFinanceData().trialBalance,
        asOfDate,
      }),
      listJournalEntries: async () => getMockFinanceData().journalEntries,
      listPeriods: async () => getMockFinanceData().glPeriods,
      closeMonth: async (periodId: string, options?: CloseMonthOptions) =>
        buildMockCloseMonthReport(periodId, options),
    },
    accountsPayable: {
      listBills: async () => getMockFinanceData().bills,
      getAgingSummary: async () => getMockFinanceData().apAging,
    },
    accountsReceivable: {
      getAgingSummary: async () => getMockFinanceData().arAging,
      getDso: async (days: number) => Number((28.4 + (days % 30) * 0.2).toFixed(1)),
    },
    invoices: {
      list: async () => getMockFinanceData().invoices,
    },
    fixedAssets: {
      list: async (filter?: FixedAssetFilter) => {
        let assets = getMockFinanceData().fixedAssets;
        if (filter?.status) {
          assets = assets.filter((asset) => asset.status === filter.status);
        }
        if (filter?.category) {
          assets = assets.filter((asset) => asset.category === filter.category);
        }
        return assets;
      },
      getSchedule: async (id: string) => getMockFinanceData().depreciationSchedules[id] || null,
    },
    revenueRecognition: {
      listContracts: async (filter?: RevenueContractFilter) => {
        let contracts = getMockFinanceData().revenueContracts;
        if (filter?.status) {
          contracts = contracts.filter((contract) => contract.status === filter.status);
        }
        if (filter?.customerId) {
          contracts = contracts.filter((contract) => contract.customerId === filter.customerId);
        }
        return contracts;
      },
    },
    ediDocuments: {
      list: async (filter?: EdiDocumentFilter) => {
        let documents = getMockFinanceData().ediDocuments;
        if (filter?.documentType) {
          documents = documents.filter((doc) => doc.documentType === filter.documentType);
        }
        if (filter?.direction) {
          documents = documents.filter((doc) => doc.direction === filter.direction);
        }
        if (filter?.status) {
          documents = documents.filter((doc) => doc.status === filter.status);
        }
        if (filter?.partner) {
          documents = documents.filter((doc) => doc.partner === filter.partner);
        }
        const offset = filter?.offset || 0;
        const limit = filter?.limit;
        return documents.slice(offset, limit !== undefined ? offset + limit : undefined);
      },
      get: async (id: string) =>
        getMockFinanceData().ediDocuments.find((doc) => doc.id === id) || null,
      summary: async () => summarizeEdiDocuments(getMockFinanceData().ediDocuments),
    },
    ...createMockOperationsSections(),
    analytics: {
      getDashboardMetrics: async () => getMockData().dashboardMetrics,
      getHourlyActivity: async () => getMockData().hourlyActivity,
      getSystemHealth: async () => ({
        databaseLatency: 0.48,
        errorRate: 0.12,
        activeConnections: 18,
        queueDepth: 6,
        processingSpeed: 1240,
      }),
      getRevenueByPeriod: async (params) => groupRevenueByPeriod(getMockData().orders, params),
      getTopProducts: async (limit = 5) => {
        const totals = new Map<string, { name: string; revenue: number; units: number }>();
        for (const order of getMockData().orders) {
          for (const item of order.items) {
            const current = totals.get(item.productId) || {
              name: item.name,
              revenue: 0,
              units: 0,
            };
            current.revenue += item.totalPrice;
            current.units += item.quantity;
            totals.set(item.productId, current);
          }
        }

        return Array.from(totals.entries())
          .map(([productId, current]) => ({
            productId,
            name: current.name,
            revenue: roundCurrency(current.revenue),
            units: current.units,
          }))
          .sort((left, right) => right.revenue - left.revenue)
          .slice(0, limit);
      },
      getConversionFunnel: async () => {
        const orders = getMockData().orders.length;
        const visits = Math.max(orders * 12, 1);
        const carts = Math.round(visits * 0.29);
        const checkouts = Math.round(carts * 0.61);
        const purchases = orders;
        return [
          { stage: 'Visits', count: visits, rate: 1 },
          { stage: 'Cart', count: carts, rate: Number((carts / visits).toFixed(3)) },
          { stage: 'Checkout', count: checkouts, rate: Number((checkouts / visits).toFixed(3)) },
          { stage: 'Purchase', count: purchases, rate: Number((purchases / visits).toFixed(3)) },
        ];
      },
    },
  };
}

// ============================================================================
// Orders API
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

export interface CreateOrderParams {
  customerId: string;
  items: { productId: string; quantity: number }[];
  shippingAddress?: Address;
  billingAddress?: Address;
}

export const ordersApi = {
  async list(params?: { status?: string; limit?: number; offset?: number }): Promise<Order[]> {
    const commerce = await getCommerceEngine();
    return commerce.orders.list(params);
  },

  async get(orderId: string): Promise<Order | null> {
    const commerce = await getCommerceEngine();
    return commerce.orders.get(orderId);
  },

  async create(params: CreateOrderParams): Promise<Order> {
    const commerce = await getCommerceEngine();
    return commerce.orders.create(params);
  },

  async updateStatus(orderId: string, status: Order['status']): Promise<Order> {
    const commerce = await getCommerceEngine();
    return commerce.orders.updateStatus(orderId, status);
  },

  async cancel(orderId: string, reason?: string): Promise<Order> {
    const commerce = await getCommerceEngine();
    return commerce.orders.cancel(orderId, reason);
  },

  async getAnalytics(params?: { startDate?: string; endDate?: string }): Promise<OrderAnalytics> {
    const commerce = await getCommerceEngine();
    return commerce.orders.getAnalytics(params);
  },
};

export interface OrderAnalytics {
  totalOrders: number;
  totalRevenue: number;
  averageOrderValue: number;
  ordersByStatus: Record<string, number>;
  ordersByDay: { date: string; count: number; revenue: number }[];
}

// ============================================================================
// Inventory API
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

export interface InventoryAdjustment {
  id: string;
  sku: string;
  adjustmentType: 'add' | 'remove' | 'set' | 'reserve' | 'release';
  quantity: number;
  previousQuantity: number;
  newQuantity: number;
  reason?: string;
  createdAt: string;
}

export const inventoryApi = {
  async list(params?: { warehouseId?: string; lowStock?: boolean }): Promise<InventoryItem[]> {
    const commerce = await getCommerceEngine();
    return commerce.inventory.list(params);
  },

  async get(sku: string): Promise<InventoryItem | null> {
    const commerce = await getCommerceEngine();
    return commerce.inventory.get(sku);
  },

  async adjust(sku: string, quantity: number, reason?: string): Promise<InventoryAdjustment> {
    const commerce = await getCommerceEngine();
    return commerce.inventory.adjust(sku, quantity, reason);
  },

  async reserve(sku: string, quantity: number, orderId: string): Promise<InventoryItem> {
    const commerce = await getCommerceEngine();
    return commerce.inventory.reserve(sku, quantity, orderId);
  },

  async release(sku: string, quantity: number, orderId: string): Promise<InventoryItem> {
    const commerce = await getCommerceEngine();
    return commerce.inventory.release(sku, quantity, orderId);
  },

  async getLowStock(threshold?: number): Promise<InventoryItem[]> {
    const commerce = await getCommerceEngine();
    return commerce.inventory.getLowStock(threshold);
  },

  async getAnalytics(): Promise<InventoryAnalytics> {
    const commerce = await getCommerceEngine();
    return commerce.inventory.getAnalytics();
  },

  async forecast(sku: string, days: number): Promise<DemandForecast> {
    const commerce = await getCommerceEngine();
    return commerce.inventory.forecast(sku, days);
  },
};

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

export interface DemandForecast {
  sku: string;
  currentStock: number;
  forecastedDemand: number[];
  recommendedReorder: number;
  stockoutDate?: string;
  confidence: number;
}

// ============================================================================
// Returns API
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

export interface CreateReturnParams {
  orderId: string;
  items: { productId: string; quantity: number; reason?: string }[];
  reason: string;
  reasonCategory: Return['reasonCategory'];
}

export const returnsApi = {
  async list(params?: { status?: string; customerId?: string }): Promise<Return[]> {
    const commerce = await getCommerceEngine();
    return commerce.returns.list(params);
  },

  async get(returnId: string): Promise<Return | null> {
    const commerce = await getCommerceEngine();
    return commerce.returns.get(returnId);
  },

  async create(params: CreateReturnParams): Promise<Return> {
    const commerce = await getCommerceEngine();
    return commerce.returns.create(params);
  },

  async approve(returnId: string): Promise<Return> {
    const commerce = await getCommerceEngine();
    return commerce.returns.approve(returnId);
  },

  async reject(returnId: string, reason: string): Promise<Return> {
    const commerce = await getCommerceEngine();
    return commerce.returns.reject(returnId, reason);
  },

  async receive(
    returnId: string,
    items: { productId: string; condition: string }[],
  ): Promise<Return> {
    const commerce = await getCommerceEngine();
    return commerce.returns.receive(returnId, items);
  },

  async processRefund(returnId: string, method: Return['refundMethod']): Promise<Return> {
    const commerce = await getCommerceEngine();
    return commerce.returns.processRefund(returnId, method);
  },

  async getAnalytics(params?: { startDate?: string; endDate?: string }): Promise<ReturnAnalytics> {
    const commerce = await getCommerceEngine();
    return commerce.returns.getAnalytics(params);
  },
};

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
// Customers API
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

export interface CustomerSegment {
  id: string;
  name: string;
  description?: string;
  criteria: Record<string, unknown>;
  customerCount: number;
}

export const customersApi = {
  async list(params?: { segment?: string; limit?: number; offset?: number }): Promise<Customer[]> {
    const commerce = await getCommerceEngine();
    return commerce.customers.list(params);
  },

  async get(customerId: string): Promise<Customer | null> {
    const commerce = await getCommerceEngine();
    return commerce.customers.get(customerId);
  },

  async getByEmail(email: string): Promise<Customer | null> {
    const commerce = await getCommerceEngine();
    return commerce.customers.getByEmail(email);
  },

  async create(params: Partial<Customer>): Promise<Customer> {
    const commerce = await getCommerceEngine();
    return commerce.customers.create(params);
  },

  async update(customerId: string, params: Partial<Customer>): Promise<Customer> {
    const commerce = await getCommerceEngine();
    return commerce.customers.update(customerId, params);
  },

  async getOrders(customerId: string): Promise<Order[]> {
    const commerce = await getCommerceEngine();
    return commerce.customers.getOrders(customerId);
  },

  async getHealthScore(customerId: string): Promise<CustomerHealthScore> {
    const commerce = await getCommerceEngine();
    return commerce.customers.getHealthScore(customerId);
  },

  async getSegments(): Promise<CustomerSegment[]> {
    const commerce = await getCommerceEngine();
    return commerce.customers.getSegments();
  },

  async getAnalytics(): Promise<CustomerAnalytics> {
    const commerce = await getCommerceEngine();
    return commerce.customers.getAnalytics();
  },
};

export interface CustomerHealthScore {
  customerId: string;
  overallScore: number; // 0-100
  factors: {
    recency: number;
    frequency: number;
    monetary: number;
    engagement: number;
    satisfaction: number;
  };
  segment: 'champion' | 'loyal' | 'potential' | 'at_risk' | 'hibernating' | 'lost';
  nextBestAction?: string;
  churnRisk: number;
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
// Products API
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

export const productsApi = {
  async list(params?: { status?: string; category?: string }): Promise<Product[]> {
    const commerce = await getCommerceEngine();
    return commerce.products.list(params);
  },

  async get(productId: string): Promise<Product | null> {
    const commerce = await getCommerceEngine();
    return commerce.products.get(productId);
  },

  async create(params: Partial<Product>): Promise<Product> {
    const commerce = await getCommerceEngine();
    return commerce.products.create(params);
  },

  async update(productId: string, params: Partial<Product>): Promise<Product> {
    const commerce = await getCommerceEngine();
    return commerce.products.update(productId, params);
  },

  async delete(productId: string): Promise<void> {
    const commerce = await getCommerceEngine();
    return commerce.products.delete(productId);
  },
};

// ============================================================================
// Analytics API
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

export const analyticsApi = {
  async getDashboardMetrics(): Promise<DashboardMetrics> {
    const commerce = await getCommerceEngine();
    return commerce.analytics.getDashboardMetrics();
  },

  async getHourlyActivity(date?: string): Promise<HourlyActivity[]> {
    const commerce = await getCommerceEngine();
    return commerce.analytics.getHourlyActivity(date);
  },

  async getSystemHealth(): Promise<SystemHealth> {
    const commerce = await getCommerceEngine();
    return commerce.analytics.getSystemHealth();
  },

  async getRevenueByPeriod(params: {
    startDate: string;
    endDate: string;
    groupBy: 'day' | 'week' | 'month';
  }): Promise<{ period: string; revenue: number; orders: number }[]> {
    const commerce = await getCommerceEngine();
    return commerce.analytics.getRevenueByPeriod(params);
  },

  async getTopProducts(
    limit?: number,
  ): Promise<{ productId: string; name: string; revenue: number; units: number }[]> {
    const commerce = await getCommerceEngine();
    return commerce.analytics.getTopProducts(limit);
  },

  async getConversionFunnel(): Promise<{ stage: string; count: number; rate: number }[]> {
    const commerce = await getCommerceEngine();
    return commerce.analytics.getConversionFunnel();
  },
};

// ============================================================================
// Subscriptions API
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

export const subscriptionsApi = {
  async list(params?: { status?: string; customerId?: string }): Promise<Subscription[]> {
    const commerce = await getCommerceEngine();
    return commerce.subscriptions.list(params);
  },

  async get(subscriptionId: string): Promise<Subscription | null> {
    const commerce = await getCommerceEngine();
    return commerce.subscriptions.get(subscriptionId);
  },

  async create(params: Partial<Subscription>): Promise<Subscription> {
    const commerce = await getCommerceEngine();
    return commerce.subscriptions.create(params);
  },

  async pause(subscriptionId: string): Promise<Subscription> {
    const commerce = await getCommerceEngine();
    return commerce.subscriptions.pause(subscriptionId);
  },

  async resume(subscriptionId: string): Promise<Subscription> {
    const commerce = await getCommerceEngine();
    return commerce.subscriptions.resume(subscriptionId);
  },

  async cancel(subscriptionId: string, reason?: string): Promise<Subscription> {
    const commerce = await getCommerceEngine();
    return commerce.subscriptions.cancel(subscriptionId, reason);
  },

  async getAnalytics(): Promise<SubscriptionAnalytics> {
    const commerce = await getCommerceEngine();
    return commerce.subscriptions.getAnalytics();
  },
};

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
// Finance API (General Ledger + Accounts Payable)
// ============================================================================

export interface GlAccount {
  id: string;
  accountNumber: string;
  name: string;
  accountType: string;
  balance: number;
  status: string;
  description?: string;
}

export interface JournalEntry {
  id: string;
  entryNumber: string;
  entryDate: string;
  description: string;
  status: string;
  createdAt: string;
}

export interface TrialBalance {
  asOfDate: string;
  totalDebits: number;
  totalCredits: number;
  isBalanced: boolean;
}

export interface GlPeriod {
  id: string;
  periodName: string;
  fiscalYear: number;
  periodNumber: number;
  startDate: string;
  endDate: string;
  /** One of `future`, `open`, `closed`, `locked` */
  status: string;
  closedBy?: string;
}

export interface CloseMonthOptions {
  /** Compute per-step counts/amounts without writing anything */
  dryRun?: boolean;
  skipDepreciation?: boolean;
  skipRevenueRecognition?: boolean;
  skipFxRevaluation?: boolean;
  skipPeriodClose?: boolean;
  closedBy?: string;
}

export interface CloseMonthStep {
  /** One of `executed`, `skipped`, `dry_run` */
  status: string;
  entryCount: number;
  /** Exact decimal string — render only, never parse for math */
  totalAmount: string;
  warnings: string[];
}

export interface CloseMonthReport {
  periodId: string;
  periodName: string;
  dryRun: boolean;
  depreciation: CloseMonthStep;
  revenueRecognition: CloseMonthStep;
  fxRevaluation: CloseMonthStep;
  periodClose: CloseMonthStep;
  closingEntry?: JournalEntry;
  periodStatus: string;
}

export interface Bill {
  id: string;
  billNumber: string;
  supplierId: string;
  status: string;
  totalAmount: number;
  amountPaid: number;
  amountDue: number;
  dueDate: string;
  createdAt: string;
}

export interface ApAgingSummary {
  current: number;
  days130: number;
  days3160: number;
  days6190: number;
  daysOver90: number;
  total: number;
}

export interface ArAgingSummary {
  current: number;
  days130: number;
  days3160: number;
  days6190: number;
  daysOver90: number;
  total: number;
}

export interface Invoice {
  id: string;
  invoiceNumber: string;
  customerId: string;
  orderId?: string;
  status: string;
  subtotal: number;
  taxAmount: number;
  total: number;
  amountPaid: number;
  dueDate: string;
  createdAt: string;
  updatedAt: string;
}

export interface FixedAssetFilter {
  category?: string;
  /** One of `draft`, `in_service`, `fully_depreciated`, `disposed`, `written_off` */
  status?: string;
  search?: string;
  limit?: number;
  offset?: number;
}

export interface FixedAsset {
  id: string;
  assetNumber: string;
  name: string;
  description?: string;
  category: string;
  /** ISO date (YYYY-MM-DD) */
  acquisitionDate: string;
  /** Exact decimal string — render only, never parse for math */
  acquisitionCost: string;
  /** Exact decimal string */
  salvageValue: string;
  usefulLifeMonths: number;
  /** One of `straight_line`, `declining_balance`, `units_of_production` */
  depreciationMethod: string;
  /** One of `draft`, `in_service`, `fully_depreciated`, `disposed`, `written_off` */
  status: string;
  inServiceDate?: string;
  /** Exact decimal string */
  accumulatedDepreciation: string;
  /** Exact decimal string: acquisition_cost - accumulated_depreciation */
  bookValue: string;
  currency: string;
  createdAt: string;
  updatedAt: string;
}

export interface DepreciationEntry {
  period: number;
  /** Exact decimal string */
  amount: string;
  /** Exact decimal string */
  accumulated: string;
  /** Exact decimal string */
  bookValue: string;
  /** One of `scheduled`, `posted` */
  status: string;
}

export interface DepreciationSchedule {
  assetId: string;
  method: string;
  entries: DepreciationEntry[];
  /** Exact decimal string */
  totalDepreciation: string;
}

export interface RevenueContractFilter {
  customerId?: string;
  /** One of `draft`, `active`, `completed`, `cancelled` */
  status?: string;
  search?: string;
  limit?: number;
  offset?: number;
}

export interface PerformanceObligation {
  id: string;
  contractId: string;
  description: string;
  /** Exact decimal string */
  allocatedAmount: string;
  /** One of `point_in_time`, `ratable_over_time`, `milestone` */
  recognitionMethod: string;
  recognitionStart?: string;
  recognitionEnd?: string;
  /** Exact decimal string */
  recognizedAmount: string;
  /** Exact decimal string: allocated_amount - recognized_amount */
  deferredAmount: string;
  createdAt: string;
  updatedAt: string;
}

export interface RevenueContract {
  id: string;
  contractNumber: string;
  customerId: string;
  orderId?: string;
  invoiceId?: string;
  /** Exact decimal string */
  transactionPrice: string;
  currency: string;
  /** One of `draft`, `active`, `completed`, `cancelled` */
  status: string;
  /** ISO date (YYYY-MM-DD) */
  effectiveDate: string;
  obligations: PerformanceObligation[];
  /** Exact decimal string: total recognized across obligations */
  totalRecognized: string;
  /** Exact decimal string: transaction_price - total_recognized */
  deferredBalance: string;
  createdAt: string;
  updatedAt: string;
}

// ============================================================================
// EDI documents (trading-partner document tracking)
// ============================================================================

/** An EDI document exchanged with a trading partner (850/855/856/810, …). */
export interface EdiDocument {
  id: string;
  /** EDI document type, e.g. `850`, `855`, `856`, `810` */
  documentType: string;
  /** One of `inbound`, `outbound` */
  direction: string;
  /** One of `pending`, `sent`, `acknowledged`, `processed`, `error` */
  status: string;
  /** Trading partner name / id */
  partner?: string;
  /** Related business reference (PO number, order number, etc.) */
  reference?: string;
  /** Raw EDI payload */
  payload?: string;
  /** Error detail when `status = error` */
  errorMessage?: string;
  createdAt: string;
  updatedAt: string;
}

export interface EdiDocumentFilter {
  documentType?: string;
  direction?: string;
  status?: string;
  partner?: string;
  limit?: number;
  offset?: number;
}

/** A keyed count used in the EDI aggregate summary. */
export interface EdiCount {
  key: string;
  count: number;
}

/** Aggregate EDI counts by status and document type. */
export interface EdiSummary {
  total: number;
  byStatus: EdiCount[];
  byType: EdiCount[];
}

/** Compute an aggregate summary from a document list (fallback for engine builds without `summary`). */
export function summarizeEdiDocuments(documents: EdiDocument[]): EdiSummary {
  const tally = (key: (doc: EdiDocument) => string): EdiCount[] => {
    const counts = new Map<string, number>();
    for (const doc of documents) {
      const k = key(doc);
      counts.set(k, (counts.get(k) || 0) + 1);
    }
    return Array.from(counts.entries())
      .map(([k, count]) => ({ key: k, count }))
      .sort((left, right) => left.key.localeCompare(right.key));
  };
  return {
    total: documents.length,
    byStatus: tally((doc) => doc.status),
    byType: tally((doc) => doc.documentType),
  };
}

type MockFinanceData = {
  glAccounts: GlAccount[];
  journalEntries: JournalEntry[];
  trialBalance: TrialBalance;
  glPeriods: GlPeriod[];
  bills: Bill[];
  apAging: ApAgingSummary;
  arAging: ArAgingSummary;
  invoices: Invoice[];
  fixedAssets: FixedAsset[];
  depreciationSchedules: Record<string, DepreciationSchedule>;
  revenueContracts: RevenueContract[];
  ediDocuments: EdiDocument[];
};

let mockFinanceCache: MockFinanceData | null = null;

function buildMockFinanceData(): MockFinanceData {
  const accountSpecs: [string, string, string, number][] = [
    ['1000', 'Cash', 'asset', 182_450.25],
    ['1100', 'Accounts Receivable', 'asset', 42_310.4],
    ['1200', 'Inventory', 'asset', 96_780.1],
    ['2000', 'Accounts Payable', 'liability', 38_920.15],
    ['2100', 'Deferred Revenue', 'liability', 12_400.0],
    ['3000', 'Retained Earnings', 'equity', 148_305.2],
    ['4000', 'Sales Revenue', 'revenue', 240_115.6],
    ['5000', 'Cost of Goods Sold', 'expense', 118_200.2],
  ];

  const glAccounts = accountSpecs.map(([accountNumber, name, accountType, balance]) => ({
    id: `gl_${accountNumber}`,
    accountNumber,
    name,
    accountType,
    balance,
    status: 'active',
    description: `${name} account`,
  }));

  const journalEntries = Array.from({ length: 12 }, (_, index) => {
    const createdAt = formatMockDate(-(index * 2) - 1, index % 8);
    return {
      id: `je_${2000 + index}`,
      entryNumber: `JE-${2000 + index}`,
      entryDate: toDateKey(createdAt),
      description:
        index % 3 === 0
          ? 'Daily sales posting'
          : index % 3 === 1
            ? 'Inventory receipt accrual'
            : 'Supplier bill accrual',
      status: index % 5 === 0 ? 'draft' : 'posted',
      createdAt,
    };
  });

  const glPeriods: GlPeriod[] = [
    {
      id: 'per_2026_01',
      periodName: '2026-01',
      fiscalYear: 2026,
      periodNumber: 1,
      startDate: '2026-01-01',
      endDate: '2026-01-31',
      status: 'closed',
      closedBy: 'system',
    },
    {
      id: 'per_2026_02',
      periodName: '2026-02',
      fiscalYear: 2026,
      periodNumber: 2,
      startDate: '2026-02-01',
      endDate: '2026-02-28',
      status: 'open',
    },
    {
      id: 'per_2026_03',
      periodName: '2026-03',
      fiscalYear: 2026,
      periodNumber: 3,
      startDate: '2026-03-01',
      endDate: '2026-03-31',
      status: 'future',
    },
  ];

  const billStatuses = ['open', 'approved', 'paid', 'overdue', 'open', 'approved'];
  const bills = Array.from({ length: 18 }, (_, index) => {
    const totalAmount = roundCurrency(320 + deterministicNumber(index + 1601, 0, 4200));
    const status = billStatuses[index % billStatuses.length];
    const amountPaid = status === 'paid' ? totalAmount : 0;
    const createdAt = formatMockDate(-(index * 4) - 2, index % 9);
    return {
      id: `bill_${3000 + index}`,
      billNumber: `BILL-${3000 + index}`,
      supplierId: `sup_${100 + (index % 5)}`,
      status,
      totalAmount,
      amountPaid,
      amountDue: roundCurrency(totalAmount - amountPaid),
      dueDate: toDateKey(formatMockDate(20 - index * 6)),
      createdAt,
    } satisfies Bill;
  });

  const openBills = bills.filter((bill) => bill.amountDue > 0);
  const bucketTotal = (predicate: (index: number) => boolean) =>
    roundCurrency(
      openBills
        .filter((_, index) => predicate(index))
        .reduce((sum, bill) => sum + bill.amountDue, 0),
    );
  const apAging: ApAgingSummary = {
    current: bucketTotal((index) => index % 5 < 2),
    days130: bucketTotal((index) => index % 5 === 2),
    days3160: bucketTotal((index) => index % 5 === 3),
    days6190: bucketTotal((index) => index % 5 === 4),
    daysOver90: 0,
    total: roundCurrency(openBills.reduce((sum, bill) => sum + bill.amountDue, 0)),
  };

  const invoiceStatuses = ['sent', 'paid', 'overdue', 'sent', 'partially_paid', 'sent'];
  const invoices = Array.from({ length: 24 }, (_, index) => {
    const subtotal = roundCurrency(480 + deterministicNumber(index + 1701, 0, 5200));
    const taxAmount = roundCurrency(subtotal * 0.08);
    const total = roundCurrency(subtotal + taxAmount);
    const status = invoiceStatuses[index % invoiceStatuses.length];
    const amountPaid =
      status === 'paid' ? total : status === 'partially_paid' ? roundCurrency(total * 0.4) : 0;
    const createdAt = formatMockDate(-(index * 7) - 3, index % 9);
    return {
      id: `inv_ar_${4000 + index}`,
      invoiceNumber: `INV-${4000 + index}`,
      customerId: `cus_${100 + (index % 6)}`,
      status,
      subtotal,
      taxAmount,
      total,
      amountPaid,
      dueDate: toDateKey(formatMockDate(25 - index * 9)),
      createdAt,
      updatedAt: createdAt,
    } satisfies Invoice;
  });

  const openInvoices = invoices.filter(
    (invoice) => invoice.status !== 'paid' && invoice.total - invoice.amountPaid > 0,
  );
  const arBucketTotal = (predicate: (daysPastDue: number) => boolean) =>
    roundCurrency(
      openInvoices.reduce((sum, invoice) => {
        const daysPastDue = Math.floor((MOCK_NOW - Date.parse(invoice.dueDate)) / DAY_MS);
        return predicate(daysPastDue) ? sum + (invoice.total - invoice.amountPaid) : sum;
      }, 0),
    );
  const arAging: ArAgingSummary = {
    current: arBucketTotal((days) => days <= 0),
    days130: arBucketTotal((days) => days >= 1 && days <= 30),
    days3160: arBucketTotal((days) => days >= 31 && days <= 60),
    days6190: arBucketTotal((days) => days >= 61 && days <= 90),
    daysOver90: arBucketTotal((days) => days > 90),
    total: roundCurrency(
      openInvoices.reduce((sum, invoice) => sum + (invoice.total - invoice.amountPaid), 0),
    ),
  };

  const assetSpecs: [string, string, string, string, number, string, string][] = [
    // name, category, cost, salvage, life months, status, accumulated
    ['Forklift A', 'equipment', '42000.00', '2000.00', 120, 'in_service', '14000.00'],
    ['Warehouse racking', 'fixtures', '18500.00', '500.00', 84, 'in_service', '5285.71'],
    ['Delivery van', 'vehicles', '38000.00', '4000.00', 60, 'in_service', '17000.00'],
    ['Packing line', 'equipment', '96000.00', '6000.00', 96, 'draft', '0.00'],
    ['Office laptops (batch 3)', 'it', '12400.00', '400.00', 36, 'fully_depreciated', '12000.00'],
    ['Old conveyor', 'equipment', '25000.00', '1000.00', 72, 'disposed', '21500.00'],
  ];
  const fixedAssets = assetSpecs.map(
    (
      [
        name,
        category,
        acquisitionCost,
        salvageValue,
        usefulLifeMonths,
        status,
        accumulatedDepreciation,
      ],
      index,
    ) => {
      const createdAt = formatMockDate(-400 + index * 30, index % 8);
      const bookValue = (Number(acquisitionCost) - Number(accumulatedDepreciation)).toFixed(2);
      return {
        id: `fa_${5000 + index}`,
        assetNumber: `FA-${5000 + index}`,
        name,
        description: `${name} (demo asset)`,
        category,
        acquisitionDate: toDateKey(createdAt),
        acquisitionCost,
        salvageValue,
        usefulLifeMonths,
        depreciationMethod: index % 3 === 2 ? 'declining_balance' : 'straight_line',
        status,
        inServiceDate: status === 'draft' ? undefined : toDateKey(createdAt),
        accumulatedDepreciation,
        bookValue,
        currency: 'USD',
        createdAt,
        updatedAt: formatMockDate(-(index % 12), index % 6),
      } satisfies FixedAsset;
    },
  );

  const depreciationSchedules: Record<string, DepreciationSchedule> = {};
  for (const asset of fixedAssets) {
    if (asset.status === 'draft') {
      continue;
    }
    const depreciable = Number(asset.acquisitionCost) - Number(asset.salvageValue);
    const monthly = depreciable / asset.usefulLifeMonths;
    const periods = Math.min(asset.usefulLifeMonths, 12);
    let accumulated = 0;
    const entries = Array.from({ length: periods }, (_, period) => {
      accumulated += monthly;
      return {
        period: period + 1,
        amount: monthly.toFixed(2),
        accumulated: accumulated.toFixed(2),
        bookValue: (Number(asset.acquisitionCost) - accumulated).toFixed(2),
        status: accumulated <= Number(asset.accumulatedDepreciation) ? 'posted' : 'scheduled',
      } satisfies DepreciationEntry;
    });
    depreciationSchedules[asset.id] = {
      assetId: asset.id,
      method: asset.depreciationMethod,
      entries,
      totalDepreciation: accumulated.toFixed(2),
    };
  }

  const contractStatuses = ['active', 'active', 'draft', 'completed', 'cancelled'];
  const revenueContracts = Array.from({ length: 8 }, (_, index) => {
    const status = contractStatuses[index % contractStatuses.length];
    const first = roundCurrency(2400 + deterministicNumber(index + 1801, 0, 8600));
    const second = roundCurrency(900 + deterministicNumber(index + 1901, 0, 2400));
    const transactionPrice = roundCurrency(first + second);
    const firstRecognized =
      status === 'completed' ? first : status === 'active' ? roundCurrency(first * 0.5) : 0;
    const secondRecognized = status === 'completed' ? second : 0;
    const totalRecognized = roundCurrency(firstRecognized + secondRecognized);
    const createdAt = formatMockDate(-200 + index * 20, index % 7);
    const contractId = `rc_${6000 + index}`;
    const obligation = (
      suffix: string,
      description: string,
      allocatedAmount: number,
      recognizedAmount: number,
      recognitionMethod: string,
    ): PerformanceObligation => ({
      id: `${contractId}_ob_${suffix}`,
      contractId,
      description,
      allocatedAmount: allocatedAmount.toFixed(2),
      recognitionMethod,
      recognitionStart:
        recognitionMethod === 'ratable_over_time' ? toDateKey(createdAt) : undefined,
      recognitionEnd:
        recognitionMethod === 'ratable_over_time'
          ? toDateKey(formatMockDate(-200 + index * 20 + 365))
          : undefined,
      recognizedAmount: recognizedAmount.toFixed(2),
      deferredAmount: roundCurrency(allocatedAmount - recognizedAmount).toFixed(2),
      createdAt,
      updatedAt: createdAt,
    });
    return {
      id: contractId,
      contractNumber: `RC-${6000 + index}`,
      customerId: `cus_${100 + (index % 6)}`,
      transactionPrice: transactionPrice.toFixed(2),
      currency: 'USD',
      status,
      effectiveDate: toDateKey(createdAt),
      obligations: [
        obligation('1', 'Platform subscription', first, firstRecognized, 'ratable_over_time'),
        obligation('2', 'Onboarding services', second, secondRecognized, 'point_in_time'),
      ],
      totalRecognized: totalRecognized.toFixed(2),
      deferredBalance: roundCurrency(transactionPrice - totalRecognized).toFixed(2),
      createdAt,
      updatedAt: createdAt,
    } satisfies RevenueContract;
  });

  const ediTypes = ['850', '855', '856', '810'];
  const ediStatusPool = ['processed', 'processed', 'acknowledged', 'sent', 'pending', 'error'];
  const ediPartners = ['ACME-RETAIL', 'NORTHWIND', 'GLOBEX', 'INITECH'];
  const ediDocuments: EdiDocument[] = Array.from({ length: 16 }, (_, index) => {
    const documentType = ediTypes[index % ediTypes.length];
    const status = ediStatusPool[index % ediStatusPool.length];
    // Inbound purchase orders (850); outbound acks/ASNs/invoices.
    const direction = documentType === '850' ? 'inbound' : 'outbound';
    const createdAt = formatMockDate(-(index * 2) - 1, index % 10);
    return {
      id: `edi_${4000 + index}`,
      documentType,
      direction,
      status,
      partner: ediPartners[index % ediPartners.length],
      reference: `PO-${7000 + Math.floor(index / ediTypes.length)}`,
      errorMessage: status === 'error' ? 'Missing mandatory segment: BEG' : undefined,
      createdAt,
      updatedAt: createdAt,
    } satisfies EdiDocument;
  });

  return {
    glAccounts,
    journalEntries,
    trialBalance: {
      asOfDate: toDateKey(formatMockDate(0)),
      totalDebits: 439_745.95,
      totalCredits: 439_745.95,
      isBalanced: true,
    },
    glPeriods,
    bills,
    apAging,
    arAging,
    invoices,
    fixedAssets,
    depreciationSchedules,
    revenueContracts,
    ediDocuments,
  };
}

function getMockFinanceData(): MockFinanceData {
  if (!mockFinanceCache) {
    mockFinanceCache = buildMockFinanceData();
  }
  return mockFinanceCache;
}

function buildMockCloseMonthReport(
  periodId: string,
  options?: CloseMonthOptions,
): CloseMonthReport {
  const dryRun = options?.dryRun === true;
  const period = getMockFinanceData().glPeriods.find((entry) => entry.id === periodId);
  const stepStatus = dryRun ? 'dry_run' : 'executed';
  const step = (
    entryCount: number,
    totalAmount: string,
    warnings: string[] = [],
  ): CloseMonthStep => ({
    status: stepStatus,
    entryCount,
    totalAmount,
    warnings,
  });

  return {
    periodId,
    periodName: period?.periodName || periodId,
    dryRun,
    depreciation: step(3, '1250.00'),
    revenueRecognition: step(2, '840.50'),
    fxRevaluation: step(0, '0.00', ['No foreign-currency balances to revalue']),
    periodClose: step(1, '12480.75'),
    closingEntry: dryRun
      ? undefined
      : {
          id: 'je_close_1',
          entryNumber: 'JE-CLOSE-1',
          entryDate: period?.endDate || toDateKey(formatMockDate(0)),
          description: `Closing entries for ${period?.periodName || periodId}`,
          status: 'posted',
          createdAt: new Date().toISOString(),
        },
    periodStatus: dryRun ? period?.status || 'open' : 'closed',
  };
}

export const generalLedgerApi = {
  async listAccounts(): Promise<GlAccount[]> {
    const commerce = await getCommerceEngine();
    return commerce.generalLedger.listAccounts();
  },

  async getTrialBalance(asOfDate: string): Promise<TrialBalance> {
    const commerce = await getCommerceEngine();
    return commerce.generalLedger.getTrialBalance(asOfDate);
  },

  async listJournalEntries(): Promise<JournalEntry[]> {
    const commerce = await getCommerceEngine();
    return commerce.generalLedger.listJournalEntries();
  },

  async listPeriods(): Promise<GlPeriod[]> {
    const commerce = await getCommerceEngine();
    // Not every engine build exposes period listing yet; degrade to an
    // empty list so the close page can explain rather than crash.
    if (typeof commerce.generalLedger.listPeriods !== 'function') {
      return [];
    }
    return commerce.generalLedger.listPeriods();
  },

  async closeMonth(periodId: string, options?: CloseMonthOptions): Promise<CloseMonthReport> {
    const commerce = await getCommerceEngine();
    return commerce.generalLedger.closeMonth(periodId, options);
  },
};

export const accountsPayableApi = {
  async listBills(): Promise<Bill[]> {
    const commerce = await getCommerceEngine();
    return commerce.accountsPayable.listBills();
  },

  async getAgingSummary(): Promise<ApAgingSummary> {
    const commerce = await getCommerceEngine();
    return commerce.accountsPayable.getAgingSummary();
  },
};

export const accountsReceivableApi = {
  async getAgingSummary(): Promise<ArAgingSummary> {
    const commerce = await getCommerceEngine();
    return commerce.accountsReceivable.getAgingSummary();
  },

  /**
   * Days Sales Outstanding over the trailing `days` window. Not every engine
   * build exposes DSO yet; degrade to `null` so the page can hide the stat
   * rather than crash.
   */
  async getDso(days: number): Promise<number | null> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.accountsReceivable.getDso !== 'function') {
      return null;
    }
    return commerce.accountsReceivable.getDso(days);
  },

  async listInvoices(): Promise<Invoice[]> {
    const commerce = await getCommerceEngine();
    return commerce.invoices.list();
  },
};

export const fixedAssetsApi = {
  async list(filter?: FixedAssetFilter): Promise<FixedAsset[]> {
    const commerce = await getCommerceEngine();
    return commerce.fixedAssets.list(filter);
  },

  async getSchedule(assetId: string): Promise<DepreciationSchedule | null> {
    const commerce = await getCommerceEngine();
    return commerce.fixedAssets.getSchedule(assetId);
  },
};

export const revenueRecognitionApi = {
  async listContracts(filter?: RevenueContractFilter): Promise<RevenueContract[]> {
    const commerce = await getCommerceEngine();
    return commerce.revenueRecognition.listContracts(filter);
  },
};

export const ediDocumentsApi = {
  /**
   * List EDI documents. Older engine builds do not expose the EDI surface;
   * degrade to an empty list so the operations page can explain rather
   * than crash.
   */
  async list(filter?: EdiDocumentFilter): Promise<EdiDocument[]> {
    const commerce = await getCommerceEngine();
    if (!commerce.ediDocuments || typeof commerce.ediDocuments.list !== 'function') {
      return [];
    }
    return commerce.ediDocuments.list(filter);
  },

  async get(id: string): Promise<EdiDocument | null> {
    const commerce = await getCommerceEngine();
    if (!commerce.ediDocuments || typeof commerce.ediDocuments.get !== 'function') {
      return null;
    }
    return commerce.ediDocuments.get(id);
  },

  /**
   * Aggregate counts by status and document type. Uses the engine's summary
   * accessor when present, otherwise computes from the document list.
   */
  async summary(): Promise<EdiSummary> {
    const commerce = await getCommerceEngine();
    if (!commerce.ediDocuments) {
      return summarizeEdiDocuments([]);
    }
    if (typeof commerce.ediDocuments.summary === 'function') {
      return commerce.ediDocuments.summary();
    }
    return summarizeEdiDocuments(await commerce.ediDocuments.list());
  },
};

// ============================================================================
// Operations domains (purchasing, warehouse, manufacturing) — read-only
//
// Types mirror the napi binding outputs in bindings/node/index.d.ts. Amounts
// are passed through untouched; formatting is display-only in components.
// ============================================================================

/** A purchase order header (`commerce.purchaseOrders.list`). */
export interface PurchaseOrder {
  id: string;
  poNumber: string;
  supplierId: string;
  status: string;
  subtotal: number;
  total: number;
  createdAt: string;
  updatedAt: string;
}

/** A supplier record (`commerce.purchaseOrders.listSuppliers`). */
export interface Supplier {
  id: string;
  name: string;
  supplierCode?: string;
  email?: string;
  phone?: string;
  isActive: boolean;
  createdAt: string;
}

/** A warehouse (`commerce.warehouse.listWarehouses`). */
export interface WarehouseRecord {
  id: number;
  code: string;
  name: string;
  warehouseType: string;
  isActive: boolean;
  timezone?: string;
  createdAt: string;
}

/** A storage location inside a warehouse (`commerce.warehouse.listLocations`). */
export interface WarehouseLocation {
  id: number;
  warehouseId: number;
  code: string;
  locationType: string;
  zone?: string;
  aisle?: string;
  rack?: string;
  bin?: string;
  isActive: boolean;
  isPickable: boolean;
  isReceivable: boolean;
}

/** One counted line on a cycle count. Quantities are exact decimal strings. */
export interface CycleCountLine {
  id: string;
  cycleCountId: string;
  sku: string;
  lotId?: string;
  expectedQuantity: string;
  countedQuantity?: string;
  variance?: string;
}

/** A cycle count (`commerce.cycleCounts.list`). */
export interface CycleCount {
  id: string;
  warehouseId: number;
  locationId?: number;
  /** draft, in_progress, completed, cancelled */
  status: string;
  scheduledDate?: string;
  countedBy?: string;
  lines: CycleCountLine[];
  createdAt: string;
  updatedAt: string;
  completedAt?: string;
}

/** Filter accepted by `commerce.cycleCounts.list`. */
export interface CycleCountFilter {
  warehouseId?: number;
  status?: string;
  limit?: number;
  offset?: number;
}

/** A manufacturing work order (`commerce.workOrders.list`). */
export interface WorkOrder {
  id: string;
  workOrderNumber: string;
  productId: string;
  bomId?: string;
  status: string;
  priority: string;
  quantityToBuild: number;
  quantityCompleted: number;
  version: number;
  createdAt: string;
  updatedAt: string;
}

/** A quality inspection (`commerce.quality.listInspections`). */
export interface QualityInspection {
  id: string;
  inspectionNumber: string;
  inspectionType: string;
  referenceType: string;
  referenceId: string;
  status: string;
  createdAt: string;
  updatedAt: string;
}

/** A non-conformance report (`commerce.quality.listNcrs`). */
export interface NonConformanceReport {
  id: string;
  ncrNumber: string;
  source: string;
  severity: string;
  sku: string;
  quantityAffected: number;
  status: string;
  description: string;
  createdAt: string;
}

/** A fulfillment wave (`commerce.fulfillment.listWaves`). */
export interface Wave {
  id: string;
  waveNumber: string;
  warehouseId: number;
  orderCount: number;
  /** created, released, picking, completed, cancelled */
  status: string;
  createdAt: string;
}

/** A pick task (`commerce.fulfillment.listPicks`). */
export interface PickTask {
  id: string;
  waveId?: string;
  orderId: string;
  sku: string;
  quantityRequested: number;
  quantityPicked: number;
  /** pending, assigned, in_progress, picked, cancelled */
  status: string;
  sourceLocationId: number;
  assignedTo?: string;
}

/** A production/inventory lot (`commerce.lots.list`). */
export interface Lot {
  id: string;
  lotNumber: string;
  sku: string;
  quantityProduced: number;
  quantityAvailable: number;
  quantityReserved: number;
  /** active, quarantined, expired, consumed */
  status: string;
  productionDate?: string;
  expirationDate?: string;
  createdAt: string;
}

/** A serial number (`commerce.serials.list`). */
export interface SerialNumber {
  id: string;
  serial: string;
  sku: string;
  lotId?: string;
  /** available, allocated, sold, quarantined */
  status: string;
  ownerId?: string;
  locationId?: number;
  createdAt: string;
}

/** An inbound receipt (`commerce.receiving.listReceipts`). */
export interface Receipt {
  id: string;
  receiptNumber: string;
  receiptType: string;
  warehouseId: number;
  /** pending, receiving, completed, cancelled */
  status: string;
  carrier?: string;
  trackingNumber?: string;
  createdAt: string;
}

type MockOperationsData = {
  waves: Wave[];
  picks: PickTask[];
  lots: Lot[];
  serials: SerialNumber[];
  receipts: Receipt[];
  purchaseOrders: PurchaseOrder[];
  suppliers: Supplier[];
  warehouses: WarehouseRecord[];
  locations: WarehouseLocation[];
  cycleCounts: CycleCount[];
  workOrders: WorkOrder[];
  inspections: QualityInspection[];
  ncrs: NonConformanceReport[];
};

let mockOperationsCache: MockOperationsData | null = null;

function buildMockOperationsData(): MockOperationsData {
  const supplierSpecs: [string, string, string, boolean][] = [
    ['Northwind Components', 'SUP-NWC', 'orders@northwind.example', true],
    ['Globex Packaging', 'SUP-GLX', 'ap@globex.example', true],
    ['Initech Electronics', 'SUP-INI', 'sales@initech.example', true],
    ['Acme Raw Materials', 'SUP-ACM', 'hello@acme.example', true],
    ['Umbrella Logistics', 'SUP-UMB', 'ops@umbrella.example', false],
  ];
  const suppliers = supplierSpecs.map(
    ([name, supplierCode, email, isActive], index) =>
      ({
        id: `sup_${100 + index}`,
        name,
        supplierCode,
        email,
        phone: `+1-555-01${(10 + index).toString().padStart(2, '0')}`,
        isActive,
        createdAt: formatMockDate(-300 + index * 12, index % 9),
      }) satisfies Supplier,
  );

  const poStatuses = ['draft', 'submitted', 'approved', 'sent', 'received', 'cancelled'];
  const purchaseOrders = Array.from({ length: 20 }, (_, index) => {
    const subtotal = roundCurrency(750 + deterministicNumber(index + 2101, 0, 9400));
    const createdAt = formatMockDate(-(index * 3) - 1, index % 11);
    return {
      id: `po_${7000 + index}`,
      poNumber: `PO-${7000 + index}`,
      supplierId: suppliers[index % suppliers.length].id,
      status: poStatuses[index % poStatuses.length],
      subtotal,
      total: roundCurrency(subtotal * 1.07),
      createdAt,
      updatedAt: createdAt,
    } satisfies PurchaseOrder;
  });

  const warehouseSpecs: [string, string, string][] = [
    ['WH-MAIN', 'Reno Distribution Center', 'distribution'],
    ['WH-EAST', 'Columbus Fulfillment', 'fulfillment'],
    ['WH-MFG', 'Fremont Manufacturing', 'manufacturing'],
  ];
  const warehouses = warehouseSpecs.map(
    ([code, name, warehouseType], index) =>
      ({
        id: index + 1,
        code,
        name,
        warehouseType,
        isActive: true,
        timezone: index === 1 ? 'America/New_York' : 'America/Los_Angeles',
        createdAt: formatMockDate(-500 + index * 40, index % 7),
      }) satisfies WarehouseRecord,
  );

  const locationTypes = ['bin', 'bulk', 'staging', 'receiving'];
  const locations: WarehouseLocation[] = [];
  let locationId = 0;
  for (const warehouse of warehouses) {
    for (let index = 0; index < 8; index += 1) {
      locationId += 1;
      const zone = String.fromCharCode(65 + (index % 3));
      const aisle = `${1 + (index % 4)}`;
      const rack = `${1 + (index % 2)}`;
      const bin = (index + 1).toString().padStart(2, '0');
      const locationType = locationTypes[index % locationTypes.length];
      locations.push({
        id: locationId,
        warehouseId: warehouse.id,
        code: `${warehouse.code}-${zone}${aisle}-${rack}${bin}`,
        locationType,
        zone,
        aisle,
        rack,
        bin,
        isActive: true,
        isPickable: locationType !== 'receiving',
        isReceivable: locationType === 'receiving' || locationType === 'staging',
      });
    }
  }

  const cycleCountStatuses = ['draft', 'in_progress', 'completed', 'completed', 'cancelled'];
  const cycleCounts = Array.from({ length: 12 }, (_, index) => {
    const status = cycleCountStatuses[index % cycleCountStatuses.length];
    const warehouse = warehouses[index % warehouses.length];
    const createdAt = formatMockDate(-(index * 5) - 2, index % 10);
    const id = `cc_${8000 + index}`;
    const lines: CycleCountLine[] = Array.from({ length: 3 }, (_, line) => {
      const expected = 40 + deterministicNumber(index * 10 + line + 2201, 0, 160);
      const counted = status === 'completed' ? expected - (line === 1 ? 2 : 0) : undefined;
      return {
        id: `${id}_line_${line + 1}`,
        cycleCountId: id,
        sku: `SKU-${1000 + ((index * 3 + line) % 40)}`,
        expectedQuantity: expected.toFixed(2),
        countedQuantity: counted === undefined ? undefined : counted.toFixed(2),
        variance: counted === undefined ? undefined : (counted - expected).toFixed(2),
      };
    });
    return {
      id,
      warehouseId: warehouse.id,
      locationId: locations[index % locations.length].id,
      status,
      scheduledDate: toDateKey(createdAt),
      countedBy: status === 'draft' ? undefined : `operator_${1 + (index % 3)}`,
      lines,
      createdAt,
      updatedAt: createdAt,
      completedAt:
        status === 'completed' ? formatMockDate(-(index * 5) - 1, index % 10) : undefined,
    } satisfies CycleCount;
  });

  const workOrderStatuses = ['draft', 'released', 'in_progress', 'completed', 'cancelled'];
  const priorities = ['low', 'normal', 'high', 'urgent'];
  const workOrders = Array.from({ length: 16 }, (_, index) => {
    const status = workOrderStatuses[index % workOrderStatuses.length];
    const quantityToBuild = 25 + deterministicNumber(index + 2301, 0, 475);
    const quantityCompleted =
      status === 'completed'
        ? quantityToBuild
        : status === 'in_progress'
          ? Math.floor(quantityToBuild / 2)
          : 0;
    const createdAt = formatMockDate(-(index * 4) - 1, index % 12);
    return {
      id: `wo_${9000 + index}`,
      workOrderNumber: `WO-${9000 + index}`,
      productId: `prod_${200 + (index % 8)}`,
      bomId: `bom_${300 + (index % 5)}`,
      status,
      priority: priorities[index % priorities.length],
      quantityToBuild,
      quantityCompleted,
      version: 1,
      createdAt,
      updatedAt: createdAt,
    } satisfies WorkOrder;
  });

  const inspectionTypes = ['incoming', 'in_process', 'final', 'return'];
  const inspectionStatuses = ['pending', 'in_progress', 'passed', 'failed'];
  const inspections = Array.from({ length: 14 }, (_, index) => {
    const createdAt = formatMockDate(-(index * 3) - 1, index % 9);
    return {
      id: `insp_${9500 + index}`,
      inspectionNumber: `QI-${9500 + index}`,
      inspectionType: inspectionTypes[index % inspectionTypes.length],
      referenceType: index % 2 === 0 ? 'work_order' : 'purchase_order',
      referenceId: index % 2 === 0 ? `wo_${9000 + (index % 16)}` : `po_${7000 + (index % 20)}`,
      status: inspectionStatuses[index % inspectionStatuses.length],
      createdAt,
      updatedAt: createdAt,
    } satisfies QualityInspection;
  });

  const severities = ['minor', 'major', 'critical'];
  const ncrStatuses = ['open', 'under_review', 'closed'];
  const ncrs = Array.from(
    { length: 9 },
    (_, index) =>
      ({
        id: `ncr_${9700 + index}`,
        ncrNumber: `NCR-${9700 + index}`,
        source: index % 2 === 0 ? 'inspection' : 'customer_return',
        severity: severities[index % severities.length],
        sku: `SKU-${1000 + (index % 40)}`,
        quantityAffected: 1 + deterministicNumber(index + 2401, 0, 60),
        status: ncrStatuses[index % ncrStatuses.length],
        description: 'Dimensional tolerance out of spec on inbound lot',
        createdAt: formatMockDate(-(index * 6) - 2, index % 8),
      }) satisfies NonConformanceReport,
  );

  const waveStatuses = ['created', 'released', 'picking', 'completed', 'cancelled'];
  const waves = Array.from(
    { length: 10 },
    (_, index) =>
      ({
        id: `wave_${4000 + index}`,
        waveNumber: `WAVE-${4000 + index}`,
        warehouseId: warehouses[index % warehouses.length].id,
        orderCount: 3 + deterministicNumber(index + 2501, 0, 22),
        status: waveStatuses[index % waveStatuses.length],
        createdAt: formatMockDate(-(index * 2) - 1, index % 10),
      }) satisfies Wave,
  );

  const pickStatuses = ['pending', 'assigned', 'in_progress', 'picked', 'cancelled'];
  const picks = Array.from({ length: 24 }, (_, index) => {
    const status = pickStatuses[index % pickStatuses.length];
    const quantityRequested = 1 + deterministicNumber(index + 2601, 0, 24);
    const quantityPicked =
      status === 'picked'
        ? quantityRequested
        : status === 'in_progress'
          ? Math.floor(quantityRequested / 2)
          : 0;
    return {
      id: `pick_${5000 + index}`,
      waveId: waves[index % waves.length].id,
      orderId: `ord_${6000 + (index % 18)}`,
      sku: `SKU-${1000 + (index % 40)}`,
      quantityRequested,
      quantityPicked,
      status,
      sourceLocationId: locations[index % locations.length].id,
      assignedTo: status === 'pending' ? undefined : `picker_${1 + (index % 4)}`,
    } satisfies PickTask;
  });

  const lotStatuses = ['active', 'active', 'quarantined', 'expired', 'consumed'];
  const lots = Array.from({ length: 18 }, (_, index) => {
    const status = lotStatuses[index % lotStatuses.length];
    const quantityProduced = 100 + deterministicNumber(index + 2701, 0, 900);
    const quantityReserved = deterministicNumber(index + 2801, 0, 40);
    // Spread expirations across expired / near-expiry / far-out buckets.
    const expiryOffset = status === 'expired' ? -(5 + (index % 20)) : (index % 6) * 9 + 2;
    const createdAt = formatMockDate(-(index * 7) - 3, index % 11);
    return {
      id: `lot_${3000 + index}`,
      lotNumber: `LOT-${3000 + index}`,
      sku: `SKU-${1000 + (index % 40)}`,
      quantityProduced,
      quantityAvailable:
        status === 'consumed' ? 0 : Math.max(0, quantityProduced - quantityReserved),
      quantityReserved: status === 'consumed' ? 0 : quantityReserved,
      status,
      productionDate: toDateKey(createdAt),
      expirationDate: toDateKey(formatMockDate(expiryOffset)),
      createdAt,
    } satisfies Lot;
  });

  const serialStatuses = ['available', 'allocated', 'sold', 'quarantined'];
  const serials = Array.from({ length: 20 }, (_, index) => {
    const status = serialStatuses[index % serialStatuses.length];
    return {
      id: `ser_${2000 + index}`,
      serial: `SN-${(2000 + index).toString().padStart(8, '0')}`,
      sku: `SKU-${1000 + (index % 40)}`,
      lotId: lots[index % lots.length].id,
      status,
      ownerId: status === 'sold' ? `cus_${400 + (index % 12)}` : undefined,
      locationId: status === 'sold' ? undefined : locations[index % locations.length].id,
      createdAt: formatMockDate(-(index * 4) - 2, index % 9),
    } satisfies SerialNumber;
  });

  const receiptStatuses = ['pending', 'receiving', 'completed', 'completed', 'cancelled'];
  const receiptTypes = ['purchase_order', 'return', 'transfer'];
  const carriers = ['UPS', 'FedEx', 'DHL'];
  const receipts = Array.from(
    { length: 12 },
    (_, index) =>
      ({
        id: `rcpt_${1500 + index}`,
        receiptNumber: `RCV-${1500 + index}`,
        receiptType: receiptTypes[index % receiptTypes.length],
        warehouseId: warehouses[index % warehouses.length].id,
        status: receiptStatuses[index % receiptStatuses.length],
        carrier: carriers[index % carriers.length],
        trackingNumber: `1Z${(90000 + index * 137).toString()}`,
        createdAt: formatMockDate(-(index * 3) - 1, index % 8),
      }) satisfies Receipt,
  );

  return {
    waves,
    picks,
    lots,
    serials,
    receipts,
    purchaseOrders,
    suppliers,
    warehouses,
    locations,
    cycleCounts,
    workOrders,
    inspections,
    ncrs,
  };
}

function getMockOperationsData(): MockOperationsData {
  if (!mockOperationsCache) {
    mockOperationsCache = buildMockOperationsData();
  }
  return mockOperationsCache;
}

/**
 * Mock implementations of the operations engine sections. Split out so the
 * mock engine factory stays readable.
 */
function createMockOperationsSections(): Pick<
  CommerceEngine,
  | 'purchaseOrders'
  | 'warehouse'
  | 'cycleCounts'
  | 'workOrders'
  | 'quality'
  | 'fulfillment'
  | 'lots'
  | 'serials'
  | 'receiving'
> {
  return {
    purchaseOrders: {
      list: async () => getMockOperationsData().purchaseOrders,
      get: async (id: string) =>
        getMockOperationsData().purchaseOrders.find((po) => po.id === id) || null,
      listSuppliers: async () => getMockOperationsData().suppliers,
    },
    warehouse: {
      listWarehouses: async () => getMockOperationsData().warehouses,
      listLocations: async (warehouseId?: number) => {
        const { locations } = getMockOperationsData();
        return warehouseId === undefined
          ? locations
          : locations.filter((location) => location.warehouseId === warehouseId);
      },
    },
    cycleCounts: {
      list: async (filter?: CycleCountFilter) => {
        let counts = getMockOperationsData().cycleCounts;
        if (filter?.warehouseId !== undefined) {
          counts = counts.filter((count) => count.warehouseId === filter.warehouseId);
        }
        if (filter?.status) {
          counts = counts.filter((count) => count.status === filter.status);
        }
        const offset = filter?.offset || 0;
        const limit = filter?.limit;
        return counts.slice(offset, limit === undefined ? undefined : offset + limit);
      },
    },
    workOrders: {
      list: async () => getMockOperationsData().workOrders,
      get: async (id: string) =>
        getMockOperationsData().workOrders.find((order) => order.id === id) || null,
    },
    quality: {
      listInspections: async () => getMockOperationsData().inspections,
      listNcrs: async () => getMockOperationsData().ncrs,
    },
    fulfillment: {
      listWaves: async () => getMockOperationsData().waves,
      listPicks: async () => getMockOperationsData().picks,
    },
    lots: {
      list: async () => getMockOperationsData().lots,
    },
    serials: {
      list: async () => getMockOperationsData().serials,
    },
    receiving: {
      listReceipts: async () => getMockOperationsData().receipts,
    },
  };
}

/**
 * Purchasing accessors. Older engine builds do not expose the purchasing
 * surface; those degrade to empty results so the page can explain rather
 * than crash (same contract as `ediDocumentsApi`).
 */
export const purchaseOrdersApi = {
  async list(): Promise<PurchaseOrder[]> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.purchaseOrders?.list !== 'function') {
      return [];
    }
    return commerce.purchaseOrders.list();
  },

  async get(id: string): Promise<PurchaseOrder | null> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.purchaseOrders?.get !== 'function') {
      return null;
    }
    return commerce.purchaseOrders.get(id);
  },

  async listSuppliers(): Promise<Supplier[]> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.purchaseOrders?.listSuppliers !== 'function') {
      return [];
    }
    return commerce.purchaseOrders.listSuppliers();
  },
};

/** Warehouse + location accessors. Degrade to empty lists on older builds. */
export const warehouseApi = {
  async listWarehouses(): Promise<WarehouseRecord[]> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.warehouse?.listWarehouses !== 'function') {
      return [];
    }
    return commerce.warehouse.listWarehouses();
  },

  async listLocations(warehouseId?: number): Promise<WarehouseLocation[]> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.warehouse?.listLocations !== 'function') {
      return [];
    }
    return commerce.warehouse.listLocations(warehouseId);
  },
};

/** Cycle-count accessors. Degrade to an empty list on older builds. */
export const cycleCountsApi = {
  async list(filter?: CycleCountFilter): Promise<CycleCount[]> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.cycleCounts?.list !== 'function') {
      return [];
    }
    return commerce.cycleCounts.list(filter);
  },
};

/** Work-order accessors. Degrade to empty results on older builds. */
export const workOrdersApi = {
  async list(): Promise<WorkOrder[]> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.workOrders?.list !== 'function') {
      return [];
    }
    return commerce.workOrders.list();
  },

  async get(id: string): Promise<WorkOrder | null> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.workOrders?.get !== 'function') {
      return null;
    }
    return commerce.workOrders.get(id);
  },
};

/** Quality accessors (inspections + NCRs). Degrade to empty lists. */
export const qualityApi = {
  async listInspections(): Promise<QualityInspection[]> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.quality?.listInspections !== 'function') {
      return [];
    }
    return commerce.quality.listInspections();
  },

  async listNcrs(): Promise<NonConformanceReport[]> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.quality?.listNcrs !== 'function') {
      return [];
    }
    return commerce.quality.listNcrs();
  },
};

/**
 * Fulfillment accessors (waves + pick tasks). Pack/ship task listing is not
 * exposed by the napi binding yet, so the page reports on waves and picks
 * only. Degrades to empty lists on older builds.
 */
export const fulfillmentApi = {
  async listWaves(): Promise<Wave[]> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.fulfillment?.listWaves !== 'function') {
      return [];
    }
    return commerce.fulfillment.listWaves();
  },

  async listPicks(): Promise<PickTask[]> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.fulfillment?.listPicks !== 'function') {
      return [];
    }
    return commerce.fulfillment.listPicks();
  },
};

/** Lot accessors. Degrade to an empty list on older builds. */
export const lotsApi = {
  async list(): Promise<Lot[]> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.lots?.list !== 'function') {
      return [];
    }
    return commerce.lots.list();
  },
};

/** Serial-number accessors. Degrade to an empty list on older builds. */
export const serialsApi = {
  async list(): Promise<SerialNumber[]> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.serials?.list !== 'function') {
      return [];
    }
    return commerce.serials.list();
  },
};

/** Receiving accessors. Degrade to an empty list on older builds. */
export const receivingApi = {
  async listReceipts(): Promise<Receipt[]> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.receiving?.listReceipts !== 'function') {
      return [];
    }
    return commerce.receiving.listReceipts();
  },
};

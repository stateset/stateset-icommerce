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

import {
  AP_AGING_EXCLUDED_BILL_STATUSES,
  AR_OPEN_INVOICE_STATUSES,
  ORDER_LIFECYCLE,
  RETURN_LIFECYCLE,
  RETURN_REASONS,
  SUBSCRIPTION_ACTION_TARGETS,
  canApplySubscriptionAction,
  canTransition,
  normalizeEngineStatus,
  statusIn,
  type BillStatus,
  type OrderStatusUpdate,
  type ReturnReason,
  type ReturnStatus,
  type SubscriptionAction,
  type GlAccountStatus,
  type InspectionStatus,
  type InvoiceStatus,
  type JournalEntryStatus,
  type LotStatus,
  type NcrSeverity,
  type NcrStatus,
  type PickTaskStatus,
  type PurchaseOrderStatus,
  type ReceiptStatus,
  type SerialStatus,
  type WaveStatus,
  type WorkOrderStatus,
} from './engine-lifecycles';
import {
  decimalScale,
  divideDecimal,
  multiplyDecimal,
  sumDecimals,
  toDecimalString,
} from './finance/decimal';
import type {
  Address,
  AnalyticsCoverage,
  Customer,
  CustomerAnalytics,
  DashboardMetrics,
  HourlyActivity,
  InventoryAnalytics,
  InventoryItem,
  Order,
  OrderAnalytics,
  OrderItem,
  Product,
  ProductVariant,
  Return,
  ReturnAnalytics,
  ReturnItem,
  Subscription,
  SubscriptionAnalytics,
  SystemHealth,
} from './types';

// The commerce record shapes are declared once, in `./types` (client-safe),
// and re-exported here so server code can keep importing them from the
// engine module. Their status unions derive from the native binding via
// `./engine-lifecycles`; nothing in this file redeclares them.
export type {
  Address,
  AnalyticsCoverage,
  Customer,
  CustomerAnalytics,
  DashboardMetrics,
  HourlyActivity,
  InventoryAnalytics,
  InventoryItem,
  Order,
  OrderAnalytics,
  OrderItem,
  Product,
  ProductVariant,
  Return,
  ReturnAnalytics,
  ReturnItem,
  Subscription,
  SubscriptionAnalytics,
  SystemHealth,
};

/**
 * Thrown when the connected engine build has no equivalent for an admin
 * feature (the native binding lacks several methods the admin's mock offers).
 * Server actions catch it and hand the page an explanation instead of
 * crashing with `x is not a function`.
 */
export class EngineCapabilityError extends Error {
  readonly code = 'ENGINE_CAPABILITY_UNAVAILABLE';

  constructor(
    readonly feature: string,
    explanation: string,
  ) {
    super(`${explanation} (${feature} is not available on this engine build)`);
    this.name = 'EngineCapabilityError';
  }
}

// Commerce engine interface matching the API surface. Members marked
// optional exist on the mock but not on the native binding; the `*Api`
// wrappers below route them to a native equivalent or raise
// `EngineCapabilityError`.
export interface CommerceEngine {
  orders: {
    list: (params?: {
      status?: string;
      customerId?: string;
      /** RFC 3339, inclusive (native `OrderFilterInput.fromDate`). */
      fromDate?: string;
      /** RFC 3339, inclusive (native `OrderFilterInput.toDate`). */
      toDate?: string;
      limit?: number;
      offset?: number;
    }) => Promise<Order[]>;
    get: (id: string) => Promise<Order | null>;
    create: (params: CreateOrderParams) => Promise<Order>;
    updateStatus: (id: string, status: string) => Promise<Order>;
    cancel: (id: string, reason?: string) => Promise<Order>;
    /** Mock-only aggregate; `ordersApi` derives it from the order list otherwise. */
    getAnalytics?: (params?: { startDate?: string; endDate?: string }) => Promise<OrderAnalytics>;
  };
  inventory: {
    /** Mock-only: the binding has no inventory listing. */
    list?: (params?: { warehouseId?: string; lowStock?: boolean }) => Promise<InventoryItem[]>;
    /** Mock-only; the native lookup is `getStock`. */
    get?: (sku: string) => Promise<InventoryItem | null>;
    /** Native stock lookup (`Inventory.getStock`). */
    getStock?: (sku: string) => Promise<NativeStockLevel | null>;
    adjust: (sku: string, quantity: number, reason?: string) => Promise<InventoryAdjustment>;
    /** The binding's signature: a reservation references its source record. */
    reserve: (
      sku: string,
      quantity: number,
      referenceType: string,
      referenceId: string,
    ) => Promise<unknown>;
    /** Mock-only: the binding releases by reservation id (`releaseReservation`). */
    release?: (sku: string, quantity: number, orderId: string) => Promise<InventoryItem>;
    /** Mock-only; natively `analytics.lowStockItems`. */
    getLowStock?: (threshold?: number) => Promise<InventoryItem[]>;
    /** Mock-only; natively `analytics.inventoryHealth`. */
    getAnalytics?: () => Promise<InventoryAnalytics>;
    /** Mock-only; natively `analytics.demandForecast`. */
    forecast?: (sku: string, days: number) => Promise<DemandForecast>;
  };
  /**
   * Return lifecycle, named after the native binding's methods
   * (`bindings/node/index.d.ts` `class Returns`). Each one moves a return to
   * a single state of `RETURN_LIFECYCLE`; the engine refuses every other move.
   */
  returns: {
    list: (params?: {
      status?: string;
      customerId?: string;
      /** RFC 3339, inclusive (native `ReturnFilterInput.fromDate`). */
      fromDate?: string;
      /** RFC 3339, inclusive (native `ReturnFilterInput.toDate`). */
      toDate?: string;
      limit?: number;
      offset?: number;
    }) => Promise<Return[]>;
    get: (id: string) => Promise<Return | null>;
    create: (params: CreateReturnParams) => Promise<Return>;
    /** requested → approved */
    approve: (id: string) => Promise<Return>;
    /** requested | inspecting → rejected */
    reject: (id: string, reason: string) => Promise<Return>;
    /** requested | approved → cancelled */
    cancel: (id: string) => Promise<Return>;
    /** approved → in_transit (records the carrier tracking number) */
    addTracking: (id: string, trackingNumber: string) => Promise<Return>;
    /** in_transit → received */
    markReceived: (id: string) => Promise<Return>;
    /** received | inspecting → completed (settles the refund) */
    complete: (id: string) => Promise<Return>;
    /** Mock-only aggregate; the binding has none, so `returnsApi` derives it. */
    getAnalytics?: (params?: { startDate?: string; endDate?: string }) => Promise<ReturnAnalytics>;
  };
  customers: {
    list: (params?: { segment?: string; limit?: number; offset?: number }) => Promise<Customer[]>;
    get: (id: string) => Promise<Customer | null>;
    getByEmail: (email: string) => Promise<Customer | null>;
    create: (params: Partial<Customer>) => Promise<Customer>;
    update: (id: string, params: Partial<Customer>) => Promise<Customer>;
    /** Mock-only; natively `orders.list({ customerId })`. */
    getOrders?: (customerId: string) => Promise<Order[]>;
    /** Mock-only: the engine computes no customer health score. */
    getHealthScore?: (customerId: string) => Promise<CustomerHealthScore>;
    /** Mock-only: engine segments are a separate rule-based API. */
    getSegments?: () => Promise<CustomerSegment[]>;
    /** Mock-only: the engine has no equivalent customer aggregate. */
    getAnalytics?: () => Promise<CustomerAnalytics>;
  };
  products: {
    list: (params?: { status?: string; category?: string }) => Promise<Product[]>;
    get: (id: string) => Promise<Product | null>;
    create: (params: Partial<Product>) => Promise<Product>;
    update: (id: string, params: Partial<Product>) => Promise<Product>;
    delete: (id: string) => Promise<void>;
  };
  /**
   * Subscriptions, named after the native binding's methods
   * (`bindings/node/index.d.ts` `class Subscriptions`); each action accepts
   * the states in `SUBSCRIPTION_ACTION_SOURCES` (the engine's action guards).
   */
  subscriptions: {
    list: (params?: {
      status?: string;
      customerId?: string;
      limit?: number;
      offset?: number;
    }) => Promise<Subscription[]>;
    get: (id: string) => Promise<Subscription | null>;
    /** Start a subscription to a plan (the binding has no `create`). */
    subscribe: (input: { customerId: string; planId: string }) => Promise<Subscription>;
    /** active | trial → paused */
    pause: (id: string) => Promise<Subscription>;
    /** paused → active */
    resume: (id: string) => Promise<Subscription>;
    /** pending | trial | active | paused | past_due → cancelled */
    cancel: (id: string, input?: { reason?: string }) => Promise<Subscription>;
    /** Mock-only aggregate; the binding has none, so `subscriptionsApi` derives it. */
    getAnalytics?: () => Promise<SubscriptionAnalytics>;
  };
  generalLedger: {
    getAutoPostingConfig: () => Promise<AutoPostingConfig | null>;
    setAutoPostingConfig: (input: AutoPostingConfigInput) => Promise<AutoPostingConfig>;
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
    getDso?: (days: number) => Promise<string | number>;
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
  /**
   * The admin's dashboard aggregates are mock-only: the native `Analytics`
   * class exposes differently shaped reports (`salesSummary`,
   * `revenueByPeriod`, ...), so these raise `EngineCapabilityError` there.
   */
  analytics: {
    getDashboardMetrics?: () => Promise<DashboardMetrics>;
    getHourlyActivity?: (date?: string) => Promise<HourlyActivity[]>;
    getSystemHealth?: () => Promise<SystemHealth>;
    getRevenueByPeriod?: (params: {
      startDate: string;
      endDate: string;
      groupBy: 'day' | 'week' | 'month';
    }) => Promise<{ period: string; revenue: number; orders: number }[]>;
    getTopProducts?: (
      limit?: number,
    ) => Promise<{ productId: string; name: string; revenue: number; units: number }[]>;
    getConversionFunnel?: () => Promise<{ stage: string; count: number; rate: number }[]>;
    /**
     * Native only (`Analytics.returnMetrics`): engine-computed return rate and
     * refund total. `period` takes only named windows (`AnalyticsPeriod`) and
     * defaults to the last 30 days.
     */
    returnMetrics?: (query?: { period?: string }) => Promise<NativeReturnMetrics>;
    /** Native only (`Analytics.lowStockItems`). */
    lowStockItems?: (threshold?: number) => Promise<NativeLowStockItem[]>;
    /** Native only (`Analytics.inventoryHealth`). */
    inventoryHealth?: () => Promise<NativeInventoryHealth>;
    /** Native only (`Analytics.demandForecast`). */
    demandForecast?: (skus?: string[], daysAhead?: number) => Promise<NativeDemandForecast[]>;
  };
  initialize?: () => Promise<void>;
}

/** The binding's `StockLevelOutput` (`Inventory.getStock`); quantities are decimal strings. */
export interface NativeStockLevel {
  sku: string;
  name: string;
  totalOnHand: string;
  totalAllocated: string;
  totalAvailable: string;
}

/** The binding's `LowStockItemOutput` (`Analytics.lowStockItems`). */
export interface NativeLowStockItem {
  sku: string;
  name: string;
  onHand: number;
  allocated: number;
  available: number;
  reorderPoint?: number;
}

/** The fields of the binding's `InventoryHealthOutput` the admin reads. */
export interface NativeInventoryHealth {
  totalSkus: number;
  lowStockSkus: number;
  outOfStockSkus: number;
  totalValue: number;
  totalValueExact?: string;
}

/** The fields of the binding's `DemandForecastOutput` the admin reads. */
export interface NativeDemandForecast {
  sku: string;
  averageDailyDemand: number;
  forecastedDemand: number;
  confidence: number;
  currentStock: number;
  daysUntilStockout?: number;
  recommendedReorderQty?: number;
}

/** The fields of the binding's `ReturnMetricsOutput` the admin reads. */
export interface NativeReturnMetrics {
  totalReturns: number;
  returnRatePercent: number;
  totalRefunded: number;
  totalRefundedExact?: string;
}

// Dynamic import to prevent bundling issues
let StateSetCommerce: (new (databasePath: string) => CommerceEngine) | null = null;

const DAY_MS = 24 * 60 * 60 * 1000;
const HOUR_MS = 60 * 60 * 1000;
const DEFAULT_DATABASE_PATH = process.env.DATABASE_PATH || './data/admin.db';
const MOCK_FLAG = 'STATESET_ADMIN_ALLOW_MOCK_DATA';
const MOCK_NOW = Date.parse('2026-03-01T12:00:00.000Z');

/**
 * The demo session's state. Orders, returns and subscriptions change during a
 * session (creation, lifecycle moves), so their aggregates are not cached
 * here: `mockOrderAnalytics` and friends derive them from these records on
 * every read. `simulated` holds the figures demo mode invents because no
 * record supports them; they are kept as seeded.
 */
type MockCommerceData = {
  orders: Order[];
  inventory: InventoryItem[];
  inventoryAnalytics: InventoryAnalytics;
  returns: Return[];
  customers: Customer[];
  customerAnalytics: CustomerAnalytics;
  products: Product[];
  subscriptions: Subscription[];
  hourlyActivity: HourlyActivity[];
  simulated: {
    returns: Pick<ReturnAnalytics, 'averageProcessingTime' | 'topReturnedProducts'>;
    subscriptions: Pick<SubscriptionAnalytics, 'mrrGrowth' | 'averageLifetime'>;
  };
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
    const candidate = (moduleExports.Commerce || moduleExports.default || moduleExports) as unknown;
    if (typeof candidate !== 'function') {
      throw new Error('Embedded module did not export a constructor');
    }
    StateSetCommerce = candidate as new (databasePath: string) => CommerceEngine;
    return StateSetCommerce;
  } catch (error) {
    if (!shouldAllowMockEngine()) {
      throw mockEngineError(error);
    }

    console.warn(`[embedded] Falling back to deterministic mock data because ${MOCK_FLAG}=true.`);
    return null;
  }
}

// ============================================================================
// Native boundary: one normalisation for the binding's Debug-cased enums
// ============================================================================

/**
 * Enum-valued fields the native binding renders in the Rust `Debug` form
 * (`'PartiallyPaid'`, `'InProgress'`) instead of the engine's canonical
 * snake_case, keyed by the sub-API that returns them (see the "rendered Debug"
 * unions in `bindings/node/index.d.ts`). The binding's output is deliberately
 * left as-is — other consumers depend on it — and the admin normalises here,
 * once, so every page compares against one vocabulary
 * (`src/lib/engine-lifecycles.ts`) whether it runs on the native engine or
 * the mock.
 */
const NATIVE_DEBUG_ENUM_FIELDS: Readonly<Record<string, readonly string[]>> = {
  lots: ['status'],
  serials: ['status'],
  quality: ['status', 'severity', 'source', 'disposition', 'inspectionType', 'holdType'],
  fulfillment: ['status'],
  receiving: ['status', 'receiptType'],
  warehouse: ['warehouseType', 'locationType'],
  accountsPayable: ['status'],
  accountsReceivable: ['status', 'reason'],
  generalLedger: ['status', 'accountType'],
};

/** Rewrite the listed enum fields to snake_case on a record, a list, or nested records. */
function normalizeNativeOutput(value: unknown, fields: readonly string[], depth = 0): unknown {
  if (depth > 3 || value === null || typeof value !== 'object') return value;
  if (Array.isArray(value)) {
    return value.map((entry) => normalizeNativeOutput(entry, fields, depth + 1));
  }
  const record = value as Record<string, unknown>;
  let changed: Record<string, unknown> | null = null;
  for (const [key, entry] of Object.entries(record)) {
    const next =
      typeof entry === 'string'
        ? fields.includes(key)
          ? normalizeEngineStatus(entry)
          : entry
        : normalizeNativeOutput(entry, fields, depth + 1);
    if (next !== entry) {
      changed = changed ?? { ...record };
      changed[key] = next;
    }
  }
  return changed ?? record;
}

function isThenable(value: unknown): value is PromiseLike<unknown> {
  return (
    value !== null &&
    (typeof value === 'object' || typeof value === 'function') &&
    typeof (value as { then?: unknown }).then === 'function'
  );
}

function wrapNativeSubApi(api: object, fields: readonly string[]): object {
  return new Proxy(api, {
    get(target, property) {
      const member: unknown = Reflect.get(target, property, target);
      if (typeof member !== 'function') return member;
      return (...args: unknown[]) => {
        const result: unknown = Reflect.apply(member, target, args);
        return isThenable(result)
          ? result.then((resolved) => normalizeNativeOutput(resolved, fields))
          : normalizeNativeOutput(result, fields);
      };
    },
  });
}

/**
 * Wrap a native `Commerce` instance so the sub-APIs in
 * `NATIVE_DEBUG_ENUM_FIELDS` return snake_case enum values. Every other member
 * passes through untouched (methods stay bound to the native instance).
 */
export function adaptNativeEngine<T extends object>(engine: T): T {
  return new Proxy(engine, {
    get(target, property) {
      const member: unknown = Reflect.get(target, property, target);
      if (
        typeof property === 'string' &&
        Object.prototype.hasOwnProperty.call(NATIVE_DEBUG_ENUM_FIELDS, property) &&
        member !== null &&
        typeof member === 'object'
      ) {
        return wrapNativeSubApi(member, NATIVE_DEBUG_ENUM_FIELDS[property]);
      }
      return typeof member === 'function' ? member.bind(target) : member;
    },
  });
}

/**
 * Initialize or get the embedded commerce engine instance.
 * Mock data is only allowed when explicitly enabled for demo or test mode.
 */
export async function getCommerceEngine(): Promise<CommerceEngine> {
  if (!commerceInstance) {
    const CommerceModule = await loadCommerceModule();
    if (CommerceModule) {
      commerceInstance = adaptNativeEngine(new CommerceModule(DEFAULT_DATABASE_PATH));
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
  // Every engine order status (weighted towards delivered), including the
  // `partially_shipped` and `refunded` states the admin used to ignore.
  const statusPool: Order['status'][] = [
    'delivered',
    'delivered',
    'shipped',
    'partially_shipped',
    'processing',
    'confirmed',
    'pending',
    'cancelled',
    'refunded',
  ];
  // Every state of the engine's return lifecycle, so demo mode exercises the
  // same queue (and the same action gating) as the native engine.
  const returnStatuses: Return['status'][] = [
    'requested',
    'approved',
    'in_transit',
    'received',
    'inspecting',
    'completed',
    'rejected',
    'cancelled',
  ];
  const returnReasons: Return['reasonCategory'][] = [...RETURN_REASONS];
  // Every engine subscription status, weighted towards active.
  const subscriptionStatuses: Subscription['status'][] = [
    'active',
    'active',
    'active',
    'trial',
    'paused',
    'past_due',
    'pending',
    'cancelled',
    'expired',
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

  const shippedBack: Return['status'][] = ['in_transit', 'received', 'inspecting', 'completed'];
  const returns = Array.from({ length: 25 }, (_, index) => {
    const order = orders[(index * 2) % orders.length];
    const amount = roundCurrency(
      order.totalAmount * (0.35 + deterministicNumber(index + 801, 0, 0.45)),
    );
    const createdAt = formatMockDate(-(13 - (index % 14)), index % 12);
    const status = returnStatuses[index % returnStatuses.length];
    return {
      id: `ret_${1000 + index}`,
      orderId: order.id,
      customerId: order.customerId,
      status,
      // The engine records a tracking number when a return ships back.
      trackingNumber: shippedBack.includes(status) ? `1ZRET${10_000 + index}` : undefined,
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
    const updatedAt = formatMockDate(-(index % 10), index % 6);
    const status = deterministicChoice(index + 1201, subscriptionStatuses);
    return {
      id: `sub_${1000 + index}`,
      customerId: customer.id,
      status,
      plan: plan.plan,
      planId: plan.planId,
      frequency: plan.frequency,
      nextBillingDate: formatMockDate(1 + (index % 28), index % 10),
      currentPeriodEnd: formatMockDate(7 + (index % 28), index % 10),
      items: [{ productId: products[index % products.length].id, quantity }],
      quantity,
      totalAmount: roundCurrency(plan.amount * quantity),
      // The engine stamps a cancellation; its last update is the demo's.
      ...(status === 'cancelled' && { cancelledAt: updatedAt }),
      createdAt,
      updatedAt,
    } satisfies Subscription;
  });

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
    hourlyActivity,
    simulated: {
      returns: {
        averageProcessingTime: 48,
        topReturnedProducts: returns.slice(0, 3).map((item, index) => ({
          productId: item.items[0]?.productId || `prod_${index}`,
          name: item.items[0]?.name || `Product ${index + 1}`,
          count: 2 + index,
          rate: Number((0.02 + index * 0.01).toFixed(2)),
        })),
      },
      subscriptions: { mrrGrowth: 0.08, averageLifetime: 18 },
    },
  };
}

function getMockData(): MockCommerceData {
  if (!mockDataCache) {
    mockDataCache = buildMockCommerceData();
  }
  return mockDataCache;
}

// Demo aggregates are derived from the session's current records on every
// read, with the same summaries (and the same window rules) the native
// fallbacks use, so a demo lifecycle move or creation shows up in them.

function mockOrderAnalytics(params?: { startDate?: string; endDate?: string }): OrderAnalytics {
  return summarizeOrders(getMockData().orders, {
    now: MOCK_NOW,
    window: resolveAnalyticsWindow(params),
  });
}

function mockReturnAnalytics(params?: { startDate?: string; endDate?: string }): ReturnAnalytics {
  const data = getMockData();
  const window = resolveAnalyticsWindow(params);
  return {
    ...summarizeReturns(data.returns, {
      orderCount: data.orders.filter((order) => isWithinWindow(order.createdAt, window)).length,
      window,
    }),
    ...data.simulated.returns,
  };
}

function mockSubscriptionAnalytics(): SubscriptionAnalytics {
  const data = getMockData();
  return {
    ...summarizeSubscriptions(data.subscriptions, { now: MOCK_NOW }),
    ...data.simulated.subscriptions,
  };
}

function mockDashboardMetrics(): DashboardMetrics {
  const { orders, customers, returns, inventory } = getMockData();
  const totalOf = (subset: Order[]) => Number(sumDecimals(subset.map(exactOrderTotal)));
  const today = toDateKey(formatMockDate(0));
  const yesterday = toDateKey(formatMockDate(-1));
  const todayOrders = orders.filter((order) => toDateKey(order.createdAt) === today);
  const yesterdayOrders = orders.filter((order) => toDateKey(order.createdAt) === yesterday);
  const gmvToday = totalOf(todayOrders);
  const gmvYesterday = totalOf(yesterdayOrders);
  const averageOrderValue = orders.length ? roundCurrency(totalOf(orders) / orders.length) : 0;
  const lowStockItems = inventory.filter((item) => item.availableQuantity <= item.reorderPoint);
  return {
    gmvToday,
    gmvChange: gmvYesterday > 0 ? Number(((gmvToday - gmvYesterday) / gmvYesterday).toFixed(3)) : 0,
    ordersToday: todayOrders.length,
    ordersChange:
      yesterdayOrders.length > 0
        ? Number(
            ((todayOrders.length - yesterdayOrders.length) / yesterdayOrders.length).toFixed(3),
          )
        : 0,
    averageOrderValue,
    // Simulated: demo mode has no prior period to compare with.
    aovChange: Number((averageOrderValue / 140).toFixed(3)),
    conversionRate: Number(Math.min(0.12, orders.length / 1200).toFixed(3)),
    conversionChange: 0.018,
    activeCustomers: customers.filter((customer) => customer.totalOrders > 2).length,
    newCustomers: customers.filter((customer) => customer.createdAt >= formatMockDate(-30)).length,
    returnRate: Number((returns.length / Math.max(orders.length, 1)).toFixed(3)),
    inventoryHealth: Number((1 - lowStockItems.length / Math.max(inventory.length, 1)).toFixed(3)),
  };
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

/**
 * Keep a record created in demo mode, newest first as the engine lists, so
 * the session can list it, read it and move it through its lifecycle.
 */
function persistMockRecord<T>(records: T[], record: T): T {
  records.unshift(record);
  return record;
}

/** Apply a list call's `limit`/`offset`, as the engine's lists do. */
function mockPage<T>(records: T[], params?: { limit?: number; offset?: number }): T[] {
  const offset = params?.offset || 0;
  const end = params?.limit ? offset + params.limit : undefined;
  return records.slice(offset, end);
}

function getMockOrder(id: string): Order | null {
  return getMockData().orders.find((order) => order.id === id) || null;
}

/**
 * Move a mock order the way the engine's `update_status` does: the same
 * state is a no-op (the catalog marks order states re-entrant), any other
 * move must be an `ORDER_LIFECYCLE` edge. Persisted for the demo session.
 */
function transitionMockOrder(id: string, to: Order['status']): Order {
  const data = getMockData();
  const index = data.orders.findIndex((order) => order.id === id);
  if (index === -1) {
    throw new Error(`Order not found: ${id}`);
  }
  const current = data.orders[index];
  if (current.status !== to && !canTransition(ORDER_LIFECYCLE, current.status, to)) {
    throw new Error(`Order cannot move from ${current.status} to ${to}`);
  }
  const next: Order = { ...current, status: to, updatedAt: new Date().toISOString() };
  data.orders[index] = next;
  return next;
}

function getMockInventoryItem(sku: string): InventoryItem | null {
  return getMockData().inventory.find((item) => item.sku === sku) || null;
}

function getMockReturn(id: string): Return | null {
  return getMockData().returns.find((entry) => entry.id === id) || null;
}

/**
 * Move a mock return the way the engine would: refuse any move
 * `RETURN_LIFECYCLE` does not allow, otherwise persist the new state so the
 * next action in a demo session sees it (the engine's own guard messages are
 * mirrored loosely; callers only rely on the throw).
 */
function transitionMockReturn(id: string, to: ReturnStatus, patch: Partial<Return> = {}): Return {
  const data = getMockData();
  const index = data.returns.findIndex((entry) => entry.id === id);
  if (index === -1) {
    throw new Error(`Return not found: ${id}`);
  }
  const current = data.returns[index];
  if (!canTransition(RETURN_LIFECYCLE, current.status, to)) {
    throw new Error(`Return cannot move from ${current.status} to ${to}`);
  }
  const next: Return = { ...current, ...patch, status: to, updatedAt: new Date().toISOString() };
  data.returns[index] = next;
  return next;
}

function getMockSubscription(id: string): Subscription | null {
  return getMockData().subscriptions.find((subscription) => subscription.id === id) || null;
}

/**
 * Apply a subscription action to a mock subscription the way the engine
 * does: the action's own guard (`SUBSCRIPTION_ACTION_SOURCES`, which lets
 * `pause` take a trial as `pause_subscription` does), not just the catalog's
 * edges. Persisted for the demo session.
 */
function applyMockSubscriptionAction(id: string, action: SubscriptionAction): Subscription {
  const data = getMockData();
  const index = data.subscriptions.findIndex((subscription) => subscription.id === id);
  if (index === -1) {
    throw new Error(`Subscription not found: ${id}`);
  }
  const current = data.subscriptions[index];
  const to = SUBSCRIPTION_ACTION_TARGETS[action];
  if (!canApplySubscriptionAction(current.status, action)) {
    throw new Error(`Subscription cannot move from ${current.status} to ${to}`);
  }
  const updatedAt = new Date().toISOString();
  const next: Subscription = {
    ...current,
    status: to,
    updatedAt,
    // Like the engine, a cancellation records when it happened.
    ...(action === 'cancel' && { cancelledAt: updatedAt }),
  };
  data.subscriptions[index] = next;
  return next;
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
        return mockPage(orders, params);
      },
      get: async (id: string) => getMockOrder(id),
      create: async (params: CreateOrderParams) =>
        persistMockRecord(getMockData().orders, buildMockOrderFromParams(params)),
      updateStatus: async (id: string, status: string) =>
        transitionMockOrder(id, status as Order['status']),
      cancel: async (id: string) => transitionMockOrder(id, 'cancelled'),
      getAnalytics: async (params) => mockOrderAnalytics(params),
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
        return mockPage(returns, params);
      },
      get: async (id: string) => getMockReturn(id),
      create: async (params: CreateReturnParams) =>
        persistMockRecord(getMockData().returns, buildMockReturnFromParams(params)),
      approve: async (id: string) => transitionMockReturn(id, 'approved'),
      reject: async (id: string, reason: string) =>
        transitionMockReturn(id, 'rejected', { notes: reason }),
      cancel: async (id: string) => transitionMockReturn(id, 'cancelled'),
      addTracking: async (id: string, trackingNumber: string) =>
        transitionMockReturn(id, 'in_transit', { trackingNumber }),
      markReceived: async (id: string) => transitionMockReturn(id, 'received'),
      complete: async (id: string) => transitionMockReturn(id, 'completed'),
      getAnalytics: async (params) => mockReturnAnalytics(params),
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
        return mockPage(subscriptions, params);
      },
      get: async (id: string) => getMockSubscription(id),
      subscribe: async (input: { customerId: string; planId: string }) =>
        persistMockRecord(
          getMockData().subscriptions,
          buildMockSubscription(
            { customerId: input.customerId, planId: input.planId, plan: input.planId },
            nextMockId('sub'),
          ),
        ),
      pause: async (id: string) => applyMockSubscriptionAction(id, 'pause'),
      resume: async (id: string) => applyMockSubscriptionAction(id, 'resume'),
      cancel: async (id: string) => applyMockSubscriptionAction(id, 'cancel'),
      getAnalytics: async () => mockSubscriptionAnalytics(),
    },
    generalLedger: {
      getAutoPostingConfig: async () => null,
      setAutoPostingConfig: async () => {
        throw new Error('Posting settings require a live commerce engine');
      },
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
      getDashboardMetrics: async () => mockDashboardMetrics(),
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
// Derived analytics: every record, not the first page
// ============================================================================

/**
 * Page size for the admin's scans of native lists. The engine pages every
 * list (`append_limit_offset` in `crates/stateset-db/src/sqlite/mod.rs`: 500
 * rows when no limit is given, never more than 1000), so a single `list()`
 * call is never "the whole collection".
 */
export const NATIVE_LIST_PAGE_SIZE = 500;

/**
 * The most records one derived aggregate reads. A scan that stops here
 * reports `coverage.complete: false` instead of passing a subset off as the
 * whole store.
 */
export const ANALYTICS_SCAN_LIMIT = 50_000;

/**
 * Read a paged list to the end (or to `ANALYTICS_SCAN_LIMIT`). `complete` is
 * true only when the engine signalled the last page; an engine that repeats
 * a page (ignores `offset`) or a collection past the cap yields `false`.
 */
async function collectAllPages<T extends { id: string }>(
  fetchPage: (page: { limit: number; offset: number }) => Promise<T[]>,
): Promise<{ records: T[]; complete: boolean }> {
  const seen = new Set<string>();
  const records: T[] = [];
  let offset = 0;
  for (;;) {
    const page = await fetchPage({ limit: NATIVE_LIST_PAGE_SIZE, offset });
    let added = 0;
    for (const record of page) {
      if (records.length === ANALYTICS_SCAN_LIMIT) {
        return { records, complete: false };
      }
      if (!seen.has(record.id)) {
        seen.add(record.id);
        records.push(record);
        added += 1;
      }
    }
    // A short page is the last one; a page longer than asked for means the
    // engine ignored paging and returned everything at once.
    if (page.length !== NATIVE_LIST_PAGE_SIZE) {
      return { records, complete: true };
    }
    if (added === 0) {
      return { records, complete: false };
    }
    if (records.length === ANALYTICS_SCAN_LIMIT) {
      const [next] = await fetchPage({ limit: 1, offset: offset + page.length });
      return { records, complete: next === undefined };
    }
    offset += page.length;
  }
}

/** A requested reporting window: inclusive epoch-millisecond bounds, either one optional. */
export interface AnalyticsWindow {
  start?: number;
  end?: number;
}

const DATE_ONLY = /^\d{4}-\d{2}-\d{2}$/;

function parseWindowBound(value: string, field: 'startDate' | 'endDate'): number {
  // A bare date means the whole (UTC) day: from its first to its last millisecond.
  const text = DATE_ONLY.test(value)
    ? `${value}T${field === 'startDate' ? '00:00:00.000' : '23:59:59.999'}Z`
    : value;
  const parsed = Date.parse(text);
  if (Number.isNaN(parsed)) {
    throw new Error(`Invalid analytics ${field}: ${value}`);
  }
  return parsed;
}

/**
 * The window an analytics caller asked for, or `undefined` for all time.
 * Refuses an unparseable date rather than silently reporting all time.
 */
export function resolveAnalyticsWindow(params?: {
  startDate?: string;
  endDate?: string;
}): AnalyticsWindow | undefined {
  const start = params?.startDate ? parseWindowBound(params.startDate, 'startDate') : undefined;
  const end = params?.endDate ? parseWindowBound(params.endDate, 'endDate') : undefined;
  return start === undefined && end === undefined ? undefined : { start, end };
}

/** The window as the native list filters take it (`fromDate`/`toDate`, RFC 3339, inclusive). */
function nativeDateFilter(window?: AnalyticsWindow): { fromDate?: string; toDate?: string } {
  return {
    ...(window?.start !== undefined && { fromDate: new Date(window.start).toISOString() }),
    ...(window?.end !== undefined && { toDate: new Date(window.end).toISOString() }),
  };
}

/** Whether a record's timestamp falls in the window (all records do when there is none). */
function isWithinWindow(timestamp: string, window?: AnalyticsWindow): boolean {
  if (!window) return true;
  const at = Date.parse(timestamp);
  if (Number.isNaN(at)) return false;
  return (
    (window.start === undefined || at >= window.start) &&
    (window.end === undefined || at <= window.end)
  );
}

function coverageOf(
  records: readonly unknown[],
  complete: boolean,
  window?: AnalyticsWindow,
  unavailable: string[] = [],
): AnalyticsCoverage {
  const range = nativeDateFilter(window);
  return {
    complete,
    records: records.length,
    ...(window && { window: { start: range.fromDate, end: range.toDate } }),
    ...(unavailable.length > 0 && { unavailable }),
  };
}

// ============================================================================
// Orders API
// ============================================================================

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

  async updateStatus(orderId: string, status: OrderStatusUpdate): Promise<Order> {
    const commerce = await getCommerceEngine();
    return commerce.orders.updateStatus(orderId, status);
  },

  async cancel(orderId: string, reason?: string): Promise<Order> {
    const commerce = await getCommerceEngine();
    return commerce.orders.cancel(orderId, reason);
  },

  async getAnalytics(params?: { startDate?: string; endDate?: string }): Promise<OrderAnalytics> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.orders.getAnalytics === 'function') {
      return commerce.orders.getAnalytics(params);
    }
    // The binding has no order aggregate; every figure here is derivable
    // from the order records themselves, all of them, in the requested window
    // (the engine filters by order date; `summarizeOrders` re-applies the
    // window in case a build ignores the filter).
    const window = resolveAnalyticsWindow(params);
    const { records, complete } = await collectAllPages((page) =>
      commerce.orders.list({ ...nativeDateFilter(window), ...page }),
    );
    return summarizeOrders(records, { complete, window });
  },
};

/** An order's total as an exact decimal string (the native exact twin when present). */
function exactOrderTotal(order: Order): string {
  return toDecimalString(order.totalAmountExact ?? order.totalAmount);
}

/** The UTC days the daily breakdown spans: the window, or the 30 days to `now`; at most 366. */
function dailyBreakdownDays(now: number, window?: AnalyticsWindow): string[] {
  const end = window?.end ?? now;
  const start = window?.start ?? end - 29 * DAY_MS;
  const lastDay = Date.parse(toDateKey(new Date(end).toISOString()));
  const firstDay = Math.max(
    Date.parse(toDateKey(new Date(start).toISOString())),
    lastDay - 365 * DAY_MS,
  );
  const days: string[] = [];
  for (let day = firstDay; day <= lastDay; day += DAY_MS) {
    days.push(toDateKey(new Date(day).toISOString()));
  }
  return days;
}

/**
 * Order analytics computed from the order records (engines without an
 * aggregate). With a `window`, every figure covers only the orders created in
 * it and the daily breakdown spans it.
 */
export function summarizeOrders(
  allOrders: Order[],
  options: { now?: number; complete?: boolean; window?: AnalyticsWindow } = {},
): OrderAnalytics {
  const now = options.now ?? Date.now();
  const orders = allOrders.filter((order) => isWithinWindow(order.createdAt, options.window));
  const revenueOf = (subset: Order[]) => sumDecimals(subset.map(exactOrderTotal));
  const totalRevenueExact = revenueOf(orders);
  const averageOrderValueExact = orders.length
    ? divideDecimal(totalRevenueExact, orders.length)
    : '0';
  const ordersByStatus = orders.reduce<Record<string, number>>((acc, order) => {
    acc[order.status] = (acc[order.status] || 0) + 1;
    return acc;
  }, {});
  const byDay = new Map<string, Order[]>();
  for (const order of orders) {
    const day = toDateKey(order.createdAt);
    const bucket = byDay.get(day);
    if (bucket) bucket.push(order);
    else byDay.set(day, [order]);
  }
  const ordersByDay = dailyBreakdownDays(now, options.window).map((date) => {
    const daily = byDay.get(date) ?? [];
    const revenueExact = revenueOf(daily);
    return { date, count: daily.length, revenue: Number(revenueExact), revenueExact };
  });
  return {
    totalOrders: orders.length,
    totalRevenue: Number(totalRevenueExact),
    totalRevenueExact,
    averageOrderValue: Number(averageOrderValueExact),
    averageOrderValueExact,
    ordersByStatus,
    ordersByDay,
    coverage: coverageOf(orders, options.complete ?? true, options.window),
  };
}

// ============================================================================
// Inventory API
// ============================================================================

function toInventoryItem(stock: {
  sku: string;
  name: string;
  onHand: number;
  allocated: number;
  available: number;
  reorderPoint?: number;
}): InventoryItem {
  return {
    id: stock.sku,
    sku: stock.sku,
    productId: '',
    productName: stock.name,
    quantity: stock.onHand,
    reservedQuantity: stock.allocated,
    availableQuantity: stock.available,
    // Not reported by the engine's stock lookup; 0 means "no reorder rule".
    reorderPoint: stock.reorderPoint ?? 0,
    reorderQuantity: 0,
    updatedAt: '',
  };
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
  /** Whether this engine can list inventory (the native binding cannot). */
  async supportsListing(): Promise<boolean> {
    const commerce = await getCommerceEngine();
    return typeof commerce.inventory.list === 'function';
  },

  async list(params?: { warehouseId?: string; lowStock?: boolean }): Promise<InventoryItem[]> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.inventory.list !== 'function') {
      throw new EngineCapabilityError(
        'inventory.list',
        'The embedded engine has no inventory listing; look stock up by SKU instead',
      );
    }
    return commerce.inventory.list(params);
  },

  async get(sku: string): Promise<InventoryItem | null> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.inventory.get === 'function') {
      return commerce.inventory.get(sku);
    }
    if (typeof commerce.inventory.getStock === 'function') {
      const stock = await commerce.inventory.getStock(sku);
      return stock
        ? toInventoryItem({
            sku: stock.sku,
            name: stock.name,
            onHand: Number(stock.totalOnHand),
            allocated: Number(stock.totalAllocated),
            available: Number(stock.totalAvailable),
          })
        : null;
    }
    throw new EngineCapabilityError('inventory.get', 'This engine cannot look up stock by SKU');
  },

  async adjust(sku: string, quantity: number, reason?: string): Promise<InventoryAdjustment> {
    const commerce = await getCommerceEngine();
    return commerce.inventory.adjust(sku, quantity, reason);
  },

  /** Reserve stock for an order (the binding records the order as the reservation's source). */
  async reserve(sku: string, quantity: number, orderId: string): Promise<unknown> {
    const commerce = await getCommerceEngine();
    return commerce.inventory.reserve(sku, quantity, 'order', orderId);
  },

  async release(sku: string, quantity: number, orderId: string): Promise<InventoryItem> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.inventory.release !== 'function') {
      throw new EngineCapabilityError(
        'inventory.release',
        'The embedded engine releases stock by reservation id, not by SKU and order',
      );
    }
    return commerce.inventory.release(sku, quantity, orderId);
  },

  async getLowStock(threshold?: number): Promise<InventoryItem[]> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.inventory.getLowStock === 'function') {
      return commerce.inventory.getLowStock(threshold);
    }
    if (typeof commerce.analytics.lowStockItems === 'function') {
      return (await commerce.analytics.lowStockItems(threshold)).map(toInventoryItem);
    }
    throw new EngineCapabilityError('inventory.getLowStock', 'This engine reports no low stock');
  },

  async getAnalytics(): Promise<InventoryAnalytics> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.inventory.getAnalytics === 'function') {
      return commerce.inventory.getAnalytics();
    }
    if (typeof commerce.analytics.inventoryHealth === 'function') {
      const health = await commerce.analytics.inventoryHealth();
      const totalValueExact = toDecimalString(health.totalValueExact ?? health.totalValue);
      return {
        totalSKUs: health.totalSkus,
        // Not part of the engine's health report.
        totalUnits: 0,
        totalValue: Number(totalValueExact),
        totalValueExact,
        lowStockItems: health.lowStockSkus,
        outOfStockItems: health.outOfStockSkus,
        turnoverRate: 0,
        topMovingItems: [],
        slowMovingItems: [],
      };
    }
    throw new EngineCapabilityError(
      'inventory.getAnalytics',
      'This engine reports no inventory health',
    );
  },

  async forecast(sku: string, days: number): Promise<DemandForecast> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.inventory.forecast === 'function') {
      return commerce.inventory.forecast(sku, days);
    }
    if (typeof commerce.analytics.demandForecast === 'function') {
      const [forecast] = await commerce.analytics.demandForecast([sku], days);
      if (!forecast) {
        throw new EngineCapabilityError(
          'inventory.forecast',
          `The engine has no forecast for ${sku}`,
        );
      }
      return {
        sku: forecast.sku,
        currentStock: forecast.currentStock,
        // The engine forecasts an average daily rate; spread it over the window.
        forecastedDemand: Array.from(
          { length: Math.max(days, 1) },
          () => forecast.averageDailyDemand,
        ),
        recommendedReorder: forecast.recommendedReorderQty ?? 0,
        stockoutDate:
          forecast.daysUntilStockout === undefined
            ? undefined
            : new Date(Date.now() + forecast.daysUntilStockout * DAY_MS).toISOString(),
        confidence: forecast.confidence,
      };
    }
    throw new EngineCapabilityError('inventory.forecast', 'This engine has no demand forecast');
  },
};

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

export interface CreateReturnParams {
  orderId: string;
  items: { productId: string; quantity: number; reason?: string }[];
  reason: string;
  reasonCategory: Return['reasonCategory'];
}

function isReturnReason(value: unknown): value is ReturnReason {
  return typeof value === 'string' && (RETURN_REASONS as readonly string[]).includes(value);
}

/**
 * The native `ReturnOutput` is a header — id, orderId, status, the reason
 * *code* and createdAt — while the admin's `Return` also carries the customer,
 * line items and a free-text reason. Fill those so list views never read
 * `undefined.length`; mock records already carry every field and pass through.
 */
export function toAdminReturn(record: Return): Return {
  const partial = record as Partial<Return> & Pick<Return, 'id' | 'orderId' | 'status'>;
  const reasonCategory = isReturnReason(partial.reasonCategory)
    ? partial.reasonCategory
    : isReturnReason(partial.reason)
      ? partial.reason
      : 'other';
  const createdAt = partial.createdAt ?? '';
  return {
    ...partial,
    customerId: partial.customerId ?? '',
    items: partial.items ?? [],
    reason: partial.reason ?? '',
    reasonCategory,
    createdAt,
    updatedAt: partial.updatedAt ?? createdAt,
  };
}

/**
 * Return analytics computed from the return records themselves, for engines
 * without an aggregate. Every figure describes ONE population: the returns in
 * `window` (all of them when there is none).
 *
 *  - `metrics` (the engine's `analytics.returnMetrics`) supplies the rate and
 *    refund total; pass it only when it was computed over that same
 *    population.
 *  - Otherwise the rate is returns per order over `orderCount` (the orders
 *    created in the window, as the engine computes it), and the refund total
 *    is summed from the records' refund amounts.
 *
 * A figure neither source can supply reads zero and is listed in
 * `coverage.unavailable`; figures nothing reports (processing time, top
 * products) are left empty rather than invented.
 */
export function summarizeReturns(
  allReturns: Return[],
  options: {
    metrics?: NativeReturnMetrics;
    orderCount?: number;
    window?: AnalyticsWindow;
    complete?: boolean;
  } = {},
): ReturnAnalytics {
  const { metrics, orderCount, window } = options;
  const returns = allReturns.filter((entry) => isWithinWindow(entry.createdAt, window));
  const unavailable: string[] = [];
  const tally = (key: (entry: Return) => string) =>
    returns.reduce<Record<string, number>>((acc, entry) => {
      acc[key(entry)] = (acc[key(entry)] || 0) + 1;
      return acc;
    }, {});

  let returnRate = 0;
  if (metrics) {
    returnRate = metrics.returnRatePercent / 100;
  } else if (orderCount !== undefined) {
    returnRate = orderCount > 0 ? Number((returns.length / orderCount).toFixed(4)) : 0;
  } else {
    unavailable.push('returnRate');
  }

  let refundTotalExact = '0';
  const refundAmounts = returns.flatMap((entry) =>
    entry.refundAmount === undefined ? [] : [entry.refundAmount],
  );
  if (metrics) {
    refundTotalExact = toDecimalString(metrics.totalRefundedExact ?? metrics.totalRefunded);
  } else if (refundAmounts.length > 0) {
    refundTotalExact = sumDecimals(refundAmounts);
  } else if (returns.length > 0) {
    // Records without refund amounts (the native return header) and no
    // engine total for this population: the refund total is unknown.
    unavailable.push('refundTotal');
  }

  return {
    totalReturns: returns.length,
    returnRate,
    refundTotal: Number(refundTotalExact),
    refundTotalExact,
    returnsByReason: tally((entry) => entry.reasonCategory),
    returnsByStatus: tally((entry) => entry.status),
    averageProcessingTime: 0,
    topReturnedProducts: [],
    coverage: coverageOf(returns, options.complete ?? true, window, unavailable),
  };
}

export const returnsApi = {
  async list(params?: {
    status?: string;
    customerId?: string;
    limit?: number;
    offset?: number;
  }): Promise<Return[]> {
    const commerce = await getCommerceEngine();
    return (await commerce.returns.list(params)).map(toAdminReturn);
  },

  async get(returnId: string): Promise<Return | null> {
    const commerce = await getCommerceEngine();
    const record = await commerce.returns.get(returnId);
    return record ? toAdminReturn(record) : null;
  },

  async create(params: CreateReturnParams): Promise<Return> {
    const commerce = await getCommerceEngine();
    return toAdminReturn(await commerce.returns.create(params));
  },

  async approve(returnId: string): Promise<Return> {
    const commerce = await getCommerceEngine();
    return toAdminReturn(await commerce.returns.approve(returnId));
  },

  async reject(returnId: string, reason: string): Promise<Return> {
    const commerce = await getCommerceEngine();
    return toAdminReturn(await commerce.returns.reject(returnId, reason));
  },

  async cancel(returnId: string): Promise<Return> {
    const commerce = await getCommerceEngine();
    return toAdminReturn(await commerce.returns.cancel(returnId));
  },

  async addTracking(returnId: string, trackingNumber: string): Promise<Return> {
    const commerce = await getCommerceEngine();
    return toAdminReturn(await commerce.returns.addTracking(returnId, trackingNumber));
  },

  async markReceived(returnId: string): Promise<Return> {
    const commerce = await getCommerceEngine();
    return toAdminReturn(await commerce.returns.markReceived(returnId));
  },

  /** Completes the return; the engine settles its refund in the same step. */
  async complete(returnId: string): Promise<Return> {
    const commerce = await getCommerceEngine();
    return toAdminReturn(await commerce.returns.complete(returnId));
  },

  async getAnalytics(params?: { startDate?: string; endDate?: string }): Promise<ReturnAnalytics> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.returns.getAnalytics === 'function') {
      return commerce.returns.getAnalytics(params);
    }
    const window = resolveAnalyticsWindow(params);
    const dateFilter = nativeDateFilter(window);
    const returns = await collectAllPages((page) =>
      commerce.returns.list({ ...dateFilter, ...page }),
    );
    // The engine's metrics take only named periods (and default to the last
    // 30 days), so they describe the same returns as the listing only for an
    // undated request, asked for explicitly as all time.
    const metrics =
      !window && typeof commerce.analytics?.returnMetrics === 'function'
        ? await commerce.analytics.returnMetrics({ period: 'all_time' })
        : undefined;
    if (metrics) {
      return summarizeReturns(returns.records.map(toAdminReturn), {
        metrics,
        complete: returns.complete,
      });
    }
    // For a requested window, derive the rate as the engine does: returns per
    // order created in the same window.
    const orders = await collectAllPages((page) =>
      commerce.orders.list({ ...dateFilter, ...page }),
    );
    return summarizeReturns(returns.records.map(toAdminReturn), {
      orderCount: orders.records.filter((order) => isWithinWindow(order.createdAt, window)).length,
      window,
      complete: returns.complete && orders.complete,
    });
  },
};

// ============================================================================
// Customers API
// ============================================================================

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
    if (typeof commerce.customers.getOrders === 'function') {
      return commerce.customers.getOrders(customerId);
    }
    // The binding filters orders by customer instead.
    return commerce.orders.list({ customerId });
  },

  async getHealthScore(customerId: string): Promise<CustomerHealthScore> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.customers.getHealthScore !== 'function') {
      throw new EngineCapabilityError(
        'customers.getHealthScore',
        'The embedded engine does not score customer health',
      );
    }
    return commerce.customers.getHealthScore(customerId);
  },

  async getSegments(): Promise<CustomerSegment[]> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.customers.getSegments !== 'function') {
      throw new EngineCapabilityError(
        'customers.getSegments',
        'The embedded engine keeps segments as rules, not as a customer breakdown',
      );
    }
    return commerce.customers.getSegments();
  },

  async getAnalytics(): Promise<CustomerAnalytics> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.customers.getAnalytics !== 'function') {
      throw new EngineCapabilityError(
        'customers.getAnalytics',
        'The embedded engine has no customer health and retention analytics',
      );
    }
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

// ============================================================================
// Products API
// ============================================================================

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

/** Resolve a mock-only dashboard aggregate, or explain that the engine lacks it. */
async function dashboardAggregate<K extends keyof CommerceEngine['analytics']>(
  name: K,
): Promise<NonNullable<CommerceEngine['analytics'][K]>> {
  const commerce = await getCommerceEngine();
  const method = commerce.analytics[name];
  if (typeof method !== 'function') {
    throw new EngineCapabilityError(
      `analytics.${String(name)}`,
      'The embedded engine has no equivalent dashboard aggregate',
    );
  }
  return method.bind(commerce.analytics) as NonNullable<CommerceEngine['analytics'][K]>;
}

export const analyticsApi = {
  async getDashboardMetrics(): Promise<DashboardMetrics> {
    return (await dashboardAggregate('getDashboardMetrics'))();
  },

  async getHourlyActivity(date?: string): Promise<HourlyActivity[]> {
    return (await dashboardAggregate('getHourlyActivity'))(date);
  },

  async getSystemHealth(): Promise<SystemHealth> {
    return (await dashboardAggregate('getSystemHealth'))();
  },

  async getRevenueByPeriod(params: {
    startDate: string;
    endDate: string;
    groupBy: 'day' | 'week' | 'month';
  }): Promise<{ period: string; revenue: number; orders: number }[]> {
    return (await dashboardAggregate('getRevenueByPeriod'))(params);
  },

  async getTopProducts(
    limit?: number,
  ): Promise<{ productId: string; name: string; revenue: number; units: number }[]> {
    return (await dashboardAggregate('getTopProducts'))(limit);
  },

  async getConversionFunnel(): Promise<{ stage: string; count: number; rate: number }[]> {
    return (await dashboardAggregate('getConversionFunnel'))();
  },
};

// ============================================================================
// Subscriptions API
// ============================================================================

/** The fields of the binding's `SubscriptionOutput` the admin maps onto `Subscription`. */
interface NativeSubscriptionFields {
  planName?: string;
  billingInterval?: string;
  price?: number;
  /** Exact decimal twin of `price`. */
  priceExact?: string;
}

/**
 * The native `SubscriptionOutput` names its plan, price and interval
 * `planName`, `price` and `billingInterval` and has no line items; map those
 * onto the admin's `Subscription` so the subscription views read real values
 * instead of `undefined`. Mock records already match and pass through.
 */
export function toAdminSubscription(record: Subscription): Subscription {
  const partial = record as Partial<Subscription> &
    NativeSubscriptionFields &
    Pick<Subscription, 'id' | 'customerId' | 'status'>;
  const createdAt = partial.createdAt ?? '';
  return {
    ...partial,
    plan: partial.plan ?? partial.planName ?? partial.planId ?? '',
    frequency: partial.frequency ?? (partial.billingInterval as Subscription['frequency']),
    nextBillingDate: partial.nextBillingDate ?? '',
    items: partial.items ?? [],
    quantity: partial.quantity ?? 1,
    totalAmount: partial.totalAmount ?? partial.price ?? 0,
    totalAmountExact:
      partial.totalAmountExact ??
      partial.priceExact ??
      toDecimalString(partial.totalAmount ?? partial.price ?? 0),
    createdAt,
    updatedAt: partial.updatedAt ?? createdAt,
  };
}

/**
 * Billing periods per year, for both the admin's and the engine's interval
 * names. Whole numbers, so a yearly value is an exact product.
 */
const PERIODS_PER_YEAR: Record<string, number> = {
  weekly: 52,
  biweekly: 26,
  'bi-weekly': 26,
  monthly: 12,
  'bi-monthly': 6,
  quarterly: 4,
  'semi-annual': 2,
  annually: 1,
  yearly: 1,
};

/** Days of subscription starts and cancellations the analytics report. */
const SUBSCRIPTION_ACTIVITY_DAYS = 30;
/** Days ahead the renewal schedule covers. */
const SUBSCRIPTION_RENEWAL_DAYS = 7;
/** Renewals listed one by one; the count and amount cover all of them. */
const SUBSCRIPTION_RENEWALS_LISTED = 5;

function subscriptionPrice(subscription: Subscription): string {
  return toDecimalString(subscription.totalAmountExact ?? subscription.totalAmount);
}

/**
 * Monthly recurring revenue of `subscriptions`, exact: the sum of price x
 * periods per year (subscriptions with an unknown interval contribute
 * nothing), divided by 12 and rounded once, half to even, at the sum's scale
 * and at least cents. Also returns that yearly sum.
 */
function recurringRevenue(subscriptions: Subscription[]): { arrExact: string; mrrExact: string } {
  const arrExact = sumDecimals(
    subscriptions.flatMap((subscription) => {
      const periods = PERIODS_PER_YEAR[subscription.frequency];
      return periods ? [multiplyDecimal(subscriptionPrice(subscription), periods)] : [];
    }),
  );
  return { arrExact, mrrExact: divideDecimal(arrExact, 12, Math.max(2, decimalScale(arrExact))) };
}

/** Epoch milliseconds of an RFC 3339 field, or undefined when absent or unparseable. */
function timestampOf(value: string | undefined): number | undefined {
  const parsed = value ? Date.parse(value) : Number.NaN;
  return Number.isNaN(parsed) ? undefined : parsed;
}

/**
 * Subscription analytics computed from the subscription records, for
 * engines without an aggregate. Recurring revenue covers active
 * subscriptions whose interval is known, in exact decimals: ARR is the exact
 * sum of price x periods per year, and MRR and ARPU are rounded once from
 * it. Figures nothing reports (growth, lifetime) are left at zero rather
 * than invented.
 *
 * Measured against `now` (the demo clock in demo mode): starts and
 * cancellations since `SUBSCRIPTION_ACTIVITY_DAYS` before it, from
 * `createdAt` and `cancelledAt`, and active subscriptions whose next billing
 * date falls in the `SUBSCRIPTION_RENEWAL_DAYS` after it. A cancelled
 * subscription without a `cancelledAt` makes the cancellation figures
 * unknown (null) rather than undercounted.
 */
export function summarizeSubscriptions(
  subscriptions: Subscription[],
  options: { complete?: boolean; now?: number } = {},
): SubscriptionAnalytics {
  const now = options.now ?? Date.now();
  const active = subscriptions.filter((subscription) => subscription.status === 'active');
  const { arrExact, mrrExact } = recurringRevenue(active);
  const scale = Math.max(2, decimalScale(arrExact));
  const arpuExact = active.length ? divideDecimal(arrExact, 12 * active.length, scale) : '0';
  const churned = subscriptions.filter(
    (subscription) => subscription.status === 'cancelled' || subscription.status === 'expired',
  ).length;
  const keyOf = (value: string | undefined) => value || 'unknown';
  const tally = (key: (subscription: Subscription) => string | undefined) =>
    subscriptions.reduce<Record<string, number>>((acc, subscription) => {
      const k = keyOf(key(subscription));
      acc[k] = (acc[k] || 0) + 1;
      return acc;
    }, {});

  const plans = new Map<string, Subscription[]>();
  for (const subscription of active) {
    const plan = keyOf(subscription.plan);
    const members = plans.get(plan);
    if (members) members.push(subscription);
    else plans.set(plan, [subscription]);
  }
  const activeByPlan = Array.from(plans, ([plan, members]) => ({
    plan,
    count: members.length,
    mrrExact: recurringRevenue(members).mrrExact,
  })).sort((left, right) => right.count - left.count || left.plan.localeCompare(right.plan));

  const since = now - SUBSCRIPTION_ACTIVITY_DAYS * DAY_MS;
  const started = subscriptions.filter(
    (subscription) => (timestampOf(subscription.createdAt) ?? -Infinity) >= since,
  );
  const cancellationDatesKnown = subscriptions.every(
    (subscription) =>
      subscription.status !== 'cancelled' || timestampOf(subscription.cancelledAt) !== undefined,
  );
  const cancelled = subscriptions.filter(
    (subscription) => (timestampOf(subscription.cancelledAt) ?? -Infinity) >= since,
  );

  const until = now + SUBSCRIPTION_RENEWAL_DAYS * DAY_MS;
  const renewals = active
    .map((subscription) => ({ subscription, at: timestampOf(subscription.nextBillingDate) }))
    .filter((entry): entry is { subscription: Subscription; at: number } => {
      return entry.at !== undefined && entry.at >= now && entry.at < until;
    })
    .sort((left, right) => left.at - right.at);

  return {
    totalSubscriptions: subscriptions.length,
    activeSubscriptions: active.length,
    mrr: Number(mrrExact),
    mrrExact,
    arr: Number(arrExact),
    arrExact,
    churnRate: subscriptions.length ? Number((churned / subscriptions.length).toFixed(3)) : 0,
    arpu: Number(arpuExact),
    arpuExact,
    averageLifetime: 0,
    subscriptionsByPlan: tally((subscription) => subscription.plan),
    subscriptionsByFrequency: tally((subscription) => subscription.frequency),
    subscriptionsByStatus: tally((subscription) => subscription.status),
    activeByPlan,
    recentActivity: {
      days: SUBSCRIPTION_ACTIVITY_DAYS,
      newSubscriptions: started.length,
      newMrrExact: recurringRevenue(
        started.filter((subscription) => subscription.status === 'active'),
      ).mrrExact,
      cancellations: cancellationDatesKnown ? cancelled.length : null,
      churnedMrrExact: cancellationDatesKnown ? recurringRevenue(cancelled).mrrExact : null,
    },
    upcomingRenewals: {
      days: SUBSCRIPTION_RENEWAL_DAYS,
      count: renewals.length,
      amountExact: sumDecimals(renewals.map(({ subscription }) => subscriptionPrice(subscription))),
      next: renewals.slice(0, SUBSCRIPTION_RENEWALS_LISTED).map(({ subscription, at }) => ({
        id: subscription.id,
        customerId: subscription.customerId,
        plan: keyOf(subscription.plan),
        renewalDate: new Date(at).toISOString().slice(0, 10),
        amountExact: subscriptionPrice(subscription),
      })),
    },
    coverage: coverageOf(subscriptions, options.complete ?? true),
  };
}

export const subscriptionsApi = {
  async list(params?: {
    status?: string;
    customerId?: string;
    limit?: number;
    offset?: number;
  }): Promise<Subscription[]> {
    const commerce = await getCommerceEngine();
    return (await commerce.subscriptions.list(params)).map(toAdminSubscription);
  },

  async get(subscriptionId: string): Promise<Subscription | null> {
    const commerce = await getCommerceEngine();
    const record = await commerce.subscriptions.get(subscriptionId);
    return record ? toAdminSubscription(record) : null;
  },

  /**
   * Start a subscription. The engine's `subscribe` needs the customer and the
   * plan; everything else (price, interval, trial) comes from the plan.
   */
  async create(params: Partial<Subscription>): Promise<Subscription> {
    if (!params.customerId || !params.planId) {
      throw new Error('A subscription needs a customerId and a planId');
    }
    const commerce = await getCommerceEngine();
    return toAdminSubscription(
      await commerce.subscriptions.subscribe({
        customerId: params.customerId,
        planId: params.planId,
      }),
    );
  },

  async pause(subscriptionId: string): Promise<Subscription> {
    const commerce = await getCommerceEngine();
    return toAdminSubscription(await commerce.subscriptions.pause(subscriptionId));
  },

  async resume(subscriptionId: string): Promise<Subscription> {
    const commerce = await getCommerceEngine();
    return toAdminSubscription(await commerce.subscriptions.resume(subscriptionId));
  },

  async cancel(subscriptionId: string, reason?: string): Promise<Subscription> {
    const commerce = await getCommerceEngine();
    // The binding takes `CancelSubscriptionInput`, not a bare reason string.
    return toAdminSubscription(
      await commerce.subscriptions.cancel(subscriptionId, reason ? { reason } : undefined),
    );
  },

  async getAnalytics(): Promise<SubscriptionAnalytics> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.subscriptions.getAnalytics === 'function') {
      return commerce.subscriptions.getAnalytics();
    }
    const { records, complete } = await collectAllPages((page) =>
      commerce.subscriptions.list(page),
    );
    return summarizeSubscriptions(records.map(toAdminSubscription), { complete });
  },
};

// ============================================================================
// Finance API (General Ledger + Accounts Payable)
// ============================================================================

export interface AutoPostingConfigInput {
  configName: string;
  cashAccountId: string;
  accountsReceivableAccountId: string;
  inventoryAccountId: string;
  accountsPayableAccountId: string;
  unearnedRevenueAccountId?: string;
  salesRevenueAccountId: string;
  shippingRevenueAccountId?: string;
  salesTaxPayableAccountId?: string;
  goodsReceivedNotInvoicedAccountId?: string;
  inventoryAdjustmentAccountId?: string;
  workInProgressAccountId?: string;
  cogsAccountId: string;
  badDebtExpenseAccountId?: string;
  fxGainLossAccountId?: string;
  autoPostDepreciation: boolean;
  autoPostRevenueRecognition: boolean;
}
export interface AutoPostingConfig extends AutoPostingConfigInput {
  id: string;
  isActive: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface GlAccount {
  isHeader?: boolean;
  isPosting?: boolean;
  currency?: string;
  id: string;
  accountNumber: string;
  name: string;
  /** asset, liability, equity, revenue, expense (normalised from the binding's `Debug` form) */
  accountType: string;
  balance: number;
  status: GlAccountStatus;
  description?: string;
}

export interface JournalEntry {
  id: string;
  entryNumber: string;
  entryDate: string;
  description: string;
  status: JournalEntryStatus;
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
  currency?: string;
  status: BillStatus;
  totalAmount: string | number;
  amountPaid: string | number;
  amountDue: string | number;
  dueDate: string;
  createdAt: string;
}

export interface ApAgingSummary {
  current: string | number;
  days130: string | number;
  days3160: string | number;
  days6190: string | number;
  daysOver90: string | number;
  total: string | number;
}

export interface ArAgingSummary {
  current: string | number;
  days130: string | number;
  days3160: string | number;
  days6190: string | number;
  daysOver90: string | number;
  total: string | number;
}

export interface Invoice {
  id: string;
  invoiceNumber: string;
  customerId: string;
  currency?: string;
  balanceDue?: string;
  orderId?: string;
  status: InvoiceStatus;
  subtotal: string | number;
  taxAmount: string | number;
  total: string | number;
  amountPaid: string | number;
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

  const glAccounts = accountSpecs.map(
    ([accountNumber, name, accountType, balance]) =>
      ({
        id: `gl_${accountNumber}`,
        accountNumber,
        name,
        accountType,
        balance,
        status: 'active',
        description: `${name} account`,
      }) satisfies GlAccount,
  );

  // The engine's journal entry vocabulary (`GlJournalEntryStatus`).
  const journalStatuses: JournalEntryStatus[] = [
    'draft',
    'posted',
    'posted',
    'pending',
    'posted',
    'posted',
    'reversed',
    'posted',
    'voided',
    'posted',
    'posted',
    'posted',
  ];
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
      status: journalStatuses[index],
      createdAt,
    } satisfies JournalEntry;
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

  // The engine's bill vocabulary (`BillStatus`, normalised from its `Debug` render).
  const billStatuses: BillStatus[] = [
    'pending',
    'approved',
    'paid',
    'overdue',
    'draft',
    'partially_paid',
    'approved',
    'disputed',
    'cancelled',
  ];
  const bills = Array.from({ length: 18 }, (_, index) => {
    const totalAmount = roundCurrency(320 + deterministicNumber(index + 1601, 0, 4200));
    const status = billStatuses[index % billStatuses.length];
    const amountPaid =
      status === 'paid'
        ? totalAmount
        : status === 'partially_paid'
          ? roundCurrency(totalAmount * 0.4)
          : 0;
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

  // Same scope as the engine's AP aging: everything but paid and cancelled.
  const openBills = bills.filter(
    (bill) => !statusIn(AP_AGING_EXCLUDED_BILL_STATUSES, bill.status) && bill.amountDue > 0,
  );
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

  const invoiceStatuses: InvoiceStatus[] = [
    'sent',
    'paid',
    'overdue',
    'viewed',
    'partially_paid',
    'disputed',
    'draft',
    'sent',
  ];
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

  // Same scope as the engine's AR aging: issued, open invoices only.
  const openInvoices = invoices.filter(
    (invoice) =>
      statusIn(AR_OPEN_INVOICE_STATUSES, invoice.status) && invoice.total - invoice.amountPaid > 0,
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
  async getAutoPostingConfig(): Promise<AutoPostingConfig | null> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.generalLedger.getAutoPostingConfig !== 'function') {
      throw new Error('This engine build does not expose posting settings');
    }
    return commerce.generalLedger.getAutoPostingConfig();
  },
  async setAutoPostingConfig(input: AutoPostingConfigInput): Promise<AutoPostingConfig> {
    const commerce = await getCommerceEngine();
    if (typeof commerce.generalLedger.setAutoPostingConfig !== 'function') {
      throw new Error('This engine build does not expose posting settings');
    }
    return commerce.generalLedger.setAutoPostingConfig(input);
  },
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
  async getDso(days: number): Promise<string | number | null> {
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
// Status (and other enum) fields carry the engine's snake_case vocabulary:
// `adaptNativeEngine` normalises the binding's `Debug`-cased renders.
// ============================================================================

/** A purchase order header (`commerce.purchaseOrders.list`). */
export interface PurchaseOrder {
  id: string;
  poNumber: string;
  supplierId: string;
  status: PurchaseOrderStatus;
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
  /** distribution, manufacturing, retail, third_party, consignment, returns */
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
  /** bulk, pick, staging, receiving, shipping, quarantine, returns, production, packing, cross_dock */
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
  status: WorkOrderStatus;
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
  /** incoming, receiving, in_process, final, random, return */
  inspectionType: string;
  referenceType: string;
  referenceId: string;
  status: InspectionStatus;
  createdAt: string;
  updatedAt: string;
}

/** A non-conformance report (`commerce.quality.listNcrs`). */
export interface NonConformanceReport {
  id: string;
  ncrNumber: string;
  /** inspection, customer_complaint, internal_audit, supplier_issue, production_defect, shipping_damage */
  source: string;
  severity: NcrSeverity;
  sku: string;
  quantityAffected: number;
  status: NcrStatus;
  description: string;
  createdAt: string;
}

/** A fulfillment wave (`commerce.fulfillment.listWaves`). */
export interface Wave {
  id: string;
  waveNumber: string;
  warehouseId: number;
  orderCount: number;
  status: WaveStatus;
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
  status: PickTaskStatus;
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
  status: LotStatus;
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
  status: SerialStatus;
  ownerId?: string;
  locationId?: number;
  createdAt: string;
}

/** An inbound receipt (`commerce.receiving.listReceipts`). */
export interface Receipt {
  id: string;
  receiptNumber: string;
  /** purchase_order, transfer, return, adjustment, production, other */
  receiptType: string;
  warehouseId: number;
  status: ReceiptStatus;
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

  const poStatuses: PurchaseOrderStatus[] = [
    'draft',
    'pending_approval',
    'approved',
    'sent',
    'acknowledged',
    'partially_received',
    'received',
    'completed',
    'on_hold',
    'cancelled',
  ];
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
    ['WH-EAST', 'Columbus Fulfillment', 'third_party'],
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

  const locationTypes = ['pick', 'bulk', 'staging', 'receiving'];
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

  const workOrderStatuses: WorkOrderStatus[] = [
    'planned',
    'in_progress',
    'completed',
    'partially_completed',
    'on_hold',
    'cancelled',
  ];
  const priorities = ['low', 'normal', 'high', 'urgent'];
  const workOrders = Array.from({ length: 16 }, (_, index) => {
    const status = workOrderStatuses[index % workOrderStatuses.length];
    const quantityToBuild = 25 + deterministicNumber(index + 2301, 0, 475);
    const quantityCompleted =
      status === 'completed'
        ? quantityToBuild
        : status === 'in_progress' || status === 'partially_completed'
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
  const inspectionStatuses: InspectionStatus[] = [
    'pending',
    'scheduled',
    'in_progress',
    'passed',
    'failed',
    'partial_pass',
    'on_hold',
    'cancelled',
  ];
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

  const severities: NcrSeverity[] = ['minor', 'major', 'critical', 'observation'];
  const ncrStatuses: NcrStatus[] = [
    'open',
    'under_review',
    'closed',
    'pending_disposition',
    'corrective_action',
    'cancelled',
  ];
  const ncrs = Array.from(
    { length: 9 },
    (_, index) =>
      ({
        id: `ncr_${9700 + index}`,
        ncrNumber: `NCR-${9700 + index}`,
        source: index % 2 === 0 ? 'inspection' : 'customer_complaint',
        severity: severities[index % severities.length],
        sku: `SKU-${1000 + (index % 40)}`,
        quantityAffected: 1 + deterministicNumber(index + 2401, 0, 60),
        status: ncrStatuses[index % ncrStatuses.length],
        description: 'Dimensional tolerance out of spec on inbound lot',
        createdAt: formatMockDate(-(index * 6) - 2, index % 8),
      }) satisfies NonConformanceReport,
  );

  const waveStatuses: WaveStatus[] = ['draft', 'released', 'in_progress', 'completed', 'cancelled'];
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

  const pickStatuses: PickTaskStatus[] = [
    'pending',
    'assigned',
    'in_progress',
    'completed',
    'short',
    'cancelled',
  ];
  const picks = Array.from({ length: 24 }, (_, index) => {
    const status = pickStatuses[index % pickStatuses.length];
    const quantityRequested = 1 + deterministicNumber(index + 2601, 0, 24);
    const quantityPicked =
      status === 'completed'
        ? quantityRequested
        : status === 'in_progress' || status === 'short'
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

  const lotStatuses: LotStatus[] = [
    'active',
    'active',
    'quarantine',
    'expired',
    'consumed',
    'on_hold',
    'recalled',
  ];
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

  const serialStatuses: SerialStatus[] = [
    'available',
    'reserved',
    'sold',
    'quarantined',
    'shipped',
    'returned',
  ];
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

  const receiptStatuses: ReceiptStatus[] = [
    'expected',
    'in_progress',
    'received',
    'putting_away',
    'completed',
    'cancelled',
  ];
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

/**
 * Mock objects for testing
 *
 * Provides reusable mocks for Commerce, Logger, Telemetry, and PermissionGate
 * to reduce boilerplate in test files.
 */

/**
 * Create a mock Commerce instance with stubs for all resource types.
 * Each method returns sensible defaults or can be overridden.
 * @param {object} [overrides] - Override specific resource methods
 * @returns {object} Mock commerce instance
 */
export function createMockCommerce(overrides = {}) {
  const customers = {
    list: async () => [],
    get: async () => null,
    getByEmail: async () => null,
    create: async (data) => ({ id: 'cust_test_001', ...data, createdAt: new Date().toISOString() }),
    count: async () => 0,
    ...overrides.customers,
  };

  const orders = {
    list: async () => [],
    get: async () => null,
    create: async (data) => ({
      id: 'ord_test_001',
      status: 'pending',
      ...data,
      createdAt: new Date().toISOString(),
    }),
    updateStatus: async (id, status) => ({ id, status }),
    ship: async (id, tracking) => ({ id, status: 'shipped', tracking }),
    cancel: async (id) => ({ id, status: 'cancelled' }),
    count: async () => 0,
    ...overrides.orders,
  };

  const products = {
    list: async () => [],
    get: async () => null,
    getVariant: async () => null,
    create: async (data) => ({ id: 'prod_test_001', ...data }),
    count: async () => 0,
    ...overrides.products,
  };

  const inventory = {
    getStock: async () => ({ quantity: 0, reserved: 0, available: 0 }),
    create: async (data) => ({ id: 'inv_test_001', ...data }),
    adjust: async (sku, qty) => ({ sku, quantity: qty }),
    reserve: async (sku, qty) => ({ sku, reserved: qty }),
    confirm: async (reservationId) => ({ id: reservationId, confirmed: true }),
    release: async (reservationId) => ({ id: reservationId, released: true }),
    ...overrides.inventory,
  };

  const returns = {
    list: async () => [],
    get: async () => null,
    create: async (data) => ({ id: 'ret_test_001', status: 'pending', ...data }),
    approve: async (id) => ({ id, status: 'approved' }),
    reject: async (id, reason) => ({ id, status: 'rejected', reason }),
    ...overrides.returns,
  };

  const carts = {
    list: async () => [],
    get: async () => null,
    create: async (data) => ({ id: 'cart_test_001', items: [], ...data }),
    addItem: async (cartId, item) => ({ cartId, item }),
    updateItem: async (cartId, itemId, data) => ({ cartId, itemId, ...data }),
    removeItem: async (cartId, itemId) => ({ cartId, itemId }),
    setShippingAddress: async (cartId, addr) => ({ cartId, ...addr }),
    setPayment: async (cartId, payment) => ({ cartId, ...payment }),
    applyDiscount: async (cartId, code) => ({ cartId, code }),
    checkout: async (cartId) => ({ orderId: 'ord_test_001', cartId }),
    cancel: async (cartId) => ({ cartId, status: 'cancelled' }),
    abandon: async (cartId) => ({ cartId, status: 'abandoned' }),
    getAbandoned: async () => [],
    getShippingRates: async () => [],
    ...overrides.carts,
  };

  const payments = {
    list: async () => [],
    get: async () => null,
    create: async (data) => ({ id: 'pay_test_001', status: 'pending', ...data }),
    complete: async (id) => ({ id, status: 'completed' }),
    refund: async (id, data) => ({ id: 'ref_test_001', paymentId: id, ...data }),
    ...overrides.payments,
  };

  const analytics = {
    getSalesSummary: async () => ({ revenue: 0, orders: 0, aov: 0 }),
    getTopProducts: async () => [],
    getCustomerMetrics: async () => ({ total: 0, new: 0, returning: 0 }),
    getTopCustomers: async () => [],
    getInventoryHealth: async () => ({ inStock: 0, lowStock: 0, outOfStock: 0 }),
    getLowStockItems: async () => [],
    getDemandForecast: async () => [],
    getRevenueForecast: async () => ({ predicted: 0 }),
    getOrderStatusBreakdown: async () => ({}),
    getReturnMetrics: async () => ({ returnRate: 0, totalRefunds: 0 }),
    ...overrides.analytics,
  };

  const shipments = {
    list: async () => [],
    create: async (data) => ({ id: 'ship_test_001', ...data }),
    deliver: async (id) => ({ id, status: 'delivered' }),
    ...overrides.shipments,
  };

  const suppliers = {
    list: async () => [],
    create: async (data) => ({ id: 'sup_test_001', ...data }),
    ...overrides.suppliers,
  };

  const purchaseOrders = {
    list: async () => [],
    create: async (data) => ({ id: 'po_test_001', status: 'draft', ...data }),
    approve: async (id) => ({ id, status: 'approved' }),
    send: async (id) => ({ id, status: 'sent' }),
    ...overrides.purchaseOrders,
  };

  const invoices = {
    list: async () => [],
    create: async (data) => ({ id: 'inv_test_001', status: 'draft', ...data }),
    send: async (id) => ({ id, status: 'sent' }),
    recordPayment: async (id, data) => ({ id, status: 'paid', ...data }),
    getOverdue: async () => [],
    ...overrides.invoices,
  };

  const warranties = {
    list: async () => [],
    create: async (data) => ({ id: 'war_test_001', ...data }),
    createClaim: async (data) => ({ id: 'claim_test_001', status: 'pending', ...data }),
    approveClaim: async (id) => ({ id, status: 'approved' }),
    ...overrides.warranties,
  };

  const promotions = {
    list: async () => [],
    get: async () => null,
    create: async (data) => ({ id: 'promo_test_001', status: 'draft', ...data }),
    activate: async (id) => ({ id, status: 'active' }),
    deactivate: async (id) => ({ id, status: 'inactive' }),
    ...overrides.promotions,
  };

  const coupons = {
    list: async () => [],
    create: async (data) => ({ id: 'coupon_test_001', ...data }),
    validate: async (code) => ({ valid: true, code }),
    ...overrides.coupons,
  };

  const subscriptions = {
    plans: {
      list: async () => [],
      get: async () => null,
      create: async (data) => ({ id: 'plan_test_001', ...data }),
      activate: async (id) => ({ id, status: 'active' }),
      archive: async (id) => ({ id, status: 'archived' }),
      ...(overrides.subscriptions?.plans || {}),
    },
    list: async () => [],
    get: async () => null,
    create: async (data) => ({ id: 'sub_test_001', status: 'active', ...data }),
    pause: async (id) => ({ id, status: 'paused' }),
    resume: async (id) => ({ id, status: 'active' }),
    cancel: async (id) => ({ id, status: 'cancelled' }),
    skip: async (id) => ({ id, skipped: true }),
    getBillingCycles: async () => [],
    getBillingCycle: async () => null,
    getEvents: async () => [],
    ...overrides.subscriptions,
  };

  const manufacturing = {
    boms: {
      list: async () => [],
      get: async () => null,
      create: async (data) => ({ id: 'bom_test_001', ...data }),
      addComponent: async (bomId, data) => ({ bomId, ...data }),
      activate: async (id) => ({ id, status: 'active' }),
      ...(overrides.manufacturing?.boms || {}),
    },
    workOrders: {
      list: async () => [],
      get: async () => null,
      create: async (data) => ({ id: 'wo_test_001', status: 'draft', ...data }),
      start: async (id) => ({ id, status: 'in_progress' }),
      complete: async (id, qty) => ({ id, status: 'completed', quantity: qty }),
      cancel: async (id) => ({ id, status: 'cancelled' }),
      ...(overrides.manufacturing?.workOrders || {}),
    },
    ...overrides.manufacturing,
  };

  const currency = {
    getRate: async () => ({ rate: 1.0 }),
    listRates: async () => [],
    convert: async (amount) => ({ converted: amount }),
    setRate: async (data) => data,
    getSettings: async () => ({ baseCurrency: 'USD' }),
    setBaseCurrency: async (c) => ({ baseCurrency: c }),
    enableCurrencies: async (list) => ({ enabled: list }),
    format: (amount, currency) => `${currency} ${amount}`,
    ...overrides.currency,
  };

  const tax = {
    calculate: async () => ({ tax: 0, total: 0 }),
    calculateCart: async () => ({ tax: 0 }),
    getRate: async () => ({ rate: 0 }),
    listJurisdictions: async () => [],
    listRates: async () => [],
    getSettings: async () => ({}),
    getStateInfo: async () => null,
    getExemptions: async () => [],
    createExemption: async (data) => ({ id: 'exempt_test_001', ...data }),
    ...overrides.tax,
  };

  const vector = {
    search: async () => [],
    index: async () => ({ indexed: true }),
    delete: async () => ({ deleted: true }),
    ...overrides.vector,
  };

  return {
    customers,
    orders,
    products,
    inventory,
    returns,
    carts,
    payments,
    analytics,
    shipments,
    suppliers,
    purchaseOrders,
    invoices,
    warranties,
    promotions,
    coupons,
    subscriptions,
    manufacturing,
    currency,
    tax,
    vector,
  };
}

/**
 * Create a mock logger that captures log calls for assertions.
 * @returns {{ info: Function, warn: Function, error: Function, debug: Function, trace: Function, child: Function, calls: object }}
 */
export function createMockLogger() {
  const calls = { info: [], warn: [], error: [], debug: [], trace: [] };
  const mockLogger = {
    info: (msg, meta) => calls.info.push({ msg, meta }),
    warn: (msg, meta) => calls.warn.push({ msg, meta }),
    error: (msg, meta) => calls.error.push({ msg, meta }),
    debug: (msg, meta) => calls.debug.push({ msg, meta }),
    trace: (msg, meta) => calls.trace.push({ msg, meta }),
    child: () => mockLogger,
    time: () => {},
    timeEnd: () => 0,
    calls,
  };
  return mockLogger;
}

/**
 * Create a mock telemetry instance.
 * @returns {object} Mock telemetry
 */
export function createMockTelemetry() {
  const spans = [];
  return {
    startSpan: (name) => {
      const span = { id: `span_${spans.length}`, name, ended: false };
      spans.push(span);
      return span;
    },
    endSpan: (span) => {
      if (span) span.ended = true;
    },
    logToolCall: () => {},
    logError: () => {},
    spans,
  };
}

/**
 * Create a mock permission gate.
 * @param {'read'|'write'|'admin'} [level='write'] - Permission level to grant
 * @returns {object} Mock permission gate
 */
export function createMockPermissionGate(level = 'write') {
  const auditLog = [];
  return {
    checkPermission: async (toolName, requiredLevel) => ({
      allowed: true,
      level,
      toolName,
      requiredLevel,
    }),
    audit: (entry) => auditLog.push(entry),
    auditLog,
  };
}

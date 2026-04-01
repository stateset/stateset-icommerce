/**
 * End-to-end commerce workflow tests
 *
 * These tests exercise complete business scenarios by calling MCP tool handlers
 * directly with a stateful in-memory commerce mock. Each workflow chains
 * multiple tool calls together, verifying intermediate and final state.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';

// Domain tool modules
import { customerTools } from '../../src/tools/customers.js';
import { orderTools } from '../../src/tools/orders.js';
import { productTools } from '../../src/tools/products.js';
import { inventoryTools } from '../../src/tools/inventory.js';
import { paymentTools } from '../../src/tools/payments.js';
import { shipmentTools } from '../../src/tools/shipments.js';
import { returnTools } from '../../src/tools/returns.js';
import { cartTools } from '../../src/tools/carts.js';
import { subscriptionTools } from '../../src/tools/subscriptions.js';
import { currencyTools } from '../../src/tools/currency.js';
import { giftCardTools } from '../../src/tools/gift-cards.js';
import { loyaltyTools } from '../../src/tools/loyalty.js';

// ---------------------------------------------------------------------------
// Helper: find a tool handler by name from a tools array
// ---------------------------------------------------------------------------

function findTool(toolsArray, name) {
  const tool = toolsArray.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool "${name}" not found`);
  return tool;
}

/**
 * Invoke a tool handler with the shared context.
 * @param {Array} toolsArray - The tool module's exported array
 * @param {string} toolName - Tool name to invoke
 * @param {object} params - Tool parameters
 * @param {object} ctx - Shared context (commerce, allowApply, etc.)
 */
async function callTool(toolsArray, toolName, params, ctx) {
  const tool = findTool(toolsArray, toolName);
  return tool.handler({ ...ctx, params });
}

// ---------------------------------------------------------------------------
// Stateful in-memory commerce mock
//
// Each domain stores entities in Maps keyed by id. Mutations update the maps
// so that subsequent reads in the same test return consistent state.
// ---------------------------------------------------------------------------

function createStatefulCommerce() {
  const stores = {
    customers: new Map(),
    products: new Map(),
    inventory: new Map(),      // keyed by SKU
    reservations: new Map(),
    orders: new Map(),
    payments: new Map(),
    shipments: new Map(),
    returns: new Map(),
    carts: new Map(),
    cartItems: new Map(),
    exchangeRates: new Map(),
    currencySettings: { baseCurrency: 'USD', enabledCurrencies: ['USD'], autoConvert: false },
    giftCards: new Map(),
    giftCardTransactions: [],
    loyaltyPrograms: new Map(),
    loyaltyAccounts: new Map(), // keyed by `${programId}:${customerId}`
    loyaltyTransactions: [],
    subscriptionPlans: new Map(),
    subscriptions: new Map(),
    billingCycles: [],
    subscriptionEvents: [],
  };

  let orderSeq = 1000;
  let cartSeq = 5000;
  let subSeq = 7000;

  // ---- Customers ----
  const customers = {
    list: async () => [...stores.customers.values()],
    get: async (id) => stores.customers.get(id) || null,
    getByEmail: async (email) =>
      [...stores.customers.values()].find((c) => c.email === email) || null,
    create: async (data) => {
      const id = randomUUID();
      const customer = {
        id,
        ...data,
        status: 'active',
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      };
      stores.customers.set(id, customer);
      return customer;
    },
    count: async () => stores.customers.size,
  };

  // ---- Products ----
  const products = {
    list: async () => [...stores.products.values()],
    get: async (id) => stores.products.get(id) || null,
    getVariantBySku: async (sku) =>
      [...stores.products.values()].find((p) => p.sku === sku) || null,
    create: async (data) => {
      const id = randomUUID();
      const product = {
        id,
        ...data,
        status: 'active',
        slug: data.name.toLowerCase().replace(/\s+/g, '-'),
        createdAt: new Date().toISOString(),
      };
      stores.products.set(id, product);
      return product;
    },
    count: async () => stores.products.size,
  };

  // ---- Inventory ----
  const inventory = {
    getStock: async (sku) => stores.inventory.get(sku) || null,
    createItem: async (data) => {
      const id = randomUUID();
      const item = {
        id,
        sku: data.sku,
        name: data.name,
        description: data.description || '',
        totalOnHand: data.initialQuantity || 0,
        totalAllocated: 0,
        totalAvailable: data.initialQuantity || 0,
        reorderPoint: data.reorderPoint || 0,
      };
      stores.inventory.set(data.sku, item);
      return item;
    },
    adjust: async (sku, quantity, _reason) => {
      const item = stores.inventory.get(sku);
      if (!item) throw new Error(`Inventory not found for SKU ${sku}`);
      item.totalOnHand += quantity;
      item.totalAvailable += quantity;
      return item;
    },
    reserve: async (sku, quantity, referenceType, referenceId, _expiresInSeconds) => {
      const item = stores.inventory.get(sku);
      if (!item) throw new Error(`Inventory not found for SKU ${sku}`);
      if (item.totalAvailable < quantity)
        throw new Error(`Insufficient stock for ${sku}: need ${quantity}, have ${item.totalAvailable}`);
      const resId = randomUUID();
      item.totalAllocated += quantity;
      item.totalAvailable -= quantity;
      const reservation = {
        id: resId,
        sku,
        quantity,
        status: 'reserved',
        referenceType,
        referenceId,
      };
      stores.reservations.set(resId, reservation);
      return reservation;
    },
    confirmReservation: async (reservationId) => {
      const res = stores.reservations.get(reservationId);
      if (!res) throw new Error('Reservation not found');
      const item = stores.inventory.get(res.sku);
      item.totalOnHand -= res.quantity;
      item.totalAllocated -= res.quantity;
      res.status = 'confirmed';
      return res;
    },
    releaseReservation: async (reservationId) => {
      const res = stores.reservations.get(reservationId);
      if (!res) throw new Error('Reservation not found');
      const item = stores.inventory.get(res.sku);
      item.totalAllocated -= res.quantity;
      item.totalAvailable += res.quantity;
      res.status = 'released';
      return res;
    },
  };

  // ---- Orders ----
  const orders = {
    list: async () => [...stores.orders.values()],
    get: async (id) => stores.orders.get(id) || [...stores.orders.values()].find((o) => o.orderNumber === id) || null,
    create: async (data) => {
      const id = randomUUID();
      orderSeq += 1;
      const items = (data.items || []).map((i, idx) => ({
        id: randomUUID(),
        ...i,
        total: i.quantity * i.unitPrice,
      }));
      const totalAmount = items.reduce((s, i) => s + i.total, 0);
      const order = {
        id,
        orderNumber: `ORD-${orderSeq}`,
        customerId: data.customerId,
        status: 'pending',
        totalAmount,
        currency: data.currency || 'USD',
        paymentStatus: 'unpaid',
        fulfillmentStatus: 'unfulfilled',
        trackingNumber: null,
        items,
        notes: data.notes || '',
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      };
      stores.orders.set(id, order);
      return order;
    },
    updateStatus: async (id, status) => {
      const order = stores.orders.get(id);
      if (!order) throw new Error('Order not found');
      order.status = status;
      order.updatedAt = new Date().toISOString();
      return order;
    },
    ship: async (id, trackingNumber) => {
      const order = stores.orders.get(id);
      if (!order) throw new Error('Order not found');
      order.status = 'shipped';
      order.fulfillmentStatus = 'shipped';
      if (trackingNumber) order.trackingNumber = trackingNumber;
      return order;
    },
    cancel: async (id) => {
      const order = stores.orders.get(id);
      if (!order) throw new Error('Order not found');
      order.status = 'cancelled';
      return order;
    },
    count: async () => stores.orders.size,
  };

  // ---- Payments ----
  const payments = {
    list: async () => [...stores.payments.values()],
    get: async (id) => stores.payments.get(id) || null,
    create: async (data) => {
      const id = randomUUID();
      const payment = {
        id,
        orderId: data.orderId,
        amount: data.amount,
        currency: data.currency || 'USD',
        method: data.method || 'credit_card',
        status: 'pending',
        createdAt: new Date().toISOString(),
      };
      stores.payments.set(id, payment);
      return payment;
    },
    markCompleted: async (id) => {
      const payment = stores.payments.get(id);
      if (!payment) throw new Error('Payment not found');
      payment.status = 'completed';
      // Also update the order's paymentStatus
      const order = stores.orders.get(payment.orderId);
      if (order) order.paymentStatus = 'paid';
      return payment;
    },
    createRefund: async (data) => {
      const refundId = randomUUID();
      const payment = stores.payments.get(data.paymentId);
      if (payment) payment.status = 'refunded';
      return {
        id: refundId,
        paymentId: data.paymentId,
        amount: data.amount,
        reason: data.reason,
        status: 'completed',
        createdAt: new Date().toISOString(),
      };
    },
    count: async () => stores.payments.size,
  };

  // ---- Shipments ----
  const shipments = {
    list: async () => [...stores.shipments.values()],
    create: async (data) => {
      const id = randomUUID();
      const shipment = {
        id,
        orderId: data.orderId,
        carrier: data.carrier || 'USPS',
        service: data.service || 'standard',
        trackingNumber: `TRK-${Date.now()}`,
        status: 'in_transit',
        createdAt: new Date().toISOString(),
      };
      stores.shipments.set(id, shipment);
      return shipment;
    },
    deliver: async (id) => {
      const shipment = stores.shipments.get(id);
      if (!shipment) throw new Error('Shipment not found');
      shipment.status = 'delivered';
      // Also update the order
      const order = stores.orders.get(shipment.orderId);
      if (order) {
        order.status = 'delivered';
        order.fulfillmentStatus = 'delivered';
      }
      return shipment;
    },
    count: async () => stores.shipments.size,
  };

  // ---- Returns ----
  const returns = {
    list: async () => [...stores.returns.values()],
    get: async (id) => stores.returns.get(id) || null,
    create: async (data) => {
      const id = randomUUID();
      const ret = {
        id,
        orderId: data.orderId,
        status: 'pending',
        reason: data.reason,
        reasonDetails: data.reasonDetails || '',
        items: data.items,
        createdAt: new Date().toISOString(),
      };
      stores.returns.set(id, ret);
      return ret;
    },
    approve: async (id) => {
      const ret = stores.returns.get(id);
      if (!ret) throw new Error('Return not found');
      ret.status = 'approved';
      return ret;
    },
    reject: async (id, reason) => {
      const ret = stores.returns.get(id);
      if (!ret) throw new Error('Return not found');
      ret.status = 'rejected';
      ret.rejectionReason = reason;
      return ret;
    },
    count: async () => stores.returns.size,
  };

  // ---- Carts ----
  const carts = {
    list: async () => [...stores.carts.values()],
    get: async (id) => stores.carts.get(id) || null,
    getByNumber: async (num) => [...stores.carts.values()].find((c) => c.cartNumber === num) || null,
    create: async (data) => {
      const id = randomUUID();
      cartSeq += 1;
      const cart = {
        id,
        cartNumber: `CART-${cartSeq}`,
        customerId: data.customerId || null,
        customerEmail: data.customerEmail || null,
        customerName: data.customerName || null,
        status: 'active',
        paymentStatus: 'unpaid',
        currency: data.currency || 'USD',
        subtotal: 0,
        taxAmount: 0,
        shippingAmount: 0,
        discountAmount: 0,
        grandTotal: 0,
        paymentMethod: null,
        shippingMethod: null,
        couponCode: null,
        items: [],
        itemCount: 0,
        shippingAddress: null,
        billingAddress: null,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
        expiresAt: null,
      };
      stores.carts.set(id, cart);
      return cart;
    },
    addItem: async (cartId, itemData) => {
      const cart = stores.carts.get(cartId);
      if (!cart) throw new Error('Cart not found');
      const itemId = randomUUID();
      const item = {
        id: itemId,
        sku: itemData.sku,
        name: itemData.name,
        quantity: itemData.quantity,
        unitPrice: itemData.unitPrice,
        total: itemData.quantity * itemData.unitPrice,
        description: itemData.description || '',
      };
      cart.items.push(item);
      cart.itemCount = cart.items.length;
      cart.subtotal = cart.items.reduce((s, i) => s + i.total, 0);
      cart.grandTotal = cart.subtotal + cart.taxAmount + cart.shippingAmount - cart.discountAmount;
      stores.cartItems.set(itemId, { ...item, cartId });
      return item;
    },
    setShippingAddress: async (cartId, address) => {
      const cart = stores.carts.get(cartId);
      if (!cart) throw new Error('Cart not found');
      cart.shippingAddress = address;
      cart.shippingAmount = 9.99;
      cart.grandTotal = cart.subtotal + cart.taxAmount + cart.shippingAmount - cart.discountAmount;
      return cart;
    },
    setPayment: async (cartId, paymentData) => {
      const cart = stores.carts.get(cartId);
      if (!cart) throw new Error('Cart not found');
      cart.paymentMethod = paymentData.paymentMethod;
      return cart;
    },
    applyDiscount: async (cartId, code) => {
      const cart = stores.carts.get(cartId);
      if (!cart) throw new Error('Cart not found');
      cart.couponCode = code;
      cart.discountAmount = Math.round(cart.subtotal * 0.1 * 100) / 100; // 10% off
      cart.grandTotal = cart.subtotal + cart.taxAmount + cart.shippingAmount - cart.discountAmount;
      return cart;
    },
    getShippingRates: async (_cartId) => [
      { id: 'rate-1', carrier: 'USPS', service: 'Priority', price: 9.99, currency: 'USD', estimatedDays: 3 },
      { id: 'rate-2', carrier: 'FedEx', service: 'Ground', price: 7.99, currency: 'USD', estimatedDays: 5 },
    ],
    complete: async (cartId) => {
      const cart = stores.carts.get(cartId);
      if (!cart) throw new Error('Cart not found');
      cart.status = 'converted';
      // Create an order from the cart
      const order = await orders.create({
        customerId: cart.customerId,
        items: cart.items.map((i) => ({
          sku: i.sku,
          name: i.name,
          quantity: i.quantity,
          unitPrice: i.unitPrice,
        })),
        currency: cart.currency,
      });
      return {
        orderId: order.id,
        orderNumber: order.orderNumber,
        cartId: cart.id,
        totalCharged: cart.grandTotal,
        currency: cart.currency,
        paymentId: null,
      };
    },
    cancel: async (cartId) => {
      const cart = stores.carts.get(cartId);
      if (!cart) throw new Error('Cart not found');
      cart.status = 'cancelled';
      return cart;
    },
    removeItem: async (itemId) => {
      const itemMeta = stores.cartItems.get(itemId);
      if (!itemMeta) throw new Error('Cart item not found');
      const cart = stores.carts.get(itemMeta.cartId);
      cart.items = cart.items.filter((i) => i.id !== itemId);
      cart.itemCount = cart.items.length;
      cart.subtotal = cart.items.reduce((s, i) => s + i.total, 0);
      cart.grandTotal = cart.subtotal + cart.taxAmount + cart.shippingAmount - cart.discountAmount;
      return { id: itemId };
    },
    count: async () => stores.carts.size,
  };

  // ---- Currency ----
  const currency = {
    getRate: async (from, to) => {
      const key = `${from}:${to}`;
      return stores.exchangeRates.get(key) || null;
    },
    getRatesFor: async (base) =>
      [...stores.exchangeRates.values()].filter((r) => r.baseCurrency === base),
    listRates: async () => [...stores.exchangeRates.values()],
    convert: async ({ from, to, amount }) => {
      const key = `${from}:${to}`;
      const rateObj = stores.exchangeRates.get(key);
      if (!rateObj) throw new Error(`No rate for ${from} -> ${to}`);
      const converted = Math.round(amount * rateObj.rate * 100) / 100;
      return {
        originalAmount: amount,
        originalCurrency: from,
        convertedAmount: converted,
        targetCurrency: to,
        rate: rateObj.rate,
        inverseRate: Math.round((1 / rateObj.rate) * 10000) / 10000,
        rateAt: rateObj.rateAt,
      };
    },
    setRate: async (data) => {
      const id = randomUUID();
      const entry = {
        id,
        baseCurrency: data.baseCurrency,
        quoteCurrency: data.quoteCurrency,
        rate: data.rate,
        source: data.source || 'manual',
        rateAt: new Date().toISOString(),
      };
      stores.exchangeRates.set(`${data.baseCurrency}:${data.quoteCurrency}`, entry);
      return entry;
    },
    getSettings: async () => stores.currencySettings,
    setBaseCurrency: async (c) => {
      stores.currencySettings.baseCurrency = c;
      return stores.currencySettings;
    },
    enableCurrencies: async (list) => {
      stores.currencySettings.enabledCurrencies = list;
      return stores.currencySettings;
    },
    format: (amount, cur) => `${cur} ${amount}`,
  };

  // ---- Gift Cards ----
  const giftCards = {
    list: async (_filters) => [...stores.giftCards.values()],
    get: async (identifier) =>
      stores.giftCards.get(identifier) ||
      [...stores.giftCards.values()].find((gc) => gc.code === identifier) ||
      null,
    create: async (data) => {
      const id = randomUUID();
      const code = `GC-${Date.now().toString(36).toUpperCase()}`;
      const gc = {
        id,
        code,
        initialBalance: data.initialBalance,
        currentBalance: data.initialBalance,
        currency: data.currency || 'USD',
        status: 'active',
        customerId: data.customerId || null,
        recipientEmail: data.recipientEmail || null,
        expiresAt: data.expiresAt || null,
        createdAt: new Date().toISOString(),
      };
      stores.giftCards.set(id, gc);
      return gc;
    },
    charge: async (data) => {
      const gc = stores.giftCards.get(data.giftCardId);
      if (!gc) throw new Error('Gift card not found');
      const amt = parseFloat(data.amount);
      if (parseFloat(gc.currentBalance) < amt)
        throw new Error('Insufficient gift card balance');
      gc.currentBalance = String(parseFloat(gc.currentBalance) - amt);
      const tx = {
        id: randomUUID(),
        giftCardId: gc.id,
        type: 'charge',
        amount: data.amount,
        orderId: data.orderId || null,
        balanceAfter: gc.currentBalance,
        createdAt: new Date().toISOString(),
      };
      stores.giftCardTransactions.push(tx);
      return tx;
    },
    refund: async (data) => {
      const gc = stores.giftCards.get(data.giftCardId);
      if (!gc) throw new Error('Gift card not found');
      gc.currentBalance = String(parseFloat(gc.currentBalance) + parseFloat(data.amount));
      const tx = {
        id: randomUUID(),
        giftCardId: gc.id,
        type: 'refund',
        amount: data.amount,
        orderId: data.orderId || null,
        balanceAfter: gc.currentBalance,
        createdAt: new Date().toISOString(),
      };
      stores.giftCardTransactions.push(tx);
      return tx;
    },
    disable: async (id, _reason) => {
      const gc = stores.giftCards.get(id);
      if (!gc) throw new Error('Gift card not found');
      gc.status = 'disabled';
      return gc;
    },
    count: async () => stores.giftCards.size,
  };

  // ---- Loyalty ----
  const loyalty = {
    createProgram: async (data) => {
      const id = randomUUID();
      const program = {
        id,
        name: data.name,
        description: data.description || '',
        pointsPerDollar: data.pointsPerDollar || 1,
        currency: data.currency || 'USD',
        tiers: data.tiers || [],
        totalMembers: 0,
        status: 'active',
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      };
      stores.loyaltyPrograms.set(id, program);
      return program;
    },
    getProgram: async (id) => stores.loyaltyPrograms.get(id) || null,
    enrollCustomer: async (programId, customerId) => {
      const key = `${programId}:${customerId}`;
      const id = randomUUID();
      const account = {
        id,
        programId,
        customerId,
        pointsBalance: 0,
        lifetimePoints: 0,
        currentTier: null,
        nextTier: null,
        pointsToNextTier: null,
        enrolledAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      };
      stores.loyaltyAccounts.set(key, account);
      const program = stores.loyaltyPrograms.get(programId);
      if (program) program.totalMembers += 1;
      return account;
    },
    getAccount: async (programId, customerId) => {
      const key = `${programId}:${customerId}`;
      return stores.loyaltyAccounts.get(key) || null;
    },
    earnPoints: async (data) => {
      const key = `${data.programId}:${data.customerId}`;
      const account = stores.loyaltyAccounts.get(key);
      if (!account) throw new Error('Loyalty account not found');
      account.pointsBalance += data.points;
      account.lifetimePoints += data.points;
      account.updatedAt = new Date().toISOString();
      const tx = {
        id: randomUUID(),
        type: 'earn',
        programId: data.programId,
        customerId: data.customerId,
        points: data.points,
        reason: data.reason || 'manual',
        orderId: data.orderId || null,
        createdAt: new Date().toISOString(),
      };
      stores.loyaltyTransactions.push(tx);
      return tx;
    },
    redeemPoints: async (data) => {
      const key = `${data.programId}:${data.customerId}`;
      const account = stores.loyaltyAccounts.get(key);
      if (!account) throw new Error('Loyalty account not found');
      if (account.pointsBalance < data.points)
        throw new Error('Insufficient loyalty points');
      account.pointsBalance -= data.points;
      account.updatedAt = new Date().toISOString();
      const tx = {
        id: randomUUID(),
        type: 'redeem',
        programId: data.programId,
        customerId: data.customerId,
        points: data.points,
        orderId: data.orderId || null,
        createdAt: new Date().toISOString(),
      };
      stores.loyaltyTransactions.push(tx);
      return tx;
    },
    listRewards: async (_programId, _opts) => [],
  };

  // ---- Subscriptions (top-level methods on commerce) ----
  // The subscription tools call commerce.createSubscriptionPlan, etc.
  // directly, not commerce.subscriptions.*
  const topLevelSubMethods = {
    listSubscriptionPlans: async (_filters) => [...stores.subscriptionPlans.values()],
    getSubscriptionPlan: async (id) => stores.subscriptionPlans.get(id) || null,
    createSubscriptionPlan: async (data) => {
      const id = randomUUID();
      const plan = {
        id,
        code: `PLAN-${id.slice(0, 6).toUpperCase()}`,
        name: data.name,
        status: 'draft',
        billingInterval: data.billingInterval,
        price: data.price,
        currency: data.currency || 'USD',
        trialDays: data.trialDays || 0,
        description: data.description || '',
        setupFee: data.setupFee || null,
        createdAt: new Date().toISOString(),
      };
      stores.subscriptionPlans.set(id, plan);
      return plan;
    },
    activateSubscriptionPlan: async (planId) => {
      const plan = stores.subscriptionPlans.get(planId);
      if (!plan) throw new Error('Plan not found');
      plan.status = 'active';
      return plan;
    },
    archiveSubscriptionPlan: async (planId) => {
      const plan = stores.subscriptionPlans.get(planId);
      if (!plan) throw new Error('Plan not found');
      plan.status = 'archived';
      return plan;
    },
    listSubscriptions: async (filters) => {
      let subs = [...stores.subscriptions.values()];
      if (filters?.customerId) subs = subs.filter((s) => s.customerId === filters.customerId);
      if (filters?.planId) subs = subs.filter((s) => s.planId === filters.planId);
      if (filters?.status) subs = subs.filter((s) => s.status === filters.status);
      return subs;
    },
    getSubscription: async (id) => stores.subscriptions.get(id) || null,
    createSubscription: async (data) => {
      const id = randomUUID();
      subSeq += 1;
      const plan = stores.subscriptionPlans.get(data.planId);
      const sub = {
        id,
        subscriptionNumber: `SUB-${subSeq}`,
        customerId: data.customerId,
        planId: data.planId,
        planName: plan?.name || 'Unknown',
        status: plan?.trialDays > 0 && !data.skipTrial ? 'trial' : 'active',
        price: plan?.price || '0',
        currency: plan?.currency || 'USD',
        billingInterval: plan?.billingInterval || 'monthly',
        nextBillingDate: new Date(Date.now() + 30 * 86400000).toISOString(),
        billingCycleCount: 0,
        createdAt: new Date().toISOString(),
      };
      stores.subscriptions.set(id, sub);
      stores.subscriptionEvents.push({
        subscriptionId: id,
        type: 'created',
        at: new Date().toISOString(),
      });
      return sub;
    },
    pauseSubscription: async (id, _opts) => {
      const sub = stores.subscriptions.get(id);
      if (!sub) throw new Error('Subscription not found');
      sub.status = 'paused';
      stores.subscriptionEvents.push({ subscriptionId: id, type: 'paused', at: new Date().toISOString() });
      return sub;
    },
    resumeSubscription: async (id) => {
      const sub = stores.subscriptions.get(id);
      if (!sub) throw new Error('Subscription not found');
      sub.status = 'active';
      stores.subscriptionEvents.push({ subscriptionId: id, type: 'resumed', at: new Date().toISOString() });
      return sub;
    },
    cancelSubscription: async (id, opts) => {
      const sub = stores.subscriptions.get(id);
      if (!sub) throw new Error('Subscription not found');
      sub.status = 'cancelled';
      stores.subscriptionEvents.push({ subscriptionId: id, type: 'cancelled', at: new Date().toISOString() });
      return sub;
    },
    skipBillingCycle: async (id, _opts) => {
      const sub = stores.subscriptions.get(id);
      if (!sub) throw new Error('Subscription not found');
      sub.nextBillingDate = new Date(
        new Date(sub.nextBillingDate).getTime() + 30 * 86400000,
      ).toISOString();
      return sub;
    },
    listBillingCycles: async (filters) => {
      return stores.billingCycles.filter((c) => c.subscriptionId === filters?.subscriptionId);
    },
    getBillingCycle: async (id) => stores.billingCycles.find((c) => c.id === id) || null,
    getSubscriptionEvents: async (subId, _limit) =>
      stores.subscriptionEvents.filter((e) => e.subscriptionId === subId),
  };

  return {
    customers,
    products,
    inventory,
    orders,
    payments,
    shipments,
    returns,
    carts,
    currency,
    giftCards,
    loyalty,
    // Subscription methods are top-level on commerce
    ...topLevelSubMethods,
    // Expose stores for direct assertions
    _stores: stores,
  };
}

// ---------------------------------------------------------------------------
// Shared context builder
// ---------------------------------------------------------------------------

function makeCtx(commerce, overrides = {}) {
  return {
    commerce,
    allowApply: true,
    autoIndexEntity: () => {},
    ...overrides,
  };
}

// ============================================================================
// WORKFLOW 1: Order-to-Cash
// ============================================================================

describe('Workflow: Order-to-Cash', () => {
  let commerce;
  let ctx;

  beforeEach(() => {
    commerce = createStatefulCommerce();
    ctx = makeCtx(commerce);
  });

  it('should create a customer', async () => {
    const result = await callTool(customerTools, 'create_customer', {
      email: 'alice@example.com',
      firstName: 'Alice',
      lastName: 'Johnson',
    }, ctx);
    assert.equal(result.success, true);
    assert.ok(result.customer.id);
    assert.equal(result.customer.email, 'alice@example.com');
  });

  it('should create a product', async () => {
    const result = await callTool(productTools, 'create_product', {
      name: 'Widget Pro',
      sku: 'WIDGET-001',
      price: 29.99,
      description: 'A professional widget',
    }, ctx);
    assert.equal(result.success, true);
    assert.ok(result.product.id);
  });

  it('should add inventory for a product', async () => {
    const result = await callTool(inventoryTools, 'create_inventory_item', {
      sku: 'WIDGET-001',
      name: 'Widget Pro',
      initialQuantity: 100,
    }, ctx);
    assert.equal(result.success, true);
    assert.equal(result.item.sku, 'WIDGET-001');
  });

  it('should complete the full order-to-cash lifecycle', async () => {
    // Step 1: Create customer
    const custResult = await callTool(customerTools, 'create_customer', {
      email: 'bob@example.com',
      firstName: 'Bob',
      lastName: 'Smith',
    }, ctx);
    const customerId = custResult.customer.id;

    // Step 2: Create product
    await callTool(productTools, 'create_product', {
      name: 'Gadget X',
      sku: 'GADGET-X',
      price: 49.99,
    }, ctx);

    // Step 3: Add inventory
    await callTool(inventoryTools, 'create_inventory_item', {
      sku: 'GADGET-X',
      name: 'Gadget X',
      initialQuantity: 50,
    }, ctx);

    // Step 4: Create order
    const orderResult = await callTool(orderTools, 'create_order', {
      customerId,
      items: [{ sku: 'GADGET-X', name: 'Gadget X', quantity: 2, unitPrice: 49.99 }],
    }, ctx);
    assert.equal(orderResult.success, true);
    const orderId = orderResult.order.id;
    assert.equal(orderResult.order.status, 'pending');
    assert.equal(orderResult.order.totalAmount, 99.98);

    // Step 5: Create payment
    const payResult = await callTool(paymentTools, 'create_payment', {
      orderId,
      amount: 99.98,
      method: 'credit_card',
    }, ctx);
    assert.equal(payResult.success, true);
    const paymentId = payResult.payment.id;
    assert.equal(payResult.payment.status, 'pending');

    // Step 6: Complete payment
    const completePayResult = await callTool(paymentTools, 'complete_payment', {
      paymentId,
    }, ctx);
    assert.equal(completePayResult.success, true);
    assert.equal(completePayResult.payment.status, 'completed');

    // Step 7: Create shipment
    const shipResult = await callTool(shipmentTools, 'create_shipment', {
      orderId,
      carrier: 'FedEx',
    }, ctx);
    assert.equal(shipResult.success, true);
    const shipmentId = shipResult.shipment.id;
    assert.equal(shipResult.shipment.status, 'in_transit');

    // Step 8: Deliver shipment
    const deliverResult = await callTool(shipmentTools, 'deliver_shipment', {
      shipmentId,
    }, ctx);
    assert.equal(deliverResult.success, true);
    assert.equal(deliverResult.shipment.status, 'delivered');

    // Verify final state: order should be delivered
    const finalOrder = await commerce.orders.get(orderId);
    assert.equal(finalOrder.status, 'delivered');
    assert.equal(finalOrder.paymentStatus, 'paid');
    assert.equal(finalOrder.fulfillmentStatus, 'delivered');
  });

  it('should block writes without --apply flag', async () => {
    const noApplyCtx = makeCtx(commerce, { allowApply: false });
    const result = await callTool(customerTools, 'create_customer', {
      email: 'blocked@example.com',
      firstName: 'No',
      lastName: 'Apply',
    }, noApplyCtx);
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('should verify order state after payment and before shipment', async () => {
    const cust = await callTool(customerTools, 'create_customer', {
      email: 'eve@example.com', firstName: 'Eve', lastName: 'Chen',
    }, ctx);
    const order = await callTool(orderTools, 'create_order', {
      customerId: cust.customer.id,
      items: [{ sku: 'SKU-A', name: 'Item A', quantity: 1, unitPrice: 10 }],
    }, ctx);
    const pay = await callTool(paymentTools, 'create_payment', {
      orderId: order.order.id, amount: 10,
    }, ctx);
    await callTool(paymentTools, 'complete_payment', { paymentId: pay.payment.id }, ctx);

    const fetched = await commerce.orders.get(order.order.id);
    assert.equal(fetched.paymentStatus, 'paid');
    assert.equal(fetched.fulfillmentStatus, 'unfulfilled');
  });
});

// ============================================================================
// WORKFLOW 2: Return-to-Refund
// ============================================================================

describe('Workflow: Return-to-Refund', () => {
  let commerce;
  let ctx;

  beforeEach(() => {
    commerce = createStatefulCommerce();
    ctx = makeCtx(commerce);
  });

  it('should process a full return-to-refund cycle', async () => {
    // Setup: create customer + order + payment + shipment + delivery
    const cust = await callTool(customerTools, 'create_customer', {
      email: 'carol@example.com', firstName: 'Carol', lastName: 'Davis',
    }, ctx);
    const order = await callTool(orderTools, 'create_order', {
      customerId: cust.customer.id,
      items: [
        { sku: 'SHOE-001', name: 'Running Shoes', quantity: 1, unitPrice: 89.99 },
      ],
    }, ctx);
    const orderId = order.order.id;
    const orderItemId = commerce._stores.orders.get(orderId).items[0].id;

    const pay = await callTool(paymentTools, 'create_payment', {
      orderId, amount: 89.99,
    }, ctx);
    await callTool(paymentTools, 'complete_payment', { paymentId: pay.payment.id }, ctx);

    const ship = await callTool(shipmentTools, 'create_shipment', { orderId }, ctx);
    await callTool(shipmentTools, 'deliver_shipment', { shipmentId: ship.shipment.id }, ctx);

    // Step 1: Create return
    const retResult = await callTool(returnTools, 'create_return', {
      orderId,
      reason: 'wrong_item',
      items: [{ orderItemId, quantity: 1 }],
    }, ctx);
    assert.equal(retResult.success, true);
    assert.equal(retResult.return.status, 'pending');
    const returnId = retResult.return.id;

    // Step 2: Approve return
    const approveResult = await callTool(returnTools, 'approve_return', { returnId }, ctx);
    assert.equal(approveResult.success, true);
    assert.equal(approveResult.return.status, 'approved');

    // Step 3: Issue refund
    const refundResult = await callTool(paymentTools, 'create_refund', {
      paymentId: pay.payment.id,
      amount: 89.99,
      reason: 'Wrong item sent to customer',
    }, ctx);
    assert.equal(refundResult.success, true);
    assert.equal(refundResult.refund.status, 'completed');
    assert.equal(refundResult.refund.amount, '89.99');

    // Verify: payment should be refunded
    const finalPayment = await commerce.payments.get(pay.payment.id);
    assert.equal(finalPayment.status, 'refunded');
  });

  it('should allow partial refund on a return', async () => {
    const cust = await callTool(customerTools, 'create_customer', {
      email: 'dan@example.com', firstName: 'Dan', lastName: 'Lee',
    }, ctx);
    const order = await callTool(orderTools, 'create_order', {
      customerId: cust.customer.id,
      items: [
        { sku: 'SHIRT-001', name: 'T-Shirt', quantity: 3, unitPrice: 25.00 },
      ],
    }, ctx);
    const pay = await callTool(paymentTools, 'create_payment', {
      orderId: order.order.id, amount: 75.00,
    }, ctx);
    await callTool(paymentTools, 'complete_payment', { paymentId: pay.payment.id }, ctx);

    // Partial refund for 1 item
    const refund = await callTool(paymentTools, 'create_refund', {
      paymentId: pay.payment.id,
      amount: 25.00,
      reason: 'Customer returned 1 of 3 shirts',
    }, ctx);
    assert.equal(refund.success, true);
    assert.equal(parseFloat(refund.refund.amount), 25);
  });

  it('should list returns after creation', async () => {
    const cust = await callTool(customerTools, 'create_customer', {
      email: 'faye@example.com', firstName: 'Faye', lastName: 'Wong',
    }, ctx);
    const order = await callTool(orderTools, 'create_order', {
      customerId: cust.customer.id,
      items: [{ sku: 'HAT-001', name: 'Hat', quantity: 1, unitPrice: 15 }],
    }, ctx);
    const orderItemId = commerce._stores.orders.get(order.order.id).items[0].id;

    await callTool(returnTools, 'create_return', {
      orderId: order.order.id,
      reason: 'damaged',
      items: [{ orderItemId, quantity: 1 }],
    }, ctx);

    const listResult = await callTool(returnTools, 'list_returns', {}, ctx);
    assert.equal(listResult.success, true);
    assert.equal(listResult.totalCount, 1);
    assert.equal(listResult.returns[0].reason, 'damaged');
  });
});

// ============================================================================
// WORKFLOW 3: Cart-to-Checkout
// ============================================================================

describe('Workflow: Cart-to-Checkout', () => {
  let commerce;
  let ctx;

  beforeEach(() => {
    commerce = createStatefulCommerce();
    ctx = makeCtx(commerce);
  });

  it('should complete a full cart-to-checkout flow', async () => {
    // Step 1: Create customer
    const cust = await callTool(customerTools, 'create_customer', {
      email: 'grace@example.com', firstName: 'Grace', lastName: 'Hopper',
    }, ctx);

    // Step 2: Create cart
    const cartResult = await callTool(cartTools, 'create_cart', {
      customerId: cust.customer.id,
      customerEmail: 'grace@example.com',
    }, ctx);
    assert.equal(cartResult.success, true);
    const cartId = cartResult.cart.id;

    // Step 3: Add multiple items
    const item1 = await callTool(cartTools, 'add_cart_item', {
      cartId,
      sku: 'LAPTOP-001',
      name: 'Laptop Pro',
      quantity: 1,
      unitPrice: 999.99,
    }, ctx);
    assert.equal(item1.success, true);

    const item2 = await callTool(cartTools, 'add_cart_item', {
      cartId,
      sku: 'MOUSE-001',
      name: 'Wireless Mouse',
      quantity: 2,
      unitPrice: 29.99,
    }, ctx);
    assert.equal(item2.success, true);

    // Step 4: Set shipping address
    const addrResult = await callTool(cartTools, 'set_cart_shipping_address', {
      cartId,
      firstName: 'Grace',
      lastName: 'Hopper',
      line1: '123 Computing Ave',
      city: 'New York',
      state: 'NY',
      postalCode: '10001',
      country: 'US',
    }, ctx);
    assert.equal(addrResult.success, true);

    // Step 5: Apply discount
    const discResult = await callTool(cartTools, 'apply_cart_discount', {
      cartId,
      couponCode: 'SAVE10',
    }, ctx);
    assert.equal(discResult.success, true);
    assert.ok(discResult.cart.discountAmount > 0);

    // Step 6: Complete checkout
    const checkoutResult = await callTool(cartTools, 'complete_checkout', { cartId }, ctx);
    assert.equal(checkoutResult.success, true);
    assert.ok(checkoutResult.result.orderId);
    assert.ok(checkoutResult.result.orderNumber);

    // Verify: cart status should be converted
    const finalCart = await commerce.carts.get(cartId);
    assert.equal(finalCart.status, 'converted');

    // Verify: an order should exist
    const order = await commerce.orders.get(checkoutResult.result.orderId);
    assert.ok(order);
    assert.equal(order.items.length, 2);
    assert.equal(order.customerId, cust.customer.id);
  });

  it('should calculate correct totals with items and discount', async () => {
    const cart = await callTool(cartTools, 'create_cart', { customerEmail: 'test@example.com' }, ctx);
    const cartId = cart.cart.id;

    await callTool(cartTools, 'add_cart_item', {
      cartId, sku: 'A', name: 'Item A', quantity: 3, unitPrice: 10,
    }, ctx);
    await callTool(cartTools, 'add_cart_item', {
      cartId, sku: 'B', name: 'Item B', quantity: 1, unitPrice: 50,
    }, ctx);

    // Before discount: subtotal should be 80
    let cartState = await commerce.carts.get(cartId);
    assert.equal(cartState.subtotal, 80);
    assert.equal(cartState.itemCount, 2);

    // Apply discount (10% off)
    await callTool(cartTools, 'apply_cart_discount', { cartId, couponCode: 'DISCOUNT' }, ctx);
    cartState = await commerce.carts.get(cartId);
    assert.equal(cartState.discountAmount, 8); // 10% of 80
  });

  it('should get shipping rates for a cart', async () => {
    const cart = await callTool(cartTools, 'create_cart', { customerEmail: 'rates@test.com' }, ctx);
    const ratesResult = await callTool(cartTools, 'get_shipping_rates', {
      cartId: cart.cart.id,
    }, ctx);
    assert.equal(ratesResult.success, true);
    assert.ok(ratesResult.rates.length >= 2);
    assert.ok(ratesResult.rates[0].carrier);
    assert.ok(ratesResult.rates[0].price > 0);
  });

  it('should set payment method on cart', async () => {
    const cart = await callTool(cartTools, 'create_cart', { customerEmail: 'pay@test.com' }, ctx);
    const payResult = await callTool(cartTools, 'set_cart_payment', {
      cartId: cart.cart.id,
      paymentMethod: 'credit_card',
    }, ctx);
    assert.equal(payResult.success, true);
    assert.equal(payResult.cart.paymentMethod, 'credit_card');
  });

  it('should cancel a cart', async () => {
    const cart = await callTool(cartTools, 'create_cart', { customerEmail: 'cancel@test.com' }, ctx);
    const cancelResult = await callTool(cartTools, 'cancel_cart', { cartId: cart.cart.id }, ctx);
    assert.equal(cancelResult.success, true);
    assert.equal(cancelResult.cart.status, 'cancelled');
  });
});

// ============================================================================
// WORKFLOW 4: Subscription Lifecycle
// ============================================================================

describe('Workflow: Subscription Lifecycle', () => {
  let commerce;
  let ctx;

  beforeEach(() => {
    commerce = createStatefulCommerce();
    ctx = makeCtx(commerce);
  });

  it('should complete the full subscription lifecycle', async () => {
    // Step 1: Create plan
    const planResult = await callTool(subscriptionTools, 'create_subscription_plan', {
      name: 'Coffee Club Monthly',
      billingInterval: 'monthly',
      price: 29.99,
      trialDays: 14,
    }, ctx);
    assert.equal(planResult.success, true);
    const planId = planResult.plan.id;
    assert.equal(planResult.plan.status, 'draft');

    // Step 2: Activate plan
    const activateResult = await callTool(subscriptionTools, 'activate_subscription_plan', {
      planId,
    }, ctx);
    assert.equal(activateResult.success, true);
    assert.equal(activateResult.plan.status, 'active');

    // Step 3: Create customer
    const cust = await callTool(customerTools, 'create_customer', {
      email: 'subscriber@example.com', firstName: 'Sub', lastName: 'User',
    }, ctx);

    // Step 4: Subscribe customer
    const subResult = await callTool(subscriptionTools, 'create_subscription', {
      customerId: cust.customer.id,
      planId,
    }, ctx);
    assert.equal(subResult.success, true);
    const subId = subResult.subscription.id;
    // Should start in trial since plan has trialDays
    assert.equal(subResult.subscription.status, 'trial');

    // Step 5: Verify subscription exists
    const getResult = await callTool(subscriptionTools, 'get_subscription', {
      subscriptionId: subId,
    }, ctx);
    assert.ok(getResult.id);
    assert.equal(getResult.status, 'trial');

    // Step 6: Pause subscription
    const pauseResult = await callTool(subscriptionTools, 'pause_subscription', {
      subscriptionId: subId,
      reason: 'Going on vacation',
    }, ctx);
    assert.equal(pauseResult.success, true);
    assert.equal(pauseResult.subscription.status, 'paused');

    // Step 7: Resume subscription
    const resumeResult = await callTool(subscriptionTools, 'resume_subscription', {
      subscriptionId: subId,
    }, ctx);
    assert.equal(resumeResult.success, true);
    assert.equal(resumeResult.subscription.status, 'active');

    // Step 8: Cancel subscription
    const cancelResult = await callTool(subscriptionTools, 'cancel_subscription', {
      subscriptionId: subId,
      immediate: true,
      reason: 'No longer needed',
    }, ctx);
    assert.equal(cancelResult.success, true);
    assert.equal(cancelResult.subscription.status, 'cancelled');

    // Verify terminal state
    const finalSub = await commerce.getSubscription(subId);
    assert.equal(finalSub.status, 'cancelled');

    // Verify events were recorded
    const events = await commerce.getSubscriptionEvents(subId);
    assert.ok(events.length >= 3); // created, paused, resumed, cancelled
  });

  it('should list subscription plans by status', async () => {
    await callTool(subscriptionTools, 'create_subscription_plan', {
      name: 'Plan A', billingInterval: 'monthly', price: 10,
    }, ctx);
    const planB = await callTool(subscriptionTools, 'create_subscription_plan', {
      name: 'Plan B', billingInterval: 'annual', price: 100,
    }, ctx);
    await callTool(subscriptionTools, 'activate_subscription_plan', {
      planId: planB.plan.id,
    }, ctx);

    const allPlans = await callTool(subscriptionTools, 'list_subscription_plans', {}, ctx);
    assert.equal(allPlans.count, 2);
  });

  it('should skip trial when skipTrial is set', async () => {
    const plan = await callTool(subscriptionTools, 'create_subscription_plan', {
      name: 'Trial Plan', billingInterval: 'monthly', price: 19.99, trialDays: 7,
    }, ctx);
    await callTool(subscriptionTools, 'activate_subscription_plan', { planId: plan.plan.id }, ctx);

    const cust = await callTool(customerTools, 'create_customer', {
      email: 'notrial@example.com', firstName: 'No', lastName: 'Trial',
    }, ctx);

    const sub = await callTool(subscriptionTools, 'create_subscription', {
      customerId: cust.customer.id,
      planId: plan.plan.id,
      skipTrial: true,
    }, ctx);
    assert.equal(sub.subscription.status, 'active');
  });

  it('should list subscriptions filtered by customer', async () => {
    const plan = await callTool(subscriptionTools, 'create_subscription_plan', {
      name: 'Filter Plan', billingInterval: 'monthly', price: 5,
    }, ctx);
    await callTool(subscriptionTools, 'activate_subscription_plan', { planId: plan.plan.id }, ctx);

    const cust1 = await callTool(customerTools, 'create_customer', {
      email: 'c1@test.com', firstName: 'C', lastName: 'One',
    }, ctx);
    const cust2 = await callTool(customerTools, 'create_customer', {
      email: 'c2@test.com', firstName: 'C', lastName: 'Two',
    }, ctx);

    await callTool(subscriptionTools, 'create_subscription', {
      customerId: cust1.customer.id, planId: plan.plan.id, skipTrial: true,
    }, ctx);
    await callTool(subscriptionTools, 'create_subscription', {
      customerId: cust2.customer.id, planId: plan.plan.id, skipTrial: true,
    }, ctx);

    const c1Subs = await callTool(subscriptionTools, 'list_subscriptions', {
      customerId: cust1.customer.id,
    }, ctx);
    assert.equal(c1Subs.count, 1);
    assert.equal(c1Subs.subscriptions[0].customerId, cust1.customer.id);
  });
});

// ============================================================================
// WORKFLOW 5: Inventory Management
// ============================================================================

describe('Workflow: Inventory Management', () => {
  let commerce;
  let ctx;

  beforeEach(() => {
    commerce = createStatefulCommerce();
    ctx = makeCtx(commerce);
  });

  it('should create multiple SKUs and manage stock', async () => {
    // Create three inventory items
    await callTool(inventoryTools, 'create_inventory_item', {
      sku: 'SKU-A', name: 'Item A', initialQuantity: 100,
    }, ctx);
    await callTool(inventoryTools, 'create_inventory_item', {
      sku: 'SKU-B', name: 'Item B', initialQuantity: 50,
    }, ctx);
    await callTool(inventoryTools, 'create_inventory_item', {
      sku: 'SKU-C', name: 'Item C', initialQuantity: 0,
    }, ctx);

    // Verify initial state
    let stockA = await callTool(inventoryTools, 'get_stock', { sku: 'SKU-A' }, ctx);
    assert.equal(stockA.stock.totalOnHand, 100);
    assert.equal(stockA.stock.totalAvailable, 100);

    // Adjust stock
    await callTool(inventoryTools, 'adjust_inventory', {
      sku: 'SKU-A', quantity: -20, reason: 'Damaged goods',
    }, ctx);
    await callTool(inventoryTools, 'adjust_inventory', {
      sku: 'SKU-C', quantity: 75, reason: 'Received shipment',
    }, ctx);

    stockA = await callTool(inventoryTools, 'get_stock', { sku: 'SKU-A' }, ctx);
    assert.equal(stockA.stock.totalOnHand, 80);

    const stockC = await callTool(inventoryTools, 'get_stock', { sku: 'SKU-C' }, ctx);
    assert.equal(stockC.stock.totalOnHand, 75);
  });

  it('should reserve and confirm inventory', async () => {
    await callTool(inventoryTools, 'create_inventory_item', {
      sku: 'RESERVE-SKU', name: 'Reserved Item', initialQuantity: 30,
    }, ctx);

    // Reserve 10 units
    const resResult = await callTool(inventoryTools, 'reserve_inventory', {
      sku: 'RESERVE-SKU',
      quantity: 10,
      referenceType: 'order',
      referenceId: 'ord-123',
    }, ctx);
    assert.equal(resResult.success, true);
    assert.equal(resResult.reservation.status, 'reserved');

    // Check stock: 30 on hand, 10 allocated, 20 available
    let stock = await callTool(inventoryTools, 'get_stock', { sku: 'RESERVE-SKU' }, ctx);
    assert.equal(stock.stock.totalOnHand, 30);
    assert.equal(stock.stock.totalAllocated, 10);
    assert.equal(stock.stock.totalAvailable, 20);

    // Confirm reservation: deducts from on-hand
    await callTool(inventoryTools, 'confirm_reservation', {
      reservationId: resResult.reservation.id,
    }, ctx);

    stock = await callTool(inventoryTools, 'get_stock', { sku: 'RESERVE-SKU' }, ctx);
    assert.equal(stock.stock.totalOnHand, 20);
    assert.equal(stock.stock.totalAllocated, 0);
    assert.equal(stock.stock.totalAvailable, 20);
  });

  it('should fail to reserve more than available', async () => {
    await callTool(inventoryTools, 'create_inventory_item', {
      sku: 'LIMITED', name: 'Limited Item', initialQuantity: 5,
    }, ctx);

    await assert.rejects(
      callTool(inventoryTools, 'reserve_inventory', {
        sku: 'LIMITED',
        quantity: 10,
        referenceType: 'order',
        referenceId: 'ord-456',
      }, ctx),
      /Insufficient stock/,
    );
  });

  it('should return error for non-existent SKU', async () => {
    const result = await callTool(inventoryTools, 'get_stock', { sku: 'NONEXISTENT' }, ctx);
    assert.equal(result.success, false);
    assert.ok(result.error.includes('NONEXISTENT'));
  });

  it('should handle concurrent reservations on same SKU', async () => {
    await callTool(inventoryTools, 'create_inventory_item', {
      sku: 'SHARED', name: 'Shared Item', initialQuantity: 20,
    }, ctx);

    const r1 = await callTool(inventoryTools, 'reserve_inventory', {
      sku: 'SHARED', quantity: 8, referenceType: 'order', referenceId: 'ord-1',
    }, ctx);
    const r2 = await callTool(inventoryTools, 'reserve_inventory', {
      sku: 'SHARED', quantity: 7, referenceType: 'order', referenceId: 'ord-2',
    }, ctx);

    assert.equal(r1.success, true);
    assert.equal(r2.success, true);

    const stock = await callTool(inventoryTools, 'get_stock', { sku: 'SHARED' }, ctx);
    assert.equal(stock.stock.totalAllocated, 15);
    assert.equal(stock.stock.totalAvailable, 5);
  });
});

// ============================================================================
// WORKFLOW 6: Multi-Currency Order
// ============================================================================

describe('Workflow: Multi-Currency Order', () => {
  let commerce;
  let ctx;

  beforeEach(() => {
    commerce = createStatefulCommerce();
    ctx = makeCtx(commerce);
  });

  it('should set exchange rate and convert currency', async () => {
    // Set USD -> EUR rate
    const rateResult = await callTool(currencyTools, 'set_exchange_rate', {
      baseCurrency: 'USD',
      quoteCurrency: 'EUR',
      rate: 0.92,
    }, ctx);
    assert.equal(rateResult.success, true);
    assert.equal(rateResult.rate.rate, 0.92);

    // Get the rate
    const getResult = await callTool(currencyTools, 'get_exchange_rate', {
      from: 'USD',
      to: 'EUR',
    }, ctx);
    assert.equal(getResult.success, true);
    assert.equal(getResult.rate.rate, 0.92);

    // Convert $100 USD to EUR
    const convertResult = await callTool(currencyTools, 'convert_currency', {
      from: 'USD',
      to: 'EUR',
      amount: 100,
    }, ctx);
    assert.equal(convertResult.success, true);
    assert.equal(convertResult.conversion.convertedAmount, 92);
    assert.equal(convertResult.conversion.originalAmount, 100);
    assert.equal(convertResult.conversion.originalCurrency, 'USD');
    assert.equal(convertResult.conversion.targetCurrency, 'EUR');
  });

  it('should create product and verify multi-currency total', async () => {
    // Set rate
    await callTool(currencyTools, 'set_exchange_rate', {
      baseCurrency: 'USD',
      quoteCurrency: 'EUR',
      rate: 0.92,
    }, ctx);

    // Create product at $49.99
    await callTool(productTools, 'create_product', {
      name: 'Euro Widget',
      sku: 'EUR-WIDGET',
      price: 49.99,
    }, ctx);

    // Create customer
    const cust = await callTool(customerTools, 'create_customer', {
      email: 'euro@example.com', firstName: 'Euro', lastName: 'Customer',
    }, ctx);

    // Create order in EUR by converting the price
    const convertResult = await callTool(currencyTools, 'convert_currency', {
      from: 'USD', to: 'EUR', amount: 49.99,
    }, ctx);
    const eurPrice = convertResult.conversion.convertedAmount;

    const order = await callTool(orderTools, 'create_order', {
      customerId: cust.customer.id,
      items: [{ sku: 'EUR-WIDGET', name: 'Euro Widget', quantity: 1, unitPrice: eurPrice }],
      currency: 'EUR',
    }, ctx);
    assert.equal(order.success, true);
    assert.equal(order.order.totalAmount, eurPrice);
  });

  it('should list exchange rates', async () => {
    await callTool(currencyTools, 'set_exchange_rate', {
      baseCurrency: 'USD', quoteCurrency: 'EUR', rate: 0.92,
    }, ctx);
    await callTool(currencyTools, 'set_exchange_rate', {
      baseCurrency: 'USD', quoteCurrency: 'GBP', rate: 0.79,
    }, ctx);

    const listResult = await callTool(currencyTools, 'list_exchange_rates', {}, ctx);
    assert.equal(listResult.success, true);
    assert.equal(listResult.count, 2);
  });

  it('should get currency settings', async () => {
    const settingsResult = await callTool(currencyTools, 'get_currency_settings', {}, ctx);
    assert.equal(settingsResult.success, true);
    assert.equal(settingsResult.settings.baseCurrency, 'USD');
  });
});

// ============================================================================
// WORKFLOW 7: Gift Card Workflow
// ============================================================================

describe('Workflow: Gift Card', () => {
  let commerce;
  let ctx;

  beforeEach(() => {
    commerce = createStatefulCommerce();
    ctx = makeCtx(commerce);
  });

  it('should create, charge, and check balance on a gift card', async () => {
    // Step 1: Create gift card with $50 balance
    const createResult = await callTool(giftCardTools, 'create_gift_card', {
      initialBalance: 50,
      recipientEmail: 'friend@example.com',
    }, ctx);
    assert.equal(createResult.success, true);
    assert.ok(createResult.giftCard.code);
    const gcId = createResult.giftCard.id;

    // Step 2: Check balance
    const getResult = await callTool(giftCardTools, 'get_gift_card', {
      identifier: gcId,
    }, ctx);
    assert.equal(getResult.success, true);
    assert.equal(getResult.giftCard.currentBalance, '50');
    assert.equal(getResult.giftCard.status, 'active');

    // Step 3: Charge $30 to the gift card (apply to order)
    const chargeResult = await callTool(giftCardTools, 'charge_gift_card', {
      giftCardId: gcId,
      amount: 30,
      orderId: 'order-gc-001',
    }, ctx);
    assert.equal(chargeResult.success, true);
    assert.equal(chargeResult.transaction.balanceAfter, '20');

    // Step 4: Verify reduced balance
    const checkResult = await callTool(giftCardTools, 'get_gift_card', {
      identifier: gcId,
    }, ctx);
    assert.equal(checkResult.giftCard.currentBalance, '20');
  });

  it('should refund to a gift card', async () => {
    const gc = await callTool(giftCardTools, 'create_gift_card', {
      initialBalance: 100,
    }, ctx);
    const gcId = gc.giftCard.id;

    // Charge $60
    await callTool(giftCardTools, 'charge_gift_card', {
      giftCardId: gcId, amount: 60,
    }, ctx);

    // Refund $20
    const refundResult = await callTool(giftCardTools, 'refund_to_gift_card', {
      giftCardId: gcId, amount: 20, reason: 'Partial return',
    }, ctx);
    assert.equal(refundResult.success, true);
    assert.equal(refundResult.transaction.balanceAfter, '60'); // 100 - 60 + 20

    const check = await callTool(giftCardTools, 'get_gift_card', { identifier: gcId }, ctx);
    assert.equal(check.giftCard.currentBalance, '60');
  });

  it('should fail to charge more than the balance', async () => {
    const gc = await callTool(giftCardTools, 'create_gift_card', {
      initialBalance: 10,
    }, ctx);

    await assert.rejects(
      callTool(giftCardTools, 'charge_gift_card', {
        giftCardId: gc.giftCard.id, amount: 50,
      }, ctx),
      /Insufficient gift card balance/,
    );
  });

  it('should list gift cards', async () => {
    await callTool(giftCardTools, 'create_gift_card', { initialBalance: 25 }, ctx);
    await callTool(giftCardTools, 'create_gift_card', { initialBalance: 75 }, ctx);

    const listResult = await callTool(giftCardTools, 'list_gift_cards', {}, ctx);
    assert.equal(listResult.success, true);
    assert.equal(listResult.totalCount, 2);
  });
});

// ============================================================================
// WORKFLOW 8: Loyalty Program
// ============================================================================

describe('Workflow: Loyalty Program', () => {
  let commerce;
  let ctx;

  beforeEach(() => {
    commerce = createStatefulCommerce();
    ctx = makeCtx(commerce);
  });

  it('should complete the full loyalty program lifecycle', async () => {
    // Step 1: Create loyalty program
    const progResult = await callTool(loyaltyTools, 'create_loyalty_program', {
      name: 'Rewards Club',
      pointsPerDollar: 2,
      tiers: [
        { name: 'Bronze', minPoints: 0, multiplier: 1 },
        { name: 'Silver', minPoints: 500, multiplier: 1.5 },
        { name: 'Gold', minPoints: 2000, multiplier: 2 },
      ],
    }, ctx);
    assert.equal(progResult.success, true);
    const programId = progResult.program.id;

    // Step 2: Create customer
    const cust = await callTool(customerTools, 'create_customer', {
      email: 'loyal@example.com', firstName: 'Loyal', lastName: 'Customer',
    }, ctx);
    const customerId = cust.customer.id;

    // Step 3: Enroll customer
    const enrollResult = await callTool(loyaltyTools, 'enroll_customer', {
      programId,
      customerId,
    }, ctx);
    assert.equal(enrollResult.success, true);
    assert.equal(enrollResult.account.pointsBalance, 0);

    // Step 4: Earn points from a purchase
    const earnResult = await callTool(loyaltyTools, 'earn_points', {
      programId,
      customerId,
      points: 200,
      reason: 'purchase',
      orderId: 'order-loyalty-001',
    }, ctx);
    assert.equal(earnResult.success, true);
    assert.ok(earnResult.message.includes('200'));

    // Step 5: Check balance
    const accountResult = await callTool(loyaltyTools, 'get_loyalty_account', {
      programId,
      customerId,
    }, ctx);
    assert.equal(accountResult.success, true);
    assert.equal(accountResult.account.pointsBalance, 200);
    assert.equal(accountResult.account.lifetimePoints, 200);

    // Step 6: Earn more points
    await callTool(loyaltyTools, 'earn_points', {
      programId, customerId, points: 150, reason: 'referral',
    }, ctx);

    // Step 7: Redeem points
    const redeemResult = await callTool(loyaltyTools, 'redeem_points', {
      programId,
      customerId,
      points: 100,
      orderId: 'order-loyalty-002',
    }, ctx);
    assert.equal(redeemResult.success, true);
    assert.ok(redeemResult.message.includes('100'));

    // Step 8: Verify final balance: 200 + 150 - 100 = 250
    const finalAccount = await callTool(loyaltyTools, 'get_loyalty_account', {
      programId,
      customerId,
    }, ctx);
    assert.equal(finalAccount.account.pointsBalance, 250);
    assert.equal(finalAccount.account.lifetimePoints, 350);
  });

  it('should fail to redeem more points than balance', async () => {
    const prog = await callTool(loyaltyTools, 'create_loyalty_program', {
      name: 'Test Program', pointsPerDollar: 1,
    }, ctx);
    const cust = await callTool(customerTools, 'create_customer', {
      email: 'poor@example.com', firstName: 'Poor', lastName: 'Points',
    }, ctx);
    await callTool(loyaltyTools, 'enroll_customer', {
      programId: prog.program.id, customerId: cust.customer.id,
    }, ctx);
    await callTool(loyaltyTools, 'earn_points', {
      programId: prog.program.id, customerId: cust.customer.id, points: 50,
    }, ctx);

    await assert.rejects(
      callTool(loyaltyTools, 'redeem_points', {
        programId: prog.program.id, customerId: cust.customer.id, points: 100,
      }, ctx),
      /Insufficient loyalty points/,
    );
  });

  it('should get loyalty program details', async () => {
    const prog = await callTool(loyaltyTools, 'create_loyalty_program', {
      name: 'VIP Rewards',
      pointsPerDollar: 3,
      tiers: [{ name: 'Member', minPoints: 0, multiplier: 1 }],
    }, ctx);

    const getResult = await callTool(loyaltyTools, 'get_loyalty_program', {
      programId: prog.program.id,
    }, ctx);
    assert.equal(getResult.success, true);
    assert.equal(getResult.program.name, 'VIP Rewards');
    assert.equal(getResult.program.pointsPerDollar, 3);
    assert.equal(getResult.program.tiers.length, 1);
  });

  it('should increment totalMembers on enrollment', async () => {
    const prog = await callTool(loyaltyTools, 'create_loyalty_program', {
      name: 'Count Program', pointsPerDollar: 1,
    }, ctx);
    const c1 = await callTool(customerTools, 'create_customer', {
      email: 'a@x.com', firstName: 'A', lastName: 'A',
    }, ctx);
    const c2 = await callTool(customerTools, 'create_customer', {
      email: 'b@x.com', firstName: 'B', lastName: 'B',
    }, ctx);

    await callTool(loyaltyTools, 'enroll_customer', {
      programId: prog.program.id, customerId: c1.customer.id,
    }, ctx);
    await callTool(loyaltyTools, 'enroll_customer', {
      programId: prog.program.id, customerId: c2.customer.id,
    }, ctx);

    const details = await callTool(loyaltyTools, 'get_loyalty_program', {
      programId: prog.program.id,
    }, ctx);
    assert.equal(details.program.totalMembers, 2);
  });
});

// ============================================================================
// CROSS-WORKFLOW INTEGRATION TESTS
// ============================================================================

describe('Cross-Workflow Integration', () => {
  let commerce;
  let ctx;

  beforeEach(() => {
    commerce = createStatefulCommerce();
    ctx = makeCtx(commerce);
  });

  it('should handle order + return + gift card refund flow', async () => {
    // Create customer
    const cust = await callTool(customerTools, 'create_customer', {
      email: 'cross@example.com', firstName: 'Cross', lastName: 'Flow',
    }, ctx);

    // Create order
    const order = await callTool(orderTools, 'create_order', {
      customerId: cust.customer.id,
      items: [{ sku: 'CROSS-001', name: 'Cross Item', quantity: 1, unitPrice: 50 }],
    }, ctx);
    const orderId = order.order.id;

    // Pay for order
    const pay = await callTool(paymentTools, 'create_payment', {
      orderId, amount: 50,
    }, ctx);
    await callTool(paymentTools, 'complete_payment', { paymentId: pay.payment.id }, ctx);

    // Ship and deliver
    const ship = await callTool(shipmentTools, 'create_shipment', { orderId }, ctx);
    await callTool(shipmentTools, 'deliver_shipment', { shipmentId: ship.shipment.id }, ctx);

    // Create return
    const orderItemId = commerce._stores.orders.get(orderId).items[0].id;
    const ret = await callTool(returnTools, 'create_return', {
      orderId, reason: 'defective', items: [{ orderItemId, quantity: 1 }],
    }, ctx);
    await callTool(returnTools, 'approve_return', { returnId: ret.return.id }, ctx);

    // Issue refund as gift card credit instead of cash
    const gc = await callTool(giftCardTools, 'create_gift_card', {
      initialBalance: 50,
      customerId: cust.customer.id,
    }, ctx);
    assert.equal(gc.giftCard.currentBalance, '50');
    assert.ok(gc.giftCard.id);
  });

  it('should handle cart checkout + loyalty points earning', async () => {
    // Create loyalty program
    const prog = await callTool(loyaltyTools, 'create_loyalty_program', {
      name: 'Shop Rewards', pointsPerDollar: 1,
    }, ctx);

    // Create customer and enroll
    const cust = await callTool(customerTools, 'create_customer', {
      email: 'shopper@example.com', firstName: 'Happy', lastName: 'Shopper',
    }, ctx);
    await callTool(loyaltyTools, 'enroll_customer', {
      programId: prog.program.id, customerId: cust.customer.id,
    }, ctx);

    // Create cart, add items, checkout
    const cart = await callTool(cartTools, 'create_cart', {
      customerId: cust.customer.id,
    }, ctx);
    await callTool(cartTools, 'add_cart_item', {
      cartId: cart.cart.id, sku: 'REWARD-001', name: 'Reward Item',
      quantity: 1, unitPrice: 75,
    }, ctx);
    const checkout = await callTool(cartTools, 'complete_checkout', {
      cartId: cart.cart.id,
    }, ctx);
    assert.equal(checkout.success, true);

    // Award loyalty points for the purchase
    await callTool(loyaltyTools, 'earn_points', {
      programId: prog.program.id,
      customerId: cust.customer.id,
      points: 75, // 1 point per dollar
      reason: 'purchase',
      orderId: checkout.result.orderId,
    }, ctx);

    const account = await callTool(loyaltyTools, 'get_loyalty_account', {
      programId: prog.program.id, customerId: cust.customer.id,
    }, ctx);
    assert.equal(account.account.pointsBalance, 75);
  });

  it('should handle multi-currency cart checkout', async () => {
    // Set exchange rate
    await callTool(currencyTools, 'set_exchange_rate', {
      baseCurrency: 'USD', quoteCurrency: 'GBP', rate: 0.79,
    }, ctx);

    const cust = await callTool(customerTools, 'create_customer', {
      email: 'uk@example.com', firstName: 'British', lastName: 'Buyer',
    }, ctx);

    // Create cart in GBP
    const cart = await callTool(cartTools, 'create_cart', {
      customerId: cust.customer.id,
      currency: 'GBP',
    }, ctx);

    // Convert price and add to cart
    const converted = await callTool(currencyTools, 'convert_currency', {
      from: 'USD', to: 'GBP', amount: 100,
    }, ctx);
    await callTool(cartTools, 'add_cart_item', {
      cartId: cart.cart.id,
      sku: 'UK-001',
      name: 'British Widget',
      quantity: 1,
      unitPrice: converted.conversion.convertedAmount,
    }, ctx);

    const checkout = await callTool(cartTools, 'complete_checkout', {
      cartId: cart.cart.id,
    }, ctx);
    assert.equal(checkout.success, true);
    assert.equal(checkout.result.currency, 'GBP');
  });
});

// ============================================================================
// EDGE-CASE AND VALIDATION TESTS
// ============================================================================

describe('Edge Cases and Validation', () => {
  let commerce;
  let ctx;

  beforeEach(() => {
    commerce = createStatefulCommerce();
    ctx = makeCtx(commerce);
  });

  it('should list empty collections gracefully', async () => {
    const custList = await callTool(customerTools, 'list_customers', {}, ctx);
    assert.equal(custList.success, true);
    assert.equal(custList.count, 0);
    assert.deepEqual(custList.customers, []);

    const orderList = await callTool(orderTools, 'list_orders', {}, ctx);
    assert.equal(orderList.success, true);
    assert.equal(orderList.totalCount, 0);

    const payList = await callTool(paymentTools, 'list_payments', {}, ctx);
    assert.equal(payList.success, true);
    assert.equal(payList.count, 0);

    const shipList = await callTool(shipmentTools, 'list_shipments', {}, ctx);
    assert.equal(shipList.success, true);
    assert.equal(shipList.count, 0);
  });

  it('should get non-existent entities gracefully', async () => {
    const custResult = await callTool(customerTools, 'get_customer', {
      identifier: 'nonexistent-id',
    }, ctx);
    assert.equal(custResult.success, false);

    const orderResult = await callTool(orderTools, 'get_order', {
      identifier: 'nonexistent-id',
    }, ctx);
    assert.equal(orderResult.success, false);

    const returnResult = await callTool(returnTools, 'get_return', {
      returnId: 'nonexistent-id',
    }, ctx);
    assert.equal(returnResult.success, false);
  });

  it('should look up customer by email', async () => {
    await callTool(customerTools, 'create_customer', {
      email: 'lookup@example.com', firstName: 'Look', lastName: 'Up',
    }, ctx);

    const result = await callTool(customerTools, 'get_customer', {
      identifier: 'lookup@example.com',
    }, ctx);
    assert.equal(result.success, true);
    assert.equal(result.customer.email, 'lookup@example.com');
    assert.equal(result.customer.firstName, 'Look');
  });

  it('should create order with multiple line items and verify totals', async () => {
    const cust = await callTool(customerTools, 'create_customer', {
      email: 'multi@example.com', firstName: 'Multi', lastName: 'Item',
    }, ctx);
    const order = await callTool(orderTools, 'create_order', {
      customerId: cust.customer.id,
      items: [
        { sku: 'A', name: 'Item A', quantity: 3, unitPrice: 10 },
        { sku: 'B', name: 'Item B', quantity: 1, unitPrice: 25.50 },
        { sku: 'C', name: 'Item C', quantity: 2, unitPrice: 7.99 },
      ],
    }, ctx);
    assert.equal(order.success, true);
    // 30 + 25.50 + 15.98 = 71.48
    assert.equal(order.order.totalAmount, 71.48);
  });

  it('should block order creation without --apply', async () => {
    const noApplyCtx = makeCtx(commerce, { allowApply: false });
    const cust = await callTool(customerTools, 'create_customer', {
      email: 'blocked@example.com', firstName: 'B', lastName: 'B',
    }, ctx);
    const result = await callTool(orderTools, 'create_order', {
      customerId: cust.customer.id,
      items: [{ sku: 'X', name: 'X', quantity: 1, unitPrice: 10 }],
    }, noApplyCtx);
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldCreate);
  });

  it('should block cart operations without --apply', async () => {
    const noApplyCtx = makeCtx(commerce, { allowApply: false });
    const cartResult = await callTool(cartTools, 'create_cart', {
      customerEmail: 'nope@test.com',
    }, noApplyCtx);
    assert.equal(cartResult.success, false);
    assert.ok(cartResult.hint.includes('--apply'));
  });

  it('should handle inventory adjust negative and positive', async () => {
    await callTool(inventoryTools, 'create_inventory_item', {
      sku: 'ADJ-SKU', name: 'Adjustable', initialQuantity: 50,
    }, ctx);

    // Add 20
    let result = await callTool(inventoryTools, 'adjust_inventory', {
      sku: 'ADJ-SKU', quantity: 20, reason: 'Received',
    }, ctx);
    assert.equal(result.stock.totalOnHand, 70);

    // Remove 30
    result = await callTool(inventoryTools, 'adjust_inventory', {
      sku: 'ADJ-SKU', quantity: -30, reason: 'Sold',
    }, ctx);
    assert.equal(result.stock.totalOnHand, 40);
  });

  it('should update order status through lifecycle', async () => {
    const cust = await callTool(customerTools, 'create_customer', {
      email: 'lifecycle@test.com', firstName: 'Life', lastName: 'Cycle',
    }, ctx);
    const order = await callTool(orderTools, 'create_order', {
      customerId: cust.customer.id,
      items: [{ sku: 'LC-1', name: 'Lifecycle Item', quantity: 1, unitPrice: 10 }],
    }, ctx);
    const orderId = order.order.id;

    // pending -> confirmed -> processing
    let result = await callTool(orderTools, 'update_order_status', {
      orderId, status: 'confirmed',
    }, ctx);
    assert.equal(result.order.status, 'confirmed');

    result = await callTool(orderTools, 'update_order_status', {
      orderId, status: 'processing',
    }, ctx);
    assert.equal(result.order.status, 'processing');
  });

  it('should handle gift card full lifecycle: create, charge, refund, disable', async () => {
    const gc = await callTool(giftCardTools, 'create_gift_card', {
      initialBalance: 200,
      currency: 'USD',
      recipientEmail: 'full@gc.com',
      recipientName: 'Full Test',
      message: 'Happy testing!',
    }, ctx);
    assert.equal(gc.giftCard.currency, 'USD');

    // Charge
    await callTool(giftCardTools, 'charge_gift_card', {
      giftCardId: gc.giftCard.id, amount: 100,
    }, ctx);

    // Refund part
    await callTool(giftCardTools, 'refund_to_gift_card', {
      giftCardId: gc.giftCard.id, amount: 30,
    }, ctx);

    // Check: 200 - 100 + 30 = 130
    const check = await callTool(giftCardTools, 'get_gift_card', {
      identifier: gc.giftCard.id,
    }, ctx);
    assert.equal(check.giftCard.currentBalance, '130');

    // Disable
    const disableResult = await callTool(giftCardTools, 'disable_gift_card', {
      giftCardId: gc.giftCard.id, reason: 'Fraud detected',
    }, ctx);
    assert.equal(disableResult.success, true);
    assert.equal(disableResult.giftCard.status, 'disabled');
  });

  it('should archive a subscription plan', async () => {
    const plan = await callTool(subscriptionTools, 'create_subscription_plan', {
      name: 'Archive Me', billingInterval: 'monthly', price: 9.99,
    }, ctx);
    await callTool(subscriptionTools, 'activate_subscription_plan', {
      planId: plan.plan.id,
    }, ctx);

    const archiveResult = await callTool(subscriptionTools, 'archive_subscription_plan', {
      planId: plan.plan.id,
    }, ctx);
    assert.equal(archiveResult.success, true);
    assert.equal(archiveResult.plan.status, 'archived');
  });

  it('should skip a billing cycle', async () => {
    const plan = await callTool(subscriptionTools, 'create_subscription_plan', {
      name: 'Skip Plan', billingInterval: 'monthly', price: 15,
    }, ctx);
    await callTool(subscriptionTools, 'activate_subscription_plan', {
      planId: plan.plan.id,
    }, ctx);
    const cust = await callTool(customerTools, 'create_customer', {
      email: 'skip@test.com', firstName: 'Skip', lastName: 'User',
    }, ctx);
    const sub = await callTool(subscriptionTools, 'create_subscription', {
      customerId: cust.customer.id, planId: plan.plan.id, skipTrial: true,
    }, ctx);

    const origDate = sub.subscription.nextBillingDate;
    const skipResult = await callTool(subscriptionTools, 'skip_billing_cycle', {
      subscriptionId: sub.subscription.id, reason: 'Vacation',
    }, ctx);
    assert.equal(skipResult.success, true);
    // Next billing date should have moved forward
    assert.ok(new Date(skipResult.nextBillingDate) > new Date(origDate));
  });
});

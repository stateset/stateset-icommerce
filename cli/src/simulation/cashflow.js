import {
  addDecimals,
  subtractDecimals,
  multiplyDecimals,
  compareDecimals,
  formatDecimal,
  clampAtZero,
} from '../utils/exact-decimal.js';
import { SIMULATION_LIMITS as limits } from './limits.js';

// Replay the native-backed timeline independently of the running cash balance.
// Opening cash and every sale/purchase balance participate, including day zero.
export function reconcileCashFlow({
  startingCash,
  cashReserve = '0.00',
  days,
  products,
  timeline,
  summary,
}) {
  const fail = () => {
    throw new Error('Simulation cash-flow reconciliation failed');
  };
  if (
    !Number.isInteger(days) ||
    days < 1 ||
    days > limits.maxPlanningDays ||
    compareDecimals(startingCash, '0') < 0 ||
    compareDecimals(cashReserve, '0') < 0
  )
    fail();
  const bySku = new Map(products.map((product) => [product.sku, product]));
  let previousDay = 0;
  for (const event of timeline) {
    if (!Number.isInteger(event.day) || event.day < previousDay || event.day > days) fail();
    previousDay = event.day;
  }
  let cash = formatDecimal(startingCash);
  let minimumCash = cash;
  let minimumCashAt = { day: 0, type: 'opening' };
  let totalSales = '0.00';
  let totalPurchases = '0.00';
  let reserveBlockedPurchases = 0;
  let index = 0;
  const daily = [];
  for (let day = 0; day <= days; day += 1) {
    const openingCash = cash;
    let dailyMinimum = cash;
    let salesReceipts = '0.00';
    let purchasingSpend = '0.00';
    while (index < timeline.length && timeline[index].day === day) {
      const event = timeline[index++];
      if (event.type === 'demand' || event.type === 'purchase_order_sent') {
        const product = bySku.get(event.sku);
        const sale = event.type === 'demand';
        const quantity = sale ? event.sold : event.quantity;
        if (
          !product ||
          !Number.isSafeInteger(quantity) ||
          quantity < (sale ? 0 : 1) ||
          (sale && day === 0)
        )
          fail();
        const amount = multiplyDecimals(product[sale ? 'unitPrice' : 'unitCost'], String(quantity));
        if (sale) {
          cash = addDecimals(cash, amount);
          salesReceipts = addDecimals(salesReceipts, amount);
        } else {
          if (compareDecimals(event.amount, amount) !== 0) fail();
          cash = subtractDecimals(cash, amount);
          if (compareDecimals(amount, '0') > 0 && compareDecimals(cash, cashReserve) < 0) fail();
          purchasingSpend = addDecimals(purchasingSpend, amount);
        }
      } else if (event.type === 'purchase_reserve_blocked') {
        const product = bySku.get(event.sku);
        if (!product || !Number.isSafeInteger(event.quantity) || event.quantity <= 0) fail();
        const required = multiplyDecimals(product.unitCost, String(event.quantity));
        const spendable = clampAtZero(subtractDecimals(cash, cashReserve));
        if (
          compareDecimals(event.required, required) !== 0 ||
          compareDecimals(event.cashReserve, cashReserve) !== 0 ||
          compareDecimals(event.spendableCash, spendable) !== 0 ||
          compareDecimals(required, cash) > 0 ||
          compareDecimals(required, spendable) <= 0
        )
          fail();
        reserveBlockedPurchases += 1;
      } else if (
        ![
          'purchase_budget_blocked',
          'purchase_constraint_blocked',
          'purchase_received',
          'purchase_delivery_missed',
          'purchase_review_deferred',
        ].includes(event.type)
      )
        fail();
      if (compareDecimals(cash, '0') < 0) fail();
      if (
        [
          'demand',
          'purchase_order_sent',
          'purchase_budget_blocked',
          'purchase_constraint_blocked',
          'purchase_reserve_blocked',
          'purchase_review_deferred',
        ].includes(event.type) &&
        compareDecimals(event.cash, cash) !== 0
      )
        fail();
      if (compareDecimals(cash, dailyMinimum) < 0) dailyMinimum = cash;
      if (compareDecimals(cash, minimumCash) < 0) {
        minimumCash = cash;
        minimumCashAt = {
          day,
          type: event.type,
          sku: event.sku,
          ...(event.purchase ? { purchase: event.purchase } : {}),
        };
      }
    }
    totalSales = addDecimals(totalSales, salesReceipts);
    totalPurchases = addDecimals(totalPurchases, purchasingSpend);
    daily.push({
      day,
      openingCash,
      salesReceipts,
      purchasingSpend,
      closingCash: cash,
      minimumCash: dailyMinimum,
    });
  }
  if (
    index !== timeline.length ||
    compareDecimals(cash, summary.endingCash) !== 0 ||
    compareDecimals(totalSales, summary.revenue) !== 0 ||
    compareDecimals(totalPurchases, summary.purchasingSpend) !== 0 ||
    reserveBlockedPurchases !== summary.reserveBlockedPurchases
  )
    fail();
  return { minimumCash, minimumCashAt, daily };
}

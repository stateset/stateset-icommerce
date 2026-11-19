import { merchantRecords, readMerchantRecord } from './merchant-records.js';
import { decimalToUnits, unitsToDecimal } from './money.js';

function roundedDifference(numerator, denominator) {
  const magnitude = numerator < 0n ? -numerator : numerator;
  const units = (magnitude + denominator / 2n) / denominator;
  return (numerator < 0n && units > 0n ? '-' : '') + unitsToDecimal(units, 2);
}

// Group refunds by order before rounding allocations. A series of partial
// refunds must reduce net merchandise revenue by the same amount as one refund.
export async function campaignAccounting(
  commerce,
  { bySource = false, from = '', to = '', includeOrders = false, control = '' } = {},
) {
  if (control && !/^[a-z0-9][a-z0-9_-]{0,47}$/.test(control))
    throw new Error('Choose a valid comparison variant');
  for (const date of [from, to]) {
    if (
      date &&
      (!/^\d{4}-\d{2}-\d{2}$/.test(date) ||
        !Number.isFinite(Date.parse(date)) ||
        new Date(date).toISOString().slice(0, 10) !== date)
    )
      throw new Error('Use calendar dates for the visitor cohort');
  }
  if (from && to && from > to) throw new Error('End date must follow the start date');
  const visits = new Map();
  for await (const entry of merchantRecords(commerce, 'campaign-visits')) {
    const date = entry.at.slice(0, 10);
    if ((!from || date >= from) && (!to || date <= to))
      visits.set(entry.campaignId + ':' + entry.visitor, entry);
  }
  const selected = (entry) => (!from && !to) || visits.has(entry.campaignId + ':' + entry.visitor);
  const rows = new Map();
  const ensure = (entry) => {
    const source = bySource ? entry.source : undefined;
    const revision = entry.revision ?? 0;
    const key = JSON.stringify([entry.campaignId, revision, entry.variant, source]);
    if (!rows.has(key))
      rows.set(key, {
        campaign: entry.campaignId,
        revision,
        variant: entry.variant,
        source,
        visitors: 0,
        arrivals: new Set(),
        checkoutVisitors: new Set(),
        paymentStarts: new Set(),
        buyers: new Set(),
        upsellViews: new Set(),
        skipped: new Set(),
        bumps: 0,
        upsells: 0,
        gross: 0n,
        net: 0n,
        trackedNet: 0n,
      });
    return rows.get(key);
  };
  for (const visit of visits.values()) {
    const row = ensure(visit);
    row.arrivals.add(visit.visitor);
    row.visitors++;
  }
  for await (const step of merchantRecords(commerce, 'campaign-steps')) {
    if (!selected(step)) continue;
    if (step.step === 'checkout') ensure(step).checkoutVisitors.add(step.visitor);
    if (step.step === 'payment-start') ensure(step).paymentStarts.add(step.visitor);
    if (step.step === 'upsell-view') ensure(step).upsellViews.add(step.id);
    if (step.step === 'upsell-skip') ensure(step).skipped.add(step.id);
  }
  const events = [];
  const refunds = new Map();
  const refundDetails = new Map();
  for await (const event of merchantRecords(commerce, 'merchant-events')) {
    if (event.type === 'refund') {
      refunds.set(
        event.orderId,
        (refunds.get(event.orderId) || 0n) + decimalToUnits(event.amount, 2),
      );
      if (includeOrders) {
        if (!refundDetails.has(event.orderId)) refundDetails.set(event.orderId, []);
        refundDetails
          .get(event.orderId)
          .push({ id: event.refundId, amount: event.amount, at: event.at });
      }
    } else if (event.type === 'sale') events.push(event);
  }
  const commissions = [];
  const orders = [];
  for (const sale of events) {
    const terms = await readMerchantRecord(commerce, 'campaign-order-' + sale.orderId);
    if (!terms || sale.currency !== 'USD' || !selected(terms)) continue;
    const row = ensure(terms);
    const charged = decimalToUnits(sale.total, 2);
    const merchandise = sale.items.reduce(
      (sum, item) =>
        sum +
        decimalToUnits(item.unitPrice, 2) * BigInt(item.quantity) -
        decimalToUnits(item.discount || '0', 2),
      0n,
    );
    const refunded = refunds.get(sale.orderId) || 0n;
    const remaining = charged > refunded ? charged - refunded : 0n;
    const net = charged ? (merchandise * remaining + charged / 2n) / charged : 0n;
    if (includeOrders)
      orders.push({
        orderId: sale.orderId,
        number: sale.number,
        at: sale.placedAt || sale.at,
        campaign: terms.campaignId,
        revision: terms.revision ?? 0,
        variant: terms.variant,
        source: terms.source,
        charged: sale.total,
        merchandise: unitsToDecimal(merchandise, 2),
        net: unitsToDecimal(net, 2),
        refunds: refundDetails.get(sale.orderId) || [],
      });
    row.gross += merchandise;
    row.net += net;
    if (row.arrivals.has(terms.visitor)) row.trackedNet += net;
    row.buyers.add(terms.visitor);
    if (terms.bump) row.bumps++;
    if (terms.parentOrderId) row.upsells++;
    if (terms.affiliate) {
      const original = (merchandise * BigInt(terms.commissionBps)) / 10000n;
      const earned = (net * BigInt(terms.commissionBps)) / 10000n;
      commissions.push({
        affiliate: terms.affiliate,
        orderId: sale.orderId,
        currency: 'USD',
        original: unitsToDecimal(original, 2),
        reversal: unitsToDecimal(original - earned, 2),
        earned: unitsToDecimal(earned, 2),
      });
    }
  }
  const controls = new Map();
  const groupKey = (r) => JSON.stringify([r.campaign, r.revision, r.source]);
  for (const r of rows.values()) {
    if (!r.visitors || (control && r.variant !== control)) continue;
    const previous = controls.get(groupKey(r));
    if (!previous || r.variant < previous.variant) controls.set(groupKey(r), r);
  }
  return {
    variants: [...rows.values()].map((r) => {
      let reachedCheckout = 0,
        reachedPayment = 0,
        purchased = 0;
      for (const visitor of r.arrivals) {
        // A settled purchase also proves its earlier stages on stores predating stage tracking.
        if (r.buyers.has(visitor)) purchased++;
        if (r.buyers.has(visitor) || r.paymentStarts.has(visitor)) reachedPayment++;
        if (
          r.buyers.has(visitor) ||
          r.paymentStarts.has(visitor) ||
          r.checkoutVisitors.has(visitor)
        )
          reachedCheckout++;
      }
      const baseline = controls.get(groupKey(r));
      let change = null,
        percent = null;
      if (baseline && r.visitors) {
        const visitors = BigInt(r.visitors),
          baseVisitors = BigInt(baseline.visitors);
        const difference = r.trackedNet * baseVisitors - baseline.trackedNet * visitors;
        change = roundedDifference(difference, visitors * baseVisitors);
        if (baseline.trackedNet > 0n)
          percent = roundedDifference(difference * 10000n, visitors * baseline.trackedNet);
      }
      return {
        campaign: r.campaign,
        revision: r.revision,
        variant: r.variant,
        source: r.source,
        visitors: r.visitors,
        checkoutVisitors: r.checkoutVisitors.size,
        trackedCheckoutVisitors: reachedCheckout,
        paymentVisitors: reachedPayment,
        purchasingVisitors: purchased,
        beforeCheckout: r.visitors - reachedCheckout,
        checkoutWithoutPayment: reachedCheckout - reachedPayment,
        paymentWithoutPurchase: reachedPayment - purchased,
        comparisonVariant: baseline?.variant ?? null,
        trackedNetPerVisitor: r.visitors
          ? unitsToDecimal(
              (r.trackedNet + BigInt(Math.floor(r.visitors / 2))) / BigInt(r.visitors),
              2,
            )
          : null,
        netRevenuePerVisitorDifference: change,
        netRevenuePerVisitorChangePercent: percent,
        buyers: r.buyers.size,
        bumps: r.bumps,
        upsells: r.upsells,
        upsellViews: r.upsellViews.size,
        upsellSkips: r.skipped.size,
        conversionPercent: r.visitors ? ((100 * r.buyers.size) / r.visitors).toFixed(2) : '0.00',
        currency: 'USD',
        merchandiseRevenue: unitsToDecimal(r.gross, 2),
        netRevenue: unitsToDecimal(r.net, 2),
        netRevenuePerVisitor: r.visitors
          ? unitsToDecimal((r.net + BigInt(Math.floor(r.visitors / 2))) / BigInt(r.visitors), 2)
          : '0',
      };
    }),
    commissions,
    ...(includeOrders && { orders }),
  };
}

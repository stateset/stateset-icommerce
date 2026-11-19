import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Commerce } from '@stateset/embedded';
import {
  installIntegration,
  processIntegrations,
  recordStorefrontSale,
  recordStorefrontRefund,
} from '../lib/merchant-integrations.js';
import { portalReport } from '../lib/merchant-portal.js';

const directory = mkdtempSync(join(tmpdir(), 'stateset-deliveries-'));
const db = join(directory, 'store.db');
let commerce = new Commerce(db);
const env = {
  SHIPSTATION_API_KEY: 'fixture',
  SHIPSTATION_API_SECRET: 'fixture',
  KLAVIYO_PRIVATE_KEY: 'fixture',
  GORGIAS_DOMAIN: 'fixture',
  GORGIAS_EMAIL: 'agent@fixture.test',
  GORGIAS_API_KEY: 'fixture',
  NEXT_PUBLIC_BASE_URL: 'https://fixture.test',
};
const email = 'buyer@fixture.test';
const tickets = new Map(),
  events = new Map();
let shipping,
  shipPosts = 0,
  ticketPosts = 0,
  eventPosts = 0,
  indexed = false,
  omitTicket = false,
  mismatch = false;
async function provider(url, options) {
  const { hostname, pathname, searchParams } = new URL(url);
  const body = options.body && JSON.parse(options.body);
  const json = Response.json.bind(Response);
  if (hostname === 'ssapi.shipstation.com') {
    if (pathname === '/stores') return json([{ storeId: 10 }]);
    if (pathname === '/orders') {
      const page = Number(searchParams.get('page'));
      if (!shipping) return json({ orders: [], page, pages: 0 });
      return json({
        orders:
          page === 1
            ? [
                {
                  ...shipping,
                  orderId: 50,
                  orderKey: 'foreign',
                  orderNumber: shipping.orderNumber + 'X',
                },
              ]
            : [shipping],
        page,
        pages: 2,
      });
    }
    if (pathname === '/orders/createorder') {
      shipPosts++;
      assert.equal(shipPosts, 1, 'A repeated upsert could reopen the held order');
      assert.equal(body.items[0].unitPrice, 10.5);
      shipping = { ...body, orderId: 51, orderStatus: 'on_hold' };
      throw new Error('ShipStation accepted the order before the connection closed');
    }
  }
  if (hostname === 'fixture.gorgias.com') {
    if (pathname === '/api/account') return json({ id: 15 });
    if (pathname === '/api/tickets' && options.method === 'GET') {
      const ticket = tickets.get(searchParams.get('external_id'));
      return json({ data: ticket ? [ticket] : [], meta: { next_cursor: null } });
    }
    if (pathname === '/api/tickets' && options.method === 'POST') {
      ticketPosts++;
      if (omitTicket) throw new Error('Connection lost before Gorgias recorded the ticket');
      const ticket = { id: 80 + ticketPosts, external_id: body.external_id };
      tickets.set(body.external_id, ticket);
      if (ticketPosts === 1) throw new Error('Connection lost after Gorgias recorded the ticket');
      return json(ticket);
    }
  }
  if (hostname === 'a.klaviyo.com') {
    if (pathname === '/api/accounts/') return json({ data: [{ id: 'account' }] });
    if (pathname === '/api/profiles/') {
      assert.equal(searchParams.get('filter'), 'equals(email,"' + email + '")');
      return json({ data: indexed ? [{ type: 'profile', id: 'p', attributes: { email } }] : [] });
    }
    if (pathname === '/api/events/' && options.method === 'POST') {
      const payload = body.data.attributes;
      eventPosts++;
      events.set(payload.unique_id, payload);
      return new Response(null, { status: 202 });
    }
    if (pathname === '/api/events/' && options.method === 'GET') {
      assert.match(
        searchParams.get('filter'),
        /equals\(profile_id,"p"\),greater-or-equal\(timestamp,/,
      );
      if (!searchParams.has('page[cursor]'))
        return json({
          data: [],
          links: { next: 'https://a.klaviyo.com/api/events/?page%5Bcursor%5D=next' },
        });
      assert.equal(searchParams.get('page[cursor]'), 'next');
      const data = [],
        included = [];
      for (const e of events.values()) {
        const metric = e.metric.data.attributes.name;
        data.push({
          type: 'event',
          id: 'provider-' + e.unique_id,
          attributes: {
            event_properties: {
              ...e.properties,
              $event_id: e.unique_id,
              ...(mismatch && { currency: 'CAD' }),
            },
          },
          relationships: { profile: { data: { id: 'p' } }, metric: { data: { id: metric } } },
        });
        included.push({ type: 'metric', id: metric, attributes: { name: metric } });
      }
      return json({ data, included, links: { next: null } });
    }
  }
  throw new Error('Unexpected provider operation: ' + options.method + ' ' + pathname);
}
const run = (options = {}) =>
  processIntegrations(commerce, env, { apply: true, fetchImpl: provider, ...options });
try {
  for (const integration of ['shipstation', 'gorgias', 'klaviyo'])
    await installIntegration(commerce, env, integration, true, provider);
  const customer = await commerce.customers.create({ email, firstName: 'Ada', lastName: 'Buyer' });
  const order = await commerce.orders.createExact({
    customerId: customer.id,
    currency: 'USD',
    shippingAddress: {
      line1: '1 Main St',
      city: 'Seattle',
      state: 'WA',
      postalCode: '98101',
      country: 'US',
    },
    items: [{ sku: 'COFFEE', name: 'Coffee', quantity: 1, unitPrice: '10.50' }],
  });
  const payment = await commerce.payments.createExact({
    orderId: order.id,
    customerId: customer.id,
    currency: 'USD',
    amount: order.totalAmountExact,
  });
  await commerce.payments.markCompleted(payment.id);
  await recordStorefrontSale(
    commerce,
    await commerce.orders.updateStatus(order.id, 'confirmed'),
    email,
  );
  async function credit(amount) {
    const refund = await commerce.payments.createRefundExact({ paymentId: payment.id, amount });
    await recordStorefrontRefund(
      commerce,
      order.id,
      await commerce.payments.completeRefund(refund.id),
    );
    return refund.id;
  }
  const firstRefund = await credit('1.25');
  const first = await run();
  assert.equal(first.filter((r) => r.status === 'accepted').length, 2);
  assert.equal(first.filter((r) => r.status === 'review').length, 2);
  await commerce.close();
  commerce = new Commerce(db);
  const resumed = await run();
  assert.equal(resumed.find((r) => r.provider === 'shipstation').externalId, '51');
  assert.equal(shipping.orderStatus, 'on_hold');
  assert.equal(
    resumed.find((r) => r.provider === 'gorgias' && r.sourceId === firstRefund).externalId,
    '81',
  );
  assert.equal(resumed.filter((r) => r.status === 'accepted').length, 2);
  const saleEvent = first.find((r) => r.provider === 'klaviyo' && r.event === 'sale');
  indexed = true;
  mismatch = true;
  const [wrong] = await run({ onlyKey: saleEvent.key, checkOnly: true });
  assert.equal(wrong.status, 'accepted');
  assert.match(wrong.message, /differs/);
  mismatch = false;
  const [matched] = await run({ onlyKey: saleEvent.key, checkOnly: true });
  assert.equal(matched.status, 'delivered');
  assert.ok(matched.externalId.startsWith('provider-'));
  assert.equal(eventPosts, 2);

  // A receipt lookup on an absent ticket is safe to repeat. Only an operator
  // reviewing the specific claim may authorize another external request.
  const secondRefund = await credit('0.75');
  const [pending] = (await run({ apply: false })).filter(
    (r) => r.provider === 'gorgias' && r.sourceId === secondRefund,
  );
  omitTicket = true;
  const [lost] = await run({ onlyKey: pending.key });
  assert.equal(lost.status, 'review');
  const now = Date.now() + 70000;
  const [review] = await run({ onlyKey: pending.key, checkOnly: true, now });
  assert.ok(review.claimId);
  assert.equal(ticketPosts, 2);
  omitTicket = false;
  await run({ onlyKey: pending.key, retryClaim: 'superseded', now });
  assert.equal(ticketPosts, 2);
  await Promise.all([
    run({ onlyKey: pending.key, retryClaim: review.claimId, now }),
    run({ onlyKey: pending.key, retryClaim: review.claimId, now }),
  ]);
  const [done] = await run({ onlyKey: pending.key, retryClaim: review.claimId, now });
  assert.equal(done.status, 'delivered');
  assert.equal(done.externalId, '83');
  assert.equal(ticketPosts, 3);
  assert.equal(tickets.size, 2);
  assert.ok(
    done.history.some((r) => r.action === 'retry missing delivery' && r.externalId === '83'),
  );
  assert.ok(done.history.some((r) => r.status === 'review'));
  const confirmedTicket = tickets.get(pending.key);
  tickets.delete(pending.key);
  const [missing] = await run({ onlyKey: pending.key, checkOnly: true, now: now + 1000 });
  assert.equal(missing.status, 'review');
  assert.equal(missing.externalId, done.externalId);
  const [worker] = await run({ onlyKey: pending.key });
  assert.equal(worker.status, 'review');
  assert.equal(ticketPosts, 3);
  tickets.set(pending.key, confirmedTicket);
  const [restored] = await run({ onlyKey: pending.key, checkOnly: true, now: now + 2000 });
  assert.equal(restored.status, 'delivered');
  assert.equal(restored.history.filter((r) => r.action === 'verify').length, 2);
  const merchant = await portalReport(commerce, env, { role: 'merchant', subject: 'shop' });
  assert.equal(merchant.deliveries.find((r) => r.key === done.key).externalId, '83');
  console.log(
    JSON.stringify({
      result: 'passed',
      externalProvidersVerified: false,
      checks: [
        'ShipStation held order after lost response and restart',
        'Klaviyo asynchronous event identity and paginated readback',
        'Gorgias lost response on both sides of creation',
        'concurrent reviewed retry, durable attempt history and merchant visibility',
      ],
    }),
  );
} finally {
  await commerce.close();
  rmSync(directory, { recursive: true, force: true });
}

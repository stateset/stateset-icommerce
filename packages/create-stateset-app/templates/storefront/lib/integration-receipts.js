// The merchant's delivery key stays stable across process and network failures.
// A remote resource must carry its matching business identity to confirm delivery.
function numericId(id, provider) {
  if (!Number.isSafeInteger(id) || id < 1)
    throw new Error(provider + ' returned an invalid resource ID');
  return String(id);
}

export function shipStationReceipt(order, event) {
  if (order?.orderKey !== event.orderId || order.orderNumber !== event.number)
    throw new Error('ShipStation order differs from the submitted order');
  return { externalId: numericId(order.orderId, 'ShipStation') };
}

export async function findShipStationOrder(get, event) {
  let receipt;
  for (let page = 1; ; page++) {
    const params = new URLSearchParams({
      orderNumber: event.number,
      page: String(page),
      pageSize: '100',
    });
    const result = await get('/orders?' + params);
    if (
      !Array.isArray(result.orders) ||
      result.page !== page ||
      !Number.isSafeInteger(result.pages) ||
      result.pages < 0
    )
      throw new Error('ShipStation returned an invalid order list');
    for (const order of result.orders) {
      // ShipStation's order-number filter is a prefix search.
      if (order.orderKey !== event.orderId) continue;
      const next = shipStationReceipt(order, event);
      if (receipt && receipt.externalId !== next.externalId)
        throw new Error('Multiple ShipStation orders match this delivery');
      receipt = next;
    }
    if (page >= result.pages) return receipt;
  }
}

export function gorgiasReceipt(ticket, key) {
  if (ticket?.external_id !== key)
    throw new Error('Gorgias ticket differs from the submitted refund');
  return { externalId: numericId(ticket.id, 'Gorgias') };
}

export async function findGorgiasTicket(get, key) {
  const result = await get('/api/tickets?' + new URLSearchParams({ external_id: key, limit: '2' }));
  if (!Array.isArray(result.data)) throw new Error('Gorgias returned an invalid ticket list');
  if (result.data.length > 1 || result.meta?.next_cursor)
    throw new Error('Multiple Gorgias tickets match this refund');
  return result.data.length ? gorgiasReceipt(result.data[0], key) : undefined;
}

export const klaviyoMetric = (event) =>
  event.type === 'sale' ? 'StateSet Order Paid' : 'StateSet Order Refunded';

export async function findKlaviyoEvent(get, event, uniqueId) {
  const profileParams = new URLSearchParams({
    filter: 'equals(email,' + JSON.stringify(event.email) + ')',
    'fields[profile]': 'email',
  });
  const profiles = await get('/api/profiles/?' + profileParams);
  if (!Array.isArray(profiles.data)) throw new Error('Klaviyo returned an invalid profile list');
  if (profiles.data.length === 0) return;
  const profile = profiles.data[0];
  if (
    profiles.data.length !== 1 ||
    !profile.id ||
    profile.type !== 'profile' ||
    profile.attributes?.email?.toLowerCase() !== event.email.toLowerCase()
  )
    throw new Error('Klaviyo profile differs from the submitted event');
  const second = Math.floor(Date.parse(event.at) / 1000);
  const params = new URLSearchParams({
    filter:
      'equals(profile_id,' +
      JSON.stringify(profile.id) +
      '),greater-or-equal(timestamp,' +
      (second - 1) +
      '),less-or-equal(timestamp,' +
      (second + 1) +
      ')',
    include: 'metric',
    'fields[metric]': 'name',
  });
  const seen = new Set();
  for (;;) {
    const page = await get('/api/events/?' + params);
    if (!Array.isArray(page.data)) throw new Error('Klaviyo returned an invalid event list');
    for (const row of page.data) {
      const properties = row.attributes?.event_properties;
      if (properties?.$event_id !== uniqueId) continue;
      const metricId = row.relationships?.metric?.data?.id;
      const metric = page.included?.find((m) => m.type === 'metric' && m.id === metricId);
      if (
        row.type !== 'event' ||
        typeof row.id !== 'string' ||
        !row.id ||
        row.relationships?.profile?.data?.id !== profile.id ||
        metric?.attributes?.name !== klaviyoMetric(event) ||
        properties.order_id !== event.orderId ||
        properties.order_number !== event.number ||
        (properties.refund_id !== undefined && properties.refund_id !== event.refundId) ||
        properties.amount !== (event.amount || event.total) ||
        properties.currency !== event.currency
      )
        throw new Error('Klaviyo event differs from the submitted order or refund');
      return { externalId: row.id };
    }
    if (!page.links?.next) return;
    // Only reuse the pagination cursor. Keep credentials and filters on the configured endpoint.
    const cursor = new URL(page.links.next, 'https://a.klaviyo.com').searchParams.get(
      'page[cursor]',
    );
    if (!cursor || seen.has(cursor)) throw new Error('Klaviyo returned an invalid event cursor');
    seen.add(cursor);
    params.set('page[cursor]', cursor);
  }
}

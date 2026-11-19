import { createHash, randomUUID } from 'node:crypto';
import {
  firstMerchantRecord,
  merchantRecords,
  readMerchantRecord,
  saveMerchantRecord,
} from './merchant-records.js';

import {
  findShipStationOrder,
  shipStationReceipt,
  findGorgiasTicket,
  gorgiasReceipt,
  findKlaviyoEvent,
  klaviyoMetric,
} from './integration-receipts.js';

const providers = ['shipstation', 'klaviyo', 'gorgias'];
const digest = (text) => createHash('sha256').update(text).digest('hex');
const installation = (name) => 'integration-' + name;

// All endpoints are provider-owned. Only the Gorgias tenant slug is configurable.
function connection(name, env) {
  let origin, auth;
  if (name === 'klaviyo' && env.KLAVIYO_PRIVATE_KEY) {
    origin = 'https://a.klaviyo.com';
    auth = { Authorization: 'Klaviyo-API-Key ' + env.KLAVIYO_PRIVATE_KEY, revision: '2026-07-15' };
  } else if (name === 'shipstation' && env.SHIPSTATION_API_KEY && env.SHIPSTATION_API_SECRET) {
    origin = 'https://ssapi.shipstation.com';
    auth = {
      Authorization:
        'Basic ' +
        Buffer.from(env.SHIPSTATION_API_KEY + ':' + env.SHIPSTATION_API_SECRET).toString('base64'),
    };
  } else if (
    name === 'gorgias' &&
    /^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/.test(env.GORGIAS_DOMAIN || '') &&
    env.GORGIAS_EMAIL &&
    env.GORGIAS_API_KEY
  ) {
    origin = 'https://' + env.GORGIAS_DOMAIN + '.gorgias.com';
    auth = {
      Authorization:
        'Basic ' + Buffer.from(env.GORGIAS_EMAIL + ':' + env.GORGIAS_API_KEY).toString('base64'),
    };
  } else throw new Error('Configure merchant credentials for ' + name);
  return { origin, auth };
}

async function request(name, env, path, body, fetchImpl) {
  const { origin, auth } = connection(name, env);
  const res = await fetchImpl(origin + path, {
    method: body === undefined ? 'GET' : 'POST',
    headers: { ...auth, Accept: 'application/json', 'Content-Type': 'application/json' },
    redirect: 'error',
    cache: 'no-store',
    signal: AbortSignal.timeout(12000),
    ...(body === undefined ? {} : { body }),
  });
  if (!res.ok) {
    const error = new Error(name + ' returned HTTP ' + res.status);
    error.status = res.status;
    throw error;
  }
  const text = await res.text();
  return text.trim() ? JSON.parse(text) : {};
}

export async function installIntegration(commerce, env, name, apply, fetchImpl = fetch) {
  if (!providers.includes(name)) throw new Error('Choose shipstation, klaviyo or gorgias');
  connection(name, env);
  if (!apply) return { mode: 'preview', integration: name };
  const probe = { shipstation: '/stores', klaviyo: '/api/accounts/', gorgias: '/api/account' };
  await request(name, env, probe[name], undefined, fetchImpl);
  const installed = await readMerchantRecord(commerce, installation(name));
  if (!installed)
    await firstMerchantRecord(commerce, installation(name), {
      name,
      installedAt: new Date().toISOString(),
    });
  return { integration: name, connected: true };
}

export async function integrationStatus(commerce, env) {
  return Promise.all(
    providers.map(async (name) => {
      let configured = true;
      try {
        connection(name, env);
      } catch {
        configured = false;
      }
      const binding = await readMerchantRecord(commerce, installation(name));
      return { name, configured, installedAt: binding?.installedAt ?? null };
    }),
  );
}

export async function recordStorefrontSale(commerce, order, contact) {
  const key = 'sale-' + order.id;
  if (await readMerchantRecord(commerce, key)) return;
  const customer = await commerce.customers.get(order.customerId);
  const snapshot = await readMerchantRecord(commerce, 'merchandise-' + order.id);
  const shippingFee = 'SHIPPING-' + String(order.shippingMethod || '').toUpperCase();
  const items =
    snapshot?.items ??
    order.items
      .filter((item, i) => i !== order.items.length - 1 || item.sku !== shippingFee)
      .map((item) => ({
        sku: item.sku,
        name: item.name,
        quantity: item.quantity,
        unitPrice: item.unitPriceExact,
      }));
  await firstMerchantRecord(
    commerce,
    key,
    {
      type: 'sale',
      orderId: order.id,
      number: order.orderNumber,
      currency: order.currency.toUpperCase(),
      total: order.totalAmountExact,
      items,
      at: new Date().toISOString(),
      placedAt: order.createdAt,
      email: contact,
      name: [customer?.firstName, customer?.lastName].filter(Boolean).join(' '),
      shipping: order.shippingAddress,
    },
    'merchant-events',
  );
}

export async function recordStorefrontRefund(commerce, orderId, refund) {
  const sale = await readMerchantRecord(commerce, 'sale-' + orderId);
  if (!sale || refund.status !== 'completed') return;
  const key = 'refund-' + refund.id;
  const previous = await readMerchantRecord(commerce, key);
  await saveMerchantRecord(
    commerce,
    key,
    {
      type: 'refund',
      at: previous?.at || refund.refundedAt || refund.createdAt,
      orderId,
      refundId: refund.id,
      number: sale.number,
      email: sale.email,
      amount: refund.amountExact,
      currency: sale.currency,
    },
    'merchant-events',
  );
}

export function sendKlaviyoEvent(env, { key, email, metric, properties, at }, fetchImpl = fetch) {
  return request(
    'klaviyo',
    env,
    '/api/events/',
    JSON.stringify({
      data: {
        type: 'event',
        attributes: {
          unique_id: digest(key),
          ...(at && { time: at }),
          properties,
          metric: { data: { type: 'metric', attributes: { name: metric } } },
          profile: { data: { type: 'profile', attributes: { email } } },
        },
      },
    }),
    fetchImpl,
  );
}

function shipStationBody(event) {
  const shipping = event.shipping;
  const address = {
    name: event.name || event.email,
    street1: shipping.line1,
    street2: shipping.line2,
    city: shipping.city,
    state: shipping.state,
    postalCode: shipping.postalCode,
    country: shipping.country,
  };
  const body = {
    orderKey: event.orderId,
    orderNumber: event.number,
    orderDate: event.placedAt || event.at,
    paymentDate: event.at,
    orderStatus: 'awaiting_shipment',
    customerEmail: event.email,
    billTo: address,
    shipTo: address,
  };
  // ShipStation expects a JSON number. Emit the validated decimal text without a float conversion.
  const items = event.items.map(({ sku, name, quantity, unitPrice }) => {
    if (
      !/^(0|[1-9]\d*)(\.\d+)?$/.test(unitPrice) ||
      !Number.isSafeInteger(quantity) ||
      quantity < 1
    )
      throw new Error('Invalid order line');
    return JSON.stringify({ sku, name, quantity }).slice(0, -1) + ',"unitPrice":' + unitPrice + '}';
  });
  return JSON.stringify(body).slice(0, -1) + ',"items":[' + items.join(',') + ']}';
}

async function findReceipt(name, env, event, key, fetchImpl) {
  const get = (path) => request(name, env, path, undefined, fetchImpl);
  if (name === 'shipstation') return findShipStationOrder(get, event);
  if (name === 'gorgias') return findGorgiasTicket(get, key);
  return findKlaviyoEvent(get, event, digest(key));
}

async function deliver(name, env, event, key, fetchImpl, commerce) {
  if (name !== 'klaviyo') {
    const existing = await findReceipt(name, env, event, key, fetchImpl);
    if (existing) return existing;
  }
  if (name === 'shipstation') {
    const order = await commerce.orders.get(event.orderId);
    if (!order) throw new Error('Local order was not found');
    if (order.paymentStatus === 'refunded') return { skipped: 'refunded' };
    if (
      ['cancelled', 'refunded', 'shipped', 'delivered', 'partially_shipped'].includes(
        order.status,
      ) ||
      ['fulfilled', 'shipped', 'delivered', 'partially_fulfilled'].includes(order.fulfillmentStatus)
    )
      return {
        skipped:
          order.fulfillmentStatus && order.fulfillmentStatus !== 'unfulfilled'
            ? order.fulfillmentStatus
            : order.status,
      };
    const response = await dispatch(
      commerce,
      name,
      key,
      env,
      '/orders/createorder',
      shipStationBody(event),
      fetchImpl,
    );
    return shipStationReceipt(response, event);
  }
  if (name === 'klaviyo') {
    await sendKlaviyoEvent(
      env,
      {
        key,
        email: event.email,
        at: event.at,
        metric: klaviyoMetric(event),
        properties: {
          order_id: event.orderId,
          order_number: event.number,
          ...(event.refundId && { refund_id: event.refundId }),
          amount: event.amount || event.total,
          currency: event.currency,
        },
      },
      fetchImpl,
    );
    return { accepted: true };
  }
  const ticket = await dispatch(
    commerce,
    name,
    key,
    env,
    '/api/tickets',
    JSON.stringify({
      external_id: key,
      status: 'open',
      subject: 'Refund for ' + event.number,
      customer: { email: event.email },
      messages: [
        {
          channel: 'internal-note',
          via: 'api',
          from_agent: true,
          sender: { email: env.GORGIAS_EMAIL },
          body_text:
            'Refund ' +
            event.refundId +
            ' for ' +
            event.amount +
            ' ' +
            event.currency +
            ' on order ' +
            event.number,
        },
      ],
    }),
    fetchImpl,
  );
  return gorgiasReceipt(ticket, key);
}

async function dispatch(commerce, name, key, env, path, body, fetchImpl) {
  const dispatched = await claimDispatch(commerce, key, name);
  if (!dispatched) throw new Error('An earlier ' + name + ' request needs a provider receipt.');
  try {
    return await request(name, env, path, body, fetchImpl);
  } catch (error) {
    if ([400, 401, 403, 422, 429].includes(error.status))
      await commerce.customObjects.deleteObject(dispatched);
    throw error;
  }
}

async function claimDispatch(commerce, key, provider) {
  const records = commerce.customObjects;
  const handle = 'claim-' + key;
  try {
    const claim = await records.createObject({
      typeHandle: 'storefront_operation',
      handle,
      valuesJson: JSON.stringify({ data: { provider, startedAt: Date.now() } }),
    });
    return claim.id;
  } catch (error) {
    if (await readMerchantRecord(commerce, handle)) return null;
    throw error;
  }
}

// Poll delayed provider indexing less often over time. A targeted check can
// always be run from the portal or CLI after the automatic checks finish.
const checkDelays = [0, 60000, 300000, 1800000, 7200000, 43200000, 86400000];
async function dueForCheck(commerce, key, now) {
  const start = await firstMerchantRecord(commerce, 'check-start-' + key, { at: now });
  const stage = checkDelays.findLastIndex((delay) => now - start.at >= delay);
  if (stage < 0) return false;
  const attempt = 'check-' + stage + '-' + key;
  if (await readMerchantRecord(commerce, attempt)) return false;
  await saveMerchantRecord(commerce, attempt, { stage });
  return true;
}

async function deliveryReport(commerce, env, provider, event, key, options, lastError) {
  const { apply, onlyKey, checkOnly, retryClaim, fetchImpl, now } = options;
  const confirmedKey = 'provider-' + key;
  const proof = await readMerchantRecord(commerce, confirmedKey);
  const previous = await readMerchantRecord(commerce, key);
  const base = {
    key,
    provider,
    orderId: event.orderId,
    event: event.type,
    sourceId: event.refundId || event.orderId,
  };
  if (proof) {
    const row = {
      ...base,
      status: 'delivered',
      externalId: proof.externalId,
      confirmedAt: proof.confirmedAt,
    };
    const collection = 'rechecks-' + key;
    if (apply && checkOnly) {
      let message;
      try {
        const existing = await findReceipt(provider, env, event, key, fetchImpl);
        if (existing?.externalId !== proof.externalId)
          message = 'The provider no longer returns the confirmed resource ID.';
      } catch (error) {
        message = error.message;
      }
      const result = {
        at: new Date(now).toISOString(),
        action: 'verify',
        status: message ? 'review' : 'delivered',
        externalId: proof.externalId,
        ...(message && { message }),
      };
      await saveMerchantRecord(commerce, 'delivery-attempt-' + randomUUID(), result, collection);
      return {
        ...row,
        status: result.status,
        ...(message ? { message } : { confirmedAt: result.at }),
      };
    }
    const [latest] = await commerce.customObjects.listObjects({
      typeHandle: 'storefront_operation',
      ownerType: 'storefront',
      ownerId: collection,
      offset: 0,
      limit: 1,
    });
    if (latest) {
      const current = JSON.parse(latest.valuesJson).data;
      if (current.status === 'review')
        return { ...row, status: 'review', message: current.message };
      return { ...row, confirmedAt: current.at };
    }
    return row;
  }
  if (previous?.skipped) return { ...base, status: 'skipped', message: previous.skipped };
  const claim = await (provider !== 'klaviyo'
    ? commerce.customObjects.getObjectByHandle('storefront_operation', 'claim-' + key)
    : null);
  const started =
    claim && (JSON.parse(claim.valuesJson).data.startedAt || Date.parse(claim.createdAt));
  const review = !!claim && !previous;
  const eligible = review && (!Number.isFinite(started) || now - started >= 60000);
  const row = {
    ...base,
    status: previous ? 'accepted' : review ? 'review' : lastError ? 'failed' : 'pending',
    ...(review && {
      message: 'Check ' + provider + ' for the original request.',
      ...(eligible && { claimId: claim.id }),
    }),
    ...(lastError && { message: lastError }),
  };
  if (!apply) return row;
  const attempt = randomUUID();
  const evidence = 'attempts-' + key;
  let action = previous || review || checkOnly ? 'check' : 'send';
  async function finish(current) {
    const { status, externalId, message } = current;
    await saveMerchantRecord(
      commerce,
      'delivery-attempt-' + attempt,
      {
        at: new Date(now).toISOString(),
        action,
        status,
        ...(externalId && { externalId }),
        ...(message && { message }),
      },
      evidence,
    );
    return current;
  }
  try {
    let result;
    if (previous || review || checkOnly) {
      if (!onlyKey && !checkOnly && !(await dueForCheck(commerce, key, now))) return row;
      result = await findReceipt(provider, env, event, key, fetchImpl);
      if (!result && retryClaim) {
        if (!eligible || retryClaim !== claim?.id)
          throw new Error('The selected provider attempt is no longer awaiting a retry');
        // Only release the specific dispatch the operator reviewed. An older
        // page or duplicate click cannot release a new worker's claim.
        action = 'retry missing delivery';
        await firstMerchantRecord(
          commerce,
          'reviewed-' + claim.id,
          { key, at: new Date(now).toISOString(), claimId: claim.id },
          'delivery-retries-' + key,
        );
        await commerce.customObjects.deleteObject(claim.id);
        result = await deliver(provider, env, event, key, fetchImpl, commerce);
      }
      if (!result)
        return finish({
          ...row,
          message: review
            ? 'The original request has no matching provider record yet.'
            : 'No matching provider record found yet.',
        });
    } else {
      if (retryClaim) throw new Error('The selected dispatch no longer exists');
      result = await deliver(provider, env, event, key, fetchImpl, commerce);
    }
    if (result.externalId) {
      if (
        previous?.externalId &&
        /^[1-9]\d*$/.test(previous.externalId) &&
        previous.externalId !== result.externalId
      )
        throw new Error('Provider ID differs from the original acknowledgement');
      const first = await firstMerchantRecord(commerce, confirmedKey, {
        externalId: result.externalId,
        confirmedAt: new Date(now).toISOString(),
      });
      if (first.externalId !== result.externalId)
        throw new Error('Concurrent delivery checks found different provider IDs');
      return finish({
        ...base,
        status: 'delivered',
        externalId: first.externalId,
        confirmedAt: first.confirmedAt,
      });
    }
    await saveMerchantRecord(commerce, key, result);
    return finish(
      result.skipped
        ? { ...base, status: 'skipped', message: result.skipped }
        : { ...base, status: 'accepted' },
    );
  } catch (error) {
    const message = error.message;
    await firstMerchantRecord(
      commerce,
      'error-' + digest(key + ':' + message),
      { key, message },
      'integration-errors',
    );
    const unresolved =
      provider !== 'klaviyo' && (await readMerchantRecord(commerce, 'claim-' + key));
    return finish({
      ...row,
      status: previous ? 'accepted' : unresolved ? 'review' : 'failed',
      message,
    });
  }
}

export async function processIntegrations(
  commerce,
  env,
  {
    apply = false,
    onlyKey = '',
    checkOnly = false,
    retryClaim = '',
    fetchImpl = fetch,
    now = Date.now(),
  } = {},
) {
  if (retryClaim && (!onlyKey || !apply || checkOnly))
    throw new Error('Choose one reviewed delivery and apply the retry');
  const installations = new Map();
  for (const provider of providers) {
    const installed = await readMerchantRecord(commerce, installation(provider));
    if (installed) installations.set(provider, installed);
  }
  const deliveries = [];
  const errors = new Map();
  for await (const failure of merchantRecords(commerce, 'integration-errors')) {
    if (!errors.has(failure.key)) errors.set(failure.key, failure.message);
  }
  for await (const event of merchantRecords(commerce, 'merchant-events')) {
    for (const [provider, installed] of installations) {
      if (
        Date.parse(event.at) < Date.parse(installed.installedAt) ||
        (provider === 'shipstation' && event.type !== 'sale') ||
        (provider === 'gorgias' && event.type !== 'refund')
      )
        continue;
      const key = 'delivery-' + digest(provider + ':' + event.key);
      if (onlyKey && key !== onlyKey) continue;
      if (retryClaim && provider === 'klaviyo')
        throw new Error('Klaviyo events use their original unique ID for retries');
      deliveries.push(
        await deliveryReport(
          commerce,
          env,
          provider,
          event,
          key,
          { apply, onlyKey, checkOnly, retryClaim, fetchImpl, now },
          errors.get(key),
        ),
      );
    }
  }
  if (onlyKey && !deliveries.length) throw new Error('Unknown integration delivery');
  if (onlyKey) {
    const history = [];
    for (const collection of ['attempts-', 'rechecks-'])
      for await (const { key: handle, ...entry } of merchantRecords(commerce, collection + onlyKey))
        history.push(entry);
    deliveries[0].history = history.sort((a, b) => b.at.localeCompare(a.at));
  }
  return deliveries;
}

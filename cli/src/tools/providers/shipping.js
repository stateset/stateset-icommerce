import {
  clone,
  deterministicId,
  ensureProvider,
  filterProvidersByCapability,
  moneyToNumber,
  normalizeMoney,
  nowIso,
  roundMoney,
} from './runtime.js';

const DEFAULT_SHIPPING_PROVIDER = 'deterministic-mock';

const SHIPPING_PROVIDERS = Object.freeze([
  {
    id: 'deterministic-mock',
    name: 'Deterministic Mock Carrier',
    mode: 'sandbox',
    status: 'active',
    capabilities: ['rate_quote', 'label_create', 'label_void', 'tracking', 'exceptions', 'replay'],
    services: [
      { code: 'ground', name: 'Ground', minDays: 3, maxDays: 5, rateMultiplier: 1.0 },
      { code: 'priority', name: 'Priority', minDays: 2, maxDays: 3, rateMultiplier: 1.45 },
      { code: 'express', name: 'Express', minDays: 1, maxDays: 2, rateMultiplier: 1.95 },
    ],
  },
  {
    id: 'shippo',
    name: 'Shippo Adapter',
    mode: 'shadow',
    status: 'shadow',
    capabilities: ['rate_quote', 'label_create', 'label_void', 'tracking', 'exceptions'],
    services: [
      {
        code: 'shippo_ground',
        name: 'Shippo Ground',
        minDays: 3,
        maxDays: 6,
        rateMultiplier: 1.05,
      },
      {
        code: 'shippo_priority',
        name: 'Shippo Priority',
        minDays: 2,
        maxDays: 4,
        rateMultiplier: 1.5,
      },
      {
        code: 'shippo_express',
        name: 'Shippo Express',
        minDays: 1,
        maxDays: 2,
        rateMultiplier: 2.1,
      },
    ],
  },
  {
    id: 'carrier-hub',
    name: 'Carrier Hub (UPS/FedEx/USPS)',
    mode: 'shadow',
    status: 'shadow',
    capabilities: ['rate_quote', 'label_create', 'label_void', 'tracking', 'exceptions'],
    services: [
      { code: 'ups_ground', name: 'UPS Ground', minDays: 3, maxDays: 5, rateMultiplier: 1.15 },
      {
        code: 'fedex_2day',
        name: 'FedEx 2Day',
        minDays: 2,
        maxDays: 2,
        rateMultiplier: 1.8,
      },
      {
        code: 'usps_priority',
        name: 'USPS Priority',
        minDays: 1,
        maxDays: 3,
        rateMultiplier: 1.35,
      },
    ],
  },
]);

const labelsById = new Map();
const quotesByRateId = new Map();
const trackingNumberIndex = new Map();
const labelIdempotencyKeys = new Map();
const providerSequences = new Map();
const processedWebhookEvents = new Map();

function nextSequence(providerId) {
  const next = (providerSequences.get(providerId) || 0) + 1;
  providerSequences.set(providerId, next);
  return next;
}

function getProviderService(provider, serviceCode) {
  if (!serviceCode) {
    return provider.services[0];
  }
  return provider.services.find((service) => service.code === serviceCode) || null;
}

function calculateZoneFactor(originAddress = {}, destinationAddress = {}) {
  const originCountry = (originAddress.country || '').toUpperCase();
  const destinationCountry = (destinationAddress.country || '').toUpperCase();

  if (!originCountry || !destinationCountry) {
    return 1.2;
  }
  if (originCountry !== destinationCountry) {
    return 1.65;
  }
  if (
    originAddress.state &&
    destinationAddress.state &&
    originAddress.state !== destinationAddress.state
  ) {
    return 1.25;
  }
  return 1.0;
}

function normalizeParcels(parcels) {
  return parcels.map((parcel, index) => ({
    index,
    weightGrams: parcel.weightGrams ? moneyToNumber(parcel.weightGrams) : 500,
    lengthCm: parcel.lengthCm ? moneyToNumber(parcel.lengthCm) : null,
    widthCm: parcel.widthCm ? moneyToNumber(parcel.widthCm) : null,
    heightCm: parcel.heightCm ? moneyToNumber(parcel.heightCm) : null,
  }));
}

function estimateBaseRate({ parcels, zoneFactor }) {
  const totalWeightKg =
    parcels.reduce((sum, parcel) => sum + moneyToNumber(parcel.weightGrams), 0) / 1000;
  const parcelCount = parcels.length || 1;
  return roundMoney((4.5 + totalWeightKg * 1.3 + parcelCount * 0.6) * zoneFactor);
}

function ensureLabelByLookup({ labelId, trackingNumber }) {
  if (labelId) {
    const label = labelsById.get(labelId);
    if (!label) {
      throw new Error(`Shipping label "${labelId}" not found`);
    }
    return label;
  }

  if (trackingNumber) {
    const resolvedLabelId = trackingNumberIndex.get(trackingNumber);
    const label = resolvedLabelId ? labelsById.get(resolvedLabelId) : null;
    if (!label) {
      throw new Error(`Shipping label for tracking number "${trackingNumber}" not found`);
    }
    return label;
  }

  throw new Error('Either labelId or trackingNumber is required');
}

function progressLabelStatus(label) {
  if (['voided', 'delivered'].includes(label.status)) {
    return;
  }

  const createdAt = nowIso();
  const transitions = {
    label_created: { status: 'in_transit', description: 'Package accepted by carrier' },
    in_transit: { status: 'out_for_delivery', description: 'Package is out for delivery' },
    out_for_delivery: { status: 'delivered', description: 'Package delivered' },
  };

  const transition = transitions[label.status];
  if (!transition) {
    return;
  }

  label.status = transition.status;
  label.updatedAt = createdAt;
  label.events.push({
    timestamp: createdAt,
    status: transition.status,
    description: transition.description,
    location: label.destinationAddress?.city || label.destinationAddress?.country || null,
  });
}

function normalizeEventType(eventType) {
  return String(eventType || '')
    .trim()
    .toLowerCase();
}

function deriveWebhookEventId(providerId, eventType, eventId, payload = {}) {
  if (eventId) {
    return String(eventId);
  }

  const candidate = payload.eventId || payload.event_id || payload.id || null;
  if (candidate) {
    return `${providerId}:${candidate}`;
  }

  return deterministicId('swevt', { providerId, eventType, payload });
}

function resolveWebhookLabel(payload = {}) {
  const labelId = payload.labelId || payload.label_id || payload.shipmentLabelId || null;
  if (labelId && labelsById.has(labelId)) {
    return labelsById.get(labelId);
  }

  const trackingNumber =
    payload.trackingNumber ||
    payload.tracking_number ||
    payload.tracking?.number ||
    payload.data?.tracking_number ||
    null;

  if (trackingNumber && trackingNumberIndex.has(trackingNumber)) {
    return labelsById.get(trackingNumberIndex.get(trackingNumber));
  }

  return null;
}

function mapEventTypeToLabelStatus(eventType, payload = {}) {
  if (payload.status) {
    return String(payload.status).toLowerCase();
  }

  const normalized = normalizeEventType(eventType);
  const statusMap = {
    track_updated: 'in_transit',
    'tracking.update': 'in_transit',
    'shipment.in_transit': 'in_transit',
    'shipment.picked_up': 'in_transit',
    'shipment.out_for_delivery': 'out_for_delivery',
    'shipment.delivered': 'delivered',
    delivered: 'delivered',
    'shipment.exception': 'exception',
    'tracking.exception': 'exception',
    exception: 'exception',
    'label.voided': 'voided',
    voided: 'voided',
  };

  return statusMap[normalized] || null;
}

export function listShippingProviders({ capability, mode } = {}) {
  let providers = filterProvidersByCapability(SHIPPING_PROVIDERS, capability);
  if (mode) {
    providers = providers.filter((provider) => provider.mode === mode);
  }
  return providers.map((provider) => clone(provider));
}

export function quoteShippingRates({
  providerId = DEFAULT_SHIPPING_PROVIDER,
  originAddress = {},
  destinationAddress = {},
  parcels = [],
  currency = 'USD',
  serviceCodes,
} = {}) {
  if (!Array.isArray(parcels) || parcels.length === 0) {
    throw new Error('At least one parcel is required to quote shipping rates');
  }

  const provider = ensureProvider(SHIPPING_PROVIDERS, providerId, DEFAULT_SHIPPING_PROVIDER);
  const normalizedParcels = normalizeParcels(parcels);
  const zoneFactor = calculateZoneFactor(originAddress, destinationAddress);
  const baseRate = estimateBaseRate({ parcels: normalizedParcels, zoneFactor });
  const normalizedCurrency = currency.toUpperCase();
  const allowedServices =
    Array.isArray(serviceCodes) && serviceCodes.length > 0
      ? provider.services.filter((service) => serviceCodes.includes(service.code))
      : provider.services;

  const rates = allowedServices.map((service) => {
    const rateAmount = roundMoney(baseRate * service.rateMultiplier);
    const rateId = deterministicId('rate', {
      providerId: provider.id,
      serviceCode: service.code,
      destinationAddress,
      originAddress,
      parcels: normalizedParcels,
      currency: normalizedCurrency,
    });

    const quote = {
      rateId,
      providerId: provider.id,
      providerName: provider.name,
      serviceCode: service.code,
      serviceName: service.name,
      amount: normalizeMoney(rateAmount),
      currency: normalizedCurrency,
      minDeliveryDays: service.minDays,
      maxDeliveryDays: service.maxDays,
      createdAt: nowIso(),
    };

    quotesByRateId.set(rateId, {
      ...quote,
      request: {
        originAddress,
        destinationAddress,
        parcels: normalizedParcels,
      },
    });

    return quote;
  });

  return {
    provider: clone(provider),
    rates: rates.map((rate) => clone(rate)),
  };
}

export function createShippingLabel({
  providerId = DEFAULT_SHIPPING_PROVIDER,
  rateId,
  serviceCode,
  orderId,
  shipmentId,
  originAddress = {},
  destinationAddress = {},
  parcels = [],
  currency = 'USD',
  metadata = {},
  idempotencyKey,
} = {}) {
  const provider = ensureProvider(SHIPPING_PROVIDERS, providerId, DEFAULT_SHIPPING_PROVIDER);

  if (idempotencyKey) {
    const existingLabelId = labelIdempotencyKeys.get(`${provider.id}:${idempotencyKey}`);
    if (existingLabelId) {
      return {
        provider: clone(provider),
        label: clone(labelsById.get(existingLabelId)),
        idempotent: true,
      };
    }
  }

  const quote = rateId ? quotesByRateId.get(rateId) : null;
  const finalOriginAddress = quote?.request?.originAddress || originAddress;
  const finalDestinationAddress = quote?.request?.destinationAddress || destinationAddress;
  const finalParcels = quote?.request?.parcels || normalizeParcels(parcels);

  if (!Array.isArray(finalParcels) || finalParcels.length === 0) {
    throw new Error('At least one parcel is required to create a shipping label');
  }

  const service = quote
    ? getProviderService(provider, quote.serviceCode)
    : getProviderService(provider, serviceCode);

  if (!service) {
    throw new Error(`Service "${serviceCode}" is not available for provider "${provider.id}"`);
  }

  const baseRate = estimateBaseRate({
    parcels: finalParcels,
    zoneFactor: calculateZoneFactor(finalOriginAddress, finalDestinationAddress),
  });
  const computedRate =
    quote?.amount || normalizeMoney(roundMoney(baseRate * service.rateMultiplier));
  const normalizedCurrency = (quote?.currency || currency).toUpperCase();

  const sequence = nextSequence(provider.id);
  const createdAt = nowIso();
  const labelId = deterministicId('lbl', {
    providerId: provider.id,
    sequence,
    orderId: orderId || null,
    shipmentId: shipmentId || null,
    serviceCode: service.code,
  });
  const trackingNumber = `SS${deterministicId('trk', { labelId, providerId: provider.id })
    .replace('_', '')
    .slice(0, 14)
    .toUpperCase()}`;

  const label = {
    id: labelId,
    providerId: provider.id,
    providerName: provider.name,
    orderId: orderId || null,
    shipmentId: shipmentId || null,
    rateId: quote?.rateId || null,
    serviceCode: service.code,
    serviceName: service.name,
    amount: computedRate,
    currency: normalizedCurrency,
    trackingNumber,
    status: 'label_created',
    labelUrl: `https://labels.stateset.local/${labelId}.pdf`,
    originAddress: finalOriginAddress,
    destinationAddress: finalDestinationAddress,
    parcels: finalParcels,
    metadata: metadata || {},
    createdAt,
    updatedAt: createdAt,
    events: [
      {
        timestamp: createdAt,
        status: 'label_created',
        description: 'Shipping label created',
        location: finalOriginAddress?.city || finalOriginAddress?.country || null,
      },
    ],
  };

  labelsById.set(label.id, label);
  trackingNumberIndex.set(trackingNumber, label.id);

  if (idempotencyKey) {
    labelIdempotencyKeys.set(`${provider.id}:${idempotencyKey}`, label.id);
  }

  return {
    provider: clone(provider),
    label: clone(label),
    idempotent: false,
  };
}

export function getShippingLabel(labelId) {
  const label = labelsById.get(labelId);
  return label ? clone(label) : null;
}

export function listShippingLabels({
  providerId,
  status,
  orderId,
  shipmentId,
  trackingNumber,
  limit = 100,
} = {}) {
  const boundedLimit = Math.min(500, Math.max(1, Math.floor(Number(limit) || 100)));
  let labels = Array.from(labelsById.values());
  if (providerId) {
    labels = labels.filter((label) => label.providerId === providerId);
  }
  if (status) {
    labels = labels.filter((label) => label.status === status);
  }
  if (orderId) {
    labels = labels.filter((label) => label.orderId === orderId);
  }
  if (shipmentId) {
    labels = labels.filter((label) => label.shipmentId === shipmentId);
  }
  if (trackingNumber) {
    labels = labels.filter((label) => label.trackingNumber === trackingNumber);
  }
  labels.sort((a, b) => String(b.createdAt).localeCompare(String(a.createdAt)));
  return labels.slice(0, boundedLimit).map((label) => clone(label));
}

export function voidShippingLabel({ labelId, reason } = {}) {
  const label = ensureLabelByLookup({ labelId });

  if (label.status === 'voided') {
    return { label: clone(label), idempotent: true };
  }

  if (label.status === 'delivered') {
    throw new Error(`Delivered label "${label.id}" cannot be voided`);
  }

  const createdAt = nowIso();
  label.status = 'voided';
  label.voidReason = reason || null;
  label.voidedAt = createdAt;
  label.updatedAt = createdAt;
  label.events.push({
    timestamp: createdAt,
    status: 'voided',
    description: reason || 'Label voided',
    location: label.originAddress?.city || label.originAddress?.country || null,
  });

  return { label: clone(label), idempotent: false };
}

export function trackShippingLabel({ labelId, trackingNumber, advanceStatus = false } = {}) {
  const label = ensureLabelByLookup({ labelId, trackingNumber });
  if (advanceStatus) {
    progressLabelStatus(label);
  }

  const latestEvent = label.events[label.events.length - 1] || null;
  return {
    label: clone(label),
    latestEvent: latestEvent ? clone(latestEvent) : null,
  };
}

export function ingestShippingProviderWebhook({
  providerId = DEFAULT_SHIPPING_PROVIDER,
  eventType,
  eventId,
  payload = {},
} = {}) {
  if (!eventType) {
    throw new Error('eventType is required');
  }

  const provider = ensureProvider(SHIPPING_PROVIDERS, providerId, DEFAULT_SHIPPING_PROVIDER);
  const normalizedEventType = normalizeEventType(eventType);
  const resolvedEventId = deriveWebhookEventId(provider.id, normalizedEventType, eventId, payload);
  const webhookKey = `${provider.id}:${resolvedEventId}`;
  const existing = processedWebhookEvents.get(webhookKey);
  if (existing) {
    return {
      ...clone(existing),
      idempotent: true,
    };
  }

  const label = resolveWebhookLabel(payload);
  if (!label) {
    const missingLabelResult = {
      provider: clone(provider),
      eventType: normalizedEventType,
      eventId: resolvedEventId,
      action: 'ignored',
      applied: false,
      reason: 'label_not_found',
      label: null,
      idempotent: false,
    };
    processedWebhookEvents.set(webhookKey, clone(missingLabelResult));
    return missingLabelResult;
  }

  const nextStatus = mapEventTypeToLabelStatus(normalizedEventType, payload);
  let action = 'ignored';
  let applied = false;
  let reason = null;

  if (!nextStatus) {
    reason = 'unsupported_event_type';
  } else if (label.status === nextStatus) {
    action = 'already_in_status';
    reason = 'status_unchanged';
  } else {
    action = 'status_updated';
    applied = true;
    label.status = nextStatus;
    label.updatedAt = nowIso();
    if (nextStatus === 'voided') {
      label.voidedAt = label.updatedAt;
      label.voidReason = payload.reason || payload.description || label.voidReason || null;
    }
    label.events.push({
      timestamp: label.updatedAt,
      status: nextStatus,
      description:
        payload.description ||
        payload.message ||
        payload.tracking_status?.status_details ||
        `Webhook event ${normalizedEventType}`,
      location:
        payload.location ||
        payload.tracking_status?.location ||
        label.destinationAddress?.city ||
        label.destinationAddress?.country ||
        null,
      eventId: resolvedEventId,
    });
  }

  const result = {
    provider: clone(provider),
    eventType: normalizedEventType,
    eventId: resolvedEventId,
    action,
    applied,
    reason,
    label: clone(label),
    idempotent: false,
  };

  processedWebhookEvents.set(webhookKey, clone(result));
  return result;
}

export function __resetShippingProviderState() {
  labelsById.clear();
  quotesByRateId.clear();
  trackingNumberIndex.clear();
  labelIdempotencyKeys.clear();
  providerSequences.clear();
  processedWebhookEvents.clear();
}

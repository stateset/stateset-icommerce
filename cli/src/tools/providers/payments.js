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

const DEFAULT_PAYMENT_PROVIDER = 'deterministic-mock';

const PAYMENT_PROVIDERS = Object.freeze([
  {
    id: 'deterministic-mock',
    name: 'Deterministic Mock Gateway',
    mode: 'sandbox',
    status: 'active',
    capabilities: [
      'intents',
      'capture',
      'cancel',
      'refund',
      'idempotency',
      'replay',
      'settlement',
      'reconciliation',
    ],
    description: 'Deterministic in-memory gateway for simulation, replay, and local development.',
  },
  {
    id: 'stripe',
    name: 'Stripe Interop Adapter',
    mode: 'shadow',
    status: 'shadow',
    capabilities: [
      'intents',
      'capture',
      'cancel',
      'refund',
      'idempotency',
      'webhooks',
      'settlement',
      'reconciliation',
    ],
    description:
      'Stripe-compatible adapter skeleton for staged rollout from shadow mode to production.',
  },
]);

const paymentIntents = new Map();
const capturesById = new Map();
const refundsById = new Map();
const providerSequences = new Map();
const intentIdempotencyKeys = new Map();
const captureIdempotencyKeys = new Map();
const refundIdempotencyKeys = new Map();
const providerIntentIdIndex = new Map();
const processedWebhookEvents = new Map();
const settlementsById = new Map();
const settlementsByIntentId = new Map();
const settlementBatchesById = new Map();
const settlementBatchReferenceIndex = new Map();
const settlementBatchIdempotencyKeys = new Map();
const settlementBatchSequences = new Map();

function nextSequence(providerId) {
  const next = (providerSequences.get(providerId) || 0) + 1;
  providerSequences.set(providerId, next);
  return next;
}

function nextSettlementBatchSequence(providerId) {
  const next = (settlementBatchSequences.get(providerId) || 0) + 1;
  settlementBatchSequences.set(providerId, next);
  return next;
}

function getIntentOrThrow(intentId) {
  const intent = paymentIntents.get(intentId);
  if (!intent) {
    throw new Error(`Payment intent "${intentId}" not found`);
  }
  return intent;
}

function remainingCaptureAmount(intent) {
  return moneyToNumber(intent.amount) - moneyToNumber(intent.capturedAmount);
}

function remainingRefundableAmount(intent) {
  return moneyToNumber(intent.capturedAmount) - moneyToNumber(intent.refundedAmount);
}

function settledAmountForIntent(intentId) {
  const settlementIds = settlementsByIntentId.get(intentId) || [];
  let settled = 0;
  for (const settlementId of settlementIds) {
    const settlement = settlementsById.get(settlementId);
    if (!settlement) continue;
    if (['paid', 'settled', 'posted'].includes(settlement.status)) {
      settled = roundMoney(settled + moneyToNumber(settlement.amount));
    }
  }
  return settled;
}

function remainingSettleableAmount(intent) {
  const netCaptured = roundMoney(
    moneyToNumber(intent.capturedAmount) - moneyToNumber(intent.refundedAmount),
  );
  return roundMoney(netCaptured - settledAmountForIntent(intent.id));
}

function appendIntentOperation(intent, operation) {
  intent.operations.push(operation);
  intent.updatedAt = operation.createdAt;
}

function indexSettlement(settlement) {
  settlementsById.set(settlement.id, settlement);
  if (!settlementsByIntentId.has(settlement.intentId)) {
    settlementsByIntentId.set(settlement.intentId, []);
  }
  settlementsByIntentId.get(settlement.intentId).push(settlement.id);
}

function normalizeEventType(eventType) {
  return String(eventType || '')
    .trim()
    .toLowerCase();
}

function deriveExternalIntentId(payload = {}) {
  const candidate =
    payload.providerIntentId ||
    payload.paymentIntentId ||
    payload.payment_intent ||
    payload.intent_id ||
    payload.data?.object?.id ||
    payload.data?.id ||
    payload.object?.id ||
    payload.id ||
    null;
  return candidate ? String(candidate) : null;
}

function resolveIntentFromPayload(providerId, payload = {}) {
  if (payload.intentId && paymentIntents.has(payload.intentId)) {
    return paymentIntents.get(payload.intentId);
  }

  const externalIntentId = deriveExternalIntentId(payload);
  if (!externalIntentId) {
    return null;
  }
  return findIntentByProviderIntentId(providerId, externalIntentId);
}

function derivePayoutReference(payload = {}) {
  const candidate =
    payload.payoutReference ||
    payload.payout_reference ||
    payload.payoutId ||
    payload.payout_id ||
    payload.balanceTransactionId ||
    payload.balance_transaction_id ||
    payload.reference ||
    payload.id ||
    null;
  return candidate ? String(candidate) : null;
}

function resolveSettlementIntentIds(providerId, payload = {}) {
  const resolvedIds = new Set();

  const explicitIntentIds = Array.isArray(payload.intentIds)
    ? payload.intentIds
    : Array.isArray(payload.intent_ids)
      ? payload.intent_ids
      : [];
  for (const intentId of explicitIntentIds) {
    if (intentId && paymentIntents.has(intentId)) {
      const intent = paymentIntents.get(intentId);
      if (intent.providerId === providerId) {
        resolvedIds.add(intent.id);
      }
    }
  }

  if (payload.intentId && paymentIntents.has(payload.intentId)) {
    const intent = paymentIntents.get(payload.intentId);
    if (intent.providerId === providerId) {
      resolvedIds.add(intent.id);
    }
  }

  const externalCandidates = [];
  for (const key of ['providerIntentId', 'paymentIntentId', 'payment_intent']) {
    if (payload[key]) {
      externalCandidates.push(payload[key]);
    }
  }
  if (Array.isArray(payload.providerIntentIds)) {
    externalCandidates.push(...payload.providerIntentIds);
  }
  if (Array.isArray(payload.paymentIntentIds)) {
    externalCandidates.push(...payload.paymentIntentIds);
  }
  if (Array.isArray(payload.payment_intent_ids)) {
    externalCandidates.push(...payload.payment_intent_ids);
  }

  for (const externalId of externalCandidates) {
    if (!externalId) continue;
    const intent = findIntentByProviderIntentId(providerId, String(externalId));
    if (intent) {
      resolvedIds.add(intent.id);
    }
  }

  return Array.from(resolvedIds);
}

function deriveWebhookEventId(providerId, eventType, eventId, payload = {}) {
  if (eventId) {
    return String(eventId);
  }

  const candidate =
    payload.eventId ||
    payload.event_id ||
    payload.id ||
    payload.data?.id ||
    payload.data?.object?.id ||
    null;

  if (candidate) {
    return `${providerId}:${candidate}`;
  }

  return deterministicId('pwevt', { providerId, eventType, payload });
}

function parseWebhookAmount(payload = {}) {
  const candidates = [
    ['amount', payload.amount],
    ['amount_received', payload.amount_received],
    ['amountReceived', payload.amountReceived],
    ['amount_captured', payload.amount_captured],
    ['amountCaptured', payload.amountCaptured],
    ['amount_refunded', payload.amount_refunded],
    ['amountRefunded', payload.amountRefunded],
    ['amount_minor', payload.amount_minor],
    ['amountMinor', payload.amountMinor],
  ];

  for (const [field, raw] of candidates) {
    if (raw === null || raw === undefined) continue;
    const numeric = Number(raw);
    if (!Number.isFinite(numeric) || numeric <= 0) continue;
    if (
      field.includes('minor') ||
      field.includes('received') ||
      field.includes('captured') ||
      field.includes('refunded')
    ) {
      return numeric / 100;
    }
    return numeric;
  }

  return null;
}

function findIntentByProviderIntentId(providerId, providerIntentId) {
  const intentId = providerIntentIdIndex.get(`${providerId}:${providerIntentId}`);
  if (!intentId) {
    return null;
  }
  return paymentIntents.get(intentId) || null;
}

function buildCaptureObject(intent, amount, idempotencyKey) {
  const sequence = intent.captures.length + 1;
  const createdAt = nowIso();
  const capture = {
    id: deterministicId('cap', { intentId: intent.id, sequence, amount, idempotencyKey }),
    intentId: intent.id,
    providerId: intent.providerId,
    amount: normalizeMoney(amount),
    status: 'succeeded',
    createdAt,
  };
  return capture;
}

function buildRefundObject(intent, amount, reason, idempotencyKey) {
  const sequence = intent.refunds.length + 1;
  const createdAt = nowIso();
  return {
    id: deterministicId('rfd', {
      intentId: intent.id,
      sequence,
      amount,
      reason: reason || null,
      idempotencyKey,
    }),
    intentId: intent.id,
    providerId: intent.providerId,
    amount: normalizeMoney(amount),
    reason: reason || null,
    status: 'succeeded',
    createdAt,
  };
}

export function listPaymentProviders({ capability, mode } = {}) {
  let providers = filterProvidersByCapability(PAYMENT_PROVIDERS, capability);
  if (mode) {
    providers = providers.filter((provider) => provider.mode === mode);
  }
  return providers.map((provider) => clone(provider));
}

export function listPaymentIntents({ providerId, status, orderId, customerId, limit = 100 } = {}) {
  const boundedLimit = Math.min(500, Math.max(1, Math.floor(Number(limit) || 100)));
  let intents = Array.from(paymentIntents.values());
  if (providerId) {
    intents = intents.filter((intent) => intent.providerId === providerId);
  }
  if (status) {
    intents = intents.filter((intent) => intent.status === status);
  }
  if (orderId) {
    intents = intents.filter((intent) => intent.orderId === orderId);
  }
  if (customerId) {
    intents = intents.filter((intent) => intent.customerId === customerId);
  }

  intents.sort((a, b) => String(b.createdAt).localeCompare(String(a.createdAt)));
  return intents.slice(0, boundedLimit).map((intent) => clone(intent));
}

export function listPaymentSettlements({
  providerId,
  status,
  batchId,
  payoutReference,
  intentId,
  orderId,
  customerId,
  limit = 100,
} = {}) {
  const boundedLimit = Math.min(500, Math.max(1, Math.floor(Number(limit) || 100)));
  let settlements = Array.from(settlementsById.values());
  if (providerId) {
    settlements = settlements.filter((settlement) => settlement.providerId === providerId);
  }
  if (status) {
    settlements = settlements.filter((settlement) => settlement.status === status);
  }
  if (batchId) {
    settlements = settlements.filter((settlement) => settlement.batchId === batchId);
  }
  if (payoutReference) {
    settlements = settlements.filter(
      (settlement) => settlement.payoutReference === payoutReference,
    );
  }
  if (intentId) {
    settlements = settlements.filter((settlement) => settlement.intentId === intentId);
  }
  if (orderId) {
    settlements = settlements.filter((settlement) => settlement.orderId === orderId);
  }
  if (customerId) {
    settlements = settlements.filter((settlement) => settlement.customerId === customerId);
  }

  settlements.sort((a, b) =>
    String(b.settledAt || b.createdAt).localeCompare(String(a.settledAt || a.createdAt)),
  );
  return settlements.slice(0, boundedLimit).map((settlement) => clone(settlement));
}

export function listPaymentSettlementBatches({
  providerId,
  status,
  payoutReference,
  limit = 100,
} = {}) {
  const boundedLimit = Math.min(500, Math.max(1, Math.floor(Number(limit) || 100)));
  let batches = Array.from(settlementBatchesById.values());
  if (providerId) {
    batches = batches.filter((batch) => batch.providerId === providerId);
  }
  if (status) {
    batches = batches.filter((batch) => batch.status === status);
  }
  if (payoutReference) {
    batches = batches.filter((batch) => batch.payoutReference === payoutReference);
  }

  batches.sort((a, b) =>
    String(b.settledAt || b.createdAt).localeCompare(String(a.settledAt || a.createdAt)),
  );
  return batches.slice(0, boundedLimit).map((batch) => clone(batch));
}

function summarizeBatchByCurrency(settlements) {
  const byCurrency = new Map();
  for (const settlement of settlements) {
    if (!byCurrency.has(settlement.currency)) {
      byCurrency.set(settlement.currency, { currency: settlement.currency, amount: 0 });
    }
    const current = byCurrency.get(settlement.currency);
    current.amount = roundMoney(current.amount + moneyToNumber(settlement.amount));
  }
  return Array.from(byCurrency.values()).map((entry) => ({
    currency: entry.currency,
    amount: normalizeMoney(entry.amount),
  }));
}

function findBatchByReference(providerId, payoutReference) {
  const indexedBatchId = settlementBatchReferenceIndex.get(`${providerId}:${payoutReference}`);
  if (!indexedBatchId) {
    return null;
  }
  return settlementBatchesById.get(indexedBatchId) || null;
}

export function createPaymentSettlementBatch({
  providerId = DEFAULT_PAYMENT_PROVIDER,
  intentIds,
  payoutReference,
  settledAt,
  includeZeroBalances = false,
  idempotencyKey,
} = {}) {
  const provider = ensureProvider(PAYMENT_PROVIDERS, providerId, DEFAULT_PAYMENT_PROVIDER);
  const normalizedIntentIds = Array.isArray(intentIds) && intentIds.length > 0 ? intentIds : null;
  const normalizedSettledAt = settledAt ? new Date(settledAt).toISOString() : nowIso();
  const batchIdempotencyKey = idempotencyKey ? `${provider.id}:${idempotencyKey}` : null;

  if (batchIdempotencyKey) {
    const existingBatchId = settlementBatchIdempotencyKeys.get(batchIdempotencyKey);
    if (existingBatchId && settlementBatchesById.has(existingBatchId)) {
      const existingBatch = settlementBatchesById.get(existingBatchId);
      const existingSettlements = (existingBatch.settlementIds || [])
        .map((settlementId) => settlementsById.get(settlementId))
        .filter(Boolean)
        .map((settlement) => clone(settlement));
      return {
        provider: clone(provider),
        batch: clone(existingBatch),
        settlements: existingSettlements,
        count: existingSettlements.length,
        idempotent: true,
      };
    }
  }

  let eligibleIntents = Array.from(paymentIntents.values()).filter(
    (intent) => intent.providerId === provider.id,
  );
  if (normalizedIntentIds) {
    const allowed = new Set(normalizedIntentIds);
    eligibleIntents = eligibleIntents.filter((intent) => allowed.has(intent.id));
  }
  eligibleIntents.sort((a, b) => String(a.id).localeCompare(String(b.id)));

  const settleable = [];
  for (const intent of eligibleIntents) {
    const remaining = remainingSettleableAmount(intent);
    if (remaining > 0 || includeZeroBalances) {
      settleable.push({ intent, amount: Math.max(remaining, 0) });
    }
  }

  if (settleable.length === 0) {
    const emptyBatch = {
      id: null,
      providerId: provider.id,
      providerName: provider.name,
      payoutReference: payoutReference || null,
      status: 'empty',
      createdAt: normalizedSettledAt,
      settledAt: normalizedSettledAt,
      settlementIds: [],
      totalsByCurrency: [],
      totalSettledAmount: '0.00',
      settlementCount: 0,
      idempotencyKey: idempotencyKey || null,
    };
    return {
      provider: clone(provider),
      batch: emptyBatch,
      settlements: [],
      count: 0,
      idempotent: false,
    };
  }

  const sequence = nextSettlementBatchSequence(provider.id);
  const resolvedPayoutReference =
    payoutReference ||
    `payout_${deterministicId('po', {
      providerId: provider.id,
      sequence,
      intentIds: settleable.map((entry) => entry.intent.id),
    })
      .replace('_', '')
      .slice(0, 18)}`;
  const batchId = deterministicId('stlb', {
    providerId: provider.id,
    sequence,
    payoutReference: resolvedPayoutReference,
    intents: settleable.map((entry) => entry.intent.id),
  });

  const settlements = settleable.map(({ intent, amount }, index) => {
    const settlement = {
      id: deterministicId('stl', {
        batchId,
        intentId: intent.id,
        amount: normalizeMoney(amount),
        sequence: index + 1,
      }),
      batchId,
      providerId: provider.id,
      providerName: provider.name,
      payoutReference: resolvedPayoutReference,
      intentId: intent.id,
      providerIntentId: intent.providerIntentId,
      orderId: intent.orderId,
      customerId: intent.customerId,
      currency: intent.currency,
      amount: normalizeMoney(amount),
      status: 'paid',
      createdAt: normalizedSettledAt,
      settledAt: normalizedSettledAt,
    };
    indexSettlement(settlement);
    appendIntentOperation(intent, {
      id: deterministicId('op', {
        intentId: intent.id,
        type: 'settlement',
        batchId,
        sequence: intent.operations.length + 1,
      }),
      type: 'settlement',
      settlementId: settlement.id,
      batchId,
      amount: settlement.amount,
      payoutReference: resolvedPayoutReference,
      createdAt: normalizedSettledAt,
    });
    return settlement;
  });

  const totalSettledAmount = settlements.reduce(
    (sum, settlement) => roundMoney(sum + moneyToNumber(settlement.amount)),
    0,
  );
  const batch = {
    id: batchId,
    providerId: provider.id,
    providerName: provider.name,
    payoutReference: resolvedPayoutReference,
    status: 'paid',
    createdAt: normalizedSettledAt,
    settledAt: normalizedSettledAt,
    settlementIds: settlements.map((settlement) => settlement.id),
    totalsByCurrency: summarizeBatchByCurrency(settlements),
    totalSettledAmount: normalizeMoney(totalSettledAmount),
    settlementCount: settlements.length,
    idempotencyKey: idempotencyKey || null,
  };

  settlementBatchesById.set(batch.id, batch);
  settlementBatchReferenceIndex.set(`${provider.id}:${resolvedPayoutReference}`, batch.id);
  if (batchIdempotencyKey) {
    settlementBatchIdempotencyKeys.set(batchIdempotencyKey, batch.id);
  }

  return {
    provider: clone(provider),
    batch: clone(batch),
    settlements: settlements.map((settlement) => clone(settlement)),
    count: settlements.length,
    idempotent: false,
  };
}

export function reconcilePaymentProvider({
  providerId,
  status,
  orderId,
  customerId,
  intentId,
  includeBalanced = true,
  limit = 100,
} = {}) {
  const boundedLimit = Math.min(500, Math.max(1, Math.floor(Number(limit) || 100)));
  let intents = Array.from(paymentIntents.values());
  if (providerId) {
    intents = intents.filter((intent) => intent.providerId === providerId);
  }
  if (status) {
    intents = intents.filter((intent) => intent.status === status);
  }
  if (orderId) {
    intents = intents.filter((intent) => intent.orderId === orderId);
  }
  if (customerId) {
    intents = intents.filter((intent) => intent.customerId === customerId);
  }
  if (intentId) {
    intents = intents.filter((intent) => intent.id === intentId);
  }

  const summary = {
    authorizedAmount: 0,
    capturedAmount: 0,
    refundedAmount: 0,
    settledAmount: 0,
    outstandingAmount: 0,
    balancedCount: 0,
    pendingCount: 0,
    overSettledCount: 0,
  };

  const reconciliation = intents
    .sort((a, b) => String(b.createdAt).localeCompare(String(a.createdAt)))
    .slice(0, boundedLimit)
    .map((intent) => {
      const authorized = moneyToNumber(intent.amount);
      const captured = moneyToNumber(intent.capturedAmount);
      const refunded = moneyToNumber(intent.refundedAmount);
      const settled = settledAmountForIntent(intent.id);
      const outstanding = roundMoney(captured - refunded - settled);
      let reconciliationStatus = 'balanced';
      if (outstanding > 0) {
        reconciliationStatus = 'pending_settlement';
      } else if (outstanding < 0) {
        reconciliationStatus = 'over_settled';
      }

      summary.authorizedAmount = roundMoney(summary.authorizedAmount + authorized);
      summary.capturedAmount = roundMoney(summary.capturedAmount + captured);
      summary.refundedAmount = roundMoney(summary.refundedAmount + refunded);
      summary.settledAmount = roundMoney(summary.settledAmount + settled);
      summary.outstandingAmount = roundMoney(summary.outstandingAmount + outstanding);
      if (reconciliationStatus === 'balanced') {
        summary.balancedCount += 1;
      } else if (reconciliationStatus === 'pending_settlement') {
        summary.pendingCount += 1;
      } else {
        summary.overSettledCount += 1;
      }

      return {
        intentId: intent.id,
        providerId: intent.providerId,
        providerIntentId: intent.providerIntentId,
        orderId: intent.orderId,
        customerId: intent.customerId,
        currency: intent.currency,
        intentStatus: intent.status,
        authorizedAmount: normalizeMoney(authorized),
        capturedAmount: normalizeMoney(captured),
        refundedAmount: normalizeMoney(refunded),
        settledAmount: normalizeMoney(settled),
        outstandingAmount: normalizeMoney(outstanding),
        settlementCount: (settlementsByIntentId.get(intent.id) || []).length,
        reconciliationStatus,
        lastUpdatedAt: intent.updatedAt,
      };
    })
    .filter((entry) => includeBalanced || entry.reconciliationStatus !== 'balanced');

  return {
    generatedAt: nowIso(),
    providerId: providerId || null,
    includeBalanced: Boolean(includeBalanced),
    count: reconciliation.length,
    summary: {
      authorizedAmount: normalizeMoney(summary.authorizedAmount),
      capturedAmount: normalizeMoney(summary.capturedAmount),
      refundedAmount: normalizeMoney(summary.refundedAmount),
      settledAmount: normalizeMoney(summary.settledAmount),
      outstandingAmount: normalizeMoney(summary.outstandingAmount),
      balancedCount: summary.balancedCount,
      pendingCount: summary.pendingCount,
      overSettledCount: summary.overSettledCount,
    },
    reconciliation,
  };
}

export function getPaymentIntent(intentId) {
  const intent = paymentIntents.get(intentId);
  return intent ? clone(intent) : null;
}

export function createPaymentIntent({
  providerId = DEFAULT_PAYMENT_PROVIDER,
  amount,
  currency = 'USD',
  captureMethod = 'manual',
  customerId,
  orderId,
  paymentMethodId,
  metadata = {},
  idempotencyKey,
} = {}) {
  if (!['manual', 'automatic'].includes(captureMethod)) {
    throw new Error('captureMethod must be either "manual" or "automatic"');
  }

  const provider = ensureProvider(PAYMENT_PROVIDERS, providerId, DEFAULT_PAYMENT_PROVIDER);
  const amountValue = normalizeMoney(amount);
  const normalizedCurrency = currency.toUpperCase();

  if (idempotencyKey) {
    const idemKey = `${provider.id}:${idempotencyKey}`;
    const existingIntentId = intentIdempotencyKeys.get(idemKey);
    if (existingIntentId) {
      return {
        provider: clone(provider),
        intent: clone(getIntentOrThrow(existingIntentId)),
        idempotent: true,
      };
    }
  }

  const sequence = nextSequence(provider.id);
  const createdAt = nowIso();
  const intentId = deterministicId('pi', {
    providerId: provider.id,
    sequence,
    amount: amountValue,
    currency: normalizedCurrency,
    captureMethod,
    customerId: customerId || null,
    orderId: orderId || null,
    paymentMethodId: paymentMethodId || null,
  });
  const providerIntentId = deterministicId('extpi', { providerId: provider.id, intentId });
  const status = captureMethod === 'automatic' ? 'succeeded' : 'requires_capture';
  const capturedAmount = captureMethod === 'automatic' ? amountValue : '0.00';

  const intent = {
    id: intentId,
    providerIntentId,
    providerId: provider.id,
    providerName: provider.name,
    status,
    amount: amountValue,
    capturedAmount,
    refundedAmount: '0.00',
    currency: normalizedCurrency,
    captureMethod,
    customerId: customerId || null,
    orderId: orderId || null,
    paymentMethodId: paymentMethodId || null,
    metadata: metadata || {},
    idempotencyKey: idempotencyKey || null,
    createdAt,
    updatedAt: createdAt,
    operations: [],
    captures: [],
    refunds: [],
  };

  appendIntentOperation(intent, {
    id: deterministicId('op', { intentId, type: 'create', sequence: 1 }),
    type: 'create',
    createdAt,
    metadata: { captureMethod },
  });

  if (captureMethod === 'automatic') {
    const autoCapture = buildCaptureObject(intent, amountValue, idempotencyKey || null);
    capturesById.set(autoCapture.id, autoCapture);
    intent.captures.push(autoCapture);
    appendIntentOperation(intent, {
      id: deterministicId('op', { intentId, type: 'capture', sequence: 2 }),
      type: 'capture',
      captureId: autoCapture.id,
      amount: autoCapture.amount,
      createdAt,
    });
  }

  paymentIntents.set(intentId, intent);
  providerIntentIdIndex.set(`${provider.id}:${providerIntentId}`, intentId);

  if (idempotencyKey) {
    intentIdempotencyKeys.set(`${provider.id}:${idempotencyKey}`, intentId);
  }

  return {
    provider: clone(provider),
    intent: clone(intent),
    idempotent: false,
  };
}

export function ingestPaymentProviderWebhook({
  providerId = DEFAULT_PAYMENT_PROVIDER,
  eventType,
  eventId,
  payload = {},
} = {}) {
  if (!eventType) {
    throw new Error('eventType is required');
  }

  const provider = ensureProvider(PAYMENT_PROVIDERS, providerId, DEFAULT_PAYMENT_PROVIDER);
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

  let intent = resolveIntentFromPayload(provider.id, payload);
  const createdAt = nowIso();
  let action = 'ignored';
  let applied = false;
  let capture = null;
  let refund = null;
  let batch = null;
  let settlements = [];
  let reason = null;
  const affectedIntents = new Map();
  if (intent) {
    affectedIntents.set(intent.id, intent);
  }

  if (
    [
      'payment_intent.succeeded',
      'charge.succeeded',
      'payment.captured',
      'checkout.completed',
    ].includes(normalizedEventType)
  ) {
    if (!intent) {
      const missingIntentResult = {
        provider: clone(provider),
        eventType: normalizedEventType,
        eventId: resolvedEventId,
        action: 'ignored',
        applied: false,
        reason: 'intent_not_found',
        intent: null,
        batch: null,
        settlements: [],
        idempotent: false,
      };
      processedWebhookEvents.set(webhookKey, clone(missingIntentResult));
      return missingIntentResult;
    }

    const remaining = remainingCaptureAmount(intent);
    if (remaining > 0) {
      const requestedAmount = parseWebhookAmount(payload);
      const captureAmount =
        requestedAmount === null || requestedAmount === undefined
          ? remaining
          : Math.min(remaining, requestedAmount);
      const captureResult = capturePaymentIntent({
        intentId: intent.id,
        amount: captureAmount,
        idempotencyKey: `webhook:${webhookKey}`,
      });
      capture = captureResult.capture;
      applied = !captureResult.idempotent;
      action = 'captured';
    } else {
      action = 'already_captured';
      reason = 'intent_already_fully_captured';
    }
  } else if (
    ['payment_intent.payment_failed', 'charge.failed', 'payment.failed'].includes(
      normalizedEventType,
    )
  ) {
    if (!intent) {
      const missingIntentResult = {
        provider: clone(provider),
        eventType: normalizedEventType,
        eventId: resolvedEventId,
        action: 'ignored',
        applied: false,
        reason: 'intent_not_found',
        intent: null,
        batch: null,
        settlements: [],
        idempotent: false,
      };
      processedWebhookEvents.set(webhookKey, clone(missingIntentResult));
      return missingIntentResult;
    }

    intent.status = 'requires_payment_method';
    applied = true;
    action = 'marked_failed';
  } else if (
    ['payment_intent.canceled', 'payment_intent.cancelled', 'payment.canceled'].includes(
      normalizedEventType,
    )
  ) {
    if (!intent) {
      const missingIntentResult = {
        provider: clone(provider),
        eventType: normalizedEventType,
        eventId: resolvedEventId,
        action: 'ignored',
        applied: false,
        reason: 'intent_not_found',
        intent: null,
        batch: null,
        settlements: [],
        idempotent: false,
      };
      processedWebhookEvents.set(webhookKey, clone(missingIntentResult));
      return missingIntentResult;
    }

    if (moneyToNumber(intent.capturedAmount) > 0) {
      action = 'ignored';
      reason = 'cannot_cancel_after_capture';
    } else {
      const cancelResult = cancelPaymentIntent({
        intentId: intent.id,
        reason: payload.reason || payload.failure_message || 'provider_webhook_canceled',
      });
      applied = !cancelResult.idempotent;
      action = 'canceled';
    }
  } else if (
    ['charge.refunded', 'refund.succeeded', 'payment.refunded'].includes(normalizedEventType)
  ) {
    if (!intent) {
      const missingIntentResult = {
        provider: clone(provider),
        eventType: normalizedEventType,
        eventId: resolvedEventId,
        action: 'ignored',
        applied: false,
        reason: 'intent_not_found',
        intent: null,
        batch: null,
        settlements: [],
        idempotent: false,
      };
      processedWebhookEvents.set(webhookKey, clone(missingIntentResult));
      return missingIntentResult;
    }

    const refundable = remainingRefundableAmount(intent);
    if (refundable <= 0) {
      action = 'already_refunded';
      reason = 'intent_has_no_refundable_balance';
    } else {
      const requestedAmount = parseWebhookAmount(payload);
      const refundAmount =
        requestedAmount === null || requestedAmount === undefined
          ? refundable
          : Math.min(refundable, requestedAmount);
      const refundResult = refundPaymentIntent({
        intentId: intent.id,
        amount: refundAmount,
        reason: payload.reason || payload.failure_message || null,
        idempotencyKey: `webhook:${webhookKey}`,
      });
      refund = refundResult.refund;
      applied = !refundResult.idempotent;
      action = 'refunded';
    }
  } else if (
    ['payout.created', 'payout.paid', 'payment.settled', 'balance.available'].includes(
      normalizedEventType,
    )
  ) {
    const resolvedIntentIds = resolveSettlementIntentIds(provider.id, payload);
    const settlementResult = createPaymentSettlementBatch({
      providerId: provider.id,
      intentIds: resolvedIntentIds.length > 0 ? resolvedIntentIds : undefined,
      payoutReference: derivePayoutReference(payload),
      settledAt: payload.settledAt || payload.settled_at || payload.created || null,
      idempotencyKey: `webhook:${webhookKey}`,
    });
    batch = settlementResult.batch;
    settlements = settlementResult.settlements;

    if (settlementResult.count > 0) {
      action = 'settled';
      applied = !settlementResult.idempotent;
      for (const settlement of settlementResult.settlements) {
        const settlementIntent = paymentIntents.get(settlement.intentId);
        if (settlementIntent) {
          affectedIntents.set(settlementIntent.id, settlementIntent);
        }
      }
      if (!intent && settlementResult.settlements.length > 0) {
        const fallbackIntent = paymentIntents.get(settlementResult.settlements[0].intentId);
        if (fallbackIntent) {
          intent = fallbackIntent;
        }
      }
    } else {
      action = 'no_settleable_balance';
      reason = 'no_settleable_balance';
    }
  } else if (
    ['payout.failed', 'payout.canceled', 'payout.cancelled'].includes(normalizedEventType)
  ) {
    const payoutReference = derivePayoutReference(payload);
    if (!payoutReference) {
      reason = 'payout_reference_missing';
    } else {
      const existingBatch = findBatchByReference(provider.id, payoutReference);
      if (!existingBatch) {
        reason = 'settlement_batch_not_found';
      } else if (existingBatch.status === 'failed') {
        action = 'settlement_already_failed';
        reason = 'status_unchanged';
        batch = existingBatch;
      } else {
        const failureAt = nowIso();
        existingBatch.status = 'failed';
        existingBatch.updatedAt = failureAt;
        existingBatch.failureReason = payload.reason || payload.failure_message || null;
        batch = existingBatch;
        settlements = [];
        for (const settlementId of existingBatch.settlementIds || []) {
          const settlement = settlementsById.get(settlementId);
          if (!settlement) continue;
          settlement.status = 'failed';
          settlement.updatedAt = failureAt;
          settlement.failureReason = payload.reason || payload.failure_message || null;
          settlements.push(clone(settlement));
          const settlementIntent = paymentIntents.get(settlement.intentId);
          if (settlementIntent) {
            affectedIntents.set(settlementIntent.id, settlementIntent);
          }
        }
        action = 'settlement_failed';
        applied = true;
      }
    }
  } else {
    action = 'ignored';
    reason = 'unsupported_event_type';
  }

  for (const affectedIntent of affectedIntents.values()) {
    appendIntentOperation(affectedIntent, {
      id: deterministicId('op', {
        intentId: affectedIntent.id,
        type: 'webhook',
        eventId: resolvedEventId,
        sequence: affectedIntent.operations.length + 1,
      }),
      type: 'webhook',
      eventType: normalizedEventType,
      eventId: resolvedEventId,
      action,
      createdAt,
    });
  }

  const result = {
    provider: clone(provider),
    eventType: normalizedEventType,
    eventId: resolvedEventId,
    action,
    applied,
    reason,
    intent: clone(intent),
    capture: capture ? clone(capture) : null,
    refund: refund ? clone(refund) : null,
    batch: batch ? clone(batch) : null,
    settlements: settlements.map((settlement) => clone(settlement)),
    idempotent: false,
  };

  processedWebhookEvents.set(webhookKey, clone(result));
  return result;
}

export function capturePaymentIntent({ intentId, amount, idempotencyKey } = {}) {
  const intent = getIntentOrThrow(intentId);

  if (intent.status === 'canceled') {
    throw new Error(`Payment intent "${intentId}" was canceled and cannot be captured`);
  }

  if (intent.status === 'refunded') {
    throw new Error(`Payment intent "${intentId}" was fully refunded and cannot be captured`);
  }

  if (idempotencyKey) {
    const existingCaptureId = captureIdempotencyKeys.get(`${intentId}:${idempotencyKey}`);
    if (existingCaptureId) {
      const existingCapture = capturesById.get(existingCaptureId);
      return {
        intent: clone(intent),
        capture: clone(existingCapture),
        idempotent: true,
      };
    }
  }

  const remaining = remainingCaptureAmount(intent);
  if (remaining <= 0) {
    return { intent: clone(intent), capture: null, idempotent: true };
  }

  const requestedAmount =
    amount === null || amount === undefined ? remaining : moneyToNumber(amount);
  if (requestedAmount <= 0) {
    throw new Error('Capture amount must be positive');
  }
  if (requestedAmount > remaining) {
    throw new Error(`Capture amount ${requestedAmount} exceeds remaining capturable ${remaining}`);
  }

  const capture = buildCaptureObject(intent, requestedAmount, idempotencyKey || null);
  capturesById.set(capture.id, capture);
  intent.captures.push(capture);

  const newCaptured = moneyToNumber(intent.capturedAmount) + requestedAmount;
  intent.capturedAmount = normalizeMoney(newCaptured);
  intent.status = newCaptured >= moneyToNumber(intent.amount) ? 'succeeded' : 'partially_captured';

  appendIntentOperation(intent, {
    id: deterministicId('op', {
      intentId: intent.id,
      type: 'capture',
      sequence: intent.operations.length + 1,
    }),
    type: 'capture',
    captureId: capture.id,
    amount: capture.amount,
    createdAt: capture.createdAt,
  });

  if (idempotencyKey) {
    captureIdempotencyKeys.set(`${intentId}:${idempotencyKey}`, capture.id);
  }

  return {
    intent: clone(intent),
    capture: clone(capture),
    idempotent: false,
  };
}

export function cancelPaymentIntent({ intentId, reason } = {}) {
  const intent = getIntentOrThrow(intentId);

  if (intent.status === 'canceled') {
    return { intent: clone(intent), idempotent: true };
  }

  if (moneyToNumber(intent.capturedAmount) > 0) {
    throw new Error(`Payment intent "${intentId}" has captured funds and cannot be canceled`);
  }

  const createdAt = nowIso();
  intent.status = 'canceled';
  intent.cancellationReason = reason || null;
  appendIntentOperation(intent, {
    id: deterministicId('op', {
      intentId: intent.id,
      type: 'cancel',
      sequence: intent.operations.length + 1,
    }),
    type: 'cancel',
    reason: reason || null,
    createdAt,
  });

  return {
    intent: clone(intent),
    idempotent: false,
  };
}

export function refundPaymentIntent({ intentId, amount, reason, idempotencyKey } = {}) {
  const intent = getIntentOrThrow(intentId);
  const refundable = remainingRefundableAmount(intent);

  if (refundable <= 0) {
    throw new Error(`Payment intent "${intentId}" has no refundable balance`);
  }

  if (idempotencyKey) {
    const existingRefundId = refundIdempotencyKeys.get(`${intentId}:${idempotencyKey}`);
    if (existingRefundId) {
      return {
        intent: clone(intent),
        refund: clone(refundsById.get(existingRefundId)),
        idempotent: true,
      };
    }
  }

  const requestedAmount =
    amount === null || amount === undefined ? refundable : moneyToNumber(amount);
  if (requestedAmount <= 0) {
    throw new Error('Refund amount must be positive');
  }
  if (requestedAmount > refundable) {
    throw new Error(`Refund amount ${requestedAmount} exceeds remaining refundable ${refundable}`);
  }

  const refund = buildRefundObject(intent, requestedAmount, reason, idempotencyKey);
  refundsById.set(refund.id, refund);
  intent.refunds.push(refund);

  const newRefunded = moneyToNumber(intent.refundedAmount) + requestedAmount;
  intent.refundedAmount = normalizeMoney(newRefunded);

  const capturedAmount = moneyToNumber(intent.capturedAmount);
  intent.status = newRefunded >= capturedAmount ? 'refunded' : 'partially_refunded';

  appendIntentOperation(intent, {
    id: deterministicId('op', {
      intentId: intent.id,
      type: 'refund',
      sequence: intent.operations.length + 1,
    }),
    type: 'refund',
    refundId: refund.id,
    amount: refund.amount,
    reason: refund.reason,
    createdAt: refund.createdAt,
  });

  if (idempotencyKey) {
    refundIdempotencyKeys.set(`${intentId}:${idempotencyKey}`, refund.id);
  }

  return {
    intent: clone(intent),
    refund: clone(refund),
    idempotent: false,
  };
}

export function __resetPaymentProviderState() {
  paymentIntents.clear();
  capturesById.clear();
  refundsById.clear();
  providerSequences.clear();
  intentIdempotencyKeys.clear();
  captureIdempotencyKeys.clear();
  refundIdempotencyKeys.clear();
  providerIntentIdIndex.clear();
  processedWebhookEvents.clear();
  settlementsById.clear();
  settlementsByIntentId.clear();
  settlementBatchesById.clear();
  settlementBatchReferenceIndex.clear();
  settlementBatchIdempotencyKeys.clear();
  settlementBatchSequences.clear();
}

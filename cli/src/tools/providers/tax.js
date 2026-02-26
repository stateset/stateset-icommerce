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

const DEFAULT_TAX_PROVIDER = 'deterministic-mock';

const TAX_PROVIDERS = Object.freeze([
  {
    id: 'deterministic-mock',
    name: 'Deterministic Mock Tax Engine',
    mode: 'sandbox',
    status: 'active',
    supportedCountries: ['US', 'CA', 'GB', 'DE', 'FR', 'AU', 'NZ'],
    capabilities: ['quote', 'commit', 'void', 'exemptions', 'replay'],
  },
  {
    id: 'avalara',
    name: 'Avalara Interop Adapter',
    mode: 'shadow',
    status: 'shadow',
    supportedCountries: ['US', 'CA', 'GB', 'DE', 'FR'],
    capabilities: ['quote', 'commit', 'void', 'exemptions'],
  },
  {
    id: 'taxjar',
    name: 'TaxJar Interop Adapter',
    mode: 'shadow',
    status: 'shadow',
    supportedCountries: ['US'],
    capabilities: ['quote', 'commit', 'void'],
  },
]);

const quotesById = new Map();
const transactionsById = new Map();
const providerSequences = new Map();
const quoteIdempotencyKeys = new Map();
const commitIdempotencyKeys = new Map();
const voidIdempotencyKeys = new Map();
const transactionReferenceIndex = new Map();
const processedWebhookEvents = new Map();

const US_STATE_BASE_RATE = {
  CA: 0.0725,
  TX: 0.0625,
  NY: 0.04,
  WA: 0.065,
  FL: 0.06,
  OR: 0,
  DE: 0,
  MT: 0,
  NH: 0,
  AK: 0,
};

const COUNTRY_BASE_RATE = {
  US: 0.05,
  CA: 0.05,
  GB: 0.2,
  DE: 0.19,
  FR: 0.2,
  AU: 0.1,
  NZ: 0.15,
};

const SUPPORTED_TAX_CATEGORIES = new Set([
  'standard',
  'reduced',
  'exempt',
  'digital',
  'food',
  'medical',
]);

const COUNTRIES_REQUIRING_SUBDIVISION = new Set(['US', 'CA', 'AU']);
const COUNTRIES_REQUIRING_POSTAL_CODE = new Set(['US', 'CA', 'GB', 'DE', 'FR', 'AU', 'NZ']);

const COUNTRY_PROVIDER_FAILOVER_ORDER = Object.freeze({
  US: ['avalara', 'taxjar', 'deterministic-mock'],
  CA: ['avalara', 'deterministic-mock'],
  GB: ['avalara', 'deterministic-mock'],
  DE: ['avalara', 'deterministic-mock'],
  FR: ['avalara', 'deterministic-mock'],
  AU: ['deterministic-mock'],
  NZ: ['deterministic-mock'],
});

function nextSequence(providerId) {
  const next = (providerSequences.get(providerId) || 0) + 1;
  providerSequences.set(providerId, next);
  return next;
}

function normalizeShippingAddress(shippingAddress = {}) {
  return {
    country: String(shippingAddress.country || 'US')
      .trim()
      .toUpperCase(),
    state: shippingAddress.state ? String(shippingAddress.state).trim().toUpperCase() : null,
    city: shippingAddress.city ? String(shippingAddress.city).trim() : null,
    postalCode: shippingAddress.postalCode ? String(shippingAddress.postalCode).trim() : null,
  };
}

function expectedCurrenciesForCountry(countryCode) {
  const map = {
    US: ['USD'],
    CA: ['CAD', 'USD'],
    GB: ['GBP', 'EUR'],
    DE: ['EUR'],
    FR: ['EUR'],
    AU: ['AUD', 'USD'],
    NZ: ['NZD', 'USD'],
  };
  return map[countryCode] || ['USD'];
}

function isKnownTaxCategory(category) {
  return SUPPORTED_TAX_CATEGORIES.has(String(category || '').toLowerCase());
}

function resolveBaseRate(country, state) {
  const normalizedCountry = (country || 'US').toUpperCase();
  if (normalizedCountry === 'US') {
    const normalizedState = (state || '').toUpperCase();
    if (normalizedState && normalizedState in US_STATE_BASE_RATE) {
      return US_STATE_BASE_RATE[normalizedState];
    }
  }
  return COUNTRY_BASE_RATE[normalizedCountry] ?? 0.05;
}

function resolveCategoryRate(baseRate, taxCategory) {
  if (taxCategory === 'exempt') {
    return 0;
  }
  if (taxCategory === 'reduced' || taxCategory === 'food' || taxCategory === 'medical') {
    return baseRate * 0.5;
  }
  if (taxCategory === 'digital') {
    return baseRate + 0.01;
  }
  return baseRate;
}

function shippingIsTaxable(country, state) {
  const normalizedCountry = (country || '').toUpperCase();
  if (normalizedCountry !== 'US') {
    return true;
  }
  const normalizedState = (state || '').toUpperCase();
  return !['OR', 'DE', 'MT', 'NH'].includes(normalizedState);
}

function getQuoteOrThrow(quoteId) {
  const quote = quotesById.get(quoteId);
  if (!quote) {
    throw new Error(`Tax quote "${quoteId}" not found`);
  }
  return quote;
}

function getTransactionOrThrow(transactionId) {
  const transaction = transactionsById.get(transactionId);
  if (!transaction) {
    throw new Error(`Tax transaction "${transactionId}" not found`);
  }
  return transaction;
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
  return deterministicId('txevt', { providerId, eventType, payload });
}

function parseWebhookMoney(payload = {}, ...fields) {
  for (const field of fields) {
    const value = payload[field];
    if (value === null || value === undefined) continue;
    const numeric = Number(value);
    if (!Number.isFinite(numeric)) continue;
    if (field.toLowerCase().includes('minor') || field.toLowerCase().includes('cents')) {
      return numeric / 100;
    }
    return numeric;
  }
  return null;
}

function resolveWebhookTransaction(payload = {}) {
  const transactionId = payload.transactionId || payload.transaction_id || null;
  if (transactionId && transactionsById.has(transactionId)) {
    return transactionsById.get(transactionId);
  }

  const reference =
    payload.reference || payload.transactionReference || payload.externalReference || null;
  if (reference) {
    const indexedTransactionId = transactionReferenceIndex.get(String(reference));
    if (indexedTransactionId && transactionsById.has(indexedTransactionId)) {
      return transactionsById.get(indexedTransactionId);
    }
  }

  const quoteId = payload.quoteId || payload.quote_id || null;
  if (quoteId && quotesById.has(quoteId)) {
    const quote = quotesById.get(quoteId);
    if (quote.transactionId && transactionsById.has(quote.transactionId)) {
      return transactionsById.get(quote.transactionId);
    }
  }

  return null;
}

export function listTaxProviders({ capability, countryCode, mode } = {}) {
  let providers = filterProvidersByCapability(TAX_PROVIDERS, capability);
  if (countryCode) {
    const normalizedCountry = countryCode.toUpperCase();
    providers = providers.filter((provider) =>
      provider.supportedCountries.includes(normalizedCountry),
    );
  }
  if (mode) {
    providers = providers.filter((provider) => provider.mode === mode);
  }
  return providers.map((provider) => clone(provider));
}

export function evaluateTaxJurisdictionCompliance({
  shippingAddress = {},
  lineItems = [],
  currency = 'USD',
  strict = true,
} = {}) {
  const normalizedAddress = normalizeShippingAddress(shippingAddress);
  const normalizedCountry = normalizedAddress.country;
  const normalizedCurrency = String(currency || 'USD')
    .trim()
    .toUpperCase();

  const errors = [];
  const warnings = [];

  if (!normalizedCountry) {
    errors.push('shippingAddress.country is required');
  }

  if (
    normalizedCountry &&
    COUNTRIES_REQUIRING_SUBDIVISION.has(normalizedCountry) &&
    !normalizedAddress.state
  ) {
    const message = `shippingAddress.state is required for country "${normalizedCountry}"`;
    if (strict) {
      errors.push(message);
    } else {
      warnings.push(message);
    }
  }

  if (
    normalizedCountry &&
    COUNTRIES_REQUIRING_POSTAL_CODE.has(normalizedCountry) &&
    !normalizedAddress.postalCode
  ) {
    const message = `shippingAddress.postalCode is required for country "${normalizedCountry}"`;
    if (strict) {
      errors.push(message);
    } else {
      warnings.push(message);
    }
  }

  if (Array.isArray(lineItems)) {
    for (const [index, lineItem] of lineItems.entries()) {
      const category = String(lineItem?.taxCategory || 'standard').toLowerCase();
      if (!isKnownTaxCategory(category)) {
        warnings.push(
          `lineItems[${index}].taxCategory "${category}" is unknown and will be treated as "standard"`,
        );
      }
    }
  }

  if (normalizedCountry) {
    const expectedCurrencies = expectedCurrenciesForCountry(normalizedCountry);
    if (!expectedCurrencies.includes(normalizedCurrency)) {
      warnings.push(
        `currency "${normalizedCurrency}" is unusual for country "${normalizedCountry}" (expected: ${expectedCurrencies.join(', ')})`,
      );
    }
  }

  return {
    valid: errors.length === 0,
    strict: Boolean(strict),
    errors,
    warnings,
    normalizedAddress,
    normalizedCurrency,
  };
}

export function buildTaxProviderRoutingPlan({
  countryCode = 'US',
  providerId,
  fallbackProviderIds = [],
  allowDeterministicFallback = true,
} = {}) {
  const normalizedCountry = String(countryCode || 'US')
    .trim()
    .toUpperCase();
  const plan = [];

  if (providerId) {
    plan.push(String(providerId));
  } else if (COUNTRY_PROVIDER_FAILOVER_ORDER[normalizedCountry]) {
    plan.push(...COUNTRY_PROVIDER_FAILOVER_ORDER[normalizedCountry]);
  } else {
    plan.push(DEFAULT_TAX_PROVIDER);
  }

  if (Array.isArray(fallbackProviderIds)) {
    for (const candidate of fallbackProviderIds) {
      if (!candidate) continue;
      plan.push(String(candidate));
    }
  }

  if (allowDeterministicFallback) {
    plan.push(DEFAULT_TAX_PROVIDER);
  }

  const seen = new Set();
  const uniquePlan = [];
  for (const provider of plan) {
    if (seen.has(provider)) continue;
    seen.add(provider);
    uniquePlan.push(provider);
  }

  return uniquePlan;
}

export function getTaxQuote(quoteId) {
  const quote = quotesById.get(quoteId);
  return quote ? clone(quote) : null;
}

export function getTaxTransaction(transactionId) {
  const transaction = transactionsById.get(transactionId);
  return transaction ? clone(transaction) : null;
}

export function listTaxTransactions({ providerId, status, quoteId, reference, limit = 100 } = {}) {
  const boundedLimit = Math.min(500, Math.max(1, Math.floor(Number(limit) || 100)));
  let transactions = Array.from(transactionsById.values());
  if (providerId) {
    transactions = transactions.filter((transaction) => transaction.providerId === providerId);
  }
  if (status) {
    transactions = transactions.filter((transaction) => transaction.status === status);
  }
  if (quoteId) {
    transactions = transactions.filter((transaction) => transaction.quoteId === quoteId);
  }
  if (reference) {
    transactions = transactions.filter((transaction) => transaction.reference === reference);
  }
  transactions.sort((a, b) =>
    String(b.committedAt || b.updatedAt).localeCompare(String(a.committedAt || a.updatedAt)),
  );
  return transactions.slice(0, boundedLimit).map((transaction) => clone(transaction));
}

export function calculateTaxQuote({
  providerId = DEFAULT_TAX_PROVIDER,
  lineItems = [],
  shippingAddress = {},
  shippingAmount = 0,
  customerId,
  orderId,
  currency = 'USD',
  taxExempt = false,
  metadata = {},
  idempotencyKey,
} = {}) {
  if (!Array.isArray(lineItems) || lineItems.length === 0) {
    throw new Error('At least one line item is required to calculate a tax quote');
  }

  const provider = ensureProvider(TAX_PROVIDERS, providerId, DEFAULT_TAX_PROVIDER);
  const countryCode = (shippingAddress.country || 'US').toUpperCase();
  const stateCode = (shippingAddress.state || '').toUpperCase();

  if (!provider.supportedCountries.includes(countryCode)) {
    throw new Error(`Provider "${provider.id}" does not support country "${countryCode}"`);
  }

  if (idempotencyKey) {
    const existingQuoteId = quoteIdempotencyKeys.get(`${provider.id}:${idempotencyKey}`);
    if (existingQuoteId) {
      return {
        provider: clone(provider),
        quote: clone(getQuoteOrThrow(existingQuoteId)),
        idempotent: true,
      };
    }
  }

  const baseRate = resolveBaseRate(countryCode, stateCode);
  const normalizedCurrency = currency.toUpperCase();
  let subtotal = 0;
  let totalTax = 0;
  let taxableAmount = 0;

  const itemBreakdown = lineItems.map((item, index) => {
    const quantity = Math.max(1, Math.floor(moneyToNumber(item.quantity)));
    const unitPrice = moneyToNumber(item.unitPrice);
    const lineSubtotal = roundMoney(quantity * unitPrice);
    const category = item.taxCategory || 'standard';
    const itemRate = taxExempt ? 0 : resolveCategoryRate(baseRate, category);
    const lineTax = roundMoney(lineSubtotal * itemRate);

    subtotal = roundMoney(subtotal + lineSubtotal);
    taxableAmount = roundMoney(taxableAmount + lineSubtotal);
    totalTax = roundMoney(totalTax + lineTax);

    return {
      index,
      lineItemId: item.id || `line-${index + 1}`,
      quantity,
      unitPrice: normalizeMoney(unitPrice),
      taxCategory: category,
      taxRate: itemRate,
      taxableAmount: normalizeMoney(lineSubtotal),
      taxAmount: normalizeMoney(lineTax),
    };
  });

  const normalizedShippingAmount = roundMoney(shippingAmount || 0);
  let shippingTax = 0;
  if (!taxExempt && normalizedShippingAmount > 0 && shippingIsTaxable(countryCode, stateCode)) {
    shippingTax = roundMoney(normalizedShippingAmount * baseRate);
    totalTax = roundMoney(totalTax + shippingTax);
    taxableAmount = roundMoney(taxableAmount + normalizedShippingAmount);
  }

  const total = roundMoney(subtotal + normalizedShippingAmount + totalTax);
  const sequence = nextSequence(provider.id);
  const createdAt = nowIso();

  const quoteId = deterministicId('taxq', {
    providerId: provider.id,
    sequence,
    countryCode,
    stateCode: stateCode || null,
    subtotal: normalizeMoney(subtotal),
    totalTax: normalizeMoney(totalTax),
    currency: normalizedCurrency,
  });

  const quote = {
    id: quoteId,
    providerId: provider.id,
    providerName: provider.name,
    status: 'quoted',
    customerId: customerId || null,
    orderId: orderId || null,
    currency: normalizedCurrency,
    shippingAddress: clone(shippingAddress),
    shippingAmount: normalizeMoney(normalizedShippingAmount),
    shippingTax: normalizeMoney(shippingTax),
    subtotal: normalizeMoney(subtotal),
    taxableAmount: normalizeMoney(taxableAmount),
    totalTax: normalizeMoney(totalTax),
    total: normalizeMoney(total),
    lineItems: itemBreakdown,
    taxBreakdown: [
      {
        jurisdiction: stateCode || countryCode,
        taxType: countryCode === 'US' ? 'sales_tax' : 'vat',
        rate: baseRate,
        taxableAmount: normalizeMoney(taxableAmount),
        taxAmount: normalizeMoney(totalTax),
      },
    ],
    metadata: metadata || {},
    idempotencyKey: idempotencyKey || null,
    createdAt,
    updatedAt: createdAt,
  };

  quotesById.set(quote.id, quote);

  if (idempotencyKey) {
    quoteIdempotencyKeys.set(`${provider.id}:${idempotencyKey}`, quote.id);
  }

  return {
    provider: clone(provider),
    quote: clone(quote),
    idempotent: false,
  };
}

export function calculateTaxQuoteWithFailover({
  providerId,
  fallbackProviderIds = [],
  allowDeterministicFallback = true,
  strictCompliance = true,
  lineItems = [],
  shippingAddress = {},
  shippingAmount = 0,
  customerId,
  orderId,
  currency = 'USD',
  taxExempt = false,
  metadata = {},
  idempotencyKey,
} = {}) {
  const compliance = evaluateTaxJurisdictionCompliance({
    shippingAddress,
    lineItems,
    currency,
    strict: strictCompliance,
  });

  if (!compliance.valid) {
    throw new Error(`Tax compliance validation failed: ${compliance.errors.join('; ')}`);
  }

  const routingPlan = buildTaxProviderRoutingPlan({
    countryCode: compliance.normalizedAddress.country,
    providerId,
    fallbackProviderIds,
    allowDeterministicFallback,
  });
  const attempts = [];

  for (const candidateProviderId of routingPlan) {
    const provider = TAX_PROVIDERS.find((entry) => entry.id === candidateProviderId);
    if (!provider) {
      attempts.push({
        providerId: candidateProviderId,
        success: false,
        reason: 'provider_not_found',
      });
      continue;
    }

    if (!provider.capabilities.includes('quote')) {
      attempts.push({
        providerId: candidateProviderId,
        success: false,
        reason: 'capability_missing_quote',
      });
      continue;
    }

    if (!provider.supportedCountries.includes(compliance.normalizedAddress.country)) {
      attempts.push({
        providerId: candidateProviderId,
        success: false,
        reason: `country_not_supported:${compliance.normalizedAddress.country}`,
      });
      continue;
    }

    try {
      const result = calculateTaxQuote({
        providerId: candidateProviderId,
        lineItems,
        shippingAddress: compliance.normalizedAddress,
        shippingAmount,
        customerId,
        orderId,
        currency: compliance.normalizedCurrency,
        taxExempt,
        metadata: {
          ...(metadata || {}),
          compliance: {
            strict: compliance.strict,
            warnings: compliance.warnings,
          },
        },
        idempotencyKey,
      });

      const routedQuote = {
        ...result.quote,
        shippingAddress: compliance.normalizedAddress,
        compliance: {
          strict: compliance.strict,
          errors: [],
          warnings: compliance.warnings,
        },
        routing: {
          primaryProviderId: providerId || null,
          selectedProviderId: candidateProviderId,
          attemptedProviderIds: routingPlan,
          failoverCount: attempts.length,
          degraded: attempts.length > 0,
        },
      };

      quotesById.set(routedQuote.id, clone(routedQuote));
      attempts.push({
        providerId: candidateProviderId,
        success: true,
        quoteId: routedQuote.id,
      });

      return {
        provider: result.provider,
        quote: clone(routedQuote),
        idempotent: result.idempotent,
        failover: {
          attempted: attempts.length > 1,
          routingPlan,
          attempts,
          selectedProviderId: candidateProviderId,
        },
      };
    } catch (error) {
      attempts.push({
        providerId: candidateProviderId,
        success: false,
        reason: error.message,
      });
    }
  }

  const failureSummary = attempts
    .map((attempt) => `${attempt.providerId}:${attempt.reason || 'unknown_failure'}`)
    .join(', ');
  throw new Error(`Tax quote failover exhausted. Attempts: ${failureSummary}`);
}

export function commitTaxTransaction({
  quoteId,
  providerId,
  transactionReference,
  idempotencyKey,
} = {}) {
  const quote = getQuoteOrThrow(quoteId);

  if (providerId && providerId !== quote.providerId) {
    throw new Error(
      `Quote "${quoteId}" belongs to provider "${quote.providerId}" and cannot be committed with "${providerId}"`,
    );
  }

  if (idempotencyKey) {
    const existingTransactionId = commitIdempotencyKeys.get(`${quote.id}:${idempotencyKey}`);
    if (existingTransactionId) {
      return {
        quote: clone(quote),
        transaction: clone(getTransactionOrThrow(existingTransactionId)),
        idempotent: true,
      };
    }
  }

  if (quote.transactionId) {
    return {
      quote: clone(quote),
      transaction: clone(getTransactionOrThrow(quote.transactionId)),
      idempotent: true,
    };
  }

  const sequence = nextSequence(quote.providerId);
  const createdAt = nowIso();
  const transactionId = deterministicId('taxtx', {
    providerId: quote.providerId,
    quoteId: quote.id,
    sequence,
  });

  const transaction = {
    id: transactionId,
    providerId: quote.providerId,
    providerName: quote.providerName,
    quoteId: quote.id,
    status: 'committed',
    reference: transactionReference || quote.orderId || quote.id,
    currency: quote.currency,
    subtotal: quote.subtotal,
    taxableAmount: quote.taxableAmount,
    totalTax: quote.totalTax,
    total: quote.total,
    lineItems: clone(quote.lineItems),
    taxBreakdown: clone(quote.taxBreakdown),
    committedAt: createdAt,
    updatedAt: createdAt,
  };

  transactionsById.set(transaction.id, transaction);
  transactionReferenceIndex.set(String(transaction.reference), transaction.id);
  quote.status = 'committed';
  quote.transactionId = transaction.id;
  quote.updatedAt = createdAt;

  if (idempotencyKey) {
    commitIdempotencyKeys.set(`${quote.id}:${idempotencyKey}`, transaction.id);
  }

  return {
    quote: clone(quote),
    transaction: clone(transaction),
    idempotent: false,
  };
}

export function ingestTaxProviderWebhook({
  providerId = DEFAULT_TAX_PROVIDER,
  eventType,
  eventId,
  payload = {},
} = {}) {
  if (!eventType) {
    throw new Error('eventType is required');
  }

  const provider = ensureProvider(TAX_PROVIDERS, providerId, DEFAULT_TAX_PROVIDER);
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

  let transaction = resolveWebhookTransaction(payload);
  let quote = transaction ? quotesById.get(transaction.quoteId) : null;
  let action = 'ignored';
  let applied = false;
  let reason = null;

  if (
    ['transaction.committed', 'tax.committed', 'tax.transaction.committed'].includes(
      normalizedEventType,
    )
  ) {
    if (!transaction) {
      const quoteId = payload.quoteId || payload.quote_id || null;
      if (!quoteId || !quotesById.has(quoteId)) {
        reason = 'quote_not_found';
      } else {
        const commitResult = commitTaxTransaction({
          quoteId,
          providerId: provider.id,
          transactionReference:
            payload.reference || payload.transactionReference || payload.externalReference,
          idempotencyKey: `webhook:${webhookKey}`,
        });
        quote = quotesById.get(quoteId);
        transaction = transactionsById.get(commitResult.transaction.id);
        action = 'committed';
        applied = !commitResult.idempotent;
      }
    } else if (transaction.status === 'committed') {
      action = 'already_committed';
      reason = 'status_unchanged';
    } else {
      transaction.status = 'committed';
      transaction.updatedAt = nowIso();
      if (quote) {
        quote.status = 'committed';
        quote.updatedAt = transaction.updatedAt;
      }
      action = 'committed';
      applied = true;
    }
  } else if (
    ['transaction.voided', 'tax.voided', 'tax.transaction.voided'].includes(normalizedEventType)
  ) {
    if (!transaction) {
      reason = 'transaction_not_found';
    } else {
      const voidResult = voidTaxTransaction({
        transactionId: transaction.id,
        reason: payload.reason || payload.message || 'provider_webhook_voided',
        idempotencyKey: `webhook:${webhookKey}`,
      });
      quote = quotesById.get(transaction.quoteId);
      transaction = transactionsById.get(voidResult.transaction.id);
      action = 'voided';
      applied = !voidResult.idempotent;
    }
  } else if (
    ['transaction.adjusted', 'tax.adjusted', 'tax.transaction.adjusted'].includes(
      normalizedEventType,
    )
  ) {
    if (!transaction) {
      reason = 'transaction_not_found';
    } else {
      const updatedAt = nowIso();
      const newTotalTax = parseWebhookMoney(
        payload,
        'totalTax',
        'total_tax',
        'total_tax_minor',
        'totalTaxMinor',
      );
      const newTotal = parseWebhookMoney(payload, 'total', 'amount', 'amount_minor', 'amountCents');

      transaction.status = 'adjusted';
      if (newTotalTax !== null && newTotalTax !== undefined) {
        transaction.totalTax = normalizeMoney(newTotalTax);
      }
      if (newTotal !== null && newTotal !== undefined) {
        transaction.total = normalizeMoney(newTotal);
      }
      transaction.adjustmentReason = payload.reason || payload.message || null;
      transaction.updatedAt = updatedAt;

      if (quote) {
        quote.status = 'adjusted';
        if (newTotalTax !== null && newTotalTax !== undefined) {
          quote.totalTax = normalizeMoney(newTotalTax);
        }
        if (newTotal !== null && newTotal !== undefined) {
          quote.total = normalizeMoney(newTotal);
        }
        quote.updatedAt = updatedAt;
      }

      action = 'adjusted';
      applied = true;
    }
  } else {
    reason = 'unsupported_event_type';
  }

  const result = {
    provider: clone(provider),
    eventType: normalizedEventType,
    eventId: resolvedEventId,
    action,
    applied,
    reason,
    quote: quote ? clone(quote) : null,
    transaction: transaction ? clone(transaction) : null,
    idempotent: false,
  };

  processedWebhookEvents.set(webhookKey, clone(result));
  return result;
}

export function voidTaxTransaction({ transactionId, reason, idempotencyKey } = {}) {
  const transaction = getTransactionOrThrow(transactionId);
  const quote = getQuoteOrThrow(transaction.quoteId);

  if (idempotencyKey) {
    const existingVoidTransactionId = voidIdempotencyKeys.get(
      `${transaction.id}:${idempotencyKey}`,
    );
    if (existingVoidTransactionId) {
      return {
        quote: clone(quote),
        transaction: clone(getTransactionOrThrow(existingVoidTransactionId)),
        idempotent: true,
      };
    }
  }

  if (transaction.status === 'voided') {
    return {
      quote: clone(quote),
      transaction: clone(transaction),
      idempotent: true,
    };
  }

  const createdAt = nowIso();
  transaction.status = 'voided';
  transaction.voidReason = reason || null;
  transaction.voidedAt = createdAt;
  transaction.updatedAt = createdAt;

  quote.status = 'voided';
  quote.updatedAt = createdAt;

  if (idempotencyKey) {
    voidIdempotencyKeys.set(`${transaction.id}:${idempotencyKey}`, transaction.id);
  }

  return {
    quote: clone(quote),
    transaction: clone(transaction),
    idempotent: false,
  };
}

export function __resetTaxProviderState() {
  quotesById.clear();
  transactionsById.clear();
  providerSequences.clear();
  quoteIdempotencyKeys.clear();
  commitIdempotencyKeys.clear();
  voidIdempotencyKeys.clear();
  transactionReferenceIndex.clear();
  processedWebhookEvents.clear();
}

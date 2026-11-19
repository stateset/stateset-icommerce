import { merchantRecords, saveMerchantRecord } from './merchant-records.js';
import { getTaxProvider } from './tax.js';
import { getShippingProvider } from './shipping.js';

export async function storePricing(commerce, env) {
  let latest;
  for await (const row of merchantRecords(commerce, 'pricing-revisions')) {
    if (!latest || row.revision > latest.revision) latest = row;
  }
  if (latest) return { revision: latest.revision, rates: latest.rates, methods: latest.methods };
  // The setup form must be able to repair malformed environment settings.
  // Checkout continues to validate the original environment until publication.
  let rates = {},
    methods = [];
  try {
    getTaxProvider(env);
    rates = JSON.parse(env.STATESET_TAX_RATES_JSON || '{}');
  } catch {}
  try {
    getShippingProvider(env);
    methods = JSON.parse(env.STATESET_SHIPPING_METHODS_JSON || '[]');
  } catch {}
  return { revision: 0, rates, methods };
}

export async function pricingEnvironment(commerce, env = process.env) {
  const current = await storePricing(commerce, env);
  if (!current.revision) return env;
  return {
    ...env,
    STATESET_TAX_RATES_JSON: JSON.stringify(current.rates),
    STATESET_SHIPPING_METHODS_JSON: JSON.stringify(current.methods),
  };
}

export async function publishPricing(commerce, env, input, apply = false) {
  if (
    !Number.isSafeInteger(input?.revision) ||
    input.revision < 0 ||
    !input.rates ||
    Array.isArray(input.rates) ||
    typeof input.rates !== 'object' ||
    Object.keys(input.rates).length === 0 ||
    !Object.values(input.rates).every((r) => typeof r === 'string') ||
    !Array.isArray(input.methods) ||
    !input.methods.length ||
    !input.methods.every((m) => typeof m?.amount === 'string')
  ) {
    throw new Error('Provide exact tax rates and shipping amounts');
  }
  const revision = input.revision + 1;
  const { rates, methods } = input;
  getTaxProvider({ STATESET_TAX_RATES_JSON: JSON.stringify(rates) });
  getShippingProvider({ STATESET_SHIPPING_METHODS_JSON: JSON.stringify(methods) });
  const current = await storePricing(commerce, env);
  if (
    current.revision !== input.revision &&
    !(
      current.revision === revision &&
      JSON.stringify([current.rates, current.methods]) === JSON.stringify([rates, methods])
    )
  ) {
    throw new Error('Pricing changed. Reload the current revision');
  }
  if (apply)
    await saveMerchantRecord(
      commerce,
      'pricing-' + revision,
      { revision, rates, methods },
      'pricing-revisions',
    );
  return { revision, rates, methods, mode: apply ? 'published' : 'preview' };
}

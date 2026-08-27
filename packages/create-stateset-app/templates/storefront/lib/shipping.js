import { decimalToUnits } from './money.js';

const US_STATE_CODES = new Set([
  'AL',
  'AK',
  'AZ',
  'AR',
  'CA',
  'CO',
  'CT',
  'DE',
  'FL',
  'GA',
  'HI',
  'ID',
  'IL',
  'IN',
  'IA',
  'KS',
  'KY',
  'LA',
  'ME',
  'MD',
  'MA',
  'MI',
  'MN',
  'MS',
  'MO',
  'MT',
  'NE',
  'NV',
  'NH',
  'NJ',
  'NM',
  'NY',
  'NC',
  'ND',
  'OH',
  'OK',
  'OR',
  'PA',
  'RI',
  'SC',
  'SD',
  'TN',
  'TX',
  'UT',
  'VT',
  'VA',
  'WA',
  'WV',
  'WI',
  'WY',
  'DC',
]);

export const STARTER_SHIPPING_METHODS = Object.freeze([
  Object.freeze({
    id: 'standard',
    label: 'Standard shipping',
    amount: '0',
    carrier: 'merchant',
    estimatedDays: '3-5 business days',
    countries: Object.freeze(['US']),
  }),
]);

export function validateShippingAddress(address) {
  if (!address || typeof address !== 'object') throw new Error('shippingAddress is required');
  const required = ['firstName', 'lastName', 'line1', 'city', 'state', 'postalCode', 'country'];
  for (const field of required) {
    if (!String(address[field] || '').trim())
      throw new Error(`shippingAddress.${field} is required`);
  }
  const normalized = {
    firstName: String(address.firstName).trim(),
    lastName: String(address.lastName).trim(),
    line1: String(address.line1).trim(),
    line2: String(address.line2 || '').trim() || undefined,
    city: String(address.city).trim(),
    state: String(address.state).trim().toUpperCase(),
    postalCode: String(address.postalCode).trim(),
    country: String(address.country).trim().toUpperCase(),
  };
  if (normalized.country !== 'US')
    throw new Error('Shipping is currently configured for US addresses');
  if (!US_STATE_CODES.has(normalized.state)) throw new Error('shippingAddress.state is invalid');
  if (!/^\d{5}(-\d{4})?$/.test(normalized.postalCode)) {
    throw new Error('shippingAddress.postalCode must be a valid US ZIP code');
  }
  return normalized;
}

function normalizeMethods(methods) {
  if (!Array.isArray(methods) || methods.length === 0) {
    throw new Error('Shipping methods must be a non-empty array');
  }
  const ids = new Set();
  return Object.freeze(
    methods.map((method) => {
      const id = String(method.id || '')
        .trim()
        .toLowerCase();
      if (!/^[a-z0-9][a-z0-9_-]{0,63}$/.test(id)) throw new Error('Invalid shipping method id');
      if (ids.has(id)) throw new Error(`Duplicate shipping method id: ${id}`);
      ids.add(id);
      const label = String(method.label || '').trim();
      if (!label) throw new Error(`Shipping method ${id} requires a label`);
      const amount = String(method.amount ?? '').trim();
      decimalToUnits(amount, 6);
      const countries = (method.countries || ['US']).map((country) =>
        String(country).trim().toUpperCase(),
      );
      return Object.freeze({
        id,
        label,
        amount,
        carrier: String(method.carrier || 'merchant').trim(),
        estimatedDays: String(method.estimatedDays || '').trim() || undefined,
        countries: Object.freeze(countries),
      });
    }),
  );
}

export function createShippingProvider(methods, source = 'operator') {
  const normalizedMethods = normalizeMethods(methods);
  const publicMethod = ({ countries: _countries, ...method }) => method;
  return Object.freeze({
    name: 'rate-table',
    source,
    listMethods(address) {
      const normalizedAddress = validateShippingAddress(address);
      return normalizedMethods
        .filter((method) => method.countries.includes(normalizedAddress.country))
        .map(publicMethod);
    },
    quote(address, methodId) {
      const available = this.listMethods(address);
      if (available.length === 0) throw new Error('No shipping methods serve this address');
      const selectedId = String(methodId || available[0].id)
        .trim()
        .toLowerCase();
      const selected = available.find((method) => method.id === selectedId);
      if (!selected) throw new Error(`Shipping method is not available: ${selectedId}`);
      return selected;
    },
  });
}

export function getShippingProvider(
  environment = typeof process === 'undefined' ? {} : process.env,
) {
  const configuredMethods = environment.STATESET_SHIPPING_METHODS_JSON;
  if (!configuredMethods) return createShippingProvider(STARTER_SHIPPING_METHODS, 'starter');
  let parsed;
  try {
    parsed = JSON.parse(configuredMethods);
  } catch {
    throw new Error('STATESET_SHIPPING_METHODS_JSON must be valid JSON');
  }
  return createShippingProvider(parsed, 'environment');
}

import { decimalToUnits, unitsToDecimal } from './money.js';

// Starter rates are intentionally explicit. Production stores should replace
// this table with an operator-owned tax service and nexus configuration.
export const STARTER_TAX_RATES = Object.freeze({
  CA: '0.0725',
  NY: '0.08',
  TX: '0.0625',
  FL: '0.06',
  WA: '0.065',
  IL: '0.0625',
  PA: '0.06',
  OH: '0.0575',
  GA: '0.04',
  NC: '0.0475',
  NJ: '0.06625',
  VA: '0.053',
  MI: '0.06',
  AZ: '0.056',
  MA: '0.0625',
});

function normalizeRateTable(rates) {
  if (!rates || typeof rates !== 'object' || Array.isArray(rates)) {
    throw new Error('Tax rates must be an object keyed by two-letter jurisdiction code');
  }
  return Object.freeze(
    Object.fromEntries(
      Object.entries(rates).map(([jurisdiction, rate]) => {
        const code = String(jurisdiction).trim().toUpperCase();
        if (!/^[A-Z]{2}$/.test(code)) throw new Error(`Invalid tax jurisdiction: ${jurisdiction}`);
        const exactRate = String(rate).trim();
        const rateUnits = decimalToUnits(exactRate, 6);
        if (rateUnits < 0n || rateUnits > 1000000n) {
          throw new Error(`Tax rate for ${code} must be between 0 and 1`);
        }
        return [code, exactRate];
      }),
    ),
  );
}

function calculateCartWithRates(items, stateCode, rates, decimals = 6) {
  const code = String(stateCode || '').toUpperCase();
  if (!Object.prototype.hasOwnProperty.call(rates, code)) {
    throw new Error(`Tax is not configured for ${code || 'the shipping jurisdiction'}`);
  }
  const rate = rates[code];
  const rateUnits = decimalToUnits(rate, 6);
  let subtotalUnits = 0n;
  let taxUnits = 0n;
  const lines = items.map((item) => {
    const lineUnits = decimalToUnits(item.unitPrice, decimals) * BigInt(item.quantity);
    const lineTaxUnits = (lineUnits * rateUnits + 500000n) / 1000000n;
    subtotalUnits += lineUnits;
    taxUnits += lineTaxUnits;
    return { ...item, tax: unitsToDecimal(lineTaxUnits, decimals) };
  });
  return {
    rate,
    subtotal: unitsToDecimal(subtotalUnits, decimals),
    tax: unitsToDecimal(taxUnits, decimals),
    total: unitsToDecimal(subtotalUnits + taxUnits, decimals),
    lines,
  };
}

export function createRateTableTaxProvider(rates, source = 'operator') {
  const normalizedRates = normalizeRateTable(rates);
  return Object.freeze({
    name: 'rate-table',
    source,
    hasJurisdiction(stateCode) {
      return Object.prototype.hasOwnProperty.call(
        normalizedRates,
        String(stateCode || '').toUpperCase(),
      );
    },
    calculateCart(items, stateCode, decimals = 6) {
      return calculateCartWithRates(items, stateCode, normalizedRates, decimals);
    },
  });
}

export function getTaxProvider(environment = typeof process === 'undefined' ? {} : process.env) {
  const configuredRates = environment.STATESET_TAX_RATES_JSON;
  if (!configuredRates) return createRateTableTaxProvider(STARTER_TAX_RATES, 'starter');
  let parsed;
  try {
    parsed = JSON.parse(configuredRates);
  } catch {
    throw new Error('STATESET_TAX_RATES_JSON must be valid JSON');
  }
  return createRateTableTaxProvider(parsed, 'environment');
}

export function hasConfiguredTaxRate(stateCode) {
  return createRateTableTaxProvider(STARTER_TAX_RATES, 'starter').hasJurisdiction(stateCode);
}

export function calculateTax(subtotal, stateCode, decimals = 6) {
  const rate = STARTER_TAX_RATES[String(stateCode || '').toUpperCase()] || '0';
  const subtotalUnits = decimalToUnits(subtotal, decimals);
  const rateUnits = decimalToUnits(rate, 6);
  const taxUnits = (subtotalUnits * rateUnits + 500000n) / 1000000n;
  return {
    rate,
    tax: unitsToDecimal(taxUnits, decimals),
    total: unitsToDecimal(subtotalUnits + taxUnits, decimals),
  };
}

export function calculateCartTax(items, stateCode, decimals = 6) {
  return createRateTableTaxProvider(STARTER_TAX_RATES, 'starter').calculateCart(
    items,
    stateCode,
    decimals,
  );
}

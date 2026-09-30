import BigNumber from 'bignumber.js';

// Isolated configuration: unrelated consumers must not change provider rounding.
const Decimal = BigNumber.clone({ DECIMAL_PLACES: 36, ROUNDING_MODE: BigNumber.ROUND_HALF_UP });
const TWO_DECIMAL_CURRENCIES = new Set(
  'USD EUR GBP CAD AUD CHF CNY HKD SGD SEK NOK DKK NZD MXN INR BRL ZAR RUB TRY PLN THB IDR MYR PHP CZK ILS AED SAR TWD'.split(
    ' ',
  ),
);
const ZERO_DECIMAL_CURRENCIES = new Set(
  'JPY KRW VND BIF CLP DJF GNF KMF PYG RWF UGX VUV XAF XOF XPF'.split(' '),
);
const THREE_DECIMAL_CURRENCIES = new Set('BHD IQD JOD KWD LYD OMR TND'.split(' '));

export function currencyCode(currency = 'USD') {
  if (typeof currency !== 'string') throw new Error('Currency must be a supported fiat code');
  const code = currency.toUpperCase();
  currencyScale(code);
  return code;
}

export function currencyScale(currency = 'USD') {
  const code = typeof currency === 'string' ? currency.toUpperCase() : '';
  if (TWO_DECIMAL_CURRENCIES.has(code)) return 2;
  if (ZERO_DECIMAL_CURRENCIES.has(code)) return 0;
  if (THREE_DECIMAL_CURRENCIES.has(code)) return 3;
  // Token symbols do not identify a chain, contract, or denomination.
  throw new Error(
    `Unsupported provider currency "${currency}"; an explicit fiat denomination is required`,
  );
}

export function decimal(value) {
  if (value instanceof Decimal) return value;
  // Compatibility for existing numeric tool callers only. All arithmetic and
  // results use decimals; unsafe numeric inputs cannot be repaired by conversion.
  if (
    typeof value === 'number' &&
    (!Number.isFinite(value) || Math.abs(value) > Number.MAX_SAFE_INTEGER)
  ) {
    throw new Error('Unsafe numeric monetary value; use a decimal string');
  }
  if (
    !['string', 'number', 'bigint'].includes(typeof value) ||
    !/^-?\d{1,40}(?:\.\d{1,36})?$/.test(String(value))
  ) {
    throw new Error(`Invalid monetary value: ${String(value)}`);
  }
  return new Decimal(String(value));
}

// Computed amounts round once at the declared currency boundary, half away from zero.
export function formatMoney(value, currency = 'USD') {
  return decimal(value).toFixed(currencyScale(currency), Decimal.ROUND_HALF_UP);
}

// Payment instructions must already be representable: never silently round a charge.
export function paymentAmount(value, currency = 'USD', { allowZero = false } = {}) {
  const amount = decimal(value);
  const scale = currencyScale(currency);
  if (amount.isNegative() || (!allowZero && amount.isZero())) {
    throw new Error(`Amount must be ${allowZero ? 'non-negative' : 'positive'}`);
  }
  if (amount.decimalPlaces() > scale)
    throw new Error(`Amount exceeds ${currency} precision (${scale} decimal places)`);
  if (
    typeof value === 'number' &&
    amount.shiftedBy(scale).abs().gt(String(Number.MAX_SAFE_INTEGER))
  ) {
    throw new Error('Unsafe numeric monetary value; use a decimal string');
  }
  return amount.toFixed(scale);
}

export function fromMinorUnits(value, currency = 'USD') {
  const units = decimal(value);
  if (!units.isInteger() || units.isNegative())
    throw new Error('Minor units must be a non-negative integer');
  return units.shiftedBy(-currencyScale(currency)).toFixed(currencyScale(currency));
}

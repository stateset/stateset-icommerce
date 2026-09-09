// Reference quoting uses exact integer arithmetic. Prices retain up to 18
// decimal places; the demo's 5% fee and final totals round half-up to cents.
export const SCALE = 10n ** 18n;
export function amount(value) {
  if (typeof value !== 'string' || !/^(0|[1-9]\d{0,19})(\.\d{1,18})?$/.test(value)) {
    throw new Error('amount must be a nonnegative exact decimal string');
  }
  const [whole, fraction = ''] = value.split('.');
  return BigInt(whole) * SCALE + BigInt(fraction.padEnd(18, '0'));
}
function cents(value) {
  return `${value / 100n}.${(value % 100n).toString().padStart(2, '0')}`;
}

// Preserve sub-cent caps and balances instead of rounding past an authorization.
export function exactMoney(value) {
  if (typeof value !== 'bigint' || value < 0n) throw new Error('invalid money units');
  const fraction = (value % SCALE).toString().padStart(18, '0').replace(/0+$/, '').padEnd(2, '0');
  return `${value / SCALE}.${fraction}`;
}

export function roundCents(value) {
  return ((value * 100n + SCALE / 2n) / SCALE) * (SCALE / 100n);
}

export function quantity(value) {
  if (!Number.isSafeInteger(value) || value <= 0)
    throw new Error('invalid positive integer quantity');
  return BigInt(value);
}

export function priceDemoQuote(items, maximum) {
  if (!Array.isArray(items) || items.length === 0 || items.length > 1000)
    throw new Error('invalid quote lines');
  if (typeof maximum?.currency !== 'string' || !maximum.currency)
    throw new Error('currency required');
  let subtotal = 0n;
  const lines = items.map((item) => {
    if (
      !Number.isSafeInteger(item.quantity) ||
      item.quantity <= 0 ||
      item.unit_price?.currency !== maximum.currency
    )
      throw new Error('invalid quantity or mixed currencies');
    const total = amount(item.unit_price.amount) * BigInt(item.quantity);
    subtotal += total;
    return {
      ...item,
      line_total: {
        amount: cents((total * 100n + SCALE / 2n) / SCALE),
        currency: maximum.currency,
      },
    };
  });
  const total = (subtotal * 105n + SCALE / 2n) / SCALE;
  return {
    lines,
    amount: cents(total),
    exceedsMaximum: total * SCALE > amount(maximum.amount) * 100n,
  };
}

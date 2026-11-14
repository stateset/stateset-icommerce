/**
 * Exact decimal arithmetic for the oracle. The engine stores money and
 * quantities as base-10 strings; summing them as JavaScript numbers would let
 * the oracle itself introduce the drift it is looking for. Values are held as
 * BigInt scaled by 10^SCALE.
 */
const SCALE = 9;
const FACTOR = 10n ** BigInt(SCALE);

export function parseDecimal(text) {
  if (text === null || text === undefined || text === '') return 0n;
  const value = String(text).trim();
  const match = /^(-)?(\d*)(?:\.(\d*))?$/.exec(value);
  if (!match) throw new Error(`not a decimal: ${JSON.stringify(text)}`);
  const [, sign, whole = '', frac = ''] = match;
  if (frac.length > SCALE && /[1-9]/.test(frac.slice(SCALE))) {
    throw new Error(`decimal ${value} exceeds ${SCALE} fractional digits`);
  }
  const scaled =
    BigInt(whole || '0') * FACTOR + BigInt((frac.slice(0, SCALE) || '0').padEnd(SCALE, '0'));
  return sign ? -scaled : scaled;
}

export function formatDecimal(scaled) {
  const negative = scaled < 0n;
  const abs = negative ? -scaled : scaled;
  const whole = abs / FACTOR;
  const frac = (abs % FACTOR).toString().padStart(SCALE, '0').replace(/0+$/, '');
  return `${negative ? '-' : ''}${whole}${frac ? `.${frac}` : ''}`;
}

export function sumDecimals(values) {
  return values.reduce((total, value) => total + parseDecimal(value), 0n);
}

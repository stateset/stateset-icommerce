/**
 * Exact decimal arithmetic for money the engine reports as decimal strings.
 *
 * Values become a BigInt count of units at a decimal scale, so nothing is
 * ever rounded through a binary float. Sums keep every digit; a quotient that
 * does not terminate (an average, a monthly share of a yearly price) is
 * rounded once, half to even, the way the engine's `Decimal::round_dp` does.
 */

const DECIMAL_TEXT = /^-?\d+(\.\d+)?$/;

/**
 * The canonical decimal text of an engine amount. Strings must already be
 * plain decimals; numbers (the binding's deprecated float twins) are written
 * out without exponent notation.
 */
export function toDecimalString(value: string | number): string {
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new Error('Invalid decimal balance');
    const text = String(value);
    if (!/e/i.test(text)) return text;
    // 1e-7, 1.5e+21: spell the digits out, then drop trailing fraction zeros.
    return value
      .toFixed(20)
      .replace(/(\.\d*?)0+$/, '$1')
      .replace(/\.$/, '');
  }
  const text = value.trim();
  if (!DECIMAL_TEXT.test(text)) throw new Error('Invalid decimal balance');
  return text;
}

function parts(value: string | number) {
  const [whole, fraction = ''] = toDecimalString(value).split('.');
  return { units: BigInt(whole + fraction), scale: fraction.length };
}

function format(units: bigint, scale: number): string {
  const negative = units < BigInt(0);
  const digits = (negative ? -units : units).toString().padStart(scale + 1, '0');
  return `${negative ? '-' : ''}${scale ? `${digits.slice(0, -scale)}.${digits.slice(-scale)}` : digits}`;
}

function rescale(units: bigint, from: number, to: number): bigint {
  return units * BigInt(10) ** BigInt(to - from);
}

/** Digits after the decimal point. */
export function decimalScale(value: string | number): number {
  return parts(value).scale;
}

export function addDecimal(
  left: string | number,
  right: string | number,
  subtract = false,
): string {
  const a = parts(left),
    b = parts(right);
  const scale = Math.max(a.scale, b.scale);
  const units =
    rescale(a.units, a.scale, scale) + rescale(subtract ? -b.units : b.units, b.scale, scale);
  return format(units, scale);
}

/** The exact sum, at the largest scale among the terms (`'0'` for none). */
export function sumDecimals(values: Iterable<string | number>): string {
  let total = '0';
  for (const value of values) total = addDecimal(total, value);
  return total;
}

/** Exact product with a whole-number factor (a count, periods per year). */
export function multiplyDecimal(value: string | number, factor: number): string {
  if (!Number.isSafeInteger(factor)) throw new Error('Decimal factor must be a whole number');
  const { units, scale } = parts(value);
  return format(units * BigInt(factor), scale);
}

/**
 * `value / divisor` rounded half to even at `scale` digits (default: the
 * value's own scale, and at least cents). The divisor is a positive whole
 * number: a record count, months per year.
 */
export function divideDecimal(
  value: string | number,
  divisor: number,
  scale: number = Math.max(2, decimalScale(value)),
): string {
  if (!Number.isSafeInteger(divisor) || divisor <= 0) {
    throw new Error('Decimal divisor must be a positive whole number');
  }
  const { units, scale: from } = parts(value);
  // units / 10^from / divisor, expressed in units of 10^-scale.
  const numerator = scale >= from ? rescale(units, from, scale) : units;
  const denominator =
    BigInt(divisor) * (scale >= from ? BigInt(1) : BigInt(10) ** BigInt(from - scale));
  const negative = numerator < BigInt(0);
  const magnitude = negative ? -numerator : numerator;
  let quotient = magnitude / denominator;
  const twiceRemainder = (magnitude % denominator) * BigInt(2);
  if (
    twiceRemainder > denominator ||
    (twiceRemainder === denominator && quotient % BigInt(2) === BigInt(1))
  ) {
    quotient += BigInt(1);
  }
  return format(negative ? -quotient : quotient, scale);
}

export function isPositiveDecimal(value: string | number): boolean {
  return parts(value).units > BigInt(0);
}

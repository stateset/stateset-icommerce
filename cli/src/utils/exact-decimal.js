/**
 * Exact decimal arithmetic on money strings.
 *
 * The binding hands money back as exact decimal strings (`totalAmountExact`,
 * `"90.0"`, `"10"`). Adding them as JavaScript numbers reintroduces the float
 * error the strings exist to avoid, so this does it on scaled BigInts:
 * every value becomes `{ units, scale }` with `value = units / 10^scale`.
 */

const DECIMAL = /^([+-])?(\d+)(?:\.(\d+))?$/;

/**
 * Parse a decimal string (or a finite number, via its shortest round-trip
 * string) into scaled units.
 *
 * @param {string|number|bigint} value
 * @returns {{ units: bigint, scale: number }}
 */
export function parseDecimal(value) {
  if (typeof value === 'bigint') return { units: value, scale: 0 };
  let text;
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new TypeError(`Not a finite amount: ${value}`);
    text = String(value);
    // String() uses exponent form for very small/large values ("1e-7");
    // shift the mantissa's digits instead of going through binary again.
    const exp = /^(-)?(\d+)(?:\.(\d+))?e([+-]\d+)$/i.exec(text);
    if (exp) {
      const [, sign = '', whole, fraction = '', power] = exp;
      const shift = Number(power);
      const digits = whole + fraction;
      const point = whole.length + shift;
      text =
        point <= 0
          ? `${sign}0.${'0'.repeat(-point)}${digits}`
          : point >= digits.length
            ? `${sign}${digits}${'0'.repeat(point - digits.length)}`
            : `${sign}${digits.slice(0, point)}.${digits.slice(point)}`;
    }
  } else if (typeof value === 'string') {
    text = value.trim();
  } else {
    throw new TypeError(`Not a decimal amount: ${String(value)}`);
  }
  const match = DECIMAL.exec(text);
  if (!match) throw new TypeError(`Not a decimal amount: ${JSON.stringify(value)}`);
  const [, sign, whole, fraction = ''] = match;
  const units = BigInt(whole + fraction) * (sign === '-' ? -1n : 1n);
  return { units, scale: fraction.length };
}

/** @param {{units: bigint, scale: number}} d @param {number} scale */
function rescale(d, scale) {
  return d.units * 10n ** BigInt(scale - d.scale);
}

/** @param {{units: bigint, scale: number}} a @param {{units: bigint, scale: number}} b */
function align(a, b) {
  const scale = Math.max(a.scale, b.scale);
  return { a: rescale(a, scale), b: rescale(b, scale), scale };
}

/**
 * Render scaled units as a decimal string with at least two fraction digits
 * and no trailing zeros beyond them ("90.0" -> "90.00", "0.125" -> "0.125").
 *
 * @param {{units: bigint, scale: number}} d
 */
function render(d) {
  let { units, scale } = d;
  while (scale > 2 && units % 10n === 0n) {
    units /= 10n;
    scale -= 1;
  }
  if (scale < 2) {
    units *= 10n ** BigInt(2 - scale);
    scale = 2;
  }
  const negative = units < 0n;
  const digits = (negative ? -units : units).toString().padStart(scale + 1, '0');
  const text = `${digits.slice(0, digits.length - scale)}.${digits.slice(digits.length - scale)}`;
  return negative ? `-${text}` : text;
}

/** Canonical money string for a decimal value ("90.0" -> "90.00"). */
export function formatDecimal(value) {
  return render(parseDecimal(value));
}

/** Exact sum of decimal values; the empty sum is "0.00". */
export function addDecimals(...values) {
  let acc = { units: 0n, scale: 0 };
  for (const value of values.flat()) {
    const { a, b, scale } = align(acc, parseDecimal(value));
    acc = { units: a + b, scale };
  }
  return render(acc);
}

/** Exact `a - b`. */
export function subtractDecimals(a, b) {
  const { a: x, b: y, scale } = align(parseDecimal(a), parseDecimal(b));
  return render({ units: x - y, scale });
}

/** Exact `a * b`. */
export function multiplyDecimals(a, b) {
  const x = parseDecimal(a);
  const y = parseDecimal(b);
  return render({ units: x.units * y.units, scale: x.scale + y.scale });
}

/** -1, 0 or 1 as `a` is less than, equal to or greater than `b`. */
export function compareDecimals(a, b) {
  const { a: x, b: y } = align(parseDecimal(a), parseDecimal(b));
  return x < y ? -1 : x > y ? 1 : 0;
}

/**
 * Round to `places` fraction digits, half to even (the engine's
 * `Decimal::round_dp` default), and render.
 */
export function roundDecimal(value, places = 2) {
  const d = parseDecimal(value);
  if (d.scale <= places) return render(d);
  const divisor = 10n ** BigInt(d.scale - places);
  const negative = d.units < 0n;
  const magnitude = negative ? -d.units : d.units;
  let quotient = magnitude / divisor;
  const remainder = magnitude % divisor;
  const twice = remainder * 2n;
  if (twice > divisor || (twice === divisor && quotient % 2n === 1n)) quotient += 1n;
  return render({ units: negative ? -quotient : quotient, scale: places });
}

/** `value` if it is positive, else "0.00". */
export function clampAtZero(value) {
  return compareDecimals(value, '0') < 0 ? '0.00' : formatDecimal(value);
}

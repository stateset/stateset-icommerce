/** Exact addition/subtraction for displayed balances; never round through Number. */
function parts(value: string | number) {
  const text = String(value);
  if (!/^-?\d+(\.\d+)?$/.test(text)) throw new Error('Invalid decimal balance');
  const [whole, fraction = ''] = text.split('.');
  return { units: BigInt(whole + fraction), scale: fraction.length };
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
    a.units * BigInt(10) ** BigInt(scale - a.scale) +
    (subtract ? -b.units : b.units) * BigInt(10) ** BigInt(scale - b.scale);
  const negative = units < BigInt(0);
  const digits = (negative ? -units : units).toString().padStart(scale + 1, '0');
  return `${negative ? '-' : ''}${scale ? `${digits.slice(0, -scale)}.${digits.slice(-scale)}` : digits}`;
}

export function isPositiveDecimal(value: string | number): boolean {
  return parts(value).units > BigInt(0);
}

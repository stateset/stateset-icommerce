export function decimalToUnits(value, decimals = 6) {
  const text = String(value).trim();
  if (!/^(0|[1-9]\d*)(\.\d+)?$/.test(text)) {
    throw new Error('Amount must be a non-negative base-10 decimal string');
  }
  const [whole, fraction = ''] = text.split('.');
  if (fraction.length > decimals) {
    throw new Error(`Amount has more than ${decimals} decimal places`);
  }
  return (
    BigInt(whole) * 10n ** BigInt(decimals) +
    BigInt((fraction + '0'.repeat(decimals)).slice(0, decimals))
  );
}

export function unitsToDecimal(units, decimals = 6) {
  const value = BigInt(units);
  if (value < 0n) throw new Error('Amount cannot be negative');
  const base = 10n ** BigInt(decimals);
  const whole = value / base;
  const fraction = (value % base).toString().padStart(decimals, '0').replace(/0+$/, '');
  return fraction ? `${whole}.${fraction}` : whole.toString();
}

export function addDecimals(values, decimals = 6) {
  return unitsToDecimal(
    values.reduce((sum, value) => sum + decimalToUnits(value, decimals), 0n),
    decimals,
  );
}

export function multiplyDecimal(value, quantity, decimals = 6) {
  if (!Number.isSafeInteger(quantity) || quantity < 0)
    throw new Error('Quantity must be a non-negative integer');
  return unitsToDecimal(decimalToUnits(value, decimals) * BigInt(quantity), decimals);
}

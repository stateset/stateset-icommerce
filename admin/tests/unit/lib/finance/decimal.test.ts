import { describe, expect, it } from 'vitest';

import {
  addDecimal,
  decimalScale,
  divideDecimal,
  isPositiveDecimal,
  multiplyDecimal,
  sumDecimals,
  toDecimalString,
} from '@/lib/finance/decimal';

describe('exact decimal arithmetic', () => {
  it('adds and subtracts without float drift', () => {
    expect(addDecimal('0.10', '0.20')).toBe('0.30');
    expect(addDecimal('1.005', '2')).toBe('3.005');
    expect(addDecimal('5', '7.25', true)).toBe('-2.25');
    expect(() => addDecimal('1e3', '1')).toThrow('Invalid decimal balance');
  });

  it('sums every digit, beyond what a float holds', () => {
    // 0.1 + 0.2 + 0.3 is 0.6000000000000001 in IEEE-754.
    expect(sumDecimals(['0.10', '0.20', '0.30'])).toBe('0.60');
    expect(sumDecimals(['90071992547409.93', '0.01'])).toBe('90071992547409.94');
    expect(sumDecimals([])).toBe('0');
  });

  it('multiplies by a whole factor exactly', () => {
    expect(multiplyDecimal('19.99', 12)).toBe('239.88');
    expect(multiplyDecimal('0.10', 4)).toBe('0.40');
    expect(() => multiplyDecimal('1', 0.5)).toThrow(/whole number/);
  });

  it('divides once, rounding half to even at the value scale (at least cents)', () => {
    expect(divideDecimal('760.28', 12)).toBe('63.36');
    expect(divideDecimal('10', 4)).toBe('2.50');
    expect(divideDecimal('1.305', 3)).toBe('0.435');
    // Ties go to the even digit, like the engine's Decimal::round_dp.
    expect(divideDecimal('0.25', 1, 1)).toBe('0.2');
    expect(divideDecimal('0.35', 1, 1)).toBe('0.4');
    expect(divideDecimal('-0.35', 1, 1)).toBe('-0.4');
    expect(divideDecimal('0.125', 1, 2)).toBe('0.12');
    expect(divideDecimal('1.239', 1, 2)).toBe('1.24');
    expect(divideDecimal('520.00', 12)).toBe('43.33');
    expect(() => divideDecimal('1', 0)).toThrow(/positive whole number/);
  });

  it('reads the binding float twins without exponent notation', () => {
    expect(toDecimalString(29.99)).toBe('29.99');
    expect(toDecimalString(1e-7)).toBe('0.0000001');
    expect(toDecimalString(' 12.50 ')).toBe('12.50');
    expect(decimalScale('1.005')).toBe(3);
    expect(isPositiveDecimal('0.00')).toBe(false);
    expect(() => toDecimalString(Number.NaN)).toThrow('Invalid decimal balance');
  });
});

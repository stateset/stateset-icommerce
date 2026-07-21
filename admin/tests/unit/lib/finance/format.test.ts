/**
 * Tests for display-only money formatting.
 * @module tests/unit/lib/finance/format
 */

import { describe, it, expect } from 'vitest';
import { formatMoney } from '@/lib/finance/format';

describe('formatMoney', () => {
  it('formats exact decimal strings without float parsing', () => {
    expect(formatMoney('1234567.89')).toBe('$1,234,567.89');
    expect(formatMoney('0.00')).toBe('$0.00');
    expect(formatMoney('12480.75')).toBe('$12,480.75');
  });

  it('preserves high-precision decimals verbatim', () => {
    // A float round-trip would mangle this; string formatting must not.
    expect(formatMoney('0.100000000000000000001')).toBe('$0.100000000000000000001');
  });

  it('handles negatives and pads short fractions', () => {
    expect(formatMoney('-42.5')).toBe('-$42.50');
    expect(formatMoney('10')).toBe('$10.00');
  });

  it('formats legacy number amounts for display', () => {
    expect(formatMoney(1250)).toBe('$1,250.00');
  });

  it('supports non-USD currencies with a code prefix', () => {
    expect(formatMoney('99.90', 'EUR')).toBe('EUR 99.90');
  });

  it('returns non-decimal input verbatim instead of coercing', () => {
    expect(formatMoney('N/A')).toBe('N/A');
    expect(formatMoney('1,000')).toBe('1,000');
  });
});

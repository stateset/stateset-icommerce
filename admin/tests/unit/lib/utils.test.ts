import { describe, expect, it } from 'vitest';
import { formatRatio } from '@/lib/utils';

describe('formatRatio', () => {
  it('renders a 0-1 ratio as a percentage', () => {
    // The admin's rates are 0-1 ratios; 0.25 used to render as "0.25%".
    expect(formatRatio(0.25)).toBe('25.0%');
    expect(formatRatio(0.032)).toBe('3.2%');
    expect(formatRatio(1)).toBe('100.0%');
    expect(formatRatio(0)).toBe('0.0%');
  });

  it('takes the number of decimals to show', () => {
    expect(formatRatio(0.98765, 2)).toBe('98.77%');
    expect(formatRatio(0.5, 0)).toBe('50%');
  });
});

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  addDecimals,
  clampAtZero,
  compareDecimals,
  formatDecimal,
  multiplyDecimals,
  parseDecimal,
  roundDecimal,
  subtractDecimals,
} from '../../src/utils/exact-decimal.js';

describe('exact-decimal', () => {
  it('adds without float error', () => {
    assert.equal(addDecimals('0.1', '0.2'), '0.30');
    assert.equal(addDecimals(['19.99', '0.01'], '80'), '100.00');
    assert.equal(addDecimals(), '0.00');
  });

  it('subtracts, multiplies and compares exactly', () => {
    assert.equal(subtractDecimals('90.0', '10.00'), '80.00');
    assert.equal(subtractDecimals('1', '1.005'), '-0.005');
    assert.equal(multiplyDecimals('19.99', 3), '59.97');
    assert.equal(compareDecimals('90.0', '90.00'), 0);
    assert.equal(compareDecimals('-1', '0'), -1);
  });

  it('renders at least two fraction digits and keeps real precision', () => {
    assert.equal(formatDecimal('90.0'), '90.00');
    assert.equal(formatDecimal('0.125'), '0.125');
    assert.equal(formatDecimal('1.2500'), '1.25');
    assert.equal(formatDecimal(-0.5), '-0.50');
  });

  it('rounds half to even like the engine', () => {
    assert.equal(roundDecimal('0.125'), '0.12');
    assert.equal(roundDecimal('0.135'), '0.14');
    assert.equal(roundDecimal('-0.125'), '-0.12');
    assert.equal(roundDecimal('0.1251'), '0.13');
  });

  it('clamps at zero and rejects non-decimals', () => {
    assert.equal(clampAtZero('-3'), '0.00');
    assert.equal(clampAtZero('3'), '3.00');
    assert.throws(() => parseDecimal('1e3x'));
    assert.throws(() => parseDecimal(Number.NaN));
    assert.deepEqual(parseDecimal(1e-7), { units: 1n, scale: 7 });
    assert.equal(formatDecimal(1.5e21), '1500000000000000000000.00');
  });
});

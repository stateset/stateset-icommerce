import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  safeParseInt,
  safeParseFloat,
  strictParseInt,
  strictParseFloat,
} from '../../src/utils/safe-numbers.js';
import { ValidationError } from '../../src/errors.js';

describe('safe-numbers', () => {
  describe('safeParseInt', () => {
    it('parses valid integer string', () => {
      assert.strictEqual(safeParseInt('42'), 42);
    });

    it('parses numeric value', () => {
      assert.strictEqual(safeParseInt(42), 42);
    });

    it('parses negative integer', () => {
      assert.strictEqual(safeParseInt('-5'), -5);
    });

    it('parses zero', () => {
      assert.strictEqual(safeParseInt('0'), 0);
    });

    it('returns default fallback (0) on NaN', () => {
      assert.strictEqual(safeParseInt('invalid'), 0);
    });

    it('returns custom fallback on NaN', () => {
      assert.strictEqual(safeParseInt('invalid', { fallback: -1 }), -1);
    });

    it('returns fallback for undefined/null', () => {
      assert.strictEqual(safeParseInt(undefined, { fallback: 10 }), 10);
      assert.strictEqual(safeParseInt(null, { fallback: 10 }), 10);
    });

    it('respects radix parameter', () => {
      assert.strictEqual(safeParseInt('ff', { radix: 16 }), 255);
      assert.strictEqual(safeParseInt('10', { radix: 2 }), 2);
    });

    it('validates minimum bound', () => {
      assert.strictEqual(safeParseInt('5', { min: 0, max: 10 }), 5);
      assert.strictEqual(safeParseInt('-1', { min: 0, fallback: 0 }), 0);
    });

    it('validates maximum bound', () => {
      assert.strictEqual(safeParseInt('100', { max: 50, fallback: 50 }), 50);
    });

    it('throws ValidationError when throwOnError is true and NaN', () => {
      assert.throws(() => safeParseInt('abc', { throwOnError: true }), ValidationError);
    });

    it('throws ValidationError when out of range with throwOnError', () => {
      assert.throws(
        () => safeParseInt('100', { min: 0, max: 50, throwOnError: true }),
        ValidationError,
      );
    });

    it('includes field name in error', () => {
      try {
        safeParseInt('bad', { throwOnError: true, field: 'port' });
        assert.fail('should have thrown');
      } catch (err) {
        assert.ok(err.message.includes('port'));
      }
    });
  });

  describe('safeParseFloat', () => {
    it('parses valid float string', () => {
      assert.strictEqual(safeParseFloat('3.14'), 3.14);
    });

    it('parses integer as float', () => {
      assert.strictEqual(safeParseFloat('42'), 42);
    });

    it('parses numeric value', () => {
      assert.strictEqual(safeParseFloat(3.14), 3.14);
    });

    it('returns fallback on NaN', () => {
      assert.strictEqual(safeParseFloat('invalid', { fallback: 0.0 }), 0.0);
    });

    it('rejects Infinity', () => {
      assert.strictEqual(safeParseFloat('Infinity', { fallback: 0 }), 0);
      assert.strictEqual(safeParseFloat('-Infinity', { fallback: 0 }), 0);
    });

    it('validates range', () => {
      assert.strictEqual(safeParseFloat('5.5', { min: 0, max: 10 }), 5.5);
      assert.strictEqual(safeParseFloat('15.5', { min: 0, max: 10, fallback: 10 }), 10);
    });

    it('throws with throwOnError for NaN', () => {
      assert.throws(() => safeParseFloat('bad', { throwOnError: true }), ValidationError);
    });

    it('throws with throwOnError for Infinity', () => {
      assert.throws(() => safeParseFloat('Infinity', { throwOnError: true }), ValidationError);
    });
  });

  describe('strictParseInt', () => {
    it('parses valid integer', () => {
      assert.strictEqual(strictParseInt('42'), 42);
    });

    it('always throws on invalid', () => {
      assert.throws(() => strictParseInt('invalid'), ValidationError);
    });

    it('includes field name in error', () => {
      try {
        strictParseInt('bad', 'port');
        assert.fail('should have thrown');
      } catch (err) {
        assert.ok(err.message.includes('port'));
      }
    });

    it('respects range options', () => {
      assert.strictEqual(strictParseInt('50', 'count', { min: 0, max: 100 }), 50);
      assert.throws(() => strictParseInt('200', 'count', { min: 0, max: 100 }), ValidationError);
    });
  });

  describe('strictParseFloat', () => {
    it('parses valid float', () => {
      assert.strictEqual(strictParseFloat('3.14'), 3.14);
    });

    it('always throws on invalid', () => {
      assert.throws(() => strictParseFloat('invalid'), ValidationError);
    });

    it('includes field name in error', () => {
      try {
        strictParseFloat('bad', 'amount');
        assert.fail('should have thrown');
      } catch (err) {
        assert.ok(err.message.includes('amount'));
      }
    });
  });
});

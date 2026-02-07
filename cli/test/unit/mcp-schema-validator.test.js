/**
 * Unit tests for mcp-schema-validator.js
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import { z } from 'zod';
import { EnhancedValidator, CommerceValidators } from '../../src/mcp-schema-validator.js';

// ===========================================================================
// EnhancedValidator
// ===========================================================================

describe('EnhancedValidator', () => {
  const validator = new EnhancedValidator();

  describe('validate', () => {
    it('returns valid for correct data', () => {
      const schema = z.object({ name: z.string(), age: z.number() });
      const result = validator.validate(schema, { name: 'Alice', age: 30 });
      assert.strictEqual(result.valid, true);
      assert.strictEqual(result.errors.length, 0);
    });

    it('returns errors for invalid data', () => {
      const schema = z.object({ name: z.string(), age: z.number() });
      const result = validator.validate(schema, { name: 123, age: 'thirty' });
      assert.strictEqual(result.valid, false);
      assert.ok(result.errors.length > 0);
    });

    it('returns errors with formatted messages for invalid data', () => {
      const schema = z.object({ email: z.string().email() });
      const result = validator.validate(schema, { email: 'not-an-email' });
      assert.strictEqual(result.valid, false);
      assert.ok(result.errors.length > 0);
      assert.ok(result.errors[0].message.length > 0);
    });

    it('includes context with error count', () => {
      const schema = z.object({ name: z.string() });
      const result = validator.validate(schema, { name: 42 });
      assert.ok(result.context);
      assert.strictEqual(result.context.errorCount, 1);
    });

    it('handles schema parse errors gracefully', () => {
      // Pass something that causes an internal error
      const result = validator.validate(null, {});
      assert.strictEqual(result.valid, false);
      assert.ok(result.errors.length > 0);
      assert.strictEqual(result.errors[0].severity, 'critical');
    });

    it('uses basePath when provided', () => {
      const schema = z.object({ name: z.string() });
      const result = validator.validate(schema, { name: 42 }, 'customer');
      assert.ok(result.errors[0].path.includes('customer'));
    });

    it('passes through extra fields with passthrough', () => {
      const schema = z.object({ name: z.string() });
      const result = validator.validate(schema, { name: 'Alice', extra: true });
      assert.strictEqual(result.valid, true);
      assert.ok(result.data.extra === true);
    });
  });

  describe('determineSeverity', () => {
    it('returns critical for required', () => {
      assert.strictEqual(validator.determineSeverity('required'), 'critical');
    });

    it('returns high for invalid_type', () => {
      assert.strictEqual(validator.determineSeverity('invalid_type'), 'high');
    });

    it('returns medium for too_small', () => {
      assert.strictEqual(validator.determineSeverity('too_small'), 'medium');
    });

    it('returns low for unknown codes', () => {
      assert.strictEqual(validator.determineSeverity('unknown_code'), 'low');
    });
  });

  describe('formatEnumValues', () => {
    it('formats string array with quotes', () => {
      const result = validator.formatEnumValues(['active', 'inactive', 'draft']);
      assert.ok(result.includes('"active"'));
      assert.ok(result.includes('"inactive"'));
    });

    it('formats non-array as string', () => {
      assert.strictEqual(validator.formatEnumValues('single'), 'single');
    });
  });

  describe('formatExpectedType', () => {
    it('maps string to text/string', () => {
      assert.strictEqual(validator.formatExpectedType('string'), 'text/string');
    });

    it('maps boolean to true/false', () => {
      assert.strictEqual(validator.formatExpectedType('boolean'), 'true/false');
    });

    it('returns unknown types as-is', () => {
      assert.strictEqual(validator.formatExpectedType('custom'), 'custom');
    });
  });

  describe('formatValue', () => {
    it('formats null/undefined', () => {
      assert.strictEqual(validator.formatValue(null), 'null/undefined');
      assert.strictEqual(validator.formatValue(undefined), 'null/undefined');
    });

    it('formats strings with quotes', () => {
      assert.ok(validator.formatValue('hello').includes('"hello"'));
    });

    it('truncates long strings', () => {
      const long = 'x'.repeat(100);
      const result = validator.formatValue(long);
      assert.ok(result.includes('...'));
    });

    it('formats numbers as string', () => {
      assert.strictEqual(validator.formatValue(42), '42');
    });
  });

  describe('getExamples', () => {
    it('returns email examples for invalid_email', () => {
      const examples = validator.getExamples('invalid_email');
      assert.ok(examples.length > 0);
      assert.ok(examples.some((e) => e.includes('@')));
    });

    it('returns empty array for unknown codes', () => {
      assert.deepStrictEqual(validator.getExamples('unknown'), []);
    });
  });

  describe('extractSafeSample', () => {
    it('redacts sensitive fields', () => {
      const data = { name: 'Alice', password: 'secret123', apiKey: 'sk-test' };
      const sample = validator.extractSafeSample(data);
      assert.strictEqual(sample.name, 'Alice');
      assert.strictEqual(sample.password, '[REDACTED]');
    });

    it('limits depth', () => {
      const deep = { a: { b: { c: { d: 'deep' } } } };
      const sample = validator.extractSafeSample(deep, 2);
      assert.strictEqual(sample.a.b, '[max depth reached]');
    });

    it('limits array items', () => {
      const data = { items: [1, 2, 3, 4, 5] };
      const sample = validator.extractSafeSample(data);
      assert.ok(sample.items.length <= 3);
    });

    it('handles null input', () => {
      assert.strictEqual(validator.extractSafeSample(null), null);
    });
  });

  describe('validateBatch', () => {
    it('validates multiple items', () => {
      const schema = z.object({ name: z.string() });
      const items = [{ name: 'Alice' }, { name: 42 }, { name: 'Bob' }];
      const { results, summary } = validator.validateBatch(schema, items);

      assert.strictEqual(results.length, 3);
      assert.strictEqual(summary.total, 3);
      assert.strictEqual(summary.valid, 2);
      assert.strictEqual(summary.invalid, 1);
    });

    it('tracks common errors', () => {
      const schema = z.object({ age: z.number() });
      const items = [{ age: 'a' }, { age: 'b' }];
      const { summary } = validator.validateBatch(schema, items);

      assert.strictEqual(summary.invalid, 2);
      assert.ok(summary.commonErrors.length > 0);
      // Total error count across all common errors should be >= 2
      const totalErrors = summary.commonErrors.reduce((sum, e) => sum + e.count, 0);
      assert.ok(totalErrors >= 2, `Expected >= 2 total errors, got ${totalErrors}`);
    });
  });

  describe('createReport', () => {
    it('includes metadata and summary', () => {
      const schema = z.object({ name: z.string() });
      const report = validator.createReport(schema, { name: 42 }, { source: 'test' });

      assert.ok(report.timestamp);
      assert.strictEqual(report.metadata.source, 'test');
      assert.strictEqual(report.result.valid, false);
      assert.ok(report.summary.high > 0 || report.summary.critical > 0);
    });

    it('creates valid report for valid data', () => {
      const schema = z.object({ name: z.string() });
      const report = validator.createReport(schema, { name: 'Alice' });

      assert.strictEqual(report.result.valid, true);
      assert.strictEqual(report.summary.total, 0);
    });
  });
});

// ===========================================================================
// CommerceValidators
// ===========================================================================

describe('CommerceValidators', () => {
  const validator = new EnhancedValidator();

  describe('customer', () => {
    it('validates correct customer', () => {
      const result = validator.validate(CommerceValidators.customer, {
        email: 'alice@example.com',
        firstName: 'Alice',
        lastName: 'Smith',
      });
      assert.strictEqual(result.valid, true);
    });

    it('rejects invalid email', () => {
      const result = validator.validate(CommerceValidators.customer, {
        email: 'not-email',
        firstName: 'Alice',
        lastName: 'Smith',
      });
      assert.strictEqual(result.valid, false);
    });

    it('rejects empty firstName', () => {
      const result = validator.validate(CommerceValidators.customer, {
        email: 'alice@example.com',
        firstName: '',
        lastName: 'Smith',
      });
      assert.strictEqual(result.valid, false);
    });
  });

  describe('order', () => {
    it('validates correct order', () => {
      const result = validator.validate(CommerceValidators.order, {
        customerId: '550e8400-e29b-41d4-a716-446655440000',
        items: [{ sku: 'SKU-1', name: 'Widget', quantity: 2, unitPrice: 9.99 }],
        currency: 'USD',
      });
      assert.strictEqual(result.valid, true);
    });

    it('rejects order with no items', () => {
      const result = validator.validate(CommerceValidators.order, {
        customerId: '550e8400-e29b-41d4-a716-446655440000',
        items: [],
        currency: 'USD',
      });
      assert.strictEqual(result.valid, false);
    });

    it('rejects negative unit price', () => {
      const result = validator.validate(CommerceValidators.order, {
        customerId: '550e8400-e29b-41d4-a716-446655440000',
        items: [{ sku: 'SKU-1', name: 'Widget', quantity: 1, unitPrice: -5 }],
        currency: 'USD',
      });
      assert.strictEqual(result.valid, false);
    });
  });

  describe('product', () => {
    it('validates correct product', () => {
      const result = validator.validate(CommerceValidators.product, {
        name: 'Widget Pro',
        slug: 'widget-pro',
        status: 'active',
      });
      assert.strictEqual(result.valid, true);
    });

    it('rejects invalid slug format', () => {
      const result = validator.validate(CommerceValidators.product, {
        name: 'Widget',
        slug: 'Widget Pro!!!', // invalid slug
        status: 'active',
      });
      assert.strictEqual(result.valid, false);
    });

    it('rejects invalid status', () => {
      const result = validator.validate(CommerceValidators.product, {
        name: 'Widget',
        slug: 'widget',
        status: 'deleted', // not a valid enum value
      });
      assert.strictEqual(result.valid, false);
    });
  });

  describe('inventoryAdjustment', () => {
    it('validates correct adjustment', () => {
      const result = validator.validate(CommerceValidators.inventoryAdjustment, {
        sku: 'SKU-001',
        quantity: -5,
        reason: 'damaged in shipping',
      });
      assert.strictEqual(result.valid, true);
    });

    it('rejects non-integer quantity', () => {
      const result = validator.validate(CommerceValidators.inventoryAdjustment, {
        sku: 'SKU-001',
        quantity: 2.5,
        reason: 'partial',
      });
      assert.strictEqual(result.valid, false);
    });
  });
});

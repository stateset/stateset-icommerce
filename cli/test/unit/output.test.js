/**
 * Unit tests for output.js
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert';
import { RichOutput, ICONS, createOutput, formatStructuredOutput } from '../../src/output.js';

describe('output', () => {
  describe('ICONS', () => {
    it('should have common icons defined', () => {
      assert.ok(ICONS.success);
      assert.ok(ICONS.error);
      assert.ok(ICONS.warning);
      assert.ok(ICONS.info);
      assert.ok(ICONS.order);
      assert.ok(ICONS.cart);
      assert.ok(ICONS.customer);
    });
  });

  describe('RichOutput', () => {
    let output;

    beforeEach(() => {
      output = new RichOutput({ color: false });
    });

    describe('table', () => {
      it('should format data as table', () => {
        const data = [
          { id: '1', name: 'Alice', status: 'active' },
          { id: '2', name: 'Bob', status: 'inactive' }
        ];

        const columns = [
          { key: 'id', header: 'ID' },
          { key: 'name', header: 'Name' },
          { key: 'status', header: 'Status' }
        ];

        const result = output.table(data, columns);

        assert.ok(result.includes('ID'));
        assert.ok(result.includes('Name'));
        assert.ok(result.includes('Alice'));
        assert.ok(result.includes('Bob'));
      });

      it('should return JSON when format is json', () => {
        const jsonOutput = new RichOutput({ format: 'json' });
        const data = [{ id: '1', name: 'Alice' }];
        const columns = [{ key: 'id', header: 'ID' }];

        const result = jsonOutput.table(data, columns);
        const parsed = JSON.parse(result);

        assert.deepStrictEqual(parsed, data);
      });

      it('should handle empty data', () => {
        const result = output.table([], [{ key: 'id', header: 'ID' }]);
        assert.ok(result.includes('no data'));
      });

      it('should apply column formatters', () => {
        const data = [{ price: 100 }];
        const columns = [{
          key: 'price',
          header: 'Price',
          format: (val) => `$${val}`
        }];

        const result = output.table(data, columns);
        assert.ok(result.includes('$100'));
      });

      it('should handle null values', () => {
        const data = [{ id: '1', name: null }];
        const columns = [
          { key: 'id', header: 'ID' },
          { key: 'name', header: 'Name' }
        ];

        const result = output.table(data, columns);
        assert.ok(result.includes('1'));
      });
    });

    describe('progress', () => {
      it('should show progress bar', () => {
        const result = output.progress(50, 100, 'Loading');
        assert.ok(result.includes('50%'));
        assert.ok(result.includes('Loading'));
        assert.ok(result.includes('50/100'));
      });

      it('should handle zero total', () => {
        const result = output.progress(0, 0);
        assert.ok(result.includes('0%'));
      });

      it('should handle complete progress', () => {
        const result = output.progress(100, 100);
        assert.ok(result.includes('100%'));
      });
    });

    describe('status', () => {
      it('should format success status', () => {
        const result = output.status('success', 'Operation completed');
        assert.ok(result.includes(ICONS.success));
        assert.ok(result.includes('Operation completed'));
      });

      it('should format error status', () => {
        const result = output.status('error', 'Something went wrong');
        assert.ok(result.includes(ICONS.error));
      });

      it('should format warning status', () => {
        const result = output.status('warning', 'Be careful');
        assert.ok(result.includes(ICONS.warning));
      });
    });

    describe('currency', () => {
      it('should format USD currency', () => {
        const result = output.currency(1234.56, 'USD');
        assert.ok(result.includes('1,234.56') || result.includes('$1,234.56'));
      });

      it('should handle null values', () => {
        const result = output.currency(null);
        assert.strictEqual(result, '—');
      });

      it('should handle unknown currency', () => {
        const result = output.currency(100, 'XYZ');
        assert.ok(result.includes('100'));
      });
    });

    describe('number', () => {
      it('should format numbers with commas', () => {
        const result = output.number(1234567);
        assert.ok(result.includes('1,234,567'));
      });

      it('should handle decimals', () => {
        const result = output.number(1234.567, 2);
        assert.ok(result.includes('1,234.57'));
      });

      it('should handle null values', () => {
        const result = output.number(null);
        assert.strictEqual(result, '—');
      });
    });

    describe('percent', () => {
      it('should format percentages', () => {
        const result = output.percent(75.5);
        assert.strictEqual(result, '75.5%');
      });

      it('should handle custom decimals', () => {
        const result = output.percent(33.333, 2);
        assert.strictEqual(result, '33.33%');
      });
    });

    describe('date', () => {
      it('should format dates', () => {
        const result = output.date('2024-03-15T10:30:00Z');
        assert.ok(result.includes('2024') || result.includes('Mar'));
      });

      it('should handle null values', () => {
        const result = output.date(null);
        assert.strictEqual(result, '—');
      });
    });

    describe('datetime', () => {
      it('should format datetime', () => {
        const result = output.datetime('2024-03-15T10:30:00Z');
        assert.ok(result.includes('2024') || result.includes('Mar'));
      });
    });

    describe('relativeTime', () => {
      it('should show "just now" for recent times', () => {
        const result = output.relativeTime(new Date().toISOString());
        assert.ok(result.includes('just now') || result.includes('m ago'));
      });

      it('should handle null values', () => {
        const result = output.relativeTime(null);
        assert.strictEqual(result, '—');
      });
    });

    describe('orderStatus', () => {
      it('should format order statuses', () => {
        const statuses = ['pending', 'confirmed', 'shipped', 'delivered', 'cancelled'];

        for (const status of statuses) {
          const result = output.orderStatus(status);
          assert.ok(result.includes(status));
        }
      });

      it('should handle unknown status', () => {
        const result = output.orderStatus('unknown');
        assert.ok(result.includes('unknown'));
      });
    });

    describe('inventoryStatus', () => {
      it('should show out of stock', () => {
        const result = output.inventoryStatus(0);
        assert.ok(result.includes('Out of Stock'));
      });

      it('should show low stock', () => {
        const result = output.inventoryStatus(5, 10);
        assert.ok(result.includes('Low Stock'));
      });

      it('should show in stock', () => {
        const result = output.inventoryStatus(100, 10);
        assert.ok(result.includes('In Stock'));
      });
    });

    describe('keyValue', () => {
      it('should format key-value pairs', () => {
        const result = output.keyValue({
          Name: 'Alice',
          Email: 'alice@example.com'
        });

        assert.ok(result.includes('Name'));
        assert.ok(result.includes('Alice'));
        assert.ok(result.includes('Email'));
      });
    });

    describe('list', () => {
      it('should format bullet list', () => {
        const result = output.list(['Item 1', 'Item 2', 'Item 3']);
        assert.ok(result.includes('Item 1'));
        assert.ok(result.includes('Item 2'));
        assert.ok(result.includes(ICONS.bullet));
      });
    });

    describe('box', () => {
      it('should create box around text', () => {
        const result = output.box('Hello World');
        assert.ok(result.includes('Hello World'));
        assert.ok(result.includes('┌'));
        assert.ok(result.includes('└'));
      });

      it('should include title if provided', () => {
        const result = output.box('Content', { title: 'Title' });
        assert.ok(result.includes('Title'));
      });
    });

    describe('toolCall', () => {
      it('should format tool call', () => {
        const result = output.toolCall('list_customers', { limit: 10 });
        assert.ok(result.includes('list_customers'));
        assert.ok(result.includes(ICONS.tool));
      });

      it('should strip mcp prefix', () => {
        const result = output.toolCall('mcp__stateset-commerce__list_customers', {});
        assert.ok(result.includes('list_customers'));
        assert.ok(!result.includes('mcp__'));
      });

      it('should truncate long input', () => {
        const longInput = { data: 'x'.repeat(100) };
        const result = output.toolCall('test', longInput);
        assert.ok(result.includes('...'));
      });
    });
  });

  describe('createOutput', () => {
    it('should create RichOutput instance', () => {
      const output = createOutput();
      assert.ok(output instanceof RichOutput);
    });

    it('should pass options', () => {
      const output = createOutput({ format: 'json' });
      assert.strictEqual(output.format, 'json');
    });
  });

  describe('formatStructuredOutput', () => {
    it('should format object data as a table', () => {
      const result = formatStructuredOutput({ foo: 1, bar: 'baz' }, 'table');
      assert.ok(result.includes('key'));
      assert.ok(result.includes('foo'));
      assert.ok(result.includes('bar'));
    });

    it('should format array data as csv', () => {
      const data = [
        { id: 1, name: 'Alice' },
        { id: 2, name: 'Bob' }
      ];
      const result = formatStructuredOutput(data, 'csv');
      assert.ok(result.startsWith('id,name'));
      assert.ok(result.includes('Alice'));
      assert.ok(result.includes('Bob'));
    });

    it('should format data as json', () => {
      const data = [{ id: '1', name: 'Alice' }];
      const result = formatStructuredOutput(data, 'json');
      const parsed = JSON.parse(result);
      assert.deepStrictEqual(parsed, data);
    });
  });
});

/**
 * Currency & Exchange Rate Tools Test Suite
 *
 * Tests for cli/src/tools/currency.js
 * Covers: get_exchange_rate, list_exchange_rates, convert_currency,
 *         set_exchange_rate, get_currency_settings, set_base_currency,
 *         enable_currencies, format_currency
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { currencyTools } from '../../src/tools/currency.js';

// ============================================================================
// Helper: find tool by name
// ============================================================================

function findTool(name) {
  const tool = currencyTools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// Mock factory
// ============================================================================

const mockRate = {
  baseCurrency: 'USD',
  quoteCurrency: 'EUR',
  rate: 0.92,
  source: 'api',
  rateAt: '2026-03-01T00:00:00Z',
};

const mockConversion = {
  originalAmount: 100,
  originalCurrency: 'USD',
  convertedAmount: 92.0,
  targetCurrency: 'EUR',
  rate: 0.92,
  inverseRate: 1.087,
  rateAt: '2026-03-01T00:00:00Z',
};

const mockSettings = {
  baseCurrency: 'USD',
  enabledCurrencies: ['USD', 'EUR', 'GBP'],
  autoConvert: true,
  roundingMode: 'half_up',
};

function makeCurrencyCommerce(overrides = {}) {
  return {
    currency: {
      getRate: async () => mockRate,
      listRates: async () => [mockRate],
      getRatesFor: async () => [mockRate],
      convert: async () => mockConversion,
      setRate: async (data) => ({ id: 'rate_001', ...data, rateAt: '2026-03-01T00:00:00Z' }),
      getSettings: async () => mockSettings,
      setBaseCurrency: async (currency) => ({
        baseCurrency: currency,
        enabledCurrencies: ['USD', 'EUR', 'GBP'],
      }),
      enableCurrencies: async (currencies) => ({
        baseCurrency: 'USD',
        enabledCurrencies: currencies,
      }),
      format: async (amount, currency) => `$${amount.toFixed(2)} ${currency}`,
      ...overrides,
    },
  };
}

// ============================================================================
// Module exports
// ============================================================================

describe('currencyTools — module exports', () => {
  it('exports an array of 8 tools', () => {
    assert.ok(Array.isArray(currencyTools));
    assert.equal(currencyTools.length, 8);
  });

  it('exports expected tool names', () => {
    const names = currencyTools.map((t) => t.name);
    assert.deepStrictEqual(names, [
      'get_exchange_rate',
      'list_exchange_rates',
      'convert_currency',
      'set_exchange_rate',
      'get_currency_settings',
      'set_base_currency',
      'enable_currencies',
      'format_currency',
    ]);
  });

  it('all tools have handler functions', () => {
    for (const tool of currencyTools) {
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
    }
  });

  it('all tools have valid permissions', () => {
    for (const tool of currencyTools) {
      assert.ok(
        ['read', 'write', 'admin'].includes(tool.permission),
        `${tool.name} has invalid permission: ${tool.permission}`,
      );
    }
  });

  it('all tools have non-empty descriptions', () => {
    for (const tool of currencyTools) {
      assert.ok(tool.description, `${tool.name} missing description`);
      assert.ok(tool.description.length > 10, `${tool.name} description too short`);
    }
  });
});

// ============================================================================
// Permission checks
// ============================================================================

describe('currencyTools — permission assignments', () => {
  it('read tools have read permission', () => {
    const readToolNames = [
      'get_exchange_rate',
      'list_exchange_rates',
      'convert_currency',
      'get_currency_settings',
      'format_currency',
    ];
    for (const name of readToolNames) {
      const tool = findTool(name);
      assert.equal(tool.permission, 'read', `${name} should be read`);
    }
  });

  it('write tools have write permission', () => {
    const writeToolNames = ['set_exchange_rate', 'set_base_currency', 'enable_currencies'];
    for (const name of writeToolNames) {
      const tool = findTool(name);
      assert.equal(tool.permission, 'write', `${name} should be write`);
    }
  });
});

// ============================================================================
// Input schema validation
// ============================================================================

describe('currencyTools — input schemas', () => {
  it('get_exchange_rate has from and to fields', () => {
    const schema = findTool('get_exchange_rate').inputSchema;
    assert.ok(schema.from, 'missing from field');
    assert.ok(schema.to, 'missing to field');
  });

  it('list_exchange_rates has optional baseCurrency field', () => {
    const schema = findTool('list_exchange_rates').inputSchema;
    assert.ok(schema.baseCurrency, 'missing baseCurrency field');
  });

  it('convert_currency has from, to, and amount fields', () => {
    const schema = findTool('convert_currency').inputSchema;
    assert.ok(schema.from, 'missing from field');
    assert.ok(schema.to, 'missing to field');
    assert.ok(schema.amount, 'missing amount field');
  });

  it('set_exchange_rate has baseCurrency, quoteCurrency, rate, and source fields', () => {
    const schema = findTool('set_exchange_rate').inputSchema;
    assert.ok(schema.baseCurrency, 'missing baseCurrency field');
    assert.ok(schema.quoteCurrency, 'missing quoteCurrency field');
    assert.ok(schema.rate, 'missing rate field');
    assert.ok(schema.source, 'missing source field');
  });

  it('get_currency_settings has empty inputSchema', () => {
    const schema = findTool('get_currency_settings').inputSchema;
    assert.deepStrictEqual(schema, {});
  });

  it('set_base_currency has currency field', () => {
    const schema = findTool('set_base_currency').inputSchema;
    assert.ok(schema.currency, 'missing currency field');
  });

  it('enable_currencies has currencies field', () => {
    const schema = findTool('enable_currencies').inputSchema;
    assert.ok(schema.currencies, 'missing currencies field');
  });

  it('format_currency has amount and currency fields', () => {
    const schema = findTool('format_currency').inputSchema;
    assert.ok(schema.amount, 'missing amount field');
    assert.ok(schema.currency, 'missing currency field');
  });
});

// ============================================================================
// Handler: get_exchange_rate
// ============================================================================

describe('currencyTools — get_exchange_rate handler', () => {
  it('returns exchange rate with correct shape', async () => {
    const tool = findTool('get_exchange_rate');
    const result = await tool.handler({
      commerce: makeCurrencyCommerce(),
      params: { from: 'USD', to: 'EUR' },
    });
    assert.equal(result.success, true);
    assert.ok(result.rate);
    assert.equal(result.rate.baseCurrency, 'USD');
    assert.equal(result.rate.quoteCurrency, 'EUR');
    assert.equal(result.rate.rate, 0.92);
    assert.equal(result.rate.source, 'api');
  });

  it('returns success: false when rate is not found', async () => {
    const tool = findTool('get_exchange_rate');
    const result = await tool.handler({
      commerce: makeCurrencyCommerce({ getRate: async () => null }),
      params: { from: 'XYZ', to: 'ABC' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('XYZ'));
    assert.ok(result.error.includes('ABC'));
  });
});

// ============================================================================
// Handler: list_exchange_rates
// ============================================================================

describe('currencyTools — list_exchange_rates handler', () => {
  it('returns all rates when no baseCurrency given', async () => {
    const tool = findTool('list_exchange_rates');
    const result = await tool.handler({
      commerce: makeCurrencyCommerce(),
      params: {},
    });
    assert.equal(result.success, true);
    assert.equal(result.count, 1);
    assert.ok(Array.isArray(result.rates));
    assert.equal(result.rates[0].baseCurrency, 'USD');
  });

  it('filters by baseCurrency when provided', async () => {
    const tool = findTool('list_exchange_rates');
    const commerce = makeCurrencyCommerce({
      getRatesFor: async (base) => {
        assert.equal(base, 'EUR');
        return [{ ...mockRate, baseCurrency: 'EUR' }];
      },
    });
    const result = await tool.handler({
      commerce,
      params: { baseCurrency: 'EUR' },
    });
    assert.equal(result.success, true);
    assert.equal(result.rates[0].baseCurrency, 'EUR');
  });
});

// ============================================================================
// Handler: convert_currency
// ============================================================================

describe('currencyTools — convert_currency handler', () => {
  it('returns conversion with correct shape', async () => {
    const tool = findTool('convert_currency');
    const result = await tool.handler({
      commerce: makeCurrencyCommerce(),
      params: { from: 'USD', to: 'EUR', amount: 100 },
    });
    assert.equal(result.success, true);
    assert.ok(result.conversion);
    assert.equal(result.conversion.originalAmount, 100);
    assert.equal(result.conversion.originalCurrency, 'USD');
    assert.equal(result.conversion.convertedAmount, 92.0);
    assert.equal(result.conversion.targetCurrency, 'EUR');
    assert.equal(result.conversion.rate, 0.92);
    assert.equal(result.conversion.inverseRate, 1.087);
  });
});

// ============================================================================
// Handler: set_exchange_rate (write, requires --apply)
// ============================================================================

describe('currencyTools — set_exchange_rate handler', () => {
  it('returns apply-guard error when allowApply is false', async () => {
    const tool = findTool('set_exchange_rate');
    const result = await tool.handler({
      commerce: makeCurrencyCommerce(),
      params: { baseCurrency: 'USD', quoteCurrency: 'EUR', rate: 0.92, source: 'manual' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.preview);
    assert.equal(result.preview.baseCurrency, 'USD');
    assert.equal(result.preview.quoteCurrency, 'EUR');
    assert.equal(result.preview.rate, 0.92);
  });

  it('sets exchange rate when allowApply is true', async () => {
    const tool = findTool('set_exchange_rate');
    const result = await tool.handler({
      commerce: makeCurrencyCommerce(),
      params: { baseCurrency: 'USD', quoteCurrency: 'EUR', rate: 0.92, source: 'manual' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('Exchange rate set'));
    assert.ok(result.rate);
    assert.equal(result.rate.id, 'rate_001');
  });

  it('handles commerce error gracefully when allowApply is true', async () => {
    const tool = findTool('set_exchange_rate');
    try {
      await tool.handler({
        commerce: {},
        params: { baseCurrency: 'USD', quoteCurrency: 'EUR', rate: 0.92 },
        allowApply: true,
      });
      assert.fail('should have thrown');
    } catch (err) {
      assert.ok(err instanceof TypeError);
    }
  });
});

// ============================================================================
// Handler: get_currency_settings
// ============================================================================

describe('currencyTools — get_currency_settings handler', () => {
  it('returns settings with correct shape', async () => {
    const tool = findTool('get_currency_settings');
    const result = await tool.handler({
      commerce: makeCurrencyCommerce(),
      params: {},
    });
    assert.equal(result.success, true);
    assert.ok(result.settings);
    assert.equal(result.settings.baseCurrency, 'USD');
    assert.deepStrictEqual(result.settings.enabledCurrencies, ['USD', 'EUR', 'GBP']);
    assert.equal(result.settings.autoConvert, true);
    assert.equal(result.settings.roundingMode, 'half_up');
  });
});

// ============================================================================
// Handler: set_base_currency (write, requires --apply)
// ============================================================================

describe('currencyTools — set_base_currency handler', () => {
  it('returns apply-guard error when allowApply is false', async () => {
    const tool = findTool('set_base_currency');
    const result = await tool.handler({
      commerce: makeCurrencyCommerce(),
      params: { currency: 'EUR' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.preview);
    assert.equal(result.preview.baseCurrency, 'EUR');
  });

  it('sets base currency when allowApply is true', async () => {
    const tool = findTool('set_base_currency');
    const result = await tool.handler({
      commerce: makeCurrencyCommerce(),
      params: { currency: 'EUR' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('EUR'));
    assert.ok(result.settings);
    assert.equal(result.settings.baseCurrency, 'EUR');
  });
});

// ============================================================================
// Handler: enable_currencies (write, requires --apply)
// ============================================================================

describe('currencyTools — enable_currencies handler', () => {
  it('returns apply-guard error when allowApply is false', async () => {
    const tool = findTool('enable_currencies');
    const result = await tool.handler({
      commerce: makeCurrencyCommerce(),
      params: { currencies: ['USD', 'EUR', 'GBP'] },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.preview);
    assert.deepStrictEqual(result.preview.currencies, ['USD', 'EUR', 'GBP']);
  });

  it('enables currencies when allowApply is true', async () => {
    const tool = findTool('enable_currencies');
    const result = await tool.handler({
      commerce: makeCurrencyCommerce(),
      params: { currencies: ['USD', 'EUR', 'GBP', 'JPY'] },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('Enabled'));
    assert.ok(result.settings);
  });
});

// ============================================================================
// Handler: format_currency
// ============================================================================

describe('currencyTools — format_currency handler', () => {
  it('returns formatted amount', async () => {
    const tool = findTool('format_currency');
    const result = await tool.handler({
      commerce: makeCurrencyCommerce(),
      params: { amount: 99.99, currency: 'USD' },
    });
    assert.equal(result.success, true);
    assert.equal(result.amount, 99.99);
    assert.equal(result.currency, 'USD');
    assert.ok(result.formatted);
  });
});

// ============================================================================
// Error paths — commerce object missing methods
// ============================================================================

describe('currencyTools — error paths (empty commerce)', () => {
  const readTools = ['get_exchange_rate', 'list_exchange_rates', 'convert_currency',
    'get_currency_settings', 'format_currency'];

  for (const toolName of readTools) {
    it(`${toolName} throws TypeError when commerce.currency is missing`, async () => {
      const tool = findTool(toolName);
      try {
        await tool.handler({
          commerce: {},
          params: { from: 'USD', to: 'EUR', amount: 100, currency: 'USD' },
        });
        assert.fail('should have thrown');
      } catch (err) {
        assert.ok(err instanceof TypeError);
      }
    });
  }

  const writeTools = ['set_exchange_rate', 'set_base_currency', 'enable_currencies'];

  for (const toolName of writeTools) {
    it(`${toolName} throws TypeError when commerce.currency is missing and allowApply is true`, async () => {
      const tool = findTool(toolName);
      try {
        await tool.handler({
          commerce: {},
          params: { baseCurrency: 'USD', quoteCurrency: 'EUR', rate: 0.92, currency: 'EUR', currencies: ['EUR'] },
          allowApply: true,
        });
        assert.fail('should have thrown');
      } catch (err) {
        assert.ok(err instanceof TypeError);
      }
    });
  }
});

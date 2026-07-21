/**
 * Extended Commerce Tools Test Suite
 *
 * Tests for tool modules not covered by tools-commerce.test.js:
 * - currency.js
 * - tax.js
 * - manufacturing.js
 * - promotions.js
 * - subscriptions.js
 * - custom-objects.js
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { currencyTools } from '../../src/tools/currency.js';
import { taxTools } from '../../src/tools/tax.js';
import { manufacturingTools } from '../../src/tools/manufacturing.js';
import { promotionTools } from '../../src/tools/promotions.js';
import { subscriptionTools } from '../../src/tools/subscriptions.js';

// ============================================================================
// Helper: find tool by name from a tools array
// ============================================================================

function findTool(tools, name) {
  const tool = tools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// CURRENCY TOOLS
// ============================================================================

describe('Currency Tools', () => {
  const mockRate = {
    baseCurrency: 'USD',
    quoteCurrency: 'EUR',
    rate: 0.92,
    source: 'manual',
    rateAt: '2026-02-19T00:00:00Z',
  };

  function makeCurrencyCommerce(overrides = {}) {
    return {
      currency: {
        getRate: async () => mockRate,
        getRatesFor: async () => [mockRate],
        listRates: async () => [mockRate, { ...mockRate, quoteCurrency: 'GBP', rate: 0.79 }],
        convert: async ({ from, to, amount }) => ({
          originalAmount: amount,
          originalCurrency: from,
          convertedAmount: amount * 0.92,
          targetCurrency: to,
          rate: 0.92,
          inverseRate: 1.087,
          rateAt: '2026-02-19T00:00:00Z',
        }),
        setRate: async (data) => ({ id: 'rate_001', ...data, rateAt: '2026-02-19T00:00:00Z' }),
        getSettings: async () => ({
          baseCurrency: 'USD',
          enabledCurrencies: ['USD', 'EUR', 'GBP'],
          autoConvert: false,
          roundingMode: 'half_up',
        }),
        setBaseCurrency: async (currency) => ({
          baseCurrency: currency,
          enabledCurrencies: ['USD', 'EUR', 'GBP'],
        }),
        enableCurrencies: async (list) => ({
          baseCurrency: 'USD',
          enabledCurrencies: list,
        }),
        format: async (amount, currency) => `$${amount.toFixed(2)}`,
        ...overrides,
      },
    };
  }

  describe('get_exchange_rate', () => {
    const tool = findTool(currencyTools, 'get_exchange_rate');

    it('returns rate for valid pair', async () => {
      const result = await tool.handler({
        commerce: makeCurrencyCommerce(),
        params: { from: 'usd', to: 'eur' },
      });
      assert.equal(result.success, true);
      assert.equal(result.rate.baseCurrency, 'USD');
      assert.equal(result.rate.rate, 0.92);
    });

    it('uppercases currency codes', async () => {
      let calledWith = {};
      const commerce = makeCurrencyCommerce({
        getRate: async (from, to) => {
          calledWith = { from, to };
          return mockRate;
        },
      });
      await tool.handler({ commerce, params: { from: 'usd', to: 'eur' } });
      assert.equal(calledWith.from, 'USD');
      assert.equal(calledWith.to, 'EUR');
    });

    it('returns error when rate not found', async () => {
      const commerce = makeCurrencyCommerce({ getRate: async () => null });
      const result = await tool.handler({ commerce, params: { from: 'USD', to: 'XYZ' } });
      assert.equal(result.success, false);
      assert.ok(result.error.includes('No exchange rate'));
    });
  });

  describe('list_exchange_rates', () => {
    const tool = findTool(currencyTools, 'list_exchange_rates');

    it('lists all rates when no filter', async () => {
      const result = await tool.handler({
        commerce: makeCurrencyCommerce(),
        params: {},
      });
      assert.equal(result.success, true);
      assert.equal(result.count, 2);
      assert.equal(result.rates.length, 2);
    });

    it('filters by base currency', async () => {
      let calledBase = null;
      const commerce = makeCurrencyCommerce({
        getRatesFor: async (base) => {
          calledBase = base;
          return [mockRate];
        },
      });
      const result = await tool.handler({ commerce, params: { baseCurrency: 'usd' } });
      assert.equal(calledBase, 'USD');
      assert.equal(result.count, 1);
    });
  });

  describe('convert_currency', () => {
    const tool = findTool(currencyTools, 'convert_currency');

    it('converts amount between currencies', async () => {
      const result = await tool.handler({
        commerce: makeCurrencyCommerce(),
        params: { from: 'usd', to: 'eur', amount: 100 },
      });
      assert.equal(result.success, true);
      assert.equal(result.conversion.originalAmount, 100);
      assert.equal(result.conversion.convertedAmount, 92);
      assert.equal(result.conversion.rate, 0.92);
    });
  });

  describe('set_exchange_rate', () => {
    const tool = findTool(currencyTools, 'set_exchange_rate');

    it('returns preview without --apply', async () => {
      const result = await tool.handler({
        commerce: makeCurrencyCommerce(),
        params: { baseCurrency: 'USD', quoteCurrency: 'EUR', rate: 0.93, source: 'api' },
        allowApply: false,
      });
      assert.ok(result.error);
      assert.ok(result.preview);
    });

    it('sets rate with --apply', async () => {
      const result = await tool.handler({
        commerce: makeCurrencyCommerce(),
        params: { baseCurrency: 'USD', quoteCurrency: 'EUR', rate: 0.93, source: 'manual' },
        allowApply: true,
      });
      assert.equal(result.success, true);
      assert.ok(result.message.includes('Exchange rate set'));
      assert.equal(result.rate.rate, 0.93);
    });
  });

  describe('get_currency_settings', () => {
    const tool = findTool(currencyTools, 'get_currency_settings');

    it('returns store settings', async () => {
      const result = await tool.handler({ commerce: makeCurrencyCommerce(), params: {} });
      assert.equal(result.success, true);
      assert.equal(result.settings.baseCurrency, 'USD');
      assert.deepEqual(result.settings.enabledCurrencies, ['USD', 'EUR', 'GBP']);
    });
  });

  describe('set_base_currency', () => {
    const tool = findTool(currencyTools, 'set_base_currency');

    it('returns preview without --apply', async () => {
      const result = await tool.handler({
        commerce: makeCurrencyCommerce(),
        params: { currency: 'EUR' },
        allowApply: false,
      });
      assert.ok(result.error);
      assert.ok(result.preview);
    });

    it('sets base currency with --apply', async () => {
      const result = await tool.handler({
        commerce: makeCurrencyCommerce(),
        params: { currency: 'eur' },
        allowApply: true,
      });
      assert.equal(result.success, true);
      assert.ok(result.message.includes('EUR'));
    });
  });

  describe('enable_currencies', () => {
    const tool = findTool(currencyTools, 'enable_currencies');

    it('returns preview without --apply', async () => {
      const result = await tool.handler({
        commerce: makeCurrencyCommerce(),
        params: { currencies: ['USD', 'EUR', 'JPY'] },
        allowApply: false,
      });
      assert.ok(result.error);
      assert.ok(result.preview);
    });

    it('enables currencies with --apply', async () => {
      const result = await tool.handler({
        commerce: makeCurrencyCommerce(),
        params: { currencies: ['usd', 'eur', 'jpy'] },
        allowApply: true,
      });
      assert.equal(result.success, true);
      assert.deepEqual(result.settings.enabledCurrencies, ['USD', 'EUR', 'JPY']);
    });
  });

  describe('format_currency', () => {
    const tool = findTool(currencyTools, 'format_currency');

    it('formats amount', async () => {
      const result = await tool.handler({
        commerce: makeCurrencyCommerce(),
        params: { amount: 99.99, currency: 'usd' },
      });
      assert.equal(result.success, true);
      assert.equal(result.currency, 'USD');
    });
  });
});

// ============================================================================
// TAX TOOLS
// ============================================================================

describe('Tax Tools', () => {
  const mockTaxResult = {
    subtotal: 100,
    totalTax: 7.25,
    shippingTax: 0,
    total: 107.25,
    exemptionsApplied: [],
    taxBreakdown: [
      {
        jurisdictionName: 'California',
        taxType: 'sales_tax',
        rateName: 'State Sales Tax',
        rate: 0.0725,
        taxableAmount: 100,
        taxAmount: 7.25,
      },
    ],
    lineItemTaxes: [
      {
        lineItemId: 'item-1',
        taxableAmount: 100,
        taxAmount: 7.25,
        effectiveRate: 0.0725,
        isExempt: false,
      },
    ],
  };

  function makeTaxCommerce(overrides = {}) {
    return {
      tax: {
        calculate: async () => mockTaxResult,
        getEffectiveRate: async () => 0.0725,
        listJurisdictions: async () => [
          {
            id: 'j1',
            code: 'US-CA',
            name: 'California',
            level: 'state',
            countryCode: 'US',
            stateCode: 'CA',
          },
        ],
        listRates: async () => [
          {
            id: 'r1',
            jurisdictionId: 'j1',
            taxType: 'sales_tax',
            productCategory: 'standard',
            rate: 0.0725,
            name: 'CA Sales Tax',
            isCompound: false,
            effectiveFrom: '2026-01-01',
          },
        ],
        getSettings: async () => ({
          enabled: true,
          calculationMethod: 'tax_exclusive',
          compoundMethod: 'additive',
          taxShipping: false,
          taxHandling: false,
          defaultProductCategory: 'standard',
          roundingMode: 'half_up',
          decimalPlaces: 2,
          taxProvider: 'internal',
        }),
        getCustomerExemptions: async () => [
          {
            id: 'e1',
            exemptionType: 'resale',
            certificateNumber: 'RS-12345',
            issuingAuthority: 'California',
            effectiveFrom: '2026-01-01',
            expiresAt: '2027-01-01',
            verified: true,
          },
        ],
        createExemption: async (data) => ({ id: 'e2', ...data }),
        ...overrides,
      },
      calculateCartTax: async () => ({
        subtotal: 50,
        totalTax: 3.63,
        total: 53.63,
        taxInclusive: false,
        taxBreakdown: [{ jurisdictionName: 'California', rate: 0.0725, taxAmount: 3.63 }],
        lineItemTaxes: [{ lineItemId: 'li-1', subtotal: 50, taxAmount: 3.63, total: 53.63 }],
      }),
    };
  }

  describe('calculate_tax', () => {
    const tool = findTool(taxTools, 'calculate_tax');

    it('calculates tax for items with shipping address', async () => {
      const result = await tool.handler({
        commerce: makeTaxCommerce(),
        params: {
          items: [{ id: 'item-1', unitPrice: 50, quantity: 2, taxCategory: 'standard' }],
          shippingAddress: { country: 'US', state: 'CA' },
        },
      });
      assert.equal(result.success, true);
      assert.equal(result.calculation.subtotal, 100);
      assert.equal(result.calculation.totalTax, 7.25);
      assert.equal(result.calculation.total, 107.25);
      assert.equal(result.calculation.taxBreakdown.length, 1);
      assert.equal(result.calculation.lineItemTaxes.length, 1);
    });
  });

  describe('get_tax_rate', () => {
    const tool = findTool(taxTools, 'get_tax_rate');

    it('returns effective rate for address', async () => {
      const result = await tool.handler({
        commerce: makeTaxCommerce(),
        params: { country: 'US', state: 'CA' },
      });
      assert.equal(result.success, true);
      assert.equal(result.effectiveRate, 0.0725);
      assert.equal(result.effectiveRatePercent, '7.25%');
    });
  });

  describe('list_tax_jurisdictions', () => {
    const tool = findTool(taxTools, 'list_tax_jurisdictions');

    it('lists jurisdictions', async () => {
      const result = await tool.handler({
        commerce: makeTaxCommerce(),
        params: {},
      });
      assert.equal(result.success, true);
      assert.equal(result.count, 1);
      assert.equal(result.jurisdictions[0].code, 'US-CA');
    });
  });

  describe('list_tax_rates', () => {
    const tool = findTool(taxTools, 'list_tax_rates');

    it('lists rates', async () => {
      const result = await tool.handler({
        commerce: makeTaxCommerce(),
        params: {},
      });
      assert.equal(result.success, true);
      assert.equal(result.count, 1);
      assert.equal(result.rates[0].ratePercent, '7.25%');
    });
  });

  describe('get_tax_settings', () => {
    const tool = findTool(taxTools, 'get_tax_settings');

    it('returns settings', async () => {
      const result = await tool.handler({ commerce: makeTaxCommerce(), params: {} });
      assert.equal(result.success, true);
      assert.equal(result.settings.enabled, true);
      assert.equal(result.settings.taxProvider, 'internal');
    });
  });

  describe('get_us_state_tax_info', () => {
    const tool = findTool(taxTools, 'get_us_state_tax_info');

    it('returns CA tax info', async () => {
      const result = await tool.handler({ params: { stateCode: 'CA' } });
      assert.equal(result.success, true);
      assert.equal(result.stateInfo.stateCode, 'CA');
      assert.equal(result.stateInfo.stateRate, 0.0725);
      assert.equal(result.stateInfo.stateRatePercent, '7.25%');
    });

    it('handles lowercase state code', async () => {
      const result = await tool.handler({ params: { stateCode: 'ny' } });
      assert.equal(result.success, true);
      assert.equal(result.stateInfo.stateCode, 'NY');
    });

    it('returns zero-tax states', async () => {
      for (const state of ['OR', 'DE', 'MT', 'NH']) {
        const result = await tool.handler({ params: { stateCode: state } });
        assert.equal(result.success, true);
        assert.equal(result.stateInfo.stateRate, 0, `Expected ${state} to have 0% rate`);
      }
    });

    it('returns error for unknown state', async () => {
      const result = await tool.handler({ params: { stateCode: 'ZZ' } });
      assert.equal(result.success, false);
      assert.ok(result.error.includes('not found'));
    });
  });

  describe('get_customer_tax_exemptions', () => {
    const tool = findTool(taxTools, 'get_customer_tax_exemptions');

    it('returns exemptions', async () => {
      const result = await tool.handler({
        commerce: makeTaxCommerce(),
        params: { customerId: 'c1' },
      });
      assert.equal(result.success, true);
      assert.equal(result.count, 1);
      assert.equal(result.exemptions[0].exemptionType, 'resale');
    });
  });

  describe('create_tax_exemption', () => {
    const tool = findTool(taxTools, 'create_tax_exemption');

    it('returns preview without --apply', async () => {
      const result = await tool.handler({
        commerce: makeTaxCommerce(),
        params: { customerId: 'c1', exemptionType: 'resale' },
        allowApply: false,
      });
      assert.ok(result.error);
      assert.ok(result.preview);
    });

    it('creates exemption with --apply', async () => {
      const result = await tool.handler({
        commerce: makeTaxCommerce(),
        params: { customerId: 'c1', exemptionType: 'resale', certificateNumber: 'RS-999' },
        allowApply: true,
      });
      assert.equal(result.success, true);
      assert.ok(result.exemption.id);
    });
  });

  describe('calculate_cart_tax', () => {
    const tool = findTool(taxTools, 'calculate_cart_tax');

    it('calculates tax for cart', async () => {
      const result = await tool.handler({
        commerce: makeTaxCommerce(),
        params: { cartId: 'cart-1' },
      });
      assert.equal(result.success, true);
      assert.equal(result.cartId, 'cart-1');
      assert.equal(result.tax.totalTax, 3.63);
      assert.equal(result.tax.total, 53.63);
      assert.equal(result.lineItems.length, 1);
    });
  });
});

// ============================================================================
// MANUFACTURING TOOLS
// ============================================================================

describe('Manufacturing Tools', () => {
  const mockBom = {
    id: 'bom-1',
    bomNumber: 'BOM-001',
    name: 'Widget Assembly',
    productId: 'prod-1',
    status: 'draft',
    revision: 'A',
    createdAt: '2026-02-01',
  };

  const mockWo = {
    id: 'wo-1',
    woNumber: 'WO-001',
    bomId: 'bom-1',
    status: 'draft',
    quantity: 100,
    createdAt: '2026-02-01',
  };

  function makeMfgCommerce(bomOverrides = {}, woOverrides = {}) {
    return {
      bom: {
        list: async () => [mockBom],
        count: async () => 1,
        get: async (id) => (id === 'bom-1' ? mockBom : null),
        getComponents: async () => [{ id: 'comp-1', name: 'Screw', quantity: 4, unit: 'piece' }],
        create: async (data) => ({ id: 'bom-2', bomNumber: 'BOM-002', status: 'draft', ...data }),
        addComponent: async (bomId, data) => ({ id: 'comp-2', bomId, ...data }),
        activate: async (id) => ({ id, status: 'active' }),
        ...bomOverrides,
      },
      workOrders: {
        list: async () => [mockWo],
        count: async () => 1,
        get: async (id) => (id === 'wo-1' ? mockWo : null),
        create: async (data) => ({ id: 'wo-2', woNumber: 'WO-002', status: 'draft', ...data }),
        start: async (id) => ({ id, status: 'in_progress' }),
        complete: async (id, qty) => ({ id, status: 'completed', quantityCompleted: qty }),
        cancel: async (id) => ({ id, status: 'cancelled' }),
        ...woOverrides,
      },
    };
  }

  describe('list_boms', () => {
    const tool = findTool(manufacturingTools, 'list_boms');

    it('returns BOMs with count', async () => {
      const result = await tool.handler({ commerce: makeMfgCommerce(), params: {} });
      assert.equal(result.success, true);
      assert.equal(result.count, 1);
      assert.equal(result.boms[0].bomNumber, 'BOM-001');
    });
  });

  describe('get_bom', () => {
    const tool = findTool(manufacturingTools, 'get_bom');

    it('returns BOM with components', async () => {
      const result = await tool.handler({
        commerce: makeMfgCommerce(),
        params: { bomId: 'bom-1' },
      });
      assert.equal(result.success, true);
      assert.equal(result.bom.name, 'Widget Assembly');
      assert.equal(result.bom.components.length, 1);
    });

    it('returns error for missing BOM', async () => {
      const result = await tool.handler({
        commerce: makeMfgCommerce(),
        params: { bomId: 'nonexistent' },
      });
      assert.ok(result.error);
    });
  });

  describe('create_bom', () => {
    const tool = findTool(manufacturingTools, 'create_bom');

    it('returns preview without --apply', async () => {
      const result = await tool.handler({
        commerce: makeMfgCommerce(),
        params: { name: 'New BOM', productId: 'prod-1' },
        allowApply: false,
      });
      assert.ok(result.error);
      assert.ok(result.wouldCreate);
    });

    it('creates BOM with --apply', async () => {
      const result = await tool.handler({
        commerce: makeMfgCommerce(),
        params: { name: 'New BOM', productId: 'prod-1' },
        allowApply: true,
      });
      assert.equal(result.success, true);
      assert.equal(result.bom.bomNumber, 'BOM-002');
    });
  });

  describe('add_bom_component', () => {
    const tool = findTool(manufacturingTools, 'add_bom_component');

    it('returns preview without --apply', async () => {
      const result = await tool.handler({
        commerce: makeMfgCommerce(),
        params: { bomId: 'bom-1', name: 'Nut', quantity: 8, unit: 'piece' },
        allowApply: false,
      });
      assert.ok(result.error);
    });

    it('adds component with --apply', async () => {
      const result = await tool.handler({
        commerce: makeMfgCommerce(),
        params: { bomId: 'bom-1', name: 'Nut', quantity: 8, unit: 'piece' },
        allowApply: true,
      });
      assert.equal(result.success, true);
    });
  });

  describe('activate_bom', () => {
    const tool = findTool(manufacturingTools, 'activate_bom');

    it('returns preview without --apply', async () => {
      const result = await tool.handler({
        commerce: makeMfgCommerce(),
        params: { bomId: 'bom-1' },
        allowApply: false,
      });
      assert.ok(result.error);
    });

    it('activates with --apply', async () => {
      const result = await tool.handler({
        commerce: makeMfgCommerce(),
        params: { bomId: 'bom-1' },
        allowApply: true,
      });
      assert.equal(result.success, true);
      assert.equal(result.bom.status, 'active');
    });
  });

  describe('list_work_orders', () => {
    const tool = findTool(manufacturingTools, 'list_work_orders');

    it('returns work orders', async () => {
      const result = await tool.handler({ commerce: makeMfgCommerce(), params: {} });
      assert.equal(result.success, true);
      assert.equal(result.count, 1);
    });
  });

  describe('get_work_order', () => {
    const tool = findTool(manufacturingTools, 'get_work_order');

    it('returns work order', async () => {
      const result = await tool.handler({
        commerce: makeMfgCommerce(),
        params: { workOrderId: 'wo-1' },
      });
      assert.equal(result.success, true);
      assert.equal(result.workOrder.status, 'draft');
    });

    it('returns error for missing', async () => {
      const result = await tool.handler({
        commerce: makeMfgCommerce(),
        params: { workOrderId: 'nope' },
      });
      assert.ok(result.error);
    });
  });

  describe('create_work_order', () => {
    const tool = findTool(manufacturingTools, 'create_work_order');

    it('returns preview without --apply', async () => {
      const result = await tool.handler({
        commerce: makeMfgCommerce(),
        params: { bomId: 'bom-1', quantity: 50 },
        allowApply: false,
      });
      assert.ok(result.error);
    });

    it('creates with --apply', async () => {
      const result = await tool.handler({
        commerce: makeMfgCommerce(),
        params: { bomId: 'bom-1', quantity: 50 },
        allowApply: true,
      });
      assert.equal(result.success, true);
    });
  });

  describe('start_work_order', () => {
    const tool = findTool(manufacturingTools, 'start_work_order');

    it('returns preview without --apply', async () => {
      const result = await tool.handler({
        commerce: makeMfgCommerce(),
        params: { workOrderId: 'wo-1' },
        allowApply: false,
      });
      assert.ok(result.error);
    });

    it('starts with --apply', async () => {
      const result = await tool.handler({
        commerce: makeMfgCommerce(),
        params: { workOrderId: 'wo-1' },
        allowApply: true,
      });
      assert.equal(result.success, true);
      assert.equal(result.workOrder.status, 'in_progress');
    });
  });

  describe('complete_work_order', () => {
    const tool = findTool(manufacturingTools, 'complete_work_order');

    it('completes with --apply', async () => {
      const result = await tool.handler({
        commerce: makeMfgCommerce(),
        params: { workOrderId: 'wo-1', quantityCompleted: 95 },
        allowApply: true,
      });
      assert.equal(result.success, true);
    });
  });

  describe('cancel_work_order', () => {
    const tool = findTool(manufacturingTools, 'cancel_work_order');

    it('cancels with --apply', async () => {
      const result = await tool.handler({
        commerce: makeMfgCommerce(),
        params: { workOrderId: 'wo-1' },
        allowApply: true,
      });
      assert.equal(result.success, true);
      assert.equal(result.workOrder.status, 'cancelled');
    });
  });
});

// ============================================================================
// PROMOTION TOOLS
// ============================================================================

describe('Promotion Tools', () => {
  const mockPromo = {
    id: 'promo-1',
    code: 'SUMMER25',
    name: 'Summer Sale',
    description: '25% off everything',
    promotionType: 'percentage_off',
    status: 'active',
    trigger: 'automatic',
    target: 'order',
    percentageOff: 25,
    fixedAmountOff: null,
    maxDiscountAmount: 100,
    startsAt: '2026-06-01',
    endsAt: '2026-08-31',
    usageCount: 42,
  };

  function makePromoCommerce(overrides = {}) {
    const promoMethods = {
      list: async () => [mockPromo],
      get: async (id) => (id === 'promo-1' ? mockPromo : null),
      getByCode: async (code) => (code === 'SUMMER25' ? mockPromo : null),
      create: async (data) => ({ id: 'promo-2', status: 'draft', ...data }),
      activate: async (id) => ({ ...mockPromo, id, status: 'active' }),
      deactivate: async (id) => ({ ...mockPromo, id, status: 'paused' }),
      createCoupon: async (data) => ({ id: 'coupon-1', ...data }),
      validateCoupon: async (code) => ({
        valid: true,
        code,
        promotionId: 'promo-1',
        discountType: 'percentage_off',
        value: 25,
      }),
      listCoupons: async () => [
        { id: 'coupon-1', code: 'SAVE25', promotionId: 'promo-1', usageCount: 5 },
      ],
      getActive: async () => [mockPromo],
      applyToCart: async (cartId) => ({
        cartId,
        applied: [{ promotionId: 'promo-1', discount: 25 }],
        totalDiscount: 25,
      }),
      ...overrides,
    };
    return {
      promotions: () => promoMethods,
    };
  }

  describe('list_promotions', () => {
    const tool = findTool(promotionTools, 'list_promotions');

    it('lists all promotions', async () => {
      const result = await tool.handler({ commerce: makePromoCommerce(), params: {} });
      assert.equal(result.success, true);
      assert.equal(result.count, 1);
      assert.equal(result.promotions[0].name, 'Summer Sale');
    });

    it('passes filter params', async () => {
      let calledFilter;
      const commerce = makePromoCommerce({
        list: async (filter) => {
          calledFilter = filter;
          return [];
        },
      });
      await tool.handler({ commerce, params: { status: 'active', type: 'percentage_off' } });
      assert.equal(calledFilter.status, 'active');
      assert.equal(calledFilter.promotionType, 'percentage_off');
    });
  });

  describe('get_promotion', () => {
    const tool = findTool(promotionTools, 'get_promotion');

    it('gets by ID', async () => {
      const result = await tool.handler({
        commerce: makePromoCommerce(),
        params: { identifier: 'promo-1' },
      });
      assert.equal(result.success, true);
      assert.equal(result.promotion.name, 'Summer Sale');
    });

    it('falls back to code lookup', async () => {
      const commerce = makePromoCommerce({
        get: async () => {
          throw new Error('not found');
        },
      });
      const result = await tool.handler({ commerce, params: { identifier: 'SUMMER25' } });
      assert.equal(result.success, true);
      assert.equal(result.promotion.code, 'SUMMER25');
    });

    it('returns error when not found', async () => {
      const commerce = makePromoCommerce({
        get: async () => {
          throw new Error('not found');
        },
        getByCode: async () => null,
      });
      const result = await tool.handler({ commerce, params: { identifier: 'NOPE' } });
      assert.ok(result.error);
    });
  });

  describe('create_promotion', () => {
    const tool = findTool(promotionTools, 'create_promotion');

    it('returns preview without --apply', async () => {
      const result = await tool.handler({
        commerce: makePromoCommerce(),
        params: { name: 'New Promo', promotionType: 'percentage_off', percentageOff: 10 },
        allowApply: false,
      });
      assert.ok(result.error);
    });

    it('creates with --apply', async () => {
      const result = await tool.handler({
        commerce: makePromoCommerce(),
        params: { name: 'New Promo', promotionType: 'percentage_off', percentageOff: 10 },
        allowApply: true,
      });
      assert.equal(result.success, true);
    });
  });

  describe('activate_promotion', () => {
    const tool = findTool(promotionTools, 'activate_promotion');

    it('activates with --apply', async () => {
      const result = await tool.handler({
        commerce: makePromoCommerce(),
        params: { promotionId: 'promo-1' },
        allowApply: true,
      });
      assert.equal(result.success, true);
      assert.equal(result.promotion.status, 'active');
    });
  });

  describe('deactivate_promotion', () => {
    const tool = findTool(promotionTools, 'deactivate_promotion');

    it('deactivates with --apply', async () => {
      const result = await tool.handler({
        commerce: makePromoCommerce(),
        params: { promotionId: 'promo-1' },
        allowApply: true,
      });
      assert.equal(result.success, true);
      assert.equal(result.promotion.status, 'paused');
    });
  });

  describe('validate_coupon', () => {
    const tool = findTool(promotionTools, 'validate_coupon');

    it('validates valid coupon', async () => {
      const result = await tool.handler({
        commerce: makePromoCommerce(),
        params: { code: 'save25' },
      });
      assert.equal(result.success, true);
      assert.equal(result.valid, true);
    });
  });

  describe('list_coupons', () => {
    const tool = findTool(promotionTools, 'list_coupons');

    it('lists coupons', async () => {
      const result = await tool.handler({
        commerce: makePromoCommerce(),
        params: {},
      });
      assert.equal(result.success, true);
      assert.equal(result.count, 1);
    });
  });

  describe('get_active_promotions', () => {
    const tool = findTool(promotionTools, 'get_active_promotions');

    it('returns active promotions', async () => {
      const result = await tool.handler({
        commerce: makePromoCommerce(),
        params: {},
      });
      assert.equal(result.success, true);
      assert.equal(result.count, 1);
    });
  });
});

// ============================================================================
// SUBSCRIPTION TOOLS
// ============================================================================

describe('Subscription Tools', () => {
  const mockPlan = {
    id: 'plan-1',
    code: 'COFFEE-MONTHLY',
    name: 'Coffee Club',
    status: 'active',
    billingInterval: 'monthly',
    price: 29.99,
    currency: 'USD',
    trialDays: 14,
  };

  const mockSub = {
    id: 'sub-1',
    planId: 'plan-1',
    customerId: 'cust-1',
    status: 'active',
    currentPeriodStart: '2026-02-01',
    currentPeriodEnd: '2026-03-01',
    nextBillingDate: '2026-03-01',
  };

  function makeSubCommerce(overrides = {}) {
    return {
      listSubscriptionPlans: async () => [mockPlan],
      getSubscriptionPlan: async (id) => (id === 'plan-1' ? mockPlan : null),
      createSubscriptionPlan: async (data) => ({
        id: 'plan-2',
        status: 'draft',
        code: 'NEW',
        ...data,
      }),
      activateSubscriptionPlan: async (id) => ({ ...mockPlan, id, status: 'active' }),
      archiveSubscriptionPlan: async (id) => ({ ...mockPlan, id, status: 'archived' }),
      listSubscriptions: async () => [mockSub],
      getSubscription: async (id) => (id === 'sub-1' ? mockSub : null),
      createSubscription: async (data) => ({ id: 'sub-2', status: 'active', ...data }),
      pauseSubscription: async (id) => ({ ...mockSub, id, status: 'paused' }),
      resumeSubscription: async (id) => ({ ...mockSub, id, status: 'active' }),
      cancelSubscription: async (id) => ({ ...mockSub, id, status: 'cancelled' }),
      skipBillingCycle: async (id) => ({
        ...mockSub,
        id,
        message: 'Next billing cycle skipped',
      }),
      listBillingCycles: async () => [
        { id: 'bc-1', subscriptionId: 'sub-1', status: 'paid', amount: 29.99 },
      ],
      getBillingCycle: async (id) => ({
        id,
        subscriptionId: 'sub-1',
        status: 'paid',
        amount: 29.99,
      }),
      getSubscriptionEvents: async () => [
        { id: 'ev-1', type: 'subscription.created', timestamp: '2026-02-01' },
      ],
      ...overrides,
    };
  }

  describe('list_subscription_plans', () => {
    const tool = findTool(subscriptionTools, 'list_subscription_plans');

    it('lists plans', async () => {
      const result = await tool.handler({ commerce: makeSubCommerce(), params: {} });
      assert.equal(result.count, 1);
      assert.equal(result.plans[0].name, 'Coffee Club');
    });
  });

  describe('get_subscription_plan', () => {
    const tool = findTool(subscriptionTools, 'get_subscription_plan');

    it('returns plan by ID', async () => {
      const result = await tool.handler({
        commerce: makeSubCommerce(),
        params: { planId: 'plan-1' },
      });
      assert.strictEqual(result.success, true);
      assert.equal(result.plan.name, 'Coffee Club');
    });

    it('returns error for unknown plan', async () => {
      const result = await tool.handler({
        commerce: makeSubCommerce(),
        params: { planId: 'nope' },
      });
      assert.strictEqual(result.success, false);
      assert.ok(result.error);
    });
  });

  describe('create_subscription_plan', () => {
    const tool = findTool(subscriptionTools, 'create_subscription_plan');

    it('returns preview without --apply', async () => {
      const result = await tool.handler({
        commerce: makeSubCommerce(),
        params: { name: 'Pro Plan', billingInterval: 'monthly', price: 49.99 },
        allowApply: false,
      });
      assert.ok(result.error);
      assert.ok(result.wouldCreate);
    });

    it('creates with --apply', async () => {
      const result = await tool.handler({
        commerce: makeSubCommerce(),
        params: { name: 'Pro Plan', billingInterval: 'monthly', price: 49.99 },
        allowApply: true,
      });
      assert.equal(result.success, true);
    });
  });

  describe('activate_subscription_plan', () => {
    const tool = findTool(subscriptionTools, 'activate_subscription_plan');

    it('activates with --apply', async () => {
      const result = await tool.handler({
        commerce: makeSubCommerce(),
        params: { planId: 'plan-1' },
        allowApply: true,
      });
      assert.equal(result.success, true);
    });
  });

  describe('archive_subscription_plan', () => {
    const tool = findTool(subscriptionTools, 'archive_subscription_plan');

    it('archives with --apply', async () => {
      const result = await tool.handler({
        commerce: makeSubCommerce(),
        params: { planId: 'plan-1' },
        allowApply: true,
      });
      assert.equal(result.success, true);
    });
  });

  describe('list_subscriptions', () => {
    const tool = findTool(subscriptionTools, 'list_subscriptions');

    it('lists subscriptions', async () => {
      const result = await tool.handler({ commerce: makeSubCommerce(), params: {} });
      assert.equal(result.count, 1);
    });
  });

  describe('get_subscription', () => {
    const tool = findTool(subscriptionTools, 'get_subscription');

    it('returns subscription', async () => {
      const result = await tool.handler({
        commerce: makeSubCommerce(),
        params: { subscriptionId: 'sub-1' },
      });
      assert.equal(result.id, 'sub-1');
    });

    it('returns error for missing', async () => {
      const result = await tool.handler({
        commerce: makeSubCommerce(),
        params: { subscriptionId: 'nope' },
      });
      assert.ok(result.error);
    });
  });

  describe('create_subscription', () => {
    const tool = findTool(subscriptionTools, 'create_subscription');

    it('returns preview without --apply', async () => {
      const result = await tool.handler({
        commerce: makeSubCommerce(),
        params: { customerId: 'cust-1', planId: 'plan-1' },
        allowApply: false,
      });
      assert.ok(result.error);
    });

    it('creates with --apply', async () => {
      const result = await tool.handler({
        commerce: makeSubCommerce(),
        params: { customerId: 'cust-1', planId: 'plan-1' },
        allowApply: true,
      });
      assert.equal(result.success, true);
    });
  });

  describe('pause_subscription', () => {
    const tool = findTool(subscriptionTools, 'pause_subscription');

    it('pauses with --apply', async () => {
      const result = await tool.handler({
        commerce: makeSubCommerce(),
        params: { subscriptionId: 'sub-1' },
        allowApply: true,
      });
      assert.equal(result.success, true);
      assert.equal(result.subscription.status, 'paused');
    });
  });

  describe('resume_subscription', () => {
    const tool = findTool(subscriptionTools, 'resume_subscription');

    it('resumes with --apply', async () => {
      const result = await tool.handler({
        commerce: makeSubCommerce(),
        params: { subscriptionId: 'sub-1' },
        allowApply: true,
      });
      assert.equal(result.success, true);
    });
  });

  describe('cancel_subscription', () => {
    const tool = findTool(subscriptionTools, 'cancel_subscription');

    it('cancels with --apply', async () => {
      const result = await tool.handler({
        commerce: makeSubCommerce(),
        params: { subscriptionId: 'sub-1' },
        allowApply: true,
      });
      assert.equal(result.success, true);
      assert.equal(result.subscription.status, 'cancelled');
    });
  });

  describe('skip_billing_cycle', () => {
    const tool = findTool(subscriptionTools, 'skip_billing_cycle');

    it('skips with --apply', async () => {
      const result = await tool.handler({
        commerce: makeSubCommerce(),
        params: { subscriptionId: 'sub-1' },
        allowApply: true,
      });
      assert.equal(result.success, true);
    });
  });

  describe('list_billing_cycles', () => {
    const tool = findTool(subscriptionTools, 'list_billing_cycles');

    it('lists cycles', async () => {
      const result = await tool.handler({
        commerce: makeSubCommerce(),
        params: { subscriptionId: 'sub-1' },
      });
      assert.equal(result.count, 1);
    });
  });

  describe('get_billing_cycle', () => {
    const tool = findTool(subscriptionTools, 'get_billing_cycle');

    it('returns cycle', async () => {
      const result = await tool.handler({
        commerce: makeSubCommerce(),
        params: { cycleId: 'bc-1' },
      });
      assert.equal(result.status, 'paid');
    });
  });

  describe('get_subscription_events', () => {
    const tool = findTool(subscriptionTools, 'get_subscription_events');

    it('returns events', async () => {
      const result = await tool.handler({
        commerce: makeSubCommerce(),
        params: { subscriptionId: 'sub-1' },
      });
      assert.equal(result.count, 1);
    });
  });
});

// ============================================================================
// Structural sanity checks
// ============================================================================

describe('Tool module structure', () => {
  const modules = [
    { name: 'currency', tools: currencyTools, expectedMin: 8 },
    { name: 'tax', tools: taxTools, expectedMin: 9 },
    { name: 'manufacturing', tools: manufacturingTools, expectedMin: 11 },
    { name: 'promotions', tools: promotionTools, expectedMin: 10 },
    { name: 'subscriptions', tools: subscriptionTools, expectedMin: 15 },
  ];

  for (const { name, tools, expectedMin } of modules) {
    it(`${name} exports at least ${expectedMin} tools`, () => {
      assert.ok(Array.isArray(tools), `${name} should export an array`);
      assert.ok(
        tools.length >= expectedMin,
        `${name} has ${tools.length} tools, expected >= ${expectedMin}`,
      );
    });

    it(`${name} tools have name, handler, permission`, () => {
      for (const tool of tools) {
        assert.ok(tool.name, `tool in ${name} missing name`);
        assert.ok(typeof tool.handler === 'function', `${tool.name} missing handler`);
        assert.ok(tool.permission, `${tool.name} missing permission`);
      }
    });
  }
});

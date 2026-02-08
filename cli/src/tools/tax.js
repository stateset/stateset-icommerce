/**
 * Tax Calculation Tools Module
 */

import { z } from 'zod';

const US_STATE_TAX_INFO = {
  CA: {
    stateCode: 'CA',
    stateName: 'California',
    stateRate: 0.0725,
    hasLocalTaxes: true,
    originBased: true,
    taxShipping: false,
    taxClothing: true,
    taxFood: false,
  },
  TX: {
    stateCode: 'TX',
    stateName: 'Texas',
    stateRate: 0.0625,
    hasLocalTaxes: true,
    originBased: true,
    taxShipping: true,
    taxClothing: true,
    taxFood: false,
  },
  NY: {
    stateCode: 'NY',
    stateName: 'New York',
    stateRate: 0.04,
    hasLocalTaxes: true,
    originBased: false,
    taxShipping: true,
    taxClothing: false,
    taxFood: false,
  },
  FL: {
    stateCode: 'FL',
    stateName: 'Florida',
    stateRate: 0.06,
    hasLocalTaxes: true,
    originBased: false,
    taxShipping: true,
    taxClothing: true,
    taxFood: false,
  },
  WA: {
    stateCode: 'WA',
    stateName: 'Washington',
    stateRate: 0.065,
    hasLocalTaxes: true,
    originBased: false,
    taxShipping: true,
    taxClothing: true,
    taxFood: false,
  },
  OR: {
    stateCode: 'OR',
    stateName: 'Oregon',
    stateRate: 0,
    hasLocalTaxes: false,
    originBased: false,
    taxShipping: false,
    taxClothing: false,
    taxFood: false,
  },
  DE: {
    stateCode: 'DE',
    stateName: 'Delaware',
    stateRate: 0,
    hasLocalTaxes: false,
    originBased: false,
    taxShipping: false,
    taxClothing: false,
    taxFood: false,
  },
  MT: {
    stateCode: 'MT',
    stateName: 'Montana',
    stateRate: 0,
    hasLocalTaxes: false,
    originBased: false,
    taxShipping: false,
    taxClothing: false,
    taxFood: false,
  },
  NH: {
    stateCode: 'NH',
    stateName: 'New Hampshire',
    stateRate: 0,
    hasLocalTaxes: false,
    originBased: false,
    taxShipping: false,
    taxClothing: false,
    taxFood: false,
  },
  AK: {
    stateCode: 'AK',
    stateName: 'Alaska',
    stateRate: 0,
    hasLocalTaxes: true,
    originBased: false,
    taxShipping: false,
    taxClothing: false,
    taxFood: false,
  },
};

export const taxTools = [
  {
    name: 'calculate_tax',
    description:
      'Calculate tax for a transaction based on shipping address and line items. Supports US sales tax, EU VAT, and Canadian GST/HST/PST.',
    inputSchema: {
      items: z
        .array(
          z.object({
            id: z.string().describe('Line item identifier'),
            unitPrice: z.number().describe('Unit price per item'),
            quantity: z.number().describe('Quantity of items'),
            taxCategory: z
              .string()
              .optional()
              .default('standard')
              .describe(
                'Tax category: standard, reduced, exempt, digital, food, clothing, medical',
              ),
          }),
        )
        .describe('Line items to calculate tax for'),
      shippingAddress: z
        .object({
          country: z.string().describe('Country code (e.g., US, DE, CA)'),
          state: z.string().optional().describe('State/Province code (e.g., CA, TX, ON)'),
          city: z.string().optional().describe('City name'),
          postalCode: z.string().optional().describe('Postal/ZIP code'),
        })
        .describe('Shipping address for tax jurisdiction determination'),
      shippingAmount: z.number().optional().describe('Shipping amount (may be taxable)'),
      customerId: z.string().optional().describe('Customer ID for exemption lookup'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { items, shippingAddress, shippingAmount, customerId } = params;
      const result = await commerce.tax.calculate({
        lineItems: items.map((item) => ({
          id: item.id,
          unitPrice: item.unitPrice,
          quantity: item.quantity,
          discountAmount: 0,
          taxCategory: item.taxCategory || 'standard',
        })),
        shippingAddress: {
          country: shippingAddress.country,
          state: shippingAddress.state,
          city: shippingAddress.city,
          postalCode: shippingAddress.postalCode,
        },
        shippingAmount,
        customerId,
      });
      return {
        success: true,
        calculation: {
          subtotal: result.subtotal,
          totalTax: result.totalTax,
          shippingTax: result.shippingTax,
          total: result.total,
          exemptionsApplied: result.exemptionsApplied,
          taxBreakdown: result.taxBreakdown.map((b) => ({
            jurisdictionName: b.jurisdictionName,
            taxType: b.taxType,
            rateName: b.rateName,
            rate: b.rate,
            taxableAmount: b.taxableAmount,
            taxAmount: b.taxAmount,
          })),
          lineItemTaxes: result.lineItemTaxes.map((lit) => ({
            lineItemId: lit.lineItemId,
            taxableAmount: lit.taxableAmount,
            taxAmount: lit.taxAmount,
            effectiveRate: lit.effectiveRate,
            isExempt: lit.isExempt,
          })),
        },
      };
    },
  },
  {
    name: 'get_tax_rate',
    description: 'Get the effective tax rate for a shipping address and product category.',
    inputSchema: {
      country: z.string().describe('Country code (e.g., US, DE, CA)'),
      state: z.string().optional().describe('State/Province code (e.g., CA, TX, ON)'),
      city: z.string().optional().describe('City name'),
      taxCategory: z
        .string()
        .optional()
        .default('standard')
        .describe(
          'Product tax category: standard, reduced, exempt, digital, food, clothing, medical',
        ),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { country, state, city, taxCategory } = params;
      const rate = await commerce.tax.getEffectiveRate(
        { country, state, city },
        taxCategory || 'standard',
      );
      return {
        success: true,
        address: { country, state, city },
        taxCategory: taxCategory || 'standard',
        effectiveRate: rate,
        effectiveRatePercent: (rate * 100).toFixed(2) + '%',
      };
    },
  },
  {
    name: 'list_tax_jurisdictions',
    description: 'List tax jurisdictions with optional filtering by country or level.',
    inputSchema: {
      countryCode: z.string().optional().describe('Filter by country code (e.g., US, DE, CA)'),
      level: z
        .string()
        .optional()
        .describe('Filter by level: country, state, county, city, district'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { countryCode, level } = params;
      const jurisdictions = await commerce.tax.listJurisdictions({
        countryCode,
        level,
        activeOnly: true,
      });
      return {
        success: true,
        count: jurisdictions.length,
        jurisdictions: jurisdictions.map((j) => ({
          id: j.id,
          code: j.code,
          name: j.name,
          level: j.level,
          countryCode: j.countryCode,
          stateCode: j.stateCode,
        })),
      };
    },
  },
  {
    name: 'list_tax_rates',
    description: 'List tax rates for a jurisdiction or all active rates.',
    inputSchema: {
      jurisdictionId: z.string().optional().describe('Filter by jurisdiction ID'),
      taxType: z
        .string()
        .optional()
        .describe('Filter by tax type: sales_tax, vat, gst, hst, pst, qst'),
      productCategory: z
        .string()
        .optional()
        .describe('Filter by product category: standard, reduced, exempt, digital'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { jurisdictionId, taxType, productCategory } = params;
      const rates = await commerce.tax.listRates({
        jurisdictionId,
        taxType,
        productCategory,
        activeOnly: true,
      });
      return {
        success: true,
        count: rates.length,
        rates: rates.map((r) => ({
          id: r.id,
          jurisdictionId: r.jurisdictionId,
          taxType: r.taxType,
          productCategory: r.productCategory,
          rate: r.rate,
          ratePercent: (r.rate * 100).toFixed(2) + '%',
          name: r.name,
          isCompound: r.isCompound,
          effectiveFrom: r.effectiveFrom,
        })),
      };
    },
  },
  {
    name: 'get_tax_settings',
    description: 'Get the store tax calculation settings.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const settings = await commerce.tax.getSettings();
      return {
        success: true,
        settings: {
          enabled: settings.enabled,
          calculationMethod: settings.calculationMethod,
          compoundMethod: settings.compoundMethod,
          taxShipping: settings.taxShipping,
          taxHandling: settings.taxHandling,
          defaultProductCategory: settings.defaultProductCategory,
          roundingMode: settings.roundingMode,
          decimalPlaces: settings.decimalPlaces,
          taxProvider: settings.taxProvider,
        },
      };
    },
  },
  {
    name: 'get_us_state_tax_info',
    description: 'Get pre-configured US state sales tax information including rates and rules.',
    inputSchema: { stateCode: z.string().describe('US state code (e.g., CA, TX, NY)') },
    permission: 'read',
    handler: async ({ params }) => {
      const { stateCode } = params;
      const stateInfo = US_STATE_TAX_INFO[stateCode.toUpperCase()];
      if (!stateInfo)
        return {
          success: false,
          error: `State ${stateCode} not found. Try: CA, TX, NY, FL, WA, OR, DE, MT, NH, AK`,
        };
      return {
        success: true,
        stateInfo: { ...stateInfo, stateRatePercent: (stateInfo.stateRate * 100).toFixed(2) + '%' },
      };
    },
  },
  {
    name: 'get_customer_tax_exemptions',
    description: 'Get active tax exemptions for a customer.',
    inputSchema: { customerId: z.string().describe('Customer ID') },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { customerId } = params;
      const exemptions = await commerce.tax.getCustomerExemptions(customerId);
      return {
        success: true,
        count: exemptions.length,
        exemptions: exemptions.map((e) => ({
          id: e.id,
          exemptionType: e.exemptionType,
          certificateNumber: e.certificateNumber,
          issuingAuthority: e.issuingAuthority,
          effectiveFrom: e.effectiveFrom,
          expiresAt: e.expiresAt,
          verified: e.verified,
        })),
      };
    },
  },
  {
    name: 'create_tax_exemption',
    description: 'Create a tax exemption certificate for a customer.',
    inputSchema: {
      customerId: z.string().describe('Customer ID'),
      exemptionType: z
        .string()
        .describe(
          'Type: resale, non_profit, government, educational, religious, medical, manufacturing, agricultural, export, diplomatic',
        ),
      certificateNumber: z.string().optional().describe('Exemption certificate number'),
      issuingAuthority: z.string().optional().describe('Issuing authority (e.g., state name)'),
      expiresAt: z.string().optional().describe('Expiration date (YYYY-MM-DD)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { customerId, exemptionType, certificateNumber, issuingAuthority, expiresAt } = params;
      if (!allowApply)
        return {
          error: 'Write operations require --apply flag. Would create tax exemption for customer.',
          preview: { customerId, exemptionType, certificateNumber, issuingAuthority },
        };
      const today = new Date().toISOString().split('T')[0];
      const exemption = await commerce.tax.createExemption({
        customerId,
        exemptionType,
        certificateNumber,
        issuingAuthority,
        effectiveFrom: today,
        expiresAt: expiresAt || null,
        jurisdictionIds: [],
        exemptCategories: [],
      });
      return {
        success: true,
        message: 'Tax exemption created for customer',
        exemption: {
          id: exemption.id,
          customerId: exemption.customerId,
          exemptionType: exemption.exemptionType,
          certificateNumber: exemption.certificateNumber,
          effectiveFrom: exemption.effectiveFrom,
        },
      };
    },
  },
  {
    name: 'calculate_cart_tax',
    description:
      'Calculate and apply tax to a cart based on its shipping address. Must set shipping address first. Returns tax breakdown and updates cart totals.',
    inputSchema: { cartId: z.string().describe('Cart ID to calculate tax for') },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { cartId } = params;
      const result = await commerce.calculateCartTax(cartId);
      return {
        success: true,
        cartId,
        tax: {
          subtotal: result.subtotal,
          totalTax: result.totalTax,
          total: result.total,
          taxInclusive: result.taxInclusive,
          breakdown:
            result.taxBreakdown?.map((b) => ({
              jurisdiction: b.jurisdictionName,
              rate: `${(b.rate * 100).toFixed(2)}%`,
              taxAmount: b.taxAmount,
            })) || [],
        },
        lineItems:
          result.lineItemTaxes?.map((item) => ({
            id: item.lineItemId,
            subtotal: item.subtotal,
            taxAmount: item.taxAmount,
            total: item.total,
          })) || [],
      };
    },
  },
];

export default taxTools;

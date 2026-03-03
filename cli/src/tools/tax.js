/**
 * Tax Calculation Tools Module
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';
import {
  calculateTaxQuote,
  calculateTaxQuoteWithFailover,
  commitTaxTransaction,
  evaluateTaxJurisdictionCompliance,
  getTaxQuote,
  getTaxTransaction,
  ingestTaxProviderWebhook,
  listTaxProviders,
  listTaxTransactions,
  voidTaxTransaction,
} from './providers/tax.js';

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
            id: z.string().min(1).describe('Line item identifier'),
            unitPrice: z.number().positive().describe('Unit price per item'),
            quantity: z.number().int().positive().describe('Quantity of items'),
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
          country: z.string().min(1).describe('Country code (e.g., US, DE, CA)'),
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
      country: z.string().min(1).describe('Country code (e.g., US, DE, CA)'),
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
    inputSchema: { stateCode: z.string().min(1).describe('US state code (e.g., CA, TX, NY)') },
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
    inputSchema: { customerId: z.string().min(1).describe('Customer ID') },
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
      customerId: z.string().min(1).describe('Customer ID'),
      exemptionType: z
        .string()
        .min(1)
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
          success: false,
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
    inputSchema: { cartId: z.string().min(1).describe('Cart ID to calculate tax for') },
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
  {
    name: 'list_tax_providers',
    description: 'List tax providers and capabilities for quote, commit, and void workflows.',
    inputSchema: {
      capability: z
        .string()
        .optional()
        .describe('Optional capability filter (e.g., quote, commit, exemptions)'),
      countryCode: z.string().optional().describe('Optional country code filter'),
      mode: z
        .enum(['sandbox', 'shadow', 'production'])
        .optional()
        .describe('Optional provider mode filter'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const providers = listTaxProviders({
        capability: params.capability,
        countryCode: params.countryCode,
        mode: params.mode,
      });
      return {
        success: true,
        count: providers.length,
        providers,
      };
    },
  },
  {
    name: 'validate_tax_jurisdiction_compliance',
    description:
      'Validate jurisdiction readiness for tax calculation (country/state/postal requirements and category checks).',
    inputSchema: {
      items: z
        .array(
          z.object({
            id: z.string().optional().describe('Line item identifier'),
            unitPrice: z.number().positive().describe('Unit price per item'),
            quantity: z.number().int().positive().describe('Quantity'),
            taxCategory: z
              .string()
              .optional()
              .default('standard')
              .describe('Tax category: standard, reduced, exempt, digital, food, medical'),
          }),
        )
        .min(1)
        .describe('Line items'),
      shippingAddress: z
        .object({
          country: z.string().min(1).describe('Country code'),
          state: z.string().optional().describe('State/province code'),
          city: z.string().optional().describe('City'),
          postalCode: z.string().optional().describe('Postal/ZIP code'),
        })
        .describe('Shipping address'),
      currency: z.string().max(10).optional().describe('Currency code (default: USD)'),
      strictCompliance: z
        .boolean()
        .optional()
        .describe('Treat missing required jurisdiction fields as errors'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const compliance = evaluateTaxJurisdictionCompliance({
        shippingAddress: params.shippingAddress,
        lineItems: params.items,
        currency: params.currency || 'USD',
        strict: params.strictCompliance ?? true,
      });
      return {
        success: true,
        compliance,
      };
    },
  },
  {
    name: 'calculate_tax_quote',
    description:
      'Calculate a provider-backed tax quote with deterministic replay-safe output and optional idempotency key.',
    inputSchema: {
      providerId: z.string().optional().describe('Tax provider ID (default: deterministic-mock)'),
      items: z
        .array(
          z.object({
            id: z.string().optional().describe('Line item identifier'),
            unitPrice: z.number().positive().describe('Unit price per item'),
            quantity: z.number().int().positive().describe('Quantity'),
            taxCategory: z
              .string()
              .optional()
              .default('standard')
              .describe('Tax category: standard, reduced, exempt, digital, food, medical'),
          }),
        )
        .min(1)
        .describe('Line items'),
      shippingAddress: z
        .object({
          country: z.string().min(1).describe('Country code'),
          state: z.string().optional().describe('State/province code'),
          city: z.string().optional().describe('City'),
          postalCode: z.string().optional().describe('Postal/ZIP code'),
        })
        .describe('Shipping address'),
      shippingAmount: z.number().min(0).optional().describe('Shipping amount'),
      customerId: z.string().optional().describe('Customer ID'),
      orderId: z.string().optional().describe('Order ID'),
      currency: z.string().max(10).optional().describe('Currency code (default: USD)'),
      taxExempt: z.boolean().optional().describe('Force customer exemption for this quote'),
      metadata: z.record(z.string(), z.any()).optional().describe('Additional metadata'),
      idempotencyKey: z
        .string()
        .max(255)
        .optional()
        .describe('Idempotency key for deterministic retries'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const result = calculateTaxQuote({
        providerId: params.providerId,
        lineItems: params.items,
        shippingAddress: params.shippingAddress,
        shippingAmount: params.shippingAmount || 0,
        customerId: params.customerId,
        orderId: params.orderId,
        currency: params.currency || 'USD',
        taxExempt: Boolean(params.taxExempt),
        metadata: params.metadata || {},
        idempotencyKey: params.idempotencyKey,
      });

      return {
        success: true,
        provider: result.provider,
        quote: result.quote,
        idempotent: result.idempotent,
      };
    },
  },
  {
    name: 'calculate_tax_quote_with_failover',
    description:
      'Calculate a tax quote with jurisdiction compliance validation and provider failover routing.',
    inputSchema: {
      providerId: z.string().optional().describe('Primary tax provider ID'),
      fallbackProviderIds: z
        .array(z.string().min(1))
        .optional()
        .describe('Ordered fallback provider IDs'),
      allowDeterministicFallback: z
        .boolean()
        .optional()
        .describe('Allow deterministic mock fallback when all providers fail'),
      strictCompliance: z
        .boolean()
        .optional()
        .describe('Require strict jurisdiction completeness checks'),
      items: z
        .array(
          z.object({
            id: z.string().optional().describe('Line item identifier'),
            unitPrice: z.number().positive().describe('Unit price per item'),
            quantity: z.number().int().positive().describe('Quantity'),
            taxCategory: z
              .string()
              .optional()
              .default('standard')
              .describe('Tax category: standard, reduced, exempt, digital, food, medical'),
          }),
        )
        .min(1)
        .describe('Line items'),
      shippingAddress: z
        .object({
          country: z.string().min(1).describe('Country code'),
          state: z.string().optional().describe('State/province code'),
          city: z.string().optional().describe('City'),
          postalCode: z.string().optional().describe('Postal/ZIP code'),
        })
        .describe('Shipping address'),
      shippingAmount: z.number().min(0).optional().describe('Shipping amount'),
      customerId: z.string().optional().describe('Customer ID'),
      orderId: z.string().optional().describe('Order ID'),
      currency: z.string().max(10).optional().describe('Currency code (default: USD)'),
      taxExempt: z.boolean().optional().describe('Force customer exemption for this quote'),
      metadata: z.record(z.string(), z.any()).optional().describe('Additional metadata'),
      idempotencyKey: z
        .string()
        .max(255)
        .optional()
        .describe('Idempotency key for deterministic retries'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const result = calculateTaxQuoteWithFailover({
        providerId: params.providerId,
        fallbackProviderIds: params.fallbackProviderIds || [],
        allowDeterministicFallback: params.allowDeterministicFallback ?? true,
        strictCompliance: params.strictCompliance ?? true,
        lineItems: params.items,
        shippingAddress: params.shippingAddress,
        shippingAmount: params.shippingAmount || 0,
        customerId: params.customerId,
        orderId: params.orderId,
        currency: params.currency || 'USD',
        taxExempt: Boolean(params.taxExempt),
        metadata: params.metadata || {},
        idempotencyKey: params.idempotencyKey,
      });

      return {
        success: true,
        provider: result.provider,
        quote: result.quote,
        idempotent: result.idempotent,
        failover: result.failover,
      };
    },
  },
  {
    name: 'get_tax_quote',
    description: 'Get a provider-backed tax quote by ID.',
    inputSchema: {
      quoteId: z.string().min(1).describe('Tax quote ID'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const quote = getTaxQuote(params.quoteId);
      if (!quote) {
        return { success: false, error: 'Tax quote not found' };
      }
      return {
        success: true,
        quote,
      };
    },
  },
  {
    name: 'commit_tax_transaction',
    description: 'Commit a previously calculated tax quote into a provider transaction record.',
    inputSchema: {
      quoteId: z.string().min(1).describe('Tax quote ID'),
      providerId: z.string().optional().describe('Provider ID override'),
      transactionReference: z.string().optional().describe('External transaction reference'),
      idempotencyKey: z.string().max(255).optional().describe('Idempotency key for safe retries'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Commit tax transaction', params);
      }

      if (!getTaxQuote(params.quoteId)) {
        return { success: false, error: 'Tax quote not found' };
      }

      const result = commitTaxTransaction({
        quoteId: params.quoteId,
        providerId: params.providerId,
        transactionReference: params.transactionReference,
        idempotencyKey: params.idempotencyKey,
      });

      return {
        success: true,
        message: result.idempotent
          ? 'Tax transaction reused via idempotency'
          : 'Tax transaction committed',
        quote: result.quote,
        transaction: result.transaction,
        idempotent: result.idempotent,
      };
    },
  },
  {
    name: 'get_tax_transaction',
    description: 'Get a provider-backed tax transaction by ID.',
    inputSchema: {
      transactionId: z.string().min(1).describe('Tax transaction ID'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const transaction = getTaxTransaction(params.transactionId);
      if (!transaction) {
        return { success: false, error: 'Tax transaction not found' };
      }
      return {
        success: true,
        transaction,
      };
    },
  },
  {
    name: 'list_tax_transactions',
    description: 'List provider-backed tax transactions with optional filtering.',
    inputSchema: {
      providerId: z.string().optional().describe('Filter by provider ID'),
      status: z
        .enum(['pending', 'committed', 'voided'])
        .optional()
        .describe('Filter by transaction status'),
      quoteId: z.string().optional().describe('Filter by quote ID'),
      reference: z.string().optional().describe('Filter by external reference'),
      limit: z.number().int().min(1).max(500).optional().describe('Maximum transactions to return'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const transactions = listTaxTransactions({
        providerId: params.providerId,
        status: params.status,
        quoteId: params.quoteId,
        reference: params.reference,
        limit: params.limit,
      });
      return {
        success: true,
        count: transactions.length,
        transactions,
      };
    },
  },
  {
    name: 'void_tax_transaction',
    description: 'Void a committed tax transaction with optional reason.',
    inputSchema: {
      transactionId: z.string().min(1).describe('Tax transaction ID'),
      reason: z.string().max(500).optional().describe('Void reason'),
      idempotencyKey: z.string().max(255).optional().describe('Idempotency key for safe retries'),
    },
    permission: 'delete',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Void tax transaction', params);
      }

      const result = voidTaxTransaction({
        transactionId: params.transactionId,
        reason: params.reason,
        idempotencyKey: params.idempotencyKey,
      });

      return {
        success: true,
        message: result.idempotent ? 'Tax transaction already voided' : 'Tax transaction voided',
        quote: result.quote,
        transaction: result.transaction,
        idempotent: result.idempotent,
      };
    },
  },
  {
    name: 'ingest_tax_provider_webhook',
    description:
      'Ingest a tax provider webhook event and reconcile quote/transaction state in shadow or production mode.',
    inputSchema: {
      providerId: z.string().optional().describe('Provider ID (default: deterministic-mock)'),
      eventType: z.string().min(1).describe('Webhook event type'),
      eventId: z
        .string()
        .optional()
        .describe('Optional provider event ID for idempotent ingestion'),
      payload: z
        .record(z.string(), z.any())
        .optional()
        .describe('Webhook payload object from provider'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Ingest tax provider webhook', params);
      }

      const result = ingestTaxProviderWebhook({
        providerId: params.providerId,
        eventType: params.eventType,
        eventId: params.eventId,
        payload: params.payload || {},
      });

      return {
        success: true,
        message: result.applied ? 'Tax webhook ingested' : 'Tax webhook processed with no mutation',
        webhook: result,
      };
    },
  },
];

export default taxTools;

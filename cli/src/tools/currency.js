/**
 * Currency & Exchange Rate Tools Module
 */

import { z } from 'zod';

export const currencyTools = [
  {
    name: 'get_exchange_rate',
    description: 'Get the exchange rate between two currencies.',
    inputSchema: {
      from: z.string().min(1).describe('Source currency code (e.g., USD, EUR, GBP)'),
      to: z.string().min(1).describe('Target currency code (e.g., EUR, USD, GBP)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { from, to } = params;
      const rate = await commerce.currency.getRate(from.toUpperCase(), to.toUpperCase());
      if (!rate) return { success: false, error: `No exchange rate found for ${from} to ${to}` };
      return {
        success: true,
        rate: {
          baseCurrency: rate.baseCurrency,
          quoteCurrency: rate.quoteCurrency,
          rate: rate.rate,
          source: rate.source,
          rateAt: rate.rateAt,
        },
      };
    },
  },
  {
    name: 'list_exchange_rates',
    description: 'List all available exchange rates, optionally filtered by base currency.',
    inputSchema: {
      baseCurrency: z.string().optional().describe('Filter by base currency code (e.g., USD)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { baseCurrency } = params;
      let rates;
      if (baseCurrency) {
        rates = await commerce.currency.getRatesFor(baseCurrency.toUpperCase());
      } else {
        rates = await commerce.currency.listRates();
      }
      return {
        success: true,
        count: rates.length,
        rates: rates.map((r) => ({
          baseCurrency: r.baseCurrency,
          quoteCurrency: r.quoteCurrency,
          rate: r.rate,
          source: r.source,
          rateAt: r.rateAt,
        })),
      };
    },
  },
  {
    name: 'convert_currency',
    description: 'Convert an amount from one currency to another using current exchange rates.',
    inputSchema: {
      from: z.string().min(1).describe('Source currency code (e.g., USD)'),
      to: z.string().min(1).describe('Target currency code (e.g., EUR)'),
      amount: z.number().positive().describe('Amount to convert'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { from, to, amount } = params;
      const result = await commerce.currency.convert({
        from: from.toUpperCase(),
        to: to.toUpperCase(),
        amount,
      });
      return {
        success: true,
        conversion: {
          originalAmount: result.originalAmount,
          originalCurrency: result.originalCurrency,
          convertedAmount: result.convertedAmount,
          targetCurrency: result.targetCurrency,
          rate: result.rate,
          inverseRate: result.inverseRate,
          rateAt: result.rateAt,
        },
      };
    },
  },
  {
    name: 'set_exchange_rate',
    description: 'Set or update an exchange rate between two currencies.',
    inputSchema: {
      baseCurrency: z.string().min(1).describe('Base currency code (e.g., USD)'),
      quoteCurrency: z.string().min(1).describe('Quote currency code (e.g., EUR)'),
      rate: z.number().describe('Exchange rate (e.g., 0.92 for USD to EUR)'),
      source: z
        .string()
        .optional()
        .default('manual')
        .describe('Source of the rate (e.g., manual, api)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { baseCurrency, quoteCurrency, rate, source } = params;
      if (!allowApply)
        return {
          success: false,
          error:
            'Write operations require --apply flag. Would set rate: 1 ' +
            baseCurrency +
            ' = ' +
            rate +
            ' ' +
            quoteCurrency,
          preview: { baseCurrency, quoteCurrency, rate, source },
        };
      const result = await commerce.currency.setRate({
        baseCurrency: baseCurrency.toUpperCase(),
        quoteCurrency: quoteCurrency.toUpperCase(),
        rate,
        source,
      });
      return {
        success: true,
        message: `Exchange rate set: 1 ${result.baseCurrency} = ${result.rate} ${result.quoteCurrency}`,
        rate: {
          id: result.id,
          baseCurrency: result.baseCurrency,
          quoteCurrency: result.quoteCurrency,
          rate: result.rate,
          source: result.source,
          rateAt: result.rateAt,
        },
      };
    },
  },
  {
    name: 'get_currency_settings',
    description: 'Get the store currency settings including base currency and enabled currencies.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const settings = await commerce.currency.getSettings();
      return {
        success: true,
        settings: {
          baseCurrency: settings.baseCurrency,
          enabledCurrencies: settings.enabledCurrencies,
          autoConvert: settings.autoConvert,
          roundingMode: settings.roundingMode,
        },
      };
    },
  },
  {
    name: 'set_base_currency',
    description: "Set the store's base currency.",
    inputSchema: {
      currency: z.string().min(1).describe('Currency code to set as base (e.g., USD, EUR)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { currency } = params;
      if (!allowApply)
        return {
          success: false,
          error: 'Write operations require --apply flag. Would set base currency to: ' + currency,
          preview: { baseCurrency: currency.toUpperCase() },
        };
      const settings = await commerce.currency.setBaseCurrency(currency.toUpperCase());
      return {
        success: true,
        message: `Base currency set to ${settings.baseCurrency}`,
        settings: {
          baseCurrency: settings.baseCurrency,
          enabledCurrencies: settings.enabledCurrencies,
        },
      };
    },
  },
  {
    name: 'enable_currencies',
    description: 'Enable currencies for the store.',
    inputSchema: {
      currencies: z
        .array(z.string())
        .describe('List of currency codes to enable (e.g., ["USD", "EUR", "GBP"])'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { currencies } = params;
      if (!allowApply)
        return {
          success: false,
          error:
            'Write operations require --apply flag. Would enable currencies: ' +
            currencies.join(', '),
          preview: { currencies: currencies.map((c) => c.toUpperCase()) },
        };
      const settings = await commerce.currency.enableCurrencies(
        currencies.map((c) => c.toUpperCase()),
      );
      return {
        success: true,
        message: `Enabled currencies: ${settings.enabledCurrencies.join(', ')}`,
        settings: {
          baseCurrency: settings.baseCurrency,
          enabledCurrencies: settings.enabledCurrencies,
        },
      };
    },
  },
  {
    name: 'format_currency',
    description: 'Format an amount with currency symbol.',
    inputSchema: {
      amount: z.number().describe('Amount to format'),
      currency: z.string().min(1).describe('Currency code (e.g., USD, EUR)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { amount, currency } = params;
      const formatted = await commerce.currency.format(amount, currency.toUpperCase());
      return { success: true, amount, currency: currency.toUpperCase(), formatted };
    },
  },
];

export default currencyTools;
